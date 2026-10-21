#!/bin/bash
# 诊断批次（约 15 分钟）：scripts/diag_short.sh <输出目录>
# 分段计时各 1 次；A/B 对照 2–3 次交错；默认 NSH 原生后端。结果用 scripts/diag_report.py 汇总。
set -uo pipefail
cd "$(dirname "$0")/.."
OUT=$1
TUNE='glibc.malloc.mmap_threshold=268435456:glibc.malloc.trim_threshold=268435456'
run() { local dir=$1 impl=$2 sc=$3 ph=$4; shift 4; echo "$(date +%T) $dir $impl $sc $*"; env "$@" scripts/probe.sh "$impl" "$sc" "$ph" "$dir" > /dev/null || echo "失败：$dir" >&2; sleep 2; }
# 1 行情分段计时
for i in nsh hybrid rustui; do run "$OUT/quotes_trace/$i" "$i" quotes r1000,r20000 BENCH_TRACE=1; done
# 2 动画单因素对照（逐帧 left / transform / 原生轨迹 / 原生动画）
for i in rust nsh rustui; do run "$OUT/anim/$i" "$i" animation_ab anim_js_1000,anim_js_tx_1000,anim_js_curve_1000,anim_native_1000 BENCH_TRACE=1; done
# 3 详情 A/B（NSH Rust orig / memo / field，3 次交错）+ 各写法调用计数
for r in 1 2 3; do
  for d in $(printf '%s\n' orig memo field | tail -n +$r; printf '%s\n' orig memo field | head -n $((r - 1))); do
    run "$OUT/detail/$d/rep$r" rustui interaction select_row,switch_tab BENCH_DETAIL=$d
  done
done
for v in nsh hybrid rustui:orig rustui:memo; do run "$OUT/inter_trace/$v" "${v%%:*}" interaction select_row,switch_tab,select_under_load BENCH_TRACE=1 BENCH_DETAIL="${v#*:}"; done
# 4 布局属性未变时跳过重新布局（BENCH_LAYOUT_DIFF），2 次交错
for r in 1 2; do
  for v in $( ((r % 2)) && echo "off on" || echo "on off"); do
    for i in rustui nsh; do
      run "$OUT/layout/$i-$v/rep$r" "$i" animation_ab anim_js_tx_1000 BENCH_LAYOUT_DIFF=$([[ $v == on ]] && echo 1 || echo 0)
      run "$OUT/layout_quotes/$i-$v/rep$r" "$i" quotes r20000 BENCH_LAYOUT_DIFF=$([[ $v == on ]] && echo 1 || echo 0)
    done
  done
done
# 5 计算拆分 + malloc 调参对照
for i in nsh hybrid; do run "$OUT/compute_ab/$i" "$i" compute_ab compute_probe BENCH_TRACE=1; done
run "$OUT/compute_ab/hybrid-tuned" hybrid compute_ab compute_probe BENCH_TRACE=1 GLIBC_TUNABLES=$TUNE
for v in rust rust:tuned; do i=${v%%:*}; if [[ $v == *tuned ]]; then run "$OUT/compute/$v" "$i" compute compute_100k GLIBC_TUNABLES=$TUNE; else run "$OUT/compute/$v" "$i" compute compute_100k; fi; done
echo "$(date +%T) done"
