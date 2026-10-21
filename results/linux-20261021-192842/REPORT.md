# NSH performance benchmark results (linux-20261021-192842)

With several runs each cell is the median of the per-run statistics; with one run it is that run's value. CPU: one core = 100%.

Latency is from the update (or trigger) to the end of the first frame drawn after it, not physical scan-out. "verified" only counts a frame once the main thread has applied the commit carrying the update (NSH hosts); late frames only cover the work of the frames that were collected.

Implementations: this run predates the native NSH hosts, so `nsh-*` ran on nsh-platform-gpui (gpui-fast 0.1.9); `rust-gpui` is plain gpui-kit on gpui-fast 0.1.10. In the quotes scenario fps reflects how often each implementation redraws, not its throughput: compare CPU.

Memory sampling is not implemented here; the 0 in the raw JSON is a placeholder and is shown as n/a.

## Scenario: animation (1 run each)

| Phase / metric | nsh-hybrid | nsh-rust | nsh-tsx | rust-gpui |
|---|---|---|---|---|
| Startup to first frame ms | 233 | 227 | 230 | 270 |
| Peak memory MB | n/a | n/a | n/a | n/a |
| **anim_js_200** |  |  |  |  |
| fps | 142.9 | 142.6 | 143.1 | 142.9 |
| CPU % | 13.1 | 10.9 | 12.6 | 6.6 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.11 / 0.25 | 0.11 / 0.23 | 0.10 / 0.20 | 0.23 / 0.45 |
| Frame work p50 / p99 ms | 0.13 / 1.21 | 0.13 / 1.18 | 0.12 / 1.18 | 0.23 / 1.23 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **anim_js_1000** |  |  |  |  |
| fps | 144.2 | 144.2 | 144.2 | 144.4 |
| CPU % | 40.2 | 32.9 | 41.0 | 16.9 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.32 / 0.61 | 0.32 / 0.61 | 0.33 / 0.60 | 0.83 / 1.66 |
| Frame work p50 / p99 ms | 0.39 / 1.55 | 0.41 / 1.55 | 0.41 / 1.55 | 0.84 / 1.86 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **anim_native_200** |  |  |  |  |
| fps | 144.2 | 144.2 | 144.2 | 143.5 |
| CPU % | 5.6 | 5.8 | 5.6 | 7.0 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.16 / 0.34 | 0.17 / 0.34 | 0.16 / 0.34 | 0.24 / 0.46 |
| Frame work p50 / p99 ms | 0.16 / 0.34 | 0.17 / 0.34 | 0.16 / 0.35 | 0.24 / 1.33 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **anim_native_1000** |  |  |  |  |
| fps | 143.5 | 143.5 | 143.6 | 144.5 |
| CPU % | 11.1 | 11.8 | 11.1 | 17.5 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.53 / 0.88 | 0.59 / 1.02 | 0.53 / 0.92 | 0.85 / 1.60 |
| Frame work p50 / p99 ms | 0.53 / 0.88 | 0.59 / 1.02 | 0.54 / 0.92 | 0.86 / 1.93 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **cooldown** |  |  |  |  |
| fps | 4.7 | 5.7 | 3.3 | 1.0 |
| CPU % | 1.4 | 1.0 | 1.0 | 0.6 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 1.07 / 1.33 | 0.62 / 1.07 | 0.51 / 0.78 | 1.45 / 2.33 |
| Frame work p50 / p99 ms | 1.07 / 1.33 | 0.62 / 1.07 | 0.51 / 1.53 | 1.45 / 2.33 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |

## Scenario: compute (1 run each)

