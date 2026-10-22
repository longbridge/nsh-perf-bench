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
    origin_pair().0
}

/// 进程起点，及同一时刻的 QueryPerformanceCounter 读数（仅 Windows）
fn origin_pair() -> (Instant, Option<i64>) {
    static T0: OnceLock<(Instant, Option<i64>)> = OnceLock::new();
    *T0.get_or_init(|| (Instant::now(), qpc_now()))
}

/// 当前 QPC 读数（仅 Windows）。Windows 上 `Instant` 本身基于 QPC，PresentMon 的 `--qpc_time` 输出同一时钟
#[cfg(windows)]
fn qpc_now() -> Option<i64> {
    let mut v = 0i64;
    unsafe { windows_sys::Win32::System::Performance::QueryPerformanceCounter(&mut v) };
    Some(v)
}
#[cfg(not(windows))]
fn qpc_now() -> Option<i64> {
    None
}

/// QPC 频率（次 / 秒，仅 Windows）
#[cfg(windows)]
fn qpc_freq() -> Option<i64> {
    let mut v = 0i64;
    unsafe { windows_sys::Win32::System::Performance::QueryPerformanceFrequency(&mut v) };
    Some(v)
}
#[cfg(not(windows))]
fn qpc_freq() -> Option<i64> {
    None
}

/// 把 `t` 换算成 QPC 读数（相对进程起点推算，仅 Windows）：scripts/presentmon_join.py 据此把帧切到各阶段
fn instant_to_qpc(t: Instant) -> Option<i64> {
    let (o, oq) = origin_pair();
    let (oq, f) = (oq?, qpc_freq()? as f64);
    let d = if t >= o { (t - o).as_secs_f64() } else { -(o - t).as_secs_f64() };
    Some(oq + (d * f).round() as i64)
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
    let v = full_schedule();
    // BENCH_PHASES=r20000,anim_js_1000：诊断时只跑列出的阶段（保留 warmup / cooldown）
    match std::env::var("BENCH_PHASES") {
        Ok(only) => v.into_iter().filter(|p| matches!(p.name, "warmup" | "cooldown") || only.split(',').any(|n| n == p.name)).collect(),
        Err(_) => v,
    }
}

