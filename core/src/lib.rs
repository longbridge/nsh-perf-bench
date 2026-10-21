//! 技术选型基准的公共部分：确定性行情生成器、压测阶段编排、帧与资源指标统计。
//! Rust 版与 NSH TSX 版共用同一份，保证输入完全一致、尺子完全一致。

use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

/// 进程起点（启动耗时、时间戳都相对它）
pub fn origin() -> Instant {
    static T0: OnceLock<Instant> = OnceLock::new();
    *T0.get_or_init(Instant::now)
}

// ---------------------------------------------------------------------------
// 行情生成器
// ---------------------------------------------------------------------------

pub const SYMBOLS: usize = 500;
pub const DEPTH_LEVELS: usize = 10;
pub const MAX_TRADES: usize = 200;

#[derive(Clone, Debug, Serialize)]
pub struct Row {
    pub idx: u32,
    pub code: String,
    pub name: String,
    pub last: f64,
    pub prev: f64,
    pub vol: u64,
    pub turnover: f64,
    pub high: f64,
    pub low: f64,
    pub bid: f64,
    pub ask: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Level {
    pub price: f64,
    pub vol: u64,
    pub orders: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct Trade {
    pub time_ms: u64,
    pub price: f64,
    pub vol: u64,
    /// 1 买 2 卖
    pub dir: u8,
}

/// 一批推送（每 10ms 一批，与真实推送批量转发的节奏一致）
#[derive(Clone, Debug, Serialize)]
pub struct Batch {
    pub seq: u64,
    /// 生成时刻，相对 origin 的纳秒
    pub emit_ns: u64,
    pub rows: Vec<Row>,
    pub bids: Option<Vec<Level>>,
    pub asks: Option<Vec<Level>>,
    pub trades: Vec<Trade>,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

pub struct Feed {
    rng: Rng,
    pub rows: Vec<Row>,
    focus_mid: f64,
    seq: u64,
    carry: f64,
}

impl Feed {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng(seed.max(1));
        let rows = (0..SYMBOLS)
            .map(|i| {
                let prev = 5.0 + rng.f() * 500.0;
                Row {
                    idx: i as u32,
                    code: format!("{:05}.HK", 1 + i * 7),
                    name: format!("示例股票{:03}", i),
                    last: prev,
                    prev,
                    vol: (rng.f() * 1e7) as u64,
                    turnover: rng.f() * 1e9,
                    high: prev * 1.01,
                    low: prev * 0.99,
                    bid: prev - 0.01,
                    ask: prev + 0.01,
                }
            })
            .collect();
        Self { rng, rows, focus_mid: 400.0, seq: 0, carry: 0.0 }
    }

    /// 生成一批：rate = 每秒行情更新条数，dt = 距上一批的秒数
    pub fn batch(&mut self, rate: u32, dt: f64, with_depth: bool, trades: usize) -> Batch {
        self.seq += 1;
        self.carry += rate as f64 * dt;
        let n = self.carry.floor() as usize;
        self.carry -= n as f64;
        let mut changed = Vec::with_capacity(n);
        for _ in 0..n {
            let i = self.rng.range(SYMBOLS);
            let r = &mut self.rows[i];
            let step = (self.rng.f() - 0.5) * r.prev * 0.002;
            r.last = (r.last + step).max(0.01);
            r.high = r.high.max(r.last);
            r.low = r.low.min(r.last);
            let v = (self.rng.f() * 5000.0) as u64;
            r.vol += v;
            r.turnover += v as f64 * r.last;
            r.bid = r.last - 0.01;
            r.ask = r.last + 0.01;
            changed.push(r.clone());
        }
        let (bids, asks) = if with_depth {
            self.focus_mid += (self.rng.f() - 0.5) * 0.4;
            let mid = self.focus_mid;
            let mut lv = |sign: f64| {
                (0..DEPTH_LEVELS)
                    .map(|k| Level {
                        price: mid + sign * (0.2 * (k as f64 + 1.0)),
                        vol: 100 + (self.rng.f() * 90000.0) as u64,
                        orders: 1 + self.rng.range(300) as u32,
                    })
                    .collect::<Vec<_>>()
            };
            (Some(lv(-1.0)), Some(lv(1.0)))
        } else {
            (None, None)
        };
        let now_ms = origin().elapsed().as_millis() as u64;
        let trades = (0..trades)
            .map(|_| Trade {
                time_ms: now_ms,
                price: self.focus_mid + (self.rng.f() - 0.5) * 0.4,
                vol: 100 * (1 + self.rng.range(50) as u64),
                dir: 1 + self.rng.range(2) as u8,
            })
            .collect();
        Batch { seq: self.seq, emit_ns: origin().elapsed().as_nanos() as u64, rows: changed, bids, asks, trades }
    }
}

// ---------------------------------------------------------------------------
// 阶段编排
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct Phase {
    pub name: &'static str,
    pub secs: u64,
    /// 每秒行情更新条数
    pub rate: u32,
    /// 是否同时自动滚动表格
    pub scroll: bool,
    /// 场景内的动作：quotes / anim_js / anim_native / select / tab / filter / compute / compute_load
    pub kind: &'static str,
    /// 动作参数：动画元素数、计算规模等
    pub param: u32,
}

fn ph(name: &'static str, secs: u64, rate: u32, scroll: bool, kind: &'static str, param: u32) -> Phase {
    Phase { name, secs, rate, scroll, kind, param }
}

/// BENCH_SCENARIO：quotes（默认）/ animation / interaction / compute
pub fn scenario() -> String {
    std::env::var("BENCH_SCENARIO").unwrap_or_else(|_| "quotes".into())
}

pub fn schedule() -> Vec<Phase> {
    // BENCH_SOAK=分钟：浸泡测试，每秒 1000 条持续 N 分钟，每 5 分钟一个阶段，看内存是否持续增长
    if let Some(min) = std::env::var("BENCH_SOAK").ok().and_then(|v| v.parse::<u64>().ok()) {
        const NAMES: [&str; 24] = ["soak05", "soak10", "soak15", "soak20", "soak25", "soak30", "soak35", "soak40", "soak45", "soak50", "soak55", "soak60",
            "soak65", "soak70", "soak75", "soak80", "soak85", "soak90", "soak95", "soak100", "soak105", "soak110", "soak115", "soak120"];
        let mut v = vec![ph("warmup", 3, 0, false, "idle", 0)];
        for i in 0..(min / 5).clamp(1, 24) as usize {
            v.push(ph(NAMES[i], 300, 1000, false, "quotes", 0));
        }
        return v;
    }
    let quick = std::env::var("BENCH_QUICK").is_ok();
    let s = |n: u64| if quick { n.min(3) } else { n };
    match scenario().as_str() {
        "animation" => {
            return vec![
                ph("warmup", s(3), 0, false, "idle", 0),
                ph("anim_js_200", s(10), 0, false, "anim_js", 200),
                ph("anim_js_1000", s(10), 0, false, "anim_js", 1000),
                ph("anim_native_200", s(10), 0, false, "anim_native", 200),
                ph("anim_native_1000", s(10), 0, false, "anim_native", 1000),
                ph("cooldown", s(3), 0, false, "idle", 0),
            ];
        }
        "interaction" => {
            return vec![
                ph("warmup", s(3), 0, false, "idle", 0),
                ph("select_row", s(10), 0, false, "select", 0),
                ph("switch_tab", s(10), 0, false, "tab", 0),
                ph("filter_50k", s(10), 0, false, "filter", 50_000),
                ph("select_under_load", s(10), 1000, false, "select", 0),
                ph("cooldown", s(3), 0, false, "idle", 0),
            ];
        }
        "compute" => {
            return vec![
                ph("warmup", s(3), 0, false, "idle", 0),
                ph("compute_100k", s(15), 0, false, "compute", 100_000),
                ph("compute_under_load", s(10), 1000, false, "compute_load", 100_000),
                ph("cooldown", s(3), 0, false, "idle", 0),
            ];
        }
        _ => {}
    }
    vec![
        ph("warmup", s(3), 0, false, "idle", 0),
        ph("idle", s(10), 0, false, "idle", 0),
        ph("r200", s(10), 200, false, "quotes", 0),
        ph("r1000", s(10), 1000, false, "quotes", 0),
        ph("r5000", s(10), 5000, false, "quotes", 0),
        ph("r5000_scroll", s(10), 5000, true, "quotes", 0),
        ph("r20000", s(10), 20000, false, "quotes", 0),
        ph("cooldown", s(5), 0, false, "idle", 0),
    ]
}

/// 压测驱动：后台线程按阶段推进，按 10ms 节拍生成批次交给 sink。
pub struct Driver {
    pub phase: Arc<AtomicUsize>,
    pub scroll: Arc<AtomicBool>,
    pub done: Arc<AtomicBool>,
    pub metrics: Arc<Metrics>,
}

impl Driver {
    pub fn start(impl_name: &str, sink: impl Fn(Batch) + Send + 'static) -> Arc<Driver> {
        let _ = origin();
        let metrics = Arc::new(Metrics::new(impl_name));
        let d = Arc::new(Driver {
            phase: Arc::new(AtomicUsize::new(0)),
            scroll: Arc::new(AtomicBool::new(false)),
            done: Arc::new(AtomicBool::new(false)),
            metrics: metrics.clone(),
        });
        let dd = d.clone();
        thread::spawn(move || {
            // 等首帧后再开始，启动耗时单独计
            // BENCH_NOWAIT：锁屏时只验证数据链路，不等首帧
            while std::env::var("BENCH_NOWAIT").is_err() && metrics.first_present_ns.load(Ordering::Relaxed) == 0 {
                thread::sleep(Duration::from_millis(5));
            }
            let mut feed = Feed::new(42);
            let phases = schedule();
            let tick = Duration::from_millis(10);
            for (pi, p) in phases.iter().enumerate() {
                dd.phase.store(pi, Ordering::Relaxed);
                dd.scroll.store(p.scroll, Ordering::Relaxed);
                metrics.begin_phase(p.clone());
                let end = Instant::now() + Duration::from_secs(p.secs);
                let mut next = Instant::now();
                let mut n: u64 = 0;
                while Instant::now() < end {
                    next += tick;
                    n += 1;
                    if p.rate > 0 {
                        // 盘口每 50ms 一次，逐笔每批 0~1 条（约 50 条/秒）
                        let b = feed.batch(p.rate, 0.01, n % 5 == 0, (n % 2) as usize);
                        metrics.on_emit(b.seq, b.rows.len());
                        sink(b);
                    }
                    let now = Instant::now();
                    if next > now {
                        thread::sleep(next - now);
                    }
                }
                metrics.end_phase();
            }
            dd.done.store(true, Ordering::Relaxed);
            let path = metrics.write_report();
            eprintln!("[bench] report: {path}");
        });
        d
    }
}

// ---------------------------------------------------------------------------
// 指标
// ---------------------------------------------------------------------------

#[derive(Default)]
struct PhaseAcc {
    phase: Option<Phase>,
    started: Option<Instant>,
    cpu_start: f64,
    draws: Vec<f64>,
    dirty_to_draw_end: Vec<f64>,
    presents: Vec<Instant>,
    latency: Vec<f64>,
    emitted_batches: u64,
    emitted_rows: u64,
    applied_batches: u64,
    late_frames: u64,
    compute: HashMap<String, Vec<f64>>,
    triggers: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Pct {
    pub n: usize,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
}

fn pct(v: &mut Vec<f64>) -> Pct {
    if v.is_empty() {
        return Pct { n: 0, p50: 0.0, p95: 0.0, p99: 0.0, max: 0.0 };
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| v[((v.len() as f64 - 1.0) * q).round() as usize];
    Pct { n: v.len(), p50: at(0.5), p95: at(0.95), p99: at(0.99), max: *v.last().unwrap() }
}

#[derive(Serialize, Clone, Debug)]
pub struct PhaseReport {
    pub name: String,
    pub rate: u32,
    pub scroll: bool,
    pub secs: f64,
    /// 每秒上屏帧数
    pub fps: f64,
    /// Window::draw 耗时（ms）
    pub draw_ms: Pct,
    /// 帧工作量：work_start → 绘制完成（ms）
    pub dirty_to_draw_ms: Pct,
    /// 行情生成 → 包含它的帧绘制完成（ms）：端到端延迟
    pub tick_to_frame_ms: Pct,
    /// 超过一个刷新周期的帧数
    pub late_frames: u64,
    /// 进程 CPU 占用（%，单核 = 100）
    pub cpu_pct: f64,
    /// 阶段结束时的 phys_footprint（MB）
    pub footprint_mb: f64,
    pub emitted_batches: u64,
    pub applied_batches: u64,
    pub emitted_rows: u64,
    /// 交互触发次数（交互场景）
    pub triggers: u64,
    /// 计算任务耗时（计算场景）：任务名 → 分位数
    pub compute_ms: Vec<(String, Pct)>,
}

#[derive(Serialize, Debug)]
pub struct Report {
    pub implementation: String,
    pub scenario: String,
    pub startup_first_frame_ms: f64,
    pub peak_footprint_mb: f64,
    pub phases: Vec<PhaseReport>,
}

pub struct Metrics {
    name: String,
    pub first_present_ns: AtomicU64,
    emits: Mutex<HashMap<u64, Instant>>,
    /// (applied_at, emit_at)：已应用、尚未出现在帧里的批次
    applied: Mutex<VecDeque<(Instant, Instant)>>,
    acc: Mutex<PhaseAcc>,
    done: Mutex<Vec<PhaseReport>>,
    peak_fp: AtomicU64,
    late_budget_us: AtomicU32,
}

pub fn cpu_seconds() -> f64 {
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut ru);
        let tv = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
        tv(ru.ru_utime) + tv(ru.ru_stime)
    }
}

/// macOS phys_footprint（MB），与活动监视器「内存」一致
pub fn footprint_mb() -> f64 {
    #[cfg(target_os = "macos")]
    unsafe {
        let mut info: libc::rusage_info_v2 = std::mem::zeroed();
        let r = libc::proc_pid_rusage(
            libc::getpid(),
            libc::RUSAGE_INFO_V2,
            &mut info as *mut _ as *mut libc::rusage_info_t,
        );
        if r == 0 {
            return info.ri_phys_footprint as f64 / 1048576.0;
        }
    }
    0.0
}

impl Metrics {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            first_present_ns: AtomicU64::new(0),
            emits: Mutex::new(HashMap::new()),
            applied: Mutex::new(VecDeque::new()),
            acc: Mutex::new(PhaseAcc::default()),
            done: Mutex::new(Vec::new()),
            peak_fp: AtomicU64::new(0),
            late_budget_us: AtomicU32::new(16_667),
        }
    }

