//! 技术选型基准 · NSH Rust 版（BENCH_MODE=rustui）：同一个宿主、同一个 NSH 引擎与渲染层，
//! 界面用 NSH 的 Rust 写法（`nsh_core::ui`：`#[component]`、`view()`、signal）实现，没有 QuickJS 参与。
//! 界面与交互逐项对应 `app/App.tsx`；行情批次直接在 Rust 里写 signal，不经 JSON。

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use bench_core::{Batch, Item, Level, Row, Trade};
use futures::StreamExt;
use nsh_core::time::unix_ms;
use nsh_core::ui::prelude::*;

use crate::{DRIVER, RX};

const ROW_H: f32 = 22.0;
const FLASH: f64 = 600.0;

const BG: Color = rgb(0x161419);
const LINE: Color = rgb(0x2e2c33);
const FG: Color = rgb(0xe7e6eb);
const MUTED: Color = rgb(0x656466);
const UP: Color = rgb(0x20a16e);
const DOWN: Color = rgb(0xcc3333);
const UP_FLASH: Color = rgba(0x20a16e73);
const DOWN_FLASH: Color = rgba(0xcc333373);
const UP_BG: Color = rgba(0x20a16e24);
const DOWN_BG: Color = rgba(0xcc333324);
const SEL: Color = rgba(0x37a0ff24);
const ROW_LINE: Color = rgba(0x2e2c3380);

/// 一行：行情 + 最近一次变化时刻 + 方向。每次写入都通知（`seq` 递增）
#[derive(Clone)]
struct Cell {
    row: Row,
    at: f64,
    dir: i8,
    seq: u64,
}
impl PartialEq for Cell {
    fn eq(&self, o: &Self) -> bool {
        self.seq == o.seq
    }
}

#[derive(Clone, PartialEq, Default)]
struct Info {
    name: String,
    kind: String,
    param: u32,
}

fn phase_info() -> Info {
    let Some(d) = DRIVER.get() else { return Info::default() };
    let p = &bench_core::schedule()[d.phase.load(Ordering::Relaxed)];
    Info { name: p.name.into(), kind: p.kind.into(), param: p.param }
}

fn txt(s: impl IntoReactive<SharedString>, w: f32, right: bool, color: impl IntoReactive<AdaptiveColor>) -> Text {
    let t = text(s).w(px(w)).px(px(4.0)).text_size(px(12.0)).tabular_nums().text_color(color);
    if right { t.text_right() } else { t }
}

fn signed(v: f64, digits: usize) -> String {
    format!("{}{v:.digits$}", if v > 0.0 { "+" } else { "" })
}

/// 共享状态（全部在引擎线程）
#[derive(Clone, Copy)]
struct State {
    rows: Stored<Vec<Signal<Cell>>>,
    bids: Signal<Vec<Level>>,
    asks: Signal<Vec<Level>>,
    trades: Signal<Vec<Trade>>,
    now: Signal<f64>,
    info: Signal<Info>,
    selected: Signal<usize>,
    tab: Signal<u8>,
    term: Signal<String>,
    hits: Signal<Vec<usize>>,
    total: Signal<usize>,
    batches: Signal<u64>,
}

thread_local! {
    static ITEMS50K: Rc<Vec<Item>> = Rc::new(bench_core::items(50_000));
}

fn row_of(st: State, i: usize) -> Signal<Cell> {
    st.rows.with(|r| r[i])
}

