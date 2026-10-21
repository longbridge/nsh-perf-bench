//! 技术选型基准 · 纯 Rust 版（GPUI Fast + gpui-kit）。
//! 场景由 BENCH_SCENARIO 选择（见 bench-core::schedule）：
//! - quotes：500 行行情表（虚拟列表、变动闪烁）+ 10 档盘口 + 逐笔，按阶段提高推送频率，含自动滚动；
//! - animation：N 个方块逐帧位移 + 透明度动画；
//! - interaction：选行 → 详情面板刷新、切换重页签、5 万行搜索过滤，逐次计「触发 → 上屏」；
//! - compute：10 万行排序 / 汇总 / 指标 / 5MB JSON 解析的耗时，以及推送中做重计算对帧的影响。

use bench_core::{Batch, Driver, Item, Level, Row, Trade, MAX_TRADES};
use futures::StreamExt;
use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const ROW_H: f32 = 22.;
const FLASH: Duration = Duration::from_millis(600);

fn up() -> Hsla {
    rgb(0x20a16e).into()
}
fn down() -> Hsla {
    rgb(0xcc3333).into()
}

struct Bench {
    rows: Vec<Row>,
    changed: Vec<Option<(Instant, i8)>>,
    bids: Vec<Level>,
    asks: Vec<Level>,
    trades: VecDeque<Trade>,
    scroll: UniformListScrollHandle,
    scroll_ix: usize,
    driver: Arc<Driver>,
    batches: u64,
    // 动画
    anim_t0: Instant,
    // 交互
    selected: usize,
    tab: usize,
    term: &'static str,
    hits: Vec<usize>,
    total: usize,
    items: Arc<Vec<Item>>,
}

impl Bench {
    fn apply(&mut self, b: Batch) {
        let now = Instant::now();
        for r in b.rows {
            let i = r.idx as usize;
            let dir = if r.last > self.rows[i].last { 1 } else if r.last < self.rows[i].last { -1 } else { 0 };
            self.changed[i] = Some((now, dir));
            self.rows[i] = r;
        }
        if let Some(b) = b.bids {
            self.bids = b;
        }
        if let Some(a) = b.asks {
            self.asks = a;
        }
        for t in b.trades {
            self.trades.push_front(t);
        }
        self.trades.truncate(MAX_TRADES);
        self.batches += 1;
    }

    fn kind(&self) -> &'static str {
        bench_core::schedule()[self.driver.phase.load(Ordering::Relaxed)].kind
    }
    fn param(&self) -> u32 {
        bench_core::schedule()[self.driver.phase.load(Ordering::Relaxed)].param
    }
}

fn cell(text: impl Into<SharedString>, w: f32, right: bool, color: Hsla) -> Div {
    let d = div().w(px(w)).px_1().overflow_hidden().text_color(color).child(text.into());
    if right { d.text_right() } else { d }
}

/// 详情面板：选中行的 40 个字段
fn detail(r: &Row, fg: Hsla, muted: Hsla) -> Div {
    let chg = r.last - r.prev;
    let mut kv: Vec<(String, String)> = vec![
        ("代码".into(), r.code.clone()),
        ("名称".into(), r.name.clone()),
        ("最新价".into(), format!("{:.3}", r.last)),
        ("昨收".into(), format!("{:.3}", r.prev)),
        ("涨跌".into(), format!("{:+.3}", chg)),
        ("涨跌幅".into(), format!("{:+.2}%", chg / r.prev * 100.)),
        ("最高".into(), format!("{:.3}", r.high)),
        ("最低".into(), format!("{:.3}", r.low)),
        ("买一".into(), format!("{:.3}", r.bid)),
        ("卖一".into(), format!("{:.3}", r.ask)),
        ("成交量".into(), format!("{}", r.vol)),
        ("成交额".into(), format!("{:.0}", r.turnover)),
    ];
    for k in 0..28 {
        kv.push((format!("指标{k:02}"), format!("{:.4}", r.last * (k as f64 + 1.0) / 7.0)));
    }
    div().flex().flex_wrap().p_2().children(kv.into_iter().map(|(k, v)| {
        div()
            .w(relative(0.5))
            .h(px(ROW_H))
            .flex()
            .items_center()
            .justify_between()
            .pr_3()
            .text_xs()
            .child(div().text_color(muted).child(k))
            .child(div().text_color(fg).child(v))
    }))
}

