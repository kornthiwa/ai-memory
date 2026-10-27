# Uninstall KIOIS Net (also used from Settings > Apps).

param(
    [switch]$KeepCredentials,
    [switch]$KeepLogs,
    [switch]$Quiet
)

$ErrorActionPreference = "Stop"

$productName = "KIOIS Net"
$installDir = Join-Path $env:LOCALAPPDATA "DormNet"
$installedExe = Join-Path $installDir "dormnet.exe"
$desktop = [Environment]::GetFolderPath("Desktop")
$startup = [Environment]::GetFolderPath("Startup")
$programs = Join-Path ([Environment]::GetFolderPath("StartMenu")) "Programs\KIOIS"
$oldPrograms = Join-Path ([Environment]::GetFolderPath("StartMenu")) "Programs\DormNet"
$uninstallReg = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\DormNet"

function Write-Step([string]$msg) {
    if (-not $Quiet) { Write-Host $msg }
}

Write-Step "=== $productName Uninstall ==="

Get-Process -Name "dormnet" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Write-Step "Stopped running process (if any)."

cmd /c "schtasks /Delete /TN DormNet /F >nul 2>&1" | Out-Null

foreach ($dir in @($desktop, $startup)) {
    Get-ChildItem $dir -Filter "*.lnk" -ErrorAction SilentlyContinue | ForEach-Object {
        if ($_.BaseName -match 'DormNet|เน็ตหอ|KIOIS|NetHo|Install Dorm') {
            Remove-Item -Force $_.FullName -ErrorAction SilentlyContinue
            Write-Step "Removed $($_.FullName)"
        }
    }
}

foreach ($p in @($programs, $oldPrograms)) {
    if (Test-Path $p) {
        Remove-Item -Recurse -Force $p -ErrorAction SilentlyContinue
        Write-Step "Removed $p"
    }
}

if (-not $KeepCredentials) {
    $scriptDir = $PSScriptRoot
    $repo = Split-Path -Parent $scriptDir
    $candidates = @(
        $installedExe,
        (Join-Path $scriptDir "dormnet.exe"),
        (Join-Path $repo "target\release\dormnet.exe"),
        (Join-Path $repo "target\debug\dormnet.exe")
    )
    foreach ($cand in $candidates) {
        if (Test-Path $cand) {
            try { & $cand creds clear 2>$null } catch {}
            break
        }
    }
    Write-Step "Cleared saved credentials (best effort)."
}

if (Test-Path $uninstallReg) {
    Remove-Item -Path $uninstallReg -Recurse -Force -ErrorAction SilentlyContinue
    Write-Step "Removed Apps & Features entry."
}

$keepSelf = $false
if ($PSCommandPath -and ($PSCommandPath.StartsWith($installDir, [System.StringComparison]::OrdinalIgnoreCase))) {
    $keepSelf = $true
}

if (-not $KeepLogs) {
    $logs = Join-Path $installDir "logs"
    if (Test-Path $logs) {
        Remove-Item -Recurse -Force $logs -ErrorAction SilentlyContinue
    }
}

if (Test-Path $installDir) {
    Get-ChildItem $installDir -Force -ErrorAction SilentlyContinue | ForEach-Object {
        if ($keepSelf -and ($_.FullName -eq $PSCommandPath -or $_.Name -eq "Uninstall.cmd" -or $_.Name -eq "uninstall.ps1")) {
            return
        }
        Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if ($keepSelf -and (Test-Path $installDir)) {
    $bat = Join-Path $env:TEMP "dormnet-remove-leftover.cmd"
    @(
        "@echo off"
        "ping 127.0.0.1 -n 2 >nul"
        "rmdir /s /q `"$installDir`""
        "del `"%~f0`""
    ) | Set-Content -Path $bat -Encoding ASCII
    Start-Process -FilePath $bat -WindowStyle Hidden
    Write-Step "Scheduled removal of $installDir"
}

Write-Step "Uninstall done."
if (-not $Quiet) {
    Write-Host "You can close this window."
}