#[component]
pub fn BenchRoot() -> impl IntoElement {
    let init = bench_core::Feed::new(42).rows;
    let seq = Rc::new(std::cell::Cell::new(0_u64));
    let st = State {
        rows: stored(init.into_iter().map(|row| signal(Cell { row, at: 0.0, dir: 0, seq: 0 })).collect()),
        bids: signal_with(Vec::new(), SignalEquals::Never),
        asks: signal_with(Vec::new(), SignalEquals::Never),
        trades: signal_with(Vec::new(), SignalEquals::Never),
        now: signal(unix_ms()),
        info: signal(Info::default()),
        selected: signal(0),
        tab: signal(0),
        term: signal(String::new()),
        hits: signal(Vec::new()),
        total: signal(0),
        batches: signal(0),
    };
    let last_change = stored(0.0_f64);

    // 行情批次：直接写 signal
    let rx = RX.lock().unwrap().take();
    if let Some(mut rx) = rx {
        let seq = seq.clone();
        spawn(async move {
            while let Some(b) = rx.next().await {
                apply(st, &seq, last_change, b);
            }
        });
    }

    set_interval(Duration::from_millis(100), move || {
        let p = phase_info();
        if p != st.info.get_untracked() {
            st.info.set(p);
        }
        let t = unix_ms();
        if t - last_change.with(|v| *v) < FLASH + 100.0 {
            st.now.set(t);
        }
    });

    // 交互 / 计算动作：节奏与 TSX 版一致
    let last = stored(Instant::now());
    let k = stored(0_usize);
    let calc: Rc<RefCell<Option<(Vec<Item>, Vec<f64>, String)>>> = Rc::new(RefCell::new(None));
    set_interval(Duration::from_millis(10), move || {
        let p = st.info.get_untracked();
        let every = match p.kind.as_str() {
            "select" => 200,
            "filter" => 250,
            "compute" => 300,
            "tab" | "compute_load" => 400,
            _ => return,
        };
        if last.with(|l| l.elapsed()) < Duration::from_millis(every) {
            return;
        }
        last.replace(Instant::now());
        let n = k.with(|k| {
            *k += 1;
            *k
        });
        let d = DRIVER.get().unwrap();
        if p.kind == "compute" || p.kind == "compute_load" {
            let mut c = calc.borrow_mut();
            let (items, closes, json) = c.get_or_insert_with(|| (bench_core::items(100_000), bench_core::closes(100_000), bench_core::big_json(33_000)));
            let task = if p.kind == "compute_load" { "agg_100k" } else { bench_core::COMPUTE_TASKS[n % 4] };
            let t0 = Instant::now();
            // black_box：防止编译器把结果没被使用的计算整段优化掉
            match task {
                "sort_100k" => drop(std::hint::black_box(bench_core::task_sort(std::hint::black_box(items)))),
                "agg_100k" => drop(std::hint::black_box(bench_core::task_agg(std::hint::black_box(items)))),
                "ema_100k" => drop(std::hint::black_box(bench_core::task_ema(std::hint::black_box(closes)))),
                _ => drop(std::hint::black_box(bench_core::task_json(std::hint::black_box(json)))),
            }
            d.metrics.record_compute(task, t0.elapsed().as_secs_f64() * 1000.0);
            return;
        }
        d.metrics.on_trigger();
        match p.kind.as_str() {
            "select" => st.selected.update(|s| *s = (*s + 1) % bench_core::SYMBOLS),
            "tab" => st.tab.update(|t| *t = 1 - *t),
            _ => {
                let term = bench_core::FILTER_TERMS[n % bench_core::FILTER_TERMS.len()];
                let (hits, total) = ITEMS50K.with(|items| bench_core::filter(items, term));
                batch(|| {
                    st.term.set(term.to_owned());
                    st.hits.set(hits);
                    st.total.set(total);
                });
            }
        }
    });

    let kind = move || st.info.with(|i| i.kind.clone());
    let param = computed(move || st.info.with(|i| i.param as usize));
    view()
        .flex_1()
        .bg(BG)
        .child(
            view()
                .flex_1()
                .child(If::new(move || kind() == "anim_js", move || dynamic(move || AnimJs(param.get()))))
                .child(If::new(move || kind() == "anim_native", move || dynamic(move || AnimNative(param.get()))))
                .child(If::new(move || kind() == "tab", move || tabs(st)))
                .child(If::new(move || kind() == "filter", move || filter_view(st)))
                .child(If::new(move || !["anim_js", "anim_native", "tab", "filter"].contains(&kind().as_str()), move || quotes(st))),
        )
        .child(
            view()
                .h(px(22.0))
                .flex_row()
                .items_center()
                .px(px(8.0))
                .gap(px(16.0))
                .border_t_1()
                .border_color(LINE)
                .child(text(move || format!("NSH Rust · {} · 阶段 {}", bench_core::scenario(), st.info.with(|i| i.name.clone()))).text_size(px(12.0)).text_color(MUTED))
                .child(text(move || format!("批次 {}", st.batches.get())).text_size(px(12.0)).text_color(MUTED)),
        )
}

