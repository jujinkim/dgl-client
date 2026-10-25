@echo off
if not "%~1"=="" goto direct
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0windows_menu.ps1"
exit /b %errorlevel%
:direct
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build_windows.ps1" -Run %*
exit /b %errorlevel%
