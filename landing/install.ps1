# MicMorph Windows 1-Click Fast Installer
$ErrorActionPreference = "Stop"
Write-Host "🎙️  Installing MicMorph for Windows..." -ForegroundColor Cyan

# 1. Check if virtual audio driver is installed
$vbcablePresent = $false
$capturePath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Capture"
if (Test-Path $capturePath) {
    Get-ChildItem $capturePath -ErrorAction SilentlyContinue | ForEach-Object {
        $props = Join-Path $_.PSPath "Properties"
        if (Test-Path $props) {
            $p = Get-ItemProperty -Path $props -ErrorAction SilentlyContinue
            $desc = $p."{a45c254e-df1c-4efd-8020-67d146a850e0},2"
            if ($desc -like "*VB-Audio*" -or $desc -like "*CABLE*") {
                $vbcablePresent = $true
            }
        }
    }
}

$tempDir = [System.IO.Path]::GetTempPath()

# Install driver if missing
if (-not $vbcablePresent) {
    Write-Host "🔌 Installing virtual audio bridge..." -ForegroundColor Gray
    $zipPath = Join-Path $tempDir "VBCABLE_Driver_Pack43.zip"
    $extractPath = Join-Path $tempDir "VBCABLE_Driver"
    
    try {
        if (-not (Test-Path $zipPath)) {
            Invoke-WebRequest -Uri "https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack43.zip" -OutFile $zipPath -UseBasicParsing
        }
        if (-not (Test-Path $extractPath)) {
            Expand-Archive -Path $zipPath -DestinationPath $extractPath -Force
        }
        $setupExe = Join-Path $extractPath "VBCABLE_Setup_x64.exe"
        if (-not (Test-Path $setupExe)) {
            $setupExe = Join-Path $extractPath "VBCABLE_Setup.exe"
        }
        Start-Process -FilePath $setupExe -ArgumentList "-i", "-h" -Verb RunAs -Wait
    } catch {
        Write-Host "⚠️ Driver auto-install skipped (can be configured inside app)." -ForegroundColor Yellow
    }
}

# 2. Rename audio endpoints to "MicMorph"
Write-Host "🔧 Configuring microphone as 'MicMorph'..." -ForegroundColor Gray
$renameScript = @'
$renderPath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render"
$capturePath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Capture"

$renamed = $false
foreach ($base in @($renderPath, $capturePath)) {
    if (Test-Path $base) {
        Get-ChildItem $base -ErrorAction SilentlyContinue | ForEach-Object {
            $props = Join-Path $_.PSPath "Properties"
            if (Test-Path $props) {
                $p = Get-ItemProperty -Path $props -ErrorAction SilentlyContinue
                $desc = $p."{a45c254e-df1c-4efd-8020-67d146a850e0},2"
                if ($desc -like "*VB-Audio*" -or $desc -like "*CABLE*") {
                    Set-ItemProperty -Path $props -Name "{b3f8fa53-0004-438e-9003-51a46e139bfc},6" -Value "MicMorph" -Force -ErrorAction SilentlyContinue
                    $renamed = $true
                }
            }
        }
    }
}
if ($renamed) {
    Restart-Service audiosrv -Force -ErrorAction SilentlyContinue
}
'@

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if ($isAdmin) {
    Invoke-Expression $renameScript
} else {
    try {
        Start-Process powershell -Verb RunAs -Wait -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", $renameScript
    } catch {
        # Fallback if user cancels elevation
    }
}

# 3. Download & launch MicMorph setup
$tempExe = Join-Path $tempDir "MicMorph_0.1.0_x64-setup.exe"
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