fn apply(st: State, seq: &std::cell::Cell<u64>, last_change: Stored<f64>, b: Batch) {
    let t = unix_ms();
    let changed = !b.rows.is_empty();
    batch(|| {
        for r in b.rows {
            let s = row_of(st, r.idx as usize);
            let prev = s.with_untracked(|c| c.row.last);
            let dir = if r.last > prev { 1 } else if r.last < prev { -1 } else { 0 };
            seq.set(seq.get() + 1);
            s.set(Cell { row: r, at: t, dir, seq: seq.get() });
        }
        if let Some(bids) = b.bids {
            st.bids.set(bids);
        }
        if let Some(asks) = b.asks {
            st.asks.set(asks);
        }
        if !b.trades.is_empty() {
            st.trades.update(|tr| {
                for x in b.trades.into_iter() {
                    tr.insert(0, x);
                }
                tr.truncate(200);
            });
        }
        st.batches.update(|n| *n += 1);
    });
    if changed {
        last_change.replace(t);
    }
    if let Some(d) = DRIVER.get() {
        d.metrics.on_applied(b.seq);
    }
}

fn quote_row(st: State, i: impl Fn() -> usize + Copy + 'static) -> impl IntoElement {
    // 绑定里用 with 借用当前行，不复制整行
    let with = move |f: fn(&Cell) -> String| move || row_of(st, i()).with(f);
    let chg = move || row_of(st, i()).with(|c| c.row.last - c.row.prev);
    let color = move || if chg() > 0.0 { UP } else if chg() < 0.0 { DOWN } else { FG };
    let flash = move || {
        let (dir, at) = row_of(st, i()).with(|c| (c.dir, c.at));
        if dir == 0 || st.now.get() - at >= FLASH { Color::TRANSPARENT } else if dir > 0 { UP_FLASH } else { DOWN_FLASH }
    };
    let sel = move || st.info.with(|p| p.kind == "select") && st.selected.get() == i();
    view()
        .flex_row()
        .h(px(ROW_H))
        .items_center()
        .border_b_1()
        .border_color(ROW_LINE)
        .bg(move || if sel() { SEL } else { Color::TRANSPARENT })
        .child(txt(with(|c| c.row.code.clone()), 80.0, false, FG))
        .child(txt(with(|c| c.row.name.clone()), 110.0, false, MUTED))
        .child(txt(with(|c| format!("{:.3}", c.row.last)), 80.0, true, color).bg(flash))
        .child(txt(with(|c| signed(c.row.last - c.row.prev, 3)), 70.0, true, color))
        .child(txt(with(|c| format!("{}%", signed((c.row.last - c.row.prev) / c.row.prev * 100.0, 2))), 70.0, true, color))
        .child(txt(with(|c| c.row.vol.to_string()), 100.0, true, FG))
        .child(txt(with(|c| format!("{:.0}", c.row.turnover)), 110.0, true, FG))
        .child(txt(with(|c| format!("{:.3}", c.row.high)), 80.0, true, FG))
        .child(txt(with(|c| format!("{:.3}", c.row.low)), 80.0, true, FG))
        .child(txt(with(|c| format!("{:.3} / {:.3}", c.row.bid, c.row.ask)), 130.0, true, FG))
}

