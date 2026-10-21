# 平铺式窗口管理器会改掉测试窗口的大小（可见行数随之变化，负载不再一致）。
# Hyprland（Lua 配置）下给标题以 “bench ·” 开头的窗口加运行时规则：浮动、1600×1000、居中；
# 只在本次 Hyprland 会话内有效，hyprctl reload 后消失。由 run.sh / probe.sh source。
bench_window_rule() {
  [[ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]] && command -v hyprctl >/dev/null || return 0
  hyprctl eval 'if nsh_bench_rule then nsh_bench_rule:set_enabled(false) end; nsh_bench_rule = hl.window_rule({ match = { title = "bench ·.*" }, float = true, size = { 1600, 1000 }, center = true })' >/dev/null 2>&1 \
    || echo '警告：无法设置 Hyprland 窗口规则，测试窗口可能被平铺成其他大小' >&2
}
bench_window_rule
