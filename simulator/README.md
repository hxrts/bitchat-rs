# BitChat Simulator

**Two-Tiered Testing Strategy for Protocol Development**

```
┌─────────────────────────────────────────────────────────────────┐
│                     Test Scenarios (TOML)                        │
│              Single Source of Truth for ALL Tests                │
└────────────────────────┬─────────────────┬──────────────────────┘
                         │                 │
         ┌───────────────▼────┐    ┌──────▼───────────────┐
         │     virtual        │    │      device          │
         │ Fast Simulation    │    │ Real-World E2E       │
         │   (seconds)        │    │   (minutes)          │
         └────────────────────┘    └──────────────────────┘
```

## Quick Start

```bash
# 1. Write a test scenario (once)
cat > scenarios/my_test.toml << 'EOF'
[metadata]
name = "My Test"

[[peers]]
name = "alice"

[[peers]]
name = "bob"

[[sequence]]
at_time_seconds = 2.0
action = "SendMessage"
from = "alice"
to = "bob"
content = "Hello!"
EOF

# 2. Run with fast simulation (seconds)
just test-scenario my_test cli,cli

# 3. Run with real devices (minutes)
just test-scenario my_test ios,android
```

## Architecture Overview

### Virtual Testing System
**Use for:** Protocol validation, regression testing, fault injection
- **Fast** - 1000+ tests in minutes
- **Deterministic** - Same input → same output, always
- **White-box** - Can inspect internal state
- **Mocked** - Network, time, crypto all controlled

### Device Testing System  
**Use for:** Integration testing, UI validation, final product verification
- **Slow** - 10+ tests in hours
- **Non-deterministic** - Real network timing varies
- **Black-box** - Only observes external behavior
- **Real** - Actual OS, network stack, Bluetooth

### Separation of Concerns

| Component | Virtual | Device | Shared |
|-----------|---------|--------|--------|
| **Test Definition** | No | No | TOML files |
| **Protocol Logic** | In-memory | Black-box | - |
| **Network** | Mocked | Real | - |
| **Time Control** | Virtual | Real | - |
| **Client Types** | CLI, WASM | iOS, Android | Both via bridge |
| **Speed** | Seconds | Minutes | - |

## Directory Structure

```
simulator/
├── scenarios/               ← TOML test definitions (shared)
│   ├── 01_discovery_basic.toml
│   ├── 02_messaging_basic.toml
│   └── 03_integration_comprehensive.toml
│
├── virtual/                 ← Fast protocol simulation
│   ├── src/simulation_executor.rs
│   ├── src/network_router.rs
│   └── harness/             # Client abstraction layer
│
├── device/                  ← Real-world E2E testing
│   ├── src/orchestrator.rs
│   ├── vendored/bitchat-ios/
│   └── vendored/bitchat-android/
│
├── shared/                  ← Common scenario types
└── ios-linker-workaround/   ← Temporary Xcode 16 fix
```

## Client Types and Usage

```bash
# Fast protocol testing (virtual simulation)
just test-scenario 02_messaging_basic cli,cli        # CLI ↔ CLI
just test-scenario 01_discovery_basic wasm,wasm      # WebAssembly ↔ WebAssembly

# Real-world device testing
just test-scenario 02_messaging_basic ios,ios        # iOS ↔ iOS
just test-scenario 02_messaging_basic android,android # Android ↔ Android
just test-scenario 02_messaging_basic ios,android    # Cross-platform

# Validation prevents mixing virtual and device clients
just test-scenario 02_messaging_basic cli,ios        # ERROR: Cannot mix
```

## Writing Test Scenarios

Scenarios are defined in **TOML files** that work with **both** testing systems.

### Minimal Example

```toml
# scenarios/hello_world.toml

[metadata]
name = "Hello World"
description = "Alice says hello to Bob"

[[peers]]
name = "alice"

[[peers]]
name = "bob"

[[sequence]]
at_time_seconds = 2.0
action = "SendMessage"
from = "alice"
to = "bob"
content = "Hello World!"

[[validation.final_checks]]
type = "MessageDelivered"
from = "alice"
to = "bob"
content = "Hello World!"
```

### Advanced Example with Network Simulation

