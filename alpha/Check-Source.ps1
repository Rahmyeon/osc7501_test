$ErrorActionPreference = 'Stop'
$sourceDirectory = Split-Path $PSScriptRoot -Parent
$logDirectory = Join-Path $PSScriptRoot 'source-checks'
[void](New-Item -ItemType Directory -Path $logDirectory -Force)
$checks = @(
    @{ Name = 'vtparse'; Arguments = @('test', '-p', 'vtparse', '--lib', '--offline') },
    @{ Name = 'vtparse-heapless'; Arguments = @('check', '-p', 'vtparse', '--no-default-features', '--features', 'no_std', '--offline') },
    @{ Name = 'parser-no-std'; Arguments = @('test', '-p', 'wezterm-escape-parser', '--lib', '--offline') },
    @{ Name = 'parser-std'; Arguments = @('test', '-p', 'wezterm-escape-parser', '--lib', '--features', 'std', '--offline') },
    @{ Name = 'terminal'; Arguments = @('test', '-p', 'wezterm-term', '--lib', '--offline') },
    @{ Name = 'lua-contract'; Arguments = @('test', '-p', 'mux-lua', '--lib', 'program_status_lua_snapshot', '--offline') }
)
$results = @()
Push-Location $sourceDirectory
try {
    foreach ($check in $checks) {
        $arguments = $check.Arguments
        Write-Host "Running $($check.Name)" -ForegroundColor Cyan
        $logPath = Join-Path $logDirectory ($check.Name + '.log')
        & cargo @arguments *> $logPath
        $code = $LASTEXITCODE
        $summary = @(Get-Content -LiteralPath $logPath | Where-Object { $_ -match '^test result:|Finished ' })
        $results += @{ name = $check.Name; command = ('cargo ' + ($arguments -join ' ')); exit_code = $code; summary = $summary }
        $results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $logDirectory 'results.json')
        if ($code -ne 0) {
            Get-Content -LiteralPath $logPath -Tail 100
            throw "Source check failed: $($check.Name). Full log: $logPath"
        }
        $summary | Write-Output
    }
}
finally { Pop-Location }
Write-Host 'All source checks passed.' -ForegroundColor Green
