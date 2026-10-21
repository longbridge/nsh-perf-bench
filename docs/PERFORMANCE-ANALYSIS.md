# NSH performance analysis (Linux, 2026-10-21)

Answers to the five questions in [PERFORMANCE-ISSUES.md](PERFORMANCE-ISSUES.md), with the evidence first and the fixes after. Everything here was measured on one machine (Intel i7-13700KF, RTX 3060 Ti, Hyprland, 3840×2160 @ 144 Hz) with the profiling build (release optimizations plus symbols). Diagnostic runs are single runs unless a repetition count is given; the A/B comparisons are 2–3 interleaved runs. Treat differences under ~3 points of CPU as noise.

## 0. What changed in the benchmark

- **NSH now runs on its native backend.** Earlier runs (including `results/linux-20261021-192842`) hosted all three NSH variants on `nsh-platform-gpui`. The default is now the platform backend: `nsh-platform-linux` (GTK 4 widgets, GSK rendering, Pango text) on Linux, `nsh-platform-macos` (AppKit, Core Animation, CoreText) on macOS. The GPUI host is kept for comparison (`BENCH_BACKEND=gpui`, `run.sh` implementations `nsh-gpui`, `hybrid-gpui`, `rustui-gpui`). The macOS host compiles in principle but could not be checked here (cross-compiling `ring` needs an Apple toolchain); it has to be run on a Mac before its numbers are used.
- **GTK 3 / GTK 4 clash.** `nsh-platform-gpui`'s default `webview` feature links GTK 3 through webkit2gtk; with GTK 4 in the same process `gtk_is_initialized` resolved to GTK 3 and the native host panicked at startup. The benchmark does not use web views, so the feature is off.
- **Window size was not controlled.** Hyprland tiles new windows: the "1600×1000" test window was actually 1181×643 (or 581×643 when two were open), so the number of visible rows, and with it the workload, depended on what else was on screen. `run.sh` and `probe.sh` now add a runtime-only Hyprland rule (float, 1600×1000, centered; gone after `hyprctl reload`). **The earlier Linux baseline was most likely run tiled**; compare it with later runs only qualitatively.
- **Frame timing on GTK.** Draws come from the window's `GdkFrameClock` (frames with a paint phase; draw = end of update → after-paint), presents from the compositor's presentation time (Wayland presentation-time). The GPUI hosts keep the GPUI profiler. On macOS a frame is the main-thread work that changed the screen (applying a batch, an animation tick), reported by a hook in the patched surface; there is no present timestamp.
- **Diagnostics added** (see §6): per-thread CPU, NSH-internal counters, stage timers, a commit-version probe, `BENCH_PHASES` to run selected phases, three diagnostic scenarios (`animation_ab`, `compute_ab`, A/B switches), `scripts/probe.sh`, `scripts/diag_report.py`, `scripts/samply_top.py`.

## Summary: who is faster

Standard run on the native backend ([report](../results/20261021-212102/REPORT.md), one run, release build). CPU: one core = 100%, lower is better.

| Phase | gpui-kit | NSH TSX | TSX + Rust | NSH Rust |
|---|---:|---:|---:|---:|
| Idle CPU | 0.6% | 0.3% | 0.3% | 0.3% |
| 1 000 quotes/s, CPU | 20.2% | 20.2% | 20.0% | 18.0% |
| 20 000 quotes/s, CPU | **25.1%** | 77.9% | 78.1% | 73.1% |
| 5 000 quotes/s + scrolling, CPU | **27.6%** | 48.0% | 48.4% | 43.9% |
| 20 000 quotes/s, push → frame p50 | **1.9 ms** | 7.6 ms | 7.6 ms | 7.2 ms |
| 1 000 boxes, logic per frame, CPU | **14.8%** | 51.5% | 49.7% | 39.9% |
| 1 000 boxes, native animation, CPU | **14.9%** | 21.9% | 21.1% | 21.9% |
| Select row → frame p50 | **1.4 ms** | 2.0 ms | 2.4 ms | 2.7 ms |
| Switch tab → frame p50 | **2.2 ms** | 22.4 ms | 22.5 ms | 22.2 ms |
| Filter 50k → frame p50 | **2.3 ms** | 12.8 ms | 9.1 ms | 9.4 ms |
| Sort 100k p50 | 3.6 ms | 59.2 ms | 3.3 ms | 3.1 ms |
| Aggregate 100k p50 | 0.43 ms | 9.36 ms | 0.43 ms | 0.41 ms |
| Parse 5 MB JSON p50 | 45.5 ms | 40.1 ms | 41.8 ms | 37.9 ms |