    fn begin_phase(&self, p: Phase) {
        let mut a = self.acc.lock().unwrap();
        *a = PhaseAcc { phase: Some(p), started: Some(Instant::now()), cpu_start: cpu_seconds(), ..Default::default() };
        self.applied.lock().unwrap().clear();
    }

    fn end_phase(&self) {
        let mut a = std::mem::take(&mut *self.acc.lock().unwrap());
        let Some(p) = a.phase.take() else { return };
        let secs = a.started.map(|s| s.elapsed().as_secs_f64()).unwrap_or(1.0);
        let fp = footprint_mb();
        self.peak_fp.fetch_max((fp * 1000.0) as u64, Ordering::Relaxed);
        let r = PhaseReport {
            name: p.name.to_string(),
            rate: p.rate,
            scroll: p.scroll,
            secs,
            fps: a.presents.len() as f64 / secs,
            draw_ms: pct(&mut a.draws),
            dirty_to_draw_ms: pct(&mut a.dirty_to_draw_end),
            tick_to_frame_ms: pct(&mut a.latency),
            late_frames: a.late_frames,
            cpu_pct: (cpu_seconds() - a.cpu_start) / secs * 100.0,
            footprint_mb: fp,
            emitted_batches: a.emitted_batches,
            applied_batches: a.applied_batches,
            emitted_rows: a.emitted_rows,
            triggers: a.triggers,
            compute_ms: {
                let mut v: Vec<(String, Pct)> = a.compute.iter_mut().map(|(k, xs)| (k.clone(), pct(xs))).collect();
                v.sort_by(|x, y| x.0.cmp(&y.0));
                v
            },
        };
        for (k, p) in &r.compute_ms {
            eprintln!("[bench]   compute {k:>16} n {:>4} p50 {:>8.2} ms p95 {:>8.2} ms", p.n, p.p50, p.p95);
        }
        eprintln!(
            "[bench] {:>13} fps {:>5.1} draw p50 {:>5.2} p99 {:>6.2} ms | tick→frame p50 {:>6.2} p99 {:>7.2} ms | late {:>4} | cpu {:>5.1}% | mem {:>6.1} MB | applied {}/{}",
            r.name, r.fps, r.draw_ms.p50, r.draw_ms.p99, r.tick_to_frame_ms.p50, r.tick_to_frame_ms.p99,
            r.late_frames, r.cpu_pct, r.footprint_mb, r.applied_batches, r.emitted_batches
        );
        self.done.lock().unwrap().push(r);
    }

