#!/bin/bash
# 准备基准用的 NSH：实测版本 d3655d3 + PR #282 补丁（Rust 组件累计约 100 万次更新后冻结的修复），放在 vendor/nsh。
# 再构建 NSH CLI（打 TSX 包用）、两个基准程序。用法：scripts/setup.sh
set -e
cd "$(dirname "$0")/.."
NSH_REV=d3655d3
if [ ! -d vendor/nsh/.git ]; then
  git clone https://github.com/longbridge/nsh.git vendor/nsh
fi
git -C vendor/nsh fetch -q origin
git -C vendor/nsh checkout -q -f "$NSH_REV"
git -C vendor/nsh apply "$PWD/patches/nsh-001-reset-runaway-count.patch"
# 诊断计数器、提交版本探针与 A/B 开关（宿主代码依赖其中的 nsh_core::diag；开关默认关闭，行为同上）
git -C vendor/nsh apply "$PWD/patches/nsh-002-perf-diagnostics.patch"
# NSH 仓库的 rust-toolchain.toml 会触发 rustup 升级并装移动端 target，这里固定用本机 stable
export RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN:-stable}
cargo build --release --manifest-path vendor/nsh/Cargo.toml -p nsh-cli
# TSX 基准包
vendor/nsh/target/release/nsh build nsh-app/app/App.tsx -o nsh-app/out/app
# 纯 Rust（gpui-kit）基准
cargo build --release -p bench-rust
# NSH 基准（纯 TSX / TSX + Rust 模块 / NSH Rust 界面 三种模式同一个二进制）
CARGO_TARGET_DIR=target-nsh cargo build --release --manifest-path nsh-app/Cargo.toml
echo "准备完成。冒烟：BENCH_QUICK=1 scripts/run.sh 1（约 5 分钟）；正式：scripts/run.sh 3（约 50 分钟）。跑的时候亮屏、不要操作电脑"
