# NSH 性能问题分析任务

## 共同背景与约束

本轮是 Linux / Intel i7-13700KF / NVIDIA RTX 3060 Ti GDDR6X 的单轮实测。四种实现、四个场景均正常退出，使用完整阶段时长；没有做 CPU 栈采样，因此下面的代码路径是待验证假设，不能当成已确认的框架根因。

- [完整报告](../results/linux-20261021-192842/REPORT.md)；[原始数据](../results/linux-20261021-192842/run1/)。
- 测量时基准源码：`0347cf1`；NSH：`d3655d32bff5c4b1346ea04f1e0d9fbde127e1cf` + `patches/nsh-001-reset-runaway-count.patch`。
- 直接 GPUI 使用 gpui-fast 0.1.10，NSH 使用 0.1.9，不能假定底层版本完全相同。
- 行情输入固定；计算和过滤的 TS / Rust 数据生成器不同，算法和规模相同，具体数值不同。
- 分析与诊断运行放在新的结果目录，保留原始基线。先测量再改代码，单个改动分别做 A/B，至少重复 3 次并交错顺序。
- 不要使用 `/tmp`。构建用仓库 target；诊断输出用 `target/agents/<任务>/`；测试临时目录用工作区外的 `$HOME/.cache/nsh-perf-bench-test-tmp`。
- 新的公共 Rust struct 使用 `#[non_exhaustive]` 并实现 `Default`。

## 任务 1：高频行情的管线成本与帧调度

**现象**：`r20000` 下，TSX / hybrid / NSH Rust / GPUI 的 fps 为 104.6 / 104.2 / 104.8 / 120.1，CPU 为 37.6% / 38.3% / 28.0% / 21.4%。`r5000_scroll` 下，NSH 三种写法约 144 fps，GPUI 127.3 fps；所以“行情帧率更低”不等于所有场景吞吐不足，也不能要求静止数据更新必须达到刷新率。

**检查入口**：`nsh-app/src/main.rs` 的 `Bench::start`、`agg_push`、`agg_flush`、`Bench::applied_post`；`App.tsx` 的 `apply`、`applyView`、`QuoteRow`、`HQuoteRow`；NSH `surface.rs`、`rt.rs`、引擎提交队列与响应式更新。

**问题**：

1. 生成→JSON 编码→callback 排队→JSON 解码→signal/effect→UI 提交→绘制各占多少 CPU / 延迟？队列长度与包字节数如何变化？
2. hybrid 每个 10ms 批次立刻 flush，仅在批内去重；它是否把可见行之外的格式化工作提前到所有变化行，抵消了移出 QuickJS 的收益？
3. NSH Rust 去掉 JSON 后 CPU 降低，但行情 fps 基本相同：是更新合并、唤醒节奏、提交队列还是渲染限制？请用时间线证明。
4. 对比同批内去重、跨批按帧合并、只格式化实际变化字段；必须检查最终显示值、闪烁方向、最大等待和背压，不能仅靠丢更新提高 fps。

**交付**：线程 CPU profile、各阶段耗时和队列深度，以及一个单因素 A/B 修复。

## 任务 2：逐帧动画的响应式与布局成本

**现象**：`anim_js_1000` 都约 144 fps，TSX / hybrid / NSH Rust / GPUI CPU 为 41.0% / 40.2% / 32.9% / 16.9%。NSH Rust 没有 QuickJS，仍比直接 GPUI 多耗 CPU。

**检查入口**：`App.tsx::AnimJs`、`rustui.rs::AnimJs`、`rust-app/src/main.rs` 动画分支，以及 NSH signal/effect、样式 patch、布局与 UI 提交。

**问题**：

1. 一个时间 signal 触发多少 effect、属性更新和布局？sin/cos 是否重复计算？1000 个方块的响应式调度、布局和跨线程提交各占多少？
2. 原生动画 CPU 为 11.1% / 11.1% / 11.8%，但原生版用 translateX，逻辑版改 left，轨迹和透明度函数也不同。这不是纯粹的 JS / native 对照。先做相同轨迹、相同属性的分组再归因。
3. 分开测试“JS 与 Rust 逻辑”“left 与 transform”“逻辑与原生调度”，避免一次改三个因素。

**交付**：按线程 / 函数拆分的 CPU 样本、effect 与布局计数、等价动画的多轮数据。

## 任务 3：计算性能与跨语言调用

**现象（p50 ms）**：

