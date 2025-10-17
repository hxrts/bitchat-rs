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
    cargo test --test integration_tests

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
    @echo "  • wss://relay.damus.io"
    @echo "  • wss://nos.lol" 
    @echo "  • wss://relay.nostr.band"
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
    @echo "  • wss://relay.damus.io"
    @echo "  • wss://nos.lol" 
    @echo "  • wss://relay.nostr.band"
    @echo ""
    @echo "No local relay logs available."
    @echo "For debugging, check BitChat application logs with RUST_LOG=debug"

# Run BitChat CLI in chat mode with external relays  
chat-external name="TestUser":
    cargo run --bin bitchat -- chat --name "{{name}}"

# Run BitChat CLI in chat mode with default relays
chat name="TestUser":
    cargo run --bin bitchat -- chat --name "{{name}}"

# Send a test message
send-message message="Hello BitChat!" to="":
    #!/usr/bin/env bash
    if [ -n "{{to}}" ]; then
        cargo run --bin bitchat -- send --to "{{to}}" "{{message}}"
    else
        cargo run --bin bitchat -- send "{{message}}"
    fi

# List discovered peers
peers:
    cargo run --bin bitchat -- peers

# Run transport tests
test-transports:
    cargo run --bin bitchat -- test

# Build and run with BLE only
run-ble-only:
    cargo run --bin bitchat -- --no-nostr chat

# Build and run with Nostr only
run-nostr-only:
    cargo run --bin bitchat -- --no-ble chat

# Run full Phase 3 demo (both transports)
demo:
    @echo "Starting BitChat Phase 3 Demo..."
    @echo "This will start both BLE and Nostr transports"
    cargo run --bin bitchat -- chat --name "Demo User"

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