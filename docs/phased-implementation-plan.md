
# Phased Implementation Plan for a Portable Rust Bitchat Library

This document outlines a phased implementation plan for creating a portable, performant, and modular Rust library for the Bitchat protocol. The library is intended to be compilable for both native platforms and WebAssembly (WASM) for use in browsers.

## Implementation Philosophy

**Core Principle**: We prioritize concise, clean, and elegant implementation over feature completeness. Every line of code should serve a clear purpose, and the overall architecture should be immediately understandable to new contributors.

**Design Guidelines**:
- **Minimal Surface Area**: Prefer fewer, well-designed APIs over many specialized ones
- **Zero Allocation Paths**: Design critical paths to avoid unnecessary allocations
- **Composition Over Inheritance**: Use traits and composition to build complex behavior from simple primitives
- **Fail Fast**: Use Rust's type system to catch errors at compile time rather than runtime
- **Self-Documenting Code**: Code should be readable without extensive comments

## 1. Architecture

The proposed architecture is a layered, modular design that separates the core protocol logic from the transport-specific implementations. This allows for a high degree of code reuse and makes it easier to add new transports in the future.

```mermaid
graph TD
    subgraph Core Library (bitchat-core)
        A[Application Layer] --> B[Session Layer];
        B --> C[Encryption Layer];
        C --> D[Data Models & Serialization];
    end

    subgraph Transport Abstraction
        E[Transport Trait]
    end

    subgraph Native Transports
        F[BLE Transport] -- implements --> E;
    end

    subgraph Web Transports
        G[Nostr/WebSocket Transport] -- implements --> E;
    end

    D -- uses --> A;
    Core Library -- uses --> E;
```

*   **`bitchat-core`**: A `no_std` compatible crate that contains the core protocol logic, including data models, serialization, encryption, session management, and application logic. This crate will be the foundation for both native and WASM builds.
    - **Design Principle**: Minimal dependencies, maximum reusability
    - **API Goal**: Single-responsibility modules with clear boundaries
*   **Transport Trait**: A generic `Transport` trait that defines the interface for sending and receiving Bitchat packets. This will allow for different transport implementations to be plugged into the core library.
    - **Design Principle**: Simple async trait with minimal methods
    - **API Goal**: Send/receive abstraction that hides transport complexity
*   **Transport Implementations**: Separate crates for each transport mechanism (e.g., `bitchat-ble`, `bitchat-nostr`). These crates will implement the `Transport` trait and handle the specifics of each communication medium.
    - **Design Principle**: One responsibility per crate, clean error propagation
    - **API Goal**: Transport-specific configuration with sensible defaults

## 2. Phased Work Plan

The implementation is divided into four phases, each with specific goals, libraries, and completion criteria.

### Learnings from `bitchat-tui` Implementation

An analysis of the existing `bitchat-tui` Rust implementation provides valuable insights:

*   **Validation of Core Libraries**: The TUI uses `btleplug`, `tokio`, and the `dalek` cryptography suite, validating our choice of these core dependencies.
*   **Custom Noise & Serialization**: The TUI implements the Noise protocol and binary packet serialization manually, rather than using the `snow` and `bincode` crates. This was likely done to ensure byte-for-byte compatibility with the Swift implementation and to have fine-grained control over the protocol logic.
*   **Rationale for Manual Implementation**: The decision to implement the Noise protocol and serialization manually in `bitchat-tui` seems to be driven by a desire for maximum control and cross-platform compatibility. The Bitchat protocol has a precise binary format, and the developers of the TUI and Swift versions likely collaborated to ensure their implementations were perfectly interoperable. Manual serialization is the most straightforward way to guarantee this. Similarly, while the Noise protocol is a standard, different libraries can have slightly different APIs or behaviors. By implementing it manually, the developers can ensure that their state machine and message handling logic are identical across platforms.
*   **Our Approach**: We will proceed with `snow` and `bincode` for their maturity and to accelerate development. However, a crucial part of our process will be to create a suite of test vectors from the `bitchat-tui` and `bitchat-swift` implementations to verify that our library produces identical binary output. This de-risks our choice of higher-level crates and ensures cross-platform compatibility.

