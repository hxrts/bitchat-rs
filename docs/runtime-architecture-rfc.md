# BitChat Runtime Architecture RFC

## Overview

BitChat is being reorganised around a four-layer architecture that explicitly separates protocol logic, shared orchestration utilities, transport adapters, and end-user runtimes. This document captures the design goals, module boundaries, and dependency rules that every crate in the workspace must respect.

## Layer Responsibilities

### 1. `bitchat-core` – Protocol & Data Structures

- Implements the BitChat wire protocol, cryptographic primitives, message store, and deterministic business logic.
- Provides `alloc`-only code by default; `std` and `wasm` conveniences are feature-gated.
- Exposes pure data types (commands, payloads, envelopes), validation helpers, and protocol state machines.
- **No awareness** of async runtimes, logging, or transport IO.

### 2. `bitchat-harness` – Shared Runtime Plumbing

- Owns channel definitions, strongly-typed dispatcher messages, and lifecycle traits that transports/runtimes rely on.
- Provides builders for channel wiring, logging hooks, and supervisor utilities (timers, heartbeats, reconnection strategies).
- Bridges `bitchat-core`’s pure types with async execution concerns while remaining transport-agnostic.
- Abstracts over platform-specific async primitives (Tokio vs. wasm-bindgen) via feature flags.

### 3. Transport Crates (`bitchat-ble`, `bitchat-nostr`, …)

- Implement the `TransportAdapter` trait defined in the harness.
- Contain only transport-specific logic (BLE advertising, Nostr subscriptions, etc.).
- Delegate channel IO, reconnection policies, and effect execution to the harness utilities.
- Must not reach directly into `bitchat-runtime` or duplicate orchestration code.

### 4. `bitchat-runtime` – Multi-Task Orchestrator

- Builds a runtime from one or more transports, plus logical tasks that handle ingress, session management, storage, delivery, and supervision.
- Owns execution topology: task spawning, cancellation, monitoring, and fault recovery.
- Provides ergonomic builders for applications (CLI, web, simulator) to configure transports and interact through command/app-event channels supplied by the harness.
- Exposes metrics/logging hooks but delegates schema and protocol correctness to `bitchat-core`.

## Dependency Rules

- `bitchat-core` has **no** upstream dependencies on harness, transports, or runtime.
- `bitchat-harness` depends on `bitchat-core`, but **nothing else** depends on runtime or transports.
- Transport crates depend on `bitchat-core` and `bitchat-harness` only.
- `bitchat-runtime` depends on `bitchat-core`, `bitchat-harness`, and transport crates, but transports must not depend back on runtime.
- Application crates (CLI, web, simulator) depend on `bitchat-runtime` and optionally specific transports.

### Feature Constraints

- Each crate should expose a maximum of three top-level features: `std`, `wasm`, and `testing`. Implementation details (Tokio, wasm-bindgen, etc.) must be hidden behind these feature flags.
- `bitchat-core` defaults to `std`; enabling `wasm` or `testing` must be mutually exclusive with other runtime-specific flags.
- `bitchat-harness` coordinates runtime-specific async shims; transports and runtime inherit the feature gating through the harness.

## Feature Matrix

The BitChat workspace uses three mutually exclusive feature flags that control platform-specific functionality and dependencies:

### Core Features

#### `std` (Default)
**Target Environment**: Native applications, CLI tools, server deployments

**Key Capabilities**:
- Full Rust standard library support
- Tokio async runtime with multi-threading
- Comprehensive logging via `tracing` crate
- File system access and networking primitives
- High-precision timing and system clock access

**Dependencies Enabled**:
- `tokio` with full feature set (sync, time, rt, macros)
- `tracing` for structured logging
- `instant` for performance timing
- Standard library features in serde, crypto crates
- `futures-channel/std` for async communication

**Example Use Cases**: Desktop CLI client, server-side message relay, integration testing

#### `wasm` 
**Target Environment**: WebAssembly in browser contexts

