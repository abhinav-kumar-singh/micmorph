# MicMorph Windows 1-Click Fast Installer
$ErrorActionPreference = "Stop"
Write-Host "🎙️  Installing MicMorph for Windows..." -ForegroundColor Cyan

$tempExe = Join-Path $env:TEMP "MicMorph_0.1.0_x64-setup.exe"
$url = "https://micmorph.work/MicMorph_0.1.0_x64-setup.exe"

Write-Host "⬇️  Downloading latest MicMorph installer..." -ForegroundColor Gray
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $url -OutFile $tempExe -UseBasicParsing
} catch {
    Write-Host "❌ Failed to download installer automatically." -ForegroundColor Red
    Write-Host "Please download directly from https://micmorph.work/" -ForegroundColor Yellow
    exit 1
}

Write-Host "🚀 Launching MicMorph setup..." -ForegroundColor Green
Start-Process -FilePath $tempExe
