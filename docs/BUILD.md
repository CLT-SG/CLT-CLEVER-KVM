# 📦 Building and Releasing Clever KVM

This guide covers how to build package installers for Clever KVM and deploy them as GitHub releases.

## 🔄 Auto-Updater Setup

Clever KVM includes built-in auto-updater functionality that automatically notifies users of new releases and allows them to update with a single click.

### Setting Up Auto-Updates

1. **Generate signing keys** (already done for you):
   ```bash
   npx tauri signer generate -w ~/.tauri/clever-kvm.key --password test123 --force
   ```

2. **Set up GitHub secrets** (required for releases):
   ```bash
   ./scripts/setup-github-secrets.sh
   ```
   
   This will show you the secrets you need to add to your GitHub repository:
   - `TAURI_PRIVATE_KEY`: Your private key for signing updates
   - `TAURI_KEY_PASSWORD`: Password for the private key

3. **Add the secrets to GitHub**:
   - Go to your repository → Settings → Secrets and variables → Actions
   - Add the two secrets shown by the setup script

### How Auto-Updates Work

- **Automatic checks**: The app checks for updates on startup
- **Manual checks**: Users can click "Check for Updates" in the Status tab
- **Secure updates**: All updates are cryptographically signed
- **User control**: Users can choose when to install updates
- **Seamless installation**: Updates are downloaded and installed with app restart

### Update Process

1. When you create a release (using tags), GitHub Actions will:
   - Build the application for all platforms
   - Sign the update packages
   - Generate a `latest.json` file with update metadata
   - Upload everything to the GitHub release

2. Users will see an update notification in the app when a new version is available

3. Users can install the update with a single click, and the app will restart automatically

## 🏗️ Local Building

### 🔧 Prerequisites

Before building, you need to install system dependencies including FFmpeg:

#### Linux (Ubuntu/Debian)
```bash
sudo apt-get update
sudo apt-get install -y build-essential curl wget file libssl-dev \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
    libxdo-dev libxcb-randr0-dev xdg-desktop-portal libpipewire-0.3-dev libopus-dev \
    libavformat-dev libavcodec-dev libavutil-dev libswscale-dev libswresample-dev \
    patchelf libxrandr-dev
```

#### macOS
```bash
# Install Homebrew if you haven't already: https://brew.sh/
brew install ffmpeg pkg-config
```

#### Windows
```batch
# Install vcpkg
git clone https://github.com/Microsoft/vcpkg.git C:\vcpkg
cd C:\vcpkg
.\bootstrap-vcpkg.bat
.\vcpkg.exe integrate install
# Install FFmpeg with all components
.\vcpkg.exe install ffmpeg[avcodec,avformat,avdevice,avfilter,swresample,swscale]:x64-windows
# Set environment variables
set VCPKG_ROOT=C:\vcpkg
set PKG_CONFIG_PATH=C:\vcpkg\installed\x64-windows\lib\pkgconfig
```

### 🩺 FFmpeg Troubleshooting

If you encounter FFmpeg-related build errors, use our troubleshooting script:

#### Linux/macOS:
```bash
./scripts/fix-ffmpeg.sh
```

#### Windows:
```batch
scripts\fix-ffmpeg.bat
```

The script will:
- Check if FFmpeg and pkg-config are installed
- Install missing dependencies
- Set required environment variables
- Test the build process

### 🛠️ Common FFmpeg Issues

**Error**: `The system library 'libavutil' required by crate 'ffmpeg-sys-next' was not found`

**Solution**:
1. Install FFmpeg development libraries (see prerequisites above)
2. Set environment variables:
   ```bash
   export PKG_CONFIG_ALLOW_SYSTEM_LIBS=1
   export PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1
   ```
3. Run the troubleshooting script for your platform

### Prerequisites

- **Node.js** (v16 or later)
- **Rust** (latest stable)
- **Platform-specific dependencies:**
  - **Linux**: `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`, `libgtk-3-dev`, `libxdo-dev`, `libxrandr-dev`
  - **Windows**: Visual Studio Build Tools
  - **macOS**: Xcode Command Line Tools

### Quick Build

```bash
# Make the build script executable (Linux/macOS)
chmod +x scripts/build.sh

# Run the build script
./scripts/build.sh

# For Windows
scripts\build.bat
```

### Manual Build Steps

1. **Install dependencies:**
   ```bash
   npm install
   ```

2. **Build the application:**
   ```bash
   npm run tauri:build
   ```