fn full_schedule() -> Vec<Phase> {
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
        // 动画单因素对照（诊断用，不进标准报告）：逐帧 left → 逐帧 transform → 逐帧走原生动画的轨迹 → 原生动画
        "animation_ab" => {
            return vec![
                ph("warmup", s(3), 0, false, "idle", 0),
                ph("anim_js_1000", s(10), 0, false, "anim_js", 1000),
                ph("anim_js_tx_1000", s(10), 0, false, "anim_js_tx", 1000),
                ph("anim_js_curve_1000", s(10), 0, false, "anim_js_curve", 1000),
                ph("anim_native_1000", s(10), 0, false, "anim_native", 1000),
                ph("cooldown", s(3), 0, false, "idle", 0),
            ];
        }
        // 计算拆分探针（诊断用）：每 100ms 轮流执行一项细分测量
        "compute_ab" => {
            return vec![
                ph("warmup", s(3), 0, false, "idle", 0),
                ph("compute_probe", s(40), 0, false, "compute_probe", 100_000),
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
            // 先写报告再置 done：宿主看到 done 即可退出
            let path = metrics.write_report();
            eprintln!("[bench] report: {path}");
            dd.done.store(true, Ordering::Relaxed);
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
    qpc_start: Option<i64>,
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
    threads_start: HashMap<u32, (String, f64)>,
    counters_start: Vec<(&'static str, u64)>,
    spans: HashMap<String, Vec<f64>>,
    /// 经提交版本确认的延迟，及其比 applied 关联晚了多少
    verified: Vec<f64>,
    early: Vec<f64>,
    /// 时间戳早于本阶段开始、却在本阶段才收集到的帧
    stale_draws: u64,
    stale_presents: u64,
    /// 阶段开始时被清掉的未上屏更新
    dropped_at_begin: u64,
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
    /// 按线程名汇总的 CPU（%，单核 = 100），仅 Linux；同名线程合并
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub threads_cpu_pct: Vec<(String, f64)>,
    /// 诊断计数器在本阶段的增量（宿主通过 `Metrics::set_counters` 提供）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub counters: Vec<(String, u64)>,
    /// 分段计时（ms）：名称 → 分位数与总和
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spans_ms: Vec<SpanReport>,
    /// 帧关联诊断
    pub frame_link: FrameLink,
    /// 阶段起止的 QPC 读数（仅 Windows）：scripts/presentmon_join.py 按它把 PresentMon 的帧切到各阶段
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qpc_start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qpc_end: Option<i64>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[non_exhaustive]
pub struct SpanReport {
    pub name: String,
    pub n: usize,
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
    pub sum: f64,
}

/// 更新与帧的关联诊断：applied 关联（现有 tick_to_frame_ms）与提交版本确认的对比
#[derive(Serialize, Clone, Debug, Default)]
#[non_exhaustive]
pub struct FrameLink {
    /// 是否有提交探针（NSH 宿主有；GPUI 在同一线程应用并绘制，applied 关联即因果）
    pub probe: bool,
    /// 经确认：UI 线程应用了包含该更新的批次之后开始的第一帧
    pub verified_ms: Option<Pct>,
    /// applied 关联早于确认帧的更新数（被计到了尚未包含它的帧上）
    pub early: u64,
    /// 早关联时少算的延迟（ms）
    pub early_gap_ms: Option<Pct>,
    pub stale_draws: u64,
    pub stale_presents: u64,
    pub dropped_at_begin: u64,
    /// 阶段结束时仍未上屏的更新
    pub pending_at_end: u64,
}

#[derive(Serialize, Debug)]
pub struct Report {
    pub implementation: String,
    pub scenario: String,
    pub startup_first_frame_ms: f64,
    pub peak_footprint_mb: f64,
    /// 宿主报告的屏幕刷新率（超时帧按它的周期判定）；没报告时省略，按 60Hz 判定
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_hz: Option<f64>,
    /// 进程号与 QPC 频率 / 起点（仅 Windows）：PresentMon 按进程过滤、对时
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qpc: Option<Qpc>,
    pub phases: Vec<PhaseReport>,
}

#[derive(Serialize, Debug)]
pub struct Qpc {
    pub freq: i64,
    pub origin: i64,
}

pub struct Metrics {
    name: String,
    pub first_present_ns: AtomicU64,
    emits: Mutex<HashMap<u64, Instant>>,
    /// 已应用、尚未出现在帧里的批次
    applied: Mutex<VecDeque<Pending>>,
    counters: OnceLock<fn() -> Vec<(&'static str, u64)>>,
    probe: OnceLock<CommitProbe>,
    acc: Mutex<PhaseAcc>,
    done: Mutex<Vec<PhaseReport>>,
    peak_fp: AtomicU64,
    late_budget_us: AtomicU32,
    refresh_set: AtomicBool,
}

/// 每个线程的（线程名, CPU 秒），读 /proc/self/task/*/stat；非 Linux 返回空
pub fn thread_cpu_seconds() -> HashMap<u32, (String, f64)> {
    let mut out = HashMap::new();
    #[cfg(target_os = "linux")]
    {
        let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
        if let Ok(dir) = std::fs::read_dir("/proc/self/task") {
            for e in dir.flatten() {
                let Ok(tid) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
                let Ok(stat) = std::fs::read_to_string(e.path().join("stat")) else { continue };
                // comm 可能含空格，取最后一个 ')' 之后的字段
                let (Some(l), Some(r)) = (stat.find('('), stat.rfind(')')) else { continue };
                let name = stat[l + 1..r].to_string();
                let f: Vec<&str> = stat[r + 2..].split_whitespace().collect();
                // 去掉 pid 与 comm 后，utime / stime 是第 12 / 13 个字段
                let (Some(u), Some(st)) = (f.get(11).and_then(|x| x.parse::<f64>().ok()), f.get(12).and_then(|x| x.parse::<f64>().ok())) else { continue };
                out.insert(tid, (name, (u + st) / hz));
            }
        }
    }
    out
}

struct Pending {
    applied_at: Instant,
    emit: Instant,
    /// 包含该更新的提交序号（UI 线程按序应用）
    target: Option<u64>,
    label: Option<&'static str>,
    naive: Option<f64>,
}

/// 跨线程提交的宿主提供：已提交批次数、第 n 批在 UI 线程应用的时刻
#[derive(Clone, Copy)]
pub struct CommitProbe {
    pub submitted: fn() -> u64,
    pub ui_applied_at: fn(u64) -> Option<Instant>,
}

/// 本线程累计的次缺页数（minor page faults）；非 Linux 返回 0
pub fn thread_minflt() -> u64 {
    #[cfg(target_os = "linux")]
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_THREAD, &mut ru);
        return ru.ru_minflt as u64;
    }
    #[allow(unreachable_code)]
    0
}

/// 进程累计 CPU 时间（秒，用户态 + 内核态）
#[cfg(unix)]
pub fn cpu_seconds() -> f64 {
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut ru);
        let tv = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
        tv(ru.ru_utime) + tv(ru.ru_stime)
    }
}

/// 进程累计 CPU 时间（秒）：QueryProcessCycleTime 的周期数 ÷ 每 CPU 秒的周期数。
/// 不用 GetProcessTimes：它按时钟中断（约 15.6ms）采样记账，跟着刷新节奏的短脉冲工作
/// 常在两次中断之间做完，被少算甚至记成 0（实测每秒 1000 条行情时读数约为 0%）
#[cfg(windows)]
pub fn cpu_seconds() -> f64 {
    use windows_sys::Win32::System::{Threading::GetCurrentProcess, WindowsProgramming::QueryProcessCycleTime};
    let mut cycles = 0u64;
    if unsafe { QueryProcessCycleTime(GetCurrentProcess(), &mut cycles) } == 0 {
        return 0.0;
    }
    cycles as f64 / cycles_per_cpu_second()
}

/// 每 CPU 秒的周期数：一个线程忙等 200ms，用它的 QueryThreadCycleTime 增量除以墙钟时间
/// （忙等线程这段时间一直在跑，周期增量 ≈ 时间 × 计数频率）。只校准一次
#[cfg(windows)]
fn cycles_per_cpu_second() -> f64 {
    static HZ: OnceLock<f64> = OnceLock::new();
    *HZ.get_or_init(|| {
        thread::spawn(|| {
            use windows_sys::Win32::System::{Threading::GetCurrentThread, WindowsProgramming::QueryThreadCycleTime};
            let read = || {
                let mut c = 0u64;
                unsafe { QueryThreadCycleTime(GetCurrentThread(), &mut c) };
                c
            };
            let (c0, t0) = (read(), Instant::now());
            while t0.elapsed() < Duration::from_millis(200) {
                std::hint::spin_loop();
            }
            (read() - c0) as f64 / t0.elapsed().as_secs_f64()
        })
        .join()
        .unwrap_or(3.0e9)
    })
}

/// Windows 进程内存（MB）：私有工作集（PROCESS_MEMORY_COUNTERS_EX2.PrivateWorkingSetSize），
/// 与任务管理器「内存」列同口径；系统不支持 EX2 时退回私有提交（PrivateUsage）
#[cfg(windows)]
pub fn footprint_mb() -> f64 {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX2};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut c: PROCESS_MEMORY_COUNTERS_EX2 = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut c as *mut _ as *mut PROCESS_MEMORY_COUNTERS, c.cb) == 0 {
            return 0.0;
        }
        let b = if c.PrivateWorkingSetSize > 0 { c.PrivateWorkingSetSize } else { c.PrivateUsage };
        b as f64 / 1048576.0
    }
}