    fn on_emit(&self, seq: u64, rows: usize) {
        self.emits.lock().unwrap().insert(seq, Instant::now());
        let mut a = self.acc.lock().unwrap();
        a.emitted_batches += 1;
        a.emitted_rows += rows as u64;
    }

    /// UI 已把这一批数据写进状态（Rust：实体更新后；TSX：JS 写完 signal 后回调宿主）
    pub fn on_applied(&self, seq: u64) {
        let emit = self.emits.lock().unwrap().remove(&seq);
        if let Some(emit) = emit {
            self.applied.lock().unwrap().push_back((Instant::now(), emit));
            self.acc.lock().unwrap().applied_batches += 1;
        }
    }

    /// 交互场景：动作已触发（状态已改），之后第一帧算作它的响应
    pub fn on_trigger(&self) {
        let now = Instant::now();
        self.applied.lock().unwrap().push_back((now, now));
        self.acc.lock().unwrap().triggers += 1;
    }

    /// 计算场景：一次计算任务的耗时
    pub fn record_compute(&self, task: &str, ms: f64) {
        self.acc.lock().unwrap().compute.entry(task.to_string()).or_default().push(ms);
    }

    pub fn set_refresh(&self, interval: Duration) {
        self.late_budget_us.store(interval.as_micros() as u32, Ordering::Relaxed);
    }

