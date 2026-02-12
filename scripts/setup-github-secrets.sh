#!/bin/bash

# Script to help set up GitHub secrets for Tauri auto-updater
# This script generates the commands you need to run to set up GitHub secrets

set -e

echo "[INFO] Setting up GitHub Secrets for Tauri Auto-Updater"
echo "=================================================="
echo ""

# Check if private key exists
if [ ! -f "$HOME/.tauri/clever-kvm.key" ]; then
    echo "[ERROR] Private key not found at $HOME/.tauri/clever-kvm.key"
    echo "Please run the following command first:"
    echo "  npx tauri signer generate -w ~/.tauri/clever-kvm.key --password Enter_Password --force"
    exit 1
fi

echo "[COMPLETE] Private key found at $HOME/.tauri/clever-kvm.key"
echo ""

# Read the private key content
PRIVATE_KEY_CONTENT=$(cat "$HOME/.tauri/clever-kvm.key")

echo "[INFO] GitHub Secrets Setup Commands"
echo "================================"
echo ""
echo "You need to add the following secrets to your GitHub repository:"
echo ""
echo "1. TAURI_PRIVATE_KEY"
echo "   Value: (copy the entire private key content below)"
echo "   -------- BEGIN PRIVATE KEY --------"
echo "$PRIVATE_KEY_CONTENT"
echo "   -------- END PRIVATE KEY --------"
echo ""
echo "2. TAURI_KEY_PASSWORD"
echo "   Value: Enter_Password"
echo ""

echo "[INFO] How to add these secrets:"
echo "============================"
echo ""
echo "1. Go to your GitHub repository: https://github.com/CLTSG/CLT-CLEVER-KVM"
echo "2. Click on 'Settings' tab"
echo "3. Click on 'Secrets and variables' → 'Actions' in the left sidebar"
echo "4. Click 'New repository secret'"
echo "5. Add 'TAURI_PRIVATE_KEY' with the private key content above"
echo "6. Add 'TAURI_KEY_PASSWORD' with value: Enter_Password"
echo ""

echo "[WARNING] Security Note:"
echo "=================="
echo "- Keep the private key secure - never share it publicly"
echo "- The private key is needed to sign updates"
echo "- If you lose the private key or password, you won't be able to release updates"
echo ""

echo "[COMPLETE] After adding the secrets, your GitHub Actions will automatically:"
echo "- Sign update packages during release"
echo "- Generate the latest.json file for auto-updates"
echo "- Users will receive update notifications in the app"
echo ""

echo "[INFO] Test the setup:"
echo "=================="
echo "1. Create a test release: ./scripts/prepare-release.sh 0.2.0"
echo "2. Commit and tag: git add . && git commit -m 'Test release' && git tag v0.2.0"
echo "3. Push: git push origin v0.2.0"
echo "4. Check GitHub Actions for successful build and signing"
