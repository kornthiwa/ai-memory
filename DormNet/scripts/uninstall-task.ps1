# Remove DormNet Task Scheduler entry.
param(
    [string]$TaskName = "DormNet"
)

$ErrorActionPreference = "Stop"

schtasks /Delete /TN $TaskName /F
Write-Host "Removed task '$TaskName'."
Write-Host "Optional: stop a running instance from Task Manager (dormnet.exe)."
Write-Host "Optional: clear credentials with: dormnet.exe creds clear"
