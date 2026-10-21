# NSH performance benchmark results (20261021-212102)

With several runs each cell is the median of the per-run statistics; with one run it is that run's value. CPU: one core = 100%.

Latency is from the update (or trigger) to the end of the first frame drawn after it, not physical scan-out. "verified" only counts a frame once the main thread has applied the commit carrying the update (NSH hosts); late frames only cover the work of the frames that were collected.

Implementations: `nsh-*` run on the platform's native NSH backend (Linux: GTK 4, macOS: AppKit); `nsh-*-gpui` on nsh-platform-gpui; `rust-gpui` is plain gpui-kit on gpui-fast 0.1.10. In the quotes scenario fps reflects how often each implementation redraws, not its throughput: compare CPU.

Memory sampling is not implemented here; the 0 in the raw JSON is a placeholder and is shown as n/a.

## Scenario: animation (1 run each)

| Phase / metric | nsh-hybrid | nsh-rust | nsh-tsx | rust-gpui |
|---|---|---|---|---|
| Startup to first frame ms | 205 | 158 | 191 | 257 |
| Peak memory MB | n/a | n/a | n/a | n/a |
| **anim_js_200** |  |  |  |  |
| fps | 142.0 | 142.7 | 142.1 | 142.9 |
| CPU % | 11.9 | 9.5 | 12.4 | 5.7 |
| Top threads CPU % | nsh-engine 6.6, bench-nsh 5.1 | bench-nsh 4.9, nsh-engine 4.3 | nsh-engine 6.6, bench-nsh 5.8 | bench-rust 5.3 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.27 / 0.68 | 0.28 / 0.58 | 0.28 / 0.70 | 0.22 / 0.42 |
| Frame work p50 / p99 ms | 0.27 / 0.68 | 0.28 / 0.58 | 0.28 / 0.70 | 0.22 / 1.25 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **anim_js_1000** |  |  |  |  |
| fps | 143.8 | 144.4 | 144.6 | 144.3 |
| CPU % | 49.7 | 39.9 | 51.5 | 14.8 |
| Top threads CPU % | nsh-engine 29.6, bench-nsh 20.0 | bench-nsh 20.3, nsh-engine 19.5 | nsh-engine 30.6, bench-nsh 20.9 | bench-rust 14.4, Timer 0.2 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 1.16 / 2.19 | 1.19 / 2.20 | 1.19 / 2.47 | 0.79 / 1.51 |
| Frame work p50 / p99 ms | 1.16 / 2.19 | 1.19 / 2.20 | 1.19 / 2.47 | 0.79 / 1.79 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **anim_native_200** |  |  |  |  |
| fps | 144.2 | 143.5 | 143.4 | 144.1 |
| CPU % | 6.3 | 5.8 | 6.2 | 5.6 |
| Top threads CPU % | bench-nsh 5.9, nsh-engine 0.4 | bench-nsh 5.5, nsh-engine 0.3 | bench-nsh 5.7, nsh-engine 0.4 | bench-rust 5.3 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.34 / 1.00 | 0.34 / 0.70 | 0.34 / 0.92 | 0.22 / 0.41 |
| Frame work p50 / p99 ms | 0.34 / 1.01 | 0.34 / 0.70 | 0.34 / 0.92 | 0.22 / 1.21 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **anim_native_1000** |  |  |  |  |
| fps | 144.2 | 144.3 | 144.3 | 143.7 |
| CPU % | 21.1 | 21.9 | 21.9 | 14.9 |
| Top threads CPU % | bench-nsh 20.7, nsh-engine 0.3, [vkps] Update 0.1 | bench-nsh 21.6, nsh-engine 0.3 | bench-nsh 21.6, nsh-engine 0.3, [vkps] Update 0.1 | bench-rust 14.5, [vkps] Update 0.1, Timer 0.1 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 1.43 / 2.17 | 1.49 / 2.05 | 1.45 / 2.79 | 0.80 / 1.54 |
| Frame work p50 / p99 ms | 1.44 / 2.17 | 1.49 / 2.05 | 1.45 / 2.79 | 0.80 / 1.81 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **cooldown** |  |  |  |  |
| fps | 7.0 | 5.0 | 6.3 | 2.0 |
| CPU % | 2.1 | 1.7 | 2.2 | 0.6 |
| Top threads CPU % | bench-nsh 1.3, nsh-engine 0.7 | bench-nsh 1.0, nsh-engine 0.3 | bench-nsh 1.0, nsh-engine 0.7 | Timer 0.3 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 1.47 / 4.22 | 1.57 / 4.36 | 1.77 / 4.31 | 0.82 / 2.11 |
| Frame work p50 / p99 ms | 1.47 / 4.22 | 1.57 / 4.36 | 1.77 / 4.31 | 0.82 / 2.11 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |

