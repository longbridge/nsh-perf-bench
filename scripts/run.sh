#!/bin/bash
# 完整基准：scripts/run.sh [N]；BENCH_QUICK=1 缩短阶段时长。
# 需要亮屏、窗口可见；运行期间不要操作电脑。
set -euo pipefail
cd "$(dirname "$0")/.."
N=${1:-3}
SC=${SCENARIOS:-quotes animation interaction compute}
IM=${IMPLS:-rust nsh hybrid rustui}
[[ "$N" =~ ^[1-9][0-9]*$ ]] || { echo '运行次数必须是正整数' >&2; exit 1; }
[[ -z "${BENCH_NOWAIT+x}${BENCH_SOAK+x}" ]] || { echo '标准对比不接受 BENCH_NOWAIT / BENCH_SOAK，请单独运行程序' >&2; exit 1; }
for s in $SC; do
  case "$s" in quotes|animation|interaction|compute) ;; *) echo "未知场景：$s" >&2; exit 1 ;; esac
done
for impl in $IM; do
  case "$impl" in
    rust) test -x target/release/bench-rust ;;
    nsh|hybrid) test -x target-nsh/release/bench-nsh && test -f nsh-app/out/app/bundle.nshc ;;
    rustui|rustui-gpui) test -x target-nsh/release/bench-nsh ;;
    nsh-gpui|hybrid-gpui) test -x target-nsh/release/bench-nsh && test -f nsh-app/out/app/bundle.nshc ;;
    *) echo "未知实现：$impl" >&2; exit 1 ;;
  esac || { echo '缺少基准程序或 TSX 包，请先运行 scripts/setup.sh' >&2; exit 1; }
done
export TMPDIR="${TMPDIR:-$HOME/.cache/nsh-perf-bench-test-tmp}"
python3 - "$TMPDIR" <<'PYCODE'
import pathlib, sys
p = pathlib.Path(sys.argv[1]).resolve()
root = pathlib.Path.cwd().resolve()
if p == pathlib.Path('/tmp') or pathlib.Path('/tmp') in p.parents or p == root or root in p.parents:
    sys.exit('TMPDIR 必须位于 /tmp 和 Git 工作区以外')
PYCODE
mkdir -p "$TMPDIR"
source scripts/window-rule.sh
OUT=results/$(date +%Y%m%d-%H%M%S)
if [[ -n "${BENCH_QUICK+x}" ]]; then OUT=results/quick-${OUT#results/}; fi
mkdir -p results
mkdir "$OUT"
python3 - "$OUT" "$N" "$SC" "$IM" <<'PY'
import hashlib, json, os, pathlib, platform, subprocess, sys
out, count, scenarios, impls = sys.argv[1:]
def command(*args):
    p = subprocess.run(args, text=True, capture_output=True)
    return p.stdout.strip() if p.returncode == 0 else 'unavailable'
files = ['Cargo.lock', 'nsh-app/Cargo.lock', 'patches/nsh-001-reset-runaway-count.patch', 'patches/nsh-002-perf-diagnostics.patch', 'patches/nsh-003-windows-diagnostics.patch']
if 'rust' in impls.split(): files.append('target/release/bench-rust')
if set(impls.split()) - {'rust'}: files.append('target-nsh/release/bench-nsh')
if set(impls.split()) & {'nsh', 'hybrid', 'nsh-gpui', 'hybrid-gpui'}: files.append('nsh-app/out/app/bundle.nshc')
meta = dict(platform=platform.platform(), system=platform.system(),
            memory_supported=platform.system() == 'Darwin',
            repetitions=int(count), scenarios=scenarios.split(), implementations=impls.split(),
            quick='BENCH_QUICK' in os.environ, nsh_backend='native', source_commit=command('git', 'rev-parse', 'HEAD'),
            source_dirty=bool(command('git', 'status', '--porcelain')),
            nsh_commit=command('git', '-C', 'vendor/nsh', 'rev-parse', 'HEAD'),
            rustc=command('rustc', '--version'), cargo=command('cargo', '--version'),
            sha256={f: hashlib.sha256(pathlib.Path(f).read_bytes()).hexdigest() for f in files})
pathlib.Path(out, 'metadata.json').write_text(json.dumps(meta, indent=2) + '\n')
PY
case "$(uname -s)" in
  Darwin) { pmset -g batt | head -1; sysctl -n machdep.cpu.brand_string; sw_vers; } > "$OUT/env.txt" ;;
  Linux) { uname -srmo; if command -v lscpu >/dev/null; then lscpu; fi; if command -v lspci >/dev/null; then lspci | grep -Ei 'VGA|3D|Display' || true; fi; } > "$OUT/env.txt" ;;
  *) uname -srmo > "$OUT/env.txt" ;;
esac
for s in $SC; do
  for i in $(seq 1 "$N"); do
    for impl in $IM; do
      echo "== $s run $i / $impl"
      mode=tsx
      case "$impl" in
        rust) name=rust-gpui; cmd=(./target/release/bench-rust) ;;
        nsh) name=nsh-tsx; cmd=(./target-nsh/release/bench-nsh nsh-app/out/app) ;;
        hybrid) name=nsh-hybrid; mode=hybrid; cmd=(./target-nsh/release/bench-nsh nsh-app/out/app) ;;
        rustui) name=nsh-rust; mode=rustui; cmd=(./target-nsh/release/bench-nsh nsh-app/out/app) ;;
        # 对照：同样三种写法跑在 nsh-platform-gpui 上
        nsh-gpui) name=nsh-tsx-gpui; cmd=(./target-nsh/release/bench-nsh nsh-app/out/app) ;;
        hybrid-gpui) name=nsh-hybrid-gpui; mode=hybrid; cmd=(./target-nsh/release/bench-nsh nsh-app/out/app) ;;
        rustui-gpui) name=nsh-rust-gpui; mode=rustui; cmd=(./target-nsh/release/bench-nsh nsh-app/out/app) ;;
      esac
      backend=native
      [[ $impl == *-gpui ]] && backend=gpui
      log="$OUT/run$i-$s-$impl.log"
      # 保留完整 stderr 和退出码；首帧一直不来时最多等待 180 秒。
      if ! BENCH_BACKEND=$backend BENCH_MODE=$mode BENCH_SCENARIO=$s BENCH_OUT=$OUT/run$i python3 - "${cmd[@]}" > "$log" 2>&1 <<'PY'
import subprocess, sys
try:
    sys.exit(subprocess.run(sys.argv[1:], timeout=180).returncode)
except subprocess.TimeoutExpired:
    sys.exit('benchmark timed out after 180 seconds')
PY
      then
        echo "测试失败，详见 $log" >&2
        exit 1
      fi
      test -s "$OUT/run$i/$name-$s.json" || { echo "缺少结果：$name-$s，详见 $log" >&2; exit 1; }
      sleep 2
    done
  done
done
python3 scripts/report.py "$OUT"