/// 详情：选中行的 40 个字段
fn detail(st: State, i: impl Fn() -> usize + Copy + 'static) -> impl IntoElement {
    let fields = move || {
        let r = row_of(st, i()).with(|c| c.row.clone());
        let chg = r.last - r.prev;
        let mut kv: Vec<(String, String)> = vec![
            ("代码".into(), r.code.clone()),
            ("名称".into(), r.name.clone()),
            ("最新价".into(), format!("{:.3}", r.last)),
            ("昨收".into(), format!("{:.3}", r.prev)),
            ("涨跌".into(), signed(chg, 3)),
            ("涨跌幅".into(), format!("{}%", signed(chg / r.prev * 100.0, 2))),
            ("最高".into(), format!("{:.3}", r.high)),
            ("最低".into(), format!("{:.3}", r.low)),
            ("买一".into(), format!("{:.3}", r.bid)),
            ("卖一".into(), format!("{:.3}", r.ask)),
            ("成交量".into(), r.vol.to_string()),
            ("成交额".into(), format!("{:.0}", r.turnover)),
        ];
        for k in 0..28 {
            kv.push((format!("指标{k:02}"), format!("{:.4}", r.last * (k + 1) as f64 / 7.0)));
        }
        kv
    };
    let field = move |k: usize| fields()[k].clone();
    view().flex_row().flex_wrap().p(px(8.0)).child(Each::new(
        || (0..40).collect::<Vec<usize>>(),
        |k: &usize| *k,
        move |k, _| {
            let k = k.get_untracked();
            view()
                .w(relative(0.5))
                .h(px(ROW_H))
                .flex_row()
                .items_center()
                .justify_between()
                .pr(px(12.0))
                .child(text(move || field(k).0).text_size(px(12.0)).text_color(MUTED))
                .child(text(move || field(k).1).text_size(px(12.0)).text_color(FG).tabular_nums())
        },
    ))
}

fn depth(st: State) -> impl IntoElement {
    let max = move || st.bids.with(|b| st.asks.with(|a| b.iter().chain(a.iter()).map(|l| l.vol).max().unwrap_or(1).max(1))) as f64;
    let side = move |up: bool| {
        let list = if up { st.bids } else { st.asks };
        view().flex_1().bg(BG).child(Each::new(
            move || (0..list.with(Vec::len)).collect::<Vec<usize>>(),
            |k: &usize| *k,
            move |k, _| {
                let k = k.get_untracked();
                let l = move || list.with(|v| v.get(k).cloned());
                view()
                    .h(px(ROW_H))
                    .flex_row()
                    .items_center()
                    .child(
                        view()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right_0()
                            .w(move || relative(l().map_or(0.0, |l| l.vol as f64 / max()) as f32))
                            .bg(if up { UP_BG } else { DOWN_BG }),
                    )
                    .child(txt(move || l().map_or(String::new(), |l| format!("{:.3}", l.price)), 70.0, false, if up { UP } else { DOWN }))
                    .child(txt(move || l().map_or(String::new(), |l| l.vol.to_string()), 80.0, true, FG))
                    .child(txt(move || l().map_or(String::new(), |l| format!("({})", l.orders)), 50.0, true, MUTED))
            },
        ))
    };
    view().flex_row().gap(px(1.0)).bg(LINE).child(side(true)).child(side(false))
}

fn trades(st: State) -> impl IntoElement {
    view().flex_1().overflow_hidden().child(Each::new(
        move || (0..st.trades.with(|t| t.len().min(40))).collect::<Vec<usize>>(),
        |k: &usize| *k,
        move |k, _| {
            let k = k.get_untracked();
            let t = move || st.trades.with(|v| v.get(k).cloned());
            view()
                .h(px(ROW_H))
                .flex_row()
                .items_center()
                .child(txt(move || t().map_or(String::new(), |t| (t.time_ms / 1000).to_string()), 70.0, false, MUTED))
                .child(txt(
                    move || t().map_or(String::new(), |t| format!("{:.3}", t.price)),
                    80.0,
                    true,
                    move || if t().is_some_and(|t| t.dir == 1) { UP } else { DOWN },
                ))
                .child(txt(move || t().map_or(String::new(), |t| t.vol.to_string()), 80.0, true, FG))
        },
    ))
}

