#!/bin/bash

# Release preparation script for CLEVER KVM
# This script helps prepare a new release

set -e

# Check if version is provided
if [ $# -eq 0 ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 1.0.0"
    exit 1
fi

VERSION=$1

echo "[INFO] Preparing release v$VERSION..."

# Update package.json version
echo "[INFO] Updating package.json version..."
npm version $VERSION --no-git-tag-version

# Update Cargo.toml version
echo "[INFO] Updating Cargo.toml version..."
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" src-tauri/Cargo.toml
rm -f src-tauri/Cargo.toml.bak

# Update tauri.conf.json version
echo "[INFO] Updating tauri.conf.json version..."
sed -i.bak "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" src-tauri/tauri.conf.json
rm -f src-tauri/tauri.conf.json.bak

echo "[COMPLETE] Version updated to $VERSION"
echo ""
echo "[INFO] Next steps:"
echo "  1. Review the changes: git diff"
echo "  2. Commit the changes: git add . && git commit -m \"Release v$VERSION\""
echo "  3. Create and push the tag: git tag v$VERSION && git push origin v$VERSION"
echo "  4. The GitHub Actions workflow will automatically build and create the release"
