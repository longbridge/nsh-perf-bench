# NSH 技术方案性能基准

同一份界面、同一份数据、同一个 GPUI Fast 渲染引擎（gpui-fast，GPUI 的优化分支，版本见下），比较四种写法的帧率、延迟、CPU、内存和计算耗时。

## 1. 三步上手

推荐环境：macOS（Apple 芯片或 Intel）、[Rust](https://rustup.rs)（stable）、Xcode 命令行工具（`xcode-select --install`，自带 git 与 python3）。

```sh
git clone https://github.com/longbridge/nsh-perf-bench.git
cd nsh-perf-bench

scripts/setup.sh      # ① 准备：拉 NSH（实测版本 + 修复补丁）、打 TSX 包、编译两个基准程序。首次约 10 分钟
BENCH_QUICK=1 scripts/run.sh 1   # ② 冒烟：每个阶段只跑 3 秒，约 5 分钟，确认能跑通
scripts/run.sh 3      # ③ 正式：4 个场景 × 4 种写法 × 3 次，约 50 分钟
```

Linux 已在本仓库的固定依赖版本上编译运行通过；需要可见的桌面会话、GPU 驱动和 GTK 4 开发库（NSH 原生后端，Arch：`gtk4`；Debian/Ubuntu：`libgtk-4-dev`），其他系统开发库按编译器报错补齐。在 Hyprland 下运行脚本会自动加一条运行时窗口规则，让测试窗口浮动并固定 1600×1000（平铺会改变窗口大小和可见行数），`hyprctl reload` 后消失。Linux 内存指标暂不可用。运行脚本支持两种系统，自动记录环境和版本；程序异常退出、超时或缺少结果会报错。

跑完看 `results/<时间>/REPORT.md`（冒烟在 `results/quick-<时间>/`）。

**跑的时候必须：亮屏、不锁屏、外接电源、不操作电脑。** 屏幕上会轮流弹出 1600×1000 的测试窗口，每个跑完自动关闭。窗口被遮挡、失焦或锁屏都会让帧率和 CPU 失真（锁屏时 GPUI Fast 不绘制）。

## 2. 四种写法

NSH 三种写法默认跑在**本平台的原生后端**上：Linux 是 `nsh-platform-linux`（GTK 4 控件、GSK 绘制、Pango 排版），macOS 是 `nsh-platform-macos`（AppKit、Core Animation、CoreText）。宿主在 `nsh-app/src/native_host.rs`。同样的写法跑在 `nsh-platform-gpui` 上作为对照：`BENCH_BACKEND=gpui`，`run.sh` 里的名字是 `nsh-gpui` / `hybrid-gpui` / `rustui-gpui`，报告列名加 `-gpui`。macOS 原生宿主尚未在 Mac 上实测。

| `run.sh` 里的名字 | 报告列名 | 写法 | 代码 |
|---|---|---|---|
| `nsh` | `nsh-tsx` | **纯 TSX**：界面与逻辑在 TSX（QuickJS），行情以 JSON 进 JS 写 signal | `nsh-app/app/App.tsx`、`nsh-app/app/work.ts`；宿主 `nsh-app/src/main.rs` |
| `hybrid` | `nsh-hybrid` | **TSX + Rust 模块**：界面在 TSX；行情合并 / 格式化、过滤、计算在 NSH 的 Rust 模块 | 同上，`BENCH_MODE=hybrid`（`main.rs` 中 `agg_*` 与 `#[module] Bench`，`App.tsx` 中 `HY` 分支） |
| `rustui` | `nsh-rust` | **NSH Rust**：界面用 NSH 的 Rust 写法（`nsh_core::ui`：`#[component]`、`view()`、signal），无 QuickJS | `nsh-app/src/rustui.rs`，`BENCH_MODE=rustui` |
| `rust` | `rust-gpui` | **纯 Rust（gpui-kit）**：直接写 GPUI Fast（gpui-fast 0.1.10），不经过 NSH，作为参照 | `rust-app/src/main.rs` |

公共部分 `core/src/lib.rs`：确定性行情生成器（固定随机种子，四种写法收到的行情输入一致）、阶段编排、指标统计、计算任务算法。计算 / 过滤数据在 TS 与 Rust 中使用不同的伪随机生成器，算法和规模相同，数值并非逐位相同。

## 3. 测试用例

每个场景以 3 秒 `warmup` 开始、3–5 秒 `cooldown` 结束（不计入对比）。阶段名即报告里的分组名。

### 场景 A：行情 `quotes` —— 高频实时数据

界面：500 行行情表（10 列、虚拟列表、变动行闪烁 600ms）+ 右侧十档盘口与 40 条逐笔 + 底部状态栏。

| 编号 | 阶段 | 时长 | 做什么 | 看什么 |
|---|---|---|---|---|
| A1 | `idle` | 10s | 静止，无推送 | 空闲时是否白耗 CPU、空刷帧（fps 应接近 0） |
| A2 | `r200` | 10s | 每秒 200 条行情（随机标的），每 50ms 一次盘口，约 50 条/秒逐笔 | 常规负载 CPU、帧率 |
| A3 | `r1000` | 10s | 每秒 1000 条 | 活跃时段 |
| A4 | `r5000` | 10s | 每秒 5000 条 | 极端行情 |
| A5 | `r5000_scroll` | 10s | 每秒 5000 条，同时行情表每帧滚动一行 | 高负载下滚动是否掉帧（应接近屏幕刷新率） |
| A6 | `r20000` | 10s | 每秒 20000 条 | 压力上限 |

### 场景 B：动画 `animation`

| 编号 | 阶段 | 时长 | 做什么 | 看什么 |
|---|---|---|---|---|
| B1 | `anim_js_200` | 10s | 200 个方块正弦运动 + 透明度，位置由逻辑层每帧计算 | 逐帧驱动动画的 CPU |
| B2 | `anim_js_1000` | 10s | 同上，1000 个 | |
| B3 | `anim_native_200` | 10s | 同样的方块，NSH 两种 TSX 写法与 NSH Rust 用原生动画值（UI 线程执行，逻辑层不参与逐帧） | 用对写法后的差距 |
| B4 | `anim_native_1000` | 10s | 同上，1000 个 | |

gpui-kit 没有对应的原生动画值，B3 / B4 与 B1 / B2 是同一种实现。

### 场景 C：交互 `interaction` —— 点击后多久看到结果

| 编号 | 阶段 | 时长 | 做什么 | 看什么 |
|---|---|---|---|---|
| C1 | `select_row` | 10s | 每 200ms 选中下一行，右侧刷新 40 字段详情 | 触发→上屏 |
| C2 | `switch_tab` | 10s | 每 400ms 在 150 行**非虚拟**列表与详情页之间切换（整块挂载 / 卸载） | 触发→上屏、超时帧 |
| C3 | `filter_50k` | 10s | 每 250ms 换一个关键词，在 5 万条里按代码 / 名称过滤，前 100 条用虚拟列表显示 | 触发→上屏 |
| C4 | `select_under_load` | 10s | 每秒 1000 条行情的同时每 200ms 点选 | 繁忙时点击是否被拖慢 |

### 场景 D：计算 `compute`

| 编号 | 阶段 | 时长 | 做什么 | 看什么 |
|---|---|---|---|---|
| D1 | `compute_100k` | 15s | 每 300ms 轮流执行：`sort_100k` 10 万条按涨跌幅排序；`agg_100k` 10 万条按 市场×行业 汇总市值与盈亏；`ema_100k` 10 万点 EMA12/26 + MACD + Signal9；`json_5mb` 解析 5MB JSON | 每项耗时 p50 / p95 |
| D2 | `compute_under_load` | 10s | 每秒 1000 条行情的同时，每 400ms 做一次 `agg_100k` | 计算是否变慢、界面是否掉帧 |

## 4. 报告怎么读

`REPORT.md` 每个场景一张表，每格是多次运行的中位数：

| 指标 | 含义 | 怎么算（`core/src/lib.rs` 的 `Metrics`） |
|---|---|---|
| 启动到首帧 ms | 进入 `main` 后到首个 present（不含进程装载） | GPUI Fast profiler 首个 present |
| 峰值内存 MB | 各阶段结束时内存采样的最大值（不是连续采样峰值） | `phys_footprint`（活动监视器"内存"列同口径） |
| fps | 每秒实际绘制帧数，有变化才绘制。**行情场景不代表快慢**：推送每 10ms 一批，帧数主要由“触发了多少次重画”决定，见第 6 节 | profiler present 次数 / 秒 |
| CPU % | 进程 CPU 占用，**单核 = 100%** | `getrusage` CPU 时间 / 墙钟时间 |
| 内存 MB | 阶段结束时的内存 | 同上 |
| 绘制 p50 / p99 | 单帧绘制耗时 | profiler `draw_start → draw_end`。**不能跨写法比较**：NSH 的状态计算在引擎线程、不计入绘制，gpui-kit 在绘制阶段构建元素 |
| 帧工作量 p50 / p99 | 单帧从开始工作到画完 | `FrameTiming::work_start() → draw_end`，不含等待垂直同步 |
| 推送→上屏 / 触发→上屏 | 数据进入发送统计或交互触发，到应用后首个绘制完成；不是屏幕扫描完成 | 交互场景显示为"触发→上屏" |
| 超时帧 | 帧工作量超过一个刷新周期的帧数 | |
| 计算 xxx p50 / p95 | 单次计算耗时 | TSX 用 `performance.now()`；`nsh-hybrid` 在 JS 侧计时（含调 Rust 的开销）；Rust 用 `Instant` |

延迟根据 applied 时间与 draw 时间关联，尚未使用帧版本确认该帧已包含对应更新。NSH 的信号应用与 UI 提交跨线程，因此这是响应近似值。`超时帧 = 0` 只说明采集到的帧工作量未超过周期，不代表没有等待或掉帧。没有采样的延迟显示为“无样本”。

每次运行的原始数据在 `results/<时间>/run<N>/<写法>-<场景>.json`，日志在 `results/<时间>/run<N>-<场景>-<写法>.log`。

## 5. 常用操作

```sh
SCENARIOS="quotes" scripts/run.sh 3                       # 只跑一个场景（quotes / animation / interaction / compute）
IMPLS="rust rustui" scripts/run.sh 3                      # 只跑部分写法（rust / nsh / hybrid / rustui）
BENCH_SCENARIO=quotes ./target/release/bench-rust         # 单独跑一次 gpui-kit 版，日志打在终端
BENCH_MODE=rustui BENCH_SCENARIO=quotes ./target-nsh/release/bench-nsh nsh-app/out/app   # NSH Rust 版
BENCH_SCENARIO=quotes ./target-nsh/release/bench-nsh nsh-app/out/app                     # 纯 TSX 版（不设 BENCH_MODE）
python3 scripts/report.py results/<时间>                  # 重新生成某次结果的汇总表
BENCH_TRACE=1 scripts/probe.sh rustui quotes r1000,r20000 target/agents/x   # 诊断：只跑指定阶段，带分段计时（profiling 构建）
python3 scripts/diag_report.py target/agents/x --counters --spans          # 诊断汇总：线程 CPU、NSH 内部计数、分段耗时
```

改了测试代码之后：TSX 改动重跑 `vendor/nsh/target/release/nsh build nsh-app/app/App.tsx -o nsh-app/out/app`；Rust 改动重跑 `scripts/setup.sh`（增量编译，很快）。

## 6. 核对时请注意

- **gpui-kit 版是精简写法**：行情表用 `uniform_list` + 普通 `div`，没有用 gpui-kit 的 `Table` 等现成组件，也没有主题、悬停态、选择框。用 gpui-kit 现成组件写同样的界面，CPU 会更高。
- gpui-kit 版每批行情 `cx.notify()` 整个视图，下一帧统一重建可见行；NSH 版按 signal 粒度更新绑定。
- 为什么固定 NSH `d3655d3` + 补丁：这是实测时的版本。补丁（`patches/`，上游 PR https://github.com/longbridge/nsh/pull/282）修复 NSH Rust 组件累计约 100 万次更新后冻结的缺陷，不打补丁 `nsh-rust` 在 A6 会掉到约 49 fps。NSH 上游 main 之后把 `nsh-ui-mobile` 改名为 `nsh-base`，换版本需要改代码。
- **渲染引擎都是 gpui-fast，版本不同**：纯 GPUI Fast 版依赖 `gpui-kit =0.8.0` 并打开 `gpui-fast` feature，gpui-kit 此时 `pub use ::gpui_fast::*`，实际渲染用 **gpui-fast 0.1.10**；依赖树里同时编译进的 `gpui-pre 0.3.9` 是 gpui-kit 的非可选依赖，这个 feature 组合下不参与渲染。NSH 三种写法经 `nsh-platform-gpui` 使用 **gpui-fast 0.1.9**。两版帧计时（profiler）接口相同。粗看 0.1.9 → 0.1.10 的源码差异（约 1000 行），主要是输入法处理器的保留重绘判断、热重载、键盘焦点与结构体字段顺序，没有看到绘制热路径的优化，但未做同版本对照。gpui-fast 的优化 NSH 与纯 GPUI Fast 版都享受到，两者的 CPU 差距在 gpui-fast 之上的那一层。
- **行情场景的 fps 不是吞吐量**：推送固定每 10ms 一批，有变化才重画，所以帧数 ≈ 推送批次（约 100/秒）+ 各写法自己的额外重画。gpui-kit 版每 50ms 检查一次闪烁，只要有行在闪就 `cx.notify()` 整个视图，额外约 20 帧/秒；NSH 版的闪烁由 100ms 一次的时间 signal 推动，多与推送帧重合。因此 r200～r20000 下 GPUI Fast 版约 118–120 fps、NSH 约 101–110 fps，是重画触发方式不同，不是 GPUI Fast 画得更快（屏幕 144Hz，`r5000_scroll` 时 NSH 也能到 144）。比较行情负载应看 CPU：同样输入下 GPUI Fast 版帧更多、CPU 更低。
- 历史 Apple M4 / macOS 26.5.1 数据见 `results/20261021-combined/`（`REPORT.md` 汇总，`SOURCES.txt` 说明每列来自哪一轮）。Linux 本轮实测及局限见下文。

## 7. 常见问题

| 现象 | 处理 |
|---|---|
| 编译时 rustup 开始下载 iOS / Android target 或升级工具链 | NSH 仓库的 `rust-toolchain.toml` 引起。`setup.sh` 已用 `RUSTUP_TOOLCHAIN=stable` 规避；手动编译 NSH 时也加上它 |
| 日志里 `no Metal Toolchain` / 首次启动稍慢 | 正常：没装 Xcode Metal 工具链时 GPUI Fast 在启动时编译着色器，不影响测量 |
| 某个阶段 fps 异常（如 `idle` 不为 0、滚动远低于刷新率） | 多半是测试期间操作了电脑或窗口被遮挡，重跑该场景：`SCENARIOS=... IMPLS=... scripts/run.sh 1` |
| 日志出现 `potential infinite loop: an effect keeps changing what it reads` | NSH 补丁没打上：删掉 `vendor/` 重跑 `scripts/setup.sh` |
| 公司代理导致 git / cargo 下载失败 | 先 `unset HTTP_PROXY HTTPS_PROXY http_proxy https_proxy` 再跑 |

## 8. Linux 实测（2026-10-21）

Intel Core i7-13700KF / Linux 7.2.8 / NVIDIA RTX 3060 Ti，Hyprland 0.56.2，3840×2160 @ 144Hz。NSH 跑在原生 GTK 4 后端，测试窗口浮动 1600×1000；4 种实现 × 4 个场景各一次完整阶段时长。CPU 单核满载为 100%，越低越好；耗时为 p50。

| 指标 | 纯 TSX | TSX + Rust | NSH Rust | 纯 GPUI Fast |
|---|---:|---:|---:|---:|
| 每秒 1000 条行情：CPU | 20.2% | 20.0% | 18.0% | 20.2% |
| 每秒 2 万条行情：CPU | 77.9% | 78.1% | 73.1% | **25.1%** |
| 同阶段推送→上屏 | 7.6 ms | 7.6 ms | 7.2 ms | **1.9 ms** |
| 5000 条/秒并滚动：CPU | 48.0% | 48.4% | 43.9% | **27.6%** |
| 1000 方块逻辑逐帧动画：CPU | 51.5% | 49.7% | 39.9% | **14.8%** |
| 1000 方块原生动画：CPU | 21.9% | 21.1% | 21.9% | **14.9%** |
| 选中行→上屏 | 2.0 ms | 2.4 ms | 2.7 ms | **1.4 ms** |
| 切换页签→上屏 | 22.4 ms | 22.5 ms | 22.2 ms | **2.2 ms** |
| 5 万条过滤→上屏 | 12.8 ms | 9.1 ms | 9.4 ms | **2.3 ms** |
| 10 万条排序 | 59.2 ms | 3.3 ms | 3.1 ms | 3.6 ms |
| 10 万条聚合 | 9.36 ms | 0.43 ms | 0.41 ms | 0.43 ms |
| 5MB JSON 解析 | 40.1 ms | 41.8 ms | 37.9 ms | 45.5 ms |

[完整报告（英文）](results/20261021-212102/REPORT.md) · [原始 JSON](results/20261021-212102/run1/) · [运行来源与限制](results/20261021-212102/SOURCES.txt) · **[性能分析（英文）](docs/PERFORMANCE-ANALYSIS.md)**。单轮数据，未评估波动；Linux 内存统计未实现。

结论（证据见分析文档）：

- **有渲染工作的场景纯 GPUI Fast 都更快**：轻负载时 NSH 持平（空闲更低），2 万条/秒时 NSH 约 3 倍 CPU，逐帧动画约 3 倍，切页签、过滤这类整块挂载慢 4–10 倍。fps 不能比较快慢：各写法都受推送节奏（约 100 帧）或屏幕刷新率（144）限制。
- **原生后端比 NSH 跑在 GPUI Fast 上（nsh-platform-gpui）更费**：引擎线程 67% 的时间在 Pango 测量文本（每次价格变化都要重新排版，平均 1.85 次）；GTK 主线程为每个节点的控件做快照和 GSK 渲染，原生动画 22% 对 GPUI Fast 后端 11%。
- **NSH 内部，语言只影响 JS 那一段**：NSH Rust 比 TSX 省 4–12 个百分点；布局、文本、渲染三种写法相同。计算放在 Rust（hybrid / NSH Rust）与纯 GPUI Fast 一样快，留在 JS 慢 15–20 倍，跨语言调用每次只有几微秒。
- **已验证的局部修复**：样式未改变布局属性时不重新布局（引擎线程 −15%～−34%）；NSH Rust 详情字段改为 `computed`（每次选中 `fields()` 从 80 次降到 1 次，约 7000 次分配降到约 90 次，选中延迟 2.23→1.69ms）。两者都是 A/B 开关，默认未启用。
- **计时可信度**：提交版本校验显示现有 applied 关联每阶段最多错 0–1 帧；阶段切换最多 7 帧计入下一阶段（fps 影响 <0.5%）。

旧的 Linux 结果 [linux-20261021-192842](results/linux-20261021-192842/REPORT.md) 中 NSH 跑在 GPUI Fast 后端（nsh-platform-gpui）上，且测试窗口很可能被平铺成更小尺寸，只作定性参考。任务清单见 [NSH 性能分析任务清单](docs/PERFORMANCE-ISSUES.md)。

## 9. 对外发布准备

本基准使用生成数据和公开依赖。已清理当前文件中的内部 clone 地址、内部项目名注释和应用 ID；GitHub 仓库已整理为不含这些标识的全新初始提交；内部仓库保留原始历史。模式扫描未发现常见凭证格式，但无法据此确认代码归属或完全排除敏感内容。

本仓库尚无自己的 LICENSE，公开发布前需由权利人确认授权和许可证，并对最终导出目录做检查。当前 GitHub 分支只保留全新初始提交；后续发布继续检查全部可达历史。检查范围、剩余事项及持续基准运行方式见 [发布准备记录](docs/PUBLISHING.md)。
