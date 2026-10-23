#!/usr/bin/env python3
"""Add PresentMon frame timing to a Windows result JSON: split the frames into the phases by QPC.

Usage: python scripts/presentmon_join.py <result JSON> <PresentMon CSV> [--refresh-hz N] [-o out.json]
Rewrites the JSON in place unless -o is given; running it again gives the same result.

The host's own metrics (fps, late_frames, draw / frame work, latency) are left as they are; this adds
`presentmon` to every phase and to the report:
  - displayed_fps: frames that reached the screen in the phase / phase seconds. A frame's time is when it
    was displayed (TimeInQPC + MsUntilDisplayed); presents that were never displayed (dropped) do not count;
  - display_interval_ms: interval between a displayed frame and the one before it (which may be in the
    previous phase), counted in the phase of the later frame;
  - late_displays: intervals longer than 1.5 refresh periods (refresh: --refresh-hz, else the JSON's
    refresh_hz, else 60). The first frame after an idle stretch counts too: read idle phases by fps;
  - presents_per_s, dropped: present calls in the phase, and how many of them were not displayed;
  - report-level startup_first_present_ms / startup_first_display_ms, from the process start (qpc.origin).

Input: the JSON a benchmark writes on Windows (bench-core adds pid, qpc.freq / qpc.origin and every
phase's qpc_start / qpc_end); a PresentMon 2.x console CSV recorded with --qpc_time (columns ProcessID,
TimeInQPC, MsUntilDisplayed, PresentMode; the --v2_metrics columns CPUStartQPC + DisplayLatency also work).
"""
import argparse
import bisect
import csv
import json
import os
import sys
from collections import Counter


def pct(xs):
    """Same as bench-core's pct(): sorted, value at round((n - 1) * q)."""
    if not xs:
        return {"n": 0, "p50": 0.0, "p95": 0.0, "p99": 0.0, "max": 0.0}
    v = sorted(xs)
    at = lambda q: v[int(round((len(v) - 1) * q))]
    return {"n": len(v), "p50": at(0.5), "p95": at(0.95), "p99": at(0.99), "max": v[-1]}


def num(s):
    s = (s or "").strip()
    if not s or s.upper() == "NA":
        return None
    try:
        return float(s)
    except ValueError:
        return None


def read_frames(path, freq, pid):
    """[(present_qpc, display_qpc or None, present_mode)] of process `pid`, by present time."""
    with open(path, newline="", encoding="utf-8-sig", errors="replace") as f:
        rows = list(csv.DictReader(f))
    cols = set(rows[0].keys()) if rows else set()
    tcol = next((c for c in ("TimeInQPC", "CPUStartQPC") if c in cols), None)
    dcol = next((c for c in ("MsUntilDisplayed", "DisplayLatency") if c in cols), None)
    if rows and tcol is None:
        sys.exit(f"{path}: no QPC time column (record with PresentMon --qpc_time); columns: {sorted(cols)}")
    out, apps = [], Counter()
    for r in rows:
        apps[f"{r.get('Application', '')}:{r.get('ProcessID', '')}"] += 1
        if str(r.get("ProcessID", "")).strip() != str(pid):
            continue
        t = num(r.get(tcol))
        if t is None:
            continue
        d = num(r.get(dcol)) if dcol else None
        out.append((t, t + d * freq / 1000.0 if d is not None and d > 0 else None, r.get("PresentMode", "")))
    out.sort(key=lambda x: x[0])
    return out, {"display_column": dcol, "rows_total": len(rows), "processes": [f"{k}={n}" for k, n in apps.most_common(8)]}


def join(report, frames, meta, refresh_hz=None, csv_name=""):
    freq, origin = report["qpc"]["freq"], report["qpc"]["origin"]
    hz = refresh_hz or report.get("refresh_hz") or 60
    late_ms = 1.5 * 1000.0 / hz
    has_display = meta["display_column"] is not None
    # Without a display column fall back to present times.
    shown = sorted(d for _, d, _ in frames if d is not None) if has_display else [t for t, _, _ in frames]
    for ph in report["phases"]:
        s, e = ph.get("qpc_start"), ph.get("qpc_end")
        if s is None or e is None:
            continue
        secs = (e - s) / freq
        i0, i1 = bisect.bisect_left(shown, s), bisect.bisect_left(shown, e)
        intervals = [(shown[i] - shown[i - 1]) / freq * 1000.0 for i in range(max(i0, 1), i1)]
        presents = [f for f in frames if s <= f[0] < e]
        ph["presentmon"] = {
            "displayed_fps": (i1 - i0) / secs if secs > 0 else 0.0,
            "display_interval_ms": pct(intervals),
            "late_displays": sum(1 for x in intervals if x > late_ms),
            "presents_per_s": len(presents) / secs if secs > 0 else 0.0,
            "dropped": sum(1 for f in presents if f[1] is None) if has_display else 0,
        }
    report["presentmon"] = {
        "joined": True,
        "csv": csv_name,
        "presents": len(frames),
        "displayed": len(shown),
        "refresh_hz": hz,
        "late_threshold_ms": late_ms,
        "present_modes": dict(Counter(m for _, _, m in frames)),
        "startup_first_present_ms": (frames[0][0] - origin) / freq * 1000.0,
        "startup_first_display_ms": (shown[0] - origin) / freq * 1000.0 if shown else None,
        **meta,
    }
    return report


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("json")
    ap.add_argument("csv")
    ap.add_argument("--refresh-hz", type=float)
    ap.add_argument("-o", "--out")
    a = ap.parse_args(argv)
    with open(a.json, encoding="utf-8") as f:
        report = json.load(f)
    if not (report.get("qpc") or {}).get("freq") or "pid" not in report:
        sys.exit(f"{a.json}: no pid / qpc (not a result recorded on Windows?)")
    for ph in report["phases"]:
        ph.pop("presentmon", None)
    frames = None
    if not os.path.exists(a.csv):
        print(f"[join] WARN no PresentMon CSV: {a.csv}", file=sys.stderr)
        report["presentmon"] = {"joined": False, "csv": os.path.basename(a.csv), "error": "csv missing"}
    else:
        frames, meta = read_frames(a.csv, report["qpc"]["freq"], report["pid"])
        if frames:
            join(report, frames, meta, a.refresh_hz, os.path.basename(a.csv))
        else:
            print(f"[join] WARN no frames of pid {report['pid']} in the CSV: {meta['processes']}", file=sys.stderr)
            report["presentmon"] = {"joined": False, "csv": os.path.basename(a.csv), "error": "no frames for process", **meta}
    with open(a.out or a.json, "w", encoding="utf-8") as f:
        json.dump(report, f, ensure_ascii=False, indent=2)
    if not frames:
        return 3
    print(f"[join] {os.path.basename(a.json)}: {len(frames)} presents; "
          + " ".join(f"{p['name']}={p['presentmon']['displayed_fps']:.1f}fps/late{p['presentmon']['late_displays']}"
                     for p in report["phases"] if "presentmon" in p))
    return 0


if __name__ == "__main__":
    sys.exit(main())