const HEAD: [(&str, f32, bool); 10] = [
    ("代码", 80.0, false),
    ("名称", 110.0, false),
    ("最新价", 80.0, true),
    ("涨跌", 70.0, true),
    ("涨跌幅", 70.0, true),
    ("成交量", 100.0, true),
    ("成交额", 110.0, true),
    ("最高", 80.0, true),
    ("最低", 80.0, true),
    ("买一/卖一", 130.0, true),
];

fn quotes(st: State) -> impl IntoElement {
    let list = element_ref();
    let alive = Rc::new(std::cell::Cell::new(true));
    let flag = alive.clone();
    cleanup(move || flag.set(false));
    // 自动滚动：只在要求滚动的阶段逐帧推进
    spawn(async move {
        let mut ix = 0_u32;
        while alive.get() {
            if DRIVER.get().is_some_and(|d| d.scroll.load(Ordering::Relaxed)) {
                next_frame().await;
                ix = (ix + 1) % bench_core::SYMBOLS as u32;
                list.scroll_to_index(ix, 0.0, 0.0, false);
            } else {
                timer(Duration::from_millis(100)).await;
            }
        }
    });
    view()
        .flex_1()
        .flex_row()
        .gap(px(1.0))
        .bg(LINE)
        .child(
            view()
                .flex_1()
                .bg(BG)
                .child(
                    view()
                        .h(px(24.0))
                        .flex_row()
                        .items_center()
                        .border_b_1()
                        .border_color(LINE)
                        .children(HEAD.iter().map(|(t, w, r)| txt(t.to_string(), *w, *r, MUTED))),
                )
                .child(virtual_list(bench_core::SYMBOLS, move |at| quote_row(st, move || at.index())).flex_1().element_ref(&list)),
        )
        .child(
            view().w(px(420.0)).bg(BG).child(
                If::new(move || st.info.with(|p| p.kind == "select"), move || detail(st, move || st.selected.get()))
                    .otherwise(move || view().flex_1().child(depth(st)).child(trades(st))),
            ),
        )
}

/// 逻辑层逐帧驱动：每帧写时间 signal，N 个方块的位置由它算出
#[allow(non_snake_case)]
fn AnimJs(n: usize) -> impl IntoElement {
    let t = signal(0.0_f64);
    let t0 = Instant::now();
    let alive = Rc::new(std::cell::Cell::new(true));
    let flag = alive.clone();
    cleanup(move || flag.set(false));
    spawn(async move {
        while alive.get() {
            next_frame().await;
            t.set(t0.elapsed().as_secs_f64());
        }
    });
    view().flex_1().overflow_hidden().child(Each::new(
        move || (0..n).collect::<Vec<usize>>(),
        |i: &usize| *i,
        move |i, _| {
            let i = i.get_untracked();
            let ph = move || t.get() * std::f64::consts::PI + i as f64 * 0.1;
            view()
                .absolute()
                .left(move || px(((ph().sin() * 0.5 + 0.5) * 1400.0) as f32))
                .top(px((i % 40) as f32 * 22.0 + 4.0))
                .w(px(16.0))
                .h(px(16.0))
                .rounded(px(2.0))
                .bg(if i % 2 == 0 { UP } else { DOWN })
                .opacity(move || (0.3 + 0.7 * ph().cos().powi(2)) as f32)
        },
    ))
}

