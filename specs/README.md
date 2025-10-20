# BitChat Protocol Specifications

This directory contains formal protocol specifications for the BitChat protocol in RON (Rusty Object Notation) format. These specifications serve as the canonical source of truth for protocol implementation and are used to generate both code and tests.

## Purpose

These RON specifications provide:

1. **Single Source of Truth**: Protocol behavior defined once, used everywhere
2. **Code Generation**: Typestate implementations generated via `build.rs`
3. **Test Generation**: Comprehensive test suites auto-generated from specs
4. **Protocol Documentation**: Machine-readable protocol definitions
5. **Canonical Compatibility**: Ensures Rust implementation matches Swift/iOS reference

## Specification Files

### `noise_xx.ron`
Defines the Noise XX handshake protocol used for end-to-end encryption.

**Contents:**
- Three-stage handshake flow (-> e, <- e ee s es, -> s se)
- Cryptographic primitives (Curve25519, ChaCha20-Poly1305, SHA256)
- State transitions for initiator and responder roles
- Invariants at each stage
- Fault injection points
- Canonical parameters (timeouts, key sizes)

**Used to generate:**
- Typestate structs: `NoiseSession<InitiatorStage1>`, etc.
- Handshake tests with all fault scenarios
- Dual protocol for responder (automatically computed)

### `session_lifecycle.ron`
Defines the session state machine from initialization through rekeying.

**Contents:**
- Session states: Uninitialized → Handshaking → Established → Rekeying → Terminated
- Valid state transitions
- Operations allowed in each state
- Rekey triggers (message count, time-based)
- Cleanup procedures
- Canonical parameters (thresholds, timeouts)

**Used to generate:**
- Session state enums and transition logic
- State machine tests (valid/invalid transitions)
- Rekey condition checks

### `message_types.ron`
Defines all BitChat message types and their wire format codes.

**Contents:**
- Packet message types (Announce, Message, NoiseHandshake, etc.)
- Noise payload types (PrivateMessage, ReadReceipt, etc.)
- Field definitions with validation rules
- Constraints and invariants
- Event generation specifications

**Used to generate:**
- `MessageType` and `NoisePayloadType` enums
- Message validation functions
- Serialization/deserialization tests

### `wire_format.ron`
Defines the binary packet structure and encoding rules.

**Contents:**
- Header structure (version, type, TTL, timestamp, flags, payload length)
- Variable field formats (sender_id, recipient_id, route, payload, signature)
- Encoding rules (big-endian, UTF-8)
- Size constraints (MTU limits, max packet sizes)
- Example packet structures
- Common implementation mistakes to avoid

**Used to generate:**
- Wire format encoder/decoder
- Packet header parsing logic
- Canonical compatibility tests

## Usage

### During Development

Query the specs to understand protocol behavior:
```bash
# View Noise handshake stages
cat specs/noise_xx.ron | grep -A 20 "stage_number: 1"

# Check message type codes
cat specs/message_types.ron | grep "wire_type:"
```

### Build-Time Code Generation

The `build.rs` script parses these RON files and generates Rust code:

```rust
// build.rs
use std::fs;
use serde::Deserialize;

fn main() {
    // Parse Noise spec
    let noise_spec: NoiseProtocolSpec = 
        ron::from_str(&fs::read_to_string("specs/noise_xx.ron")?)?;
    
    // Generate typestate implementations
    let generated_code = generate_typestates(&noise_spec);
    fs::write("src/generated/noise_session.rs", generated_code)?;
    
    // Re-run if specs change
    println!("cargo:rerun-if-changed=specs/noise_xx.ron");
}
```

### Test Generation

Tests are automatically generated from the specs:

```rust
// Generated from specs/noise_xx.ron
#[test]
fn test_noise_stage1_message_loss() {
    let transport = MockTransport::new();
    transport.set_fault(Fault::MessageLoss);
    
    let session = NoiseSession::<InitiatorStage1>::new(Box::new(transport));
    let result = session.send_ephemeral_key();
    
    assert!(matches!(result, Err(NoiseError::Timeout)));
}
```

## Canonical Compatibility

These specifications are based on the canonical Swift/iOS BitChat implementation and are verified against it through integration tests. Key compatibility notes:

- **Message type codes**: Exact values from canonical implementation (e.g., `Announce = 0x01`)
- **Wire format**: Big-endian encoding, 13-byte header for v1
- **Noise handshake**: Single `NoiseHandshake` type (not split into INIT/RESP/FINAL)
- **Rekey parameters**: 1 billion messages or 24 hours (90% threshold)
- **Fragment header**: 13 bytes (8+2+2+1)

See `docs/deepwiki-mcp-guide.md` for querying the canonical specification.

## Extending the Specifications

When adding new protocol features:

1. **Update the relevant RON file** with new messages, states, or operations
2. **Add validation rules** and constraints
3. **Specify fault injection points** for testing
4. **Mark experimental features** with `experimental: true`
5. **Run `cargo build`** to regenerate code
6. **Verify canonical compatibility** if feature exists in Swift implementation

Example adding a new message type:

```ron
// In specs/message_types.ron
(
    name: "NewFeature",
    wire_type: 0x40,  // Get next available code
    description: "New protocol feature",
    
    fields: [
        (name: "data", type: "Bytes", max_size_bytes: 1024, required: true)
    ],
    
    constraints: ["data_not_empty"],
    valid_states: ["Established"],
    requires_session: true,
    experimental: true  // Mark as experimental
)
```

## Validation

RON specs are validated during parsing. Common validation checks:

- Message type codes don't conflict
- State transitions are acyclic (except explicit loops)
- Field sizes are within packet limits
- Constraints reference valid fields
- Fault types are recognized

## Tools

**Recommended**: Use `ron` crate for parsing:
```toml
[build-dependencies]
ron = "0.8"
serde = { version = "1.0", features = ["derive"] }
```

**Validation**: Run the spec parser to check for errors:
```bash
cargo build 2>&1 | grep "specs/"
```

## References

- **Noise Protocol Framework**: https://noiseprotocol.org/
- **RON Format**: https://github.com/ron-rs/ron
- **DeepWiki (Canonical Spec)**: See `docs/deepwiki-mcp-guide.md`
- **Protocol Architecture**: `docs/protocol-architecture.md`
- **Simulation Evolution**: `docs/simulation-system-evolution.md` (Appendix B on typestate pattern)

## Questions?

For questions about:
- **Protocol behavior**: Query DeepWiki for canonical implementation
- **RON syntax**: See https://github.com/ron-rs/ron
- **Code generation**: Check `build.rs` and `docs/simulation-system-evolution.md`
- **Typestate pattern**: See Appendix B in `docs/simulation-system-evolution.md`

---

**Last Updated**: October 20, 2025  
**Spec Version**: 1.0  
**Based on**: Canonical BitChat Swift/iOS implementation (commit fb43a8)