### Phase 1: Core Protocol & Cryptography

**Goal**: To build the foundational `bitchat-core` library with all necessary data structures and cryptographic primitives. This phase will result in a testable library that can create, serialize, and deserialize Bitchat packets, as well as perform all required cryptographic operations.

**Implementation Directive**: Focus on data structure elegance and cryptographic safety. Every type should be self-validating, and the API should make misuse difficult.

**What's being built**:
*   All data structures from `data_structures.rs` (e.g., `BitchatPacket`, `BitchatMessage`).
    - **Clean Implementation**: Use newtype patterns for semantic validation
    - **Zero-Copy Design**: Minimize allocations in serialization paths
*   Binary serialization and deserialization for all data structures.
    - **Elegant Approach**: Custom `serde` implementations where needed for wire format compatibility
*   The Noise Protocol implementation (`Noise_XX_25519_ChaChaPoly_SHA256`).
    - **Concise Wrapper**: Thin abstraction over `snow` that exposes only what we need
*   Ed25519 signing and verification.
    - **Type Safety**: Use phantom types to distinguish signed vs unsigned data
*   Fingerprint generation.
    - **Clear API**: Single function that takes public key, returns fingerprint

**Selected Libraries**:
*   **Serialization**: `serde` for the serialization framework and `bincode` for the binary format.
*   **Noise Protocol**: `snow` for the Noise Protocol Framework implementation.
*   **Cryptography**: `curve25519-dalek` for X25519, `ed25519-dalek` for Ed25519 signatures, `chacha20poly1305` for the AEAD cipher, and `sha2` for hashing.
*   **UUIDs**: `uuid` for generating unique message IDs.
*   **Error Handling**: `thiserror` for creating custom error types.

**Implementation Criteria (Done when...)**:
*   All data structures are defined and can be serialized to and deserialized from the correct binary format.
*   The Noise `XX` handshake can be successfully completed between two in-memory `NoiseHandshakeState` instances.
*   Messages can be encrypted and decrypted using the transport ciphers derived from the Noise handshake.
*   Ed25519 signatures can be created and verified.
*   **Verification**: A suite of test vectors generated from the `bitchat-tui` and `bitchat-swift` implementations is used to confirm that `bitchat-core` produces byte-for-byte identical output for packet serialization and cryptographic operations.
*   Unit tests cover all serialization, deserialization, and cryptographic functions.
*   The `bitchat-core` library compiles successfully for both native and `wasm32-unknown-unknown` targets.

### Phase 2: Session & Application Layers

**Goal**: To build upon the core library by adding session management and application-level logic. This phase will result in a library that can manage multiple peer sessions and handle various message types.

**Implementation Directive**: Build stateful components that are easy to reason about. Session state should be explicit, and message flow should be predictable.

**What's being built**:
*   `NoiseSession` and `NoiseSessionManager` to manage the lifecycle of Noise sessions.
    - **Clean State Machine**: Explicit states (Handshaking, Established, Failed)
    - **Resource Management**: Automatic cleanup with RAII patterns
*   Message handlers for all Bitchat message types (e.g., `Announce`, `Message`, `ReadReceipt`).
    - **Elegant Dispatch**: Enum-based message types with trait-based handlers
    - **Minimal Boilerplate**: Derive macros where appropriate
*   The fragmentation and reassembly logic for large messages.
    - **Simple Algorithm**: Straightforward fragment numbering and reassembly
    - **Memory Efficient**: Stream-based processing for large messages
*   The `DeliveryTracker` for message reliability.
    - **Concise Logic**: Timeout-based retry with exponential backoff
    - **Clean API**: Start tracking, mark delivered, automatic cleanup

**Selected Libraries**:
*   Building on the libraries from Phase 1.

