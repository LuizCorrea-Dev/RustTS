# scripts/install.ps1 — Instalador Oficial do Compilador RustTS (rtsc) para Windows x64
param(
    [string]$Repo = "LuizCorrea-Dev/RustTS"
)

$ErrorActionPreference = "Stop"

$InstallDir = "$env:USERPROFILE\.rtsc\bin"
$ExePath = "$InstallDir\rtsc.exe"

Write-Host "==> Instalando o compilador RustTS (rtsc)..." -ForegroundColor Cyan

# 1. Cria o diretório de instalação ~/.rtsc/bin
if (!(Test-Path -Path $InstallDir)) {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
}

# 2. Baixa o executável rtsc-windows-x64.exe mais recente do GitHub Releases
$ReleaseUrl = "https://github.com/$Repo/releases/latest/download/rtsc-windows-x64.exe"
Write-Host "==> Baixando binário nativo de $ReleaseUrl ..."
Invoke-WebRequest -Uri $ReleaseUrl -OutFile $ExePath

# 3. Adiciona ~/.rtsc/bin ao PATH do usuário no Windows (se ainda não estiver)
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ([string]::IsNullOrEmpty($UserPath)) {
    $NewPath = $InstallDir
} elseif ($UserPath -notlike "*$InstallDir*") {
    $NewPath = ($UserPath.TrimEnd(';') + ";" + $InstallDir)
} else {
    $NewPath = $null
}

if ($null -ne $NewPath) {
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    $env:Path += ";$InstallDir"
    Write-Host "==> Diretório '$InstallDir' adicionado ao PATH do usuário." -ForegroundColor Green
}

Write-Host "==> RustTS instalado com sucesso! Teste executando: rtsc --version" -ForegroundColor Green

