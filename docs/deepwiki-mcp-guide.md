# DeepWiki MCP Server Guide for BitChat Development

## Overview

This guide explains how to use the DeepWiki MCP (Model Context Protocol) server to access the canonical BitChat specification during development of the Rust implementation. The DeepWiki server provides structured documentation of the Swift/iOS reference implementation, enabling cross-implementation compatibility verification.

## What is DeepWiki?

DeepWiki is an MCP server that indexes and serves structured documentation from codebases. For BitChat, it provides comprehensive documentation of the canonical Swift/iOS implementation maintained by the BitChat project.

**Website**: https://deepwiki.com/permissionlesstech/bitchat  
**MCP Endpoint**: https://mcp.deepwiki.com/  
**Last Indexed**: October 19, 2025 (commit fb43a8)

## MCP Server Configuration

The DeepWiki server is configured in `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "deepwiki": {
      "url": "https://mcp.deepwiki.com/"
    }
  }
}
```

This configuration enables AI assistants in Cursor to query the BitChat specification directly during development.

## Available Documentation Sections

The DeepWiki server provides documentation organized into the following major sections:

### 1. Architecture & Overview
- **Overview**: High-level introduction, purpose, and core principles
- **Dual Transport Architecture**: BLE mesh + Nostr relay architecture
- **Key Features and Privacy Model**: Design philosophy and privacy features
- **Core Application Architecture**: MVVM pattern and component organization

### 2. Application Layer
- **ChatViewModel - Central Coordinator**: Main business logic coordinator
- **Service Layer Architecture**: Service composition and coordination
- **Application Lifecycle and State Management**: App lifecycle handling
- **User Interface Components**: SwiftUI views and reactive patterns

### 3. Transport Layer
- **Bluetooth Mesh Network**: BLE advertising, discovery, and mesh routing
- **Nostr Protocol Implementation**: Relay management and event handling
- **Message Routing and Transport Selection**: Intelligent transport failover
- **Transport Configuration**: UUID scheme, MTU limits, connection parameters

### 4. Cryptography & Security
- **Noise Protocol Framework**: Noise_XX_25519_ChaChaPoly_SHA256 implementation
- **Identity Management and Trust System**: Fingerprints, verification, trust levels
- **Keychain and Secure Storage**: iOS/macOS Keychain integration
- **Nostr Identity Bridge and Privacy**: Per-geohash identity derivation

### 5. Location & Geohash
- **Geohash Channel System**: Location-based channel organization
- **Location Services Integration**: CoreLocation integration
- **Location Notes System**: Geographic message persistence
- **Privacy Considerations**: Location privacy and identity unlinkability

### 6. Protocol Implementation
- **BitChat Protocol Definitions**: Packet format and message structure
- **Binary Encoding System**: Canonical wire format specification
- **Message Fragmentation and Reassembly**: MTU-limited transport handling
- **Rate Limiting and Spam Protection**: DoS prevention mechanisms
- **Command Processing System**: IRC-style command handling

### 7. Data Models & State
- **Message and Packet Models**: Core data structures
- **PeerID and Identity Abstraction**: Identity representation
- **State Persistence and Memory Management**: Ephemeral vs. persistent data
- **Private Chat Management**: Direct messaging state

### 8. Build & Configuration
- **Xcode Project Structure**: iOS/macOS build configuration
- **Application Configuration**: Runtime parameters and defaults
- **Dependencies and External Libraries**: Third-party dependency management

### 9. Extensions & Integrations
- **Share Extension**: iOS share sheet integration
- **Notification System**: Push notification handling
- **URL Scheme and Deep Linking**: App URL handling
- **Peer Discovery and UnifiedPeerService**: Cross-transport peer tracking

### 10. Testing & Quality
- **Test Architecture and Organization**: Test structure and patterns
- **Mock Infrastructure**: Test doubles and simulation
- **Integration and E2E Test Scenarios**: End-to-end test cases

## How to Query the DeepWiki Server

### Using AI Assistants in Cursor

When working in Cursor with the MCP server configured, you can ask questions that will automatically query the DeepWiki documentation:

**Good Query Examples:**

```
"According to the canonical BitChat spec, how should Noise session rekeying work?"

"What are the exact BLE UUID values used in the canonical implementation?"

"How does the canonical BitChat implementation handle geohash channel identity derivation?"

"What is the canonical wire format for BitChat packets?"

"How does the canonical implementation perform message fragmentation for BLE transport?"
```

**Query Best Practices:**

1. **Be specific**: Reference specific components or features
2. **Ask about canonical behavior**: Emphasize you want the reference implementation's approach
3. **Focus on protocol details**: Query wire formats, algorithms, and state machines
4. **Request implementation patterns**: Ask how specific features are architected

