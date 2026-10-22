# Windows counterpart of scripts/setup.sh: NSH d3655d3 + patches in vendor\nsh, the NSH CLI, the TSX bundle,
# bench-rust (gpui-kit) and bench-nsh (pure TSX / TSX + Rust modules / NSH Rust, one binary). Opens no window.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\setup-windows.ps1 [-Toolchain stable]
# Needs git, Rust (MSVC toolchain) and the Visual Studio C++ build tools. Messages are in English because
# Windows PowerShell 5.1 consoles often garble other text.
param(
    # Rust toolchain for this script only (the NSH repo's rust-toolchain.toml would otherwise trigger a rustup update)
    [string]$Toolchain = $(if ($env:RUSTUP_TOOLCHAIN) { $env:RUSTUP_TOOLCHAIN } else { "stable" })
)
# Continue: cargo and git write progress to stderr, which "Stop" would turn into terminating errors
$ErrorActionPreference = "Continue"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
$env:RUSTUP_TOOLCHAIN = $Toolchain
$NshRev = "d3655d3"

function Step([string]$name, [scriptblock]$cmd) {
    Write-Host "== $name"
    $global:LASTEXITCODE = 0
    & $cmd
    if ($LASTEXITCODE -ne 0) { Write-Host "!! $name failed (exit $LASTEXITCODE)"; exit $LASTEXITCODE }
}

if (-not (Test-Path vendor\nsh\.git)) {
    Step "clone nsh" { git clone https://github.com/longbridge/nsh.git vendor/nsh }
}
# Fetch only when the pinned commit is missing (works offline on a prepared checkout)
git -C vendor/nsh cat-file -e "$NshRev^{commit}" 2>$null
if ($LASTEXITCODE -ne 0) { Step "fetch nsh" { git -C vendor/nsh fetch -q origin } }
Step "checkout nsh $NshRev" { git -C vendor/nsh checkout -q -f $NshRev }
Step "clean nsh" { git -C vendor/nsh clean -fdq }
foreach ($p in "nsh-001-reset-runaway-count", "nsh-002-perf-diagnostics", "nsh-003-windows-diagnostics") {
    Step "apply $p" { git -C vendor/nsh apply "$Root/patches/$p.patch" }
}
Step "build nsh-cli" { cargo build --release --manifest-path vendor/nsh/Cargo.toml -p nsh-cli }
Step "bundle App.tsx" { & vendor\nsh\target\release\nsh.exe build nsh-app/app/App.tsx -o nsh-app/out/app }
Step "build bench-rust" { cargo build --release -p bench-rust }
$env:CARGO_TARGET_DIR = "target-nsh"
Step "build bench-nsh" { cargo build --release --manifest-path nsh-app/Cargo.toml }
Remove-Item Env:CARGO_TARGET_DIR
Write-Host "Done. Smoke: scripts\run-windows.ps1 1 -Quick (about 5 minutes); full: scripts\run-windows.ps1 3. Keep the screen on and do not touch the PC while it runs."
