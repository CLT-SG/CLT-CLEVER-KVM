#!/bin/bash

# Build script for Clever KVM
# This script builds the VNC server application

set -e

echo "[INFO] Building Clever KVM with native WebM support..."

# Check if Node.js is installed
if ! command -v node &> /dev/null; then
    echo "[ERROR] Node.js is not installed. Please install Node.js first."
    exit 1
fi

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "[ERROR] Rust is not installed. Please install Rust first."
    exit 1
fi

# Install dependencies
echo "[INFO] Installing dependencies..."
npm install

# Build the application with VP9 hardware acceleration
echo "[PROGRESS] Building application with VP9 hardware-accelerated encoding..."
npm run tauri:build

echo "[COMPLETE] Build completed with VP9 hardware acceleration support!"
echo ""
echo "Built files can be found in:"
echo " - Linux: src-tauri/target/release/bundle/deb/ and src-tauri/target/release/bundle/appimage/"
echo " - Windows: src-tauri/target/release/bundle/msi/ and src-tauri/target/release/bundle/nsis/"
echo " - macOS: src-tauri/target/release/bundle/dmg/ and src-tauri/target/release/bundle/macos/"
echo ""
echo "Ready to distribute with VP9 hardware-accelerated streaming!"
