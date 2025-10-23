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
nix develop              # Enter development environment (optional)
just build               # Build all crates
just test                # Run tests
just demo                # Run CLI demo (BLE + Nostr)
```

### With Experimental Features
```bash
cargo build --features experimental
cargo test --features experimental
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

## Reference

- [bitchat](https://github.com/bitchat-dev/bitchat) - iOS/macOS (main implementation)
- [bitchat-android](https://github.com/bitchat-dev/bitchat-android) - Android (production)
- [deepwiki](https://deepwiki.com/permissionlesstech/bitchat) - generative docs

## License

Licensed under Apache 2.0, see [LICENSE](LICENSE).