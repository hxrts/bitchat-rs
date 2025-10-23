# BitChat Test Scenarios

This directory contains unified TOML scenario definitions for the BitChat simulator system. These scenarios test various aspects of the BitChat protocol using both fast simulation (virtual) and real-world testing (device).

## Scenario Categories

### 01-03: Basic Protocol Testing
- **01_discovery_basic.toml** - Basic peer discovery via announce packets
- **02_messaging_basic.toml** - Fundamental message sending and receiving
- **03_integration_comprehensive.toml** - Complete protocol integration test

### 04-05: Network Topology Testing
- **04_network_mesh_partition.toml** - Network partition healing and recovery
- **05_network_mesh_routing.toml** - Mesh network routing algorithms

### 06-08: Advanced Features
- **06_discovery_scaling.toml** - Large-scale peer discovery testing
- **07_reliability_unreliable_network.toml** - Protocol behavior under poor network conditions
- **08_monitoring_network_analysis.toml** - Network analysis and monitoring validation

### 09: Platform-Specific Testing
- **09_platform_android_to_android.toml** - Android cross-platform compatibility

## TOML Schema

All scenarios follow the unified TOML schema defined in `shared/src/scenario_config.rs`:

```toml
[metadata]
name = "Scenario Name"
description = "Brief description of what this scenario tests"
version = "1.0"
tags = ["category", "feature"]
duration_seconds = 60
author = "Author Name"

[network]
[network.profile]
type = "Perfect"  # or SlowWifi, Unreliable3G, etc.

[network.topology]
type = "FullyConnected"  # or Linear, Star, Custom

[[peers]]
name = "peer_name"
[peers.behavior]
auto_discovery = true
auto_connect = true

[[sequence]]
name = "action_name"
at_time_seconds = 1.0
action = "SendMessage"  # or other action types
from = "peer1"
to = "peer2"
content = "Message content"

[validation]
[[validation.final_checks]]
type = "PeerConnected"
peer1 = "peer1"
peer2 = "peer2"

[performance]
max_latency_ms = 100
max_packet_loss = 0.0
expected_throughput = 1.0
```

## Running Scenarios

### Using the Unified Interface

From the simulator root directory:

```bash
# Virtual testing (fast simulation)
just test-scenario 01_discovery_basic cli,cli
just test-scenario 02_messaging_basic wasm,wasm

# Device testing (real-world)
just test-scenario 02_messaging_basic ios,ios
just test-scenario 02_messaging_basic android,android
just test-scenario 02_messaging_basic ios,android

# Development workflows
just dev-cycle 02_messaging_basic
just test-matrix 02_messaging_basic
```

### Direct Execution

```bash
# Virtual system
cd virtual
cargo run --release -- execute ../scenarios/01_discovery_basic.toml

# Device system
cd device
cargo run --release -- execute --clients ios,ios ../scenarios/02_messaging_basic.toml
```

## Adding New Scenarios

1. Create a new TOML file following the naming pattern: `NN_category_name.toml`
2. Use the established schema and validation rules
3. Include comprehensive metadata and tags
4. Add appropriate validation checks and performance expectations
5. Test with both simulation and real-world executors if applicable

## Scenario Design Guidelines

### Network Profiles
- **Perfect**: Zero latency, no packet loss (ideal for protocol validation)
- **SlowWifi**: Realistic WiFi conditions with some latency and jitter
- **Unreliable3G**: Mobile network simulation with packet loss and reordering
- **MeshNetwork**: Multi-hop routing with partition possibilities

### Validation Best Practices
- Always include peer connectivity validation
- Validate message delivery counts
- Check performance metrics within reasonable bounds
- Use continuous validation for long-running scenarios
- Include network statistics validation for stress tests

### Performance Expectations
- Set realistic latency limits based on network profile
- Define acceptable packet loss rates
- Include memory and CPU usage limits for resource testing
- Specify expected throughput for load testing

## Integration with Test Architecture

These scenarios are designed to work with the two-tiered testing architecture:

1. **Fast Simulation** (virtual): Deterministic protocol testing with virtual time
2. **Real-World Testing** (device): End-to-end testing with actual devices/emulators

The unified TOML format ensures scenarios can be executed in both environments, providing comprehensive test coverage from unit-level protocol validation to full system integration testing.