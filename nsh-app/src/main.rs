//! 技术选型基准 · NSH TSX 版：GPUI 宿主 + QuickJS 里的 TSX（app/）。
//! 行情批次模拟批量转发路径：宿主序列化成 JSON → Callback.post → JS 解析写 signal。
//! JS 写完 signal 后调 Bench.applied(seq)，宿主据此计算端到端延迟；帧指标来自 GPUI profiler。
//!
//! BENCH_MODE=hybrid：同一个二进制跑"TSX 界面 + Rust 模块"路线。行情在 Rust 侧合并（同一批内同一标的只保留最新值）
//! 并格式化成显示用字符串；计算、过滤、详情字段也在 Rust 里做，JS 只负责写 signal 和渲染。

mod native_host;
mod rustui;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use bench_core::{Batch, Driver};
use futures::StreamExt;
use nsh_core::ui::prelude::*;
use nsh_platform_gpui::gpui::{self, AppContext as _, Bounds, Focusable as _, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use nsh_platform_gpui::{App as NshApp, AppOptions, AppView, Bundle};

static DRIVER: OnceLock<Arc<Driver>> = OnceLock::new();

/// 诊断：按线程计数的分配器（次数、字节），用于统计某段代码的分配量
struct CountingAlloc;
thread_local! {
    static ALLOCS: std::cell::Cell<(u64, u64)> = const { std::cell::Cell::new((0, 0)) };
}
unsafe impl std::alloc::GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| {
            let (n, b) = c.get();
            c.set((n + 1, b + l.size() as u64));
        });
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, new: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| {
            let (n, b) = c.get();
            c.set((n + 1, b + new as u64));
        });
        unsafe { std::alloc::System.realloc(p, l, new) }
    }
}
#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

/// 本线程累计分配（次数, 字节）
pub(crate) fn alloc_count() -> (u64, u64) {
    ALLOCS.with(|c| c.get())
}
static RX: std::sync::Mutex<Option<futures::channel::mpsc::UnboundedReceiver<Batch>>> = std::sync::Mutex::new(None);
/// hybrid：合并好的显示包（JSON）
static HRX: std::sync::Mutex<Option<futures::channel::mpsc::UnboundedReceiver<(u32, String)>>> = std::sync::Mutex::new(None);

/// BENCH_MODE=rustui：界面用 NSH 的 Rust 写法（`nsh_core::ui`），没有 QuickJS 参与
fn rustui_mode() -> bool {
    std::env::var("BENCH_MODE").is_ok_and(|m| m == "rustui")
}

fn hybrid_mode() -> bool {
    std::env::var("BENCH_MODE").is_ok_and(|m| m == "hybrid")
}

/// BENCH_BACKEND=gpui：NSH 跑在 nsh-platform-gpui 上（对照）；默认用本平台的原生后端
/// （Linux：nsh-platform-linux / GTK 4；macOS：nsh-platform-macos / AppKit）
fn gpui_backend() -> bool {
    std::env::var("BENCH_BACKEND").is_ok_and(|b| b == "gpui")
}

/// BENCH_TRACE=1：记录行情链路各段耗时（JS 侧会多调用几次宿主方法，只用于诊断）
pub(crate) fn trace() -> bool {
    static T: OnceLock<bool> = OnceLock::new();
    *T.get_or_init(|| std::env::var("BENCH_TRACE").is_ok_and(|v| v == "1"))
}

pub(crate) fn span(name: &str, ms: f64) {
    if let Some(d) = DRIVER.get() {
        d.metrics.record_span(name, ms);
    }
}