**Implementation Criteria (Done when...)**:
*   The `NoiseSessionManager` can create, manage, and terminate multiple Noise sessions.
*   The library can process all Bitchat message types and dispatch them to the correct handlers.
*   Large messages are correctly fragmented, and fragments are correctly reassembled.
*   The `DeliveryTracker` correctly tracks the delivery status of messages.
*   Integration tests simulate a multi-peer environment and verify that sessions and messages are handled correctly.

### Phase 3: Native Transport (BLE + Nostr)

**Goal**: To implement native transport layers using both Bluetooth Low Energy (BLE) and Nostr. This phase will result in a runnable native application (e.g., a TUI or CLI) that can communicate with other Bitchat peers over both transports with intelligent routing.

**Implementation Directive**: Build transport abstractions that are unified and simple. Different transports should feel identical to the core protocol layer.

**What's being built**:
*   A `bitchat-ble` crate that implements the `Transport` trait.
    - **Clean Abstraction**: Hide BLE complexity behind simple send/receive API
    - **Robust Connection**: Automatic reconnection with exponential backoff
*   Logic for scanning for peers, connecting, and exchanging data over BLE.
    - **Elegant Discovery**: Async stream of discovered peers
    - **Minimal Configuration**: Sensible defaults for scanning and advertising
*   A `bitchat-nostr` crate for native Nostr communication.
    - **Unified Interface**: Same `Transport` trait as BLE
    - **Efficient Relay Management**: Connection pooling and automatic failover
*   Intelligent transport selection (BLE preferred, Nostr fallback).
    - **Simple Policy**: Clear priority ordering with automatic fallback
    - **Transparent Switching**: Core protocol unaware of transport changes
*   A simple TUI or CLI application to demonstrate the dual-transport functionality.
    - **Concise Demo**: Focus on showcasing protocol capabilities, not UI complexity

**Selected Libraries**:
*   **BLE**: `btleplug` for cross-platform BLE communication.
*   **Nostr**: `nostr-sdk` for Nostr communication.
*   **WebSockets**: `tokio-tungstenite` for Nostr relay connections.
*   **Async Runtime**: `tokio` to drive all asynchronous operations.

**Implementation Criteria (Done when...)**:
*   The application can successfully scan for and connect to other Bitchat peers over BLE.
*   The application can connect to Nostr relays and exchange messages.
*   A Noise session can be established between two peers over either transport.
*   Encrypted messages can be sent and received between two peers via both transports.
*   Intelligent routing selects BLE when available, falls back to Nostr when needed.
*   The application can handle disconnections and reconnections on both transports.
*   End-to-end tests verify communication between two instances using both transports.

### Phase 4: Web Transport (Nostr WASM Only)

**Goal**: To implement a web-based transport layer using Nostr only and compile the library to WebAssembly. This phase will result in a WASM module that can be used in a web browser to communicate with native Bitchat peers via Nostr relays.

**Note**: Web version will NOT have Bluetooth support due to limited WebBluetooth API support (Chromium-only, experimental). The web client will be Nostr-only and can communicate with native clients through Nostr relays.

**Implementation Directive**: Create browser-native experience with minimal JavaScript glue. WASM module should be self-contained and easy to integrate.

**What's being built**:
*   A web-compatible `bitchat-nostr` crate using WASM-friendly libraries.
    - **Clean WASM API**: Minimal JavaScript surface area
    - **Async Integration**: Proper Promise/Future bridging
*   WASM bindings for the `bitchat-core` library and the Nostr transport.
    - **Elegant Bindings**: Use `wasm-bindgen` patterns for clean JS interop
    - **Type Safety**: Preserve Rust type safety across WASM boundary
*   A simple web application to demonstrate browser-based Bitchat functionality.
    - **Minimal Example**: Show core functionality without framework complexity
    - **Clear Integration**: Document how to embed in larger web applications
*   Interoperability testing between web clients and native clients via Nostr.
    - **Cross-Platform Verification**: Ensure wire-format compatibility
    - **Performance Benchmarks**: Measure WASM vs native performance

