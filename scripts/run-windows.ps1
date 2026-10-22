# Windows counterpart of scripts/run.sh: every scenario x run x implementation starts one benchmark process while
# PresentMon records its frames; scripts/presentmon_join.py then splits the frames into the phases by QPC, and
# scripts/report.py writes results\<time>\REPORT.md.
#
# Usage, from an elevated PowerShell (PresentMon needs administrator rights), local or over SSH:
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run-windows.ps1 1 -Quick -PresentMon C:\tools\PresentMon.exe   # smoke
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run-windows.ps1 3 -PresentMon C:\tools\PresentMon.exe          # full
# SCENARIOS / IMPLS / BENCH_QUICK work as in run.sh (implementations: rust nsh hybrid rustui).
#
# The benchmark windows must open on the interactive desktop of a user who is logged on at the console (not over
# Remote Desktop, which replaces the display path). An SSH session cannot show windows, so each run goes through a
# scheduled task that runs only while that user is logged on (schtasks /it); it is deleted at the end.
# While it runs: screen on, no lock screen, on AC power, nobody using the PC.
param(
    [Parameter(Position = 0)][int]$Runs = 3,
    [switch]$Quick,
    [string]$PresentMon = $(if ($env:PRESENTMON) { $env:PRESENTMON } else { "PresentMon.exe" }),
    # The user logged on at the console, whose desktop shows the windows
    [string]$User = $env:USERNAME,
    [string]$TaskName = "NshPerfBench"
)
$ErrorActionPreference = "Continue"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
$env:PYTHONUTF8 = "1"
if ($env:BENCH_QUICK) { $Quick = $true }
$Scenarios = @($(if ($env:SCENARIOS) { $env:SCENARIOS } else { "quotes animation interaction compute" }) -split "\s+" | Where-Object { $_ })
$Impls = @($(if ($env:IMPLS) { $env:IMPLS } else { "rust nsh hybrid rustui" }) -split "\s+" | Where-Object { $_ })

function Fail([string]$m) { Write-Host "!! $m"; exit 1 }

# ---- checks ----
if ($Runs -lt 1) { Fail "the number of runs must be a positive integer" }
foreach ($s in $Scenarios) { if ($s -notin "quotes", "animation", "interaction", "compute") { Fail "unknown scenario: $s" } }
$Impl = @{
    rust   = @{ name = "rust-gpui"; exe = "target\release\bench-rust.exe"; mode = "tsx" }
    nsh    = @{ name = "nsh-tsx"; exe = "target-nsh\release\bench-nsh.exe"; mode = "tsx" }
    hybrid = @{ name = "nsh-hybrid"; exe = "target-nsh\release\bench-nsh.exe"; mode = "hybrid" }
    rustui = @{ name = "nsh-rust"; exe = "target-nsh\release\bench-nsh.exe"; mode = "rustui" }
}
foreach ($i in $Impls) {
    if (-not $Impl.ContainsKey($i)) { Fail "unknown implementation: $i (rust nsh hybrid rustui)" }
    if (-not (Test-Path $Impl[$i].exe)) { Fail "missing $($Impl[$i].exe): run scripts\setup-windows.ps1 first" }
}
if (-not (Test-Path nsh-app\out\app\bundle.nshc)) { Fail "missing nsh-app\out\app\bundle.nshc: run scripts\setup-windows.ps1 first" }
$pmCmd = Get-Command $PresentMon -ErrorAction SilentlyContinue
if (-not $pmCmd) { Fail "PresentMon not found ($PresentMon): pass -PresentMon <path to PresentMon.exe>" }
$PresentMon = $pmCmd.Source
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) { Fail "PresentMon needs an elevated (administrator) PowerShell" }
$explorer = Get-Process explorer -IncludeUserName -ErrorAction SilentlyContinue | Where-Object { $_.UserName -like "*\$User" } | Select-Object -First 1
if (-not $explorer) { Fail "user $User is not logged on to an interactive desktop" }
Add-Type -Namespace NshPerfBench -Name Wts -MemberDefinition '[DllImport("kernel32.dll")] public static extern uint WTSGetActiveConsoleSessionId();'
if ($explorer.SessionId -ne [NshPerfBench.Wts]::WTSGetActiveConsoleSessionId()) {
    Fail "the desktop of $User is not on the console (Remote Desktop?): sign in at the console and keep Remote Desktop disconnected"
}
if (Get-Command py -ErrorAction SilentlyContinue) { $PyExe = "py"; $PyPre = @("-3") } else { $PyExe = "python"; $PyPre = @() }

