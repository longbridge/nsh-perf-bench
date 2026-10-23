# NSH performance benchmark results (20261022-211106)

With several runs each cell is the median of the per-run statistics; with one run it is that run's value. CPU: one core = 100%.

Latency is from the update (or trigger) to the end of the first frame drawn after it, not physical scan-out. "verified" only counts a frame once the main thread has applied the commit carrying the update (NSH hosts); late frames only cover the work of the frames that were collected.

Implementations: `nsh-*` run on the platform's native NSH backend (Linux: GTK 4, macOS: AppKit, Windows: Win32 + Direct2D); `nsh-*-gpui` on nsh-platform-gpui; `rust-gpui` is plain gpui-kit on gpui-fast 0.1.10. In the quotes scenario fps reflects how often each implementation redraws, not its throughput: compare CPU.

Windows: CPU is QueryProcessCycleTime cycles over calibrated cycles per second; memory is the private working set (Task Manager's Memory column). An `nsh-*` frame is one WM_PAINT (drawing plus Present(1), which can wait for the compositor), so draw and frame work include that wait. Rows marked PresentMon come from PresentMon's record of the same process (scripts/presentmon_join.py): frames that reached the screen, the same measure for every implementation.

## Scenario: animation (3 runs each)

| Phase / metric | nsh-hybrid | nsh-rust | nsh-tsx | rust-gpui |
|---|---|---|---|---|
| Startup to first frame ms | 236 | 209 | 357 | 266 |
| Peak memory MB | 271.5 | 248.4 | 264.7 | 56.0 |
| Startup to first display ms (PresentMon) | 215 | 214 | 212 | 3693 |
| **anim_js_200** |  |  |  |  |
| fps | 2.0 | 2.0 | 2.0 | 59.6 |
| CPU % | 39.6 | 39.4 | 39.4 | 6.5 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 235.4 | 202.5 | 226.6 | 50.0 |
| Draw p50 / p99 ms | 498.02 / 504.92 | 498.12 / 499.05 | 497.88 / 499.48 | 0.42 / 0.89 |
| Frame work p50 / p99 ms | 498.02 / 504.92 | 498.12 / 499.05 | 497.88 / 499.48 | 0.44 / 0.95 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 20 | 20 | 20 | 0 |
| Displayed fps (PresentMon) | 1.9 | 1.9 | 1.9 | 59.5 |
| Display interval p50 / p99 ms (PresentMon) | 500.0 / 3800.0 | 500.0 / 3750.0 | 500.0 / 3833.3 | 16.7 / 16.8 |
| Late displays (PresentMon) | 19 | 19 | 19 | 0 |
| **anim_js_1000** |  |  |  |  |
| fps | 0.6 | 0.8 | 0.7 | 59.9 |
| CPU % | 102.4 | 104.6 | 104.5 | 15.1 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 271.5 | 248.4 | 264.7 | 54.1 |
| Draw p50 / p99 ms | 1956.16 / 2672.11 | 919.78 / 2598.46 | 913.79 / 2690.93 | 1.61 / 2.52 |
| Frame work p50 / p99 ms | 1956.16 / 2672.11 | 919.78 / 2598.46 | 913.79 / 2690.93 | 1.63 / 2.61 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 6 | 8 | 7 | 0 |
| Displayed fps (PresentMon) | 0.8 | 0.9 | 0.9 | 59.6 |
| Display interval p50 / p99 ms (PresentMon) | 900.1 / 2700.0 | 900.0 / 2700.0 | 883.4 / 2700.0 | 16.7 / 16.8 |
| Late displays (PresentMon) | 8 | 9 | 9 | 3 |
| **anim_native_200** |  |  |  |  |
| fps | 6.7 | 6.6 | 6.3 | 59.8 |
| CPU % | 110.9 | 110.8 | 112.4 | 8.2 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 260.0 | 233.1 | 254.4 | 53.8 |
| Draw p50 / p99 ms | 168.25 / 231.16 | 170.64 / 231.24 | 168.81 / 227.55 | 0.53 / 1.11 |
| Frame work p50 / p99 ms | 168.25 / 231.16 | 170.64 / 231.24 | 168.81 / 227.55 | 0.56 / 1.15 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 63 | 60 | 59 | 0 |
| Displayed fps (PresentMon) | 6.7 | 6.6 | 6.3 | 60.0 |
| Display interval p50 / p99 ms (PresentMon) | 166.7 / 233.4 | 166.7 / 233.4 | 166.7 / 233.3 | 16.7 / 16.8 |
| Late displays (PresentMon) | 61 | 59 | 58 | 0 |
| **anim_native_1000** |  |  |  |  |
| fps | 1.4 | 1.7 | 1.5 | 59.9 |
| CPU % | 111.0 | 111.2 | 112.5 | 17.4 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 256.8 | 225.6 | 249.9 | 55.1 |
| Draw p50 / p99 ms | 784.85 / 1086.27 | 738.23 / 1093.15 | 825.33 / 1039.61 | 1.78 / 2.67 |
| Frame work p50 / p99 ms | 784.85 / 1086.27 | 738.23 / 1093.15 | 825.33 / 1039.61 | 1.81 / 2.80 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 14 | 16 | 15 | 0 |
| Displayed fps (PresentMon) | 1.4 | 1.7 | 1.5 | 60.0 |
| Display interval p50 / p99 ms (PresentMon) | 783.3 / 1083.3 | 733.3 / 1099.9 | 816.6 / 1033.4 | 16.7 / 16.8 |
| Late displays (PresentMon) | 14 | 16 | 15 | 0 |
| **cooldown** |  |  |  |  |
| fps | 0.7 | 0.7 | 0.7 | 0.7 |
| CPU % | 29.2 | 24.3 | 23.4 | 2.1 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 263.2 | 233.9 | 256.8 | 56.0 |
| Draw p50 / p99 ms | 884.81 / 884.81 | 897.62 / 897.62 | 831.31 / 843.14 | 4.35 / 4.36 |
| Frame work p50 / p99 ms | 884.81 / 884.81 | 897.62 / 897.62 | 831.31 / 843.14 | 4.35 / 4.37 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 1 | 1 | 1 | 0 |
| Displayed fps (PresentMon) | 0.3 | 0.3 | 0.3 | 0.7 |
| Display interval p50 / p99 ms (PresentMon) | 900.1 / 900.1 | 900.0 / 900.0 | 833.2 / 850.0 | 16.7 / 16.7 |
| Late displays (PresentMon) | 1 | 1 | 1 | 0 |

## Scenario: compute (3 runs each)

| Phase / metric | nsh-hybrid | nsh-rust | nsh-tsx | rust-gpui |
|---|---|---|---|---|
| Startup to first frame ms | 234 | 204 | 355 | 260 |
| Peak memory MB | 64.5 | 59.2 | 106.6 | 59.4 |
| Startup to first display ms (PresentMon) | 213 | 208 | 205 | 18912 |
| **compute_100k** |  |  |  |  |
| fps | 0.1 | 0.1 | 0.1 | 0.0 |
| CPU % | 9.0 | 7.9 | 21.0 | 8.5 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 62.4 | 48.8 | 105.8 | 51.6 |
| Draw p50 / p99 ms | 3.19 / 3.19 | 3.81 / 3.81 | 3.69 / 3.69 | 0.00 / 0.00 |
| Frame work p50 / p99 ms | 3.19 / 3.19 | 3.81 / 3.81 | 3.69 / 3.69 | 0.00 / 0.00 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| Displayed fps (PresentMon) | 0.1 | 0.1 | 0.1 | 0.0 |
| Display interval p50 / p99 ms (PresentMon) | 3300.0 / 3300.0 | 3266.7 / 3266.7 | 3666.6 / 3666.6 | 0.0 / 0.0 |
| Late displays (PresentMon) | 1 | 1 | 1 | 0 |
| Compute agg_100k p50 / p95 ms | 0.92 / 1.15 | 0.83 / 1.19 | 16.19 / 16.62 | 0.66 / 0.80 |
| Compute ema_100k p50 / p95 ms | 0.34 / 0.56 | 0.35 / 0.86 | 13.67 / 14.21 | 0.33 / 0.42 |
| Compute json_5mb p50 / p95 ms | 98.30 / 99.90 | 84.17 / 84.44 | 88.03 / 91.66 | 83.69 / 85.23 |
| Compute sort_100k p50 / p95 ms | 5.13 / 5.36 | 4.85 / 5.31 | 108.62 / 111.25 | 5.10 / 5.29 |
| **compute_under_load** |  |  |  |  |
| fps | 60.1 | 60.0 | 60.0 | 59.4 |
| CPU % | 52.3 | 44.0 | 56.1 | 22.0 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 64.5 | 59.2 | 106.6 | 59.4 |
| Draw p50 / p99 ms | 16.14 / 16.78 | 16.14 / 16.74 | 16.11 / 16.70 | 2.63 / 3.57 |
| Frame work p50 / p99 ms | 16.14 / 16.78 | 16.14 / 16.74 | 16.11 / 16.70 | 2.74 / 3.68 |
| Push to frame p50 / p99 ms | 26.2 / 33.2 | 24.2 / 31.1 | 24.3 / 36.3 | 10.9 / 19.8 |
| Push to frame (verified) p50 / p99 ms | 26.2 / 33.2 | 24.2 / 31.1 | 24.3 / 36.3 | 10.9 / 19.8 |
| Late frames | 23 | 21 | 8 | 0 |
| Displayed fps (PresentMon) | 59.9 | 59.8 | 59.8 | 56.7 |
| Display interval p50 / p99 ms (PresentMon) | 16.7 / 16.8 | 16.7 / 16.8 | 16.7 / 16.8 | 16.7 / 33.3 |
| Late displays (PresentMon) | 1 | 1 | 1 | 31 |
| Compute agg_100k p50 / p95 ms | 0.63 / 0.93 | 0.61 / 0.80 | 16.05 / 16.95 | 0.69 / 1.02 |
| Compute json_5mb p50 / p95 ms | 0.00 / 0.00 | 0.00 / 0.00 | 0.00 / 0.00 | 83.93 / 83.93 |
| **cooldown** |  |  |  |  |
| fps | 2.0 | 2.7 | 2.0 | 3.7 |
| CPU % | 1.8 | 1.6 | 2.9 | 2.5 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 64.3 | 59.1 | 106.6 | 58.7 |
| Draw p50 / p99 ms | 2.90 / 3.41 | 3.33 / 16.47 | 3.39 / 3.75 | 2.48 / 2.76 |
| Frame work p50 / p99 ms | 2.90 / 3.41 | 3.33 / 16.47 | 3.39 / 3.75 | 2.58 / 2.93 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| Displayed fps (PresentMon) | 2.3 | 3.0 | 2.3 | 3.7 |
| Display interval p50 / p99 ms (PresentMon) | 100.0 / 116.7 | 66.8 / 116.7 | 100.0 / 116.7 | 66.7 / 66.8 |
| Late displays (PresentMon) | 5 | 5 | 5 | 9 |
| Compute agg_100k p50 / p95 ms | 0.00 / 0.00 | 1.08 / 1.08 | 15.98 / 15.98 | 0.00 / 0.00 |

## Scenario: interaction (3 runs each)

| Phase / metric | nsh-hybrid | nsh-rust | nsh-tsx | rust-gpui |
|---|---|---|---|---|
| Startup to first frame ms | 232 | 207 | 356 | 261 |
| Peak memory MB | 68.8 | 41.4 | 62.1 | 61.7 |
| Startup to first display ms (PresentMon) | 210 | 211 | 206 | 3651 |
| **select_row** |  |  |  |  |
| fps | 4.9 | 4.9 | 4.9 | 4.9 |
| CPU % | 3.9 | 3.7 | 4.1 | 3.0 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 60.1 | 31.6 | 53.0 | 53.0 |
| Draw p50 / p99 ms | 2.70 / 3.86 | 2.65 / 3.48 | 2.66 / 5.10 | 2.36 / 4.00 |
| Frame work p50 / p99 ms | 2.70 / 3.86 | 2.65 / 3.48 | 2.66 / 5.10 | 2.48 / 4.14 |
| Trigger to frame p50 / p99 ms | 5.8 / 7.3 | 6.0 / 9.0 | 5.8 / 8.9 | 10.5 / 19.1 |
| Trigger to frame (verified) p50 / p99 ms | 5.8 / 7.3 | 6.0 / 9.0 | 5.8 / 8.9 | 10.5 / 19.1 |
| Late frames | 0 | 0 | 0 | 0 |
| Displayed fps (PresentMon) | 4.9 | 4.8 | 4.9 | 4.9 |
| Display interval p50 / p99 ms (PresentMon) | 200.0 / 3300.0 | 200.0 / 3266.6 | 200.0 / 3299.9 | 200.1 / 216.8 |
| Late displays (PresentMon) | 49 | 48 | 49 | 48 |
| **switch_tab** |  |  |  |  |
| fps | 2.5 | 2.5 | 2.5 | 2.5 |
| CPU % | 7.5 | 5.9 | 7.7 | 2.7 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 64.7 | 30.7 | 57.8 | 58.0 |
| Draw p50 / p99 ms | 2.24 / 3.13 | 2.21 / 2.65 | 2.22 / 2.76 | 6.25 / 11.34 |
| Frame work p50 / p99 ms | 2.24 / 3.13 | 2.21 / 2.65 | 2.22 / 2.76 | 6.29 / 11.39 |
| Trigger to frame p50 / p99 ms | 38.7 / 41.9 | 29.5 / 31.5 | 39.0 / 41.2 | 13.4 / 22.9 |
| Trigger to frame (verified) p50 / p99 ms | 38.7 / 41.9 | 29.5 / 31.5 | 39.0 / 41.2 | 13.4 / 22.9 |
| Late frames | 0 | 0 | 0 | 0 |
| Displayed fps (PresentMon) | 2.5 | 2.5 | 2.5 | 2.5 |
| Display interval p50 / p99 ms (PresentMon) | 383.4 / 450.0 | 383.4 / 450.0 | 383.4 / 450.1 | 416.7 / 416.7 |
| Late displays (PresentMon) | 25 | 25 | 25 | 25 |
| **filter_50k** |  |  |  |  |
| fps | 4.0 | 4.1 | 4.5 | 4.0 |
| CPU % | 7.7 | 6.9 | 11.5 | 2.9 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 57.8 | 31.3 | 51.0 | 58.6 |
| Draw p50 / p99 ms | 2.04 / 3.98 | 2.07 / 3.03 | 2.05 / 4.56 | 1.54 / 4.13 |
| Frame work p50 / p99 ms | 2.04 / 3.98 | 2.07 / 3.03 | 2.05 / 4.56 | 1.65 / 4.18 |
| Trigger to frame p50 / p99 ms | 17.1 / 26.6 | 15.4 / 21.4 | 25.9 / 34.2 | 10.7 / 18.1 |
| Trigger to frame (verified) p50 / p99 ms | 17.1 / 26.6 | 15.4 / 21.4 | 25.9 / 34.2 | 10.7 / 18.1 |
| Late frames | 0 | 0 | 0 | 0 |
| Displayed fps (PresentMon) | 4.0 | 4.1 | 4.5 | 4.0 |
| Display interval p50 / p99 ms (PresentMon) | 250.0 / 366.7 | 250.0 / 350.0 | 250.0 / 383.3 | 250.0 / 266.8 |
| Late displays (PresentMon) | 40 | 40 | 39 | 40 |
| **select_under_load** |  |  |  |  |
| fps | 59.9 | 59.9 | 60.1 | 59.7 |
| CPU % | 34.6 | 30.5 | 34.2 | 18.6 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 67.1 | 39.5 | 60.5 | 58.6 |
| Draw p50 / p99 ms | 16.43 / 16.76 | 16.42 / 16.71 | 16.37 / 16.74 | 2.19 / 2.96 |
| Frame work p50 / p99 ms | 16.43 / 16.76 | 16.42 / 16.71 | 16.37 / 16.74 | 2.30 / 3.09 |
| Trigger to frame p50 / p99 ms | 24.4 / 33.3 | 24.6 / 31.5 | 24.6 / 33.1 | 11.2 / 18.7 |
| Trigger to frame (verified) p50 / p99 ms | 24.4 / 33.3 | 24.6 / 31.5 | 24.6 / 33.1 | 11.2 / 18.7 |
| Late frames | 27 | 22 | 29 | 0 |
| Displayed fps (PresentMon) | 59.7 | 59.7 | 59.9 | 58.6 |
| Display interval p50 / p99 ms (PresentMon) | 16.7 / 16.8 | 16.7 / 16.8 | 16.7 / 16.8 | 16.7 / 33.3 |
| Late displays (PresentMon) | 1 | 1 | 2 | 13 |
| **cooldown** |  |  |  |  |
| fps | 2.7 | 2.7 | 2.7 | 4.7 |
| CPU % | 2.4 | 1.9 | 2.5 | 2.7 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 68.8 | 41.4 | 62.1 | 61.7 |
| Draw p50 / p99 ms | 3.22 / 16.27 | 3.20 / 16.49 | 2.89 / 16.52 | 2.51 / 4.01 |
| Frame work p50 / p99 ms | 3.22 / 16.27 | 3.20 / 16.49 | 2.89 / 16.52 | 2.64 / 4.15 |
| Trigger to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Trigger to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| Displayed fps (PresentMon) | 3.0 | 3.0 | 3.0 | 3.7 |
| Display interval p50 / p99 ms (PresentMon) | 66.6 / 116.7 | 66.7 / 116.6 | 33.4 / 116.7 | 66.7 / 66.7 |
| Late displays (PresentMon) | 5 | 5 | 5 | 10 |

## Scenario: quotes (3 runs each)

| Phase / metric | nsh-hybrid | nsh-rust | nsh-tsx | rust-gpui |
|---|---|---|---|---|
| Startup to first frame ms | 238 | 204 | 361 | 265 |
| Peak memory MB | 77.3 | 50.2 | 69.9 | 72.7 |
| Startup to first display ms (PresentMon) | 217 | 209 | 211 | 13747 |
| **idle** |  |  |  |  |
| fps | 0.1 | 0.1 | 0.1 | 0.0 |
| CPU % | 0.6 | 0.5 | 0.6 | 1.3 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 57.8 | 29.1 | 50.9 | 48.9 |
| Draw p50 / p99 ms | 3.14 / 3.14 | 3.18 / 3.18 | 3.43 / 3.43 | 0.00 / 0.00 |
| Frame work p50 / p99 ms | 3.14 / 3.14 | 3.18 / 3.18 | 3.43 / 3.43 | 0.00 / 0.00 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| Displayed fps (PresentMon) | 0.1 | 0.1 | 0.1 | 0.0 |
| Display interval p50 / p99 ms (PresentMon) | 3250.0 / 3250.0 | 3149.9 / 3149.9 | 3333.3 / 3333.3 | 0.0 / 0.0 |
| Late displays (PresentMon) | 1 | 1 | 1 | 0 |
| **r200** |  |  |  |  |
| fps | 60.0 | 60.0 | 60.1 | 59.8 |
| CPU % | 45.5 | 39.5 | 45.7 | 21.1 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 63.0 | 39.8 | 56.5 | 58.7 |
| Draw p50 / p99 ms | 16.20 / 16.75 | 16.19 / 16.75 | 16.17 / 16.68 | 2.62 / 3.33 |
| Frame work p50 / p99 ms | 16.20 / 16.75 | 16.19 / 16.75 | 16.17 / 16.68 | 2.73 / 3.51 |
| Push to frame p50 / p99 ms | 26.1 / 33.1 | 24.0 / 30.8 | 25.9 / 32.9 | 10.1 / 17.7 |
| Push to frame (verified) p50 / p99 ms | 26.1 / 33.1 | 24.0 / 30.8 | 25.9 / 32.9 | 10.1 / 17.7 |
| Late frames | 22 | 25 | 9 | 0 |
| Displayed fps (PresentMon) | 59.8 | 59.8 | 59.9 | 55.1 |
| Display interval p50 / p99 ms (PresentMon) | 16.7 / 16.8 | 16.7 / 16.8 | 16.7 / 16.8 | 16.7 / 33.4 |
| Late displays (PresentMon) | 1 | 1 | 1 | 47 |
| **r1000** |  |  |  |  |
| fps | 60.0 | 60.1 | 60.1 | 59.8 |
| CPU % | 52.3 | 43.0 | 52.8 | 24.4 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 63.9 | 40.1 | 57.1 | 60.5 |
| Draw p50 / p99 ms | 16.14 / 16.66 | 16.16 / 16.64 | 16.10 / 16.75 | 2.91 / 4.24 |
| Frame work p50 / p99 ms | 16.14 / 16.66 | 16.16 / 16.64 | 16.10 / 16.75 | 3.01 / 4.40 |
| Push to frame p50 / p99 ms | 26.6 / 33.8 | 25.6 / 32.7 | 25.8 / 32.9 | 11.0 / 19.4 |
| Push to frame (verified) p50 / p99 ms | 26.6 / 33.8 | 25.6 / 32.7 | 25.8 / 32.9 | 11.0 / 19.4 |
| Late frames | 5 | 5 | 22 | 0 |
| Displayed fps (PresentMon) | 59.8 | 59.9 | 59.9 | 58.0 |
| Display interval p50 / p99 ms (PresentMon) | 16.7 / 16.8 | 16.7 / 16.8 | 16.7 / 16.8 | 16.7 / 33.4 |
| Late displays (PresentMon) | 1 | 1 | 1 | 18 |
| **r5000** |  |  |  |  |
| fps | 60.0 | 60.0 | 60.0 | 59.8 |
| CPU % | 73.9 | 57.2 | 70.9 | 28.0 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 64.6 | 39.9 | 57.8 | 59.7 |
| Draw p50 / p99 ms | 15.93 / 16.66 | 15.90 / 16.41 | 15.89 / 16.59 | 3.33 / 4.33 |
| Frame work p50 / p99 ms | 15.93 / 16.66 | 15.90 / 16.41 | 15.89 / 16.59 | 3.46 / 4.60 |
| Push to frame p50 / p99 ms | 25.8 / 32.9 | 24.9 / 31.9 | 25.4 / 32.3 | 11.6 / 19.8 |
| Push to frame (verified) p50 / p99 ms | 25.8 / 32.9 | 24.9 / 31.9 | 25.4 / 32.3 | 11.6 / 19.8 |
| Late frames | 5 | 1 | 1 | 0 |
| Displayed fps (PresentMon) | 60.0 | 60.0 | 60.0 | 59.9 |
| Display interval p50 / p99 ms (PresentMon) | 16.7 / 16.8 | 16.7 / 16.8 | 16.7 / 16.8 | 16.7 / 16.8 |
| Late displays (PresentMon) | 0 | 0 | 0 | 2 |
| **r5000_scroll** |  |  |  |  |
| fps | 60.0 | 60.0 | 60.0 | 59.9 |
| CPU % | 95.3 | 74.5 | 93.2 | 30.3 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 74.5 | 48.5 | 67.3 | 72.7 |
| Draw p50 / p99 ms | 15.69 / 16.44 | 15.69 / 16.49 | 15.66 / 16.49 | 3.66 / 5.07 |
| Frame work p50 / p99 ms | 15.69 / 16.44 | 15.69 / 16.49 | 15.66 / 16.49 | 3.81 / 5.41 |
| Push to frame p50 / p99 ms | 25.0 / 33.8 | 24.9 / 32.9 | 26.2 / 33.2 | 12.2 / 19.7 |
| Push to frame (verified) p50 / p99 ms | 25.0 / 33.8 | 24.9 / 32.9 | 26.2 / 33.2 | 12.2 / 19.7 |
| Late frames | 1 | 0 | 1 | 0 |
| Displayed fps (PresentMon) | 60.0 | 60.0 | 60.0 | 60.0 |
| Display interval p50 / p99 ms (PresentMon) | 16.7 / 16.8 | 16.7 / 16.8 | 16.7 / 16.8 | 16.7 / 16.8 |
| Late displays (PresentMon) | 0 | 0 | 0 | 0 |
| **r20000** |  |  |  |  |
| fps | 32.1 | 60.0 | 27.4 | 59.9 |
| CPU % | 131.7 | 141.3 | 120.1 | 33.0 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 77.3 | 50.2 | 69.9 | 72.1 |
| Draw p50 / p99 ms | 4.03 / 14.92 | 14.78 / 15.83 | 4.04 / 14.99 | 3.95 / 5.69 |
| Frame work p50 / p99 ms | 4.03 / 14.92 | 14.78 / 15.83 | 4.04 / 14.99 | 4.06 / 5.85 |
| Push to frame p50 / p99 ms | 54.3 / 79.7 | 29.0 / 40.7 | 62.4 / 91.6 | 13.2 / 21.1 |
| Push to frame (verified) p50 / p99 ms | 54.3 / 79.7 | 29.0 / 40.7 | 62.4 / 91.6 | 13.2 / 21.1 |
| Late frames | 0 | 1 | 0 | 0 |
| Displayed fps (PresentMon) | 32.3 | 60.0 | 27.6 | 60.0 |
| Display interval p50 / p99 ms (PresentMon) | 33.3 / 50.0 | 16.7 / 16.8 | 33.3 / 50.1 | 16.7 / 16.8 |
| Late displays (PresentMon) | 252 | 0 | 242 | 0 |
| **cooldown** |  |  |  |  |
| fps | 0.8 | 1.0 | 0.8 | 2.6 |
| CPU % | 1.7 | 1.0 | 1.9 | 2.7 |
| Top threads CPU % | - | - | - | - |
| Memory MB | 76.6 | 50.0 | 69.0 | 71.7 |
| Draw p50 / p99 ms | 4.04 / 6.09 | 4.27 / 15.32 | 4.34 / 4.84 | 2.89 / 4.38 |
| Frame work p50 / p99 ms | 4.04 / 6.09 | 4.27 / 15.32 | 4.34 / 4.84 | 2.97 / 4.44 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| Displayed fps (PresentMon) | 0.8 | 1.2 | 0.8 | 2.4 |
| Display interval p50 / p99 ms (PresentMon) | 50.0 / 450.0 | 16.7 / 483.3 | 33.4 / 333.3 | 66.6 / 66.8 |
| Late displays (PresentMon) | 3 | 2 | 4 | 10 |