pub(crate) fn ms_since(t: std::time::Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// 行情批次在驱动线程生成到被引擎任务取走：已发出 / 已取走的批次数（队列深度）
static SENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static TAKEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// 已 post 给 JS、尚未回报的包：seq / 包 id → post 时刻
static POSTED: std::sync::Mutex<Option<HashMap<u64, std::time::Instant>>> = std::sync::Mutex::new(None);

/// 引擎任务取到一批：记生成 → 取走的等待与队列深度
pub(crate) fn trace_taken(emit_ns: u64) {
    let taken = TAKEN.fetch_add(1, Ordering::Relaxed) + 1;
    if trace() {
        let emit = bench_core::origin() + Duration::from_nanos(emit_ns);
        span("q.emit_to_engine", ms_since(emit));
        span("q.depth", (SENT.load(Ordering::Relaxed).saturating_sub(taken)) as f64);
    }
}

fn trace_posted(id: u64) {
    if trace() {
        POSTED.lock().unwrap().get_or_insert_default().insert(id, std::time::Instant::now());
    }
}

// ---------------------------------------------------------------------------
// hybrid：Rust 侧的行情合并与格式化
// ---------------------------------------------------------------------------

/// 一行的显示字段：代码 名称 最新价 涨跌 涨跌幅 成交量 成交额 最高 最低 买一/卖一
fn row_view(r: &bench_core::Row) -> (Vec<String>, i8) {
    let chg = r.last - r.prev;
    let sign = if chg > 0.0 { "+" } else { "" };
    let color = if chg > 0.0 { 1 } else if chg < 0.0 { -1 } else { 0 };
    (
        vec![
            r.code.clone(),
            r.name.clone(),
            format!("{:.3}", r.last),
            format!("{sign}{chg:.3}"),
            format!("{sign}{:.2}%", chg / r.prev * 100.0),
            r.vol.to_string(),
            format!("{:.0}", r.turnover),
            format!("{:.3}", r.high),
            format!("{:.3}", r.low),
            format!("{:.3} / {:.3}", r.bid, r.ask),
        ],
        color,
    )
}

#[derive(Default)]
struct Agg {
    /// 已发给 JS 的最新价（算闪烁方向）
    shown: Vec<f64>,
    rows: HashMap<u32, bench_core::Row>,
    bids: Option<Vec<bench_core::Level>>,
    asks: Option<Vec<bench_core::Level>>,
    trades: VecDeque<bench_core::Trade>,
    trades_dirty: bool,
    seqs: Vec<u64>,
    latest: Vec<bench_core::Row>,
}

static AGG: std::sync::Mutex<Option<Agg>> = std::sync::Mutex::new(None);
/// 包 id → 其中包含的批次 seq（JS 应用完一包后统一记延迟）
static POSTS: std::sync::Mutex<Option<HashMap<u32, Vec<u64>>>> = std::sync::Mutex::new(None);

fn agg_push(b: Batch) {
    let mut g = AGG.lock().unwrap();
    let a = g.as_mut().unwrap();
    for r in b.rows {
        a.latest[r.idx as usize] = r.clone();
        a.rows.insert(r.idx, r);
    }
    if b.bids.is_some() {
        a.bids = b.bids;
        a.asks = b.asks;
    }
    if !b.trades.is_empty() {
        for t in b.trades {
            a.trades.push_front(t);
        }
        a.trades.truncate(bench_core::MAX_TRADES);
        a.trades_dirty = true;
    }
    a.seqs.push(b.seq);
}

/// 取出攒下的变化，生成一包显示数据
fn agg_flush(id: u32) -> Option<String> {
    let mut g = AGG.lock().unwrap();
    let a = g.as_mut().unwrap();
    if a.seqs.is_empty() {
        return None;
    }
    let rows: Vec<serde_json::Value> = a
        .rows
        .drain()
        .map(|(i, r)| {
            let prev = std::mem::replace(&mut a.shown[i as usize], r.last);
            let flash = if r.last > prev { 1 } else if r.last < prev { -1 } else { 0 };
            let (s, c) = row_view(&r);
            serde_json::json!([i, s, c, flash])
        })
        .collect();
    let depth = |l: &Option<Vec<bench_core::Level>>, max: u64| {
        l.as_ref().map(|v| {
            v.iter()
                .map(|x| serde_json::json!([format!("{:.3}", x.price), x.vol.to_string(), format!("({})", x.orders), x.vol as f64 / max as f64 * 100.0]))
                .collect::<Vec<_>>()
        })
    };
    let max = a.bids.iter().chain(a.asks.iter()).flatten().map(|l| l.vol).max().unwrap_or(1).max(1);
    let bids = depth(&a.bids.take(), max);
    let asks = depth(&a.asks.take(), max);
    let trades = if std::mem::take(&mut a.trades_dirty) {
        Some(
            a.trades
                .iter()
                .take(40)
                .map(|t| serde_json::json!([(t.time_ms / 1000).to_string(), format!("{:.3}", t.price), t.vol.to_string(), t.dir]))
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };
    POSTS.lock().unwrap().get_or_insert_default().insert(id, std::mem::take(&mut a.seqs));
    Some(serde_json::json!({ "id": id, "rows": rows, "bids": bids, "asks": asks, "trades": trades }).to_string())
}

struct Calc {
    items: Vec<bench_core::Item>,
    closes: Vec<f64>,
    json: String,
    items50k: Vec<bench_core::Item>,
}
fn calc() -> &'static Calc {
    static C: OnceLock<Calc> = OnceLock::new();
    C.get_or_init(|| Calc {
        items: bench_core::items(100_000),
        closes: bench_core::closes(100_000),
        json: bench_core::big_json(33_000),
        items50k: bench_core::items(50_000),
    })
}

#[module]
pub struct Bench;

#[module]
impl Bench {
    /// 把每批行情（JSON）交给 JS，直到压测结束
    pub async fn start(on_batch: Callback<String>) {
        let Some(mut rx) = RX.lock().unwrap().take() else { return };
        while let Some(b) = rx.next().await {
            trace_taken(b.emit_ns);
            let t = std::time::Instant::now();
            let json = serde_json::to_string(&b).unwrap_or_default();
            if trace() {
                span("tsx.ser", ms_since(t));
                span("tsx.kb", json.len() as f64 / 1024.0);
                trace_posted(b.seq);
            }
            on_batch.post(json);
        }
    }
    /// hybrid：转发 Rust 合并好的显示包（JSON），直到压测结束
    pub async fn start_view(on_view: Callback<String>) {
        let Some(mut rx) = HRX.lock().unwrap().take() else { return };
        while let Some((id, v)) = rx.next().await {
            trace_posted(id as u64);
            on_view.post(v);
        }
    }
    /// BENCH_TRACE：是否记录分段耗时
    pub fn trace() -> bool {
        trace()
    }
    /// BENCH_TRACE：JS 侧一包的解析与写 signal 耗时（ms）；宿主据 post 时刻算出排队等待
    pub fn trace_js(id: u32, parse: f64, apply: f64) {
        let posted = POSTED.lock().unwrap().get_or_insert_default().remove(&(id as u64));
        span("js.parse", parse);
        span("js.batch", apply);
        if let Some(t) = posted {
            span("js.post_to_done", ms_since(t));
            span("js.wait", (ms_since(t) - parse - apply).max(0.0));
        }
    }
    /// BENCH_TRACE：JS 侧任意分段耗时（ms）
    pub fn span(name: String, ms: f64) {
        span(&name, ms);
    }
    /// hybrid：JS 已把这一包写进 signal
    pub fn applied_post(id: u32) {
        let seqs = POSTS.lock().unwrap().get_or_insert_default().remove(&id).unwrap_or_default();
        if let Some(d) = DRIVER.get() {
            for s in seqs {
                d.metrics.on_applied(s);
            }
        }
    }
    /// 是否为 hybrid 模式
    pub fn hybrid() -> bool {
        hybrid_mode()
    }
    /// hybrid：初始 500 行的显示字段（JSON：[[字段...], 颜色]）
    pub fn initial_view() -> String {
        let rows: Vec<_> = bench_core::Feed::new(42).rows.iter().map(row_view).collect();
        serde_json::to_string(&rows).unwrap_or_default()
    }
    /// hybrid：选中行的 40 个详情字段（JSON：[[名, 值]...]）
    pub fn detail(idx: u32) -> String {
        let r = {
            let g = AGG.lock().unwrap();
            g.as_ref().unwrap().latest[idx as usize].clone()
        };
        let (s, _) = row_view(&r);
        let mut kv: Vec<(String, String)> = vec![
            ("代码".into(), s[0].clone()), ("名称".into(), s[1].clone()), ("最新价".into(), s[2].clone()), ("昨收".into(), format!("{:.3}", r.prev)),
            ("涨跌".into(), s[3].clone()), ("涨跌幅".into(), s[4].clone()), ("最高".into(), s[7].clone()), ("最低".into(), s[8].clone()),
            ("买一".into(), format!("{:.3}", r.bid)), ("卖一".into(), format!("{:.3}", r.ask)), ("成交量".into(), s[5].clone()), ("成交额".into(), s[6].clone()),
        ];
        for k in 0..28 {
            kv.push((format!("指标{k:02}"), format!("{:.4}", r.last * (k + 1) as f64 / 7.0)));
        }
        serde_json::to_string(&kv).unwrap_or_default()
    }
    /// hybrid：在 Rust 里执行计算任务（数据常驻 Rust），返回校验值
    pub fn task(name: String) -> f64 {
        let c = calc();
        match name.as_str() {
            "sort_100k" => bench_core::task_sort(&c.items),
            "agg_100k" => bench_core::task_agg(&c.items),
            "ema_100k" => bench_core::task_ema(&c.closes),
            _ => bench_core::task_json(&c.json) as f64,
        }
    }
    /// compute_ab：空调用（测跨语言调用开销）
    pub fn noop() {}
    /// compute_ab：带一个字符串参数、返回数字的空调用
    pub fn noop_str(name: String) -> f64 {
        name.len() as f64
    }
    /// compute_ab：执行计算任务，返回 Rust 内部计时（ms）；JS 外层计时减去它即为调用开销
    pub fn task_timed(name: String) -> f64 {
        let c = calc();
        let f0 = bench_core::thread_minflt();
        let (a0, _) = alloc_count();
        let t = std::time::Instant::now();
        let x = match name.as_str() {
            "sort_100k" => bench_core::task_sort(std::hint::black_box(&c.items)),
            "agg_100k" => bench_core::task_agg(std::hint::black_box(&c.items)),
            "ema_100k" => bench_core::task_ema(std::hint::black_box(&c.closes)),
            _ => bench_core::task_json(std::hint::black_box(&c.json)) as f64,
        };
        std::hint::black_box(x);
        let ms = ms_since(t);
        span(&format!("hy.{name}.minflt"), (bench_core::thread_minflt() - f0) as f64);
        span(&format!("hy.{name}.allocs"), (alloc_count().0 - a0) as f64);
        ms
    }
    /// hybrid：在 Rust 里过滤 5 万条，返回 { total, rows: [[代码, 名称, 价格, 涨跌幅]] }
    pub fn filter(term: String) -> String {
        let c = calc();
        let (hits, total) = bench_core::filter(&c.items50k, &term);
        let rows: Vec<_> = hits
            .iter()
            .map(|&i| {
                let it = &c.items50k[i];
                let sign = if it.chg_pct > 0.0 { "+" } else { "" };
                serde_json::json!([it.code, it.name, format!("{:.3}", it.price), format!("{sign}{:.2}%", it.chg_pct)])
            })
            .collect();
        serde_json::json!({ "total": total, "rows": rows }).to_string()
    }
    /// JS 已把 seq 这一批写进 signal
    pub fn applied(seq: u32) {
        if let Some(d) = DRIVER.get() {
            d.metrics.on_applied(seq as u64);
        }
    }
    /// 当前阶段名
    pub fn phase() -> String {
        DRIVER.get().map(|d| bench_core::schedule()[d.phase.load(Ordering::Relaxed)].name.to_string()).unwrap_or_default()
    }
    /// 当前阶段是否要求自动滚动
    pub fn scroll() -> bool {
        DRIVER.get().is_some_and(|d| d.scroll.load(Ordering::Relaxed))
    }
    /// 当前阶段 { name, kind, param, scenario }（JSON）
    pub fn phase_info() -> String {
        let Some(d) = DRIVER.get() else { return "{}".into() };
        let p = &bench_core::schedule()[d.phase.load(Ordering::Relaxed)];
        serde_json::json!({ "name": p.name, "kind": p.kind, "param": p.param, "scenario": bench_core::scenario() }).to_string()
    }
    /// 交互动作已触发（JS 改完状态前调用），之后第一帧算作它的响应
    pub fn mark() {
        if let Some(d) = DRIVER.get() {
            d.metrics.on_trigger();
        }
    }
    /// 切页签已触发：分别统计切入详情 / 切入列表的触发→上屏
    pub fn mark_tab(to_detail: bool) {
        if let Some(d) = DRIVER.get() {
            d.metrics.on_trigger_labeled(Some(if to_detail { "to_detail" } else { "to_list" }));
        }
    }
    /// 一次计算任务的耗时（JS 用 performance.now() 计）
    pub fn compute(task: String, ms: f64) {
        if let Some(d) = DRIVER.get() {
            d.metrics.record_compute(&task, ms);
        }
    }
    /// 约 5MB 的 JSON（与 Rust 版同一份数据），供 JSON.parse 计时
    pub fn big_json() -> String {
        bench_core::big_json(33_000)
    }
    /// 初始 500 行（JSON），与 Rust 版同一个种子
    pub fn initial() -> String {
        serde_json::to_string(&bench_core::Feed::new(42).rows).unwrap_or_default()
    }
}

fn registry() -> Registry {
    let mut r = Registry::ui_set("host/bench");
    r.module::<Bench>();
    r
}

fn main() {
    let _ = bench_core::origin();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let suffix = if gpui_backend() { "-gpui" } else { "" };

    let driver = if hybrid_mode() {
        let feed = bench_core::Feed::new(42);
        *AGG.lock().unwrap() = Some(Agg { shown: feed.rows.iter().map(|r| r.last).collect(), latest: feed.rows.clone(), ..Default::default() });
        let (htx, hrx) = futures::channel::mpsc::unbounded::<(u32, String)>();
        *HRX.lock().unwrap() = Some(hrx);
        // 节奏与 TSX 版一致：每批行情（10ms）到达即合并、格式化后发给 JS，不额外攒批
        let id = std::sync::atomic::AtomicU32::new(0);
        let sink = move |b: Batch| {
            let t = std::time::Instant::now();
            agg_push(b);
            let t1 = std::time::Instant::now();
            let n = id.fetch_add(1, Ordering::Relaxed);
            let v = agg_flush(n);
            if trace() {
                span("hy.push", (t1 - t).as_secs_f64() * 1000.0);
                span("hy.flush", ms_since(t1));
                if let Some(v) = &v {
                    span("hy.kb", v.len() as f64 / 1024.0);
                }
            }
            if let Some(v) = v {
                let _ = htx.unbounded_send((n, v));
            }
        };
        // 计算用的数据在后台预备，不占启动时间（TSX 版同样是首次用到时才生成）
        std::thread::spawn(|| {
            let _ = calc();
        });
        Driver::start(&format!("nsh-hybrid{suffix}"), sink)
    } else {
        let (tx, rx) = futures::channel::mpsc::unbounded::<Batch>();
        *RX.lock().unwrap() = Some(rx);
        Driver::start(&format!("{}{suffix}", if rustui_mode() { "nsh-rust" } else { "nsh-tsx" }), move |b| {
            SENT.fetch_add(1, Ordering::Relaxed);
            let _ = tx.unbounded_send(b);
        })
    };
    let _ = DRIVER.set(driver.clone());
    // 诊断：NSH 内部计数器（引擎 / core / UI 线程）与提交版本探针
    driver.metrics.set_counters(nsh_core::diag::snapshot);
    driver.metrics.set_commit_probe(bench_core::CommitProbe {
        submitted: || nsh_core::diag::get(nsh_core::diag::C::eng_submits),
        ui_applied_at: nsh_core::diag::ui_applied_at,
    });

    let bundle_path: PathBuf = std::env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| "out/app".into());
    let bundle_path = if bundle_path.is_dir() { bundle_path.join("bundle.nshc") } else { bundle_path };

    if gpui_backend() {
        run_gpui(driver, bundle_path);
    } else {
        native_host::run(driver, bundle_path, registry());
    }
}

