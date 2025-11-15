# MyriadMesh Android Application

This directory contains the Android application for MyriadMesh, a multi-network communication aggregation protocol with appliance integration.

## Overview

The MyriadMesh Android app provides:
- **Native Android UI** using Jetpack Compose
- **Appliance Integration** for pairing with home/office MyriadNode appliances
- **Message Caching** with priority-based delivery
- **Mesh Networking** via WiFi Direct, Bluetooth, and cellular
- **Background Service** for persistent mesh connectivity
- **Power Optimization** by offloading operations to paired appliances

## Architecture

The app follows Clean Architecture principles with the following layers:

```
app/src/main/kotlin/com/myriadmesh/android/
├── core/               # JNI bridge to Rust MyriadNode
├── data/              # Data sources and repositories
│   ├── local/         # Room database
│   └── remote/        # Retrofit API clients
├── domain/            # Business logic and models
│   ├── model/         # Domain models
│   ├── usecase/       # Use cases
│   └── repository/    # Repository interfaces
├── presentation/      # UI layer (Jetpack Compose)
│   ├── appliance/     # Appliance pairing and management
│   ├── dashboard/     # Main dashboard
│   ├── messages/      # Messaging UI
│   ├── settings/      # Settings and preferences
│   └── navigation/    # Navigation logic
└── service/           # Background services
```

## Prerequisites

### Required Software

1. **Android Studio** (Hedgehog 2023.1.1 or later)
   - Download from: https://developer.android.com/studio

2. **Android SDK** (API 26+)
   - Install via Android Studio SDK Manager

3. **Android NDK** (26.1.10909125 or later)
   - Install via Android Studio SDK Manager
   - Tools → SDK Manager → SDK Tools → NDK (Side by side)

4. **Rust** (1.70.0 or later)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

5. **Rust Android Targets**
   ```bash
   rustup target add aarch64-linux-android
   rustup target add armv7-linux-androideabi
   rustup target add i686-linux-android
   rustup target add x86_64-linux-android
   ```

### Environment Variables

Set the `ANDROID_NDK_HOME` environment variable to your NDK installation path:

```bash
# Find your NDK path (typically in Android SDK)
# Example paths:
# - Linux: $HOME/Android/Sdk/ndk/26.1.10909125
# - macOS: $HOME/Library/Android/sdk/ndk/26.1.10909125
# - Windows: C:\Users\<username>\AppData\Local\Android\Sdk\ndk\26.1.10909125

# Add to your shell profile (~/.bashrc, ~/.zshrc, etc.)
export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/26.1.10909125
```

## Building

### Step 1: Build Rust Libraries

The Android app requires native Rust libraries compiled for Android targets.

```bash
# From the android/ directory
cd android

# Build debug version
./build-rust.sh debug

# Or build release version
./build-rust.sh release
```

This will:
1. Cross-compile the Rust code for all Android ABIs (arm64-v8a, armeabi-v7a, x86, x86_64)
2. Copy the compiled `.so` libraries to `app/src/main/jniLibs/`

### Step 2: Build Android App

#### Using Android Studio (Recommended)

1. Open Android Studio
2. File → Open → Select the `android/` directory
3. Wait for Gradle sync to complete
4. Build → Make Project (or Ctrl+F9)

#### Using Command Line

```bash
cd android

# Debug build
./gradlew assembleDebug

# Release build
./gradlew assembleRelease
```

The APK will be generated at:
- Debug: `app/build/outputs/apk/debug/app-debug.apk`
- Release: `app/build/outputs/apk/release/app-release.apk`

### Step 3: Install on Device/Emulator

#### Via Android Studio

1. Connect your Android device or start an emulator
2. Run → Run 'app' (or Shift+F10)

#### Via Command Line

```bash
# Install debug APK
adb install app/build/outputs/apk/debug/app-debug.apk

# Install release APK
adb install app/build/outputs/apk/release/app-release.apk
```

## Development Workflow

### Rust Code Changes

When you modify Rust code in `crates/myriadmesh-android/`:

```bash
# Rebuild Rust libraries
cd android
./build-rust.sh debug

# Then rebuild the Android app
./gradlew assembleDebug
```

### Kotlin Code Changes

When you modify Kotlin code in `app/src/main/kotlin/`:

```bash
# Just rebuild the Android app
./gradlew assembleDebug
```

Or use Android Studio's instant run feature for faster iteration.

## Project Structure

```
android/
├── app/
│   ├── src/
│   │   ├── main/
│   │   │   ├── AndroidManifest.xml      # App manifest
│   │   │   ├── kotlin/                  # Kotlin source code
│   │   │   ├── res/                     # Android resources
│   │   │   └── jniLibs/                 # Native libraries (generated)
│   │   ├── test/                        # Unit tests
│   │   └── androidTest/                 # Instrumentation tests
│   ├── build.gradle.kts                 # App module Gradle config
│   └── proguard-rules.pro               # ProGuard rules
├── gradle/
│   └── wrapper/                         # Gradle wrapper
├── build.gradle.kts                     # Project Gradle config
├── settings.gradle.kts                  # Gradle settings
├── gradle.properties                    # Gradle properties
├── build-rust.sh                        # Rust build script
└── README.md                            # This file
```

## Testing

### Unit Tests

Run Kotlin unit tests:

```bash
./gradlew test
```