impl Render for Bench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (fg, muted, border, bg) = (theme.foreground, theme.muted_foreground, theme.border, theme.background);
        let kind = self.kind();
        let phase = bench_core::schedule()[self.driver.phase.load(Ordering::Relaxed)].name;

        let body: AnyElement = match kind {
            "anim_js" | "anim_native" => {
                // 逐帧：每个方块按正弦位移，透明度随相位变化
                window.request_animation_frame();
                let t = self.anim_t0.elapsed().as_secs_f32();
                let n = self.param() as usize;
                div()
                    .relative()
                    .size_full()
                    .overflow_hidden()
                    .children((0..n).map(|i| {
                        let ph = t * std::f32::consts::TAU * 0.5 + i as f32 * 0.1;
                        let x = (ph.sin() * 0.5 + 0.5) * 1400.;
                        let y = (i % 40) as f32 * 22. + 4.;
                        let a = 0.3 + 0.7 * ph.cos().powi(2);
                        let c = if i % 2 == 0 { up() } else { down() };
                        div().absolute().left(px(x)).top(px(y)).size(px(16.)).rounded(px(2.)).bg(c.opacity(a))
                    }))
                    .into_any_element()
            }
            "tab" => {
                let tabs = div().flex().h(px(28.)).items_center().gap_4().px_2().border_b_1().border_color(border).children(
                    ["持仓明细", "账户概览"]
                        .iter()
                        .enumerate()
                        .map(|(i, t)| div().text_xs().text_color(if self.tab == i { fg } else { muted }).child(*t)),
                );
                let content = if self.tab == 0 {
                    // 150 行非虚拟列表：切换时整块挂载 / 卸载
                    div().flex().flex_col().children(self.rows.iter().take(150).map(|r| {
                        div()
                            .flex()
                            .h(px(ROW_H))
                            .items_center()
                            .text_xs()
                            .border_b_1()
                            .border_color(border.opacity(0.5))
                            .child(cell(r.code.clone(), 80., false, fg))
                            .child(cell(r.name.clone(), 110., false, muted))
                            .child(cell(format!("{:.3}", r.last), 80., true, fg))
                            .child(cell(format!("{}", r.vol), 100., true, fg))
                            .child(cell(format!("{:.0}", r.turnover), 110., true, fg))
                            .child(cell(format!("{:.3}", r.high), 80., true, fg))
                    }))
                } else {
                    detail(&self.rows[0], fg, muted)
                };
                div().flex().flex_col().size_full().overflow_hidden().child(tabs).child(content).into_any_element()
            }
            "filter" => {
                let view = cx.entity().clone();
                let list = uniform_list("hits", self.hits.len(), move |range, _, cx| {
                    let this = view.read(cx);
                    range
                        .map(|k| {
                            let it = &this.items[this.hits[k]];
                            div()
                                .id(k)
                                .flex()
                                .h(px(ROW_H))
                                .items_center()
                                .text_xs()
                                .child(cell(it.code.clone(), 80., false, fg))
                                .child(cell(it.name.clone(), 140., false, muted))
                                .child(cell(format!("{:.3}", it.price), 80., true, fg))
                                .child(cell(format!("{:+.2}%", it.chg_pct), 80., true, fg))
                        })
                        .collect()
                })
                .flex_1();
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .child(
                        div()
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .px_2()
                            .gap_3()
                            .text_xs()
                            .border_b_1()
                            .border_color(border)
                            .child(div().px_2().border_1().border_color(border).child(format!("搜索：{}", self.term)))
                            .child(div().text_color(muted).child(format!("共 {} 条，显示前 100", self.total))),
                    )
                    .child(list)
                    .into_any_element()
            }
            _ => self.quotes_view(window, cx, fg, muted, border, bg, kind == "select").into_any_element(),
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(bg)
            .text_color(fg)
            .child(div().flex_1().min_h_0().child(body))
            .child(
                div()
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .px_2()
                    .gap_4()
                    .text_xs()
                    .text_color(muted)
                    .border_t_1()
                    .border_color(border)
                    .child(format!("Rust · {} · 阶段 {phase}", bench_core::scenario()))
                    .child(format!("批次 {}", self.batches)),
            )
    }
}

