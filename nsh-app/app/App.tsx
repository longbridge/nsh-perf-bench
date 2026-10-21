// 技术选型基准 · NSH TSX 版：与 Rust 版同场景、同数据、同动作节奏（见 bench-core::schedule）。
// quotes：500 行行情表 + 盘口 + 逐笔；animation：JS 逐帧驱动 / 原生动画值；interaction：选行、切页签、搜索过滤；compute：重计算。
import { batch, cleanup, mount, signal, Text, View, type Signal, type YNode } from "nsh";
import { List } from "nsh/list";
import { nativeModule } from "nsh/native";
import { Animated, createAnimatedValue, Easing } from "nsh/animated";
import * as W from "./work";

interface Row { idx: number; code: string; name: string; last: number; prev: number; vol: number; turnover: number; high: number; low: number; bid: number; ask: number }
interface Level { price: number; vol: number; orders: number }
interface Trade { time_ms: number; price: number; vol: number; dir: number }
interface Batch { seq: number; emit_ns: number; rows: Row[]; bids: Level[] | null; asks: Level[] | null; trades: Trade[] }
interface PhaseInfo { name: string; kind: string; param: number; scenario: string }
interface BenchApi {
  start(onBatch: (json: string) => void): Promise<void>;
  applied(seq: number): void;
  scroll(): boolean;
  initial(): string;
  phaseInfo(): string;
  mark(): void;
  compute(task: string, ms: number): void;
  bigJson(): string;
  // hybrid（TSX 界面 + Rust 模块）
  hybrid(): boolean;
  startView(onView: (json: string) => void): Promise<void>;
  appliedPost(id: number): void;
  initialView(): string;
  detail(idx: number): string;
  task(name: string): number;
  filter(term: string): string;
  // BENCH_TRACE 诊断
  trace(): boolean;
  traceJs(id: number, parse: number, apply: number): void;
  span(name: string, ms: number): void;
  markTab(toDetail: boolean): void;
  noop(): void;
  noopStr(name: string): number;
  taskTimed(name: string): number;
}
const { Bench } = nativeModule<{ Bench: BenchApi }>("host/bench");
/** BENCH_TRACE=1：记录解析、写 signal 等分段耗时 */
const TRACE = Bench.trace();

const C = { bg: "#161419", line: "#2e2c33", fg: "#e7e6eb", muted: "#656466", up: "#20a16e", down: "#cc3333", upFlash: "rgba(32,161,110,0.45)", downFlash: "rgba(204,51,51,0.45)", upBg: "rgba(32,161,110,0.14)", downBg: "rgba(204,51,51,0.14)", sel: "rgba(55,160,255,0.14)" };
const ROW_H = 22;
const FLASH = 600;
const NUM = { fontVariant: ["tabular-nums"] as ["tabular-nums"] };

type Cell = { row: Row; at: number; dir: number };
const init = JSON.parse(Bench.initial()) as Row[];
const rows: Signal<Cell>[] = init.map((row) => signal({ row, at: 0, dir: 0 }));
const indices = init.map((_, i) => i);
const bids = signal<Level[]>([]);
const asks = signal<Level[]>([]);
const trades = signal<Trade[]>([]);
const batches = signal(0);
const info = signal<PhaseInfo>({ name: "", kind: "idle", param: 0, scenario: "" });
const now = signal(Date.now());
let lastChange = 0;
// 交互
const selected = signal(0);
const tab = signal(0);
const term = signal("");
const hits = signal<number[]>([]);
const total = signal(0);
/** hybrid：行情合并、格式化、计算、过滤都在 Rust 模块里，JS 只拿显示用的字符串写 signal */
const HY = Bench.hybrid();
const items50k = HY ? [] : W.items(50_000);

type HCell = { s: string[]; c: number; f: number; at: number };
type HLevel = [string, string, string, number];
type HTrade = [string, string, string, number];
const hrows: Signal<HCell>[] = HY ? (JSON.parse(Bench.initialView()) as [string[], number][]).map(([s, c]) => signal({ s, c, f: 0, at: 0 })) : [];
const hbids = signal<HLevel[]>([]);
const hasks = signal<HLevel[]>([]);
const htrades = signal<HTrade[]>([]);
const hhits = signal<string[][]>([]);

