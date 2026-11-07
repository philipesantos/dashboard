@echo off
rem Double-click: runs windows-check.ps1 and writes spike-report.txt in this folder.
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0windows-check.ps1"
echo.
echo Send spike-report.txt from this folder.
pause
