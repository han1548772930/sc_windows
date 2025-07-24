@echo off
setlocal enabledelayedexpansion
echo ========================================
echo     OCR 截图工具 - 最终打包脚本
echo ========================================

REM 编译 Release 版本
echo 1. 编译 Release 版本...
cargo build --release
if %ERRORLEVEL% neq 0 (
    echo ❌ 编译失败！
    pause
    exit /b 1
)
echo ✅ 编译完成

REM 检查必要文件
echo 2. 检查必要文件...
if not exist "target\release\sc_windows.exe" (
    echo ❌ 错误: 找不到编译后的程序文件
    echo    请确保编译成功
    pause
    exit /b 1
)

if exist "PaddleOCR-json_v1.4.exe\PaddleOCR-json.exe" (
    echo ✅ PaddleOCR 文件夹已存在
) else (
    echo ❌ 错误: 未找到 PaddleOCR-json_v1.4.exe 文件夹
    echo    请确保 PaddleOCR 文件夹在当前目录中
    pause
    exit /b 1
)

REM 创建使用说明
echo 3. 创建使用说明...
(
echo OCR 截图工具 v1.0 - 最终版本
echo ================================
echo.
echo 📋 使用方法:
echo 1. 双击运行 sc_windows.exe
echo 2. 使用快捷键进行截图和OCR识别
echo 3. 识别结果会显示在弹出窗口中
echo.
echo 📁 文件说明:
echo - sc_windows.exe: 主程序 ^(约3.5MB^)
echo - PaddleOCR-json_v1.4.exe/: OCR引擎及所有依赖文件
echo   └── PaddleOCR-json.exe: OCR识别引擎
echo   └── *.dll: 必要的动态链接库
echo   └── models/: OCR识别模型文件
echo.
echo ⚠️  重要提醒:
echo - 请保持文件夹结构完整
echo - 首次运行可能需要管理员权限
echo - 确保有足够的磁盘空间用于临时文件
echo.
echo 🔧 系统要求:
echo - Windows 10/11 ^(64位^)
echo - .NET Framework ^(通常系统自带^)
echo.
echo 📞 技术支持:
echo 如有问题，请检查 PaddleOCR 相关文件是否完整
) > "README.txt"
echo ✅ 使用说明已创建

REM 创建完整的 ZIP 压缩包
echo 4. 创建完整的 ZIP 压缩包...
REM 先清理旧的临时目录和ZIP文件
if exist "temp_package" rmdir /s /q "temp_package"
if exist "OCR_Screenshot_Tool_Complete.zip" del "OCR_Screenshot_Tool_Complete.zip"
mkdir "temp_package"

REM 直接复制主程序
echo    - 复制主程序...
copy "target\release\sc_windows.exe" "temp_package\" >nul
echo ✅ 主程序已复制

REM 复制使用说明
copy "README.txt" "temp_package\" >nul
echo ✅ 使用说明已复制

REM 复制PaddleOCR文件夹
echo    - 复制 PaddleOCR 引擎...
xcopy "PaddleOCR-json_v1.4.exe" "temp_package\PaddleOCR-json_v1.4.exe" /E /I /H /Y >nul
echo ✅ PaddleOCR 引擎已复制

REM 显示打包内容
echo    - 打包内容预览:
dir "temp_package" /B

REM 创建ZIP
echo    - 正在压缩文件...
powershell -Command "Compress-Archive -Path 'temp_package\*' -DestinationPath 'OCR_Screenshot_Tool_Complete.zip' -Force" 2>nul

REM 清理临时目录和临时README
rmdir /s /q "temp_package"
del "README.txt" 2>nul
if exist "OCR_Screenshot_Tool_Complete.zip" (
    echo ✅ 完整 ZIP 文件已创建
) else (
    echo ❌ ZIP 创建失败
)

echo.
echo ========================================
echo 🎉 打包完成！
echo ========================================
echo.
echo 📦 分发文件:
if exist "OCR_Screenshot_Tool_Complete.zip" (
    for %%f in ("OCR_Screenshot_Tool_Complete.zip") do (
        set size=%%~zf
        set /a sizeMB=!size!/1024/1024
        echo    ✅ 完整ZIP包: %%~nxf ^(!sizeMB! MB^)
    )
) else (
    echo    ❌ ZIP文件创建失败
)
echo    📁 源文件位置: target\release\sc_windows.exe + PaddleOCR-json_v1.4.exe\
echo.
echo 📋 ZIP包内容:
echo - sc_windows.exe ^(主程序^)
echo - PaddleOCR-json_v1.4.exe\ ^(OCR引擎文件夹^)
echo - README.txt ^(使用说明^)
echo.
echo 📋 分发说明:
echo - 推荐使用 ZIP 压缩包进行分发
echo - 用户解压后直接运行 sc_windows.exe
echo - 无需额外安装，开箱即用
echo - 首次运行会自动启动OCR引擎
echo.
pause
