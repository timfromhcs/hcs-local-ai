@echo off
setlocal enabledelayedexpansion

echo ======================================================================
echo           HCS Local AI v5.0.0 Stable - Starting Server
echo ======================================================================
echo.

:: Locate daemon binary
set "DAEMON_EXE="
if exist "%~dp0hcs-daemon.exe" (
    set "DAEMON_EXE=%~dp0hcs-daemon.exe"
) else if exist "%~dp0target\release\hcs-daemon.exe" (
    set "DAEMON_EXE=%~dp0target\release\hcs-daemon.exe"
) else if exist "%~dp0target\debug\hcs-daemon.exe" (
    set "DAEMON_EXE=%~dp0target\debug\hcs-daemon.exe"
)

if "%DAEMON_EXE%"=="" (
    echo [ERROR] Could not find hcs-daemon.exe.
    echo Please build the project with "cargo build --release" or place hcs-daemon.exe in this folder.
    pause
    exit /b 1
)

:: Check if already running
curl -s -m 2 http://127.0.0.1:8787/hcs/v1/system >nul 2>&1
if %errorlevel% equ 0 (
    echo [INFO] HCS Local AI Daemon is already active and healthy!
    goto open_browser
)

echo [INFO] Starting daemon from: !DAEMON_EXE!
start "HCS Local AI Daemon" /MIN "!DAEMON_EXE!" run

echo [INFO] Waiting for server to initialize on http://127.0.0.1:8787...

set /a attempts=0
:wait_loop
set /a attempts+=1
if !attempts! geq 30 (
    echo [WARNING] Server initialization took longer than expected.
    echo Please check the daemon terminal window or logs if the UI does not load.
    goto open_browser
)

ping 127.0.0.1 -n 2 >nul
curl -s -m 2 http://127.0.0.1:8787/hcs/v1/system >nul 2>&1
if %errorlevel% neq 0 (
    goto wait_loop
)

echo [OK] Daemon is online and healthy!

:open_browser
echo.
echo ======================================================================
echo  Dashboard:           http://127.0.0.1:8787/
echo  OpenAI API:          http://127.0.0.1:8787/v1
echo  Anthropic API:       http://127.0.0.1:8787/v1/messages
echo  J-Space / Decider:   http://127.0.0.1:8787/hcs/v2/jspace
echo  HCS Aider CLI:       hcsaider (run in ANY directory!)
echo ======================================================================
echo.
echo Opening browser dashboard...
start http://127.0.0.1:8787/

echo.
echo To shut down cleanly at any time, run: stop.bat
echo.
exit /b 0
