# BitChat Integration Simulator

Event-driven cross-client compatibility testing framework for the BitChat protocol.

## Overview

The BitChat simulator validates protocol interoperability between **all client implementations** through deterministic, event-driven testing. It treats all implementations equally - testing real client executables through their JSON automation interfaces.

## Architecture Principles

**Pure Event Orchestrator Design**: All clients tested through the same external process interface
- **No special treatment** for any implementation (including Rust)
- **No internal API access** - tests real user-facing executables  
- **Build independence** - simulator works even when core crates have issues
- **True compatibility validation** - tests actual client implementations

## Quick Start

```bash
# Enter simulator development environment
cd simulator/scenario-runner
nix develop

# List available scenarios
cargo run -- list

# Run control test (Kotlin ↔ Kotlin)
cargo run -- --client-type kotlin scenario deterministic-messaging

# Run cross-client test (Rust ↔ Kotlin)  
cargo run -- --client-type rust-cli scenario deterministic-messaging
```

## Event Orchestrator Architecture

### Unified Client Interface
All clients implement the same automation interface:
```bash
client --automation-mode --name alice --relay wss://relay.damus.io
```

### Test Flow Example
```rust
// Start any client type
orchestrator.start_client_by_type(ClientType::Kotlin, "alice").await?;
orchestrator.start_client_by_type(ClientType::Swift, "bob").await?;

// Same event-driven logic for all clients
orchestrator.wait_for_event("alice", "Ready").await?;
orchestrator.wait_for_peer_event("alice", "PeerDiscovered", "bob").await?;
let event = orchestrator.wait_for_event("bob", "MessageReceived").await?;
```

## Client Support

| Client | Status | Automation Mode | Build Status |
|--------|---------|-----------------|--------------|
| **Rust CLI** | **Tested & Working** | Full JSON events | Built |
| **Kotlin CLI** | **Built & Verified** | Full JSON events | Built |
| **Swift CLI** | **Built & Verified** | Full JSON events | Built |
| **WASM Client** | **Tested & Working** | Full JSON events | Built |

## Test Scenarios

### Control Testing (Same Implementation)
- `deterministic-messaging` - Basic message exchange validation
- `transport-failover` - BLE ↔ Nostr switching robustness
- `session-rekey` - Cryptographic session management
- `byzantine-fault` - Malicious peer behavior resistance

### Cross-Implementation Testing
- `cross-implementation-test` - Different client types communicating
- `all-client-types` - Multi-implementation mesh networking

### Advanced Protocol Scenarios
- `file-transfer-resume` - Large message handling
- `mesh-partition` - Network partitioning and healing
- `version-compatibility` - Protocol version mismatch handling
- `peer-scaling` - Multiple peer discovery and management

## JSON Automation Events

All clients emit standardized JSON events:

```json
{"type":"client_started","data":{"timestamp":1760864510999,"peer_id":"alice"}}
{"type":"Ready","data":{"timestamp":1760864511007,"peer_id":"alice"}}
{"type":"PeerDiscovered","data":{"peer_id":"bob","transport":"Nostr","timestamp":1760864512000}}
{"type":"MessageReceived","data":{"from":"bob","content":"hello","timestamp":1760864513000}}
{"type":"SessionEstablished","data":{"peer_id":"bob","timestamp":1760864514000}}
```

## Commands

### Single Client Type Testing
```bash
# Test specific implementations
cargo run -- --client-type kotlin scenario deterministic-messaging
cargo run -- --client-type swift scenario transport-failover  
cargo run -- --client-type rust-cli scenario session-rekey

# List all available scenarios
cargo run -- list
```

### Cross-Client Testing
```bash
# Run comprehensive cross-client tests (future)
cargo run -- cross-implementation-test
cargo run -- all-client-types
```

## Development Workflow

### Adding New Scenarios
1. Add scenario function in `src/main.rs`
2. Implement event-driven logic using `EventOrchestrator`
3. Update `run_scenario()` match block
4. Test with control implementation first

### Adding New Client Types
1. Implement automation mode in client (`--automation-mode` flag)
2. Emit standardized JSON events to stdout
3. Add client startup method in `event_orchestrator.rs`
4. Update `ClientType` enum and `start_client_by_type()`

## Architecture Benefits

### **True Compatibility Testing**
- All clients tested through the same interface
- No implementation gets special treatment
- Tests real user-facing executables

### **Build Independence**
- Simulator builds independently of core crates
- Can test clients even when internals have issues
- Faster development iteration

### **Deterministic Testing**
- Event-driven synchronization (no sleep() calls)
- Immune to log format changes
- Structured, parseable output

### **Maintainable & Extensible**
- Single testing paradigm for all clients
- Easy to add new client implementations
- Clear separation of concerns

## Control Testing Status

Current focus: **Kotlin ↔ Kotlin** control tests to validate simulator infrastructure

| Scenario | Status | Notes |
|----------|--------|-------|
| `deterministic-messaging` | **Testing** | Basic message exchange |
| `transport-failover` | **Pending** | Transport switching |
| `session-rekey` | **Pending** | Session management |
| `byzantine-fault` | **Pending** | Attack resistance |