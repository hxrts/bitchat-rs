{
  description = "BitChat Test Runner - Standalone Event Orchestrator";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust toolchain
            rustc
            cargo
            
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
            echo "  - cargo (Rust build system)"
            echo "  - just (task runner)"
            echo "  - java $(java --version | head -1 | cut -d' ' -f2) (for Kotlin clients)"
            echo ""
            echo "Commands:"
            echo "  cargo check        - Check compilation"
            echo "  cargo build        - Build test runner"
            echo "  cargo run -- list  - List available scenarios"
            echo "  cargo run -- --client-type kotlin scenario deterministic-messaging"
            echo ""
            echo "Environment ready for cross-client testing."
            echo ""
          '';
        };
      });
}