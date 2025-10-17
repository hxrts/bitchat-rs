# Bitchat Rust

A performant Rust implementation of the [Bitchat protocol](https://bitchat.free/) with WebAssembly support for browsers.

## Overview

Decentralized, peer-to-peer messaging protocol designed for secure, private, and censorship-resistant communication over ephemeral mesh networks.

Features:
- End-to-end encryption using Noise Protocol Framework (XX pattern)
- BLE mesh networking with Nostr fallback
- Compiles to native and WebAssembly
- Zero servers, zero accounts

## Architecture

- **Transport:** BLE mesh (primary), Nostr relays via Tor (fallback)
- **Encryption:** Noise_XX_25519_ChaChaPoly_SHA256
- **Identity:** Ed25519 signatures, Curve25519 keys
- **Sync:** GCS filter-based gossip

See `docs/protocol-architecture.md` for complete specification.

## Building

### Native
```bash
nix develop              # Enter development environment
just build               # Build all crates
just test                # Run tests
just demo                # Run CLI demo (BLE + Nostr)
```

### WebAssembly
```bash
nix develop
cargo build --target wasm32-unknown-unknown --release
```

## Documentation

- `docs/protocol-architecture.md` - Complete protocol specification
- `docs/rust-library-recommendations.md` - Library choices and rationale
- `docs/phased-implementation-plan.md` - Development roadmap

## Reference Implementations

- [bitchat](https://github.com/bitchat-dev/bitchat) - iOS/macOS (main implementation)
- [bitchat-android](https://github.com/bitchat-dev/bitchat-android) - Android (production)
- [bitchat-tui](https://github.com/bitchat-dev/bitchat-tui) - Terminal UI (reference)

## License

Licensed under Apache 2.0, see [LICENSE](LICENSE).
