#!/usr/bin/env node
/**
 * BitChat WASM Client Automation Runner
 * 
 * This Node.js script serves as a wrapper for the BitChat WASM implementation
 * to enable automation testing alongside native CLI clients.
 */

const fs = require('fs');
const path = require('path');

// Command line argument parsing
const args = process.argv.slice(2);
let automationMode = false;
let clientName = 'wasm-client';
let relayUrl = 'wss://relay.damus.io';

for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
        case '--automation-mode':
            automationMode = true;
            break;
        case '--name':
            clientName = args[++i];
            break;
        case '--relay':
            relayUrl = args[++i];
            break;
        case '--help':
            console.log('Usage: node wasm-runner.js [options]');
            console.log('Options:');
            console.log('  --automation-mode    Enable automation mode for testing');
            console.log('  --name <name>        Set client name');
            console.log('  --relay <url>        Set relay URL');
            console.log('  --help               Show this help');
            process.exit(0);
    }
}

if (!automationMode) {
    console.error('WASM runner currently only supports automation mode');
    process.exit(1);
}

/**
 * Emit an automation event for the test orchestrator
 */
function emitEvent(eventType, data = {}) {
    const event = {
        event: eventType,
        timestamp: Date.now(),
        client_name: clientName,
        client_type: 'wasm',
        ...data
    };
    console.log(JSON.stringify(event));
}

/**
 * Simple WASM client simulation
 * 
 * This is a placeholder implementation that simulates the WASM client behavior
 * for testing purposes. In a real implementation, this would load and run
 * the actual BitChat WASM module.
 */
class BitchatWasmClient {
    constructor(name, relayUrl) {
        this.name = name;
        this.relayUrl = relayUrl;
        this.isRunning = false;
        this.peers = new Map();
        this.discoveryActive = false;
        
        // Generate a mock peer ID for simulation
        this.peerId = this.generateMockPeerId();
    }
    
    generateMockPeerId() {
        const bytes = Array.from({length: 8}, () => Math.floor(Math.random() * 256));
        return bytes.map(b => b.toString(16).padStart(2, '0')).join('');
    }
    
    async start() {
        this.isRunning = true;
        emitEvent('Ready', {
            peer_id: this.peerId,
            relay_url: this.relayUrl
        });
        
        // Start command processing
        this.startCommandProcessor();
    }
    
    startCommandProcessor() {
        // Listen for commands from stdin
        process.stdin.setEncoding('utf8');
        process.stdin.on('data', (data) => {
            const command = data.toString().trim();
            this.handleCommand(command);
        });
    }
    
    async handleCommand(command) {
        const parts = command.split(' ');
        const cmd = parts[0];
        
        switch (cmd) {
            case 'send':
                const message = parts.slice(1).join(' ');
                this.handleSendMessage(message);
                break;
            case 'private':
                if (parts.length >= 3) {
                    const targetPeerId = parts[1];
                    const privateMessage = parts.slice(2).join(' ');
                    this.handlePrivateMessage(targetPeerId, privateMessage);
                }
                break;
            case 'discover':
                this.handleStartDiscovery();
                break;
            case 'stop-discovery':
                this.handleStopDiscovery();
                break;
            case 'connect':
                if (parts.length >= 2) {
                    this.handleConnectToPeer(parts[1]);
                }
                break;
            case 'status':
                this.handleGetStatus();
                break;
            case 'quit':
            case 'exit':
                this.handleShutdown();
                break;
            default:
                // Unknown command - ignore for now
                break;
        }
    }
    
    handleSendMessage(message) {
        // Simulate sending a broadcast message
        emitEvent('MessageSent', {
            content: message,
            timestamp: Date.now() / 1000,
            to: 'broadcast'
        });
    }
    
    handlePrivateMessage(targetPeerId, message) {
        // Simulate sending a private message
        emitEvent('MessageSent', {
            content: message,
            timestamp: Date.now() / 1000,
            to: targetPeerId
        });
    }
    
    handleStartDiscovery() {
        this.discoveryActive = true;
        emitEvent('DiscoveryStateChanged', {
            active: true,
            transport: 'nostr'
        });
        
        // Simulate discovering a peer after a short delay
        setTimeout(() => {
            const mockPeerId = this.generateMockPeerId();
            this.peers.set(mockPeerId, {
                discovered_at: Date.now(),
                transport: 'nostr'
            });
            
            emitEvent('PeerDiscovered', {
                peer_id: mockPeerId,
                transport: 'nostr',
                signal_strength: -42
            });
        }, 1000);
    }
    
    handleStopDiscovery() {
        this.discoveryActive = false;
        emitEvent('DiscoveryStateChanged', {
            active: false,
            transport: 'nostr'
        });
    }
    
    handleConnectToPeer(peerId) {
        emitEvent('ConnectionEstablished', {
            peer_id: peerId,
            transport: 'nostr'
        });
        
        // Simulate session establishment
        setTimeout(() => {
            emitEvent('SessionEstablished', {
                peer_id: peerId,
                session_id: `wasm-session-${Date.now()}`
            });
        }, 500);
    }
    
    handleGetStatus() {
        emitEvent('SystemStatusReport', {
            peer_count: this.peers.size,
            active_connections: Array.from(this.peers.keys()).length,
            message_count: 0,
            uptime_seconds: Math.floor((Date.now() - this.startTime) / 1000),
            transport_status: [{
                transport: 'nostr',
                status: 'active'
            }]
        });
    }
    
    handleShutdown() {
        this.isRunning = false;
        emitEvent('Shutdown', {});
        process.exit(0);
    }
}

// Main execution
async function main() {
    try {
        const client = new BitchatWasmClient(clientName, relayUrl);
        
        // Handle process termination
        process.on('SIGINT', () => {
            client.handleShutdown();
        });
        
        process.on('SIGTERM', () => {
            client.handleShutdown();
        });
        
        // Start the client
        await client.start();
        
        // Keep the process alive
        process.stdin.resume();
        
    } catch (error) {
        emitEvent('SystemError', {
            error: error.message
        });
        process.exit(1);
    }
}

main();