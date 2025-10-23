# Xcode 26 (Xcode 17 Beta) Tor Linker Fix

## Problem

Xcode 26.0.1 (Xcode 17 beta, Build version 17A400) has a regression bug in its linker that affects Swift Package Manager C targets that depend on binary frameworks. When building the Tor package, the linker fails with:

```
ld: unknown options: -Xlinker -isysroot -iframework -nostdlib -Xlinker -rdynamic -Xlinker -Xlinker
```

This occurs during **partial linking** (`ld -r`) of the `TorC` target, which has a dependency on the `tor-nolzma.xcframework` binary target.

## Root Cause

When SPM builds a C target with dependencies, Xcode performs partial linking to create an intermediate `.o` file. The linker flags passed during this phase include options that are:

1. **Invalid for partial linking**: `-isysroot`, `-iframework`, `-nostdlib`, `-rdynamic`
2. **Malformed**: Duplicate `-Xlinker` prefixes

This appears to be a bug introduced in Xcode 26 beta, as these flags should not be passed during partial linking.

## Solution

We've implemented a **linker wrapper** that intercepts linker calls and filters out problematic flags during partial linking, while passing through all flags for normal linking.

### How It Works

1. **Wrapper Script** (`scripts/ld-wrapper.sh`):
   - Detects if the linker is being called for partial linking (`-r` flag)
   - If yes: filters out `-Xlinker -isysroot`, `-Xlinker -iframework`, `-Xlinker -nostdlib`, `-Xlinker -rdynamic`
   - If no: passes all flags through unchanged
   - Calls the original Xcode linker with the filtered arguments

2. **Installation** (`scripts/install-ld-wrapper.sh`):
   - Backs up the original Xcode linker to `ld.orig`
   - Replaces Xcode's linker with our wrapper script
   - Requires `sudo` because the linker is in `/Applications/Xcode.app`

3. **Restoration** (`scripts/restore-ld.sh`):
   - Restores the original linker from backup
   - Removes the backup file
   - Requires `sudo`

## Usage

### First-Time Setup

1. Install the linker wrapper (one-time setup):
   ```bash
   cd simulator/emulator-rig
   just install-ld-wrapper
   ```
   
   This will prompt for your password (sudo required).

2. Build the iOS app:
   ```bash
   just build-ios
   ```

### After Building

You can optionally restore the original linker:
```bash
cd simulator/emulator-rig
just restore-ld
```

**Note**: The wrapper doesn't affect normal builds - it only filters flags during partial linking. You can leave it installed permanently if you're actively developing.

### Checking Current State

To see if the wrapper is installed:
```bash
ls -la /Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/ld*
```

- If you see `ld.orig`, the wrapper is installed
- If you don't see `ld.orig`, the original linker is active

## Why This Approach?

We considered several alternatives:

1. **NO Remove Tor dependency**: Would break functionality, not acceptable
2. **NO Modify Tor Package.swift**: Would diverge from upstream bitchat-ios repo
3. **NO PATH-based wrapper**: Xcode calls linker with absolute path, ignores PATH
4. **NO Mixed-language target**: SPM doesn't support Swift + C in one target
5. **NO System library module**: Still triggers partial linking for C code
6. **YES Linker replacement**: Clean, doesn't modify source code, easily reversible

## When Can We Remove This?

This workaround can be removed when:

1. **Xcode 17 ships**: The final release may fix this regression
2. **Tor updates their package**: They could restructure to avoid the issue
3. **You downgrade**: Xcode 15.x doesn't have this bug

## Files Modified

- `simulator/emulator-rig/Justfile`: Added `install-ld-wrapper` and `restore-ld` commands
- `simulator/emulator-rig/scripts/ld-wrapper.sh`: Linker wrapper script
- `simulator/emulator-rig/scripts/install-ld-wrapper.sh`: Installation script
- `simulator/emulator-rig/scripts/restore-ld.sh`: Restoration script

## Files NOT Modified

- YES `simulator/emulator-rig/vendored/bitchat-ios/**`: iOS code unchanged, tracks upstream
- YES `simulator/emulator-rig/vendored/bitchat-ios/localPackages/Tor/**`: Tor package unchanged

## Troubleshooting

### Build still fails after installing wrapper

1. Check if wrapper is actually installed:
   ```bash
   file /Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/ld
   ```
   Should show: `POSIX shell script executable`

2. Check if backup exists:
   ```bash
   file /Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/ld.orig
   ```
   Should show: `Mach-O 64-bit executable arm64`

3. Try restoring and reinstalling:
   ```bash
   just restore-ld
   just install-ld-wrapper
   ```

### Permission denied when installing

Make sure you have admin access and can use `sudo`. The script needs to modify files in `/Applications/Xcode.app`.

### Xcode complains about modified installation

If Xcode complains, you can always restore the original:
```bash
just restore-ld
```

Xcode will work normally without the wrapper; you just won't be able to build the Tor-dependent iOS app.

## References

- Xcode Version: 26.0.1 (Build 17A400)
- Issue: Partial linking regression with C targets in SPM
- Related: https://github.com/apple/swift-package-manager/issues (similar issues reported for Xcode betas)