```toml
# scenarios/network_partition.toml

[metadata]
name = "Network Partition Recovery"
description = "Test mesh heals after partition"

# Network simulation (virtual system only)
[network]
latency_ms = 50
packet_loss = 0.05

[[peers]]
name = "alice"

[[peers]]
name = "bob"

[[peers]]
name = "charlie"

[[sequence]]
at_time_seconds = 5.0
action = "PartitionNetwork"
isolated_peers = ["charlie"]

[[sequence]]
at_time_seconds = 10.0
action = "SendMessage"
from = "alice"
to = "charlie"
content = "Are you there?"

[[sequence]]
at_time_seconds = 15.0
action = "HealNetwork"

[[validation.final_checks]]
type = "MessageDelivered"
from = "alice"
to = "charlie"
content = "Are you there?"
timeout_seconds = 30
```

## Design Principles

### Do This
```rust
// virtual: Use mocked network
let router = NetworkRouter::new()
    .with_latency(Duration::from_millis(50))
    .with_packet_loss(0.1);

// device: Use real devices
let sim = IosSimulator::create("alice")?;
sim.install_app("BitChat.app")?;
```

### Don't Do This
```rust
// virtual should NOT use real devices
let sim = IosSimulator::create("alice")?;  // WRONG!

// device should NOT mock protocol
let runtime = BitchatRuntime::new_with_mocks()?;  // WRONG!
```

### Abstract vs Concrete Actions
```toml
# GOOD - abstract action
[[sequence]]
action = "SendMessage"
from = "alice"
to = "bob"

# BAD - platform-specific implementation
[[sequence]]
action = "TapButton"
button_id = "send_button_ios"
```

## Decision Tree: Which System to Use?

```
Are you testing...

├─ Protocol logic?
│  ├─ State transitions? → virtual
│  ├─ Message ordering? → virtual
│  ├─ Fault tolerance? → virtual
│  └─ Cryptography? → virtual
│
└─ Final application?
   ├─ UI behavior? → device
   ├─ OS integration? → device
   ├─ Real Bluetooth? → device
   └─ App stability? → device
```

**Rule of Thumb:** Need speed and determinism → `virtual`. Need real-world confidence → `device`.

## Common Commands

### Development Workflow
```bash
# Development cycle (fast → real-world verification)
just dev-cycle 02_messaging_basic

# Test across all client combinations
just test-matrix 02_messaging_basic

# Quick health check
just smoke-test

# List available scenarios
just list-scenarios
```

### Build System
```bash
# Build all testing systems
just build

# Build specific systems
just build-virtual
just build-device

# Check environments are ready
just check-all
```

### Debugging
```bash
# Virtual testing with verbose output
cd virtual
cargo run --release -- execute ../scenarios/my_test.toml --verbose

# Device testing with debug logs
cd device  
RUST_LOG=debug cargo run --release -- execute --clients ios,ios ../scenarios/my_test.toml
```

### Adding New Scenarios
```bash
# 1. Create from template
just new-scenario my_new_test

# 2. Edit the TOML file
vim scenarios/my_new_test.toml

# 3. Test with fast simulation first
just test-scenario my_new_test cli,cli

# 4. Test with real devices when ready
just test-scenario my_new_test ios,ios
```

## Testing Pyramid

```
         /\
        /  \       device
       /E2E \      (10+ tests, minutes, high confidence)
      /______\     
     /        \    
    /          \   virtual
   / Integration\  (100+ tests, seconds, deterministic)
  /______________\ 
 /                \
/  Unit Tests      \ cargo test
\__________________/ (1000+ tests, instant, focused)
```

## Troubleshooting

### Virtual Tests Failing
```bash
cd virtual
cargo run --release -- execute ../scenarios/failing_test.toml
cargo check
cargo test
```

### Device Tests Failing
```bash
# Check simulators and emulators
xcrun simctl list devices
adb devices

# Rebuild apps
cd device && just build-apps

# Check environment
just check-env
```

### Environment Issues
```bash
# Check both environments
just check-all

# Clean and rebuild
cd virtual && just clean && just build
cd device && just clean && just build
```

## Implementation Status

**Current:**
- TOML scenario format implemented
- Universal client bridge working
- iOS and Android emulation functional
- Virtual system with deterministic simulation

**In Progress:**
- Scenario executor trait standardization
- Enhanced TOML schema with validation rules
- Justfile simplification

**Benefits:**
- Single scenario definition works everywhere
- Clear separation between fast simulation and real-world testing
- Unified interface for all client types
- Optimal speed: seconds for development, minutes for confidence
- Easy maintenance and flexibility for new client types

This architecture ensures fast development cycles with thorough real-world validation before shipping.