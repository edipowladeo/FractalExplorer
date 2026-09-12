$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repositoryPath = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location -LiteralPath $repositoryPath
$env:TERM = 'xterm-256color'

$codex = Get-Command codex.exe -ErrorAction Stop
& $codex.Source
