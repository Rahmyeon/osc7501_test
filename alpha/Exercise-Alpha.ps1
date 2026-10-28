param(
    [ValidateSet('Demo', 'Smoke', 'Shell')]
    [string]$Mode = 'Demo'
)
$ErrorActionPreference = 'Stop'
$escapeByte = [char]27

function global:Send-Status([string]$Body, [switch]$Bell) {
    $statusEscape = [char]27
    $terminator = if ($Bell) { [string][char]7 } else { "$statusEscape\" }
    [Console]::Write("$statusEscape]7501;$Body$terminator")
}

function global:Encode-Text([string]$Text) {
    [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($Text))
}

function Pause-Demo([string]$Message) {
    Write-Host "`n$Message" -ForegroundColor Cyan
    Write-Host 'Check the tab. Ctrl+Shift+U inspects the records; Esc returns here.'
    [void](Read-Host 'Press Enter for the next step')
}

function Assert-Snapshot([string]$Case, [array]$Expected, [switch]$AfterExit) {
    $request = @{ case = $Case; expected = @($Expected); complete = [bool]$AfterExit } | ConvertTo-Json -Depth 10 -Compress
    $encoded = Encode-Text $request
    $checkFrame = "$escapeByte]1337;SetUserVar=osc7501_alpha_check=$encoded$escapeByte\"
    [Console]::Write($checkFrame)
    if ($AfterExit) { return }
    $resultPath = Join-Path $env:OSC7501_ALPHA_RESULT_DIR "$Case.json"
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    $retryAt = [DateTime]::UtcNow.AddMilliseconds(500)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $resultPath) {
            try { $result = Get-Content -Raw -LiteralPath $resultPath | ConvertFrom-Json }
            catch { Start-Sleep -Milliseconds 50; continue }
            if (-not $result.ok) { throw "$Case failed: $($result.problem). Actual: $($result.actual | ConvertTo-Json -Depth 8 -Compress)" }
            Write-Host "PASS $Case" -ForegroundColor Green
            return
        }
        # The shell can start before the window subscribes to user-var events.
        # Retry the test request only; task reports need no heartbeat or replay.
        if ([DateTime]::UtcNow -ge $retryAt) {
            [Console]::Write($checkFrame)
            $retryAt = [DateTime]::UtcNow.AddMilliseconds(500)
        }
        Start-Sleep -Milliseconds 50
    }
    throw "Timed out checking $Case. Confirm that this is the alpha build with alpha.lua loaded."
}

function Want([hashtable]$Values, [string[]]$Absent = @()) {
    @{ values = $Values; absent = @($Absent) }
}

if ($Mode -eq 'Shell') {
    Write-Host 'OSC 7501 alpha shell. Use Send-Status "state=working:progress=42".'
    Write-Host 'Use Send-Status "state=clear" to remove all records.'
    return
}

if ($Mode -eq 'Demo') {
    Write-Host 'OSC 7501 alpha guided demo' -ForegroundColor Yellow
    Send-Status 'state=clear'
    Send-Status "state=working:app=demo:progress=15:msg=$(Encode-Text 'Compiling the application')"
    Pause-Demo '1. The root task is working at 15%.'
    Send-Status "state=blocked:id=build/tests:kind=permission:title=$(Encode-Text 'Integration tests'):msg=$(Encode-Text 'Approve access to the test database?')"
    Pause-Demo '2. The child is blocked on permission. Root and child both exist.'
    Send-Status 'state=working:id=build/tests:progress=65'
    Pause-Demo '3. The child is working. Its previous kind, title and message were replaced.'
    Send-Status 'state=clear:id=build'
    Pause-Demo '4. Clearing build removes build/tests. The root remains.'
    Send-Status "state=done:app=demo:msg=$(Encode-Text 'Build finished successfully')"
    Pause-Demo '5. Done survives a new shell prompt.'
    [Console]::Write("$escapeByte]133;A$escapeByte\")
    Pause-Demo '6. Done is still present after OSC 133 A.'
    Send-Status 'state=error:id=deploy:app=demo'
    Pause-Demo '7. Error takes priority over done in this example formatter.'
    Send-Status 'state=clear'
    Write-Host "`n8. All records cleared. The demo is complete."
    Write-Host 'This shell will remain available for your own Send-Status commands.'
    return
}

if (-not $env:OSC7501_ALPHA_RESULT_DIR) { throw 'Launch Smoke mode through Start-Alpha.ps1.' }
try {
    Write-Host 'Testing the native parser -> model -> pane -> Lua -> title snapshot path.'
    Send-Status 'state=clear'
    Assert-Snapshot '01-empty' @()

    Send-Status 'state=working:app=cargo:progress=42'
    $root = Want @{ id = ''; state = 'working'; app = 'cargo'; effective_app = 'cargo'; progress = 42 }
    Assert-Snapshot '02-working' @($root)

    Send-Status 'state=blocked:id=build/tests:kind=permission'
    $child = Want @{ id = 'build/tests'; state = 'blocked'; kind = 'permission'; effective_app = 'cargo' } @('app', 'progress')
    Assert-Snapshot '03-child-inherits' @($root, $child)

    Send-Status 'state=error:id=bad//id'
    Assert-Snapshot '04-invalid-is-atomic' @($root, $child)

    Send-Status 'state=done:id=build/tests' -Bell
    $done = Want @{ id = 'build/tests'; state = 'done'; effective_app = 'cargo' } @('kind', 'progress', 'app', 'msg', 'title')
    Assert-Snapshot '05-replace-and-bell' @($root, $done)

    [Console]::Write("$escapeByte[!p")
    [Console]::Write("$escapeByte[?1049h$escapeByte[?1049l")
    Assert-Snapshot '06-soft-reset-and-alt' @($root, $done)

    Send-Status 'state=working:id=builder'
    Send-Status 'state=clear:id=build'
    $builder = Want @{ id = 'builder'; state = 'working'; effective_app = 'cargo' }
    Assert-Snapshot '07-clear-boundary' @($root, $builder)

    Send-Status 'state=error:id=result'
    [Console]::Write("$escapeByte]133;A$escapeByte\")
    Assert-Snapshot '08-prompt-cleanup' @((Want @{ id = 'result'; state = 'error' } @('effective_app')))

    [Console]::Write("$escapeByte" + 'c')
    Assert-Snapshot '09-full-reset' @()

    # A Lua mutation affects its copy, not the authoritative model. The native
    # snapshot test covers mutation directly; this checks fresh API reads too.
    Send-Status 'state=done:id=complete:app=smoke'
    $finished = Want @{ id = 'complete'; state = 'done'; app = 'smoke'; effective_app = 'smoke' }
    Assert-Snapshot '10-finished-record' @($finished)
    Send-Status 'state=working:id=exit-working'
    Send-Status 'state=blocked:id=exit-blocked:kind=auth'
    Write-Host 'First 10 checks passed. Exiting the attached process to test final cleanup.' -ForegroundColor Green
    Assert-Snapshot '11-attached-process-exit' @($finished) -AfterExit
    exit 0
}
catch {
    Write-Host $_ -ForegroundColor Red
    @{ ok = $false; error = [string]$_ } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $env:OSC7501_ALPHA_RESULT_DIR 'complete.json')
    exit 1
}