/// 原生动画值：每个方块一条循环动画，引擎逻辑不参与逐帧
#[allow(non_snake_case)]
fn AnimNative(n: usize) -> impl IntoElement {
    view().flex_1().overflow_hidden().child(Each::new(
        move || (0..n).collect::<Vec<usize>>(),
        |i: &usize| *i,
        move |i, _| {
            let i = i.get_untracked();
            let v = animated_value(0.0);
            let go = |to: f32| Animation::timing(&v, to, Timing::new(1000.0, Easing::Curve(Curve::Sin, EaseMode::InOut)));
            let anim = Animation::sequence([Animation::delay(((i * 32) % 1000) as f32), go(1.0), go(0.0)]).looped(None);
            let _ = anim.start();
            cleanup(move || anim.stop());
            view()
                .absolute()
                .left_0()
                .top(px((i % 40) as f32 * 22.0 + 4.0))
                .w(px(16.0))
                .h(px(16.0))
                .rounded(px(2.0))
                .bg(if i % 2 == 0 { UP } else { DOWN })
                .bind_translate_x(&v.interpolate(&[0.0, 1.0], &[0.0, 1400.0]))
                .bind_opacity(&v.interpolate(&[0.0, 0.5, 1.0], &[1.0, 0.3, 1.0]))
        },
    ))
}

/// 切换重页签：150 行非虚拟列表 ↔ 40 字段详情
fn tabs(st: State) -> impl IntoElement {
    let tab = st.tab;
    view()
        .flex_1()
        .overflow_hidden()
        .child(
            view()
                .h(px(28.0))
                .flex_row()
                .items_center()
                .gap(px(16.0))
                .px(px(8.0))
                .border_b_1()
                .border_color(LINE)
                .child(text("持仓明细").text_size(px(12.0)).text_color(move || if tab.get() == 0 { FG } else { MUTED }))
                .child(text("账户概览").text_size(px(12.0)).text_color(move || if tab.get() == 1 { FG } else { MUTED })),
        )
        .child(
            If::new(
                move || tab.get() == 0,
                move || {
                    view().children((0..150).map(move |i| {
                        let r = row_of(st, i).get_untracked().row;
                        view()
                            .h(px(ROW_H))
                            .flex_row()
                            .items_center()
                            .border_b_1()
                            .border_color(ROW_LINE)
                            .child(txt(r.code.clone(), 80.0, false, FG))
                            .child(txt(r.name.clone(), 110.0, false, MUTED))
                            .child(txt(format!("{:.3}", r.last), 80.0, true, FG))
                            .child(txt(r.vol.to_string(), 100.0, true, FG))
                            .child(txt(format!("{:.0}", r.turnover), 110.0, true, FG))
                            .child(txt(format!("{:.3}", r.high), 80.0, true, FG))
                    }))
                },
            )
            .otherwise(move || detail(st, || 0)),
        )
}

/// 5 万行搜索过滤：结果前 100 条用虚拟列表显示
fn filter_view(st: State) -> impl IntoElement {
    let items = ITEMS50K.with(Rc::clone);
    view()
        .flex_1()
        .child(
            view()
                .h(px(28.0))
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .px(px(8.0))
                .border_b_1()
                .border_color(LINE)
                .child(text(move || format!("搜索：{}", st.term.get())).text_size(px(12.0)).text_color(FG).border_1().border_color(LINE).px(px(8.0)))
                .child(text(move || format!("共 {} 条，显示前 100", st.total.get())).text_size(px(12.0)).text_color(MUTED)),
        )
        .child(
            virtual_list(move || st.hits.with(Vec::len), move |at| {
                let items = items.clone();
                let it = move || st.hits.with(|h| h.get(at.index()).map(|&i| items[i].clone()));
                let it2 = it.clone();
                let it3 = it.clone();
                let it4 = it.clone();
                view()
                    .h(px(ROW_H))
                    .flex_row()
                    .items_center()
                    .child(txt(move || it().map_or(String::new(), |i| i.code), 80.0, false, FG))
                    .child(txt(move || it2().map_or(String::new(), |i| i.name), 140.0, false, MUTED))
                    .child(txt(move || it3().map_or(String::new(), |i| format!("{:.3}", i.price)), 80.0, true, FG))
                    .child(txt(move || it4().map_or(String::new(), |i| format!("{}%", signed(i.chg_pct, 2))), 80.0, true, FG))
            })
            .keys(move || st.hits.get())
            .flex_1(),
        )
}
