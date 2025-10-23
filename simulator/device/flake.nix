{
  description = "BitChat Emulator Harness - Real App Testing Framework with System Tools Integration";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { 
          inherit system; 
          # Allow unfree packages for Android tools
          config.allowUnfree = true;
        };
      in
      {
        devShells = {
          # Default development shell with Nix tools + system tool detection
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              # Rust toolchain from Nix (for consistency)
              rustc
              cargo
              clippy
              rustfmt
              
              # Development tools
              just
              
              # C/C++ compilation toolchain for general use
              # Note: iOS compilation uses system clang to avoid wrapper conflicts
              llvm
              gcc
              cmake
              autoconf
              automake
              libtool
              
              # Java runtime for Android development
              jdk17
              
              # Android development tools (from Nix)
              android-tools  # Provides adb, fastboot, etc.
              
              # System libraries needed for Rust and C compilation
              openssl
              pkg-config
              zlib
              
              # Build tools for native dependencies
              gnumake
              
              # Utilities
              which
              curl
              unzip
              git
            ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              # macOS specific tools for iOS development
              libiconv
              darwin.cctools
            ];

            shellHook = ''
              echo "BitChat Emulator Harness - Hybrid Development Environment"
              echo "========================================================"
              echo ""
              
              # Set up tool detection and integration
              
              # iOS Development Tools Detection
              if [ -d "/Applications/Xcode.app" ]; then
                export DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer"
                export PATH="$DEVELOPER_DIR/usr/bin:$PATH"
                
                # Set up iOS SDK paths for C compilation
                export SDKROOT="$(xcrun --sdk iphonesimulator --show-sdk-path 2>/dev/null || echo "")"
                export IPHONEOS_DEPLOYMENT_TARGET="16.0"
                
                # Use system clang for iOS compilation (avoid Nix wrapper conflicts)
                export CC="$(xcrun --find clang)"
                export CXX="$(xcrun --find clang++)"
                export CPP="$(xcrun --find cpp)"
                export LD="$(xcrun --find ld)"
                
                # Configure for iOS compilation without interfering with Xcode's linking
                if [ -n "$SDKROOT" ]; then
                  # Only set minimal flags that don't interfere with Xcode's build system
                  export CFLAGS="-isysroot $SDKROOT"
                  export CPPFLAGS="-isysroot $SDKROOT"
                  # Don't set LDFLAGS to avoid interfering with Xcode's linker arguments
                  unset LDFLAGS
                fi
                
                if command -v xcrun >/dev/null 2>&1 && xcrun simctl help >/dev/null 2>&1; then
                  echo "[OK] iOS Development: Xcode tools available"
                  echo "   - xcrun/simctl: $(xcrun --find simctl)"
                  echo "   - DEVELOPER_DIR: $DEVELOPER_DIR"
                  echo "   - SDKROOT: $SDKROOT"
                  echo "   - iOS SDK configured for C compilation"
                else
                  echo "[WARN]  iOS Development: Xcode found but tools not working"
                fi
              else
                echo "[ERROR] iOS Development: Xcode not found"
                echo "   Install Xcode from the Mac App Store for iOS testing"
              fi
              
              # Android Development Tools Detection
              echo ""
              if [ -n "''${ANDROID_HOME:-}" ] && [ -d "$ANDROID_HOME" ]; then
                export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$ANDROID_HOME/tools/bin:$PATH"
                echo "[OK] Android Development: SDK found at $ANDROID_HOME"
                
                if command -v adb >/dev/null 2>&1; then
                  echo "   - adb: $(which adb) ($(adb version 2>/dev/null | head -1 | cut -d' ' -f5))"
                else
                  echo "   - adb: not found in SDK"
                fi
                
                if command -v emulator >/dev/null 2>&1; then
                  echo "   - emulator: $(which emulator)"
                else
                  echo "   - emulator: not found in SDK"
                fi
              else
                echo "[ERROR] Android Development: ANDROID_HOME not set or directory doesn't exist"
                echo "   Common locations to check:"
                echo "   - $HOME/Library/Android/sdk (macOS)"
                echo "   - $HOME/Android/Sdk (Linux)"
                echo "   Set: export ANDROID_HOME=/path/to/android/sdk"
              fi
              
              # Java Detection
              echo ""
              echo "[Java] Java Development:"
              echo "   - java: $(which java) ($(java --version 2>/dev/null | head -1 | cut -d' ' -f2 || java -version 2>&1 | head -1 | cut -d'"' -f2))"
              if [ -n "''${JAVA_HOME:-}" ]; then
                echo "   - JAVA_HOME: $JAVA_HOME"
              fi
              
              # Rust Tools
              echo ""
              echo "[Rust] Rust Development:"
              echo "   - cargo: $(which cargo) ($(cargo --version | cut -d' ' -f2))"
              echo "   - rustc: $(which rustc) ($(rustc --version | cut -d' ' -f2))"
              
              echo ""
              echo "[Commands] Available Commands:"
              echo "   cargo run -- --help                                    # Show emulator harness help"
              echo "   cargo run -- test --client1 ios --client2 ios          # iOS ↔ iOS testing"
              echo "   cargo run -- test --client1 android --client2 android  # Android ↔ Android testing"
              echo "   cargo run -- test --client1 ios --client2 android      # Cross-platform testing"
              echo "   just show-native-usage                                  # Show system tools usage"
              echo ""
              
              # Environment validation
              MISSING_TOOLS=()
              if ! command -v xcrun >/dev/null 2>&1 || ! xcrun simctl help >/dev/null 2>&1; then
                MISSING_TOOLS+=("iOS")
              fi
              if [ -z "''${ANDROID_HOME:-}" ] || ! command -v adb >/dev/null 2>&1; then
                MISSING_TOOLS+=("Android")
              fi
              
              if [ ''${#MISSING_TOOLS[@]} -eq 0 ]; then
                echo "[OK] Environment Status: Ready for full mobile testing!"
              else
                echo "[WARN]  Environment Status: Some tools missing (''${MISSING_TOOLS[*]})"
                echo "   You can still use mock simulation and available platform testing"
              fi
              
              echo ""
            '';
          };
          
          # Pure Nix environment (no system tool integration)
          pure = pkgs.mkShell {
            buildInputs = with pkgs; [
              rustc cargo just jdk17 android-tools
              openssl pkg-config which curl unzip git
            ];
            
            shellHook = ''
              echo "BitChat Emulator Harness - Pure Nix Environment"
              echo "==============================================="
              echo ""
              echo "This environment only includes Nix-provided tools."
              echo "For real mobile device testing, use: nix develop"
              echo ""
            '';
          };
          
          # System tools environment (bypasses Nix for mobile tools)
          system = pkgs.mkShell {
            buildInputs = with pkgs; [
              rustc cargo just openssl pkg-config git
            ];
            
            shellHook = ''
              echo "BitChat Emulator Harness - System Tools Environment"
              echo "=================================================="
              echo ""
              
              # Prioritize system tools
              export PATH="/usr/bin:/usr/local/bin:$PATH"
              
              if [ -d "/Applications/Xcode.app" ]; then
                export DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer"
                export PATH="$DEVELOPER_DIR/usr/bin:$PATH"
              fi
              
              if [ -n "''${ANDROID_HOME:-}" ]; then
                export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH"
              fi
              
              echo "Environment configured to prioritize system tools."
              echo "Use this for maximum compatibility with system installations."
              echo ""
            '';
          };
        };
        
        # Provide the tools as packages for other flakes to use
        packages = {
          emulator-harness = pkgs.rustPlatform.buildRustPackage {
            pname = "bitchat-emulator-harness";
            version = "0.1.0";
            src = ./.;
            cargoHash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="; # Will need to be updated
            buildInputs = with pkgs; [ openssl pkg-config ];
            nativeBuildInputs = with pkgs; [ pkg-config ];
          };
        };
      });
}