/// macOS phys_footprint（MB），与活动监视器「内存」一致
#[cfg(not(windows))]
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
            counters: OnceLock::new(),
            probe: OnceLock::new(),
            acc: Mutex::new(PhaseAcc::default()),
            done: Mutex::new(Vec::new()),
            peak_fp: AtomicU64::new(0),
            late_budget_us: AtomicU32::new(16_667),
            refresh_set: AtomicBool::new(false),
        }
    }

    fn begin_phase(&self, p: Phase) {
        let mut a = self.acc.lock().unwrap();
        let counters_start = self.counters.get().map(|f| f()).unwrap_or_default();
        let mut q = self.applied.lock().unwrap();
        let dropped_at_begin = q.len() as u64;
        q.clear();
        let now = Instant::now();
        *a = PhaseAcc {
            phase: Some(p),
            started: Some(now),
            qpc_start: instant_to_qpc(now),
            cpu_start: cpu_seconds(),
            threads_start: thread_cpu_seconds(),
            counters_start,
            dropped_at_begin,
            ..Default::default()
        };
    }

    fn end_phase(&self) {
        let mut a = std::mem::take(&mut *self.acc.lock().unwrap());
        let Some(p) = a.phase.take() else { return };
        let ended = Instant::now();
        let secs = a.started.map(|s| (ended - s).as_secs_f64()).unwrap_or(1.0);
        let fp = footprint_mb();
        self.peak_fp.fetch_max((fp * 1000.0) as u64, Ordering::Relaxed);
        let counters: Vec<(String, u64)> = match self.counters.get() {
            Some(f) => f()
                .into_iter()
                .map(|(n, v)| (n.to_string(), v - a.counters_start.iter().find(|x| x.0 == n).map_or(0, |x| x.1)))
                .collect(),
            None => Vec::new(),
        };
        let mut spans_ms: Vec<SpanReport> = a
            .spans
            .iter_mut()
            .map(|(k, v)| {
                let sum = v.iter().sum();
                let p = pct(v);
                SpanReport { name: k.clone(), n: p.n, p50: p.p50, p95: p.p95, max: p.max, sum }
            })
            .collect();
        spans_ms.sort_by(|x, y| x.name.cmp(&y.name));
        let frame_link = FrameLink {
            probe: self.probe.get().is_some(),
            verified_ms: (!a.verified.is_empty()).then(|| pct(&mut a.verified)),
            early: a.early.len() as u64,
            early_gap_ms: (!a.early.is_empty()).then(|| pct(&mut a.early)),
            stale_draws: a.stale_draws,
            stale_presents: a.stale_presents,
            dropped_at_begin: a.dropped_at_begin,
            pending_at_end: self.applied.lock().unwrap().len() as u64,
        };
        let mut threads: HashMap<String, f64> = HashMap::new();
        for (tid, (name, cpu)) in thread_cpu_seconds() {
            let start = a.threads_start.get(&tid).map_or(0.0, |x| x.1);
            *threads.entry(name).or_default() += (cpu - start) / secs * 100.0;
        }
        let mut threads_cpu_pct: Vec<(String, f64)> = threads.into_iter().filter(|x| x.1 >= 0.05).collect();
        threads_cpu_pct.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());
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
            threads_cpu_pct,
            counters,
            spans_ms,
            frame_link,
            qpc_start: a.qpc_start,
            qpc_end: a.qpc_start.and(instant_to_qpc(ended)),
        };
        for (k, p) in &r.compute_ms {
            eprintln!("[bench]   compute {k:>16} n {:>4} p50 {:>8.2} ms p95 {:>8.2} ms", p.n, p.p50, p.p95);
        }
        eprintln!(
            "[bench] {:>13} fps {:>5.1} draw p50 {:>5.2} p99 {:>6.2} ms | tick→frame p50 {:>6.2} p99 {:>7.2} ms | late {:>4} | cpu {:>5.1}% | mem {:>6.1} MB | applied {}/{}",
            r.name, r.fps, r.draw_ms.p50, r.draw_ms.p99, r.tick_to_frame_ms.p50, r.tick_to_frame_ms.p99,
            r.late_frames, r.cpu_pct, r.footprint_mb, r.applied_batches, r.emitted_batches
        );
        if !r.threads_cpu_pct.is_empty() {
            let t: Vec<String> = r.threads_cpu_pct.iter().map(|(n, c)| format!("{n} {c:.1}%")).collect();
            eprintln!("[bench]   threads {}", t.join(" | "));
        }
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
            self.push_pending(Instant::now(), emit, None);
            self.acc.lock().unwrap().applied_batches += 1;
        }
    }

    /// 交互场景：动作已触发（状态已改），之后第一帧算作它的响应
    pub fn on_trigger(&self) {
        self.on_trigger_labeled(None);
    }

    /// 同 on_trigger；label 另记一份 `lat.<label>` 分段延迟（如切入列表 / 切入详情）
    pub fn on_trigger_labeled(&self, label: Option<&'static str>) {
        let now = Instant::now();
        self.push_pending(now, now, label);
        self.acc.lock().unwrap().triggers += 1;
    }

    fn push_pending(&self, applied_at: Instant, emit: Instant, label: Option<&'static str>) {
        // 引擎线程上本轮的改动随下一次提交送往 UI 线程
        let target = self.probe.get().map(|p| (p.submitted)() + 1);
        self.applied.lock().unwrap().push_back(Pending { applied_at, emit, target, label, naive: None });
    }

    /// 诊断计数器：阶段开始 / 结束时各读一次，报告增量
    pub fn set_counters(&self, f: fn() -> Vec<(&'static str, u64)>) {
        let _ = self.counters.set(f);
    }

    /// 跨线程提交的宿主设置后，额外按提交版本确认每个更新所在的帧
    pub fn set_commit_probe(&self, probe: CommitProbe) {
        let _ = self.probe.set(probe);
    }

    /// 分段计时（ms），按名称汇总
    pub fn record_span(&self, name: &str, ms: f64) {
        let mut a = self.acc.lock().unwrap();
        match a.spans.get_mut(name) {
            Some(v) => v.push(ms),
            None => {
                a.spans.insert(name.to_string(), vec![ms]);
            }
        }
    }

    /// 计算场景：一次计算任务的耗时
    pub fn record_compute(&self, task: &str, ms: f64) {
        self.acc.lock().unwrap().compute.entry(task.to_string()).or_default().push(ms);
    }

    pub fn set_refresh(&self, interval: Duration) {
        self.late_budget_us.store(interval.as_micros() as u32, Ordering::Relaxed);
        self.refresh_set.store(true, Ordering::Relaxed);
    }

    /// 一帧绘制完成：draw_start/draw_end；work_start 为 GPUI FrameTiming::work_start()
    /// （刷新信号与首次失效中较晚者，等待刷新信号的时间不算帧的工作量）
    pub fn on_draw(&self, draw_start: Instant, draw_end: Instant, work_start: Option<Instant>) {
        let mut a = self.acc.lock().unwrap();
        if a.started.is_some_and(|s| draw_start < s) {
            a.stale_draws += 1;
        }
        a.draws.push((draw_end - draw_start).as_secs_f64() * 1000.0);
        if let Some(d) = work_start {
            let w = draw_end.saturating_duration_since(d);
            a.dirty_to_draw_end.push(w.as_secs_f64() * 1000.0);
            if w.as_micros() as u32 > self.late_budget_us.load(Ordering::Relaxed) {
                a.late_frames += 1;
            }
        }
        // applied 关联（现有口径）：绘制开始前已应用的批次，都算包含在这一帧里
        let mut q = self.applied.lock().unwrap();
        for it in q.iter_mut() {
            if it.applied_at > draw_start {
                break;
            }
            if it.naive.is_none() {
                let ms = draw_end.saturating_duration_since(it.emit).as_secs_f64() * 1000.0;
                it.naive = Some(ms);
                a.latency.push(ms);
            }
        }
        // 提交版本确认：UI 线程在这一帧开始前应用了包含它的批次
        let probe = self.probe.get();
        while let Some(it) = q.front() {
            let Some(naive) = it.naive else { break };
            let ok = match (probe, it.target) {
                (Some(p), Some(n)) => (p.ui_applied_at)(n).is_some_and(|t| t <= draw_start),
                _ => true,
            };
            if !ok {
                break;
            }
            let ms = draw_end.saturating_duration_since(it.emit).as_secs_f64() * 1000.0;
            if let Some(l) = it.label {
                a.spans.entry(format!("lat.{l}")).or_default().push(ms);
            }
            a.verified.push(ms);
            if ms > naive + 1e-3 {
                a.early.push(ms - naive);
            }
            q.pop_front();
        }
    }

    pub fn on_present(&self, present_end: Instant) {
        if self.first_present_ns.load(Ordering::Relaxed) == 0 {
            self.first_present_ns
                .store(present_end.saturating_duration_since(origin()).as_nanos().max(1) as u64, Ordering::Relaxed);
        }
        let mut a = self.acc.lock().unwrap();
        if a.started.is_some_and(|s| present_end < s) {
            a.stale_presents += 1;
        }
        a.presents.push(present_end);
    }

    fn write_report(&self) -> String {
        let r = Report {
            implementation: self.name.clone(),
            scenario: scenario(),
            startup_first_frame_ms: self.first_present_ns.load(Ordering::Relaxed) as f64 / 1e6,
            peak_footprint_mb: self.peak_fp.load(Ordering::Relaxed) as f64 / 1000.0,
            refresh_hz: self
                .refresh_set
                .load(Ordering::Relaxed)
                .then(|| (1e7 / self.late_budget_us.load(Ordering::Relaxed).max(1) as f64).round() / 10.0),
            pid: cfg!(windows).then(std::process::id),
            qpc: qpc_freq().zip(origin_pair().1).map(|(freq, origin)| Qpc { freq, origin }),
            phases: self.done.lock().unwrap().clone(),
        };
        let dir = std::env::var("BENCH_OUT").unwrap_or_else(|_| "results".into());
        let _ = std::fs::create_dir_all(&dir);
        let path = format!("{dir}/{}-{}.json", self.name, scenario());
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&r).unwrap());
        eprintln!(
            "[bench] startup first frame {:.1} ms, peak footprint {:.1} MB, refresh {}",
            r.startup_first_frame_ms,
            r.peak_footprint_mb,
            r.refresh_hz.map_or("unknown".into(), |hz| format!("{hz} Hz"))
        );
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

