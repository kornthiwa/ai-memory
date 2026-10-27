# Install DormNet to run at user logon via Task Scheduler (no admin required for current user).
param(
    [string]$ExePath = "",
    [string]$TaskName = "DormNet"
)

$ErrorActionPreference = "Stop"

if (-not $ExePath) {
    $repo = Split-Path -Parent $PSScriptRoot
    $candidate = Join-Path $repo "target\release\dormnet.exe"
    if (-not (Test-Path $candidate)) {
        Write-Error "Release binary not found at $candidate. Build first: cargo build --release"
    }
    $ExePath = (Resolve-Path $candidate).Path
} else {
    $ExePath = (Resolve-Path $ExePath).Path
}

Write-Host "Installing task '$TaskName' -> $ExePath"

# Remove existing task if present
schtasks /Delete /TN $TaskName /F 2>$null | Out-Null

# At logon, current user, highest available privileges for user, hidden window.
# /RL LIMITED avoids requiring admin; /IT runs only when user is logged on interactively.
schtasks /Create /TN $TaskName /TR "`"$ExePath`" run" /SC ONLOGON /RL LIMITED /F | Out-Null

Write-Host "Installed. Verify with: schtasks /Query /TN $TaskName /V /FO LIST"
Write-Host "Logs: $env:LOCALAPPDATA\DormNet\logs"
