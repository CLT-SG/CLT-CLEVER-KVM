# 🔄 CLEVER KVM Auto-Updater Guide

This guide covers the complete setup and usage of the auto-updater functionality in CLEVER KVM.

## ✨ Features

- **Automatic Update Detection**: App checks for updates on startup
- **Manual Update Checks**: Users can manually check for updates
- **Secure Updates**: All updates are cryptographically signed
- **User-Friendly Interface**: Simple update dialog with progress indication
- **Background Downloads**: Updates download in the background
- **One-Click Installation**: Updates install with a single click and app restart

## 🔧 Setup for Developers

### 1. Keys are Already Generated

The signing keys have been generated and are located at:
- Private key: `~/.tauri/clever-kvm.key` 
- Public key: `~/.tauri/clever-kvm.key.pub`
- Password: `test123`

### 2. Configure GitHub Secrets

Run the setup script to see what secrets need to be added:

```bash
./scripts/setup-github-secrets.sh
```

Then add these secrets to your GitHub repository:
- Go to `Settings` → `Secrets and variables` → `Actions`
- Add `TAURI_PRIVATE_KEY` with the private key content
- Add `TAURI_KEY_PASSWORD` with value `test123`

### 3. Configuration Files

The auto-updater is configured in:

**src-tauri/tauri.conf.json**:
```json
{
  "updater": {
    "active": true,
    "endpoints": [
      "https://github.com/CLTSG/CLT-CLEVER-KVM/releases/latest/download/latest.json"
    ],
    "dialog": true,
    "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDVDRTRCOEZENEM5ODUwQTYKUldTbVVKaE0vYmprWEZEU0F4aVFOcUZHSW9VTjZ3eXRHYmlXZDBhamkrdnZVY0hsOHNLMmI2UGwK"
  }
}
```

**src-tauri/Cargo.toml**:
```toml
[dependencies]
tauri = { version = "1.5", features = ["updater", ...] }
```

## 🚀 How It Works

### For Users

1. **Automatic Checks**: When the app starts, it automatically checks for updates
2. **Manual Checks**: Users can click "Check for Updates" in the Status tab
3. **Update Notification**: When an update is available, a dialog appears
4. **Download**: Users can choose to download the update
5. **Installation**: After download, users can restart to install the update

### For Developers

1. **Release Creation**: When you push a version tag (e.g., `v1.0.0`), GitHub Actions:
   - Builds the app for all platforms
   - Signs the update packages using your private key
   - Creates installers (.msi, .dmg, .deb, .AppImage)
   - Generates a `latest.json` file with update metadata
   - Uploads everything to the GitHub release

2. **Update Detection**: The app periodically checks the endpoint URL for new versions

3. **Signature Verification**: Updates are verified using the public key before installation

## 📋 Creating Releases with Auto-Updates

### Method 1: Automated Release (Recommended)

```bash
# Prepare new version
./scripts/prepare-release.sh 1.0.0

# Commit and tag
git add .
git commit -m "Release v1.0.0"
git tag v1.0.0
git push origin v1.0.0
```

### Method 2: Manual GitHub Actions

1. Go to your repository on GitHub
2. Click **Actions** → **Build and Release**  
3. Click **Run workflow**
4. Enter the version tag (e.g., `v1.0.0`)

## 🧪 Testing the Auto-Updater

### Local Testing

```bash
# Test the updater configuration and build
./scripts/test-updater.sh
```

### Testing Update Flow

1. **Build with current version** (e.g., 1.0.0):
   ```bash
   npm run tauri:build
   ```

2. **Create a test release** (e.g., 0.1.1):
   ```bash
   ./scripts/prepare-release.sh 0.1.1
   git add . && git commit -m "Test release" && git tag v0.1.1
   git push origin v0.1.1
   ```

3. **Test the update**:
   - Run the 1.0.0 version locally
   - Click "Check for Updates" in the Status tab
   - The app should detect version 0.1.1 and offer to update

## 🔍 Troubleshooting

### Common Issues

1. **"No updates available" when there should be**:
   - Check that the GitHub release contains the `latest.json` file
   - Verify the endpoint URL in `tauri.conf.json` matches your repository
   - Ensure the release is published (not draft)

2. **"Update verification failed"**:
   - Check that the GitHub secrets are set correctly
   - Verify the public key in `tauri.conf.json` matches your private key
   - Ensure the private key password is correct

3. **Updates not downloading**:
   - Check network connectivity
   - Verify the release assets are publicly accessible
   - Check browser dev tools for network errors

### Debug Information

The app logs update-related information to the console. Check:
- Browser dev tools (F12) → Console tab
- Look for messages starting with "Checking for updates..."

### Manual Verification

You can manually verify the update endpoint:
```bash
curl -s https://github.com/CLTSG/CLT-CLEVER-KVM/releases/latest/download/latest.json
```

This should return JSON with version information.

## 🔐 Security Considerations

- **Private Key Security**: Never commit the private key to version control
- **GitHub Secrets**: Store the private key in GitHub secrets, not in code
- **Key Rotation**: If the private key is compromised, generate a new one and update all releases
- **Verification**: The app only installs signed updates, preventing malicious updates

## 📊 Update Metadata

The `latest.json` file contains:
```json
{
  "version": "1.0.0",
  "notes": "Release notes here",
  "pub_date": "2025-01-01T00:00:00Z",
  "platforms": {
    "darwin-x86_64": {
      "signature": "...",
      "url": "https://github.com/.../releases/download/v1.0.0/app.app.tar.gz"
    },
    "darwin-aarch64": { ... },
    "linux-x86_64": { ... },
    "windows-x86_64": { ... }
  }
}
```

## 🎯 Best Practices

1. **Version Naming**: Use semantic versioning (e.g., v1.2.3)
2. **Release Notes**: Include meaningful release notes in GitHub releases
3. **Testing**: Test updates on all target platforms
4. **Backup Keys**: Keep secure backups of your signing keys
5. **Regular Updates**: Release updates regularly to keep users secure

## 📞 Support

If you encounter issues with the auto-updater:
1. Check this guide for common solutions
2. Review the GitHub Actions logs for build errors
3. Test with the provided scripts
4. Open an issue with detailed error messages and logs

---

The auto-updater is now fully configured and ready to provide seamless updates to your users! 🎉