### Query Patterns for Different Development Tasks

#### When Implementing New Features

```
"How does the canonical BitChat implementation handle [feature]?"
"What are the state transitions for [component] in the reference implementation?"
"What configuration parameters does the canonical implementation use for [subsystem]?"
```

#### When Debugging Protocol Compatibility

```
"What is the exact binary format for [packet type] in the canonical spec?"
"How should [edge case] be handled according to the reference implementation?"
"What are the canonical timeout values for [operation]?"
```

#### When Refactoring Architecture

```
"How is [component] organized in the canonical implementation?"
"What design patterns does the reference implementation use for [subsystem]?"
"How does the canonical implementation separate concerns in [layer]?"
```

## Cross-Implementation Development Strategy

The Rust implementation in this repository aims for canonical compatibility with the Swift/iOS reference implementation. Use DeepWiki to:

### 1. Protocol Compatibility Verification

**Check against canonical behavior:**
- Wire format encoding/decoding
- Packet structure and field ordering
- Cryptographic parameter selection
- Timeout and retry behavior

**Example workflow:**
1. Implement feature in Rust
2. Query DeepWiki for canonical behavior
3. Compare implementations
4. Run integration tests against canonical clients

### 2. Feature Parity Tracking

The Rust implementation distinguishes between:

**Canonical Features** (always enabled):
- Core messaging and encryption
- BLE mesh networking
- Nostr relay fallback
- Message fragmentation
- Location-based channels

**Experimental Features** (opt-in via `--features experimental`):
- File transfer
- Group messaging
- Multi-device sync
- Capability negotiation

Use DeepWiki to verify which features exist in the canonical implementation before promoting experimental features to canonical status.

### 3. Test Case Generation

Use DeepWiki documentation to:
- Identify edge cases from canonical implementation
- Extract expected behavior for test assertions
- Generate compatibility test scenarios
- Validate protocol compliance

### 4. Documentation Synchronization

When updating `docs/protocol-architecture.md` or other specification documents, cross-reference with DeepWiki to ensure alignment with canonical behavior.

## Effective Usage Tips

### 1. Combine with Local Documentation

**Use DeepWiki for:**
- Canonical behavior and reference implementation details
- Swift/iOS-specific implementation patterns
- UI/UX patterns from the reference app
- Test scenarios and edge cases

**Use local docs/ for:**
- Rust-specific architecture decisions
- CSP/channel-based design patterns
- WASM/browser-specific adaptations
- Simulator and testing infrastructure

### 2. Map Canonical Components to Rust Equivalents

| Canonical (Swift) | Rust Implementation | Notes |
|-------------------|---------------------|-------|
| `ChatViewModel` | `CoreLogicTask` | Central business logic |
| `BLEService` | `bitchat-ble` crate | BLE transport |
| `NostrRelayManager` | `bitchat-nostr` crate | Nostr transport |
| `NoiseEncryptionService` | `protocol::crypto` module | Noise Protocol |
| `MessageRouter` | `logic::handlers` | Transport selection |
| `SecureIdentityStateManager` | `identity::manager` | Identity + trust |
| `NostrIdentityBridge` | `geohash` module | Per-geohash identities |

### 3. Protocol Compatibility Checklist

When implementing protocol-level features, verify against DeepWiki:

- [ ] Binary wire format matches canonical spec
- [ ] Packet header flags align with canonical implementation
- [ ] Cryptographic parameters match (key sizes, algorithms)
- [ ] Timeout values align with canonical defaults
- [ ] Error handling follows canonical patterns
- [ ] State machine transitions match canonical behavior

### 4. Integration Testing Strategy

Use DeepWiki to inform integration tests:

```rust
// Test based on canonical behavior from DeepWiki
#[tokio::test]
async fn test_canonical_noise_handshake() {
    // DeepWiki: Noise_XX_25519_ChaChaPoly_SHA256
    // 3-way handshake: -> e, <- e ee s es, -> s se
    
    // Verify handshake matches canonical pattern
    // ...
}
```

### 5. When Implementations Diverge

If the Rust implementation must diverge from the canonical approach:

1. **Document the reason** in code comments
2. **Reference DeepWiki** for canonical behavior
3. **Add compatibility layer** if needed for wire protocol
4. **Update `docs/implementation-completeness-analysis.md`**
5. **Consider experimental feature flag** if significant

Example:

```rust
// NOTE: Canonical implementation (per DeepWiki) uses Objective-C GCD for
// background BLE scanning. Rust uses tokio tasks, but wire protocol remains
// identical for cross-platform compatibility.
```

## Common DeepWiki Queries for BitChat Development

### Protocol & Wire Format

