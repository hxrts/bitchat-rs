# BitChat Integration Simulator

Cross-platform testing framework for the BitChat protocol with real mobile app testing capabilities.

## Overview

The BitChat simulator provides two complementary testing frameworks:

### 1. emulator-rig - Real Mobile App Testing
Black-box testing of actual BitChat mobile applications:
- **iOS (Swift)** - Native Swift implementation in `vendored/bitchat-ios/`
- **Android (Kotlin)** - Native Kotlin implementation in `vendored/bitchat-android/`
- Runs apps in iOS Simulator and Android Emulator with Appium automation
- Network analysis and protocol compliance validation

### 2. scenario-runner - Simulation-Based Protocol Testing
Event-driven protocol testing with mock simulation:
- **CLI (Rust)** - Native Rust command-line client
- **Web (WASM)** - Browser-based WebAssembly client *(WASM build fixed 2025-01-20)*
- TOML-configured scenarios with network condition simulation
- Deterministic, reproducible test execution
- Cross-implementation testing (CLI ↔ Web)
- **Unified Client Bridge** - Bridges to emulator-rig for cross-framework testing *(New 2025-01-20)*

## Quick Start

### iOS Setup

1. **Install Xcode**: Download from the [Mac App Store](https://apps.apple.com/us/app/xcode/id497799835)
2. **Install Command Line Tools**:
   ```bash
   xcode-select --install
   ```
3. **Configure Signing** (for simulator testing, automatic signing works):
   - Open Xcode → Settings → Accounts
   - Add your Apple ID (free account works for simulator testing)
   - Xcode will automatically manage certificates and provisioning profiles

> **Note**: For simulator-only testing (no physical device), you don't need a paid Apple Developer account.

### Android Setup

1. **Install Android Studio**: Download from [developer.android.com/studio](https://developer.android.com/studio)
2. **Install Android SDK** (via Android Studio):
   - Open Android Studio → Settings → Appearance & Behavior → System Settings → Android SDK
   - Install the latest SDK Platform and SDK Build Tools
3. **Set Environment Variable**:
   ```bash
   export ANDROID_HOME="$HOME/Library/Android/sdk"  # macOS
   # Or for Linux:
   # export ANDROID_HOME="$HOME/Android/Sdk"
   ```
   Add this to your `~/.zshrc` or `~/.bashrc` to make it permanent.

> **Tip**: Verify setup with `$ANDROID_HOME/platform-tools/adb --version`

### Real Mobile App Testing
```bash
# Set ANDROID_HOME if you have Android Studio installed
export ANDROID_HOME="/Users/username/Library/Android/sdk"  # macOS location

# Enter Nix environment with hybrid system tools support
cd emulator-rig && ANDROID_HOME="$ANDROID_HOME" nix develop

# All tools automatically available - iOS, Android, and Nix tools integrated
cargo run -- test --client1 ios --client2 ios          # iOS ↔ iOS testing
cargo run -- test --client1 android --client2 android  # Android ↔ Android testing
cargo run -- test --client1 ios --client2 android      # Cross-platform testing
```

### Data-Driven Scenario Testing
```bash
cd scenario-runner && nix develop

# Run TOML scenario with Android emulator integration
ANDROID_HOME="/path/to/android/sdk" cargo run -- run-android ../scenarios/android_to_android.toml

# Run TOML scenario with mock simulation
cargo run -- run-file ../scenarios/android_to_android.toml

# Validate scenario configuration
cargo run -- validate ../scenarios/android_to_android.toml
```

## Architecture

```
simulator/
├── emulator-rig/                      # Real mobile app testing framework
│   ├── src/                           # Emulator orchestration and Appium automation
│   └── vendored/
│       ├── bitchat-ios/               # Swift implementation (canonical)
│       └── bitchat-android/           # Kotlin implementation  
├── scenario-runner/                   # Simulation-based protocol testing
│   ├── src/                           # Event orchestration and mock networking
│   └── scenarios/                     # Protocol test scenarios (Rust code)
├── scenarios/                         # Data-driven test scenarios (TOML files)
└── Justfile                           # Build and test automation commands
```

### Client Type Mapping

| Client Type | Language | Framework | Unified Type | Framework-Specific Type |
|-------------|----------|-----------|--------------|------------------------|
| **iOS** | Swift | emulator-rig | `UnifiedClientType::Ios` | `bitchat_emulator_harness::ClientType::Ios` |
| **Android** | Kotlin | emulator-rig | `UnifiedClientType::Android` | `bitchat_emulator_harness::ClientType::Android` |
| **CLI** | Rust | scenario-runner | `UnifiedClientType::Cli` | `EventOrchestrator::ClientType::Cli` |
| **Web** | Rust (WASM) | scenario-runner | `UnifiedClientType::Web` | `EventOrchestrator::ClientType::Web` |

**Note**: The `UnifiedClientType` enum (added 2025-01-20) bridges both frameworks, enabling:
- Same-framework testing: CLI↔Web, iOS↔Android
- Cross-framework testing: CLI↔iOS, CLI↔Android, Web↔iOS, Web↔Android (orchestration needed)

## Available Test Scenarios

### Real Mobile App Scenarios
- **Android ↔ Android** - Cross-device messaging with real Android emulators
- **iOS ↔ iOS** - iOS Simulator communication testing
- **Android ↔ iOS** - Cross-platform interoperability

### Data-Driven TOML Scenarios
- `android_to_android.toml` - Comprehensive Android testing configuration
- `basic_messaging.toml` - Simple peer-to-peer communication
- `mesh_network.toml` - Multi-peer mesh networking scenarios
- `unreliable_network.toml` - Network condition simulation

### Protocol Testing Scenarios
- `deterministic-messaging` - Basic message exchange validation
- `transport-failover` - BLE ↔ Nostr switching robustness
- `session-rekey` - Cryptographic session management
- `byzantine-fault` - Malicious peer behavior resistance

## Commands Reference

### Environment Setup
```bash
# Check environments
just check-android-env
just check-ios-env

# Build apps
just build-android
just build-ios
```

### Real Mobile App Testing
```bash
# Flexible client combinations
just test-android-android
just test-ios-ios  
just test-cross-platform
just test-emulator-matrix
```

### Data-Driven Scenario Testing
```bash
cd scenario-runner && nix develop

# Run scenarios with Android environment
ANDROID_HOME="/path/to/android/sdk" cargo run -- run-android ../scenarios/android_to_android.toml
cargo run -- run-file ../scenarios/basic_messaging.toml
cargo run -- validate ../scenarios/mesh_network.toml
cargo run -- list
```

### CLI/Web Client Testing (scenario-runner)
```bash
cd scenario-runner && nix develop

# Test with CLI client (default)
cargo run -- scenario deterministic-messaging
cargo run -- scenario transport-failover

# Test with Web (WASM) client
cargo run -- --client-type web scenario deterministic-messaging
cargo run -- --client-type web scenario transport-failover

# Cross-implementation testing (CLI ↔ Web)
cargo run -- cross-implementation-test --client1 cli --client2 web

# Run all scenarios
cargo run -- all-scenarios

# List available scenarios
cargo run -- list
```

### Cross-Framework Testing (Using Client Bridge)
```bash
cd scenario-runner && nix develop

# The UnifiedClientType enum enables testing across frameworks
# Currently supports: cli, web, ios, android

# Check client pair compatibility (in Rust code)
use scenario_runner::{UnifiedClientType, ClientPair};

let pair = ClientPair::new(UnifiedClientType::Cli, UnifiedClientType::Ios);
println!("Requires bridging: {}", pair.requires_bridging());  // true
println!("Strategy: {:?}", pair.testing_strategy());

// Same-framework pairs (ready to use)
// - CLI ↔ Web (scenario-runner)
// - iOS ↔ Android (emulator-rig)

// Cross-framework pairs (bridge implemented, orchestration needed)
// - CLI ↔ iOS
// - CLI ↔ Android  
// - Web ↔ iOS
// - Web ↔ Android
```

## Platform Support

| Client | Language | Framework | Implementation Path | Automation | Status |
|--------|----------|-----------|---------------------|------------|--------|
| **iOS** | Swift | emulator-rig | `vendored/bitchat-ios/` | Appium + XCUITest | ✅ Ready |
| **Android** | Kotlin | emulator-rig | `vendored/bitchat-android/` | Appium + UiAutomator2 | ✅ Ready |
| **CLI** | Rust | scenario-runner | `../../crates/bitchat-cli/` | JSON events | ✅ Working |
| **Web** | Rust (WASM) | scenario-runner | `../../crates/bitchat-web/` | JSON events | ✅ **Fixed 2025-01-20** |

### Implementation Details

- **iOS (Swift)**: Canonical iOS implementation with full feature parity, includes Noise protocol, BLE mesh, Nostr relay support, and Tor integration
- **Android (Kotlin)**: Canonical Android implementation with full feature parity, includes all protocol features and transport layers
- **CLI (Rust)**: Reference Rust implementation for testing and development
- **Web (WASM)**: Browser-based Rust implementation compiled to WebAssembly
  - **Note**: WASM build fixed 2025-01-20 by making `std` and `wasm` features independent
  - Uses `async-channel` for no_std compatibility instead of `futures-channel::mpsc`
  - Feature flags now composable: can enable `std`, `wasm`, or both together

## Key Features

### emulator-rig Framework
- **Real Mobile App Testing** - Black-box testing of actual Swift (iOS) and Kotlin (Android) implementations
- **Appium Integration** - UI automation for iOS Simulator and Android Emulator
- **Network Analysis** - mitmproxy integration for protocol validation
- **Cross-Platform Testing** - iOS ↔ iOS, Android ↔ Android, and iOS ↔ Android scenarios
- **Hybrid Nix Environment** - Automatic detection of system development tools (Xcode, Android SDK)

### scenario-runner Framework
- **Simulation-Based Testing** - Mock network environment with configurable conditions
- **Protocol Scenarios** - Comprehensive protocol testing (handshake, rekey, byzantine fault, etc.)
- **Event-Driven Orchestration** - Deterministic, reproducible test execution
- **Data-Driven Configuration** - TOML scenario files with validation rules
- **CLI & WASM Support** - Tests Rust CLI and browser-based WASM clients *(WASM fixed 2025-01-20)*
- **Cross-Implementation Testing** - Validates CLI ↔ Web compatibility
- **Independent Feature Flags** - `std` and `wasm` features can be used independently or together
- **Client Type Bridge** - Unified client type system for cross-framework testing *(New 2025-01-20)*

### Integration & Client Bridge (New 2025-01-20)
- **Unified Client Types** - `UnifiedClientType` enum supports all 4 implementations (CLI, Web, iOS, Android)
- **Framework Bridging** - Automatic conversion between scenario-runner and emulator-rig client types
- **Client Pair Validation** - Validates compatibility and determines testing strategy
- **Testing Strategies**:
  - **Single Framework**: CLI↔Web (scenario-runner), iOS↔Android (emulator-rig)
  - **Cross Framework**: CLI↔iOS, CLI↔Android, Web↔iOS, Web↔Android (bridge ready, orchestration needed)
- **Fallback Mechanisms** - Automatic mock simulation when emulators unavailable
- **Comprehensive Coverage** - From protocol-level simulation to real app black-box testing

### Client Bridge API
```rust
use scenario_runner::{UnifiedClientType, ClientPair, TestingFramework};

// Create unified client types
let cli = UnifiedClientType::Cli;
let ios = UnifiedClientType::Ios;

// Check framework requirements
assert!(cli.supports_scenario_runner());
assert!(ios.requires_emulator_rig());

// Validate client pairs
let pair = ClientPair::new(cli, ios);
assert!(pair.requires_bridging());  // Cross-framework testing

// Convert between framework-specific types
use std::convert::TryFrom;
let scenario_cli = scenario_runner::event_orchestrator::ClientType::try_from(cli).unwrap();
let emulator_ios = bitchat_emulator_harness::ClientType::try_from(ios).unwrap();
```