/// 原生动画的轨迹（逐帧版对照用）：循环 [延迟 (i*32)%1000ms → 1000ms 缓入缓出到 1 → 1000ms 回到 0]，
/// 返回 (位移 px, 透明度)，与 `Animated.loop(sequence([delay, timing(1), timing(0)]))` + interpolate 相同
pub fn native_curve(i: usize, t_ms: f64) -> (f64, f64) {
    let d = ((i * 32) % 1000) as f64;
    let u = t_ms % (d + 2000.0);
    let v = if u < d {
        0.0
    } else {
        let w = u - d;
        let x = if w < 1000.0 { w / 1000.0 } else { 2.0 - w / 1000.0 };
        (1.0 - (std::f64::consts::PI * x).cos()) / 2.0
    };
    let opacity = if v < 0.5 { 1.0 - 1.4 * v } else { 0.3 + 1.4 * (v - 0.5) };
    (v * 1400.0, opacity)
}

pub const FILTER_TERMS: [&str; 8] = ["1", "12", "123", "2", "23", "234", "标的9", "999"];
pub const COMPUTE_TASKS: [&str; 4] = ["sort_100k", "agg_100k", "ema_100k", "json_5mb"];

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as M;

    /// 测试用的提交探针：已提交批次数、第 n 批在 UI 线程应用的时刻
    static SUBMITTED: AtomicU64 = AtomicU64::new(0);
    static UI_APPLIED: M<Vec<(u64, Instant)>> = M::new(Vec::new());
    fn submitted() -> u64 {
        SUBMITTED.load(Ordering::SeqCst)
    }
    fn ui_applied_at(n: u64) -> Option<Instant> {
        UI_APPLIED.lock().unwrap().iter().find(|x| x.0 == n).map(|x| x.1)
    }
    const MS: Duration = Duration::from_millis(1);

    fn metrics(probe: bool) -> Metrics {
        let m = Metrics::new("test");
        if probe {
            m.set_commit_probe(CommitProbe { submitted, ui_applied_at });
        }
        m.begin_phase(ph("p", 1, 0, false, "select", 0));
        m
    }

    fn link(m: &Metrics) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let a = m.acc.lock().unwrap();
        (a.latency.clone(), a.verified.clone(), a.early.clone())
    }

    /// 引擎线程已写状态（applied），但 UI 线程在下一帧开始之后才应用该提交：
    /// applied 关联把更新算进了一帧不含它的帧，提交探针算到下一帧。
    #[test]
    fn applied_link_counts_a_frame_without_the_commit() {
        let m = metrics(true);
        let t0 = Instant::now();
        SUBMITTED.store(7, Ordering::SeqCst);
        m.on_trigger(); // 本轮改动随第 8 批提交
        let applied = m.applied.lock().unwrap().front().unwrap().applied_at;
        // 帧 A：applied 之后开始，但第 8 批尚未在 UI 线程应用
        let a_start = applied + MS;
        m.on_draw(a_start, a_start + MS, None);
        let (naive, verified, _) = link(&m);
        assert_eq!(naive.len(), 1, "applied 关联把帧 A 算作响应");
        assert!(verified.is_empty(), "提交尚未应用，帧 A 不含该更新");
        // UI 线程应用第 8 批，之后的帧 B 才包含它
        UI_APPLIED.lock().unwrap().push((8, a_start + 2 * MS));
        let b_start = a_start + 7 * MS;
        m.on_draw(b_start, b_start + MS, None);
        let (naive, verified, early) = link(&m);
        assert_eq!(naive.len(), 1);
        assert_eq!(verified.len(), 1);
        assert_eq!(early.len(), 1, "applied 关联早了一帧");
        assert!((verified[0] - naive[0] - 7.0).abs() < 0.5, "少算约 7ms：{naive:?} {verified:?}");
        let _ = t0;
    }

    /// 同线程应用并绘制（GPUI）：没有探针时两种口径一致
    #[test]
    fn without_probe_verified_equals_applied_link() {
        let m = metrics(false);
        m.on_trigger();
        let at = m.applied.lock().unwrap().front().unwrap().applied_at;
        m.on_draw(at + MS, at + 2 * MS, None);
        let (naive, verified, early) = link(&m);
        assert_eq!(naive, verified);
        assert!(early.is_empty());
    }

    /// 50ms 收集一次 profiler 事件：上一阶段末尾的帧在下一阶段开始后才收到，会计入下一阶段
    #[test]
    fn frames_collected_after_phase_switch_count_in_the_new_phase() {
        let m = metrics(false);
        let before = Instant::now();
        std::thread::sleep(2 * MS);
        m.begin_phase(ph("next", 1, 0, false, "idle", 0));
        m.on_draw(before, before + MS, None);
        m.on_present(before + MS);
        let a = m.acc.lock().unwrap();
        assert_eq!((a.stale_draws, a.stale_presents), (1, 1));
        assert_eq!(a.presents.len(), 1, "现有 fps 把它算进了新阶段");
    }

    /// 阶段开始时清空未上屏的更新：这些更新既不进延迟，也不算丢失
    #[test]
    fn pending_updates_are_dropped_at_phase_start() {
        let m = metrics(false);
        m.on_trigger();
        m.on_trigger();
        m.begin_phase(ph("next", 1, 0, false, "idle", 0));
        assert_eq!(m.acc.lock().unwrap().dropped_at_begin, 2);
        assert!(m.applied.lock().unwrap().is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn windows_cpu_memory_and_qpc() {
        // 忙等 100ms：周期计数的 CPU 时间应接近墙钟时间（按时钟中断记账的 GetProcessTimes 可能少算）
        let (c0, t0) = (cpu_seconds(), Instant::now());
        while t0.elapsed() < Duration::from_millis(100) {
            std::hint::spin_loop();
        }
        let busy = cpu_seconds() - c0;
        assert!(busy > 0.07 && busy < 0.3, "{busy}");
        assert!(footprint_mb() > 0.0);
        let m = Metrics::new("t");
        m.begin_phase(ph("p", 1, 0, false, "idle", 0));
        thread::sleep(Duration::from_millis(20));
        m.end_phase();
        let r = m.done.lock().unwrap()[0].clone();
        let (a, b, f) = (r.qpc_start.unwrap(), r.qpc_end.unwrap(), qpc_freq().unwrap() as f64);
        assert!(((b - a) as f64 / f - r.secs).abs() < 0.005);
    }
}
