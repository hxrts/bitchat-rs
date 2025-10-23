{
  description = "BitChat Test Runner - Standalone Event Orchestrator";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain with WASM target
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "clippy" "rustfmt" ];
          targets = [ "wasm32-unknown-unknown" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust toolchain with WASM support
            rustToolchain
            
            # WASM linker and tools
            lld
            wasm-pack
            
            # Development tools
            just
            
            # Java runtime for Kotlin clients
            jdk17
            
            # System libraries
            openssl
            pkg-config
          ];

          shellHook = ''
            echo "BitChat Test Runner Development Environment"
            echo "==========================================="
            echo ""
            echo "Available tools:"
            echo "  - rust $(rustc --version | cut -d' ' -f2) (with WASM target)"
            echo "  - cargo (Rust build system)"
            echo "  - just (task runner)"
            echo "  - lld (WASM linker)"
            echo "  - wasm-pack (WASM package tool)"
            echo "  - java $(java --version | head -1 | cut -d' ' -f2) (for Kotlin clients)"
            echo ""
            echo "Commands:"
            echo "  cargo check        - Check compilation"
            echo "  cargo build        - Build test runner"
            echo "  cargo run -- list  - List available scenarios"
            echo "  cargo run -- --client-type web scenario deterministic-messaging"
            echo "  cargo run -- --client-type kotlin scenario deterministic-messaging"
            echo ""
            echo "Environment ready for cross-client testing."
            echo ""
          '';
        };
      });
}