function applyView(json: string) {
  const t0 = TRACE ? performance.now() : 0;
  const v = JSON.parse(json) as { id: number; rows: [number, string[], number, number][]; bids: HLevel[] | null; asks: HLevel[] | null; trades: HTrade[] | null };
  const t1 = TRACE ? performance.now() : 0;
  const t = Date.now();
  batch(() => {
    for (const [i, s, c, f] of v.rows) hrows[i].set({ s, c, f, at: t });
    if (v.bids) hbids.set(v.bids);
    if (v.asks) hasks.set(v.asks);
    if (v.trades) htrades.set(v.trades);
    batches.set(batches() + 1);
  });
  if (v.rows.length) lastChange = t;
  if (TRACE) Bench.traceJs(v.id, t1 - t0, performance.now() - t1);
  Bench.appliedPost(v.id);
}

function apply(json: string) {
  const t0 = TRACE ? performance.now() : 0;
  const b = JSON.parse(json) as Batch;
  const t1 = TRACE ? performance.now() : 0;
  const t = Date.now();
  batch(() => {
    for (const r of b.rows) {
      const s = rows[r.idx];
      const prev = s().row.last;
      s.set({ row: r, at: t, dir: r.last > prev ? 1 : r.last < prev ? -1 : 0 });
    }
    if (b.bids) bids.set(b.bids);
    if (b.asks) asks.set(b.asks);
    if (b.trades.length) trades.set([...[...b.trades].reverse(), ...trades()].slice(0, 200));
    batches.set(batches() + 1);
  });
  if (b.rows.length) lastChange = t;
  if (TRACE) Bench.traceJs(b.seq, t1 - t0, performance.now() - t1);
  Bench.applied(b.seq);
}

setInterval(() => {
  const p = JSON.parse(Bench.phaseInfo()) as PhaseInfo;
  if (p.name !== info().name) info.set(p);
  if (Date.now() - lastChange < FLASH + 100) now.set(Date.now());
}, 100);