- **gpui-kit is faster wherever there is rendering work**: at light load NSH is on par (and idles lower), but at 20 000 quotes/s NSH uses ~3× the CPU, per-frame animation ~3×, and mounting-heavy interactions (tab switch, filter) are 4–10× slower. fps is not a differentiator: everyone draws at the push rate or the display rate.
- **Within NSH, the language matters only for the JS share**: NSH Rust saves 4–12 points over TSX; layout, text measuring and GTK rendering are the same for all three. For computation, Rust (hybrid or NSH Rust) matches gpui-kit and TSX is 15–20× slower.
- **The native backend costs more than NSH on GPUI** (one run each, same machine): at 20 000 quotes/s NSH Rust 27% on GPUI vs 73% native; native animation 12% vs 22%. The causes are Pango text measuring on the engine thread (§1) and one GTK widget per node on the main thread (§2).

### Why: retained vs immediate mode

NSH is retained and fine-grained reactive. The engine thread keeps a node tree (core nodes, styles, the taffy layout tree, text measurements); a signal change re-runs only the effects that read it, which emit per-node ops; the core applies them, relays out what is dirty and sends the differences as mount items to the main thread, which keeps its own view tree (one GTK widget or `NSView` per node on the native backends; an `NshView` tree repainted each frame on `nsh-platform-gpui`). gpui-kit keeps entity state but is immediate at the view level: after `cx.notify()` its `render()` rebuilds the frame's elements (only the ~40 visible rows of the quote table), lays them out and paints them, then drops them.

Retained mode pays per node and per change: a style write is hashed, lowered and dirties layout, a text change is re-measured, every change becomes a mount item crossing threads and touching a widget. When many nodes change every frame (20 000 quotes/s, 1 000 moving boxes) that adds up to more than redrawing the visible part. When little changes it wins: NSH idles lower (0.3% vs 0.6%), and a native animated value on the GPUI backend (11%) beat gpui-kit (15%) because only the main thread touches transforms.

## 1. Quote updates: pipeline cost and frame scheduling

### fps is not throughput in this scenario

Quotes arrive in one batch every 10 ms and a frame is drawn only when something changed, so frames ≈ batches (100/s) plus each implementation's own extra redraws. Every NSH variant commits ~100 times a second and draws ~100 frames (`eng_submits` = `ui_batches` ≈ 100/s; one frame per commit). gpui-kit draws ~20 more because its 50 ms timer calls `cx.notify()` on the whole view while any row is flashing; NSH's flash is driven by a 100 ms time signal that mostly coincides with batch frames. The display runs at 144 Hz, and in `r5000_scroll` NSH reaches 144 fps, so the 104 vs 120 fps of the first report is a difference in redraw triggers, not drawing speed. **Compare CPU, not fps, in the quotes phases.**

The queues never back up: the driver → engine channel depth is 0, a batch waits 5 µs (p50) before the engine task takes it, and a posted JS callback waits 4–43 µs before it runs. The engine thread is busy 36–47% of the time at `r20000`.

### Where the time goes (native GTK backend, `r20000`, ms of CPU per second; 10 ms/s = 1% of a core)

| Stage | TSX | TSX + Rust | NSH Rust |
|---|---:|---:|---:|
| Driver thread: Rust merge + format (`agg_push`/`agg_flush`) | – | 33 | – |
| Host: serialize batch to JSON | 4 | – | – |
| JS: `JSON.parse` | 36 | 17 | – |
| JS: write signals + run effects (`batch`) | 76 | 57 | 17 (Rust task writing signals) |
| Core: apply ops / render Rust views | 10 | 8 | 10 |
| Core: flush dirty views into mount items | 41 | 34 | 35 |
| Core: layout (taffy, incl. text measuring) | 266 | 217 | 263 |
| Core: text blocks | 26 | 25 | 28 |
| Engine thread total (busy) | 473 | 367 | 357 |
| Main thread: apply mount items | 32 | 29 | 28 |
| Main thread total (GTK snapshot + GSK render + apply) | 203 | 209* | 175 |
| Process CPU | 67.7% | 57.8% | 53.4% |