    /// 一帧绘制完成：draw_start/draw_end；work_start 为 GPUI FrameTiming::work_start()
    /// （刷新信号与首次失效中较晚者，等待刷新信号的时间不算帧的工作量）
    pub fn on_draw(&self, draw_start: Instant, draw_end: Instant, work_start: Option<Instant>) {
        let mut a = self.acc.lock().unwrap();
        a.draws.push((draw_end - draw_start).as_secs_f64() * 1000.0);
        if let Some(d) = work_start {
            let w = draw_end.saturating_duration_since(d);
            a.dirty_to_draw_end.push(w.as_secs_f64() * 1000.0);
            if w.as_micros() as u32 > self.late_budget_us.load(Ordering::Relaxed) {
                a.late_frames += 1;
            }
        }
        // 绘制开始前已应用的批次，都包含在这一帧里
        let mut q = self.applied.lock().unwrap();
        while let Some(&(applied_at, emit)) = q.front() {
            if applied_at > draw_start {
                break;
            }
            q.pop_front();
            a.latency.push(draw_end.saturating_duration_since(emit).as_secs_f64() * 1000.0);
        }
    }

    pub fn on_present(&self, present_end: Instant) {
        if self.first_present_ns.load(Ordering::Relaxed) == 0 {
            self.first_present_ns
                .store(present_end.saturating_duration_since(origin()).as_nanos().max(1) as u64, Ordering::Relaxed);
        }
        self.acc.lock().unwrap().presents.push(present_end);
    }

