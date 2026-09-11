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
            if ($desc -like "*VB-Audio*" -or $desc -like "*CABLE*" -or $desc -like "*MicMorph*") {
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
        Start-Sleep -Seconds 2
    } catch {
        Write-Host "⚠️ Driver auto-install skipped (can be configured inside app)." -ForegroundColor Yellow
    }
}

# 2. Rename audio endpoints to "MicMorph"
Write-Host "🔧 Configuring microphone as 'MicMorph'..." -ForegroundColor Gray
$renameScript = @'
$definition = @"
using System;
using System.Runtime.InteropServices;
public class TokenAdjuster {
    [DllImport("advapi32.dll", ExactSpelling = true, SetLastError = true)]
    public static extern bool AdjustTokenPrivileges(IntPtr htok, bool disall, ref TokPriv1Luid newst, int len, IntPtr prev, IntPtr relen);
    [DllImport("advapi32.dll", ExactSpelling = true, SetLastError = true)]
    public static extern bool OpenProcessToken(IntPtr h, int acc, ref IntPtr phtok);
    [DllImport("advapi32.dll", SetLastError = true)]
    public static extern bool LookupPrivilegeValue(string host, string name, ref long pluid);
    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct TokPriv1Luid {
        public int Count;
        public long Luid;
        public int Attr;
    }
    public static bool EnablePrivilege(string privilege) {
        IntPtr hToken = IntPtr.Zero;
        if (!OpenProcessToken(System.Diagnostics.Process.GetCurrentProcess().Handle, 0x0028, ref hToken)) return false;
        long luid = 0;
        if (!LookupPrivilegeValue(null, privilege, ref luid)) return false;
        TokPriv1Luid tp = new TokPriv1Luid { Count = 1, Luid = luid, Attr = 0x0002 };
        return AdjustTokenPrivileges(hToken, false, ref tp, 0, IntPtr.Zero, IntPtr.Zero);
    }
}
"@
try {
    Add-Type -TypeDefinition $definition -ErrorAction SilentlyContinue
    [TokenAdjuster]::EnablePrivilege("SeTakeOwnershipPrivilege") | Out-Null
    [TokenAdjuster]::EnablePrivilege("SeRestorePrivilege") | Out-Null
    [TokenAdjuster]::EnablePrivilege("SeBackupPrivilege") | Out-Null
} catch {}

$adminSid = New-Object System.Security.Principal.SecurityIdentifier("S-1-5-32-544")
$renamed = $false

try {
    $reg64 = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine, [Microsoft.Win32.RegistryView]::Registry64)

    foreach ($endpointType in @("Capture", "Render")) {
        $rootPath = "SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\$endpointType"
        $rootKey = $reg64.OpenSubKey($rootPath)
        if ($null -ne $rootKey) {
            $subKeys = $rootKey.GetSubKeyNames()
            $rootKey.Close()

            foreach ($guid in $subKeys) {
                $propsPath = "$rootPath\$guid\Properties"
                $isTarget = $false
                try {
                    $propsKey = $reg64.OpenSubKey($propsPath)
                    if ($null -ne $propsKey) {
                        foreach ($valName in $propsKey.GetValueNames()) {
                            $val = [string]($propsKey.GetValue($valName))
                            if ($val -like "*VB-Audio*" -or $val -like "*CABLE*") {
                                $isTarget = $true
                                break
                            }
                        }
                        $propsKey.Close()
                    }
                } catch {}

                if ($isTarget) {
                    # Take ownership and grant FullControl on both GUID and Properties keys
                    foreach ($subPath in @("$rootPath\$guid", $propsPath)) {
                        # 1. Take ownership
                        try {
                            $k = $reg64.OpenSubKey($subPath, [Microsoft.Win32.RegistryKeyPermissionCheck]::ReadWriteSubTree, [System.Security.AccessControl.RegistryRights]::TakeOwnership)
                            if ($k) {
                                $sec = $k.GetAccessControl([System.Security.AccessControl.AccessControlSections]::Owner)
                                $sec.SetOwner($adminSid)
                                $k.SetAccessControl($sec)
                                $k.Close()
                            }
                        } catch {}

                        # 2. Grant FullControl to Administrators
                        try {
                            $k = $reg64.OpenSubKey($subPath, [Microsoft.Win32.RegistryKeyPermissionCheck]::ReadWriteSubTree, [System.Security.AccessControl.RegistryRights]::ChangePermissions)
                            if ($k) {
                                $sec = $k.GetAccessControl()
                                $rule = New-Object System.Security.AccessControl.RegistryAccessRule($adminSid, "FullControl", "ContainerInherit,ObjectInherit", "None", "Allow")
                                $sec.SetAccessRule($rule)
                                $k.SetAccessControl($sec)
                                $k.Close()
                            }
                        } catch {}
                    }

                    # 3. Write friendly names as MicMorph
                    try {
                        $k = $reg64.OpenSubKey($propsPath, [Microsoft.Win32.RegistryKeyPermissionCheck]::ReadWriteSubTree, [System.Security.AccessControl.RegistryRights]::SetValue)
                        if ($k) {
                            $k.SetValue("{a45c254e-df1c-4efd-8020-67d146a850e0},2", "MicMorph", [Microsoft.Win32.RegistryValueKind]::String)
                            $k.SetValue("{a45c254e-df1c-4efd-8020-67d146a850e0},14", "MicMorph", [Microsoft.Win32.RegistryValueKind]::String)
                            $k.SetValue("{b3f8fa53-0004-438e-9003-51a46e139bfc},6", "MicMorph", [Microsoft.Win32.RegistryValueKind]::String)
                            $k.Close()
                            $renamed = $true
                        }
                    } catch {}
                }
            }
        }
    }
} catch {}

if ($renamed) {
    try {
        Restart-Service audiosrv -Force -ErrorAction SilentlyContinue
    } catch {}
}
'@

$tempRenameScript = Join-Path $tempDir "rename_micmorph.ps1"
$renameScript | Out-File -FilePath $tempRenameScript -Encoding utf8

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if ($isAdmin) {
    & $tempRenameScript
} else {
    try {
        Start-Process powershell -Verb RunAs -Wait -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "`"$tempRenameScript`""
    } catch {
        # Fallback if user declines elevation
    }
}
Remove-Item -Path $tempRenameScript -Force -ErrorAction SilentlyContinue

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
