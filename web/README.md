# BitChat Web Demo - Phase 4

This directory contains the web demo for BitChat Phase 4, showcasing WebAssembly compilation and browser-based communication via Nostr relays.

## Features

- **WebAssembly Integration**: BitChat core protocol compiled to WASM
- **Nostr Transport**: Browser-compatible Nostr relay communication
- **Real-time Messaging**: Send and receive messages in real-time
- **Peer Discovery**: Discover other BitChat peers via Nostr
- **Cross-platform Communication**: Web clients can communicate with native clients

## Quick Start

1. **Build the WASM module**:
   ```bash
   just build-wasm
   ```

2. **Start the web server**:
   ```bash
   just serve-web
   ```

3. **Open in browser**:
   Navigate to `http://localhost:8000`

## Usage

### Connecting
1. Enter your display name
2. Configure Nostr relay URLs (default relays are provided)
3. Click "Connect" to join the BitChat network

### Messaging
1. Type your message in the message box
2. Optionally specify a recipient peer ID for direct messages
3. Leave recipient empty for broadcast messages
4. Click "Send Message" or press Enter

### Monitoring
- **Status Panel**: Shows connection status, peer ID, and message statistics
- **Peers Panel**: Lists discovered BitChat peers
- **Debug Log**: Shows detailed connection and message logs

## Technical Details

### Architecture
- **WASM Module**: `bitchat-web` crate compiled to WebAssembly
- **JavaScript Interface**: Clean API for web integration
- **Nostr Transport**: WebSocket-based communication with Nostr relays
- **Real-time Updates**: Async message handling with JavaScript Promises

### Browser Compatibility
- **Modern Browsers**: Chrome, Firefox, Safari, Edge (latest versions)
- **WebAssembly Support**: Required for WASM module execution
- **WebSocket Support**: Required for Nostr relay communication

### Limitations
- **No Bluetooth**: Web browsers don't support BLE peer-to-peer communication
- **Nostr Only**: Web clients communicate exclusively via Nostr relays
- **CORS Restrictions**: Some relays may have CORS policies that affect browser access

## Development

### Building for Different Targets

**Web (ES modules)**:
```bash
just build-wasm
```

**Node.js**:
```bash
just build-wasm-node
```

**Bundlers (webpack, rollup)**:
```bash
just build-wasm-bundler
```

### Cleaning Build Artifacts
```bash
just clean-wasm
```

### Local Development
The demo uses a simple HTTP server for local development. For production deployment, serve the files through any web server that supports:
- Static file serving
- MIME type application/wasm for .wasm files
- ES module support for .js files

## Interoperability

Web clients can communicate with native BitChat clients (Phase 3) through shared Nostr relays. Both implementations use the same:
- Wire protocol format
- Encryption (Noise Protocol)
- Message structure
- Nostr event format

This enables seamless communication between web browsers and native applications.

## Security

- **End-to-End Encryption**: All messages encrypted using Noise Protocol
- **Ephemeral Keys**: New cryptographic keys generated for each session
- **Relay Privacy**: Nostr relays only see encrypted message metadata
- **No Key Storage**: Private keys exist only in browser memory during session