    fn write_report(&self) -> String {
        let r = Report {
            implementation: self.name.clone(),
            scenario: scenario(),
            startup_first_frame_ms: self.first_present_ns.load(Ordering::Relaxed) as f64 / 1e6,
            peak_footprint_mb: self.peak_fp.load(Ordering::Relaxed) as f64 / 1000.0,
            phases: self.done.lock().unwrap().clone(),
        };
        let dir = std::env::var("BENCH_OUT").unwrap_or_else(|_| "results".into());
        let _ = std::fs::create_dir_all(&dir);
        let path = format!("{dir}/{}-{}.json", self.name, scenario());
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&r).unwrap());
        eprintln!("[bench] startup first frame {:.1} ms, peak footprint {:.1} MB", r.startup_first_frame_ms, r.peak_footprint_mb);
        path
    }
}

// ---------------------------------------------------------------------------
// 交互 / 计算场景的数据与任务（JS 版在 nsh-app/app/work.ts 里按同样算法实现）
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Item {
    pub code: String,
    pub name: String,
    pub market: u8,
    pub sector: u8,
    pub price: f64,
    pub cost: f64,
    pub qty: f64,
    pub chg_pct: f64,
    pub vol: u64,
}

pub fn items(n: usize) -> Vec<Item> {
    let mut rng = Rng(7);
    (0..n)
        .map(|i| {
            let price = 1.0 + rng.f() * 500.0;
            Item {
                code: format!("{:06}", (i * 37) % 1_000_000),
                name: format!("标的{}", i),
                market: (i % 5) as u8,
                sector: (rng.range(20)) as u8,
                price,
                cost: price * (0.7 + rng.f() * 0.6),
                qty: (1 + rng.range(5000)) as f64,
                chg_pct: (rng.f() - 0.5) * 20.0,
                vol: (rng.f() * 1e7) as u64,
            }
        })
        .collect()
}