```
"What is the exact structure of a BitChat packet header?"
"How are fragment IDs generated in the canonical implementation?"
"What byte order is used for multi-byte fields in BitChat packets?"
```

### Cryptography

```
"What are the Noise Protocol handshake message sizes?"
"How long are Noise static keys stored in the canonical implementation?"
"When should Noise sessions be rekeyed according to the canonical spec?"
```

### Transport Layer

```
"What are the BLE service and characteristic UUIDs?"
"How does the canonical implementation handle BLE connection timeouts?"
"What Nostr event kind is used for BitChat messages?"
```

### Identity & Privacy

```
"How is the PeerID derived from a Noise public key?"
"What HKDF parameters are used for geohash identity derivation?"
"How are fingerprints calculated and displayed?"
```

### State Management

```
"How long are messages kept in memory in the canonical implementation?"
"What triggers session cleanup in the reference implementation?"
"How does the canonical implementation handle peer ID rotation?"
```

## Limitations & Considerations

### DeepWiki Limitations

1. **Snapshot in Time**: Documentation reflects a specific commit (fb43a8 as of Oct 19, 2025)
2. **Swift/iOS Focus**: May include platform-specific details not relevant to Rust
3. **Implementation vs. Spec**: Documents implementation, not always formal specification
4. **UI/UX Patterns**: Includes SwiftUI patterns that don't translate directly

### When NOT to Use DeepWiki

**Don't use DeepWiki for:**
- Rust-specific idioms and patterns
- WASM/browser-specific adaptations
- CSP channel architecture decisions
- Simulator infrastructure design
- Nix build system configuration

**Do use local documentation for:**
- `docs/runtime-architecture-rfc.md` - Rust architecture
- `docs/crate-dependency-diagram.md` - Crate organization
- `simulator/README.md` - Testing infrastructure
- `docs/browser-p2p-architecture.md` - WASM specifics

## Integration with Testing Infrastructure

### Canonical Compatibility Tests

The Rust implementation includes canonical compatibility tests in:
- `crates/bitchat-core/tests/canonical_compatibility.rs`
- `tests/canonical_compatibility.rs`

Use DeepWiki to inform these tests:

```rust
#[test]
fn test_canonical_packet_encoding() {
    // Based on canonical wire format from DeepWiki
    let packet = BitchatPacket { /* ... */ };
    let encoded = WireFormat::encode(&packet).unwrap();
    
    // Verify structure matches canonical implementation
    assert_eq!(encoded[0], PROTOCOL_VERSION); // Header byte
    // ...
}
```

### Cross-Implementation Testing

The `simulator/emulator-rig/` directory provides infrastructure for testing against real iOS and Android apps:

1. Query DeepWiki for expected behavior
2. Configure test scenario in `simulator/scenarios/*.toml`
3. Run cross-platform tests: `cargo run -- test --client1 ios --client2 rust`
4. Verify protocol compatibility

## Reference Links

- **DeepWiki Documentation**: https://deepwiki.com/permissionlesstech/bitchat
- **MCP Server**: https://mcp.deepwiki.com/
- **Canonical Repository**: https://github.com/permissionlesstech/bitchat (Swift/iOS)
- **This Repository**: Rust implementation (CLI, Web, BLE, Nostr)
- **Protocol Docs**: `docs/protocol-architecture.md` (this repository)

## Quick Reference Card

| Task | Query DeepWiki For | Use Local Docs For |
|------|-------------------|-------------------|
| Wire format | Canonical packet structure | Rust serialization impl |
| Crypto params | Noise Protocol configuration | Rust crypto library usage |
| BLE UUIDs | Service/characteristic IDs | Rust BLE transport impl |
| Geohash derivation | HKDF parameters | Rust geohash module |
| Session lifecycle | State transitions | CSP task orchestration |
| Transport selection | Routing logic | Channel-based dispatch |
| Error handling | Expected error conditions | Rust error types |
| Testing | Test scenarios & edge cases | Simulator infrastructure |

## Conclusion

The DeepWiki MCP server provides essential access to the canonical BitChat specification during Rust implementation development. By combining DeepWiki queries for protocol-level details with local documentation for Rust-specific architecture, developers can maintain canonical compatibility while leveraging Rust's strengths.

**Key Takeaways:**

1. **Query DeepWiki** for canonical protocol behavior and wire formats
2. **Use local docs** for Rust architecture and platform-specific details
3. **Verify compatibility** through integration tests against canonical clients
4. **Document divergences** when Rust implementation differs from canonical
5. **Maintain test matrix** to track canonical compatibility status

**Last Updated**: October 20, 2025  
**DeepWiki Last Indexed**: October 19, 2025 (commit fb43a8)

