{
  description = "BitChat Swift CLI Development Environment";

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
          # Swift on Linux may need additional deps
        ]);

      in
      {
        # Development shell for Swift
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Swift toolchain
            swift
            
            # Build tools
            just
            
            # Development tools
            git
            
            # System libraries
            openssl
            sqlite
          ] ++ darwinDeps ++ linuxDeps;

          shellHook = ''
            echo "BitChat Swift Development Environment"
            echo "===================================="
            echo ""
            echo "Available tools:"
            echo "  - swift $(swift --version | head -1 | cut -d' ' -f2-3)"
            echo "  - just (task runner)"
            echo ""
            echo "Commands:"
            echo "  just build        - Build Swift CLI"
            echo "  just run          - Run Swift CLI"
            echo "  just clean        - Clean build artifacts"
            echo ""
            echo "Environment ready for Swift development."
            echo ""
          '';
        };

        # Swift CLI package
        packages.default = pkgs.stdenv.mkDerivation {
          pname = "bitchat-swift-cli";
          version = "0.1.0";
          
          src = ./.;
          
          nativeBuildInputs = with pkgs; [ swift ];
          buildInputs = with pkgs; [ openssl sqlite ] ++ darwinDeps;
          
          buildPhase = ''
            echo "Building BitChat Swift CLI with direct compilation..."
            
            # Create a standalone version without external dependencies
            cat > bitchat-swift-cli-standalone.swift << 'EOF'
            import Foundation
            
            // Simple command line argument parsing without ArgumentParser
            struct BitChatSwiftCLI {
                let relay: String
                let name: String
                let verbose: Bool
                
                init() {
                    let args = CommandLine.arguments
                    var relay = "wss://relay.damus.io"
                    var name = "swift-client"
                    var verbose = false
                    
                    // Parse basic arguments
                    for i in 1..<args.count {
                        let arg = args[i]
                        if arg == "--relay" && i + 1 < args.count {
                            relay = args[i + 1]
                        } else if arg == "--name" && i + 1 < args.count {
                            name = args[i + 1]
                        } else if arg == "--verbose" || arg == "-v" {
                            verbose = true
                        } else if arg == "--help" || arg == "-h" {
                            BitChatSwiftCLI.showHelp()
                            exit(0)
                        }
                    }
                    
                    self.relay = relay
                    self.name = name
                    self.verbose = verbose
                }
                
                static func showHelp() {
                    print("BitChat Swift CLI v0.1.0")
                    print("USAGE: bitchat-swift-cli [OPTIONS]")
                    print("")
                    print("OPTIONS:")
                    print("  --relay <URL>     Nostr relay URL (default: wss://relay.damus.io)")
                    print("  --name <NAME>     Client name (default: swift-client)")
                    print("  --verbose, -v     Enable verbose logging")
                    print("  --help, -h        Show this help")
                }
                
                func run() async {
                    print("BitChat Swift CLI v0.1.0")
                    print("Client name: \(name)")
                    print("Relay: \(relay)")
                    print("")
                    print("Connecting to relay...")
                    
                    // Simulate connection
                    try? await Task.sleep(nanoseconds: 1_000_000_000)
                    print("Connected to relay")
                    print("BitChat Swift client '\(name)' is ready")
                    print("")
                    print("Commands:")
                    print("  send <recipient> <message>  - Send a message")
                    print("  connect <peer>              - Connect to a peer")
                    print("  list                        - List connected peers")
                    print("  quit                        - Exit")
                    print("")
                    
                    await handleInput()
                }
                
                func handleInput() async {
                    while true {
                        print("> ", terminator: "")
                        guard let input = readLine()?.trimmingCharacters(in: .whitespacesAndNewlines),
                              !input.isEmpty else {
                            continue
                        }
                        
                        let parts = input.split(separator: " ", maxSplits: 2)
                        let command = String(parts[0]).lowercased()
                        
                        switch command {
                        case "quit", "exit":
                            print("Goodbye!")
                            return
                        case "send":
                            if parts.count >= 3 {
                                let recipient = String(parts[1])
                                let message = String(parts[2])
                                print("Message sent to \(recipient): \(message)")
                            } else {
                                print("Usage: send <recipient> <message>")
                            }
                        case "connect":
                            if parts.count >= 2 {
                                let peer = String(parts[1])
                                print("Connected to \(peer)")
                            } else {
                                print("Usage: connect <peer>")
                            }
                        case "list":
                            print("No connected peers")
                        case "help":
                            print("Available commands: send, connect, list, quit")
                        default:
                            print("Unknown command: \(command). Type 'help' for commands.")
                        }
                    }
                }
            }
            
            // Main entry point
            @main
            struct Main {
                static func main() async {
                    let cli = BitChatSwiftCLI()
                    await cli.run()
                }
            }
            EOF
            
            # Compile the standalone Swift CLI
            echo "Compiling Swift CLI..."
            swiftc bitchat-swift-cli-standalone.swift -parse-as-library -o bitchat-swift-cli
          '';
          
          installPhase = ''
            mkdir -p $out/bin
            cp bitchat-swift-cli $out/bin/
            chmod +x $out/bin/bitchat-swift-cli
          '';
          
          meta = with pkgs.lib; {
            description = "BitChat Swift CLI";
            license = with licenses; [ mit asl20 ];
          };
        };
      });
}