**Key Capabilities**:
- Browser-compatible async primitives via `wasm-bindgen-futures`
- JavaScript interop through `js-sys`
- Alternative channel implementations using `async-broadcast`
- Browser-safe timing via `instant` crate's WASM backend
- No-std core with `alloc` for memory management

**Dependencies Enabled**:
- `wasm-bindgen-futures` for browser Promise integration
- `js-sys` for JavaScript API access
- `async-broadcast` for cross-task communication
- `instant` with WASM performance timing
- `futures-channel` without std dependency

**Limitations**:
- No file system access
- No native networking (must use browser APIs)
- Limited crypto randomness (uses browser crypto APIs)
- Single-threaded execution model

**Example Use Cases**: Browser-based messaging app, WebRTC signaling, WASM-based relay nodes

#### `testing`
**Target Environment**: Test environments with additional debugging utilities

**Key Capabilities**:
- All `std` features plus enhanced testing utilities
- Mock transport implementations
- Additional monitoring and metrics collection  
- Debug assertions and validation helpers
- Extended logging for test debugging

**Dependencies Enabled**:
- All `std` dependencies
- `task-logging` and `monitoring` internal flags
- Mock transport and channel inspection utilities
- Property testing framework integration

**Example Use Cases**: Unit tests, integration tests, property-based testing, simulator environments

### Feature Interaction Rules

1. **Mutual Exclusion**: Only one of `std`, `wasm`, or `testing` may be enabled at compilation time
2. **Default Behavior**: `std` is enabled by default for backwards compatibility
3. **Inheritance**: Transport crates and runtime inherit feature selection from core
4. **Validation**: Compile-time guards prevent invalid feature combinations

### Internal Feature Flags

The following features are implementation details and should not be used by external consumers:

- `task-logging`: Enables `tracing` dependency (controlled by top-level features)
- `monitoring`: Enables metrics collection (controlled by top-level features)

### Migration Notes

When updating from legacy feature flags:
1. Replace `default` with explicit `std` if needed
2. Remove any transport-specific feature combinations
3. Ensure only one of the three main features is active
4. Update build scripts to use the new feature names

### Build Examples

```bash
# Native/CLI development (default)
cargo build

# WebAssembly compilation  
cargo build --no-default-features --features wasm --target wasm32-unknown-unknown

# Testing with enhanced utilities
cargo test --no-default-features --features testing

# CI validation of different targets
cargo check --features std
cargo check --no-default-features --features wasm --target wasm32-unknown-unknown
cargo test --no-default-features --features testing
```

## Implementation Guidelines

1. **Canonical Messages**: Harness defines a single set of channel payload types (commands/events/effects). Transports use helper constructors to normalise raw packets before forwarding them.
2. **Lifecycle Builders**: Transports receive `TransportHandle` objects from the harness rather than manipulating raw senders/receivers. This guarantees channels are attached before execution and simplifies shutdown.
3. **Task Decomposition**: Runtime spawns independent async tasks for ingress, session management, storage/delivery, and supervision. Communication between these tasks uses the canonical message types provided by the harness.
4. **Testing Contracts**: Harness supplies mock transports and channel inspectors to unit-test both transports and runtime components without real IO. Integration tests live in `bitchat-runtime` and application crates.

## Removal of Legacy Paths

- The previous `CoreLogicTask` monolith, transport-specific channel wiring, and CLI orchestrator will be deleted once the new architecture is in place.
- Documentation and code comments referencing “single task core logic” must be updated to describe the multi-task supervisor approach.
- No compatibility shims will be added; consumers are expected to migrate directly to the new runtime API.

## Open Work

- Channel schema: finalise `bitchat-harness` message definitions and validation helpers.
- Transport harness: implement lifecycle traits and shared utilities.
- Runtime decomposition: refactor `bitchat-runtime` into multiple tasks per this RFC.
- Feature simplification: align all crates with the `std/wasm/testing` gating model.

---

This RFC will evolve alongside the refactor. Once the implementation is complete, this document becomes the authoritative architecture description and supersedes legacy docs in `docs/hybrid-architecture-proposal.md`.
