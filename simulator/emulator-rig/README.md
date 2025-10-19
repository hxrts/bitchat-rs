# BitChat Emulator Testing Rig

Real app black box testing infrastructure for BitChat iOS and Android applications using emulator automation.

## Overview

This testing rig treats real BitChat applications as black boxes and tests them through emulator automation, network interception, and UI automation. This approach provides the most realistic testing environment possible while maintaining complete isolation and repeatability.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Rust Orchestration Harness               │
├─────────────────────────────────────────────────────────────┤
│  iOS Simulator     │  Android AVD      │  Network Proxy     │
│  + simctl          │  + adb            │  + mitmproxy       │
├─────────────────────────────────────────────────────────────┤
│  Real BitChat iOS  │  Real BitChat     │  Traffic Capture   │
│  + Appium/XCUITest │  + Appium/UiAuto  │  + Analysis        │
└─────────────────────────────────────────────────────────────┘
```

## Features

- **Real App Testing**: Uses actual BitChat iOS and Android applications
- **Emulator Automation**: Manages iOS Simulator and Android AVD lifecycle
- **Network Interception**: Captures and analyzes network traffic with mitmproxy
- **UI Automation**: Controls apps via Appium WebDriver (XCUITest/UiAutomator2)
- **Cross-Platform Testing**: Tests iOS ↔ iOS, Android ↔ Android, and iOS ↔ Android
- **Nix-First**: Reproducible development environment with all dependencies

## Prerequisites

- **Nix**: Package manager for reproducible environments
- **macOS**: Required for iOS Simulator testing
- **Xcode**: Required for iOS Simulator and app installation
- **Android SDK**: Required for Android emulator testing
- **Node.js**: Required for Appium automation server

## Real App Sources

The testing rig uses actual BitChat applications from:
- **iOS**: https://github.com/permissionlesstech/bitchat (Swift)
- **Android**: https://github.com/permissionlesstech/bitchat-android (Kotlin)

These repositories are vendored in `vendored/` and automatically built as part of the testing process.

## Quick Start

1. **Enter development environment:**
   ```bash
   nix develop
   ```

2. **Check prerequisites:**
   ```bash
   just check
   ```

3. **Check iOS build environment:**
   ```bash
   bash build-ios-check.sh
   ```

4. **Check Android build environment:**
   ```bash
   bash build-android-check.sh
   ```

5. **Build real apps:**
   ```bash
   just build-apps
   ```

6. **Set up environment:**
   ```bash
   just setup
   ```

7. **Run iOS tests:**
   ```bash
   just test-ios
   ```

8. **Run Android tests:**
   ```bash
   just test-android
   ```

9. **Run full compatibility matrix:**
   ```bash
   just test-matrix
   ```

## Test Scenarios

### Individual Scenarios
- `deterministic-messaging`: Basic message exchange validation
- `transport-failover`: Network resilience testing
- `session-rekey`: Cryptographic session management
- `network-partition`: Network isolation testing

### Compatibility Matrix
- `ios-ios`: iOS ↔ iOS communication testing
- `android-android`: Android ↔ Android communication testing
- `ios-android`: Cross-platform communication testing

## Configuration

The testing rig is configured via `src/config.rs` with the following key settings:

```rust
pub struct TestConfig {
    pub ios: IosConfig,        // iOS Simulator settings
    pub android: AndroidConfig, // Android emulator settings
    pub network: NetworkConfig, // mitmproxy configuration
    pub appium: AppiumConfig,   // UI automation settings
    pub timing: TimingConfig,   // Timeout configurations
}
```

Configuration can be customized via:
- Environment variable: `BITCHAT_TEST_CONFIG`
- Configuration file: `BITCHAT_TEST_CONFIG_PATH`
- Default values in code

## Network Interception

The network proxy captures all BitChat-related traffic:

- **Automatic filtering**: Identifies BitChat protocol traffic
- **Deep packet inspection**: Analyzes Nostr relay communication
- **Performance metrics**: Latency, throughput, error rates
- **Export formats**: JSON, PCAP, HAR

## App Automation

UI automation is performed via Appium with platform-specific drivers:

### iOS (XCUITest)
- **Deep link launching**: `bitchat://` URL scheme
- **Accessibility identifiers**: For reliable element location
- **Simulator lifecycle**: Automatic device creation/cleanup

### Android (UiAutomator2)
- **Intent launching**: Deep link via intent system
- **Package management**: APK installation and app state
- **AVD lifecycle**: Automatic emulator management

## Commands Reference

```bash
# Environment
just setup           # Set up testing environment
just cleanup         # Clean up all emulators and state
just check           # Verify prerequisites
just check-ios       # Check iOS build environment
just check-android   # Check Android build environment
just health          # Complete health check

# Testing
just test-ios        # Run all iOS scenarios
just test-android    # Run all Android scenarios
just test-matrix     # Run full compatibility matrix

# Targeted Testing
just test-ios-ios           # iOS ↔ iOS only
just test-android-android   # Android ↔ Android only
just test-cross-platform    # iOS ↔ Android only

# Development
just build           # Build Rust harness
just build-apps      # Build real BitChat iOS and Android apps
just build-ios       # Build only iOS app
just build-android   # Build only Android app
just dev             # Start development session
just dev-test        # Quick development test
just logs            # View test logs

# Results
just export-results  # Export test results

# Android Emulator Management
just android-list-avds          # List available Android Virtual Devices
just android-create-avd         # Create new Android AVD for testing
just android-start-emulator     # Start Android emulator
just android-install-apk        # Install BitChat APK on emulator
just android-launch-app         # Launch BitChat app on emulator
just android-workflow           # Complete build → install → launch workflow
```

## Implementation Status

- ✓ **Rust orchestration harness**: Complete CLI and infrastructure
- ✓ **iOS Simulator automation**: simctl integration and lifecycle management
- ✓ **Android AVD automation**: adb integration and lifecycle management
- ✓ **Network proxy integration**: mitmproxy with BitChat filtering
- ✓ **Appium automation**: WebDriver integration for both platforms
- ✓ **Nix development environment**: Reproducible toolchain
- ✓ **Real app integration**: BitChat iOS/Android apps cloned and integrated
- ⏳ **Network traffic analysis**: Advanced protocol analysis
- ⏳ **Performance benchmarking**: Latency and throughput metrics

## Architecture Benefits

1. **Black Box Testing**: Tests real user-facing behavior
2. **No Source Dependencies**: Works with any BitChat implementation
3. **Realistic Network Conditions**: Uses actual network stacks
4. **Platform Isolation**: Each test runs in clean environment
5. **Deterministic Results**: Reproducible test conditions
6. **CI/CD Ready**: Headless operation for automated testing

## Development Notes

- The harness is designed to be extended with new test scenarios
- Network analysis can be customized for specific protocols
- Emulator configuration supports various device types and API levels
- All operations are logged for debugging and analysis
- Test results are exportable in multiple formats for integration

This approach provides comprehensive BitChat compatibility testing while maintaining the flexibility to test any real-world deployment scenario.