**Selected Libraries**:
*   **Nostr (WASM)**: `nostr-sdk` with WASM compatibility.
*   **WebSockets (WASM)**: `ws_stream_wasm` for WebSocket communication in the browser.
*   **WASM Binding**: `wasm-bindgen` to generate the JavaScript-Rust interoperability layer.
*   **Async Runtime (WASM)**: `tokio_with_wasm` for browser-compatible async operations.
*   **Crypto (WASM)**: Pure Rust `dalek` cryptography suite (no `ring` due to WASM incompatibility).

**Implementation Criteria (Done when...)**:
*   The `bitchat-core` library and the `bitchat-nostr` crate compile successfully to WASM.
*   The WASM module can connect to Nostr relays and establish communication channels.
*   A Noise session can be established between web clients and native clients over Nostr.
*   Encrypted messages can be sent and received between web clients and native clients.
*   The web application demonstrates full Bitchat functionality in a browser (minus BLE).
*   Cross-platform interoperability is verified between web and native clients.

## 3. Testing Infrastructure

### Local Nostr Relay Setup

For comprehensive testing and development, we will include a local Nostr relay in this repository to provide a controlled testing environment that doesn't depend on external services.

**Implementation Directive**: Keep test infrastructure simple and reliable. The relay should "just work" with minimal configuration.

**Implementation Plan**:
*   **Relay Choice**: Use a Rust-based Nostr relay for consistency with our tech stack
*   **Recommended**: [`nostr-rs-relay`](https://github.com/scsibug/nostr-rs-relay) - mature, well-maintained Rust implementation
*   **Location**: `/relay/` directory in this repository
*   **Configuration**: Minimal config for local testing, no authentication required
    - **Clean Setup**: Single configuration file with sensible defaults
    - **Zero Dependencies**: Should work without external databases or services
*   **Integration**: Docker setup for easy local deployment
    - **Simple Dockerfile**: Minimal image with just the relay binary
    - **Quick Start**: `docker run` command gets you running immediately
*   **Usage**: Used for all integration tests and local development
    - **Automated CI**: Relay starts automatically in test pipeline
    - **Development Workflow**: One command to start local testing environment

**Benefits**:
*   **Deterministic Testing**: Controlled environment with known state
*   **Offline Development**: No internet required for core development
*   **Performance Testing**: Measure relay performance and optimize accordingly
*   **Protocol Compliance**: Verify our implementation against a known-good relay
*   **CI/CD Integration**: Automated testing in continuous integration

**Setup Commands**:
```bash
# Run local relay for testing
cd relay/
cargo run --release

# Or using Docker
docker-compose up nostr-relay

# Connect clients to local relay
# ws://localhost:8080 (default nostr-rs-relay port)
```

### Excluded Features (For Now)

**Tor Integration**: Not implementing Tor support in initial phases. This can be added later as an additional transport layer or proxy configuration.

**Rationale**: 
*   Focus on core protocol implementation first
*   Tor adds significant complexity to networking layer
*   Can be implemented as a separate transport or proxy without changing core protocol
*   Current Swift implementation shows it's an optional enhancement

**Future Consideration**: Tor support could be added in Phase 5 as an optional networking proxy for enhanced privacy.

## Implementation Standards

### Code Quality Guidelines

**Conciseness**:
- Each module should have a single, clear responsibility
- Prefer composition over complex inheritance hierarchies
- Eliminate duplicate code through well-designed abstractions

**Cleanliness**:
- Use descriptive names that explain intent
- Keep functions small and focused (< 50 lines as a guideline)
- Maintain consistent error handling patterns throughout

**Elegance**:
- Design APIs that feel natural to use
- Use Rust's type system to enforce correctness
- Prefer explicit state over implicit assumptions
- Make common cases easy, rare cases possible

### Performance Targets

**Memory Usage**:
- Core protocol operations should not allocate unnecessarily
- Session management should have bounded memory usage
- Transport buffers should be reusable

**Latency**:
- Message encryption/decryption should complete in microseconds
- Session establishment should complete in < 100ms over local transport
- Cross-platform compatibility should not compromise performance

**Throughput**:
- Support 100+ concurrent sessions on modest hardware
- Handle message fragmentation efficiently for large payloads
- Maintain consistent performance across native and WASM targets