# ---- results directory, metadata, environment ----
$ts = Get-Date -Format "yyyyMMdd-HHmmss"
$Out = "results\" + $(if ($Quick) { "quick-" } else { "" }) + $ts
if (Test-Path $Out) { Fail "$Out exists" }
$Jobs = Join-Path $Out "jobs"
New-Item -ItemType Directory -Force $Jobs, (Join-Path $Out "presentmon") | Out-Null
@'
import hashlib, json, os, pathlib, platform, subprocess, sys
out, count, scenarios, impls = sys.argv[1:]
def command(*args):
    p = subprocess.run(args, text=True, capture_output=True)
    return p.stdout.strip() if p.returncode == 0 else 'unavailable'
files = ['Cargo.lock', 'nsh-app/Cargo.lock', 'patches/nsh-001-reset-runaway-count.patch', 'patches/nsh-002-perf-diagnostics.patch',
         'patches/nsh-003-windows-diagnostics.patch']
if 'rust' in impls.split(): files.append('target/release/bench-rust.exe')
if set(impls.split()) - {'rust'}: files += ['target-nsh/release/bench-nsh.exe', 'nsh-app/out/app/bundle.nshc']
meta = dict(platform=platform.platform(), system=platform.system(), memory_supported=True,
            repetitions=int(count), scenarios=scenarios.split(), implementations=impls.split(),
            quick='BENCH_QUICK' in os.environ, nsh_backend='native', source_commit=command('git', 'rev-parse', 'HEAD'),
            source_dirty=bool(command('git', 'status', '--porcelain')),
            nsh_commit=command('git', '-C', 'vendor/nsh', 'rev-parse', 'HEAD'),
            rustc=command('rustc', '--version'), cargo=command('cargo', '--version'),
            sha256={f: hashlib.sha256(pathlib.Path(f).read_bytes()).hexdigest() for f in files})
pathlib.Path(out, 'metadata.json').write_text(json.dumps(meta, indent=2) + '\n')
'@ | & $PyExe @PyPre - $Out $Runs ($Scenarios -join " ") ($Impls -join " ")
if ($LASTEXITCODE -ne 0) { Fail "cannot write metadata.json" }
& {
    "Windows"
    $os = Get-CimInstance Win32_OperatingSystem
    "os: $($os.Caption) $($os.Version)"
    "cpu: " + (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
    "ram_gb: " + [math]::Round($os.TotalVisibleMemorySize / 1MB, 1)
    foreach ($g in Get-CimInstance Win32_VideoController) {
        "gpu: $($g.Name) driver $($g.DriverVersion) $($g.CurrentHorizontalResolution)x$($g.CurrentVerticalResolution) @ $($g.CurrentRefreshRate)Hz"
    }
    $bat = Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue
    if ($bat) { "battery: status $($bat.BatteryStatus) (2 = on AC), $($bat.EstimatedChargeRemaining)%" } else { "battery: none" }
    "power scheme: " + ((powercfg /getactivescheme) -replace "[^\x20-\x7E]", "?")
    "presentmon: $PresentMon " + (Get-Item $PresentMon).VersionInfo.ProductVersion
} | Set-Content -Encoding UTF8 (Join-Path $Out "env.txt")

# ---- scheduled task: interactive, only while $User is logged on; its action runs jobs\current.ps1 ----
$Current = (Resolve-Path $Jobs).Path + "\current.ps1"
Set-Content -Encoding ASCII $Current "exit 0"
$tr = "powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -File $Current"
if ($tr.Length -gt 261) { Fail "task command too long ($($tr.Length) characters): use a shorter checkout path" }
cmd /c "schtasks /create /tn $TaskName /tr `"$tr`" /sc once /st 00:00 /ru $User /it /f >nul 2>&1"
if ($LASTEXITCODE -ne 0) { Fail "schtasks /create failed ($LASTEXITCODE)" }

function Stop-Capture($pm) {
    cmd /c "`"$PresentMon`" --terminate_existing_session --session_name $TaskName >nul 2>&1"
    if ($pm -and -not $pm.WaitForExit(15000)) { Stop-Process -Id $pm.Id -Force -ErrorAction SilentlyContinue }
}

