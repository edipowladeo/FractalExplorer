$ErrorActionPreference = 'Stop'

$scriptPath = Join-Path $PSScriptRoot '..\scripts\launch-codex.ps1'

if (-not (Test-Path -LiteralPath $scriptPath)) {
    throw "Launcher ausente: $scriptPath"
}

$tokens = $null
$errors = $null
[System.Management.Automation.Language.Parser]::ParseFile(
    $scriptPath,
    [ref] $tokens,
    [ref] $errors
) | Out-Null

if ($errors.Count -gt 0) {
    $messages = $errors | ForEach-Object Message
    throw "Launcher contém erro de sintaxe: $($messages -join '; ')"
}

$content = Get-Content -LiteralPath $scriptPath -Raw
if ($content -notmatch 'Set-Location') {
    throw 'Launcher deve posicionar o Codex no diretório do repositório.'
}
if ($content -notmatch 'codex\.exe') {
    throw 'Launcher deve invocar codex.exe.'
}

Write-Output 'launch-codex.tests.ps1: PASS'
