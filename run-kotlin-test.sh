#!/bin/bash
set -e

echo "Running Kotlin ↔ Kotlin Control Test"
echo "===================================="

# Navigate to project root
cd "$(dirname "$0")"

# Ensure Kotlin client is built
echo "Building Kotlin client..."
cd simulator/clients/kotlin-cli
nix develop --command just build
cd ../../..

# Run the test with Java runtime available
echo "Running deterministic messaging test..."
cd simulator/clients/kotlin-cli
nix develop --command bash -c "
  cd ../../..
  simulator/standalone-test-runner/target/debug/bitchat-test-runner --client-type kotlin scenario deterministic-messaging
"