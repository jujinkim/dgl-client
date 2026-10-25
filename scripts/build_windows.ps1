param(
    [switch]$CheckOnly,
    [switch]$Test,
    [switch]$Run
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ([Environment]::OSVersion.Platform -ne 'Win32NT') {
    throw 'Native Windows x64 is required. WSL builds Linux binaries.'
}

function Invoke-Checked {
    param([string]$Executable, [string[]]$Arguments)
    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Executable failed with exit code $LASTEXITCODE" }
}

$projectRoot = Split-Path -Parent $PSScriptRoot
$toolchain = '1.98.1-x86_64-pc-windows-msvc'
$problems = [System.Collections.Generic.List[string]]::new()
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vsPath = $null
try {
if (-not (Test-Path $vswhere)) { throw 'Install Visual Studio 2022 Build Tools with desktop C++, Windows SDK and CMake. See docs/WINDOWS.md.' }
$vsPath = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0 -or -not $vsPath) { throw 'Visual C++ x64 build tools were not found.' }
Import-Module (Join-Path $vsPath 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll')
Enter-VsDevShell -VsInstallPath $vsPath -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null
} catch { $problems.Add("Visual Studio environment: $($_.Exception.Message)") }

if ($vsPath -and -not (Get-Command cmake.exe -ErrorAction SilentlyContinue)) {
    $cmakeBin = Join-Path $vsPath 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin'
    if (Test-Path $cmakeBin) { $env:PATH = "$cmakeBin;$env:PATH" }
}
foreach ($tool in @('git.exe', 'rustup.exe', 'cargo.exe', 'cl.exe', 'rc.exe', 'cmake.exe')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { $problems.Add("Missing $tool. See docs/WINDOWS.md.") }
    else { Write-Host "[OK] $tool" }
}
$fxc = Get-Command fxc.exe -ErrorAction SilentlyContinue
if (-not $fxc) { $problems.Add('Windows SDK fxc.exe was not found. Install the Windows 11 SDK.') }
else { $env:GPUI_FXC_PATH = $fxc.Source; Write-Host "[OK] fxc.exe: $($fxc.Source)" }
$python = Get-Command python.exe -ErrorAction SilentlyContinue
if (-not $python) { $problems.Add('Install Python 3.9+ with python.exe on PATH.') }
else {
    try {
        Invoke-Checked -Executable $python.Source -Arguments @('-c', 'import sys; print(sys.version); sys.exit(sys.version_info < (3, 9))')
    } catch { $problems.Add("Python 3.9+: $($_.Exception.Message)") }
}
Write-Host "MSVC: $vsPath"
if ($CheckOnly) {
    foreach ($arguments in @(
        @('run', $toolchain, 'rustc', '--version'),
        @('run', $toolchain, 'cargo', '--version')
    )) {
        try { Invoke-Checked -Executable rustup -Arguments $arguments }
        catch { $problems.Add("Rust ($arguments): $($_.Exception.Message)") }
    }
    try {
        $targets = & rustup target list --installed --toolchain $toolchain
        if ($LASTEXITCODE -ne 0 -or 'x86_64-pc-windows-msvc' -notin $targets) {
            throw 'Required x86_64-pc-windows-msvc target is not installed.'
        }
        Write-Host '[OK] Rust Windows MSVC target'
    } catch { $problems.Add("Rust target: $($_.Exception.Message)") }
}
if ($problems.Count) {
    Write-Host "`nPrerequisite problems ($($problems.Count)):" -ForegroundColor Red
    foreach ($problem in $problems) { Write-Host " - $problem" -ForegroundColor Red }
    throw 'All prerequisite checks completed. Fix the listed problems before building.'
}
if ($CheckOnly) {
    Write-Host 'Windows build prerequisites found. No app build or runtime test performed.'
    return
}

$previousToolchain = $env:RUSTUP_TOOLCHAIN
Push-Location $projectRoot
try {
    Invoke-Checked -Executable rustup -Arguments @('toolchain', 'install', $toolchain, '--profile', 'minimal', '--target', 'x86_64-pc-windows-msvc')
    $env:RUSTUP_TOOLCHAIN = $toolchain
    Invoke-Checked -Executable $python.Source -Arguments @('scripts/package_rust_windows.py')
    if ($Test) {
        Invoke-Checked -Executable cargo -Arguments @('test', '--locked', '--target', 'x86_64-pc-windows-msvc', '-p', 'dgl-core', '-p', 'dgl-uniffi', '-p', 'dgl-desktop')
    }
    if ($Run) {
        $metadataJson = & cargo metadata --locked --no-deps --format-version 1
        if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed.' }
        $metadata = $metadataJson | ConvertFrom-Json
        $executable = Join-Path $metadata.target_directory 'x86_64-pc-windows-msvc\release\dgl-desktop.exe'
        Start-Process -FilePath $executable -WorkingDirectory $projectRoot
    }
} finally {
    $env:RUSTUP_TOOLCHAIN = $previousToolchain
    Pop-Location
}
