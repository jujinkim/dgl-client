param(
    [switch]$CheckOnly,
    [switch]$Test
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
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path $vswhere)) { throw 'Install Visual Studio 2022 Build Tools with desktop C++, Windows SDK and CMake. See docs/WINDOWS.md.' }
$vsPath = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0 -or -not $vsPath) { throw 'Visual C++ x64 build tools were not found.' }
Import-Module (Join-Path $vsPath 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll')
Enter-VsDevShell -VsInstallPath $vsPath -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null

if (-not (Get-Command cmake.exe -ErrorAction SilentlyContinue)) {
    $cmakeBin = Join-Path $vsPath 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin'
    if (Test-Path $cmakeBin) { $env:PATH = "$cmakeBin;$env:PATH" }
}
foreach ($tool in @('git.exe', 'rustup.exe', 'cargo.exe', 'cl.exe', 'rc.exe', 'cmake.exe')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "Missing $tool. See docs/WINDOWS.md." }
}
$fxc = Get-Command fxc.exe -ErrorAction SilentlyContinue
if (-not $fxc) { throw 'Windows SDK fxc.exe was not found in the developer environment. Install the Windows 11 SDK.' }
$env:GPUI_FXC_PATH = $fxc.Source
$python = Get-Command python.exe -ErrorAction SilentlyContinue
if (-not $python) { throw 'Install Python 3.9+ with python.exe on PATH.' }
Invoke-Checked -Executable $python.Source -Arguments @('-c', 'import sys; sys.exit(sys.version_info < (3, 9))')
Write-Host "MSVC: $vsPath"
Write-Host "HLSL compiler: $env:GPUI_FXC_PATH"
if ($CheckOnly) {
    Invoke-Checked -Executable rustup -Arguments @('run', $toolchain, 'rustc', '--version')
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
} finally {
    $env:RUSTUP_TOOLCHAIN = $previousToolchain
    Pop-Location
}
