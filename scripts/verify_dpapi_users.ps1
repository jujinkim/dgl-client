param([string]$Output = 'artifacts/windows-dpapi.json')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
$outputPath = Join-Path $root $Output
$report = @{ schema = 1; complete = $false; status = 'failed'; checks = @{} }
$context = (& python -c 'import sys,json; sys.path.insert(0,"scripts"); from evidence import context; print(json.dumps(context()))') | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw 'Cannot get evidence context' }
$report.context = $context
New-Item (Split-Path $outputPath) -ItemType Directory -Force | Out-Null
function Save-Report { $report | ConvertTo-Json -Depth 8 | Set-Content $outputPath -Encoding utf8NoBOM }
Save-Report
$suffix = [Guid]::NewGuid().ToString('N').Substring(0, 10)
$work = Join-Path $env:ProgramData "dgl-dpapi-$suffix"
$accounts = @()
$children = [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
$cleanupOk = $true
try {
    if (-not $env:GITHUB_ACTIONS -or $env:RUNNER_OS -ne 'Windows') { throw 'Disposable Windows Actions runner required' }
    New-Item $work -ItemType Directory | Out-Null
    Copy-Item (Join-Path $root 'app/build/dpapi-probe') (Join-Path $work 'harness') -Recurse
    # Do not rely on access to the runner account's JDK or Gradle cache.
    Copy-Item $env:JAVA_HOME (Join-Path $work 'jdk') -Recurse
    foreach ($letter in @('a', 'b')) {
        $name = "dgl${letter}$suffix"
        $password = ConvertTo-SecureString ('Dgl!9a' + [Guid]::NewGuid().ToString('N')) -AsPlainText -Force
        $user = New-LocalUser -Name $name -Password $password -AccountNeverExpires -PasswordNeverExpires
        $accounts += @{ name = $name; sid = $user.SID.Value; credential = [PSCredential]::new("$env:COMPUTERNAME\$name", $password) }
    }
    foreach ($account in $accounts) {
        & icacls.exe $work /grant "*$($account.sid):(OI)(CI)M" /T /Q | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Cannot grant shared fixture access' }
    }
    function Probe($account, $mode, $reference) {
        $tag = "$($account.name)-$mode-$reference"
        $stdout = Join-Path $work "$tag.stdout"
        $stderr = Join-Path $work "$tag.stderr"
        $arguments = '-Dfile.encoding=UTF-8 -cp "' + $work + '\harness;' + $work + '\harness\*" kim.jujin.rovessh.DpapiProbe ' + $mode + ' "' + $work + '\data" ' + $reference
        $process = Start-Process -FilePath (Join-Path $work 'jdk/bin/java.exe') -ArgumentList $arguments -Credential $account.credential -LoadUserProfile -WorkingDirectory $work -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
        $children.Add($process)
        if (-not $process.WaitForExit(60000)) { throw 'DPAPI child timeout' }
        $process.Refresh()
        if ($process.ExitCode -ne 0) { throw "DPAPI child failed: $mode" }
        return Get-Content $stdout -Raw -Encoding utf8
    }
    foreach ($i in 0, 1) {
        if ((Probe $accounts[$i] 'write' "user$i").Trim() -ne 'OWN_ROUNDTRIP_OK') { throw 'Own DPAPI round trip missing' }
    }
    $report.checks.ownRoundTrips = 2
    foreach ($i in 0, 1) {
        $foreign = 1 - $i
        $expected = (Get-FileHash (Join-Path $work "data/user$foreign.sealed") -Algorithm SHA256).Hash.ToLowerInvariant()
        $result = Probe $accounts[$i] 'deny' "user$foreign"
        if (-not $result.Contains("FOREIGN_READ_SHA256=$expected") -or -not $result.Contains('DPAPI_DENIED=13')) { throw 'Foreign read or DPAPI denial missing' }
    }
    $report.checks.foreignReadable = 2
    $report.checks.foreignDecryptDenied = 2
} finally {
    foreach ($child in $children) {
        try { if (-not $child.HasExited) { $child.Kill($true); $child.WaitForExit() }; $child.Dispose() } catch { $cleanupOk = $false }
    }
    foreach ($account in $accounts) {
        try {
            Get-CimInstance Win32_UserProfile -Filter "SID='$($account.sid)'" | Remove-CimInstance
            Remove-LocalUser -Name $account.name
            if (Get-LocalUser -Name $account.name -ErrorAction SilentlyContinue) { throw 'Account remains' }
            if (Get-CimInstance Win32_UserProfile -Filter "SID='$($account.sid)'") { throw 'Profile remains' }
        } catch { $cleanupOk = $false }
    }
    try { if (Test-Path $work) { Remove-Item $work -Recurse -Force }; if (Test-Path $work) { throw 'Fixture remains' } } catch { $cleanupOk = $false }
    $report.checks.cleanup = $cleanupOk
    Save-Report
}
if (-not $cleanupOk) { throw 'DPAPI cleanup failed' }
$report.complete = $true
$report.status = 'passed'
Save-Report
Write-Output 'DPAPI: two own round trips, two readable foreign blobs denied, cleanup passed'