Or in Android Studio: Right-click on test file → Run Tests

### Instrumentation Tests

Run Android instrumentation tests on a device/emulator:

```bash
./gradlew connectedAndroidTest
```

## Troubleshooting

### Build Errors

**"ANDROID_NDK_HOME is not set"**
- Set the environment variable as described in Prerequisites
- Restart your terminal/IDE after setting it

**"linker 'aarch64-linux-android-clang' not found"**
- Ensure Android NDK is installed
- Verify `ANDROID_NDK_HOME` points to the correct NDK version
- Check that the NDK toolchain exists at `$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/`

**"Unable to load native library"**
- Ensure you ran `build-rust.sh` before building the Android app
- Check that `.so` files exist in `app/src/main/jniLibs/`
- Verify the ABI matches your device/emulator architecture

### Runtime Issues

**App crashes on startup**
- Check logcat for errors: `adb logcat | grep MyriadMesh`
- Verify all permissions are granted in Settings → Apps → MyriadMesh → Permissions

**Cannot discover appliances**
- Ensure location permission is granted (required for WiFi/Bluetooth)
- Check that the appliance is on the same local network
- Verify the appliance has appliance mode enabled in its config

## Configuration

### Default Configuration

The app looks for configuration in:
- `/data/data/com.myriadmesh.android/files/config/`

You can push a custom config via adb:

```bash
adb push config.yaml /sdcard/Download/myriadmesh-config.yaml
# Then import via app settings
```

### Appliance Pairing

To pair with an appliance:

1. Ensure the appliance is running with appliance mode enabled
2. Open the MyriadMesh app
3. Navigate to Appliances tab
4. Tap "Discover Appliances"
5. Select your appliance from the list
6. Scan the QR code displayed on the appliance's web UI
7. Wait for pairing approval (if required)

## Architecture Details

### JNI Bridge

The app uses JNI to communicate with the Rust MyriadNode core:

- **Kotlin Side**: `com.myriadmesh.android.core.MyriadNode`
- **Rust Side**: `crates/myriadmesh-android/src/lib.rs`

### Dependency Injection

The app uses Hilt for dependency injection:

- Modules: `com.myriadmesh.android.di.*`
- Application: `@HiltAndroidApp` on `MyriadMeshApplication`
- Activities/Fragments: `@AndroidEntryPoint`

### Navigation

Navigation is handled by Jetpack Compose Navigation:

- Navigation graph: `com.myriadmesh.android.presentation.navigation.MyriadMeshApp`
- Screens: Dashboard, Messages, Appliances, Settings

## API Documentation

### Appliance REST API

The app communicates with appliances via REST API:

- Base URL: `http://<appliance-ip>:3030/`
- API Interface: `com.myriadmesh.android.data.remote.ApplianceApi`
- Documentation: `docs/phase4/APPLIANCE_API_GUIDE.md`

### Key Endpoints

- `GET /api/appliance/info` - Get appliance information
- `POST /api/appliance/pair/request` - Initiate pairing
- `POST /api/appliance/pair/complete` - Complete pairing
- `GET /api/appliance/cache/retrieve` - Retrieve cached messages
- `POST /api/appliance/cache/delivered` - Mark messages as delivered

## Performance Optimization

### Battery Life

The app implements several battery optimization strategies:

1. **Offloading** - Heavy operations delegated to paired appliances
2. **Adaptive Intervals** - Heartbeat and sync intervals adjust based on battery level
3. **Power Profiles** - Automatic switching between High Performance, Balanced, and Power Saver modes
4. **Doze Compatibility** - Uses WorkManager for background tasks during Doze mode

### Network Efficiency

- Connection pooling with OkHttp
- Gzip compression for API requests
- Caching of appliance information
- Batch message retrieval

## Security

### Data Protection

- Session tokens stored in `EncryptedSharedPreferences`
- Crypto keys stored in Android Keystore
- TLS 1.3 for all network communications
- Certificate pinning for appliance connections
- No sensitive data in backups (see `backup_rules.xml`)

### Permissions

The app requests only necessary permissions:

- **Required**: Internet, Network State, WiFi State
- **Optional**: Location (for WiFi Direct/Bluetooth discovery)
- **Optional**: Camera (for QR code scanning)
- **Optional**: Notifications (for message alerts)

## Contributing

See the main project's [CONTRIBUTING.md](../CONTRIBUTING.md) for contribution guidelines.

## License

GPL-3.0-only - See [LICENSE](../LICENSE) for details.

## Support

For issues and questions:
- GitHub Issues: https://github.com/Societus/myriadmesh/issues
- Documentation: `docs/phase4/`

## Future Work

### Planned Features (Phase 4.5+)

- [ ] Complete MyriadNode JNI integration
- [ ] WiFi Direct adapter implementation
- [ ] Bluetooth Classic adapter
- [ ] Bluetooth LE adapter
- [ ] Advanced power management
- [ ] Multi-appliance support
- [ ] Location-based appliance selection
- [ ] Mesh routing through mobile device
- [ ] Offline message composition
- [ ] Message encryption UI
- [ ] Advanced QoS settings

### Known Limitations (Pre-Hardware Testing)

Since we don't have hardware appliances yet:
- Appliance discovery uses mock data
- Pairing flow is stubbed
- Message caching is simulated locally
- Background service is a placeholder

These will be completed once hardware appliances are available for testing.