## Scenario: compute (1 run each)

| Phase / metric | nsh-hybrid | nsh-rust | nsh-tsx | rust-gpui |
|---|---|---|---|---|
| Startup to first frame ms | 174 | 182 | 185 | 281 |
| Peak memory MB | n/a | n/a | n/a | n/a |
| **compute_100k** |  |  |  |  |
| fps | 0.1 | 0.1 | 0.1 | 0.0 |
| CPU % | 4.0 | 3.9 | 11.2 | 4.5 |
| Top threads CPU % | nsh-engine 3.9, bench-nsh 0.1 | nsh-engine 3.8, bench-nsh 0.1 | nsh-engine 11.1, bench-nsh 0.1 | bench-rust 4.2, Timer 0.1 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 1.11 / 1.11 | 1.16 / 1.16 | 1.02 / 1.02 | 0.00 / 0.00 |
| Frame work p50 / p99 ms | 1.11 / 1.11 | 1.16 / 1.16 | 1.02 / 1.02 | 0.00 / 0.00 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| Compute agg_100k p50 / p95 ms | 0.43 / 0.84 | 0.41 / 0.45 | 9.36 / 9.54 | 0.43 / 0.85 |
| Compute ema_100k p50 / p95 ms | 0.21 / 0.25 | 0.20 / 0.24 | 7.24 / 7.33 | 0.24 / 0.31 |
| Compute json_5mb p50 / p95 ms | 41.84 / 43.82 | 37.93 / 47.68 | 40.12 / 43.43 | 45.53 / 46.81 |
| Compute sort_100k p50 / p95 ms | 3.30 / 3.45 | 3.12 / 3.19 | 59.19 / 66.99 | 3.61 / 4.54 |
| **compute_under_load** |  |  |  |  |
| fps | 108.1 | 109.1 | 107.1 | 118.1 |
| CPU % | 19.4 | 17.5 | 22.3 | 19.8 |
| Top threads CPU % | nsh-engine 9.8, bench-nsh 9.3 | bench-nsh 8.7, nsh-engine 8.6 | nsh-engine 12.9, bench-nsh 9.4 | bench-rust 19.4, Timer 0.1 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.61 / 1.66 | 0.59 / 1.83 | 0.63 / 1.83 | 1.32 / 2.42 |
| Frame work p50 / p99 ms | 0.61 / 1.66 | 0.59 / 1.83 | 0.63 / 1.83 | 1.33 / 2.49 |
| Push to frame p50 / p99 ms | 1.8 / 9.8 | 1.7 / 4.9 | 1.9 / 11.1 | 1.4 / 11.1 |
| Push to frame (verified) p50 / p99 ms | 1.8 / 9.8 | 1.7 / 4.9 | 1.9 / 11.1 | 1.4 / 11.1 |
| Late frames | 0 | 0 | 0 | 0 |
| Compute agg_100k p50 / p95 ms | 0.37 / 0.46 | 0.36 / 0.55 | 9.22 / 9.52 | 0.37 / 0.46 |
| **cooldown** |  |  |  |  |
| fps | 3.0 | 3.0 | 3.0 | 5.7 |
| CPU % | 0.7 | 0.6 | 1.8 | 1.3 |
| Top threads CPU % | nsh-engine 0.3 | nsh-engine 0.3, bench-nsh 0.3 | nsh-engine 1.3, bench-nsh 0.3 | bench-rust 1.0, Timer 0.3 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.63 / 1.07 | 0.73 / 1.17 | 0.84 / 1.38 | 1.30 / 1.93 |
| Frame work p50 / p99 ms | 0.63 / 1.07 | 0.73 / 1.17 | 0.84 / 1.38 | 1.31 / 1.93 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| Compute agg_100k p50 / p95 ms | 0.00 / 0.00 | 0.00 / 0.00 | 8.80 / 8.80 | 0.00 / 0.00 |

## Scenario: interaction (1 run each)

