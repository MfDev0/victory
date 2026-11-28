@echo off
rem VIctory NVIDIA - build victory.exe from this source. ASCII only.
rem Needs: Rust (https://rustup.rs) and the Visual Studio Build Tools with
rem "Desktop development with C++". rust-toolchain.toml picks Rust 1.98.1,
rem which rustup downloads by itself the first time.
rem   BUILD_VICTORY.cmd          release build, then copies victory.exe here
rem   BUILD_VICTORY.cmd test     also runs the unit tests of the launcher
setlocal EnableExtensions DisableDelayedExpansion
cd /d "%~dp0"
title VIctory - build
echo.
echo    /\_/\    VIctory NVIDIA Try build
echo   ( o.o )   by MfDev0 - the cat is watching
echo    ^> ^^ ^<    you compile. No pressure.
echo.
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
