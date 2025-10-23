# HOW TO REVERT THIS WORKAROUND

## When Xcode is Fixed

This is a **TEMPORARY** workaround for the Xcode 16+ linker bug that breaks C library partial linking.

Once Apple fixes the bug in a future Xcode release, follow these steps to revert:

## Step 1: Remove the Rust Crypto Crate

```bash
cd /Users/hxrts/projects/bitchat
rm -rf simulator/ios-linker-workaround
```

## Step 2: Remove from Workspace

Edit `/Users/hxrts/projects/bitchat/Cargo.toml`:
- Remove `simulator/ios-linker-workaround` from the workspace members list

## Step 3: Restore swift-secp256k1 in iOS Package

Edit `/Users/hxrts/projects/bitchat/simulator/device/vendored/bitchat-ios/Package.swift`:

```swift
dependencies:[
    // Uncomment this line:
    .package(url: "https://github.com/21-DOT-DEV/swift-secp256k1", exact: "0.21.1"),
    // Comment out or remove the binaryTarget for BitchatCrypto
    .package(path: "localPackages/BitLogger"),
],
targets: [
    .executableTarget(
        name: "bitchat",
        dependencies: [
            // Uncomment this:
            .product(name: "P256K", package: "swift-secp256k1"),
            .product(name: "BitLogger", package: "BitLogger")
        ],
```

## Step 4: Clean and Rebuild

```bash
cd /Users/hxrts/projects/bitchat/simulator/device/vendored/bitchat-ios
rm -rf build Frameworks/BitchatCrypto.xcframework
cd ../../..
just build-ios
```

## Step 5: Re-enable Tor (if needed)

If you also disabled Tor due to the same bug, uncomment it in `Package.swift`:
```swift
.package(path: "localPackages/Tor"),
// and
.product(name: "Tor", package: "Tor")
```

And restore Swift imports:
```swift
import Tor  // Uncomment in affected files
```

## Files to Delete
- `/Users/hxrts/projects/bitchat/simulator/ios-linker-workaround/` (entire directory)
- `/Users/hxrts/projects/bitchat/simulator/device/vendored/bitchat-ios/Frameworks/BitchatCrypto.xcframework/`

## Verification

After reverting, verify the build works:
```bash
cd /Users/hxrts/projects/bitchat/simulator
just build-ios
```

You should see no linker errors about "unknown options".

---

**Note**: If you see the linker bug again after reverting, the Xcode version still has the bug.  
Check https://developer.apple.com/xcode/resources/ for updates.