// 场景动作：选行 200ms / 搜索 250ms / 计算 300ms / 切页签与推送中计算 400ms（同 Rust 版）
let calc: { items: W.Item[]; closes: number[]; json: string } | undefined;
let last = 0;
let k = 0;
const EVERY: Record<string, number> = { select: 200, filter: 250, compute: 300, tab: 400, compute_load: 400, compute_probe: 100 };
let probe: W.ProbeData | undefined;
const probeSeen = new Set<string>();
/** compute_ab：一项细分测量；首次执行另记 `.first` */
function probeSpan(name: string, f: () => unknown, check?: (r: unknown) => boolean) {
  const t0 = performance.now();
  const r = f();
  const ms = performance.now() - t0;
  Bench.span(probeSeen.has(name) ? name : `${name}.first`, ms);
  probeSeen.add(name);
  if (check && !check(r)) Bench.span(`${name}.invalid`, 1);
}
const PROBES_JS: (() => void)[] = [
  () => probeSpan("js.call_noop_x1000", () => { for (let i = 0; i < 1000; i++) Bench.noop(); }),
  () => probeSpan("js.call_str_x1000", () => { for (let i = 0; i < 1000; i++) Bench.noopStr("sort_100k"); }),
  () => probeSpan("sort.full", () => W.taskSort(probe!.items)),
  () => probeSpan("sort.slice", () => probe!.items.slice()),
  () => {
    const v = probe!.items.slice();
    probeSpan("sort.cmp_only", () => v.sort((a, b) => b.chg_pct - a.chg_pct), (r) => W.sortedDesc((r as W.Item[]).map((x) => x.chg_pct)));
  },
  () => {
    const v = probe!.chg.slice();
    probeSpan("sort.f64_native", () => v.sort().reverse(), (r) => W.sortedDesc(r as Float64Array));
  },
  () => {
    const v = Array.from(probe!.chg);
    probeSpan("sort.num_cmp", () => v.sort((a, b) => b - a), (r) => W.sortedDesc(r as number[]));
  },
  () => {
    let n = 0;
    const v = probe!.items.slice();
    v.sort((a, b) => (n++, b.chg_pct - a.chg_pct));
    Bench.span("sort.cmp_calls", n);
  },
  () => probeSpan("agg.obj", () => W.taskAgg(probe!.items)),
  () => probeSpan("agg.soa", () => W.aggSoa(probe!), (r) => Math.abs((r as number) - W.taskAgg(probe!.items)) < 1e-3 * Math.abs(r as number)),
  () => probeSpan("ema.arr", () => W.taskEma(probe!.closes)),
  () => probeSpan("ema.f64", () => W.emaF64(probe!.closesF64), (r) => r === W.taskEma(probe!.closes)),
  () => probeSpan("json.obj", () => W.taskJson(probe!.json)),
  () => probeSpan("json.arr", () => (JSON.parse(probe!.jsonArr) as unknown[]).length),
];
const PROBES_HY: (() => void)[] = [
  () => probeSpan("js.call_noop_x1000", () => { for (let i = 0; i < 1000; i++) Bench.noop(); }),
  () => probeSpan("js.call_str_x1000", () => { for (let i = 0; i < 1000; i++) Bench.noopStr("sort_100k"); }),
  ...W.COMPUTE_TASKS.map((task) => () => {
    const t0 = performance.now();
    const inner = Bench.taskTimed(task);
    const outer = performance.now() - t0;
    Bench.span(`hy.${task}.outer`, outer);
    Bench.span(`hy.${task}.inner`, inner);
    Bench.span(`hy.${task}.bridge_us`, (outer - inner) * 1000);
  }),
];
setInterval(() => {
  const p = info();
  const ms = EVERY[p.kind];
  if (!ms || performance.now() - last < ms) return;
  last = performance.now();
  k++;
  if (p.kind === "compute_probe") {
    if (!HY) {
      calc ??= { items: W.items(100_000), closes: W.closes(100_000), json: Bench.bigJson() };
      probe ??= W.probeData(calc.items, calc.closes, calc.json);
    }
    const list = HY ? PROBES_HY : PROBES_JS;
    list[k % list.length]();
    return;
  }
  if (p.kind === "compute" || p.kind === "compute_load") {
    const task = p.kind === "compute_load" ? "agg_100k" : W.COMPUTE_TASKS[k % 4];
    if (HY) {
      // 计时包含跨语言调用开销
      const t0 = performance.now();
      Bench.task(task);
      Bench.compute(task, performance.now() - t0);
      return;
    }
    calc ??= { items: W.items(100_000), closes: W.closes(100_000), json: Bench.bigJson() };
    const t0 = performance.now();
    if (task === "sort_100k") W.taskSort(calc.items);
    else if (task === "agg_100k") W.taskAgg(calc.items);
    else if (task === "ema_100k") W.taskEma(calc.closes);
    else W.taskJson(calc.json);
    Bench.compute(task, performance.now() - t0);
    return;
  }
  if (p.kind === "tab") Bench.markTab(tab() === 0);
  else Bench.mark();
  if (p.kind === "select") selected.set((selected() + 1) % rows.length);
  else if (p.kind === "tab") tab.set(1 - tab());
  else {
    const t = W.FILTER_TERMS[k % W.FILTER_TERMS.length];
    if (HY) {
      const r = JSON.parse(Bench.filter(t)) as { total: number; rows: string[][] };
      batch(() => {
        term.set(t);
        hhits.set(r.rows);
        total.set(r.total);
      });
      return;
    }
    const r = W.filter(items50k, t);
    batch(() => {
      term.set(t);
      hits.set(r.hits);
      total.set(r.total);
    });
  }
}, 10);

function Txt(props: { children: unknown; w: number; right?: boolean; color?: string; bg?: string }) {
  return (
    <Text numberOfLines={1} style={{ width: props.w, paddingHorizontal: 4, fontSize: 12, textAlign: props.right ? "right" : "left", color: props.color ?? C.fg, backgroundColor: props.bg, ...NUM }}>
      {props.children as string}
    </Text>
  );
}

