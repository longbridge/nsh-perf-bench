#!/usr/bin/env python3
"""诊断汇总：python3 scripts/diag_report.py <目录>... [--phases a,b] [--spans] [--counters]
读取目录下 *.json（scripts/probe.sh 的输出，可多次重复），按 (实现, 变体目录, 阶段) 取各次的中位数。
计数器里的 *_ns 换算成每秒毫秒（= 单核占用 ‰ 的十分之一；10 ms/s = 1% CPU）。"""
import json, pathlib, statistics, sys
from collections import defaultdict

args = [a for a in sys.argv[1:] if not a.startswith('--')]
opts = {a.split('=')[0]: (a.split('=')[1] if '=' in a else True) for a in sys.argv[1:] if a.startswith('--')}
only = set(opts['--phases'].split(',')) if '--phases' in opts else None
med = statistics.median
rows = defaultdict(list)
for d in args:
    for f in sorted(pathlib.Path(d).rglob('*.json')):
        try:
            r = json.loads(f.read_text())
        except Exception:
            continue
        if 'phases' not in r:
            continue
        group = f.parent.name
        for p in r['phases']:
            if p['name'] in ('warmup', 'cooldown') or (only and p['name'] not in only):
                continue
            rows[(r['implementation'], group, p['name'])].append(p)

def fmt(x, d=1):
    return '-' if x is None else f'{x:.{d}f}'

for (impl, group, phase), ps in sorted(rows.items(), key=lambda k: (k[0][2], k[0][0], k[0][1])):
    secs = med([p['secs'] for p in ps])
    th = defaultdict(list)
    for p in ps:
        for n, v in p.get('threads_cpu_pct', []):
            th[n].append(v)
    thr = ' '.join(f'{n}={med(v):.1f}' for n, v in sorted(th.items(), key=lambda x: -med(x[1])))
    fl = [p.get('frame_link') or {} for p in ps]
    ver = [x['verified_ms']['p50'] for x in fl if x.get('verified_ms')]
    print(f"== {phase:20} {impl:10} [{group}] n={len(ps)} fps {fmt(med([p['fps'] for p in ps]))} cpu {fmt(med([p['cpu_pct'] for p in ps]))}% | {thr}")
    print(f"   lat p50 {fmt(med([p['tick_to_frame_ms']['p50'] for p in ps]),2)} verified p50 {fmt(med(ver) if ver else None,2)} early {[x.get('early') for x in fl]} stale_draws {[x.get('stale_draws') for x in fl]} pending_end {[x.get('pending_at_end') for x in fl]} dropped_begin {[x.get('dropped_at_begin') for x in fl]} | draw p50 {fmt(med([p['draw_ms']['p50'] for p in ps]),2)} | applied {[p['applied_batches'] for p in ps]}/{[p['emitted_batches'] for p in ps]} lat.n {[p['tick_to_frame_ms']['n'] for p in ps]}")
    if '--counters' in opts:
        cs = defaultdict(list)
        for p in ps:
            for n, v in p.get('counters', []):
                cs[n].append(v / p['secs'])
        if cs:
            parts = []
            for n, v in cs.items():
                m = med(v)
                parts.append(f"{n[:-3]}={m/1e6:.1f}ms/s" if n.endswith('_ns') else f"{n}={m:.0f}/s")
            print('   ' + ' '.join(parts))
    if '--spans' in opts:
        sp = defaultdict(lambda: defaultdict(list))
        for p in ps:
            for s in p.get('spans_ms', []):
                for k in ('n', 'p50', 'p95', 'max', 'sum'):
                    sp[s['name']][k].append(s[k])
        for n, v in sorted(sp.items()):
            print(f"   span {n:24} n {med(v['n']):7.0f} p50 {med(v['p50']):8.3f} p95 {med(v['p95']):8.3f} max {med(v['max']):8.3f} sum/s {med(v['sum'])/secs:8.2f}")
    for p in ps[:1]:
        for k, c in p.get('compute_ms', []):
            print(f"   compute {k:14} p50 {med([dict(q['compute_ms'])[k]['p50'] for q in ps if k in dict(q['compute_ms'])]):8.3f}")
