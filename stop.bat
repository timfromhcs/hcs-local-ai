@echo off
setlocal

echo ======================================================================
echo           HCS Local AI v5.0.2 Stable - Stopping Server
echo ======================================================================
echo.

echo [INFO] Stopping any running inference workers and daemons...

:: Kill daemon process
taskkill /F /IM hcs-daemon.exe >nul 2>&1
if %errorlevel% equ 0 (
    echo [OK] Stopped hcs-daemon.exe
) else (
    echo [INFO] hcs-daemon.exe was not running.
)

:: Kill any active llama-server worker processes
taskkill /F /IM llama-server.exe >nul 2>&1
if %errorlevel% equ 0 (
    echo [OK] Stopped llama-server.exe (Vulkan inference worker)
) else (
    echo [INFO] llama-server.exe was not running.
)

:: Kill any active stable-diffusion worker processes
taskkill /F /IM sd-cli.exe >nul 2>&1
if %errorlevel% equ 0 (
    echo [OK] Stopped sd-cli.exe
) else (
    echo [INFO] sd-cli.exe was not running.
)

echo.
echo [SUCCESS] All HCS Local AI services stopped and unified memory freed.
echo ======================================================================
ping 127.0.0.1 -n 2 >nul
exit /b 0
