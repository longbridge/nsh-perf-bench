#!/usr/bin/env python3
"""Summarize results/<time>/run*/<impl>-<scenario>.json: median over runs per scenario, phase and metric; writes REPORT.md (English)."""
import json, sys, glob, statistics, os
from pathlib import Path
out = str(Path(sys.argv[1]))
meta_path = Path(out, "metadata.json")
meta = json.loads(meta_path.read_text()) if meta_path.exists() else {}
env_path = Path(out, "env.txt")
linux = env_path.exists() and "Linux" in env_path.read_text()
memory_supported = meta.get("memory_supported", not linux)
data = {}  # scenario -> impl -> [report]
files = sorted(glob.glob(f"{out}/run*/*.json"))
if not files:
    sys.exit("no raw JSON results; cannot build a report")
NAMES = {"rust": "rust-gpui", "nsh": "nsh-tsx", "hybrid": "nsh-hybrid", "rustui": "nsh-rust",
         "nsh-gpui": "nsh-tsx-gpui", "hybrid-gpui": "nsh-hybrid-gpui", "rustui-gpui": "nsh-rust-gpui"}
if "repetitions" in meta:
    expected = {str(Path(out, f"run{i}", f"{NAMES[impl]}-{sc}.json"))
                for i in range(1, meta["repetitions"] + 1)
                for impl in meta["implementations"] for sc in meta["scenarios"]}
    if set(files) != expected:
        sys.exit("result files do not match metadata.json: runs are missing or extra")
for f in files:
    r = json.load(open(f))
    if Path(f).name != f"{r['implementation']}-{r.get('scenario', 'quotes')}.json":
        sys.exit(f"result identity does not match its file name: {f}")
    if "repetitions" in meta:
        standard_phases = {
            "quotes": ["warmup", "idle", "r200", "r1000", "r5000", "r5000_scroll", "r20000", "cooldown"],
            "animation": ["warmup", "anim_js_200", "anim_js_1000", "anim_native_200", "anim_native_1000", "cooldown"],
            "interaction": ["warmup", "select_row", "switch_tab", "filter_50k", "select_under_load", "cooldown"],
            "compute": ["warmup", "compute_100k", "compute_under_load", "cooldown"],
        }
        if [p["name"] for p in r["phases"]] != standard_phases[r["scenario"]]:
            sys.exit(f"standard scenario has missing phases: {f}")
    data.setdefault(r.get("scenario", "quotes"), {}).setdefault(r["implementation"], []).append(r)
for sc, runs in data.items():
    reference = next(iter(runs.values()))
    phases = [p["name"] for p in reference[0]["phases"]]
    if not phases or any(len(rs) != len(reference) or
                         any([p["name"] for p in r["phases"]] != phases for r in rs)
                         for rs in runs.values()):
        sys.exit(f"{sc}: run counts or phases differ between implementations; not a fair comparison")
med = lambda xs: statistics.median(xs) if xs else 0.0
def phase(r, ph):
    return next((p for p in r["phases"] if p["name"] == ph), None)
def pm(reports, ph, key, sub=None):
    v = [(p[key][sub] if sub else p[key]) for r in reports if (p := phase(r, ph))]
    return med(v)
def cm(reports, ph, task, sub):
    v = [pc[sub] for r in reports if (p := phase(r, ph)) for name, pc in p.get("compute_ms", []) if name == task]
    return med(v)
def latency(reports, ph, verified=False):
    ps = [phase(r, ph) for r in reports]
    if verified:
        vs = [(p.get("frame_link") or {}).get("verified_ms") for p in ps]
        if not all(vs):
            return "no samples"
        return f"{med([v['p50'] for v in vs]):.1f} / {med([v['p99'] for v in vs]):.1f}"
    if not all(p["tick_to_frame_ms"]["n"] > 0 for p in ps):
        return "no samples"
    return f"{pm(reports, ph, 'tick_to_frame_ms', 'p50'):.1f} / {pm(reports, ph, 'tick_to_frame_ms', 'p99'):.1f}"
def threads(reports, ph):
    acc = {}
    for r in reports:
        for n, v in (phase(r, ph) or {}).get("threads_cpu_pct", []):
            acc.setdefault(n, []).append(v)
    top = sorted(((n, med(v)) for n, v in acc.items()), key=lambda x: -x[1])[:3]
    return ", ".join(f"{n} {v:.1f}" for n, v in top) or "-"
