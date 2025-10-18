{
  description = "BitChat Kotlin CLI Development Environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Platform-specific dependencies
        darwinDeps = pkgs.lib.optionals pkgs.stdenv.isDarwin (with pkgs; [
          libiconv
        ]);

        linuxDeps = pkgs.lib.optionals pkgs.stdenv.isLinux (with pkgs; [
          # Additional Linux deps if needed
        ]);

      in
      {
        # Development shell for Kotlin
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Java and Kotlin toolchain
            jdk17
            gradle
            
            # Build tools
            just
            
            # Development tools
            git
            
            # System libraries
            openssl
            sqlite
          ] ++ darwinDeps ++ linuxDeps;

          # Set JAVA_HOME for Gradle
          JAVA_HOME = if pkgs.stdenv.isDarwin 
            then "${pkgs.jdk17}/Library/Java/JavaVirtualMachines/zulu-17.jdk/Contents/Home"
            else "${pkgs.jdk17}/lib/openjdk";

          shellHook = ''
            echo "BitChat Kotlin Development Environment"
            echo "======================================"
            echo ""
            echo "Available tools:"
            echo "  - java $(java --version | head -1 | cut -d' ' -f2)"
            echo "  - gradle $(gradle --version | grep Gradle | cut -d' ' -f2)"
            echo "  - just (task runner)"
            echo ""
            echo "Environment variables:"
            echo "  JAVA_HOME=$JAVA_HOME"
            echo ""
            echo "Commands:"
            echo "  just build        - Build Kotlin CLI"
            echo "  just run          - Run Kotlin CLI"  
            echo "  just clean        - Clean build artifacts"
            echo ""
            echo "Environment ready for Kotlin development."
            echo ""
          '';
        };

        # Kotlin CLI package
        packages.default = pkgs.stdenv.mkDerivation {
          pname = "bitchat-kotlin-cli";
          version = "0.1.0";
          
          src = ./.;
          
          nativeBuildInputs = with pkgs; [ jdk17 gradle ];
          
          buildPhase = ''
            export JAVA_HOME=${if pkgs.stdenv.isDarwin 
              then "${pkgs.jdk17}/Library/Java/JavaVirtualMachines/zulu-17.jdk/Contents/Home"
              else "${pkgs.jdk17}/lib/openjdk"}
            gradle build installDist
          '';
          
          installPhase = ''
            mkdir -p $out/bin
            cp build/install/bitchat-kotlin-cli/bin/bitchat-kotlin-cli $out/bin/
            cp -r build/install/bitchat-kotlin-cli/lib $out/
          '';
          
          meta = with pkgs.lib; {
            description = "BitChat Kotlin CLI";
            license = with licenses; [ mit asl20 ];
          };
        };
      });
}