# Create a Desktop shortcut that opens the DormNet credential GUI.
param(
    [string]$ExePath = ""
)

$ErrorActionPreference = "Stop"

if (-not $ExePath) {
    $repo = Split-Path -Parent $PSScriptRoot
    $candidate = Join-Path $repo "target\release\dormnet.exe"
    if (-not (Test-Path $candidate)) {
        Write-Error "Build release first: cargo build --release"
    }
    $ExePath = (Resolve-Path $candidate).Path
} else {
    $ExePath = (Resolve-Path $ExePath).Path
}

$desktop = [Environment]::GetFolderPath("Desktop")
$lnkPath = Join-Path $desktop "DormNet.lnk"

$w = New-Object -ComObject WScript.Shell
$sc = $w.CreateShortcut($lnkPath)
$sc.TargetPath = $ExePath
$sc.Arguments = "gui"
$sc.WorkingDirectory = Split-Path $ExePath
$sc.WindowStyle = 1
$sc.Description = "DormNet - save dorm internet credentials"
$sc.Save()

Write-Host "Shortcut created: $lnkPath"
