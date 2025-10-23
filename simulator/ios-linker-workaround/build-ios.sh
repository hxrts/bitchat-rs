#!/usr/bin/env bash
# TEMPORARY: Xcode 16+ linker workaround build script
# Builds Rust secp256k1 as XCFramework for iOS
# DELETE THIS FILE when Xcode is fixed

set -euo pipefail

cd "$(dirname "$0")"

# Use system Rust, not Nix Rust
unset RUST_SRC_PATH
export PATH="/Users/hxrts/.cargo/bin:$PATH"

echo "Building Rust secp256k1 for iOS (Xcode 16 workaround)..."
echo "Using Rust from: $(which rustc)"

# Install iOS targets if not already present
rustup target add aarch64-apple-ios
rustup target add aarch64-apple-ios-sim
rustup target add x86_64-apple-ios

# Build for iOS device (arm64)
echo "Building for iOS device..."
cargo build --release --target aarch64-apple-ios

# Build for iOS simulator (arm64 + x86_64)
echo "Building for iOS simulator (arm64)..."
cargo build --release --target aarch64-apple-ios-sim

echo "Building for iOS simulator (x86_64)..."
cargo build --release --target x86_64-apple-ios

# Create output directory
OUTPUT_DIR="../device/vendored/bitchat-ios/Frameworks"
mkdir -p "$OUTPUT_DIR"

echo "Creating XCFramework..."

# Use workspace target directory  
WORKSPACE_TARGET="../../../target"

# Create fat library for simulator (combining arm64-sim and x86_64)
mkdir -p "$WORKSPACE_TARGET/universal-sim"
lipo -create \
    "$WORKSPACE_TARGET/aarch64-apple-ios-sim/release/libbitchat_ios_crypto.a" \
    "$WORKSPACE_TARGET/x86_64-apple-ios/release/libbitchat_ios_crypto.a" \
    -output "$WORKSPACE_TARGET/universal-sim/libbitchat_ios_crypto.a"

# Create header file
HEADER_FILE="$WORKSPACE_TARGET/bitchat_crypto.h"
cat > "$HEADER_FILE" << 'EOF'
#ifndef BITCHAT_CRYPTO_H
#define BITCHAT_CRYPTO_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
int secp256k1_privkey_generate(uint8_t *out);
int secp256k1_pubkey_from_privkey(const uint8_t *privkey, uint8_t *pubkey_out);
int secp256k1_xonly_pubkey_from_privkey(const uint8_t *privkey, uint8_t *xonly_out);
int secp256k1_ecdh(const uint8_t *privkey, const uint8_t *pubkey, size_t pubkey_len, uint8_t *secret_out);
int secp256k1_schnorr_sign(const uint8_t *privkey, const uint8_t *msg_hash, uint8_t *sig_out);
int secp256k1_schnorr_verify(const uint8_t *xonly_pubkey, const uint8_t *msg_hash, const uint8_t *signature);
#ifdef __cplusplus
}
#endif
#endif
EOF

# Create XCFramework with headers
xcodebuild -create-xcframework \
    -library "$WORKSPACE_TARGET/aarch64-apple-ios/release/libbitchat_ios_crypto.a" \
    -headers "$WORKSPACE_TARGET" \
    -library "$WORKSPACE_TARGET/universal-sim/libbitchat_ios_crypto.a" \
    -headers "$WORKSPACE_TARGET" \
    -output "$OUTPUT_DIR/BitchatCrypto.xcframework"

# Add module maps for Swift import
for PLATFORM_DIR in "$OUTPUT_DIR"/BitchatCrypto.xcframework/*/; do
    MODULE_MAP="$PLATFORM_DIR/Headers/module.modulemap"
    cat > "$MODULE_MAP" << 'MODMAP'
module BitchatCryptoFFI {
    header "bitchat_crypto.h"
    export *
}
MODMAP
done

echo "BitchatCrypto.xcframework created at $OUTPUT_DIR/"
echo ""
echo "REMINDER: This is a TEMPORARY workaround for Xcode 16 linker bug"
echo "    Remove this when Xcode is fixed!"