/// nsh-platform-gpui 宿主：帧指标来自 GPUI profiler
fn run_gpui(driver: Arc<Driver>, bundle_path: PathBuf) {
    gpui::profiler::set_trace_enabled(true);
    nsh_platform_gpui::set_ui_sets(vec![nsh_ui_mobile::registry(), registry()]);
    gpui_platform::application().run(move |cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1600.), px(1000.)), cx))),
            titlebar: Some(TitlebarOptions { title: Some("bench · NSH TSX".into()), ..Default::default() }),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            let app = if rustui_mode() {
                NshApp::Rust(nsh_platform_gpui::Root::of::<rustui::BenchRoot>())
            } else {
                // SAFETY: 本工程自己的构建产物
                NshApp::Js(unsafe { Bundle::read_trusted(&bundle_path) }.expect("bundle"))
            };
            let view = cx.new(|cx| AppView::new(AppOptions::default(), window, cx));
            view.update(cx, |v, _| {
                v.on_error(|m| log::error!("app: {m}"));
                v.load(app);
            });
            window.focus(&view.focus_handle(cx), cx);
            view
        })
        .expect("window");
        cx.activate(true);

        // 帧指标：每 50ms 收一次 profiler 事件；压测结束后退出
        let d = driver.clone();
        cx.spawn(async move |cx| {
            let mut collector = gpui::profiler::FrameTimingCollector::new();
            loop {
                cx.background_executor().timer(Duration::from_millis(50)).await;
                for e in collector.collect_unseen() {
                    match e {
                        gpui::profiler::FrameEvent::Draw(t) => {
                            if let Some(r) = t.refresh_interval {
                                d.metrics.set_refresh(r);
                            }
                            d.metrics.on_draw(t.draw_start, t.draw_end, t.work_start());
                        }
                        gpui::profiler::FrameEvent::Present(p) => d.metrics.on_present(p.present_end),
                    }
                }
                if d.done.load(Ordering::Relaxed) {
                    cx.background_executor().timer(Duration::from_millis(300)).await;
                    let _ = cx.update(|cx| cx.quit());
                    break;
                }
            }
        })
        .detach();
    });
}