# One benchmark process; returns an error message, or nothing when it produced a joined result
function Run-One([string]$s, [int]$run, [string]$key) {
    $im = $Impl[$key]
    $runDir = (Resolve-Path $Out).Path + "\run$run"
    New-Item -ItemType Directory -Force $runDir | Out-Null
    $tag = "run$run-$s-$key"
    $log = (Resolve-Path $Out).Path + "\$tag.log"
    $exitFile = (Resolve-Path $Jobs).Path + "\$tag.exit"
    $csv = (Resolve-Path $Out).Path + "\presentmon\$tag.csv"
    $exe = (Resolve-Path $im.exe).Path
    $exeName = Split-Path -Leaf $exe
    $bundle = (Resolve-Path nsh-app\out\app).Path
    $argLine = if ($key -eq "rust") { "" } else { "`$a.ArgumentList = '`"$bundle`"'" }
    # Runs on the interactive desktop: environment, start the benchmark (its own window shows normally), wait, exit code
    $job = @"
`$ErrorActionPreference = 'Continue'
Set-Location '$Root'
`$env:BENCH_BACKEND = 'native'
`$env:BENCH_MODE = '$($im.mode)'
`$env:BENCH_SCENARIO = '$s'
`$env:BENCH_OUT = '$runDir'
$(if ($Quick) { "`$env:BENCH_QUICK = '1'" } else { "Remove-Item Env:BENCH_QUICK -ErrorAction SilentlyContinue" })
# Minimize the other windows so that nothing covers the benchmark window
try { (New-Object -ComObject Shell.Application).MinimizeAll(); Start-Sleep -Milliseconds 800 } catch { }
`$code = -1
try {
    `$a = @{ FilePath = '$exe'; NoNewWindow = `$true; Wait = `$true; PassThru = `$true; RedirectStandardError = '$log'; RedirectStandardOutput = '$log.out' }
    $argLine
    `$code = (Start-Process @a).ExitCode
} catch { `$_ | Out-File -Encoding UTF8 '$log.launch' }
Set-Content -Encoding ASCII '$exitFile' "`$code"
"@
    Set-Content -Encoding UTF8 $Current $job
    Remove-Item $exitFile, $csv -ErrorAction SilentlyContinue

    Stop-Capture $null
    $pmArgs = "--process_name $exeName --output_file `"$csv`" --qpc_time --no_console_stats --session_name $TaskName --stop_existing_session"
    $pm = Start-Process -FilePath $PresentMon -ArgumentList $pmArgs -NoNewWindow -PassThru `
        -RedirectStandardOutput "$csv.log" -RedirectStandardError "$csv.err"
    Start-Sleep -Seconds 2
    $t0 = Get-Date
    cmd /c "schtasks /run /tn $TaskName >nul 2>&1"
    if ($LASTEXITCODE -ne 0) { Stop-Capture $pm; return "schtasks /run failed ($LASTEXITCODE)" }
    # Same limit as run.sh: 180 seconds per process
    while (-not (Test-Path $exitFile)) {
        if (((Get-Date) - $t0).TotalSeconds -gt 180) {
            Get-Process ([IO.Path]::GetFileNameWithoutExtension($exeName)) -ErrorAction SilentlyContinue | Stop-Process -Force
            Stop-Capture $pm
            return "benchmark timed out after 180 seconds, see $log"
        }
        Start-Sleep -Milliseconds 500
    }
    Start-Sleep -Seconds 1
    Stop-Capture $pm
    $code = (Get-Content $exitFile -Raw).Trim()
    if ($code -ne "0") { return "exit code $code, see $log" }
    $json = "$runDir\$($im.name)-$s.json"
    if (-not (Test-Path $json)) { return "missing result $($im.name)-$s, see $log" }
    # Write-Host: the function's output is its error message
    & $PyExe @PyPre scripts\presentmon_join.py $json $csv 2>&1 | ForEach-Object { Write-Host "   $_" }
    if ($LASTEXITCODE -ne 0) { return "PresentMon recorded no frames of $exeName ($csv)" }
}

$failed = $null
try {
    :all foreach ($s in $Scenarios) {
        for ($i = 1; $i -le $Runs; $i++) {
            foreach ($key in $Impls) {
                Write-Host "== $s run $i / $key"
                $err = Run-One $s $i $key
                if ($err) { $failed = $err; break all }
                Start-Sleep -Seconds 2
            }
        }
    }
} finally {
    cmd /c "schtasks /delete /tn $TaskName /f >nul 2>&1"
}
if ($failed) { Fail "test failed: $failed" }
& $PyExe @PyPre scripts\report.py $Out
