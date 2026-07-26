@echo off
REM Builds a minimal static OpenCV 4.12.0 matching .cargo/config.toml:
REM   include -> third_party/opencv-static/include
REM   libs    -> third_party/opencv-static/x64/vc17/staticlib
REM   modules -> core, imgproc, features2d (+ static CRT, no DLLs)
setlocal

set "THIRD_PARTY=%~dp0"
set "CMAKE=%THIRD_PARTY%cmake\bin\cmake.exe"
set "SOURCE=%THIRD_PARTY%opencv-sdk\sources"
set "BUILD=%THIRD_PARTY%opencv-static-build"
set "INSTALL=%THIRD_PARTY%opencv-static"

"%CMAKE%" -S "%SOURCE%" -B "%BUILD%" -G "Visual Studio 17 2022" -A x64 ^
  -DCMAKE_INSTALL_PREFIX="%INSTALL%" ^
  -DCMAKE_BUILD_TYPE=Release ^
  -DBUILD_SHARED_LIBS=OFF ^
  -DBUILD_WITH_STATIC_CRT=ON ^
  -DOPENCV_INSTALL_BINARIES_PREFIX=x64/vc17/ ^
  -DBUILD_LIST=core,imgproc,features2d ^
  -DBUILD_opencv_apps=OFF ^
  -DBUILD_opencv_python_bindings_generator=OFF ^
  -DBUILD_opencv_python_tests=OFF ^
  -DBUILD_opencv_js_bindings_generator=OFF ^
  -DBUILD_opencv_objc_bindings_generator=OFF ^
  -DBUILD_JAVA=OFF ^
  -DBUILD_TESTS=OFF ^
  -DBUILD_PERF_TESTS=OFF ^
  -DBUILD_EXAMPLES=OFF ^
  -DBUILD_DOCS=OFF ^
  -DBUILD_PACKAGE=OFF ^
  -DBUILD_ZLIB=ON ^
  -DWITH_FFMPEG=OFF ^
  -DWITH_GSTREAMER=OFF ^
  -DWITH_MSMF=OFF ^
  -DWITH_DSHOW=OFF ^
  -DWITH_OPENCL=OFF ^
  -DWITH_CUDA=OFF ^
  -DWITH_IPP=OFF ^
  -DWITH_ITT=OFF ^
  -DWITH_TBB=OFF ^
  -DWITH_OPENEXR=OFF ^
  -DWITH_PROTOBUF=OFF ^
  -DWITH_QUIRC=OFF ^
  -DWITH_ADE=OFF ^
  -DWITH_EIGEN=OFF ^
  -DWITH_JPEG=OFF ^
  -DWITH_PNG=OFF ^
  -DWITH_TIFF=OFF ^
  -DWITH_WEBP=OFF ^
  -DWITH_OPENJPEG=OFF ^
  -DWITH_JASPER=OFF ^
  -DWITH_VTK=OFF ^
  -DWITH_1394=OFF ^
  -DWITH_LAPACK=OFF ^
  -DVIDEOIO_ENABLE_PLUGINS=OFF ^
  -DOPENCV_GENERATE_SETUPVARS=OFF ^
  -DINSTALL_CREATE_DISTRIB=OFF ^
  -DINSTALL_C_EXAMPLES=OFF ^
  -DINSTALL_PYTHON_EXAMPLES=OFF ^
  -DINSTALL_TESTS=OFF
if errorlevel 1 exit /b 1

"%CMAKE%" --build "%BUILD%" --config Release --target install --parallel 16
if errorlevel 1 exit /b 1

endlocal