impl Bench {
    #[allow(clippy::too_many_arguments)]
    fn quotes_view(&mut self, window: &mut Window, cx: &mut Context<Self>, fg: Hsla, muted: Hsla, border: Hsla, bg: Hsla, with_detail: bool) -> Div {
        if self.driver.scroll.load(Ordering::Relaxed) {
            self.scroll_ix = (self.scroll_ix + 1) % bench_core::SYMBOLS;
            self.scroll.scroll_to_item_strict(self.scroll_ix, ScrollStrategy::Top);
            window.request_animation_frame();
        }
        let header = div().flex().h(px(24.)).items_center().text_xs().text_color(muted).border_b_1().border_color(border).children(
            [
                ("代码", 80., false),
                ("名称", 110., false),
                ("最新价", 80., true),
                ("涨跌", 70., true),
                ("涨跌幅", 70., true),
                ("成交量", 100., true),
                ("成交额", 110., true),
                ("最高", 80., true),
                ("最低", 80., true),
                ("买一/卖一", 130., true),
            ]
            .map(|(t, w, r)| cell(t, w, r, muted)),
        );
        let rows_view = cx.entity().clone();
        let table = uniform_list("quotes", self.rows.len(), move |range, _, cx| {
            let this = rows_view.read(cx);
            let now = Instant::now();
            let select = this.kind() == "select";
            range
                .map(|i| {
                    let r = &this.rows[i];
                    let chg = r.last - r.prev;
                    let c = if chg > 0. { up() } else if chg < 0. { down() } else { fg };
                    let flash = match this.changed[i] {
                        Some((t, d)) if now - t < FLASH && d != 0 => Some(if d > 0 { up().opacity(0.45) } else { down().opacity(0.45) }),
                        _ => None,
                    };
                    let price = cell(format!("{:.3}", r.last), 80., true, c);
                    let price = if let Some(f) = flash { price.bg(f) } else { price };
                    let row = div().id(i).flex().h(px(ROW_H)).items_center().text_xs().border_b_1().border_color(border.opacity(0.5));
                    let row = if select && this.selected == i { row.bg(Hsla::from(rgb(0x37a0ff)).opacity(0.14)) } else { row };
                    row.child(cell(r.code.clone(), 80., false, fg))
                        .child(cell(r.name.clone(), 110., false, muted))
                        .child(price)
                        .child(cell(format!("{:+.3}", chg), 70., true, c))
                        .child(cell(format!("{:+.2}%", chg / r.prev * 100.), 70., true, c))
                        .child(cell(format!("{}", r.vol), 100., true, fg))
                        .child(cell(format!("{:.0}", r.turnover), 110., true, fg))
                        .child(cell(format!("{:.3}", r.high), 80., true, fg))
                        .child(cell(format!("{:.3}", r.low), 80., true, fg))
                        .child(cell(format!("{:.3} / {:.3}", r.bid, r.ask), 130., true, fg))
                })
                .collect()
        })
        .track_scroll(&self.scroll)
        .flex_1();

        let right: Div = if with_detail {
            div().flex().flex_col().w(px(420.)).bg(bg).child(detail(&self.rows[self.selected], fg, muted))
        } else {
            let max_vol = self.bids.iter().chain(self.asks.iter()).map(|l| l.vol).max().unwrap_or(1) as f32;
            let level = |l: &Level, side: i8| {
                let ratio = l.vol as f32 / max_vol;
                let color = if side > 0 { up() } else { down() };
                div()
                    .relative()
                    .flex()
                    .h(px(ROW_H))
                    .items_center()
                    .text_xs()
                    .child(div().absolute().top_0().bottom_0().right_0().w(relative(ratio)).bg(color.opacity(0.14)))
                    .child(cell(format!("{:.3}", l.price), 70., false, color))
                    .child(cell(format!("{}", l.vol), 80., true, fg))
                    .child(cell(format!("({})", l.orders), 50., true, muted))
            };
            let depth = div()
                .flex()
                .gap_px()
                .bg(border)
                .child(div().flex_1().bg(bg).children(self.bids.iter().map(|l| level(l, 1))))
                .child(div().flex_1().bg(bg).children(self.asks.iter().map(|l| level(l, -1))));
            let trades = div().flex_1().overflow_hidden().children(self.trades.iter().take(40).map(|t| {
                let color = if t.dir == 1 { up() } else { down() };
                div()
                    .flex()
                    .h(px(ROW_H))
                    .items_center()
                    .text_xs()
                    .child(cell(format!("{}", t.time_ms / 1000), 70., false, muted))
                    .child(cell(format!("{:.3}", t.price), 80., true, color))
                    .child(cell(format!("{}", t.vol), 80., true, fg))
            }));
            div().flex().flex_col().w(px(420.)).bg(bg).child(depth).child(trades)
        };
        div()
            .flex()
            .size_full()
            .gap_px()
            .bg(border)
            .child(div().flex().flex_col().flex_1().min_w_0().bg(bg).child(header).child(table))
            .child(right)
    }
}

