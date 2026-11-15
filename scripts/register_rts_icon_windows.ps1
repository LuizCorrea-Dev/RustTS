# Associa a extensão .rts ao ícone rustts.ico no Windows Explorer e editores nativos
$CandidatePaths = @(
    (Join-Path $PSScriptRoot "..\editors\vscode-rustts\icons\rustts.ico"),
    (Join-Path $PSScriptRoot "rustts.ico")
)
$ResolvedIcon = $CandidatePaths | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $ResolvedIcon) {
    Write-Error "Nao foi possivel localizar rustts.ico em editors\vscode-rustts\icons\."
    exit 1
}
$IconPath = (Resolve-Path $ResolvedIcon).Path

New-Item -Path "HKCU:\Software\Classes\.rts" -Force | Out-Null
Set-ItemProperty -Path "HKCU:\Software\Classes\.rts" -Name "(Default)" -Value "RustTS.SourceFile"
New-Item -Path "HKCU:\Software\Classes\RustTS.SourceFile" -Force | Out-Null
Set-ItemProperty -Path "HKCU:\Software\Classes\RustTS.SourceFile" -Name "(Default)" -Value "RustTS Source File"
New-Item -Path "HKCU:\Software\Classes\RustTS.SourceFile\DefaultIcon" -Force | Out-Null
Set-ItemProperty -Path "HKCU:\Software\Classes\RustTS.SourceFile\DefaultIcon" -Name "(Default)" -Value "`"$IconPath`",0"
Write-Host "[OK] Icone $IconPath associado a arquivos .rts no Windows Explorer!" -ForegroundColor Green