3. **Build artifacts will be created in:**
   - **Linux**: 
     - `src-tauri/target/release/bundle/deb/*.deb` (Debian package)
     - `src-tauri/target/release/bundle/appimage/*.AppImage` (AppImage)
   - **Windows**: 
     - `src-tauri/target/release/bundle/msi/*.msi` (MSI installer)
     - `src-tauri/target/release/bundle/nsis/*.exe` (NSIS installer)
   - **macOS**: 
     - `src-tauri/target/release/bundle/dmg/*.dmg` (DMG image)
     - `src-tauri/target/release/bundle/macos/*.app` (App bundle)

## 🚀 Automated GitHub Releases

### Setting Up Automated Releases

1. **Ensure your repository has the GitHub Actions workflows** (already included)
2. **Set up repository secrets** (if needed for code signing)

### Creating a Release

#### Method 1: Version Tag (Recommended)

1. **Prepare the release:**
   ```bash
   ./scripts/prepare-release.sh 1.0.0
   ```

2. **Review and commit changes:**
   ```bash
   git diff
   git add .
   git commit -m "Release v1.0.0"
   ```

3. **Create and push the tag:**
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

4. **The GitHub Actions workflow will automatically:**
   - Build for all platforms (Windows, macOS, Linux)
   - Create installers/packages
   - Create a GitHub release
   - Upload all artifacts to the release

#### Method 2: Manual Workflow Trigger

1. Go to your repository on GitHub
2. Navigate to **Actions** → **Build and Release**
3. Click **Run workflow**
4. Enter the version tag (e.g., `v1.0.0`)
5. Click **Run workflow**

### Release Artifacts

Each release will include:

- **Windows**:
  - `clever-kvm_1.0.0_x64.msi` - MSI installer
  - `clever-kvm_1.0.0_x64-setup.exe` - NSIS installer

- **macOS**:
  - `clever-kvm_1.0.0_universal.dmg` - Universal DMG (Intel + Apple Silicon)

- **Linux**:
  - `clever-kvm_1.0.0_amd64.deb` - Debian package
  - `clever-kvm_1.0.0_amd64.AppImage` - AppImage

## 🔧 Development Building

For testing and development, you can use the manual build workflow:

1. Go to **Actions** → **Manual Build**
2. Select the platform to build for
3. Choose debug or release mode
4. Download the artifacts from the workflow run

## 📋 Platform-Specific Notes

### Linux Dependencies

Before building on Linux, install the required dependencies:

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install -y build-essential curl wget file libssl-dev \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
  libxdo-dev libxcb-randr0-dev xdg-desktop-portal libpipewire-0.3-dev libopus-dev \
  libavformat-dev libavcodec-dev libavutil-dev libswscale-dev libswresample-dev \
  patchelf libxrandr-dev

# Fedora/RHEL
sudo dnf install webkit2gtk4.1-devel libayatana-appindicator-gtk3-devel \
  librsvg2-devel gtk3-devel libxdo-devel libXrandr-devel pipewire-devel opus-devel
```

### Windows Code Signing

To enable code signing for Windows builds:

1. Add your certificate to repository secrets
2. Update the workflow with signing configuration
3. Set `certificateThumbprint` in `tauri.conf.json`

### macOS Code Signing

For macOS code signing and notarization:

1. Add Apple Developer certificates to repository secrets
2. Configure signing identity in `tauri.conf.json`
3. Set up notarization credentials

## 🔍 Troubleshooting

### Common Issues

1. **Build fails with "command not found"**
   - Ensure all prerequisites are installed
   - Check that Rust and Node.js are in your PATH

2. **Linux build fails with library errors**
   - Install all required system dependencies
   - Update your package manager first

3. **Windows build fails with MSBuild errors**
   - Install Visual Studio Build Tools
   - Ensure Windows SDK is installed

4. **macOS build fails with signing errors**
   - Set up proper Apple Developer certificates
   - Configure signing identity correctly

### Getting Help

- Check the [Tauri documentation](https://tauri.app/v1/guides/building/)
- Review GitHub Actions logs for detailed error messages
- Open an issue in the repository for specific problems

## 🎯 Distribution

After building, you can distribute the installers:

- **Windows**: Share the `.msi` or `.exe` installer files
- **macOS**: Share the `.dmg` file
- **Linux**: Share the `.deb` package or `.AppImage` file

Users can then install the application using their platform's standard installation methods.