pub fn closes(n: usize) -> Vec<f64> {
    let mut rng = Rng(11);
    let mut p = 100.0;
    (0..n)
        .map(|_| {
            p = (p + (rng.f() - 0.5) * 2.0).max(1.0);
            p
        })
        .collect()
}

/// 按涨跌幅降序排序（复制后排序，返回校验值）
pub fn task_sort(items: &[Item]) -> f64 {
    let mut v: Vec<&Item> = items.iter().collect();
    v.sort_by(|a, b| b.chg_pct.partial_cmp(&a.chg_pct).unwrap());
    v[0].chg_pct + v[v.len() - 1].chg_pct
}

/// 持仓汇总：按 市场 × 行业 汇总市值与盈亏
pub fn task_agg(items: &[Item]) -> f64 {
    let mut mv = [[0.0f64; 20]; 5];
    let mut pnl = [[0.0f64; 20]; 5];
    for it in items {
        mv[it.market as usize][it.sector as usize] += it.price * it.qty;
        pnl[it.market as usize][it.sector as usize] += (it.price - it.cost) * it.qty;
    }
    mv.iter().flatten().sum::<f64>() + pnl.iter().flatten().sum::<f64>()
}

/// K 线指标：EMA12 / EMA26 / MACD / Signal9
pub fn task_ema(closes: &[f64]) -> f64 {
    let (a12, a26, a9) = (2.0 / 13.0, 2.0 / 27.0, 2.0 / 10.0);
    let (mut e12, mut e26, mut sig) = (closes[0], closes[0], 0.0);
    let mut hist = 0.0;
    for &c in closes {
        e12 += a12 * (c - e12);
        e26 += a26 * (c - e26);
        let macd = e12 - e26;
        sig += a9 * (macd - sig);
        hist += macd - sig;
    }
    hist
}

pub fn big_json(n: usize) -> String {
    serde_json::to_string(&items(n)).unwrap()
}

/// 解析大 JSON（约 5MB），返回元素数
pub fn task_json(s: &str) -> usize {
    let v: serde_json::Value = serde_json::from_str(s).unwrap();
    v.as_array().map(|a| a.len()).unwrap_or(0)
}

/// 搜索过滤：代码或名称包含 term，返回前 100 条下标与总数
pub fn filter(items: &[Item], term: &str) -> (Vec<usize>, usize) {
    let mut hits = Vec::with_capacity(100);
    let mut total = 0;
    for (i, it) in items.iter().enumerate() {
        if it.code.contains(term) || it.name.contains(term) {
            total += 1;
            if hits.len() < 100 {
                hits.push(i);
            }
        }
    }
    (hits, total)
}

pub const FILTER_TERMS: [&str; 8] = ["1", "12", "123", "2", "23", "234", "标的9", "999"];
pub const COMPUTE_TASKS: [&str; 4] = ["sort_100k", "agg_100k", "ema_100k", "json_5mb"];
