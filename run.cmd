@echo off
setlocal EnableExtensions
cd /d "%~dp0"

rem Explorer double-click does not include rustup.
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set "EXE=%~dp0target\release\shackhack.exe"

if /I "%~1"=="--help" goto console
if /I "%~1"=="-h" goto console
if /I "%~1"=="--list" goto console
if /I "%~1"=="--setup-driver" goto console

if not exist "%EXE%" call :build
if not exist "%EXE%" goto fail

rem Drop the trailing slash so start does not treat \" as an escaped quote.
set "ROOT=%~dp0"
if "%ROOT:~-1%"=="\" set "ROOT=%ROOT:~0,-1%"

echo Opening SHACKHACK window...
rem First quoted arg is the console title, not the program.
start "SHACKHACK" /D "%ROOT%" "%EXE%" %*
if errorlevel 1 goto fail
exit /b 0

:console
where cargo >nul 2>&1
if errorlevel 1 goto nocargo
cargo run --release -p shackhack -- %*
set "ERR=%ERRORLEVEL%"
if not "%ERR%"=="0" goto fail
exit /b 0

:build
where cargo >nul 2>&1
if errorlevel 1 goto nocargo
echo Building the release. The window opens when this finishes.
cargo build --release -p shackhack
exit /b 0

:nocargo
echo cargo not found. Install Rust from https://rustup.rs
goto fail

:fail
echo.
echo ShackHack did not start.
pause
exit /b 1