| Phase / metric | nsh-hybrid | nsh-rust | nsh-tsx | rust-gpui |
|---|---|---|---|---|
| Startup to first frame ms | 204 | 194 | 227 | 227 |
| Peak memory MB | n/a | n/a | n/a | n/a |
| **compute_100k** |  |  |  |  |
| fps | 5.7 | 0.1 | 0.1 | 0.1 |
| CPU % | 3.8 | 3.7 | 11.7 | 4.3 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.29 / 0.60 | 0.83 / 0.83 | 0.69 / 0.69 | 1.13 / 1.13 |
| Frame work p50 / p99 ms | 0.29 / 0.63 | 0.84 / 0.84 | 0.70 / 0.70 | 1.15 / 1.15 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| Compute agg_100k p50 / p95 ms | 0.36 / 0.38 | 0.38 / 0.40 | 9.28 / 9.60 | 0.41 / 0.83 |
| Compute ema_100k p50 / p95 ms | 0.20 / 0.22 | 0.19 / 0.24 | 7.24 / 7.89 | 0.24 / 0.24 |
| Compute json_5mb p50 / p95 ms | 33.97 / 35.82 | 36.09 / 37.21 | 41.74 / 44.18 | 42.72 / 44.11 |
| Compute sort_100k p50 / p95 ms | 3.33 / 3.55 | 3.31 / 3.61 | 64.69 / 67.64 | 3.74 / 4.70 |
| **compute_under_load** |  |  |  |  |
| fps | 108.3 | 109.1 | 107.6 | 117.9 |
| CPU % | 16.0 | 15.0 | 19.5 | 17.0 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.57 / 0.98 | 0.63 / 1.22 | 0.61 / 1.15 | 1.09 / 2.03 |
| Frame work p50 / p99 ms | 0.58 / 1.66 | 0.64 / 1.94 | 0.62 / 1.90 | 1.12 / 2.43 |
| Push to frame p50 / p99 ms | 1.2 / 7.4 | 1.2 / 6.3 | 1.3 / 11.1 | 1.2 / 11.4 |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| Compute agg_100k p50 / p95 ms | 0.37 / 0.43 | 0.37 / 0.40 | 8.97 / 9.75 | 0.39 / 0.83 |
| **cooldown** |  |  |  |  |
| fps | 4.0 | 2.3 | 2.7 | 5.7 |
| CPU % | 0.8 | 0.7 | 1.2 | 1.3 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.80 / 0.83 | 0.92 / 0.93 | 0.81 / 0.87 | 1.29 / 1.99 |
| Frame work p50 / p99 ms | 0.81 / 0.84 | 0.93 / 0.94 | 0.82 / 0.87 | 1.31 / 3.00 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |

## Scenario: interaction (1 run each)

| Phase / metric | nsh-hybrid | nsh-rust | nsh-tsx | rust-gpui |
|---|---|---|---|---|
| Startup to first frame ms | 227 | 233 | 196 | 235 |
| Peak memory MB | n/a | n/a | n/a | n/a |
| **select_row** |  |  |  |  |
| fps | 5.1 | 4.9 | 5.1 | 5.1 |
| CPU % | 1.5 | 1.6 | 1.7 | 1.5 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.65 / 1.89 | 0.76 / 1.99 | 0.66 / 1.98 | 1.23 / 3.58 |
| Frame work p50 / p99 ms | 0.66 / 1.90 | 0.77 / 2.00 | 0.67 / 1.99 | 1.23 / 3.59 |
| Trigger to frame p50 / p99 ms | 1.5 / 2.6 | 1.8 / 5.4 | 1.4 / 2.1 | 1.2 / 3.6 |
| Trigger to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **switch_tab** |  |  |  |  |
| fps | 2.5 | 2.5 | 2.5 | 2.5 |
| CPU % | 2.1 | 1.7 | 2.4 | 1.5 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.29 / 0.83 | 0.31 / 0.82 | 0.29 / 0.81 | 2.28 / 5.81 |
| Frame work p50 / p99 ms | 0.30 / 0.84 | 0.33 / 0.83 | 0.31 / 0.82 | 2.30 / 5.82 |
| Trigger to frame p50 / p99 ms | 7.0 / 9.1 | 4.8 / 7.8 | 7.0 / 11.9 | 2.3 / 5.8 |
| Trigger to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **filter_50k** |  |  |  |  |
| fps | 4.1 | 4.0 | 4.1 | 3.9 |
| CPU % | 1.8 | 1.7 | 3.8 | 1.7 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.31 / 0.70 | 0.43 / 0.56 | 0.41 / 0.72 | 0.80 / 2.58 |
| Frame work p50 / p99 ms | 0.33 / 0.72 | 0.44 / 0.57 | 0.43 / 0.74 | 0.83 / 2.60 |
| Trigger to frame p50 / p99 ms | 2.8 / 4.6 | 2.7 / 4.4 | 7.4 / 10.6 | 2.5 / 5.0 |
| Trigger to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **select_under_load** |  |  |  |  |
| fps | 111.6 | 106.9 | 111.9 | 120.7 |
| CPU % | 12.8 | 11.6 | 12.9 | 14.7 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.51 / 0.96 | 0.57 / 1.03 | 0.51 / 0.98 | 0.87 / 1.59 |
| Frame work p50 / p99 ms | 0.52 / 1.62 | 0.58 / 1.75 | 0.52 / 1.67 | 0.89 / 2.33 |
| Trigger to frame p50 / p99 ms | 0.9 / 9.2 | 0.8 / 10.9 | 0.8 / 10.1 | 1.0 / 9.8 |
| Trigger to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **cooldown** |  |  |  |  |
| fps | 3.3 | 4.0 | 3.0 | 6.0 |
| CPU % | 1.0 | 0.8 | 1.1 | 1.3 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.83 / 0.93 | 0.83 / 0.87 | 0.86 / 0.89 | 1.30 / 1.94 |
| Frame work p50 / p99 ms | 0.84 / 0.94 | 0.84 / 0.88 | 0.87 / 0.90 | 1.31 / 1.96 |
| Trigger to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Trigger to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |

## Scenario: quotes (1 run each)

| Phase / metric | nsh-hybrid | nsh-rust | nsh-tsx | rust-gpui |
|---|---|---|---|---|
| Startup to first frame ms | 234 | 234 | 584 | 333 |
| Peak memory MB | n/a | n/a | n/a | n/a |
| **idle** |  |  |  |  |
| fps | 0.1 | 0.1 | 0.1 | 0.7 |
| CPU % | 0.5 | 0.5 | 0.5 | 1.0 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 1.04 / 1.04 | 1.03 / 1.03 | 1.12 / 1.12 | 0.79 / 1.20 |
| Frame work p50 / p99 ms | 1.05 / 1.05 | 1.04 / 1.04 | 1.13 / 1.13 | 1.04 / 3.04 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **r200** |  |  |  |  |
| fps | 107.0 | 107.6 | 106.7 | 118.2 |
| CPU % | 15.4 | 14.9 | 15.5 | 14.5 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.59 / 1.07 | 0.65 / 1.17 | 0.58 / 1.10 | 0.87 / 2.00 |
| Frame work p50 / p99 ms | 0.60 / 1.73 | 0.66 / 1.98 | 0.59 / 1.62 | 0.89 / 2.16 |
| Push to frame p50 / p99 ms | 1.2 / 9.1 | 1.2 / 10.4 | 1.2 / 10.5 | 1.0 / 9.2 |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **r1000** |  |  |  |  |
| fps | 108.7 | 110.0 | 108.8 | 119.8 |
| CPU % | 19.9 | 16.5 | 16.8 | 16.1 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.68 / 1.44 | 0.69 / 1.20 | 0.60 / 1.06 | 0.94 / 2.12 |
| Frame work p50 / p99 ms | 0.70 / 2.24 | 0.69 / 1.91 | 0.61 / 1.64 | 0.95 / 2.32 |
| Push to frame p50 / p99 ms | 1.5 / 7.2 | 1.3 / 9.0 | 1.3 / 5.7 | 1.0 / 7.5 |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **r5000** |  |  |  |  |
| fps | 101.4 | 101.3 | 101.0 | 120.0 |
| CPU % | 22.7 | 18.4 | 19.6 | 18.4 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.68 / 1.36 | 0.67 / 1.17 | 0.62 / 1.02 | 1.16 / 2.27 |
| Frame work p50 / p99 ms | 0.68 / 1.37 | 0.68 / 1.20 | 0.62 / 1.04 | 1.19 / 2.47 |
| Push to frame p50 / p99 ms | 1.8 / 4.4 | 1.5 / 6.1 | 1.6 / 3.3 | 1.3 / 7.5 |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **r5000_scroll** |  |  |  |  |
| fps | 144.0 | 143.9 | 143.9 | 127.3 |
| CPU % | 33.6 | 25.8 | 30.8 | 22.2 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.84 / 1.69 | 0.78 / 1.30 | 0.78 / 1.38 | 1.33 / 2.66 |
| Frame work p50 / p99 ms | 1.02 / 2.59 | 0.82 / 2.25 | 0.84 / 2.29 | 1.38 / 3.18 |
| Push to frame p50 / p99 ms | 5.0 / 8.8 | 4.4 / 7.8 | 4.8 / 8.3 | 5.4 / 40.7 |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **r20000** |  |  |  |  |
| fps | 104.2 | 104.8 | 104.6 | 120.1 |
| CPU % | 38.3 | 28.0 | 37.6 | 21.4 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.77 / 1.32 | 0.76 / 1.30 | 0.77 / 1.28 | 1.41 / 2.57 |
| Frame work p50 / p99 ms | 0.78 / 1.74 | 0.77 / 1.84 | 0.78 / 1.82 | 1.44 / 2.80 |
| Push to frame p50 / p99 ms | 3.4 / 8.1 | 2.4 / 7.4 | 3.3 / 7.9 | 1.7 / 8.1 |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
| **cooldown** |  |  |  |  |
| fps | 1.2 | 0.6 | 0.8 | 3.8 |
| CPU % | 0.6 | 0.5 | 0.7 | 1.0 |
| Top threads CPU % | - | - | - | - |
| Memory MB | n/a | n/a | n/a | n/a |
| Draw p50 / p99 ms | 0.96 / 1.24 | 0.94 / 1.56 | 0.90 / 1.07 | 1.32 / 1.77 |
| Frame work p50 / p99 ms | 0.97 / 1.25 | 0.95 / 1.57 | 0.90 / 1.08 | 1.33 / 2.48 |
| Push to frame p50 / p99 ms | no samples | no samples | no samples | no samples |
| Push to frame (verified) p50 / p99 ms | no samples | no samples | no samples | no samples |
| Late frames | 0 | 0 | 0 | 0 |