| Phase / metric | nsh-hybrid | nsh-rust | nsh-tsx | rust-gpui |
|---|---|---|---|---|
| Startup to first frame ms | 192 | 215 | 158 | 231 |
| Peak memory MB | n/a | n/a | n/a | n/a |
| **select_row** |  |  |  |  |
| fps | 9.4 | 9.3 | 5.1 | 5.0 |
| CPU % | 2.0 | 2.0 | 1.7 | 1.5 |
| Top threads CPU % | nsh-engine 1.1, bench-nsh 0.7 | nsh-engine 1.0, bench-nsh 0.8 | nsh-engine 1.0, bench-nsh 0.4 | bench-rust 1.1 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.71 / 2.62 | 0.63 / 2.78 | 0.68 / 2.57 | 1.42 / 4.02 |
| Frame work p50 / p99 ms | 0.72 / 2.62 | 0.63 / 2.78 | 0.68 / 2.57 | 1.43 / 4.03 |
| Trigger to frame p50 / p99 ms | 2.4 / 9.1 | 2.6 / 8.8 | 2.0 / 3.6 | 1.4 / 4.0 |
| Trigger to frame (verified) p50 / p99 ms | 2.4 / 9.1 | 2.6 / 8.8 | 2.0 / 3.6 | 1.4 / 4.0 |
| Late frames | 0 | 0 | 0 | 0 |
| **switch_tab** |  |  |  |  |
| fps | 2.5 | 2.6 | 2.5 | 2.5 |
| CPU % | 4.7 | 4.6 | 4.8 | 1.2 |
| Top threads CPU % | nsh-engine 2.5, bench-nsh 2.1, [vkps] Update 0.1 | nsh-engine 2.5, bench-nsh 2.1 | nsh-engine 2.7, bench-nsh 2.1 | bench-rust 0.9, Timer 0.1, [vkps] Update 0.1 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 3.97 / 5.01 | 3.91 / 4.80 | 3.97 / 6.97 | 2.20 / 9.18 |
| Frame work p50 / p99 ms | 3.97 / 5.01 | 3.92 / 4.80 | 3.97 / 6.97 | 2.22 / 9.19 |
| Trigger to frame p50 / p99 ms | 22.5 / 30.0 | 22.2 / 30.5 | 22.4 / 30.3 | 2.2 / 9.2 |
| Trigger to frame (verified) p50 / p99 ms | 22.5 / 30.0 | 22.2 / 30.5 | 22.4 / 30.3 | 2.2 / 9.2 |
| Late frames | 0 | 0 | 0 | 1 |
| **filter_50k** |  |  |  |  |
| fps | 6.9 | 4.0 | 6.7 | 3.9 |
| CPU % | 4.4 | 4.2 | 5.7 | 1.4 |
| Top threads CPU % | nsh-engine 2.8, bench-nsh 1.3 | nsh-engine 3.0, bench-nsh 1.0, [vkps] Update 0.2 | nsh-engine 4.5, bench-nsh 1.1 | bench-rust 1.1, Timer 0.1 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 1.71 / 4.07 | 1.69 / 3.68 | 1.67 / 3.65 | 1.01 / 1.93 |
| Frame work p50 / p99 ms | 1.72 / 4.08 | 1.69 / 3.68 | 1.67 / 3.65 | 1.02 / 1.94 |
| Trigger to frame p50 / p99 ms | 9.1 / 18.1 | 9.4 / 12.1 | 12.8 / 16.4 | 2.3 / 3.4 |
| Trigger to frame (verified) p50 / p99 ms | 9.1 / 18.1 | 9.4 / 12.1 | 12.8 / 16.4 | 2.3 / 3.4 |
| Late frames | 0 | 0 | 0 | 0 |
| **select_under_load** |  |  |  |  |
| fps | 112.7 | 112.8 | 111.0 | 122.8 |
| CPU % | 10.5 | 9.3 | 11.8 | 16.7 |
| Top threads CPU % | bench-nsh 5.7, nsh-engine 4.9 | bench-nsh 5.2, nsh-engine 4.1 | bench-nsh 5.8, nsh-engine 5.8, [vkps] Update 0.1 | bench-rust 16.4 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.35 / 0.91 | 0.34 / 0.94 | 0.37 / 1.07 | 1.08 / 1.68 |
| Frame work p50 / p99 ms | 0.35 / 0.92 | 0.34 / 0.94 | 0.37 / 1.07 | 1.09 / 2.09 |
| Trigger to frame p50 / p99 ms | 0.8 / 9.7 | 0.7 / 9.6 | 1.4 / 10.1 | 1.1 / 9.5 |
| Trigger to frame (verified) p50 / p99 ms | 0.8 / 9.8 | 0.7 / 10.1 | 1.4 / 10.4 | 1.1 / 9.5 |
| Late frames | 0 | 0 | 0 | 0 |
| **cooldown** |  |  |  |  |
| fps | 3.3 | 3.7 | 4.0 | 5.7 |
| CPU % | 0.8 | 0.7 | 1.1 | 1.3 |
| Top threads CPU % | bench-nsh 0.3, nsh-engine 0.3 | nsh-engine 0.3 | nsh-engine 0.7, bench-nsh 0.3 | bench-rust 1.0, Timer 0.3 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.70 / 1.49 | 0.71 / 1.60 | 0.72 / 1.49 | 1.35 / 2.19 |
| Frame work p50 / p99 ms | 0.70 / 1.49 | 0.71 / 1.60 | 0.72 / 1.49 | 1.35 / 2.20 |
| Trigger to frame p50 / p99 ms | no samples | 2.1 / 2.1 | no samples | no samples |
| Trigger to frame (verified) p50 / p99 ms | no samples | 2.1 / 2.1 | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |

