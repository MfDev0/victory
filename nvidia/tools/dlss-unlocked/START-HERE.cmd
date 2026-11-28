@echo off
setlocal EnableExtensions
title Victory - Portable DLSS Frame Generation Selector
cd /d "%~dp0"

set "VICTORY=%~dp0victory.exe"
set "GAME_FILE=%~dp0game-path.txt"
set "CONFIG=%~dp0RTXMFG-Universal.json"
set "CONFIG_TMP=%~dp0RTXMFG-Universal.json.tmp"
set "UI_CONFIG=%~dp0RTXMFG-UI.ini"
set "UI_TMP=%~dp0RTXMFG-UI.ini.tmp"

cls
echo ============================================================
echo   Victory - Portable DLSS Frame Generation Selector
echo ============================================================
echo.
echo This pack does not include the game or proprietary binaries.
echo Use your own working Victory, MFG Unlocker, and NVIDIA SDK setup.
echo.

"%SystemRoot%\System32\tasklist.exe" /fi "IMAGENAME eq victory.exe" | "%SystemRoot%\System32\find.exe" /i "victory.exe" >nul
if not errorlevel 1 (
    echo ERROR: Victory is already running. Close it before using this launcher.
    pause
    exit /b 1
)

if exist "%VICTORY%" goto victory_ready
echo Victory executable not found beside this launcher.
echo Enter the full path to your existing victory.exe.
set /p "VICTORY_SOURCE=Victory path: "
set "VICTORY_SOURCE=%VICTORY_SOURCE:"=%"
if not exist "%VICTORY_SOURCE%" (
    echo ERROR: That Victory executable was not found.
    pause
    exit /b 1
)
copy /y "%VICTORY_SOURCE%" "%VICTORY%" >nul
if errorlevel 1 (
    echo ERROR: Could not copy Victory into this folder.
    pause
    exit /b 1
)
:victory_ready

set "MFG_DLL=%LOCALAPPDATA%\victory\mfg-unlocker\MFGUnlocker.dll"
if not exist "%MFG_DLL%" (
    echo.
    echo MFG Unlocker not found. Downloading the verified v1.7.2 release...
    "%VICTORY%" get-mfg
    if errorlevel 1 (
        echo ERROR: MFG Unlocker could not be downloaded or verified.
        pause
        exit /b 1
    )
)
if not exist "%MFG_DLL%" (
    echo ERROR: MFG Unlocker is still missing after the download.
    pause
    exit /b 1
)

set "GAME="
if exist "%GAME_FILE%" set /p GAME=<"%GAME_FILE%"
if defined GAME set "GAME=%GAME:"=%"
if not defined GAME goto ask_game
if not exist "%GAME%" goto ask_game
goto game_ready

:ask_game
set "GAME="
echo.
echo Enter the full path to your game_scarlett_master.exe.
set /p "GAME=Game path: "
set "GAME=%GAME:"=%"
if not exist "%GAME%" (
    echo ERROR: That game executable was not found.
    pause
    exit /b 1
)
> "%GAME_FILE%" echo %GAME%

:game_ready
cls
echo ============================================================
echo   Victory - DLSS Frame Generation Selector
echo ============================================================
echo.
echo   [3] 3x - two generated frames per real frame
echo   [4] 4x - three generated frames per real frame
echo   [5] 5x - four generated frames per real frame
echo   [6] 6x - five generated frames per real frame
echo.
choice /c 3456 /n /m "Choose a multiplier: "

set "CHOICE=%ERRORLEVEL%"
if "%CHOICE%"=="4" goto selected_6
if "%CHOICE%"=="3" goto selected_5
if "%CHOICE%"=="2" goto selected_4
if "%CHOICE%"=="1" goto selected_3
goto cancelled

:selected_3
set "FG=3"
goto selected
:selected_4
set "FG=4"
goto selected
:selected_5
set "FG=5"
goto selected
:selected_6
set "FG=6"
goto selected

:selected
> "%CONFIG_TMP%" echo {"followGame":false,"mode":"fixed","multiplier":%FG%,"dynamicTargetFrameRate":0,"dlssgPreset":3,"dlssgDefaults":1,"vsyncMode":1,"reflexFrameLimitFps":0,"generatedOnlyDebug":false,"intervalLogging":true,"version":13,"xemfgStandalone":0,"xemfgFrames":4,"xemfgDynamic":0,"xemfgTargetFps":0,"xemfgRounding":1,"xemfgAllow1x":1,"xemfgTargetAsCap":0}
> "%UI_TMP%" echo [RTXMFG]
>> "%UI_TMP%" echo FollowGameMode=0
>> "%UI_TMP%" echo Multiplier=%FG%
>> "%UI_TMP%" echo DynamicMode=0
>> "%UI_TMP%" echo DynamicTargetFrameRate=0
>> "%UI_TMP%" echo DynamicLockToRefreshRate=1
>> "%UI_TMP%" echo DynamicCustomTargetFrameRate=60
>> "%UI_TMP%" echo GeneratedOnlyDebug=0
move /y "%CONFIG_TMP%" "%CONFIG%" >nul
move /y "%UI_TMP%" "%UI_CONFIG%" >nul

echo.
echo Starting Victory with %FG%x DLSS Frame Generation...
"%VICTORY%" play "%GAME%" --framegen %FG% --fg-type dlss --fps 1000 --set VICTORY_FRAMEGEN_X=%FG% --set VICTORY_MENU_FRAMEGEN=%FG% --set VICTORY_MENU_FPS=1000 --set VICTORY_FRAME_INTERVAL_US=0 --set VICTORY_MFG_UNLOCKER=1 --set VICTORY_WARMUP_RESTART=0
set "RESULT=%ERRORLEVEL%"
echo.
echo Victory exited with code %RESULT%.
echo Run START-HERE.cmd again to choose another multiplier.
pause
exit /b %RESULT%

:cancelled
echo.
echo No multiplier selected.
pause
exit /b 1
