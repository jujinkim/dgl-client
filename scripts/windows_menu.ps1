$ErrorActionPreference = 'Stop'
while ($true) {
    Write-Host "`n=== dgl Windows ==="
    Write-Host '1. Build and run'
    Write-Host '2. Build ZIP only'
    Write-Host '3. Build, test and run'
    Write-Host '4. Check all build tools and available updates'
    Write-Host '5. Install / update build tools'
    Write-Host '0. Exit'
    $choice = Read-Host 'Select'
    if ($choice -eq '0') { exit 0 }
    try {
        switch ($choice) {
            '1' { & "$PSScriptRoot/build_windows.ps1" -Run }
            '2' { & "$PSScriptRoot/build_windows.ps1" }
            '3' { & "$PSScriptRoot/build_windows.ps1" -Test -Run }
            '4' { & "$PSScriptRoot/windows_tools.ps1" }
            '5' { & "$PSScriptRoot/windows_tools.ps1" -Install }
            default { Write-Host 'Choose 0-5.' }
        }
    } catch { Write-Host $_.Exception.Message -ForegroundColor Red }
}