\* The driver thread has no name and shows up under the process name, so this column includes the 33 ms/s of merging and formatting.

CPU samples (samply, 2 kHz) of the engine thread at `r20000`, NSH Rust: **67% inside `nsh_core::core::measure_text` → `PangoTextEngine::measure`** (Pango layout, HarfBuzz shaping), called from taffy's flex `calculate_flex_item`. The main thread spends 33% in `gsk_renderer_render`, 21% in widget snapshots and 11% in `ioctl` (GPU submission).

Findings:

1. **Text measuring dominates on the native backend.** Every price change is a new string, so the measure cache (keyed by text hash and width) misses, and taffy asks for the same text under more than one width constraint, each a full Pango layout. 22 000 text updates/s become ~25% of a core. On `nsh-platform-gpui` the same layout stage costs ~80 ms/s instead of ~260.
2. **The three NSH variants share the same floor.** Layout, flush, text and the main thread are identical in kind and close in size across TSX, hybrid and NSH Rust; only the JS stages differ. Removing QuickJS saves the 112 ms/s of parse + effects and nothing else.
3. **Why hybrid does not save much.** Hybrid moves formatting out of JS but formats *every* changed row in Rust (≈165 rows per batch at `r20000`) while only ~40 rows are visible, and still sends a JSON payload (22 KB per batch vs 48 KB) that JS parses and writes into one signal per row. Its JS stages shrink by 38 ms/s, but the driver thread gains 33 ms/s. On the GPUI backend (one run) the totals were equal: 37.2% vs 37.9%. In this native run hybrid is 10 points below TSX, but that is one run and within the run-to-run spread seen elsewhere.
4. **NSH Rust's lower CPU does not raise fps** because fps is capped by the batch rate (see above), not by work per frame.

Fix candidates, each tested on its own:

| Change | Switch | Result (r20000, 2 interleaved runs, engine ms/s) |
|---|---|---|
| Skip taffy relayout when a node's layout style did not change (`set_style` re-dirtied layout on every style write, even color/opacity/transform only) | `BENCH_LAYOUT_DIFF=1` | TSX 435 → 385, NSH Rust 421 → 349; 11 400 (TSX) / 2 700 (Rust) style writes per second no longer dirty layout. Display unchanged (same mount items). |
| Reuse an unwrapped text measure when it fits the width constraint (a single-line text that fits is the same size wrapped) | `BENCH_TEXT_REUSE=1` | **No effect** (CPU and engine time unchanged, 2 runs each). The counters show why: of ~45 000 measure calls per second, ~41 000 miss and are already width-constrained (~1.85 Pango layouts per changed text); only ~100/s are unconstrained, so there is almost nothing to reuse. |

Open item: find out which two width constraints taffy measures each changed text with (a fixed-width, single-line cell should need one), then cache by "fits without wrapping" or measure single-line text once.

Not done, recommended: format only rows that are visible (hybrid), or send numbers and format in the bound text (as TSX does); coalesce per frame only matters once batches arrive faster than frames, which they do not here (100 batches/s vs 144 Hz).

## 2. Per-frame animation: reactive and layout cost

Native GTK backend, 1000 boxes, one run each (`animation_ab`), process CPU %:

| Variant | TSX | NSH Rust | gpui-kit |
|---|---:|---:|---:|
| Logic per frame, `left` (standard `anim_js_1000`) | 51.8 | 41.4 | 15.0 |
| Logic per frame, same path, `translateX` | 50.1 | 44.7 | 14.9 |
| Logic per frame, native animation's curve, `translateX` | 50.6 | 42.8 | 15.4 |
| Native animated values (`anim_native_1000`) | 22.1 | 22.1 | 14.7 |

(gpui-kit has no transform; it always positions with `left`.)