function QuoteRow(props: { i: number }) {
  if (HY) return <HQuoteRow i={props.i} />;
  const cell = () => rows[props.i]();
  const r = () => cell().row;
  const chg = () => r().last - r().prev;
  const color = () => (chg() > 0 ? C.up : chg() < 0 ? C.down : C.fg);
  const flash = () => {
    const c = cell();
    if (!c.dir || now() - c.at >= FLASH) return undefined;
    return c.dir > 0 ? C.upFlash : C.downFlash;
  };
  const sel = () => info().kind === "select" && selected() === props.i;
  return (
    <View style={{ flexDirection: "row", height: ROW_H, alignItems: "center", borderBottomWidth: 1, borderColor: "rgba(46,44,51,0.5)", backgroundColor: sel() ? C.sel : undefined }}>
      <Txt w={80}>{r().code}</Txt>
      <Txt w={110} color={C.muted}>{r().name}</Txt>
      <Txt w={80} right color={color()} bg={flash()}>{r().last.toFixed(3)}</Txt>
      <Txt w={70} right color={color()}>{(chg() > 0 ? "+" : "") + chg().toFixed(3)}</Txt>
      <Txt w={70} right color={color()}>{(chg() > 0 ? "+" : "") + ((chg() / r().prev) * 100).toFixed(2) + "%"}</Txt>
      <Txt w={100} right>{String(r().vol)}</Txt>
      <Txt w={110} right>{r().turnover.toFixed(0)}</Txt>
      <Txt w={80} right>{r().high.toFixed(3)}</Txt>
      <Txt w={80} right>{r().low.toFixed(3)}</Txt>
      <Txt w={130} right>{`${r().bid.toFixed(3)} / ${r().ask.toFixed(3)}`}</Txt>
    </View>
  );
}

/** 详情面板：选中行的 40 个字段（同 Rust 版） */
function Detail(props: { i: number }) {
  const fields = (): [string, string][] => {
    if (TRACE) {
      const a = performance.now();
      const r = detailFields(props.i);
      Bench.span("detail.fields", performance.now() - a);
      return r;
    }
    return detailFields(props.i);
  };
  return (
    <View style={{ flexDirection: "row", flexWrap: "wrap", padding: 8 }}>
      <Each of={fields()} key="index">
        {(f) => (
          <View style={{ width: "50%", height: ROW_H, flexDirection: "row", alignItems: "center", justifyContent: "space-between", paddingRight: 12 }}>
            <Text style={{ fontSize: 12, color: C.muted }}>{f()[0]}</Text>
            <Text style={{ fontSize: 12, color: C.fg, ...NUM }}>{f()[1]}</Text>
          </View>
        )}
      </Each>
    </View>
  );
}

/** 详情的 40 个字段（TSX 现算；hybrid 由 Rust 生成） */
function detailFields(i: number): [string, string][] {
  if (HY) {
    hrows[i]();
    return JSON.parse(Bench.detail(i));
  }
  const r = rows[i]().row;
  const chg = r.last - r.prev;
  const kv: [string, string][] = [
    ["代码", r.code], ["名称", r.name], ["最新价", r.last.toFixed(3)], ["昨收", r.prev.toFixed(3)],
    ["涨跌", (chg > 0 ? "+" : "") + chg.toFixed(3)], ["涨跌幅", (chg > 0 ? "+" : "") + ((chg / r.prev) * 100).toFixed(2) + "%"],
    ["最高", r.high.toFixed(3)], ["最低", r.low.toFixed(3)], ["买一", r.bid.toFixed(3)], ["卖一", r.ask.toFixed(3)],
    ["成交量", String(r.vol)], ["成交额", r.turnover.toFixed(0)],
  ];
  for (let k = 0; k < 28; k++) kv.push([`指标${String(k).padStart(2, "0")}`, ((r.last * (k + 1)) / 7).toFixed(4)]);
  return kv;
}

