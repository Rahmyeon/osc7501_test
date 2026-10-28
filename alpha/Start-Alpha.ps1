param(
    [ValidateSet('Demo', 'Smoke', 'Shell')]
    [string]$Mode = 'Demo',
    [switch]$Wait,
    [switch]$CloseAfterTest
)
$ErrorActionPreference = 'Stop'
$alphaExecutable = Join-Path $PSScriptRoot 'bin/wezterm-gui.exe'
if (-not (Test-Path -LiteralPath $alphaExecutable)) {
    throw 'The alpha binary is missing. Build and package it using the instructions in README.md.'
}
$shellCommand = Get-Command pwsh.exe -ErrorAction SilentlyContinue
if (-not $shellCommand) { $shellCommand = Get-Command powershell.exe }
$configPath = Join-Path $PSScriptRoot 'alpha.lua'
$exercisePath = Join-Path $PSScriptRoot 'Exercise-Alpha.ps1'
$runDirectory = Join-Path $PSScriptRoot ('test-results/' + [Guid]::NewGuid().ToString('N'))
if ($Mode -eq 'Smoke') { [void](New-Item -ItemType Directory -Path $runDirectory -Force) }
$oldResultDirectory = $env:OSC7501_ALPHA_RESULT_DIR
try {
    if ($Mode -eq 'Smoke') { $env:OSC7501_ALPHA_RESULT_DIR = $runDirectory }
    else { Remove-Item Env:OSC7501_ALPHA_RESULT_DIR -ErrorAction SilentlyContinue }
    $arguments = @('--config-file', $configPath, 'start', '--always-new-process', '--no-auto-connect', '--cwd', $PSScriptRoot,
        '--', $shellCommand.Source, '-NoLogo', '-NoProfile', '-ExecutionPolicy', 'Bypass')
    if ($Mode -ne 'Smoke') { $arguments += '-NoExit' }
    $arguments += @('-File', $exercisePath, '-Mode', $Mode)
    # All arguments are fixed local paths/options. Quote spaces for Windows argv.
    $quotedArguments = ($arguments | ForEach-Object { '"' + $_.Replace('"', '\"') + '"' }) -join ' '
    # Interactive launches need a visible window. Only unattended smoke checks
    # that explicitly close themselves should request a hidden startup window.
    $launchWindowStyle = if ($Mode -eq 'Smoke' -and $CloseAfterTest) { 'Hidden' } else { 'Normal' }
    $process = Start-Process -FilePath $alphaExecutable -ArgumentList $quotedArguments -WorkingDirectory $PSScriptRoot -WindowStyle $launchWindowStyle -PassThru
}
finally {
    if ($null -eq $oldResultDirectory) { Remove-Item Env:OSC7501_ALPHA_RESULT_DIR -ErrorAction SilentlyContinue }
    else { $env:OSC7501_ALPHA_RESULT_DIR = $oldResultDirectory }
}
Write-Host "Started OSC 7501 alpha, process $($process.Id). Your normal config was not loaded."
if ($Mode -eq 'Smoke') { Write-Host "Results: $runDirectory" }
if ($Wait -and $Mode -eq 'Smoke') {
    try {
        $completionPath = Join-Path $runDirectory 'complete.json'
        $deadline = [DateTime]::UtcNow.AddSeconds(180)
        while (-not (Test-Path -LiteralPath $completionPath)) {
            if ($process.HasExited) { throw "Alpha exited before completing checks, exit code $($process.ExitCode)." }
            if ([DateTime]::UtcNow -gt $deadline) { throw 'Alpha checks timed out. Inspect the alpha window and log.' }
            Start-Sleep -Milliseconds 200
        }
        Start-Sleep -Milliseconds 100
        $result = Get-Content -Raw -LiteralPath $completionPath | ConvertFrom-Json
        if (-not $result.ok) { throw "Alpha checks failed: $($result.error)" }
        Write-Host "PASS: $($result.checks) native GUI/Lua checks." -ForegroundColor Green
    }
    finally {
        if ($CloseAfterTest -and -not $process.HasExited) { Stop-Process -Id $process.Id }
    }
}
