# BitChat Integration Simulator

Event-driven cross-client compatibility testing framework for the BitChat protocol.

## Overview

The BitChat simulator validates protocol interoperability between Rust, Swift, and Kotlin client implementations through deterministic, event-driven testing. It replaces brittle stdout parsing with structured JSON automation events for reliable continuous integration.

## Quick Start

```bash
# Build all components
just build

# Run critical test scenarios
just test-critical

# Run security-focused tests  
just test-security

# Individual scenario testing
./test_runner/target/release/bitchat-test-runner scenario deterministic-messaging
```

## Architecture

### Event-Driven Testing
- **No sleep() calls** - deterministic event synchronization
- **Structured JSON events** - immune to log format changes
- **Machine-readable output** - robust parsing and validation

### Test Flow
```rust
orchestrator.start_rust_client("alice").await?;
orchestrator.wait_for_event("alice", "Ready").await?;
orchestrator.wait_for_peer_event("alice", "PeerDiscovered", "bob").await?;
let event = orchestrator.wait_for_event("bob", "MessageReceived").await?;
```

## Client Support

| Client | Status | Automation Mode |
|--------|---------|-----------------|
| **Rust** | Ready | Full JSON events |
| **Swift** | Mock | Planned |
| **Kotlin** | Mock | Planned |

## Test Scenarios

### Critical Scenarios
- Transport failover (BLE ↔ Nostr)
- Session rekey under load
- Byzantine fault tolerance
- Mesh partitioning and healing
- Panic action and recovery

### Security Testing
- Malformed packet injection
- Replay attack detection
- Protocol conformance validation
- Resource exhaustion testing

## Automation Events

```json
{"event": "Ready", "peer_id": "abc123", "timestamp": 1234567890}
{"event": "PeerDiscovered", "peer_id": "def456", "transport": "BLE", "timestamp": 1234567891}
{"event": "MessageReceived", "from": "abc123", "content": "hello", "timestamp": 1234567892}
{"event": "SessionEstablished", "peer_id": "def456", "timestamp": 1234567893}
```

## Commands

```bash
# Core testing
just test-critical              # All critical scenarios
just test-security             # Security-focused tests
just test-transport-failover   # BLE/Nostr failover
just test-byzantine-fault      # Attack resistance

# Stress testing
just stress-session-rekey --load-factor 10
just stress-mesh-partition --nodes 20

# Network simulation
just test-lossy-network        # 10% packet loss
just test-high-latency         # 500ms latency
```

## Development

### Adding Test Scenarios
1. Implement scenario in `test_runner/src/scenarios/`
2. Add event-driven test logic using `EventOrchestrator`
3. Register in `scenarios/mod.rs`
4. Add command to `Justfile`

### Client Integration
1. Add `--automation-mode` flag to client
2. Emit structured JSON events to stdout
3. Handle commands via stdin
4. Update orchestrator with client-specific startup

## Benefits

**Before**: Unreliable, slow, maintenance-heavy test suite
**After**: Fast, deterministic, robust integration testing

- **Zero false positives** from timing issues
- **Maintainable tests** immune to log format changes
- **Advanced capabilities** for security and protocol testing
- **Foundation** for comprehensive cross-client validation

## Next Steps

1. **Real Client Integration** - Replace mock Swift/Kotlin with actual SDK wrappers
2. **Advanced Security Tests** - Leverage bitchat-core types for vulnerability testing
3. **Network Simulation** - Test under various network conditions