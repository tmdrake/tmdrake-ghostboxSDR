@echo off
setlocal EnableExtensions
cd /d "%~dp0"

rem Explorer double-click does not include rustup.
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

where cargo >nul 2>&1
if errorlevel 1 (
  echo cargo not found. Install Rust from https://rustup.rs
  goto :fail
)

echo SHACKHACK - SLS SPIRIT BOX
echo.
cargo run --release -p shackhack -- %*
set "ERR=%ERRORLEVEL%"
if not "%ERR%"=="0" goto :fail
exit /b 0

:fail
if not defined ERR set "ERR=1"
echo.
echo ShackHack exited with error %ERR%.
pause
exit /b %ERR%
