# BitChat Device Testing System

Real-world E2E testing with iOS simulators and Android emulators.

## Overview

The device testing system provides **real-world E2E testing** using actual iOS and Android apps:
- **Slow** - 10+ tests in hours
- **Non-deterministic** - Real network timing varies
- **Black-box** - Only observes external behavior
- **Real** - Actual OS, network stack, Bluetooth

## Usage

The device system is accessed through the unified simulator interface:

```bash
# Test with device clients (from simulator/ directory)
just test-scenario 02_messaging_basic ios,ios
just test-scenario 02_messaging_basic android,android
just test-scenario 02_messaging_basic ios,android

# Direct access for development
cd device
just execute 02_messaging_basic ios,ios
just setup
just cleanup
```

## Client Types

- **ios** - Real iOS app running in iOS Simulator
- **android** - Real Android app running in Android emulator

Both use actual compiled BitChat applications for realistic testing.

## Prerequisites

### iOS Testing
- **macOS** - Required for iOS Simulator
- **Xcode** - Required for iOS app building and simulation
- **iOS Simulator** - Comes with Xcode

### Android Testing
- **Android SDK** - Required for Android emulator
- **Java Development Kit** - Required for Android builds
- **Android emulator** - Part of Android SDK

The Nix environment provides most dependencies automatically.

## Architecture

```
device/
├── src/
│   ├── main.rs                 # CLI entry point
│   ├── orchestrator.rs         # Device lifecycle management
│   ├── config.rs               # Configuration management
│   └── lib.rs                  # Public API
├── vendored/
│   ├── bitchat-ios/            # iOS app source
│   └── bitchat-android/        # Android app source
└── Justfile                    # Build automation
```

## Development Commands

```bash
# Build and setup
just build                      # Build device testing system
just build-apps                 # Build iOS and Android apps
just build-ios                  # Build only iOS app
just build-android             # Build only Android app

# Environment checks
just setup                      # Setup emulator environment
just cleanup                   # Clean up emulators
just check-env                 # Check environment readiness
just check-ios                 # Check iOS environment
just check-android             # Check Android environment

# Legacy compatibility (deprecated)
just test-ios-ios              # Use unified interface instead
just test-android-android      # Use unified interface instead
just test-cross-platform       # Use unified interface instead
```

## App Management

The device system automatically manages iOS and Android applications:

### iOS App (Swift)
- Built from `vendored/bitchat-ios/`
- Uses Xcode project build system
- Installed to iOS Simulator automatically
- Supports multiple simulator instances

### Android App (Kotlin)
- Built from `vendored/bitchat-android/`
- Uses Gradle build system
- Installed to Android emulator automatically
- Supports multiple emulator instances

## Integration

The device testing system is integrated with the unified simulator interface. See the main [simulator README](../README.md) for complete usage instructions.

For fast protocol testing, see the [virtual testing system](../virtual/README.md).

## Troubleshooting

### iOS Issues
```bash
# Check Xcode installation
xcrun simctl list devices

# Check iOS build environment
just check-ios
```

### Android Issues
```bash
# Check Android SDK
adb devices

# Check Android environment
just check-android

# Check emulator status
cd vendored/bitchat-android && ./gradlew assembleDebug
```

### Environment Issues
```bash
# Full environment check
just check-env

# Clean and restart
just cleanup
just setup
```