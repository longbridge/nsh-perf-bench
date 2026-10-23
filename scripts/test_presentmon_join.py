#!/usr/bin/env python3
"""Tests of presentmon_join.py on a hand-made PresentMon CSV and result JSON (any OS).
Usage: python3 scripts/test_presentmon_join.py"""
import csv, json, os, sys, tempfile, unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import presentmon_join as pj

F = 10_000_000  # QPC frequency (10 MHz on Windows 10 and later)
ORIGIN = 1_000_000_000
HEADER = ["Application", "ProcessID", "SwapChainAddress", "PresentRuntime", "SyncInterval", "PresentFlags", "AllowsTearing",
          "PresentMode", "TimeInQPC", "MsBetweenSimulationStart", "MsBetweenPresents", "MsBetweenDisplayChange", "MsInPresentAPI",
          "MsRenderPresentLatency", "MsUntilDisplayed", "CPUStartQPC", "MsBetweenAppStart", "MsCPUBusy", "MsCPUWait"]


def q(ms):
    return ORIGIN + int(ms * F / 1000)


def report(pid=4242):
    # phases: warmup 0-1000 ms, idle 1000-2000 ms, anim 2000-3000 ms
    ph = lambda n, a, b: {"name": n, "secs": (b - a) / 1000, "fps": 99.0, "late_frames": 7, "qpc_start": q(a), "qpc_end": q(b)}
    return {"implementation": "nsh-tsx", "scenario": "animation", "startup_first_frame_ms": 555.0, "peak_footprint_mb": 1.0,
            "refresh_hz": 60.0, "pid": pid, "qpc": {"freq": F, "origin": ORIGIN},
            "phases": [ph("warmup", 0, 1000), ph("idle", 1000, 2000), ph("anim", 2000, 3000)]}


def row(pid, t_ms, until_ms, app="bench-nsh.exe"):
    r = dict.fromkeys(HEADER, "0")
    r.update({"Application": app, "ProcessID": str(pid), "PresentMode": "Hardware: Independent Flip",
              "TimeInQPC": str(q(t_ms)), "CPUStartQPC": str(q(t_ms)), "MsUntilDisplayed": "NA" if until_ms is None else f"{until_ms:.4f}"})
    return r


def frames():
    rows = [row(4242, 300, 10.0)]                  # first present at 300 ms, displayed at 310
    rows += [row(4242, 1500, 5.0)]                 # one frame in idle (1195 ms after the previous: late)
    t = 2000.0
    for i in range(60):                            # anim: 60 frames 16.667 ms apart, one 50 ms gap after the 30th
        t += 16.667 if i != 30 else 50.0
        rows.append(row(4242, t - 8, 8.0))         # displayed at t
    rows.append(row(4242, 2990, None))             # dropped: not a frame
    rows.append(row(999, 2500, 1.0, "other.exe"))  # another process
    return rows


class JoinTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.csv = os.path.join(self.dir, "pm.csv")
        self.json = os.path.join(self.dir, "r.json")
        with open(self.csv, "w", newline="", encoding="utf-8") as f:
            w = csv.DictWriter(f, fieldnames=HEADER)
            w.writeheader()
            w.writerows(frames())
        with open(self.json, "w", encoding="utf-8") as f:
            json.dump(report(), f)

    def load(self):
        with open(self.json, encoding="utf-8") as f:
            return json.load(f)

    def test_join_splits_frames_by_qpc(self):
        self.assertEqual(pj.main([self.json, self.csv]), 0)
        r = self.load()
        p = {x["name"]: x["presentmon"] for x in r["phases"]}
        self.assertAlmostEqual(r["presentmon"]["startup_first_present_ms"], 300.0, places=3)
        self.assertAlmostEqual(r["presentmon"]["startup_first_display_ms"], 310.0, places=3)
        self.assertAlmostEqual(p["warmup"]["displayed_fps"], 1.0)
        self.assertAlmostEqual(p["idle"]["displayed_fps"], 1.0)
        # anim: 57 frames displayed within 2000-3000 ms (30 up to 2500 ms, the late one at 2550, 26 more up to 2983)
        self.assertAlmostEqual(p["anim"]["displayed_fps"], 57.0)
        fi = p["anim"]["display_interval_ms"]
        self.assertEqual(fi["n"], 57)
        self.assertAlmostEqual(fi["p50"], 16.667, places=2)
        self.assertAlmostEqual(fi["max"], 2016.667 - 1505.0, places=2)  # across phases: the idle frame to the first anim frame
        self.assertEqual(p["idle"]["late_displays"], 1)
        self.assertEqual(p["anim"]["late_displays"], 2)
        self.assertEqual(p["anim"]["dropped"], 1)
        self.assertEqual(r["presentmon"]["presents"], 63)
        # the host's own metrics stay
        self.assertEqual(r["phases"][2]["fps"], 99.0)
        self.assertEqual(r["phases"][2]["late_frames"], 7)
        self.assertEqual(r["startup_first_frame_ms"], 555.0)

    def test_join_is_idempotent(self):
        pj.main([self.json, self.csv])
        first = self.load()
        pj.main([self.json, self.csv])
        self.assertEqual(self.load(), first)

    def test_refresh_override_changes_late_threshold(self):
        pj.main([self.json, self.csv, "--refresh-hz", "15"])  # 66.7 ms period, 100 ms threshold: 50 ms is not late
        p = {x["name"]: x["presentmon"] for x in self.load()["phases"]}
        self.assertEqual(p["anim"]["late_displays"], 1)

    def test_missing_process(self):
        with open(self.json, "w", encoding="utf-8") as f:
            json.dump(report(pid=1), f)
        self.assertEqual(pj.main([self.json, self.csv]), 3)
        out = self.load()
        self.assertFalse(out["presentmon"]["joined"])
        self.assertNotIn("presentmon", out["phases"][2])

    def test_missing_csv(self):
        self.assertEqual(pj.main([self.json, os.path.join(self.dir, "none.csv")]), 3)
        self.assertEqual(self.load()["presentmon"]["error"], "csv missing")

    def test_v2_metrics_columns(self):
        hdr = ["Application", "ProcessID", "PresentMode", "CPUStartQPC", "FrameTime", "DisplayLatency", "DisplayedTime"]
        with open(self.csv, "w", newline="", encoding="utf-8") as f:
            w = csv.writer(f)
            w.writerow(hdr)
            for i in range(10):
                w.writerow(["bench-nsh.exe", 4242, "Composed: Flip", q(2000 + i * 20), 20, "5.0", "20"])
        pj.main([self.json, self.csv])
        p = {x["name"]: x["presentmon"] for x in self.load()["phases"]}
        self.assertAlmostEqual(p["anim"]["displayed_fps"], 10.0)
        self.assertAlmostEqual(p["anim"]["display_interval_ms"]["p50"], 20.0, places=3)


if __name__ == "__main__":
    unittest.main()
