#!/usr/bin/env python3
"""samply 样本汇总：python3 scripts/samply_top.py <x.profile.json.gz> [线程名前缀...] [--top N] [--incl 正则]
按线程输出自身（self）与包含（inclusive）耗时最多的函数；符号来自同目录的 .syms.json（--unstable-presymbolicate）。"""
import bisect, collections, gzip, json, re, subprocess, sys

args = [a for a in sys.argv[1:] if not a.startswith('--')]
top = int(next((a.split('=')[1] for a in sys.argv if a.startswith('--top=')), 25))
incl_re = next((a.split('=', 1)[1] for a in sys.argv if a.startswith('--incl=')), None)
path, prefixes = args[0], args[1:] or ['nsh-engine', 'bench-nsh']
prof = json.load(gzip.open(path))
syms = json.load(open(path.replace('.gz', '').replace('.json', '.json.syms.json') if False else path[:-3] + '.syms.json'))
strings = syms['string_table']
tables = {}
for d in syms['data']:
    st = sorted(d['symbol_table'], key=lambda x: x['rva'])
    tables[d['debug_name']] = ([x['rva'] for x in st], st)

def demangle(names):
    try:
        out = subprocess.run(['rustfilt'], input='\n'.join(names), capture_output=True, text=True).stdout.split('\n')
        if len(out) >= len(names):
            return out[:len(names)]
    except FileNotFoundError:
        pass
    return [re.sub(r'::h[0-9a-f]{16}$', '', n) for n in names]

def short(name):
    name = re.sub(r'<[^<>]*>', '', name) if len(name) > 140 else name
    return name[:140]

for th in prof['threads']:
    if not any(th['name'].startswith(p) for p in prefixes):
        continue
    samples = th['samples']
    n = len(samples['stack'])
    if n < 50:
        continue
    st, ft, fn, rt = th['stackTable'], th['frameTable'], th['funcTable'], th['resourceTable']
    sa = th['stringArray']
    cache = {}
    def frame_name(f):
        if f in cache:
            return cache[f]
        func = ft['func'][f]
        res = fn['resource'][func]
        lib = prof['libs'][rt['lib'][res]]['debugName'] if res is not None and res >= 0 and rt['lib'][res] is not None else '?'
        addr = ft['address'][f]
        name = None
        if lib in tables and addr is not None and addr >= 0:
            rvas, entries = tables[lib]
            i = bisect.bisect_right(rvas, addr) - 1
            if i >= 0 and addr < entries[i]['rva'] + max(entries[i]['size'], 1):
                name = strings[entries[i]['symbol']]
        if name is None:
            name = sa[fn['name'][func]] if fn['name'][func] is not None else f'{lib}+{addr:#x}'
            if name.startswith('0x'):
                name = f'{lib}!{name}'
        cache[f] = name
        return name
    self_c, incl_c = collections.Counter(), collections.Counter()
    for s in samples['stack']:
        if s is None:
            continue
        fr = st['frame'][s]
        self_c[frame_name(fr)] += 1
        seen, cur = set(), s
        while cur is not None:
            nm = frame_name(st['frame'][cur])
            if nm not in seen:
                seen.add(nm)
                incl_c[nm] += 1
            cur = st['prefix'][cur]
    names = list({*[k for k, _ in self_c.most_common(top)], *[k for k, _ in incl_c.most_common(top * 3)]})
    dm = dict(zip(names, demangle(names)))
    print(f'\n### thread {th["name"]} (tid {th["tid"]}): {n} samples')
    print('  self%  function')
    for k, v in self_c.most_common(top):
        print(f'  {100 * v / n:5.1f}  {short(dm[k])}')
    print('  incl%  function' + (f' (matching {incl_re})' if incl_re else ''))
    shown = 0
    for k, v in incl_c.most_common(top * 3):
        if incl_re and not re.search(incl_re, dm[k]):
            continue
        print(f'  {100 * v / n:5.1f}  {short(dm[k])}')
        shown += 1
        if shown >= top:
            break
