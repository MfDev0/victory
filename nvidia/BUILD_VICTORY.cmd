@echo off
rem VIctory NVIDIA - build victory.exe from this source. ASCII only.
rem Needs: Rust (https://rustup.rs) and the Visual Studio Build Tools with
rem "Desktop development with C++". rust-toolchain.toml picks Rust 1.98.1,
rem which rustup downloads by itself the first time.
rem   BUILD_VICTORY.cmd          release build, then copies victory.exe here
rem   BUILD_VICTORY.cmd test     also runs the unit tests of the launcher
rem It remembers where this project is. Built from a new clone in another
rem folder, it shows the old one and asks: replace it, or keep both.
setlocal EnableExtensions DisableDelayedExpansion
cd /d "%~dp0"
title VIctory - build
set "HERE=%~dp0"
set "HERE=%HERE:~0,-1%"
set "HERE_ROOT=%HERE%"
if exist "%HERE%\..\.git" for %%R in ("%HERE%\..") do set "HERE_ROOT=%%~fR"
set "HOME_DIR=%LOCALAPPDATA%\victory"
if defined VICTORY_HOME set "HOME_DIR=%VICTORY_HOME%"
if not exist "%HOME_DIR%" mkdir "%HOME_DIR%"
set "PROJECT_FILE=%HOME_DIR%\try-project.txt"
echo.
echo    /\_/\    VIctory NVIDIA Try build
echo   ( o.o )   by MfDev0 - the cat is watching
echo    ^> ^^ ^<    you compile. No pressure.
echo.
call :older
if errorlevel 1 (
  echo Cancelled: nothing was changed.
  pause
  exit /b 1
)
call :remember
where cargo >nul 2>nul
if errorlevel 1 (
  echo Rust is not installed or not on PATH.
  echo Install it from https://rustup.rs, then open a NEW window and run
  echo this file again. Also install the Visual Studio Build Tools with
  echo "Desktop development with C++".
  pause
  exit /b 1
)
echo =============================================
echo   Building victory.exe (release). The first
echo   build takes several minutes.
echo =============================================
cargo build --release --locked -p victory
if errorlevel 1 (
  echo.
  echo BUILD FAILED. Send the text above, from the first "error", back.
  pause
  exit /b 1
)
if /i "%~1"=="test" (
  cargo test --release --locked -p victory
  if errorlevel 1 echo TESTS FAILED: send the output back.
)
copy /y "target\release\victory.exe" "%~dp0victory.exe" >nul
echo.
echo Built: %~dp0victory.exe
echo The cat approves =^^.^^=
echo Next: TESTER_MENU.cmd (or put victory.exe beside your
echo game_scarlett_master.exe and double-click it).
echo NVIDIA's DLSS files download on the first start.
pause
exit /b 0

rem ---- an older copy of this project on this PC ----
:older
set "OLD="
if exist "%PROJECT_FILE%" set /p OLD=<"%PROJECT_FILE%"
rem Copies made before it remembered: the folder of the build notes.
if not defined OLD if exist "%USERPROFILE%\victory-try\nvidia\TRY-NOTES.md" set "OLD=%USERPROFILE%\victory-try\nvidia"
if not defined OLD exit /b 0
if /i "%OLD%"=="%HERE%" exit /b 0
if not exist "%OLD%\BUILD_VICTORY.cmd" exit /b 0
set "OLD_ROOT=%OLD%"
if exist "%OLD%\..\.git" for %%R in ("%OLD%\..") do set "OLD_ROOT=%%~fR"
set "OLD_VER=version not known"
set "NEW_VER=version not known"
call :version "%OLD%" OLD_VER
call :version "%HERE%" NEW_VER
echo =============================================
echo   AN OLDER COPY OF THIS PROJECT IS ON THIS PC
echo =============================================
echo   Old: %OLD_ROOT%
echo        %OLD_VER%
echo   New: %HERE_ROOT%
echo        %NEW_VER%
echo.
echo   1  Replace the old copy with this new one
echo      (the old folder is deleted. Kept: your
echo      settings, game path, saves, DLSS files)
echo   2  Keep both (this new one is used from now)
echo   3  Cancel
echo.
choice /c 123 /n /m "Pick 1, 2 or 3: "
if errorlevel 3 exit /b 1
if errorlevel 2 exit /b 0
call :replace
exit /b 0

rem The commit and its date (git), else when its victory.exe was built.
:version
if exist "%~1\victory.exe" for %%F in ("%~1\victory.exe") do set "%~2=victory.exe built %%~tF"
where git >nul 2>nul
if errorlevel 1 exit /b 0
for /f "delims=" %%G in ('git -C "%~1" log -1 "--date=short" "--format=commit %%h of %%cd" 2^>nul') do set "%~2=%%G"
exit /b 0

:replace
rem Never this folder or a folder it is in, never a drive, never your user
rem folders, and never a folder with the game in it.
if /i "%OLD_ROOT%"=="%HERE_ROOT%" goto :not_deleted
if "%OLD_ROOT:~-1%"=="\" goto :not_deleted
if /i "%OLD_ROOT%"=="%USERPROFILE%" goto :not_deleted
if /i "%OLD_ROOT%"=="%USERPROFILE%\Desktop" goto :not_deleted
if /i "%OLD_ROOT%"=="%USERPROFILE%\Documents" goto :not_deleted
if /i "%OLD_ROOT%"=="%USERPROFILE%\Downloads" goto :not_deleted
if not exist "%OLD%\TRY-NOTES.md" goto :not_deleted
set "REST=%HERE%"
call set "REST=%%REST:%OLD_ROOT%\=%%"
if /i not "%REST%"=="%HERE%" goto :not_deleted
dir /s /b /a-d "%OLD_ROOT%\game_scarlett_master.exe" >nul 2>nul
if not errorlevel 1 (
  echo   Your game is inside the old folder,
  echo   so it is NOT deleted. Move the game out,
  echo   then delete the old folder yourself.
  exit /b 0
)
call :running
if not errorlevel 1 (
  echo   victory.exe is running. Close it, then
  pause
)
call :running
if not errorlevel 1 goto :not_deleted
echo   Moving what the old copy downloaded and
echo   built into the new one...
if not exist "%HERE%\target" if exist "%OLD%\target\" move "%OLD%\target" "%HERE%\target" >nul 2>nul
for %%D in (D3D12 nvidia-sdk amd-sdk fidelityfx) do if exist "%OLD%\%%D\" if not exist "%HERE%\%%D\" call :carry %%D
echo   Deleting the old copy...
attrib -r "%OLD_ROOT%\*" /s /d >nul 2>nul
rd /s /q "%OLD_ROOT%" >nul 2>nul
if exist "%OLD_ROOT%" goto :partly_deleted
echo   Old copy deleted.
echo.
exit /b 0

:partly_deleted
echo   Some files could not be deleted (open in
echo   another program?). Delete it by hand:
echo   %OLD_ROOT%
echo.
exit /b 0

:not_deleted
echo   The old copy was NOT deleted (not safe to
echo   delete it from here). Both are kept.
echo.
exit /b 0

:carry
move "%OLD%\%~1" "%HERE%\%~1" >nul 2>nul
if exist "%HERE%\%~1\" exit /b 0
xcopy "%OLD%\%~1" "%HERE%\%~1\" /e /i /q /y >nul
exit /b 0

:running
"%SystemRoot%\System32\tasklist.exe" /fi "IMAGENAME eq victory.exe" | "%SystemRoot%\System32\find.exe" /i "victory.exe" >nul
exit /b

rem This folder is the project from now on.
:remember
setlocal EnableDelayedExpansion
> "!PROJECT_FILE!" echo(!HERE!
endlocal
exit /b 0
