@echo off
chcp 65001 >nul
setlocal
set "PYTHONIOENCODING=utf-8"
set "PYTHONUTF8=1"
set "HCS_STACK_DIR=E:\AI Serving Stack"
if not exist "%HCS_STACK_DIR%" (
    if exist "%~dp0harnesses" (
        set "HCS_STACK_DIR=%~dp0"
    ) else if exist "%LOCALAPPDATA%\Programs\HCS-Local-AI" (
        set "HCS_STACK_DIR=%LOCALAPPDATA%\Programs\HCS-Local-AI"
    )
)
set "PYTHONPATH=%HCS_STACK_DIR%\harnesses;%PYTHONPATH%"
python -m hcs_aider %*
