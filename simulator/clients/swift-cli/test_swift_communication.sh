#!/bin/bash

# Swift-to-Swift Communication Test
# This script tests basic messaging between two Swift CLI clients

set -e

echo "=== Swift ↔ Swift Communication Test ==="
echo

# Start first client in background
echo "Starting swift-client-1..."
./.build/debug/bitchat-swift-cli --automation-mode --name swift-client-1 --relay ws://localhost:8080 > client1.log 2>&1 &
CLIENT1_PID=$!

# Start second client in background  
echo "Starting swift-client-2..."
./.build/debug/bitchat-swift-cli --automation-mode --name swift-client-2 --relay ws://localhost:8080 > client2.log 2>&1 &
CLIENT2_PID=$!

# Wait for clients to start
echo "Waiting for clients to start..."
sleep 3

echo "Client 1 Ready Events:"
grep "Ready" client1.log || echo "No Ready event found"

echo
echo "Client 2 Ready Events:"
grep "Ready" client2.log || echo "No Ready event found"

echo
echo "=== Testing Discovery ==="

# Test discovery command
echo "discover" | ./.build/debug/bitchat-swift-cli --automation-mode --name test-discovery --relay ws://localhost:8080 > discovery.log 2>&1 &
DISCOVERY_PID=$!

sleep 2

echo "Discovery Events:"
grep -E "(PeerDiscovered|DiscoveryStateChanged)" discovery.log || echo "No discovery events found"

echo
echo "=== Testing Message Send ==="

# Test message sending (this is simulated since we don't have a real BitChat protocol running)
echo "send swift-client-2 Hello from client 1" | ./.build/debug/bitchat-swift-cli --automation-mode --name message-sender --relay ws://localhost:8080 > sender.log 2>&1 &
SENDER_PID=$!

sleep 2

echo "Message Events:"
grep -E "(MessageSent|MessageReceived)" sender.log || echo "No message events found"

# Cleanup
echo
echo "=== Cleanup ==="
kill $CLIENT1_PID $CLIENT2_PID $DISCOVERY_PID $SENDER_PID 2>/dev/null || true
sleep 1

echo "Test complete. Log files: client1.log, client2.log, discovery.log, sender.log"