fn main() {
    let _ = bench_core::origin();
    gpui_kit::profiler::set_trace_enabled(true);
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        cx.activate(true);
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Batch>();
        let driver = Driver::start("rust-gpui", move |b| {
            let _ = tx.unbounded_send(b);
        });
        let feed = bench_core::Feed::new(42);
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1600.), px(1000.)), cx))),
            titlebar: Some(TitlebarOptions { title: Some("bench · Rust".into()), ..Default::default() }),
            ..Default::default()
        };
        let d2 = driver.clone();
        let items = Arc::new(bench_core::items(50_000));
        let (_window, view) = gpui_kit::open_window(opts, cx, move |_, cx| {
            cx.new(|_| Bench {
                changed: vec![None; feed.rows.len()],
                rows: feed.rows.clone(),
                bids: vec![],
                asks: vec![],
                trades: VecDeque::new(),
                scroll: UniformListScrollHandle::new(),
                scroll_ix: 0,
                driver: d2,
                batches: 0,
                anim_t0: Instant::now(),
                selected: 0,
                tab: 0,
                term: "",
                hits: vec![],
                total: 0,
                items,
            })
        })
        .expect("window");

        // 推送：一批一更新
        let metrics = driver.metrics.clone();
        let v = view.clone();
        cx.spawn(async move |cx| {
            while let Some(b) = rx.next().await {
                let seq = b.seq;
                let _ = v.update(cx, |this, cx| {
                    this.apply(b);
                    cx.notify();
                });
                metrics.on_applied(seq);
            }
        })
        .detach();

        // 场景动作：选行 200ms / 搜索 250ms / 计算 300ms / 切页签与推送中计算 400ms
        let metrics = driver.metrics.clone();
        let v = view.clone();
        let d4 = driver.clone();
        cx.spawn(async move |cx| {
            let calc_items = bench_core::items(100_000);
            let closes = bench_core::closes(100_000);
            let json = bench_core::big_json(33_000);
            let mut last = Instant::now();
            let mut k = 0usize;
            loop {
                cx.background_executor().timer(Duration::from_millis(10)).await;
                if d4.done.load(Ordering::Relaxed) {
                    break;
                }
                let p = &bench_core::schedule()[d4.phase.load(Ordering::Relaxed)];
                let every = match p.kind {
                    "select" => 200,
                    "filter" => 250,
                    "compute" => 300,
                    "tab" | "compute_load" => 400,
                    _ => continue,
                };
                if last.elapsed() < Duration::from_millis(every) {
                    continue;
                }
                last = Instant::now();
                k += 1;
                match p.kind {
                    "compute" | "compute_load" => {
                        let task = if p.kind == "compute_load" { "agg_100k" } else { bench_core::COMPUTE_TASKS[k % 4] };
                        let t0 = Instant::now();
                        let x = match task {
                            "sort_100k" => bench_core::task_sort(&calc_items),
                            "agg_100k" => bench_core::task_agg(&calc_items),
                            "ema_100k" => bench_core::task_ema(&closes),
                            _ => bench_core::task_json(&json) as f64,
                        };
                        metrics.record_compute(task, t0.elapsed().as_secs_f64() * 1000.0);
                        std::hint::black_box(x);
                    }
                    kind => {
                        metrics.on_trigger();
                        let _ = v.update(cx, |this, cx| {
                            match kind {
                                "select" => this.selected = (this.selected + 1) % this.rows.len(),
                                "tab" => this.tab = 1 - this.tab,
                                _ => {
                                    this.term = bench_core::FILTER_TERMS[k % bench_core::FILTER_TERMS.len()];
                                    let (h, t) = bench_core::filter(&this.items, this.term);
                                    this.hits = h;
                                    this.total = t;
                                }
                            }
                            cx.notify();
                        });
                    }
                }
            }
        })
        .detach();

        // 帧指标：每 50ms 收一次 profiler 事件；闪烁到期后补一次重绘
        let metrics = driver.metrics.clone();
        let d3 = driver.clone();
        cx.spawn(async move |cx| {
            let mut collector = gpui_kit::profiler::FrameTimingCollector::new();
            loop {
                cx.background_executor().timer(Duration::from_millis(50)).await;
                for e in collector.collect_unseen() {
                    match e {
                        gpui_kit::profiler::FrameEvent::Draw(t) => {
                            if let Some(r) = t.refresh_interval {
                                metrics.set_refresh(r);
                            }
                            metrics.on_draw(t.draw_start, t.draw_end, t.work_start());
                        }
                        gpui_kit::profiler::FrameEvent::Present(p) => metrics.on_present(p.present_end),
                    }
                }
                let _ = view.update(cx, |this, cx| {
                    let now = Instant::now();
                    let horizon = FLASH + Duration::from_millis(100);
                    if this.changed.iter().flatten().any(|(t, _)| now.duration_since(*t) < horizon) {
                        cx.notify();
                    }
                    // 进入动画阶段时重置起点
                    if !matches!(this.kind(), "anim_js" | "anim_native") {
                        this.anim_t0 = Instant::now();
                    } else {
                        // 动画阶段：确保开始逐帧（之后由 request_animation_frame 持续）
                        cx.notify();
                    }
                });
                if d3.done.load(Ordering::Relaxed) {
                    cx.background_executor().timer(Duration::from_millis(300)).await;
                    let _ = cx.update(|cx| cx.quit());
                    break;
                }
            }
        })
        .detach();
    });
}
