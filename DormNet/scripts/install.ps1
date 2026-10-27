# KIOIS Net installer (per-user, no admin)
# Registers in Windows Settings > Apps like a normal program.

param(
    [switch]$SkipBuild,
    [switch]$NoShortcut,
    [switch]$NoAutostart,
    [switch]$LaunchGui
)

$ErrorActionPreference = "Stop"

$productName = "KIOIS Net"
$publisher = "KIOIS"
$version = "1.0.0"

function Get-UninstallLabel([string]$appName) {
    $prefix = -join @(
        [char]0x0E16, [char]0x0E2D, [char]0x0E19, [char]0x0E01, [char]0x0E32, [char]0x0E23,
        [char]0x0E15, [char]0x0E34, [char]0x0E14, [char]0x0E15, [char]0x0E31, [char]0x0E49, [char]0x0E07
    )
    return "$prefix $appName"
}

$repo = Split-Path -Parent $PSScriptRoot
$releaseExe = Join-Path $repo "target\release\dormnet.exe"
$installDir = Join-Path $env:LOCALAPPDATA "DormNet"
$installedExe = Join-Path $installDir "dormnet.exe"
$uninstallCmd = Join-Path $installDir "Uninstall.cmd"
$uninstallPs1 = Join-Path $installDir "uninstall.ps1"
$uninstallReg = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\DormNet"

Write-Host "=== $productName Installer (by $publisher) ==="
Write-Host "Repo:       $repo"
Write-Host "Install to: $installDir"
Write-Host ""

if (-not $SkipBuild) {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Error "cargo not found. Install Rust from https://rustup.rs then re-run."
    }
    Write-Host "[1/5] Building release..."
    Push-Location $repo
    try {
        cargo build --release
        if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed" }
    } finally {
        Pop-Location
    }
} elseif (-not (Test-Path $releaseExe)) {
    Write-Error "No release binary at $releaseExe (run without -SkipBuild, or cargo build --release first)"
}

if (-not (Test-Path $releaseExe)) {
    Write-Error "Build finished but exe missing: $releaseExe"
}

Write-Host "[2/5] Copying program files..."
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $installDir "logs") | Out-Null
Copy-Item -Force $releaseExe $installedExe
Copy-Item -Force (Join-Path $PSScriptRoot "uninstall.ps1") $uninstallPs1
$iconSrc = Join-Path $repo "assets\dormnet.ico"
$iconDst = Join-Path $installDir "dormnet.ico"
if (Test-Path $iconSrc) { Copy-Item -Force $iconSrc $iconDst }