| 任务 | TSX | hybrid | NSH Rust | GPUI |
|---|---:|---:|---:|---:|
| sort_100k | 64.69 | 3.33 | 3.31 | 3.74 |
| agg_100k | 9.28 | 0.36 | 0.38 | 0.41 |
| ema_100k | 7.24 | 0.20 | 0.19 | 0.24 |
| json_5mb | 41.74 | 33.97 | 36.09 | 42.72 |

**检查入口**：`work.ts`、`core/src/lib.rs` 的任务函数、`main.rs::Bench::task`、`App.tsx` 的计算定时器。

**问题**：

1. 分别测空 native 调用、纯算法、分配 / GC、数据转换；hybrid 计时包含调用，Rust 计时范围不完全相同。
2. 保持数据预备在计时外，统计首次调用与稳态；核对输出校验值，避免只比较速度。
3. JSON.parse 与 serde_json 构造不同运行时的完整对象树，不是同一种分配工作；为何 JSON 差距远小于排序 / 聚合？
4. 计算同步运行在 JS / NSH 引擎线程或 GPUI 前台任务中。当前 compute_under_load 只做约 0.4ms 的 Rust 聚合，不能证明更重任务不会阻塞。另增重计算混合负载时保留原场景。

**交付**：执行 / 调用 / 分配拆分数据、校验与可复现实验；不要将算法差距全部称为 NSH 渲染开销。

## 任务 4：详情重复计算与整块挂载

**现象**：`switch_tab` 触发到绘制完成 p50，hybrid / NSH Rust / TSX / GPUI 约为 7.0 / 4.8 / 7.0 / 2.3ms。该场景每 400ms 切换 150 行非虚拟列表与详情，包含不同方向的挂载成本。

**已见代码问题**：`rustui.rs::detail` 的 `field(k)` 每次执行 `fields()[k].clone()`，而 `fields()` 构造完整 40 字段向量；每个字段的名称与值又各有一个绑定。存在重复克隆、格式化与分配，实际调用次数需插桩确认。

**问题**：

1. 初始挂载和一次选中行变化分别触发多少次 `fields()`、字符串格式化与节点操作？
2. 将整组详情派生为生命周期正确的 computed 或逐字段计算，能降低多少分配与延迟？核对行情更新和选中行变化后的数据一致性。
3. TSX `Detail` 的 fields/Each 与 hybrid `Bench.detail` 是否也被多次调用？确认生成后的绑定行为，不能照搬 Rust 路径的结论。
4. 将切入列表与切入详情分别计时，区分响应式、创建销毁、布局和首帧时间。

**交付**：调用次数与分配证据、局部修复、选行 / 切页 / 负载下详情更新验证。

## 任务 5：测量可信度与帧关联

**检查入口**：`core/src/lib.rs::Metrics::{on_applied,on_trigger,on_draw,on_present,begin_phase,end_phase}`；两套宿主每 50ms 收集 profiler 事件的循环。

**问题**：

1. 当前 applied 只表示已写状态；`on_draw` 将绘制开始前 applied 的更新关联到该帧，但 NSH UI 提交跨线程。能否通过序号 / commit revision 确认该帧确实包含对应状态？
2. 定时器触发测量没有经过真实 OS 输入。文档应区分程序化动作响应、输入到绘制、物理上屏时间。
3. profiler 延迟采集可能在阶段切换后才消费上一阶段事件；应用队列在 begin_phase 清空，未消费更新和阶段末尾未绘制更新如何统计？
4. `late_frames` 只按已采集的 work_start→draw_end 判断，零超时不代表无等待或无掉帧。需要增加哪些提交等待、帧间隔、未呈现更新指标？
5. 行情 applied_batches、latency.n 和 emitted_batches 不总相等；请按阶段边界、合帧和漏采分类，不能直接认定丢包。
6. 内存只在阶段末采样，现名 peak 是这些采样的最大值；Linux 返回 0。如何做真正的峰值采样，并明确 RSS 与 macOS phys_footprint 不同口径？
7. origin 在 main 初始化，因此启动计时不含装载；刷新率、GPU、窗口可见性、底层版本和运行顺序必须随数据保存。

**交付**：可重复的时序验证、指标定义与边界测试；原始报告保留，改变计时语义时另设基线版本。

## 优先顺序

先并行分析任务 1、2、5；任务 5 的结果决定延迟结论的可信程度。任务 3 可指导业务选择 Rust 计算模块，任务 4 是已经找到明确重复工作的局部优化。当前未修改任何性能工作负载，原始数据仍对应测量时源码。
