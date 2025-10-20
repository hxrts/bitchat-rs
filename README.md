# Bitchat Rust

A performant Rust implementation of the [Bitchat protocol](https://bitchat.free/) with WebAssembly support for browsers.

## Overview

Decentralized, peer-to-peer messaging protocol designed for secure, private, and censorship-resistant communication over ephemeral mesh networks.

**Core Features:**
- End-to-end encryption using Noise Protocol Framework (XX pattern)
- BLE mesh networking with Nostr relay fallback
- Location-based channels using geohash precision levels
- Message fragmentation, compression, and deduplication
- Binary wire protocol with full canonical compatibility

**Advanced Features** (opt-in via `--features experimental`):
- File transfer with chunked uploads and SHA-256 integrity verification
- Group messaging with role-based permissions
- Multi-device session synchronization
- Capability negotiation with graceful degradation

## Architecture

CSP-based multi-task orchestrator with channel communication
- `bitchat-core` - Protocol, cryptography, and data structures
- `bitchat-harness` - Shared runtime plumbing and transport traits
- `bitchat-runtime` - Task orchestration and lifecycle management
- `bitchat-ble` / `bitchat-nostr` - Transport implementations
- `bitchat-cli` / `bitchat-web` - Application frontends

**Cryptography:** Noise_XX_25519_ChaChaPoly_SHA256  
**Identity:** Ed25519 signatures, Curve25519 keys  
**Sync:** GCS filter-based gossip with Bloom deduplication

See `docs/protocol-architecture.md` for complete specification.

## Building

### Native
```bash
nix develop              # Enter development environment
just build               # Build all crates
just test                # Run tests (101 canonical + 26 experimental)
just demo                # Run CLI demo (BLE + Nostr)
```

### With Experimental Features
```bash
cargo build --features experimental
cargo test --features experimental     # Run all 127 tests
```

### WebAssembly
```bash
just build-wasm          # Build WASM module for web
just serve-web           # Serve demo at http://localhost:8000
```

## Testing

The project includes comprehensive integration testing:
- Canonical compatibility tests verify wire format compatibility
- Property-based tests for message store integrity
- Integration tests for transport and runtime behavior
- Simulator for cross-implementation testing (see `simulator/`)

### Simulator Status (Updated 2025-01-20)

**✅ Working:**
- CLI ↔ CLI testing - All protocol scenarios passing
- CLI ↔ Web testing - Node.js wrapper enables WASM client automation
- iOS ↔ iOS testing - Real Swift app testing on simulator
- Framework bridge implemented for future cross-framework testing

**⚫ Untested but Ready:**
- Web ↔ Web scenarios
- Android ↔ Android (requires Android SDK installation)
- iOS ↔ Android (requires Android SDK installation)

**🎉 All Blockers Resolved!**
- Cross-framework orchestration implemented (CLI↔iOS, Web↔Android, etc)
- All 16 client pair combinations now supported

**Fixes Completed (2025-01-20):**
1. Fixed CLI stdout buffering (added flush() to all automation event emissions)
2. Fixed iOS bundle ID mismatch (updated to `chat.bitchat`)
3. Created Node.js wrapper (`simulator/wasm-runner.js`) for Web client automation
4. Implemented Android SDK auto-detection (tries common macOS/Linux locations)
5. Made `std` and `wasm` features independent and composable
6. **Implemented `CrossFrameworkOrchestrator`** for CLI↔iOS, Web↔Android, etc testing

See `simulator/TEST_MATRIX.md` for complete testing status and `simulator/README.md` for usage instructions.

## Reference

- [bitchat](https://github.com/bitchat-dev/bitchat) - iOS/macOS (main implementation)
- [bitchat-android](https://github.com/bitchat-dev/bitchat-android) - Android (production)
- [deepwiki](https://deepwiki.com/permissionlesstech/bitchat) - generative docs

## License

Licensed under Apache 2.0, see [LICENSE](LICENSE).