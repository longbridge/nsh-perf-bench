#!/usr/bin/env python3
"""汇总 results/<时间>/run*/<impl>-<scenario>.json：每个场景、阶段、指标取 N 次中位数，输出 Markdown 对比表。"""
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
    sys.exit("没有原始 JSON，无法生成报告")
if "repetitions" in meta:
    names = {"rust": "rust-gpui", "nsh": "nsh-tsx", "hybrid": "nsh-hybrid", "rustui": "nsh-rust"}
    expected = {str(Path(out, f"run{i}", f"{names[impl]}-{sc}.json"))
                for i in range(1, meta["repetitions"] + 1)
                for impl in meta["implementations"] for sc in meta["scenarios"]}
    if set(files) != expected:
        sys.exit("结果文件与 metadata.json 不一致：缺失或多出运行结果")
for f in files:
    r = json.load(open(f))
    if Path(f).name != f"{r['implementation']}-{r.get('scenario', 'quotes')}.json":
        sys.exit(f"结果身份与文件名不一致：{f}")
    if "repetitions" in meta:
        standard_phases = {
            "quotes": ["warmup", "idle", "r200", "r1000", "r5000", "r5000_scroll", "r20000", "cooldown"],
            "animation": ["warmup", "anim_js_200", "anim_js_1000", "anim_native_200", "anim_native_1000", "cooldown"],
            "interaction": ["warmup", "select_row", "switch_tab", "filter_50k", "select_under_load", "cooldown"],
            "compute": ["warmup", "compute_100k", "compute_under_load", "cooldown"],
        }
        if [p["name"] for p in r["phases"]] != standard_phases[r["scenario"]]:
            sys.exit(f"标准场景阶段不完整：{f}")
    data.setdefault(r.get("scenario", "quotes"), {}).setdefault(r["implementation"], []).append(r)
for sc, runs in data.items():
    reference = next(iter(runs.values()))
    phases = [p["name"] for p in reference[0]["phases"]]
    if not phases or any(len(rs) != len(reference) or
                         any([p["name"] for p in r["phases"]] != phases for r in rs)
                         for rs in runs.values()):
        sys.exit(f"{sc} 的运行次数或阶段不一致，无法公平比较")
med = lambda xs: statistics.median(xs) if xs else 0.0
def pm(reports, ph, key, sub=None):
    v = []
    for r in reports:
        for p in r["phases"]:
            if p["name"] == ph:
                v.append(p[key][sub] if sub else p[key])
    return med(v)
def cm(reports, ph, task, sub):
    v = []
    for r in reports:
        for p in r["phases"]:
            if p["name"] == ph:
                for name, pc in p.get("compute_ms", []):
                    if name == task:
                        v.append(pc[sub])
    return med(v)
def latency(reports, phase):
    if not all(p["tick_to_frame_ms"]["n"] > 0 for r in reports for p in r["phases"] if p["name"] == phase):
        return "无样本"
    return f"{pm(reports, phase, 'tick_to_frame_ms', 'p50'):.1f} / {pm(reports, phase, 'tick_to_frame_ms', 'p99'):.1f}"
L = [f"# NSH 性能基准结果（{os.path.basename(out)}）", "", "多次运行时每格为各次统计值的中位数；一次运行时为该次统计值。CPU 以单核 = 100%。", "",
     "延迟是应用后首个绘制完成的近似值，不是物理上屏时间；超时帧仅统计已采集帧的工作量。", ""]
if not memory_supported:
    L += ["本环境未实现内存采样；原始 JSON 的内存 0 是占位值，报告标为不可用。", ""]
for sc, runs in data.items():
    impls = sorted(runs)
    n = len(runs[impls[0]])
    L += [f"## 场景：{sc}（各 {n} 次）", "", "| 阶段 / 指标 | " + " | ".join(impls) + " |", "|---|" + "---|" * len(impls)]
    L.append("| 启动到首帧 ms | " + " | ".join(f"{med([r['startup_first_frame_ms'] for r in runs[i]]):.0f}" for i in impls) + " |")
    L.append("| 峰值内存 MB | " + " | ".join((f"{med([r['peak_footprint_mb'] for r in runs[i]]):.1f}" if memory_supported else "不可用") for i in impls) + " |")
    for ph in [p["name"] for p in runs[impls[0]][0]["phases"] if p["name"] != "warmup"]:
        L.append(f"| **{ph}** | " + " | ".join("" for _ in impls) + " |")
        L.append("| fps | " + " | ".join(f"{pm(runs[i], ph, 'fps'):.1f}" for i in impls) + " |")
        L.append("| CPU % | " + " | ".join(f"{pm(runs[i], ph, 'cpu_pct'):.1f}" for i in impls) + " |")
        L.append("| 内存 MB | " + " | ".join((f"{pm(runs[i], ph, 'footprint_mb'):.1f}" if memory_supported else "不可用") for i in impls) + " |")
        L.append("| 绘制 p50 / p99 ms | " + " | ".join(f"{pm(runs[i], ph, 'draw_ms', 'p50'):.2f} / {pm(runs[i], ph, 'draw_ms', 'p99'):.2f}" for i in impls) + " |")
        L.append("| 帧工作量 p50 / p99 ms | " + " | ".join(f"{pm(runs[i], ph, 'dirty_to_draw_ms', 'p50'):.2f} / {pm(runs[i], ph, 'dirty_to_draw_ms', 'p99'):.2f}" for i in impls) + " |")
        lbl = "触发→上屏" if sc == "interaction" else "推送→上屏"
        L.append(f"| {lbl} p50 / p99 ms | " + " | ".join(latency(runs[i], ph) for i in impls) + " |")
        L.append("| 超时帧 | " + " | ".join(f"{pm(runs[i], ph, 'late_frames'):.0f}" for i in impls) + " |")
        tasks = sorted({t for i in impls for r in runs[i] for p in r["phases"] if p["name"] == ph for t, _ in p.get("compute_ms", [])})
        for t in tasks:
            L.append(f"| 计算 {t} p50 / p95 ms | " + " | ".join(f"{cm(runs[i], ph, t, 'p50'):.2f} / {cm(runs[i], ph, t, 'p95'):.2f}" for i in impls) + " |")
    L.append("")
md = "\n".join(L)
open(f"{out}/REPORT.md", "w").write(md)
print(md)
