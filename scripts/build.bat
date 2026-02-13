@echo off
REM Build script for CLEVER KVM (Windows)
REM This script builds the KVM application with rdengine streaming

echo [INFO] Building CLEVER KVM with native WebM support...

REM Check if Node.js is installed
node --version >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Node.js is not installed. Please install Node.js first.
    exit /b 1
)

REM Check if Rust is installed
cargo --version >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] Rust is not installed. Please install Rust first.
    exit /b 1
)

REM Install dependencies
echo [INFO] Installing dependencies...
npm install

REM Build the application with VP9 hardware acceleration
echo [INFO] Building application with VP9 hardware-accelerated encoding...
npm run tauri:build

echo [COMPLETE] Build completed with VP9 hardware acceleration support!
echo.
echo [COMPLETE] Built files can be found in:
echo   - MSI Installer: src-tauri\target\release\bundle\msi\
echo   - NSIS Installer: src-tauri\target\release\bundle\nsis\
echo.
echo Ready to distribute with VP9 hardware-accelerated streaming!