## Scenario: quotes (1 run each)

| Phase / metric | nsh-hybrid | nsh-rust | nsh-tsx | rust-gpui |
|---|---|---|---|---|
| Startup to first frame ms | 166 | 202 | 196 | 409 |
| Peak memory MB | n/a | n/a | n/a | n/a |
| **idle** |  |  |  |  |
| fps | 0.1 | 0.1 | 0.1 | 0.0 |
| CPU % | 0.3 | 0.3 | 0.3 | 0.6 |
| Top threads CPU % | nsh-engine 0.3, bench-nsh 0.1 | bench-nsh 0.1, nsh-engine 0.1 | nsh-engine 0.2 | bench-rust 0.2, Timer 0.1 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 1.18 / 1.18 | 1.70 / 1.70 | 1.12 / 1.12 | 0.00 / 0.00 |
| Frame work p50 / p99 ms | 1.18 / 1.18 | 1.71 / 1.71 | 1.12 / 1.12 | 0.00 / 0.00 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **r200** |  |  |  |  |
| fps | 107.2 | 107.8 | 107.3 | 118.2 |
| CPU % | 15.8 | 15.4 | 17.3 | 19.1 |
| Top threads CPU % | bench-nsh 8.1, nsh-engine 7.4 | bench-nsh 8.0, nsh-engine 7.3 | bench-nsh 8.9, nsh-engine 8.3 | bench-rust 18.7, Timer 0.1 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.54 / 1.72 | 0.54 / 1.81 | 0.60 / 1.81 | 1.26 / 2.49 |
| Frame work p50 / p99 ms | 0.54 / 1.72 | 0.54 / 1.81 | 0.60 / 1.81 | 1.28 / 2.55 |
| Push to frame p50 / p99 ms | 1.6 / 8.7 | 1.6 / 8.5 | 1.6 / 9.8 | 1.4 / 10.4 |
| Push to frame (verified) p50 / p99 ms | 1.6 / 8.7 | 1.6 / 8.5 | 1.6 / 9.8 | 1.4 / 10.4 |
| Late frames | 0 | 0 | 0 | 0 |
| **r1000** |  |  |  |  |
| fps | 109.3 | 109.9 | 110.0 | 119.8 |
| CPU % | 20.0 | 18.0 | 20.2 | 20.2 |
| Top threads CPU % | nsh-engine 10.2, bench-nsh 9.7 | nsh-engine 9.0, bench-nsh 8.9 | nsh-engine 10.6, bench-nsh 9.5 | bench-rust 20.0, Timer 0.1 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.65 / 1.79 | 0.61 / 1.86 | 0.63 / 1.97 | 1.36 / 2.16 |
| Frame work p50 / p99 ms | 0.65 / 1.79 | 0.61 / 1.86 | 0.63 / 1.97 | 1.37 / 2.54 |
| Push to frame p50 / p99 ms | 1.9 / 4.1 | 1.8 / 4.7 | 1.8 / 5.8 | 1.4 / 6.9 |
| Push to frame (verified) p50 / p99 ms | 1.9 / 4.1 | 1.8 / 4.7 | 1.8 / 5.8 | 1.4 / 6.9 |
| Late frames | 0 | 0 | 0 | 0 |
| **r5000** |  |  |  |  |
| fps | 102.6 | 100.4 | 101.7 | 119.4 |
| CPU % | 30.0 | 26.3 | 29.7 | 20.6 |
| Top threads CPU % | nsh-engine 16.7, bench-nsh 13.3, [vkps] Update 0.1 | nsh-engine 15.3, bench-nsh 11.0 | nsh-engine 17.7, bench-nsh 12.1, [vkps] Update 0.1 | bench-rust 20.1, Timer 0.1 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.87 / 2.06 | 0.83 / 2.04 | 0.90 / 2.13 | 1.38 / 2.42 |
| Frame work p50 / p99 ms | 0.87 / 2.06 | 0.83 / 2.04 | 0.90 / 2.13 | 1.38 / 2.55 |
| Push to frame p50 / p99 ms | 3.3 / 10.4 | 2.6 / 12.2 | 2.8 / 6.0 | 1.5 / 7.2 |
| Push to frame (verified) p50 / p99 ms | 3.3 / 10.4 | 2.6 / 12.2 | 2.8 / 6.0 | 1.5 / 7.2 |
| Late frames | 0 | 0 | 0 | 0 |
| **r5000_scroll** |  |  |  |  |
| fps | 141.7 | 142.3 | 141.9 | 144.3 |
| CPU % | 48.4 | 43.9 | 48.0 | 27.6 |
| Top threads CPU % | nsh-engine 25.3, bench-nsh 23.0 | nsh-engine 23.3, bench-nsh 20.7, [vkps] Update 0.1 | nsh-engine 26.3, bench-nsh 21.6 | bench-rust 27.4, Timer 0.1 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 1.31 / 2.64 | 1.22 / 2.53 | 1.28 / 2.88 | 1.55 / 3.10 |
| Frame work p50 / p99 ms | 1.31 / 2.64 | 1.23 / 2.53 | 1.28 / 2.89 | 1.59 / 3.56 |
| Push to frame p50 / p99 ms | 5.5 / 9.7 | 5.1 / 9.6 | 5.4 / 9.8 | 5.3 / 9.9 |
| Push to frame (verified) p50 / p99 ms | 5.5 / 9.7 | 5.1 / 9.6 | 5.4 / 9.8 | 5.3 / 9.9 |
| Late frames | 0 | 0 | 0 | 0 |
| **r20000** |  |  |  |  |
| fps | 102.0 | 101.1 | 101.7 | 120.0 |
| CPU % | 78.1 | 73.1 | 77.9 | 25.1 |
| Top threads CPU % | nsh-engine 51.6, bench-nsh 26.5 | nsh-engine 51.2, bench-nsh 21.6, [vkps] Update 0.1 | nsh-engine 55.3, bench-nsh 22.6, [vkps] Update 0.1 | bench-rust 24.7, Timer 0.2, [vkps] Update 0.2 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 1.72 / 3.37 | 1.63 / 3.27 | 1.69 / 3.23 | 1.75 / 3.28 |
| Frame work p50 / p99 ms | 1.72 / 3.37 | 1.63 / 3.27 | 1.69 / 3.23 | 1.77 / 3.36 |
| Push to frame p50 / p99 ms | 7.6 / 13.5 | 7.2 / 13.8 | 7.6 / 13.2 | 1.9 / 8.0 |
| Push to frame (verified) p50 / p99 ms | 7.6 / 13.5 | 7.2 / 13.8 | 7.6 / 13.2 | 1.9 / 8.0 |
| Late frames | 0 | 0 | 0 | 0 |
| **cooldown** |  |  |  |  |
| fps | 1.4 | 1.8 | 1.4 | 3.2 |
| CPU % | 0.6 | 0.4 | 0.7 | 1.0 |
| Top threads CPU % | nsh-engine 0.4 | bench-nsh 0.4, nsh-engine 0.2 | nsh-engine 0.4 | bench-rust 0.8 |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 1.74 / 1.81 | 1.26 / 2.13 | 1.38 / 2.98 | 1.53 / 1.93 |
| Frame work p50 / p99 ms | 1.74 / 1.81 | 1.26 / 2.13 | 1.38 / 2.98 | 1.54 / 1.95 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
