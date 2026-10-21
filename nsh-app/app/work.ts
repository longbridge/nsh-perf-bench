// 交互 / 计算场景的数据与任务：与 bench-core（Rust）同算法、同规模。
export interface Item {
  code: string;
  name: string;
  market: number;
  sector: number;
  price: number;
  cost: number;
  qty: number;
  chg_pct: number;
  vol: number;
}

/** 确定性伪随机（数据生成不计时，与 Rust 数值不必逐位一致，规模一致即可） */
function rng(seed: number) {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s >>>= 0;
    s ^= s >>> 17;
    s ^= s << 5;
    s >>>= 0;
    return s / 4294967296;
  };
}

export function items(n: number): Item[] {
  const r = rng(7);
  const out: Item[] = new Array(n);
  for (let i = 0; i < n; i++) {
    const price = 1 + r() * 500;
    out[i] = {
      code: String((i * 37) % 1_000_000).padStart(6, "0"),
      name: `标的${i}`,
      market: i % 5,
      sector: Math.floor(r() * 20),
      price,
      cost: price * (0.7 + r() * 0.6),
      qty: 1 + Math.floor(r() * 5000),
      chg_pct: (r() - 0.5) * 20,
      vol: Math.floor(r() * 1e7),
    };
  }
  return out;
}

export function closes(n: number): number[] {
  const r = rng(11);
  let p = 100;
  const out = new Array<number>(n);
  for (let i = 0; i < n; i++) {
    p = Math.max(1, p + (r() - 0.5) * 2);
    out[i] = p;
  }
  return out;
}

/** 按涨跌幅降序排序（复制后排序） */
export function taskSort(list: Item[]) {
  const v = list.slice();
  v.sort((a, b) => b.chg_pct - a.chg_pct);
  return v[0].chg_pct + v[v.length - 1].chg_pct;
}

/** 持仓汇总：按 市场 × 行业 汇总市值与盈亏 */
export function taskAgg(list: Item[]) {
  const mv = new Float64Array(100);
  const pnl = new Float64Array(100);
  for (const it of list) {
    const k = it.market * 20 + it.sector;
    mv[k] += it.price * it.qty;
    pnl[k] += (it.price - it.cost) * it.qty;
  }
  let s = 0;
  for (let k = 0; k < 100; k++) s += mv[k] + pnl[k];
  return s;
}

/** K 线指标：EMA12 / EMA26 / MACD / Signal9 */
export function taskEma(c: number[]) {
  const a12 = 2 / 13, a26 = 2 / 27, a9 = 2 / 10;
  let e12 = c[0], e26 = c[0], sig = 0, hist = 0;
  for (let i = 0; i < c.length; i++) {
    e12 += a12 * (c[i] - e12);
    e26 += a26 * (c[i] - e26);
    const macd = e12 - e26;
    sig += a9 * (macd - sig);
    hist += macd - sig;
  }
  return hist;
}

export function taskJson(s: string) {
  return (JSON.parse(s) as unknown[]).length;
}

/** 搜索过滤：代码或名称包含 term，返回前 100 条下标与总数 */
export function filter(list: Item[], term: string): { hits: number[]; total: number } {
  const hits: number[] = [];
  let total = 0;
  for (let i = 0; i < list.length; i++) {
    const it = list[i];
    if (it.code.includes(term) || it.name.includes(term)) {
      total++;
      if (hits.length < 100) hits.push(i);
    }
  }
  return { hits, total };
}

export const FILTER_TERMS = ["1", "12", "123", "2", "23", "234", "标的9", "999"];
export const COMPUTE_TASKS = ["sort_100k", "agg_100k", "ema_100k", "json_5mb"];
