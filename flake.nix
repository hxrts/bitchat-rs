{
  description = "BitChat - Peer-to-peer encrypted messaging protocol";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain with required components
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "clippy" "rustfmt" ];
          targets = [ "wasm32-unknown-unknown" ];
        };

        # Platform-specific dependencies
        darwinDeps = pkgs.lib.optionals pkgs.stdenv.isDarwin (with pkgs; [
          # Use libiconv for macOS compatibility
          libiconv
        ]);

        linuxDeps = pkgs.lib.optionals pkgs.stdenv.isLinux (with pkgs; [
          dbus
          bluez
          udev
          systemd
        ]);

      in
      {
        # Development shell
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust toolchain
            rustToolchain
            
            # Build tools
            pkg-config
            just
            
            # Development tools
            git
            
            # System libraries  
            openssl
            sqlite
          ] ++ darwinDeps ++ linuxDeps;

          # Environment variables
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          RUST_BACKTRACE = "1";

          shellHook = ''
            echo "BitChat Development Environment"
            echo "=================================="
            echo ""
            echo "Available tools:"
            echo "  • rust $(rustc --version | cut -d' ' -f2)"
            echo "  • cargo (with clippy, rustfmt)"
            echo "  • just (task runner)"
            echo ""
            echo "Getting started:"
            echo "  just --list          # Show available tasks"
            echo "  just build           # Build the project"
            echo "  just test            # Run tests"
            echo "  just demo            # Run BitChat demo"
            echo ""
            echo "For Nostr relay testing:"
            echo "  Use external relay: wss://relay.damus.io"
            echo "  Or install nostr-rs-relay manually"
            echo ""
          '';
        };

        # Note: Package build disabled until Cargo.lock is available
        # To enable, run `cargo generate-lockfile` in the project root
        packages = {
          # Placeholder package
          default = pkgs.writeShellScriptBin "bitchat-placeholder" ''
            echo "BitChat package not built yet. Run 'nix develop' and 'just build' to build the project."
            exit 1
          '';
        };
      });
}