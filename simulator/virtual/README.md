# BitChat Virtual Testing System

Fast, deterministic protocol simulation for BitChat testing.

## Overview

The virtual testing system provides **fast protocol simulation** for development and testing:
- **Fast** - Execute 1000+ tests in minutes
- **Deterministic** - Same input → same output, always
- **White-box** - Can inspect internal state
- **Mocked** - Network, time, and crypto all controlled

## Usage

The virtual system is accessed through the unified simulator interface:

```bash
# Test with virtual clients (from simulator/ directory)
just test-scenario 02_messaging_basic cli,cli
just test-scenario 01_discovery_basic wasm,wasm

# Direct access for development
cd virtual
just execute 02_messaging_basic
just list
just all
```

## Client Types

- **cli** - Command-line simulation client
- **wasm** - WebAssembly browser client

Both clients are implemented as mock/simulation versions for fast testing.

## Architecture

```
virtual/
├── src/
│   ├── main.rs                 # CLI entry point
│   ├── simulation_executor.rs  # Virtual time simulation
│   ├── network_router.rs       # Mocked network conditions
│   ├── clock.rs                # Virtual time control
│   ├── random.rs               # Deterministic randomness
│   ├── scenario_executor.rs    # TOML scenario execution
│   └── scenarios/              # Built-in scenario modules
├── harness/                    # Client abstraction layer
└── Justfile                    # Build automation
```

## Network Simulation

Configure network conditions in TOML scenarios:

```toml
[network]
latency_ms = 50
packet_loss = 0.05
bandwidth_limit = 56000
```

The virtual system simulates:
- Configurable latency and packet loss
- Network partitions and healing
- Message reordering and corruption
- Bandwidth limitations

## Development Commands

```bash
# Build and test
just build
just check
just test

# Clean up
just clean
just rebuild

# Legacy compatibility (deprecated)
just scenario my_test        # Use 'just execute my_test' instead
just test-all               # Use 'just all' instead
```

## Integration

The virtual system is integrated with the unified simulator interface. See the main [simulator README](../README.md) for complete usage instructions.

For real-world device testing, see the [device testing system](../device/README.md).