#!/bin/bash
# 诊断运行：scripts/probe.sh <rust|nsh|hybrid|rustui> <场景> <阶段,阶段> <输出目录> [samply]
# 只跑指定阶段（BENCH_PHASES），使用带符号的 profiling 构建；第 5 个参数为 samply 时同时采 CPU 样本。
set -euo pipefail
cd "$(dirname "$0")/.."
impl=$1 sc=$2 phases=$3 out=$4 prof=${5:-}
mkdir -p "$out"
source scripts/window-rule.sh
export TMPDIR="${TMPDIR:-$HOME/.cache/nsh-perf-bench-test-tmp}"
mkdir -p "$TMPDIR"
mode=tsx
case "$impl" in
  rust) cmd=(./target/profiling/bench-rust) ;;
  nsh) cmd=(./target-nsh/profiling/bench-nsh nsh-app/out/app) ;;
  hybrid) mode=hybrid; cmd=(./target-nsh/profiling/bench-nsh nsh-app/out/app) ;;
  rustui) mode=rustui; cmd=(./target-nsh/profiling/bench-nsh nsh-app/out/app) ;;
  *) echo "未知实现：$impl" >&2; exit 1 ;;
esac
log="$out/$impl-$sc.log"
if [[ "$prof" == samply ]]; then
  cmd=(samply record --save-only --unstable-presymbolicate --rate 2000 -o "$out/$impl-$sc.profile.json.gz" -- "${cmd[@]}")
fi
BENCH_MODE=$mode BENCH_SCENARIO=$sc BENCH_PHASES=$phases BENCH_OUT=$out timeout 300 "${cmd[@]}" > "$log" 2>&1
grep '\[bench\]' "$log" || true
