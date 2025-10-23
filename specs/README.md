# BitChat Protocol Specifications

Formal protocol specifications in RON (Rusty Object Notation) format, serving as the canonical source of truth for protocol implementation.

## Purpose

- **Single Source of Truth**: Protocol behavior defined once, used everywhere
- **Code Generation**: Typestate implementations generated via `build.rs`
- **Test Generation**: Comprehensive test suites auto-generated from specs
- **Canonical Compatibility**: Ensures Rust implementation matches Swift/iOS reference

## Specification Files

### `noise_xx.ron` (8.9 KB)
Noise XX handshake protocol with three-stage handshake (`-> e`, `<- e ee s es`, `-> s se`).

**Generates**: `NoiseSession<InitiatorStage1/2/3/Established>` typestate structs, ~30+ handshake tests with fault scenarios.

### `session_lifecycle.ron` (12 KB)
Session state machine: Uninitialized → Handshaking → Established → Rekeying → Terminated.

**Generates**: `SessionState` enum with compile-time transitions, rekey condition checking logic.

### `message_types.ron` (14 KB)
All BitChat message types and wire format codes (11 packet types, 18 noise payload types).

**Generates**: `MessageType` and `NoisePayloadType` enums, message validation functions.

### `wire_format.ron` (12 KB)
Binary packet structure and encoding rules (13-byte v1 header, big-endian encoding).

**Generates**: `PacketHeader` serialization/deserialization, wire format encoder/decoder.

## Usage

### Build-Time Code Generation
```rust
// build.rs
let noise_spec: NoiseProtocolSpec = ron::from_str(
    &fs::read_to_string("specs/noise_xx.ron")?
)?;

let generated = generate_typestates(&noise_spec);
fs::write("src/generated/noise_session.rs", generated)?;
```

### Query During Development
```bash
# View Noise handshake stages
cat specs/noise_xx.ron | grep -A 20 "stage_number: 1"

# Check message type codes
cat specs/message_types.ron | grep "wire_type:"
```

## Canonical Compatibility

Specifications match the canonical Swift/iOS BitChat implementation (commit fb43a8):

| Aspect | Canonical Value | Status |
|--------|----------------|--------|
| Noise Protocol | `Noise_XX_25519_ChaChaPoly_SHA256` | Yes |
| Noise Handshake Type | Single type (0x10) | Yes |
| Header Size (v1) | 13 bytes | Yes |
| Rekey Threshold | 1B messages, 90% trigger | Yes |
| Handshake Timeout | 30 seconds | Yes |

## Adding New Features

1. **Update relevant RON file** with new messages/states
2. **Add validation rules** and constraints
3. **Run `cargo build`** to regenerate code
4. **Verify canonical compatibility**

Example:
```ron
// In specs/message_types.ron
(
    name: "NewFeature",
    wire_type: 0x40,
    fields: [(name: "data", type: "Bytes", max_size_bytes: 1024)],
    constraints: ["data_not_empty"],
    valid_states: ["Established"],
    experimental: true
)
```

## Files Overview

```
specs/
├── noise_xx.ron          # 8.9 KB - Noise handshake protocol
├── session_lifecycle.ron # 12 KB  - Session state machine  
├── message_types.ron     # 14 KB  - Message type definitions
├── wire_format.ron       # 12 KB  - Binary packet structure
└── README.md             # This file
```

**Total**: 4 specification files, ~47 KB of formal protocol definitions

## References

- **Noise Protocol Framework**: https://noiseprotocol.org/
- **RON Format**: https://github.com/ron-rs/ron
- **Protocol Architecture**: `docs/protocol-architecture.md`
- **DeepWiki Guide**: `docs/deepwiki-mcp-guide.md`

---

**Generated**: October 20, 2025  
**Source**: Canonical BitChat Swift/iOS implementation (commit fb43a8)  
**Spec Version**: 1.0