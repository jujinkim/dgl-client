@echo off
if not "%~1"=="" goto direct
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0windows_menu.ps1"
exit /b %errorlevel%
:direct
rem Arguments always run the app; add -Run only once so PowerShell does not reject a repeated switch.
setlocal
set "run=-Run"
for %%A in (%*) do if /i "%%~A"=="-Run" set "run="
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build_windows.ps1" %run% %*
exit /b %errorlevel%
