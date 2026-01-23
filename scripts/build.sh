#!/bin/bash

# Build script for Clever KVM
# This script builds the VNC server application

set -e

echo "🚀 Building Clever KVM VNC server..."

# Check if Node.js is installed
if ! command -v node &> /dev/null; then
    echo "❌ Node.js is not installed. Please install Node.js first."
    exit 1
fi

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Please install Rust first."
    exit 1
fi

# Install dependencies
echo "📦 Installing dependencies..."
npm install

# Build the application with native WebM encoding
echo "🔨 Building application with VP8/Opus codecs..."
npm run tauri:build

echo "✅ Build completed with native WebM support!"
echo ""
echo "📁 Built files can be found in:"
echo "  - Linux: src-tauri/target/release/bundle/deb/ and src-tauri/target/release/bundle/appimage/"
echo "  - Windows: src-tauri/target/release/bundle/msi/ and src-tauri/target/release/bundle/nsis/"
echo "  - macOS: src-tauri/target/release/bundle/dmg/ and src-tauri/target/release/bundle/macos/"
echo ""
echo "🎉 Ready to distribute with native WebM streaming!"
