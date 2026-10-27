@echo off
title KIOIS Net Installer
cd /d "%~dp0"
echo Installing KIOIS Net (by KIOIS)...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\install.ps1" -LaunchGui
if errorlevel 1 (
  echo.
  echo Install failed.
  pause
)