function Depth() {
  if (HY) return <HDepth />;
  const max = () => Math.max(1, ...bids().map((l) => l.vol), ...asks().map((l) => l.vol));
  const side = (list: () => Level[], up: boolean) => (
    <View style={{ flex: 1, backgroundColor: C.bg }}>
      <Each of={list()} key="index">
        {(l) => (
          <View style={{ height: ROW_H, flexDirection: "row", alignItems: "center" }}>
            <View style={{ position: "absolute", top: 0, bottom: 0, right: 0, width: `${(l().vol / max()) * 100}%` as `${number}%`, backgroundColor: up ? C.upBg : C.downBg }} />
            <Txt w={70} color={up ? C.up : C.down}>{l().price.toFixed(3)}</Txt>
            <Txt w={80} right>{String(l().vol)}</Txt>
            <Txt w={50} right color={C.muted}>{`(${l().orders})`}</Txt>
          </View>
        )}
      </Each>
    </View>
  );
  return (
    <View style={{ flexDirection: "row", gap: 1, backgroundColor: C.line }}>
      {side(bids, true)}
      {side(asks, false)}
    </View>
  );
}

function Trades() {
  if (HY) return <HTrades />;
  return (
    <View style={{ flex: 1, overflow: "hidden" }}>
      <Each of={trades().slice(0, 40)} key="index">
        {(t) => (
          <View style={{ height: ROW_H, flexDirection: "row", alignItems: "center" }}>
            <Txt w={70} color={C.muted}>{String(Math.floor(t().time_ms / 1000))}</Txt>
            <Txt w={80} right color={t().dir === 1 ? C.up : C.down}>{t().price.toFixed(3)}</Txt>
            <Txt w={80} right>{String(t().vol)}</Txt>
          </View>
        )}
      </Each>
    </View>
  );
}

const HEAD: [string, number, boolean][] = [["代码", 80, false], ["名称", 110, false], ["最新价", 80, true], ["涨跌", 70, true], ["涨跌幅", 70, true], ["成交量", 100, true], ["成交额", 110, true], ["最高", 80, true], ["最低", 80, true], ["买一/卖一", 130, true]];

function Quotes() {
  let list: YNode | undefined;
  let ix = 0;
  let running = false;
  // 自动滚动：与 Rust 版一样只在滚动阶段逐帧、每帧滚一行（严格定位到顶部）；其他时间不跑 rAF
  const frame = () => {
    if (!Bench.scroll() || !list) {
      running = false;
      return;
    }
    ix = (ix + 1) % rows.length;
    list.scrollToIndex({ index: ix, animated: false, viewPosition: 0 });
    requestAnimationFrame(frame);
  };
  const poll = setInterval(() => {
    if (!running && Bench.scroll()) {
      running = true;
      requestAnimationFrame(frame);
    }
  }, 100);
  cleanup(() => clearInterval(poll));
  return (
    <View style={{ flex: 1, flexDirection: "row", gap: 1, backgroundColor: C.line }}>
      <View style={{ flex: 1, backgroundColor: C.bg }}>
        <View style={{ height: 24, flexDirection: "row", alignItems: "center", borderBottomWidth: 1, borderColor: C.line }}>
          <Each of={HEAD}>{(h) => <Txt w={h()[1]} right={h()[2]} color={C.muted}>{h()[0]}</Txt>}</Each>
        </View>
        <List ref={(n) => (list = n)} each={indices} keyExtractor={(i) => String(i)} style={{ flex: 1 }}>
          {(i) => <QuoteRow i={i()} />}
        </List>
      </View>
      <View style={{ width: 420, backgroundColor: C.bg }}>
        <If when={info().kind === "select"} else={<><Depth /><Trades /></>}>
          <Detail i={selected()} />
        </If>
      </View>
    </View>
  );
}

/** 原生动画的轨迹（animation_ab 对照，同 bench_core::native_curve）：返回 [位移, 透明度] */
function nativeCurve(i: number, tMs: number): [number, number] {
  const d = (i * 32) % 1000;
  const u = tMs % (d + 2000);
  let v = 0;
  if (u >= d) {
    const w = u - d;
    const x = w < 1000 ? w / 1000 : 2 - w / 1000;
    v = (1 - Math.cos(Math.PI * x)) / 2;
  }
  return [v * 1400, v < 0.5 ? 1 - 1.4 * v : 0.3 + 1.4 * (v - 0.5)];
}

