param([switch]$Install)
$ErrorActionPreference = 'Stop'
$results = [System.Collections.Generic.List[object]]::new()
function Invoke-ToolTask {
    param([string]$Name, [scriptblock]$Action)
    Write-Host "`n--- $Name ---"
    try {
        & $Action
        $results.Add([pscustomobject]@{ Task = $Name; Result = 'OK' })
    } catch {
        $results.Add([pscustomobject]@{ Task = $Name; Result = $_.Exception.Message })
        Write-Host $_.Exception.Message -ForegroundColor Red
    }
}
$packages = @('Git.Git', 'Python.Python.3.13', 'Rustlang.Rustup', 'Microsoft.VisualStudio.2022.BuildTools')
foreach ($package in $packages) {
    Invoke-ToolTask $package {
        if (-not (Get-Command winget.exe -ErrorAction SilentlyContinue)) {
            throw 'winget unavailable. Install/update App Installer from Microsoft Store, or install this tool manually.'
        }
        $arguments = @('list', '--id', $package, '--exact', '--upgrade-available', '--source', 'winget', '--accept-source-agreements')
        if ($Install) {
            $arguments = @('install', '--id', $package, '--exact', '--source', 'winget', '--accept-source-agreements', '--accept-package-agreements')
            if ($package -eq 'Microsoft.VisualStudio.2022.BuildTools') {
                $config = Join-Path (Split-Path $PSScriptRoot -Parent) 'desktop\windows.vsconfig'
                $arguments += @('--override', "--passive --wait --norestart --config `"$config`"")
            }
        }
        & winget.exe @arguments
        # Winget uses these codes for no matching package / no applicable update.
        if ($LASTEXITCODE -eq -1978335189) {
            Write-Host 'No applicable update (or package not installed); see local prerequisite checks below.'
        } elseif ($LASTEXITCODE -eq -1978335212 -and -not $Install) {
            Write-Host 'No package with an available update matched; see local prerequisite checks below.'
        } elseif ($LASTEXITCODE -notin @(0, 3010)) {
            throw "winget exited with code $LASTEXITCODE. Review output above."
        }
    }
}
if ($Install) {
    # Pick up PATH entries added by installers without discarding this session's entries.
    $env:PATH = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
        [Environment]::GetEnvironmentVariable('Path', 'User') + ';' + $env:PATH
    Invoke-ToolTask 'Pinned Rust MSVC toolchain and target' {
        & rustup toolchain install 1.98.1-x86_64-pc-windows-msvc --profile minimal --target x86_64-pc-windows-msvc
        if ($LASTEXITCODE -ne 0) { throw "rustup exited with code $LASTEXITCODE" }
    }
}
if (-not $Install) {
    Invoke-ToolTask 'Rust toolchain update check (project remains pinned to 1.98.1)' {
        & rustup check
        if ($LASTEXITCODE -ne 0) { throw "rustup check exited with code $LASTEXITCODE" }
    }
}
Invoke-ToolTask 'All local prerequisites' { & "$PSScriptRoot/build_windows.ps1" -CheckOnly }
Write-Host "`n=== All tool tasks completed ==="
$results | Format-Table -AutoSize -Wrap | Out-Host
if ($Install) { Write-Host 'If PATH changes or a restart are required, reopen this menu after restarting the terminal/Windows. Existing VS installations may need windows.vsconfig imported in Visual Studio Installer to add missing components.' }
if (@($results | Where-Object Result -ne 'OK').Count) { throw 'Some tool tasks need attention. All independent tasks were attempted.' }
