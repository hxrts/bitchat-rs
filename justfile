# BitChat Development Tasks
# Use `just --list` to see available commands

# Default recipe - show help
default:
    @echo "BitChat Development Tasks"
    @echo "========================"
    @echo ""
    @echo "Available commands:"
    @just --list

# Build all crates
build:
    cargo build --workspace

# Run tests
test:
    cargo test --workspace

# Run tests with output
test-verbose:
    cargo test --workspace -- --nocapture

# Run only integration tests
test-integration:
    cargo test --test integration_decomposed_runtime
    cargo test --test integration_tests_csp
    cargo test --workspace --test '*integration*'

# Check code without building
check:
    cargo check --workspace

# Format code
fmt:
    cargo fmt --all

# Run clippy lints
clippy:
    cargo clippy --workspace -- -D warnings

# Clean build artifacts
clean:
    cargo clean

# Setup external relay configuration for testing
setup-relay:
    @echo "BitChat Nostr Transport Testing"
    @echo "=============================="
    @echo ""
    @echo "BitChat will use external Nostr relays for testing:"
    @echo "  - wss://relay.damus.io"
    @echo "  - wss://nos.lol" 
    @echo "  - wss://relay.nostr.band"
    @echo ""
    @echo "No local relay setup needed!"
    @echo "These relays are configured by default in the Nostr transport."

# Start BitChat with external relays (no local relay needed)
start-relay:
    @echo "No local relay needed!"
    @echo "BitChat uses external Nostr relays by default."
    @echo "Run 'just demo' to test with external relays."

# Stop relay command (no-op since we use external relays)
stop-relay:
    @echo "No local relay to stop."
    @echo "BitChat uses external Nostr relays."

# View relay information
relay-logs:
    @echo "BitChat uses external Nostr relays:"
    @echo "  - wss://relay.damus.io"
    @echo "  - wss://nos.lol" 
    @echo "  - wss://relay.nostr.band"
    @echo ""
    @echo "No local relay logs available."
    @echo "For debugging, check BitChat application logs with RUST_LOG=debug"

# Run BitChat CLI in chat mode with external relays  
chat-external name="TestUser":
    cargo run --bin bitchat-cli -- chat --name "{{name}}"

# Run BitChat CLI in chat mode with default relays
chat name="TestUser":
    cargo run --bin bitchat-cli -- chat --name "{{name}}"

# Send a test message
send-message message="Hello BitChat!" to="":
    #!/usr/bin/env bash
    if [ -n "{{to}}" ]; then
        cargo run --bin bitchat-cli -- send --to "{{to}}" "{{message}}"
    else
        cargo run --bin bitchat-cli -- send "{{message}}"
    fi

# List discovered peers
peers:
    cargo run --bin bitchat-cli -- peers

# Run transport tests
test-transports:
    @echo "Running transport-specific tests..."
    cargo test -p bitchat-nostr --lib
    cargo test -p bitchat-runtime --lib --features testing

# Build and run with BLE only
run-ble-only:
    cargo run --bin bitchat-cli -- --no-nostr chat

# Build and run with Nostr only
run-nostr-only:
    cargo run --bin bitchat-cli -- --no-ble chat

# Run full Phase 3 demo (both transports)
demo:
    @echo "Starting BitChat Phase 3 Demo..."
    @echo "This will start both BLE and Nostr transports"
    cargo run --bin bitchat-cli -- chat --name "Demo User"

# Quick development check (format + clippy + test)
dev-check:
    just fmt
    just clippy
    just test

# Release build
release:
    cargo build --workspace --release

# Generate documentation
docs:
    cargo doc --workspace --no-deps --open

# Build WASM module for web
build-wasm:
    @echo "Building BitChat WASM module..."
    @echo "Checking for wasm-pack..."
    @which wasm-pack || (echo "wasm-pack not found. Install with: cargo install wasm-pack" && exit 1)
    @echo "Building for web target..."
    cd crates/bitchat-web && wasm-pack build --target web --out-dir ../../web/pkg --out-name bitchat-web
    @echo "WASM build complete. Files generated in web/pkg/"

# Build WASM for Node.js
build-wasm-node:
    @echo "Building BitChat WASM module for Node.js..."
    @which wasm-pack || (echo "wasm-pack not found. Install with: cargo install wasm-pack" && exit 1)
    cd crates/bitchat-web && wasm-pack build --target nodejs --out-dir ../../web/pkg-node --out-name bitchat-web
    @echo "Node.js WASM build complete. Files generated in web/pkg-node/"

# Build WASM for bundlers (webpack, rollup, etc.)
build-wasm-bundler:
    @echo "Building BitChat WASM module for bundlers..."
    @which wasm-pack || (echo "wasm-pack not found. Install with: cargo install wasm-pack" && exit 1)
    cd crates/bitchat-web && wasm-pack build --target bundler --out-dir ../../web/pkg-bundler --out-name bitchat-web
    @echo "Bundler WASM build complete. Files generated in web/pkg-bundler/"

# Clean WASM build artifacts
clean-wasm:
    @echo "Cleaning WASM build artifacts..."
    rm -rf web/pkg web/pkg-node web/pkg-bundler
    @echo "WASM artifacts cleaned"

# Serve web demo locally
serve-web:
    @echo "Starting local web server for BitChat demo..."
    @echo "Make sure to run 'just build-wasm' first"
    @echo "Demo will be available at http://localhost:8000"
    cd web && python3 -m http.server 8000

# Run Phase 4 demo (Web + WASM)
demo-web:
    @echo "Starting BitChat Phase 4 Demo (Web + WASM)..."
    @echo "Building WASM module..."
    just build-wasm
    @echo "Starting web server..."
    @echo "Open http://localhost:8000 in your browser"
    just serve-web