/** JS 逐帧驱动：rAF 写一个时间 signal，N 个方块的样式各自由它算出。
 *  variant（animation_ab 对照）：anim_js 改 left；anim_js_tx 同轨迹改 translateX；anim_js_curve 走原生动画的轨迹（translateX） */
function AnimJs(props: { n: number; variant: string }) {
  const t = signal(0);
  const t0 = performance.now();
  let alive = true;
  cleanup(() => (alive = false));
  const frame = () => {
    if (!alive) return;
    const a = TRACE ? performance.now() : 0;
    t.set((performance.now() - t0) / 1000);
    if (TRACE) Bench.span("anim.set", performance.now() - a);
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  // 元素数随阶段变化（200 → 1000）：按数量增减行
  const boxes = () => Array.from({ length: props.n }, (_, i) => i);
  return (
    <View style={{ flex: 1, overflow: "hidden" }}>
      <Each of={boxes()}>
        {(i) => {
          const ph = () => t() * Math.PI + i() * 0.1;
          if (props.variant === "anim_js_tx" || props.variant === "anim_js_curve") {
            const curve = props.variant === "anim_js_curve";
            const x = () => (curve ? nativeCurve(i(), t() * 1000)[0] : (Math.sin(ph()) * 0.5 + 0.5) * 1400);
            const o = () => (curve ? nativeCurve(i(), t() * 1000)[1] : 0.3 + 0.7 * Math.cos(ph()) ** 2);
            return (
              <View
                style={{
                  position: "absolute",
                  left: 0,
                  top: (i() % 40) * 22 + 4,
                  width: 16,
                  height: 16,
                  borderRadius: 2,
                  backgroundColor: i() % 2 === 0 ? C.up : C.down,
                  transform: [{ translateX: x() }],
                  opacity: o(),
                }}
              />
            );
          }
          return (
            <View
              style={{
                position: "absolute",
                left: (Math.sin(ph()) * 0.5 + 0.5) * 1400,
                top: (i() % 40) * 22 + 4,
                width: 16,
                height: 16,
                borderRadius: 2,
                backgroundColor: i() % 2 === 0 ? C.up : C.down,
                opacity: 0.3 + 0.7 * Math.cos(ph()) ** 2,
              }}
            />
          );
        }}
      </Each>
    </View>
  );
}

/** 原生动画值：每个方块一条循环动画，跑在 UI 线程上，JS 不参与逐帧 */
function NativeBox(props: { i: number }) {
  const i = props.i;
  const v = createAnimatedValue(0);
  const go = (to: number) => Animated.timing(v, { toValue: to, duration: 1000, easing: Easing.inOut(Easing.sin) });
  const anim = Animated.loop(Animated.sequence([Animated.delay((i * 32) % 1000), go(1), go(0)]));
  anim.start();
  cleanup(() => anim.stop());
  return (
    <View
      style={{
        position: "absolute",
        left: 0,
        top: (i % 40) * 22 + 4,
        width: 16,
        height: 16,
        borderRadius: 2,
        backgroundColor: i % 2 === 0 ? C.up : C.down,
        transform: [{ translateX: v.interpolate({ inputRange: [0, 1], outputRange: [0, 1400] }) }],
        opacity: v.interpolate({ inputRange: [0, 0.5, 1], outputRange: [1, 0.3, 1] }),
      }}
    />
  );
}

function AnimNative(props: { n: number }) {
  const boxes = () => Array.from({ length: props.n }, (_, i) => i);
  return (
    <View style={{ flex: 1, overflow: "hidden" }}>
      <Each of={boxes()}>{(i) => <NativeBox i={i()} />}</Each>
    </View>
  );
}

/** 切换重页签：150 行非虚拟列表 ↔ 40 字段详情，切换时整块挂载 / 卸载 */
function Tabs() {
  return (
    <View style={{ flex: 1, overflow: "hidden" }}>
      <View style={{ height: 28, flexDirection: "row", alignItems: "center", gap: 16, paddingHorizontal: 8, borderBottomWidth: 1, borderColor: C.line }}>
        <Text style={{ fontSize: 12, color: tab() === 0 ? C.fg : C.muted }}>持仓明细</Text>
        <Text style={{ fontSize: 12, color: tab() === 1 ? C.fg : C.muted }}>账户概览</Text>
      </View>
      <If when={tab() === 0} else={<Detail i={0} />}>
        <View>
          <Each of={indices.slice(0, 150)}>
            {(i) => {
              if (HY) {
                const s = hrows[i()]().s;
                return (
                  <View style={{ height: ROW_H, flexDirection: "row", alignItems: "center", borderBottomWidth: 1, borderColor: "rgba(46,44,51,0.5)" }}>
                    <Txt w={80}>{s[0]}</Txt>
                    <Txt w={110} color={C.muted}>{s[1]}</Txt>
                    <Txt w={80} right>{s[2]}</Txt>
                    <Txt w={100} right>{s[5]}</Txt>
                    <Txt w={110} right>{s[6]}</Txt>
                    <Txt w={80} right>{s[7]}</Txt>
                  </View>
                );
              }
              const r = rows[i()]().row;
              return (
                <View style={{ height: ROW_H, flexDirection: "row", alignItems: "center", borderBottomWidth: 1, borderColor: "rgba(46,44,51,0.5)" }}>
                  <Txt w={80}>{r.code}</Txt>
                  <Txt w={110} color={C.muted}>{r.name}</Txt>
                  <Txt w={80} right>{r.last.toFixed(3)}</Txt>
                  <Txt w={100} right>{String(r.vol)}</Txt>
                  <Txt w={110} right>{r.turnover.toFixed(0)}</Txt>
                  <Txt w={80} right>{r.high.toFixed(3)}</Txt>
                </View>
              );
            }}
          </Each>
        </View>
      </If>
    </View>
  );
}

/** 5 万行搜索过滤：结果前 100 条用虚拟列表显示 */
function Filter() {
  return (
    <View style={{ flex: 1 }}>
      <View style={{ height: 28, flexDirection: "row", alignItems: "center", gap: 12, paddingHorizontal: 8, borderBottomWidth: 1, borderColor: C.line }}>
        <Text style={{ fontSize: 12, color: C.fg, borderWidth: 1, borderColor: C.line, paddingHorizontal: 8 }}>{`搜索：${term()}`}</Text>
        <Text style={{ fontSize: 12, color: C.muted }}>{`共 ${total()} 条，显示前 100`}</Text>
      </View>
      <If when={HY}>
        <List each={hhits()} keyExtractor={(r) => r[0]} style={{ flex: 1 }}>
          {(r) => (
            <View style={{ height: ROW_H, flexDirection: "row", alignItems: "center" }}>
              <Txt w={80}>{r()[0]}</Txt>
              <Txt w={140} color={C.muted}>{r()[1]}</Txt>
              <Txt w={80} right>{r()[2]}</Txt>
              <Txt w={80} right>{r()[3]}</Txt>
            </View>
          )}
        </List>
      </If>
      <If when={!HY}>
      <List each={hits()} keyExtractor={(i) => String(i)} style={{ flex: 1 }}>
        {(i) => {
          const it = () => items50k[i()];
          return (
            <View style={{ height: ROW_H, flexDirection: "row", alignItems: "center" }}>
              <Txt w={80}>{it().code}</Txt>
              <Txt w={140} color={C.muted}>{it().name}</Txt>
              <Txt w={80} right>{it().price.toFixed(3)}</Txt>
              <Txt w={80} right>{(it().chg_pct > 0 ? "+" : "") + it().chg_pct.toFixed(2) + "%"}</Txt>
            </View>
          );
        }}
      </List>
      </If>
    </View>
  );
}

// ---------------------------------------------------------------------------
// hybrid 版的行情组件：字段已在 Rust 里格式化好，这里只按字符串渲染
// ---------------------------------------------------------------------------

function HQuoteRow(props: { i: number }) {
  const cell = () => hrows[props.i]();
  const color = () => (cell().c > 0 ? C.up : cell().c < 0 ? C.down : C.fg);
  const flash = () => {
    const c = cell();
    if (!c.f || now() - c.at >= FLASH) return undefined;
    return c.f > 0 ? C.upFlash : C.downFlash;
  };
  const sel = () => info().kind === "select" && selected() === props.i;
  return (
    <View style={{ flexDirection: "row", height: ROW_H, alignItems: "center", borderBottomWidth: 1, borderColor: "rgba(46,44,51,0.5)", backgroundColor: sel() ? C.sel : undefined }}>
      <Txt w={80}>{cell().s[0]}</Txt>
      <Txt w={110} color={C.muted}>{cell().s[1]}</Txt>
      <Txt w={80} right color={color()} bg={flash()}>{cell().s[2]}</Txt>
      <Txt w={70} right color={color()}>{cell().s[3]}</Txt>
      <Txt w={70} right color={color()}>{cell().s[4]}</Txt>
      <Txt w={100} right>{cell().s[5]}</Txt>
      <Txt w={110} right>{cell().s[6]}</Txt>
      <Txt w={80} right>{cell().s[7]}</Txt>
      <Txt w={80} right>{cell().s[8]}</Txt>
      <Txt w={130} right>{cell().s[9]}</Txt>
    </View>
  );
}

function HDepth() {
  const side = (list: () => HLevel[], up: boolean) => (
    <View style={{ flex: 1, backgroundColor: C.bg }}>
      <Each of={list()} key="index">
        {(l) => (
          <View style={{ height: ROW_H, flexDirection: "row", alignItems: "center" }}>
            <View style={{ position: "absolute", top: 0, bottom: 0, right: 0, width: `${l()[3]}%` as `${number}%`, backgroundColor: up ? C.upBg : C.downBg }} />
            <Txt w={70} color={up ? C.up : C.down}>{l()[0]}</Txt>
            <Txt w={80} right>{l()[1]}</Txt>
            <Txt w={50} right color={C.muted}>{l()[2]}</Txt>
          </View>
        )}
      </Each>
    </View>
  );
  return (
    <View style={{ flexDirection: "row", gap: 1, backgroundColor: C.line }}>
      {side(hbids, true)}
      {side(hasks, false)}
    </View>
  );
}

function HTrades() {
  return (
    <View style={{ flex: 1, overflow: "hidden" }}>
      <Each of={htrades()} key="index">
        {(t) => (
          <View style={{ height: ROW_H, flexDirection: "row", alignItems: "center" }}>
            <Txt w={70} color={C.muted}>{t()[0]}</Txt>
            <Txt w={80} right color={t()[3] === 1 ? C.up : C.down}>{t()[1]}</Txt>
            <Txt w={80} right>{t()[2]}</Txt>
          </View>
        )}
      </Each>
    </View>
  );
}

function App() {
  if (HY) Bench.startView(applyView);
  else Bench.start(apply);
  const kind = () => info().kind;
  return (
    <View style={{ flex: 1, backgroundColor: C.bg }}>
      <View style={{ flex: 1 }}>
        <If when={kind().startsWith("anim_js")}>{() => <AnimJs n={info().param} variant={kind()} />}</If>
        <If when={kind() === "anim_native"}>{() => <AnimNative n={info().param} />}</If>
        <If when={kind() === "tab"}><Tabs /></If>
        <If when={kind() === "filter"}><Filter /></If>
        <If when={!(kind().startsWith("anim_") || ["tab", "filter"].includes(kind()))}><Quotes /></If>
      </View>
      <View style={{ height: 22, flexDirection: "row", alignItems: "center", paddingHorizontal: 8, gap: 16, borderTopWidth: 1, borderColor: C.line }}>
        <Text style={{ fontSize: 12, color: C.muted }}>{`NSH TSX · ${info().scenario} · 阶段 ${info().name}`}</Text>
        <Text style={{ fontSize: 12, color: C.muted }}>{`批次 ${batches()}`}</Text>
      </View>
    </View>
  );
}

mount(App);