@(
    "@echo off"
    "title Uninstall $productName"
    "echo Uninstalling $productName..."
    "powershell -NoProfile -ExecutionPolicy Bypass -File `"%~dp0uninstall.ps1`""
    "if errorlevel 1 pause"
) | Set-Content -Path $uninstallCmd -Encoding UTF8

$w = New-Object -ComObject WScript.Shell

Write-Host "[3/5] Creating shortcuts..."
$desktop = [Environment]::GetFolderPath("Desktop")
$programs = Join-Path ([Environment]::GetFolderPath("StartMenu")) "Programs\KIOIS"
New-Item -ItemType Directory -Force -Path $programs | Out-Null

# Remove leftover old shortcuts / folders
$oldPrograms = Join-Path ([Environment]::GetFolderPath("StartMenu")) "Programs\DormNet"
if (Test-Path $oldPrograms) { Remove-Item -Recurse -Force $oldPrograms -ErrorAction SilentlyContinue }

function Set-AppIcon($shortcut) {
    if (Test-Path $iconDst) {
        $shortcut.IconLocation = "$iconDst,0"
    } elseif (Test-Path $installedExe) {
        $shortcut.IconLocation = "$installedExe,0"
    }
}

if (-not $NoShortcut) {
    $startup = [Environment]::GetFolderPath("Startup")
    $oldNames = @(
        "DormNet.lnk",
        "KIOIS Net.lnk",
        "Install DormNet.lnk"
    )
    # Also remove previous Thai-named shortcuts best-effort
    Get-ChildItem $desktop -Filter "*.lnk" -ErrorAction SilentlyContinue | ForEach-Object {
        if ($_.BaseName -match 'DormNet|เน็ตหอ|NetHo|Install Dorm') {
            Remove-Item -Force $_.FullName -ErrorAction SilentlyContinue
        }
    }
    foreach ($old in $oldNames) {
        $p = Join-Path $desktop $old
        if (Test-Path $p) { Remove-Item -Force $p -ErrorAction SilentlyContinue }
    }

    $lnkPath = Join-Path $desktop "$productName.lnk"
    $sc = $w.CreateShortcut($lnkPath)
    $sc.TargetPath = $installedExe
    $sc.Arguments = "gui"
    $sc.WorkingDirectory = $installDir
    $sc.WindowStyle = 1
    $sc.Description = "$productName by $publisher"
    Set-AppIcon $sc
    $sc.Save()
    Write-Host "  Desktop -> $lnkPath"
}

$startGui = Join-Path $programs "$productName.lnk"
$scGui = $w.CreateShortcut($startGui)
$scGui.TargetPath = $installedExe
$scGui.Arguments = "gui"
$scGui.WorkingDirectory = $installDir
$scGui.Description = "$productName by $publisher"
Set-AppIcon $scGui
$scGui.Save()

$unLabel = Get-UninstallLabel $productName
$startUninst = Join-Path $programs "$unLabel.lnk"
$scUn = $w.CreateShortcut($startUninst)
$scUn.TargetPath = $uninstallCmd
$scUn.WorkingDirectory = $installDir
$scUn.Description = $unLabel
Set-AppIcon $scUn
$scUn.Save()
Write-Host "  Start Menu -> $programs"

if (-not $NoAutostart) {
    Write-Host "[4/5] Enabling autostart..."
    $startup = [Environment]::GetFolderPath("Startup")
    Get-ChildItem $startup -Filter "*.lnk" -ErrorAction SilentlyContinue | ForEach-Object {
        if ($_.BaseName -match 'DormNet|เน็ตหอ|KIOIS|NetHo') {
            Remove-Item -Force $_.FullName -ErrorAction SilentlyContinue
        }
    }

    # Delayed start: wait for NIC/DHCP after login, then run monitor (hidden).
    $runCmd = Join-Path $installDir "autostart-run.cmd"
    @(
        "@echo off"
        "rem Wait for network after Windows logon, then start background monitor"
        "timeout /t 25 /nobreak >nul"
        "start `"`" /min `"$installedExe`" run"
    ) | Set-Content -Path $runCmd -Encoding ASCII

    $startupLnk = Join-Path $startup "$productName.lnk"
    $sc2 = $w.CreateShortcut($startupLnk)
    $sc2.TargetPath = $runCmd
    $sc2.WorkingDirectory = $installDir
    $sc2.WindowStyle = 7
    $sc2.Description = "$productName background (auto login)"
    Set-AppIcon $sc2
    $sc2.Save()
    Write-Host "  -> $startupLnk (delayed 25s then: dormnet run)"
} else {
    Write-Host "[4/5] Skipping autostart"
}

Write-Host "[5/5] Registering in Settings > Apps..."
$estimatedKb = [int]((Get-Item $installedExe).Length / 1KB)
New-Item -Path $uninstallReg -Force | Out-Null
Set-ItemProperty -Path $uninstallReg -Name "DisplayName" -Value $productName
Set-ItemProperty -Path $uninstallReg -Name "DisplayVersion" -Value $version
Set-ItemProperty -Path $uninstallReg -Name "Publisher" -Value $publisher
Set-ItemProperty -Path $uninstallReg -Name "InstallLocation" -Value $installDir
$displayIcon = if (Test-Path $iconDst) { $iconDst } else { $installedExe }
Set-ItemProperty -Path $uninstallReg -Name "DisplayIcon" -Value $displayIcon
Set-ItemProperty -Path $uninstallReg -Name "UninstallString" -Value ("`"$uninstallCmd`"")
Set-ItemProperty -Path $uninstallReg -Name "QuietUninstallString" -Value ("powershell -NoProfile -ExecutionPolicy Bypass -File `"$uninstallPs1`" -Quiet")
Set-ItemProperty -Path $uninstallReg -Name "NoModify" -Value 1 -Type DWord
Set-ItemProperty -Path $uninstallReg -Name "NoRepair" -Value 1 -Type DWord
Set-ItemProperty -Path $uninstallReg -Name "EstimatedSize" -Value $estimatedKb -Type DWord

Write-Host ""
Write-Host "Installed OK."
Write-Host "  Name:      $productName"
Write-Host "  Creator:   $publisher"
Write-Host "  Apps list: Settings > Apps > $productName"
Write-Host "  Exe:       $installedExe"
Write-Host "  Uninstall: Start Menu > KIOIS > $unLabel"
Write-Host ""

if ($LaunchGui) {
    Start-Process -FilePath $installedExe -ArgumentList "gui"
}