- **JS vs Rust logic:** the `rAF` callback that sets the time signal (and synchronously runs 1000 style effects) costs 1.65 ms per frame in TSX (246 ms/s) vs 0.25 ms in NSH Rust (36 ms/s). In TSX each box's whole style object (8 keys) is rebuilt, encoded and decoded every frame (`nsh_js::protocol::decode_style`, `style_entries`); 70% of the engine thread is in `bind_prop_run`.
- **`left` vs `transform`:** with `translateX` NSH Rust stops emitting 140 000 `Frame` items a second (590/s remain), but CPU does not drop, because every style write still dirtied layout (fixed by `BENCH_LAYOUT_DIFF`, below) and the Rust style path dominates (next point). TSX still emits a `Frame` per box per frame, since its style object always carries `left`.
- **Rust style path:** 67% of NSH Rust's engine thread is `render_components` → `apply_styles`. Each binding update hashes the element's whole `StyleRefinement`, misses the lowered-style cache (every opacity/position value is new), lowers it, clones it into the cache, and every 4096 entries clears the whole cache (`drop_elements` alone is 11.6% of samples). For continuously changing values the cache only adds work. Recommendation: lower and cache the static part of a style and apply bound numeric values directly, or do not insert styles whose bound values changed.
- **Logic vs native scheduling, same property and curve:** 42.8 → 22.1 (Rust), 50.6 → 22.1 (TSX). The remaining 22% is the GTK main thread: per frame it snapshots 1000 `NshView` widgets and renders them with GSK (`gtk_widget_snapshot_child` 35%, `gsk_renderer_render` 27%). gpui-kit draws the same 1000 quads at 15% total. That is the cost of one GTK widget per node.
- **Layout-diff A/B** (`anim_js_tx_1000`, 2 interleaved runs): layout time 17–23 ms/s → 0 (143 000 style writes per second no longer dirty layout); engine thread NSH Rust 318 → 210 ms/s, TSX 399 → 340 ms/s; process CPU moves by 1–3 points (within noise in this sample).

## 3. Heavy compute and the JS ↔ Rust bridge

| Measurement (p50) | Value |
|---|---:|
| Empty native call from JS | 42 ns |
| Native call with one string argument returning a number | 82–92 ns |
| Bridge share of a hybrid task call (outer JS time − inner Rust time) | 4–9 µs |
| TSX sort: copy (`slice`) / sort only / total | 1.3 / 57.4 / 58.8 ms |
| TSX sort: comparator calls / per call | 1 739 304 / ~33 ns |
| TSX sort of plain numbers with a comparator / typed-array native sort | 35.8 / 7.6 ms |
| TSX agg over objects / over typed arrays (SoA) | 9.0 / 8.8 ms |
| TSX EMA over `number[]` / `Float64Array` | 7.3 / 8.6 ms |
| TSX `JSON.parse` 5 MB, objects / same data as arrays | 39.5 / 23.8 ms |
| Rust (standalone) sort / agg / EMA | 2.9 / 0.15 / 0.19 ms |
| Rust (standalone) agg run every 300 ms like the benchmark | 0.40 ms |
| Rust (standalone) serde `Value` / typed `Vec<Item>` / dropping the `Value` | 22.3 / 6.5 / 4.9 ms |
| Rust in the app (engine thread) JSON: allocations / minor page faults | 561 000 / 4 600–16 800 |

- The bridge is not a cost worth optimizing: microseconds per call against milliseconds of work. The hybrid numbers are the Rust algorithm's numbers.
- The TSX gap is interpretation: QuickJS has no JIT. A sort spends its time calling the comparator 1.7 million times; aggregation and EMA spend it in bytecode arithmetic (typed arrays do not help in QuickJS). Allocation is small (the copy is 1.3 ms).
- **Why JSON is close:** `JSON.parse` is QuickJS's C code, and serde's `Value` builds a full tree of maps and strings too; both are allocation-bound. Half of QuickJS's time is object shapes (arrays parse in 24 ms). A typed serde target is 3× faster than `Value`.
- **In-app Rust timings are inflated by the environment:** agg takes 0.15 ms hot but 0.40 ms when run every 300 ms (cold caches, lower clocks), which is what all in-app numbers measure. JSON parsing on the engine thread pays 4 600–16 800 page faults per run as glibc returns and re-faults the arena; `GLIBC_TUNABLES` mmap/trim thresholds cut gpui-kit's main-thread JSON from 40 to 28 ms but did not help the engine thread. These are allocator effects, not NSH.
- Checks: every probe validates its result (sortedness, aggregate vs a naive sum, EMA equality, element count); none failed. Compute runs synchronously on the engine thread, so a long task delays that thread's commits; `compute_under_load` only runs a 0.4 ms aggregation and does not test that.

