@echo off
setlocal enabledelayedexpansion

echo ========================================
echo     OCR Screenshot Tool Package
echo ========================================
echo.

REM Check if compiled
if not exist "target\release\sc_windows.exe" (
    echo Error: sc_windows.exe not found
    echo Please run: cargo build --release
    echo.
    pause
    exit /b 1
)

REM Check models folder
if not exist "models" (
    echo Error: models folder not found
    echo Please ensure OCR models folder is in current directory
    echo.
    pause
    exit /b 1
)

echo All required files check passed
echo.

REM Clean old ZIP file
if exist "sc_windows.zip" del "sc_windows.zip"

REM Show files to be packaged
echo Files to be packaged:
echo   - target\release\sc_windows.exe
echo   - models\
echo   - README.md
echo.

REM Create ZIP directly without temp folder
echo Creating ZIP package...
powershell -Command "Compress-Archive -Path 'target\release\sc_windows.exe','models','README.md' -DestinationPath 'sc_windows.zip' -Force"

if exist "sc_windows.zip" (
    echo.
    echo ========================================
    echo Package created successfully!
    echo File: sc_windows.zip
    echo ========================================
    echo.
    
    REM Show ZIP file size
    for %%I in ("sc_windows.zip") do echo Size: %%~zI bytes
    echo.
) else (
    echo.
    echo Error: Failed to create ZIP package
    echo.
    pause
    exit /b 1
)

echo Package completed successfully!
pause
