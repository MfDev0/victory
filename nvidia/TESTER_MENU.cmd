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
echo    /\_/\    VIctory NVIDIA  -  TESTER MENU
echo   ( o.o )   Try build by MfDev0
echo    ^> ^^ ^<
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
echo   T  Try: PR fixes, Discord
echo   Q  Quit
echo.
echo   In game: F1 settings, F3 stats panel.
echo.
choice /c 123456789RLCDTQ /n /m "Pick: "
set "PICK=%ERRORLEVEL%"
if "%PICK%"=="15" goto :end
if "%PICK%"=="14" goto :try
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

rem ---- T: the fixes, the left-out PRs, the language, Discord ----
rem 1, 2, 3 and A work and are on, so F1 no longer shows them: only
rem here. The rest are F1 rows too; a pick flips it in the same file.
:try
cls
call :current VICTORY_BLACK_FACE_FIX 0 F1
set "F2="
call :value VICTORY_MOVIE_FG F2
if /i "%F2%"=="smooth" (set "F2=SMOOTH") else if /i "%F2%"=="pause" (set "F2=PAUSE") else (set "F2=UPSTREAM")
call :current VICTORY_INDIRECT_WIDE_NVIDIA 0 F3
call :current VICTORY_BVH_FAULTS_ALL 16384 F4
call :current VICTORY_DISCORD 0 F5
call :current VICTORY_KEYMAP "a=20,lt=A2" F6
call :current VICTORY_KEEP_SHOWN checked F8
call :current VICTORY_PROBE_NEUTRAL 0.3 F9
call :current VICTORY_SKY_BOUND 0 FA
call :current VICTORY_PHOTOS 0 FE
set "LANG_NOW=en"
call :value VICTORY_MENU_LANGUAGE LANG_NOW
echo =============================================
echo   TRY: FIXES, PRS, DISCORD
echo   (1 2 3 A: only here, not in F1)
echo =============================================
echo   1  PR 669  black face fix       %F1%
echo   2  Movie cutscenes (DLSS FG)    %F2%
echo   3  PR 1005 wide dispatch (NV)   %F3%
echo   A  PR 393  sky shine fix        %FA%
echo   4  PR 1008 RT fault pages       %F4%
echo   5  Discord: Playing GTA VI      %F5%
echo   6  PR 982  Space=A, Ctrl=LT     %F6%
echo   7  PR 1056 menu language        %LANG_NOW%
echo   8  PR 280  outfit check         %F8%
echo   9  PR 851  neutral probe light  %F9%
echo   E  Photo mode saves             %FE%
echo   B  Back
echo.
echo   1 3 A 5 E are on unless set to 0.
echo   2 goes UPSTREAM, SMOOTH, PAUSE.
echo   Photos: Downloads\VIctory-Photos
echo.
choice /c 123456789AEB /n /m "Flip: "
set "PICK=%ERRORLEVEL%"
if "%PICK%"=="12" goto :menu
if "%PICK%"=="10" call :flip_off VICTORY_SKY_BOUND "%FA%"
if "%PICK%"=="11" call :flip_off VICTORY_PHOTOS "%FE%"
if "%PICK%"=="8" call :flip VICTORY_KEEP_SHOWN checked "%F8%"
if "%PICK%"=="9" call :flip VICTORY_PROBE_NEUTRAL 0.3 "%F9%"
if "%PICK%"=="7" goto :language
if "%PICK%"=="6" call :flip VICTORY_KEYMAP "a=20,lt=A2" "%F6%"
if "%PICK%"=="1" call :flip_off VICTORY_BLACK_FACE_FIX "%F1%"
if "%PICK%"=="2" call :movie "%F2%"
if "%PICK%"=="3" call :flip_off VICTORY_INDIRECT_WIDE_NVIDIA "%F3%"
if "%PICK%"=="4" call :flip VICTORY_BVH_FAULTS_ALL 16384 "%F4%"
if "%PICK%"=="5" call :flip_off VICTORY_DISCORD "%F5%"
goto :try

:language
echo.
echo   1 English  2 Simplified Chinese
echo   3 Traditional Chinese  4 Spanish
echo   5 Auto (as Windows)
choice /c 12345 /n /m "Language: "
set "PICK=%ERRORLEVEL%"
if "%PICK%"=="1" call :save_choice VICTORY_MENU_LANGUAGE en
if "%PICK%"=="2" call :save_choice VICTORY_MENU_LANGUAGE zh-CN
if "%PICK%"=="3" call :save_choice VICTORY_MENU_LANGUAGE zh-TW
if "%PICK%"=="4" call :save_choice VICTORY_MENU_LANGUAGE es
if "%PICK%"=="5" call :save_choice VICTORY_MENU_LANGUAGE auto
goto :try

rem Movie cutscenes: UPSTREAM (not set, the default), then smooth, then pause.
:movie
if /i "%~1"=="UPSTREAM" (call :save_choice VICTORY_MOVIE_FG smooth) else if /i "%~1"=="SMOOTH" (call :save_choice VICTORY_MOVIE_FG pause) else (call :save_choice VICTORY_MOVIE_FG "")
goto :eof

rem %1 the setting, %2 its value when it is the odd one out, %3 where ON or OFF goes.
rem A setting off unless %2 (the PRs): ON when it holds %2. A setting on unless 0
rem (%2 is 0: Discord): OFF when it holds 0.
:current
set "CUR="
call :value %~1 CUR
if "%~2"=="0" (
  if "%CUR%"=="0" (set "%~3=OFF") else (set "%~3=ON")
) else (
  if "%CUR%"=="%~2" (set "%~3=ON") else (set "%~3=OFF")
)
goto :eof

rem The last value of %1 in the settings file, into the variable %2.
:value
if not exist "%SETTINGS%" goto :eof
for /f "usebackq tokens=1,* delims==" %%A in ("%SETTINGS%") do if /i "%%A"=="%~1" set "%~2=%%B"
goto :eof

:flip
rem The value is quoted: call splits on = and , (PR 982's keys).
if /i "%~3"=="ON" (call :save_choice %~1 "") else (call :save_choice %~1 "%~2")
goto :eof

:flip_off
if /i "%~2"=="ON" (call :save_choice %~1 0) else (call :save_choice %~1 "")
goto :eof

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