L = [f"# NSH performance benchmark results ({os.path.basename(out)})", "",
     "With several runs each cell is the median of the per-run statistics; with one run it is that run's value. CPU: one core = 100%.", "",
     "Latency is from the update (or trigger) to the end of the first frame drawn after it, not physical scan-out. "
     "\"verified\" only counts a frame once the main thread has applied the commit carrying the update (NSH hosts); "
     "late frames only cover the work of the frames that were collected.", "",
     ("Implementations: `nsh-*` run on the platform's native NSH backend (Linux: GTK 4, macOS: AppKit); `nsh-*-gpui` on nsh-platform-gpui; "
      if meta.get("nsh_backend") == "native" else
      "Implementations: this run predates the native NSH hosts, so `nsh-*` ran on nsh-platform-gpui (gpui-fast 0.1.9); ")
     + "`rust-gpui` is plain gpui-kit on gpui-fast 0.1.10. In the quotes scenario fps reflects how often each implementation redraws, not its throughput: compare CPU.", ""]
if not memory_supported:
    L += ["Memory sampling is not implemented here; the 0 in the raw JSON is a placeholder and is shown as n/a.", ""]
for sc, runs in data.items():
    impls = sorted(runs)
    n = len(runs[impls[0]])
    L += [f"## Scenario: {sc} ({n} run{'s' if n > 1 else ''} each)", "", "| Phase / metric | " + " | ".join(impls) + " |", "|---|" + "---|" * len(impls)]
    L.append("| Startup to first frame ms | " + " | ".join(f"{med([r['startup_first_frame_ms'] for r in runs[i]]):.0f}" for i in impls) + " |")
    L.append("| Peak memory MB | " + " | ".join((f"{med([r['peak_footprint_mb'] for r in runs[i]]):.1f}" if memory_supported else "n/a") for i in impls) + " |")
    for ph in [p["name"] for p in runs[impls[0]][0]["phases"] if p["name"] != "warmup"]:
        L.append(f"| **{ph}** | " + " | ".join("" for _ in impls) + " |")
        L.append("| fps | " + " | ".join(f"{pm(runs[i], ph, 'fps'):.1f}" for i in impls) + " |")
        L.append("| CPU % | " + " | ".join(f"{pm(runs[i], ph, 'cpu_pct'):.1f}" for i in impls) + " |")
        L.append("| Top threads CPU % | " + " | ".join(threads(runs[i], ph) for i in impls) + " |")
        L.append("| Memory MB | " + " | ".join((f"{pm(runs[i], ph, 'footprint_mb'):.1f}" if memory_supported else "n/a") for i in impls) + " |")
        L.append("| Draw p50 / p99 ms | " + " | ".join(f"{pm(runs[i], ph, 'draw_ms', 'p50'):.2f} / {pm(runs[i], ph, 'draw_ms', 'p99'):.2f}" for i in impls) + " |")
        L.append("| Frame work p50 / p99 ms | " + " | ".join(f"{pm(runs[i], ph, 'dirty_to_draw_ms', 'p50'):.2f} / {pm(runs[i], ph, 'dirty_to_draw_ms', 'p99'):.2f}" for i in impls) + " |")
        lbl = "Trigger to frame" if sc == "interaction" else "Push to frame"
        L.append(f"| {lbl} p50 / p99 ms | " + " | ".join(latency(runs[i], ph) for i in impls) + " |")
        L.append(f"| {lbl} (verified) p50 / p99 ms | " + " | ".join(latency(runs[i], ph, True) for i in impls) + " |")
        L.append("| Late frames | " + " | ".join(f"{pm(runs[i], ph, 'late_frames'):.0f}" for i in impls) + " |")
        tasks = sorted({t for i in impls for r in runs[i] if (p := phase(r, ph)) for t, _ in p.get("compute_ms", [])})
        for t in tasks:
            L.append(f"| Compute {t} p50 / p95 ms | " + " | ".join(f"{cm(runs[i], ph, t, 'p50'):.2f} / {cm(runs[i], ph, t, 'p95'):.2f}" for i in impls) + " |")
    L.append("")
md = "\n".join(L)
open(f"{out}/REPORT.md", "w").write(md)
print(md)