## 4. Detail panel: repeated computation

NSH Rust's `detail()` builds all 40 fields inside each of the 80 text bindings (name and value of 40 rows).

| Per event | TSX | TSX + Rust | NSH Rust (as written) | NSH Rust, `computed` | NSH Rust, per field |
|---|---:|---:|---:|---:|---:|
| `fields()` calls per selection / per mount | 1 | 1 | 80 | 1 | – (80 single-field calls) |
| Allocations per `fields()` call | – | – | 87 (4.7 KB) | 87 | 2–3 per field |
| Allocations per selection | – | – | ~7 000 (380 KB) | ~90 | ~200 |
| `select_row` trigger → frame p50 (3 runs) | 2.25 | 2.27 | 2.23 | 1.69 | 1.78 |
| Engine ms/s during `select_row` | | | 7.8 | 5.1 | 5.1 |
| `switch_tab` into detail p50 (3 runs) | | | 8.35 | 7.80 | 7.79 |

Under `select_under_load` the selected row is updated by quotes too, so the original code rebuilt all 40 fields 80 times for every quote of that row (5 600 calls in 10 s vs 69 with `computed`). TSX and hybrid already evaluate the list once per change (`each(() => fields(), …)`; the counters confirm 51 calls for 50 selections). Data stays consistent: the `computed` variant reads the same row signal and is re-evaluated on selection and on row updates. Recommendation: derive the field list once (`computed`) in `rustui.rs`.

Switching tabs is dominated by mounting, not by the detail fields: into the detail 8 ms, back into the 150-row list 22 ms, mostly creating ~900 text nodes and measuring them with Pango (on the GPUI backend the whole switch was ~5 ms).

## 5. Timing validity

- **Commit-version check.** Each update now records the number of the engine commit that carries it, and the main thread records when it applied each commit. A frame counts as showing an update only if it started after that commit was applied ("verified"). Across all diagnostic runs the current applied-time matching attributed an update to a frame that did not contain it in 0–1 updates per phase (`early`), so the published latencies are not biased by cross-thread commits at these loads. The check stays in the report as its own row.
- **Phase boundaries.** Profiler events are collected every 50 ms, so up to 7 frames of the previous phase were counted in the next one (`stale_draws`, `stale_presents`); 0–4 unconsumed updates were dropped when a phase began (`dropped_at_begin`) and 0–4 were still waiting at its end (`pending_at_end`). These account for `applied_batches` (1000) vs latency samples (996–1000); nothing is lost. The effect on fps is under 0.5%.
- **Deterministic tests** (`cargo test -p bench-core`): an update applied on the engine thread but drawn before the main thread applied its commit; the same with no probe (single-thread GPUI); frames collected after a phase switch; pending updates dropped at a phase start.
- Still approximate: triggers are programmatic (no OS input), latency ends at draw completion, not scan-out; late frames only cover collected frame work; memory is sampled at phase ends only and is not implemented on Linux (RSS would not match macOS `phys_footprint` anyway).

## 6. Reproducing

```sh
# diagnostic build (release optimizations + symbols)
RUSTUP_TOOLCHAIN=stable CARGO_TARGET_DIR=target-nsh cargo build --profile profiling --manifest-path nsh-app/Cargo.toml
# one implementation, selected phases, with stage timers
BENCH_TRACE=1 scripts/probe.sh rustui quotes r1000,r20000 target/agents/x
python3 scripts/diag_report.py target/agents/x --counters --spans
# CPU samples (needs kernel.perf_event_paranoid <= 1 and a larger perf_event_mlock_kb)
scripts/probe.sh rustui quotes r20000 target/agents/x samply
python3 scripts/samply_top.py target/agents/x/rustui-quotes.profile.json.gz nsh-engine bench-nsh
# the diagnostic batch used here (~15 min)
scripts/diag_short.sh target/agents/run
```

Switches: `BENCH_BACKEND=gpui`, `BENCH_DETAIL=orig|memo|field`, `BENCH_LAYOUT_DIFF=1`, `BENCH_TEXT_REUSE=1`, `BENCH_PHASES=…`, scenarios `animation_ab` and `compute_ab`. The NSH changes are instrumentation and A/B switches in `patches/nsh-002-perf-diagnostics.patch`; with all switches off NSH behaves as at `d3655d3` + `nsh-001`.
