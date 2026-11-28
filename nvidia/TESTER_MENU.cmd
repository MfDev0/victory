@echo off
rem VIctory NVIDIA - tester menu. ASCII only, cmd style.
rem   TESTER_MENU.cmd   the menu; every play line saves its choice as F1 does.
setlocal EnableExtensions DisableDelayedExpansion
set "HERE=%~dp0"
title VIctory - tester menu
call :find_all
if not defined VICTORY goto :fail
if not defined GAME goto :fail
set "LAST="

:menu
cls
echo =============================================
echo   VIctory NVIDIA  -  TESTER MENU
echo =============================================
echo   Game: %GAME%
if defined LAST echo   Last run ended with code %LAST%
echo.
echo   1  Play with your F1 settings
echo   2  Series X   DLSS Quality
echo   3  Series X   DLSS Balanced
echo   4  Series X   DLSS Performance
echo   5  Series X   DLAA (native)
echo   6  Series S   DLSS Quality (720)
echo   7  Series S   DLSS Performance (540)
echo   8  Frame gen 2x-6x / FG UNLOCK
echo   9  Preload shaders (long, once)
echo   R  Make tester report (zip)
echo   L  Open the logs folder
echo   C  Check DLSS and frame gen here
echo   D  Reset F1 settings (kept as .bak)
echo   Q  Quit
echo.
echo   In game: F1 settings, F3 stats panel.
echo.
choice /c 123456789RLCDQ /n /m "Pick: "
set "PICK=%ERRORLEVEL%"
if "%PICK%"=="14" goto :end
if "%PICK%"=="13" goto :reset
if "%PICK%"=="12" goto :check
if "%PICK%"=="11" goto :logs
if "%PICK%"=="10" goto :report
if "%PICK%"=="9" goto :preload
if "%PICK%"=="8" goto :unlock
if "%PICK%"=="7" call :mode series-s performance 540
if "%PICK%"=="6" call :mode series-s quality 720
if "%PICK%"=="5" call :mode series-x dlaa
if "%PICK%"=="4" call :mode series-x performance
if "%PICK%"=="3" call :mode series-x balanced
if "%PICK%"=="2" call :mode series-x quality
goto :play

:mode
call :save_choice VICTORY_MENU_CONSOLE %~1
call :save_choice VICTORY_MENU_DLSS %~2
if not "%~3"=="" call :save_choice VICTORY_MENU_S_RES %~3
goto :eof

:play
call :running
if not errorlevel 1 (
  echo victory.exe is already running. Close it first.
  pause
  goto :menu
)
echo.
"%VICTORY%" play "%GAME%"
set "LAST=%ERRORLEVEL%"
if not "%LAST%"=="0" echo A crash report zip goes to Downloads\VIctory-reports.
pause
goto :menu

:unlock
call "%HERE%UNLOCK_FG.cmd" fromtester
set "LAST=%ERRORLEVEL%"
goto :menu

:preload
"%VICTORY%" play "%GAME%" --preload
set "LAST=%ERRORLEVEL%"
pause
goto :menu

:report
"%VICTORY%" report
pause
goto :menu

:logs
if not exist "%HOME_DIR%\logs" mkdir "%HOME_DIR%\logs"
start "" explorer.exe "%HOME_DIR%\logs"
goto :menu

:check
set "SL=%HOME_DIR%\nvidia-sdk\Streamline\bin\x64"
if not defined VICTORY_STREAMLINE_DIR if exist "%SL%\sl.interposer.dll" set "VICTORY_STREAMLINE_DIR=%SL%"
"%VICTORY%" upscaler-check --device
pause
goto :menu

:reset
if exist "%SETTINGS%" move /y "%SETTINGS%" "%SETTINGS%.bak" >nul
echo F1 settings reset (old file: .bak).
pause
goto :menu

:fail
echo.
pause
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
