@echo off
setlocal
cd /d "%~dp0"

echo ======================================================================
echo                  HCS AIDER - Autonomous Coding Agent
echo ======================================================================
echo Targeting Local HCS Daemon: http://127.0.0.1:8787/v1 (hcs-coder)
echo.

if "%~1"=="" (
    echo Usage: hcs-aider.bat --task "Describe bugfix or feature" [--dir "path/to/repo"] [--thinking]
    echo.
    exit /b 1
)

set PYTHONPATH=%~dp0harnesses;%PYTHONPATH%
python -m hcs_aider %*
