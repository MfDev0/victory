@echo off
rem VIctory - FG UNLOCK: DLSS frame generation 2x to 6x. ASCII only.
rem Built on MfDev0/dlss-unlocked- (tools\dlss-unlocked\START-HERE.cmd).
rem   UNLOCK_FG.cmd          pick, save as F1 does, start the game
rem   UNLOCK_FG.cmd 4        4x straight away
rem On an RTX 20, 30 or 40, 3x and more turn MFG Unlocker on by
rem themselves (F1, FG UNLOCK: AUTO). victory.exe writes the mod's
rem RTXMFG-Universal.json and RTXMFG-UI.ini at that multiplier.
setlocal EnableExtensions DisableDelayedExpansion
set "HERE=%~dp0"
set "FROM_TESTER="
if /i "%~1"=="fromtester" set "FROM_TESTER=1"
title VIctory - FG UNLOCK
call :find_all
if not defined VICTORY goto :fail
if not defined GAME goto :fail
call :running
if not errorlevel 1 (
  echo victory.exe is already running. Close it first.
  goto :fail
)
set "FG="
for %%N in (0 2 3 4 5 6) do if "%~1"=="%%N" set "FG=%%N"
if defined FG goto :picked

cls
echo =============================================
echo   VIctory  -  FG UNLOCK
echo   DLSS frame generation, 2x to 6x
echo =============================================
echo   2  2x  (RTX 20/30: with MFG Unlocker)
echo   3  3x  two generated frames per real one
echo   4  4x  three generated frames
echo   5  5x  four generated frames
echo   6  6x  five generated frames
echo   0  Frame generation OFF
echo   Q  Back
echo.
echo   RTX 50 makes 2x-6x itself. RTX 20-40 use
echo   MFG Unlocker (a mod, not NVIDIA's, MIT).
echo   It is downloaded and checked once.
echo.
choice /c 234560Q /n /m "Pick: "
set "PICK=%ERRORLEVEL%"
if "%PICK%"=="7" goto :end
if "%PICK%"=="6" set "FG=0"
if "%PICK%"=="5" set "FG=6"
if "%PICK%"=="4" set "FG=5"
if "%PICK%"=="3" set "FG=4"
if "%PICK%"=="2" set "FG=3"
if "%PICK%"=="1" set "FG=2"

:picked
if "%FG%"=="0" (
  call :save_choice VICTORY_MENU_FRAMEGEN 0
  echo Frame generation saved as OFF.
  goto :launch
)
set "MFG_DLL=%HOME_DIR%\mfg-unlocker\MFGUnlocker.dll"
if exist "%MFG_DLL%" goto :have_mfg
echo.
echo Downloading MFG Unlocker v1.7.2 (checked)...
"%VICTORY%" get-mfg
if not exist "%MFG_DLL%" (
  echo MFG Unlocker could not be downloaded or checked.
  echo RTX 50 cards do not need it; others stay at 2x.
)
:have_mfg
call :save_choice VICTORY_MENU_FRAMEGEN %FG%
call :save_choice VICTORY_MENU_FG_TYPE dlss
call :save_choice VICTORY_MENU_FPS 1000
rem 2x needs the mod switched on by name (only RTX 20/30 need it);
rem 3x-6x leave it on AUTO, which picks it on RTX 20, 30 and 40.
if "%FG%"=="2" (
  call :save_choice VICTORY_MFG_UNLOCKER 1
) else (
  call :save_choice VICTORY_MFG_UNLOCKER ""
)
echo Saved: frame generation %FG%x, DLSS, no frame cap.

:launch
echo.
echo Starting the game. In game: F1 settings, F3 stats,
echo Backspace: MFG Unlocker's own menu when it is loaded.
"%VICTORY%" play "%GAME%"
set "RESULT=%ERRORLEVEL%"
echo.
echo victory.exe ended with code %RESULT%.
if not "%RESULT%"=="0" echo A crash report zip goes to Downloads\VIctory-reports.
if not defined FROM_TESTER pause
exit /b %RESULT%

:fail
echo.
if not defined FROM_TESTER pause
exit /b 1

:end
exit /b 0

rem ---- shared: victory.exe, the home folder, the game ----
:find_all
set "VICTORY=%HERE%victory.exe"
if not exist "%VICTORY%" set "VICTORY=%HERE%target\release\victory.exe"
if not exist "%VICTORY%" (
  echo victory.exe not found beside this file or in target\release.
  echo Build it first: BUILD_VICTORY.cmd
  set "VICTORY="
  goto :eof
)
set "HOME_DIR=%LOCALAPPDATA%\victory"
if defined VICTORY_HOME set "HOME_DIR=%VICTORY_HOME%"
if not exist "%HOME_DIR%" mkdir "%HOME_DIR%"
set "SETTINGS=%HOME_DIR%\victory-settings.txt"
set "GAME_FILE=%HOME_DIR%\game-path.txt"
set "GAME="
if defined VICTORY_GAME_DIR if exist "%VICTORY_GAME_DIR%\game_scarlett_master.exe" set "GAME=%VICTORY_GAME_DIR%\game_scarlett_master.exe"
if not defined GAME for %%V in ("%VICTORY%") do if exist "%%~dpVgame_scarlett_master.exe" set "GAME=%%~dpVgame_scarlett_master.exe"
if not defined GAME if exist "%HERE%game_scarlett_master.exe" set "GAME=%HERE%game_scarlett_master.exe"
if not defined GAME if exist "%GAME_FILE%" set /p GAME=<"%GAME_FILE%"
if defined GAME set "GAME=%GAME:"=%"
if defined GAME if not exist "%GAME%" set "GAME="
if defined GAME goto :eof
echo.
echo Enter the full path to your game_scarlett_master.exe
echo (or the folder it is in). It is saved for next time.
set /p "GAME=Game path: "
if not defined GAME goto :eof
set "GAME=%GAME:"=%"
if exist "%GAME%\game_scarlett_master.exe" set "GAME=%GAME%\game_scarlett_master.exe"
if not exist "%GAME%" (
  echo Not found: the game executable.
  set "GAME="
  goto :eof
)
setlocal EnableDelayedExpansion
> "!GAME_FILE!" echo(!GAME!
endlocal
goto :eof

rem Saves NAME=VALUE as the F1 menu would (the last line of a name wins).
:save_choice
>> "%SETTINGS%" echo(
>> "%SETTINGS%" echo %~1=%~2
goto :eof

:running
"%SystemRoot%\System32\tasklist.exe" /fi "IMAGENAME eq victory.exe" | "%SystemRoot%\System32\find.exe" /i "victory.exe" >nul
goto :eof
