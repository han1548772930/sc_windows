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

REM Check PaddleOCR folder
if not exist "PaddleOCR-json_v1.4.1" (
    echo Error: PaddleOCR-json_v1.4.1 folder not found
    echo Please ensure PaddleOCR engine folder is in current directory
    echo.
    pause
    exit /b 1
)

echo All required files check passed
echo.

REM Clean old ZIP file
if exist "OCR_Screenshot_Tool_Complete.zip" del "OCR_Screenshot_Tool_Complete.zip"

REM Show files to be packaged
echo Files to be packaged:
echo   - target\release\sc_windows.exe
echo   - PaddleOCR-json_v1.4.1\
echo   - README.md
echo.

REM Create ZIP directly without temp folder
echo Creating ZIP package...
powershell -Command "Compress-Archive -Path 'target\release\sc_windows.exe','PaddleOCR-json_v1.4.1','README.md' -DestinationPath 'OCR_Screenshot_Tool_Complete.zip' -Force"

if exist "OCR_Screenshot_Tool_Complete.zip" (
    echo.
    echo ========================================
    echo Package created successfully!
    echo File: OCR_Screenshot_Tool_Complete.zip
    echo ========================================
    echo.
    
    REM Show ZIP file size
    for %%I in ("OCR_Screenshot_Tool_Complete.zip") do echo Size: %%~zI bytes
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
