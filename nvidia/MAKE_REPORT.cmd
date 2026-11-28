@echo off
rem VIctory - tester report: one zip in Downloads\VIctory-reports with the
rem newest logs, crashes.log, the F1 settings, MFG Unlocker's files and a
rem system and summary page. Safe to run any time, also while the game runs.
setlocal EnableExtensions DisableDelayedExpansion
set "VICTORY=%~dp0victory.exe"
if not exist "%VICTORY%" set "VICTORY=%~dp0target\release\victory.exe"
if not exist "%VICTORY%" (
  echo victory.exe not found beside this file or in target\release.
  pause
  exit /b 1
)
"%VICTORY%" report
pause
exit /b 0
