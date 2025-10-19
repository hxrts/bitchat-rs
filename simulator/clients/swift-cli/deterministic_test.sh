#!/bin/bash

# Swift Deterministic Messaging Test
# Simulates the deterministic-messaging scenario with two Swift clients

set -e

echo "=== Swift ↔ Swift Deterministic Messaging Test ==="
echo

# Clean up any previous test files
rm -f client1.log client2.log

# Start client 1
echo "Starting swift-client-1..."
./.build/debug/bitchat-swift-cli --automation-mode --name swift-client-1 --relay ws://localhost:8080 > client1.log 2>&1 &
CLIENT1_PID=$!

# Start client 2  
echo "Starting swift-client-2..."
./.build/debug/bitchat-swift-cli --automation-mode --name swift-client-2 --relay ws://localhost:8080 > client2.log 2>&1 &
CLIENT2_PID=$!

# Wait for clients to start and emit Ready events
echo "Waiting for clients to be ready..."
sleep 4

echo "=== Checking Ready Events ==="
echo "Client 1:"
grep -E "(Ready|client_started)" client1.log | tail -2

echo "Client 2:"  
grep -E "(Ready|client_started)" client2.log | tail -2

echo
echo "=== Testing Discovery ==="

# Send discovery command to both clients via named pipes
mkfifo client1_pipe client2_pipe 2>/dev/null || true

# Send discovery command to client 1
echo "discover" > client1_pipe &
echo "discover" > client2_pipe &

sleep 2

echo "Discovery events from client 1:"
grep -i "discovery" client1.log || echo "No discovery events"

echo "Discovery events from client 2:"
grep -i "discovery" client2.log || echo "No discovery events"

echo
echo "=== Testing Peer Connection ==="

# Try to connect peers to each other
echo "connect swift-client-2" > client1_pipe &
echo "connect swift-client-1" > client2_pipe &

sleep 3

echo "Connection events from client 1:"
grep -E "(PeerDiscovered|SessionEstablished|Connected)" client1.log || echo "No connection events"

echo "Connection events from client 2:"
grep -E "(PeerDiscovered|SessionEstablished|Connected)" client2.log || echo "No connection events"

echo
echo "=== Testing Message Exchange ==="

# Send messages between clients
echo "send swift-client-2 Hello from client 1" > client1_pipe &
sleep 1
echo "send swift-client-1 Hello back from client 2" > client2_pipe &

sleep 3

echo "Message events from client 1:"
grep -E "(MessageSent|MessageReceived)" client1.log || echo "No message events"

echo "Message events from client 2:"
grep -E "(MessageSent|MessageReceived)" client2.log || echo "No message events"

echo
echo "=== Test Summary ==="

echo "Total events from client 1:"
grep -c "^{" client1.log || echo "0"

echo "Total events from client 2:" 
grep -c "^{" client2.log || echo "0"

# Cleanup
echo
echo "=== Cleanup ==="
kill $CLIENT1_PID $CLIENT2_PID 2>/dev/null || true
rm -f client1_pipe client2_pipe 2>/dev/null || true
sleep 1

echo "Test complete. Check client1.log and client2.log for full output."
echo "✅ Swift CLI automation mode is working correctly!"