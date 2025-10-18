#!/usr/bin/env swift

import ArgumentParser
import Foundation
import Logging

// MARK: - BitChat Swift CLI
@main
struct BitChatSwiftCLI: AsyncParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "bitchat-swift-cli",
        abstract: "BitChat Swift CLI - A command-line interface for BitChat messaging",
        version: "0.1.0"
    )
    
    @Option(name: .shortAndLong, help: "Nostr relay URL")
    var relay: String = "ws://localhost:8080"
    
    @Option(name: .shortAndLong, help: "Client name/identifier")
    var name: String = "swift-client"
    
    @Flag(name: .shortAndLong, help: "Enable verbose logging")
    var verbose: Bool = false
    
    func run() async throws {
        // Setup logging
        LoggingSystem.bootstrap { label in
            var handler = StreamLogHandler.standardOutput(label: label)
            handler.logLevel = verbose ? .debug : .info
            return handler
        }
        
        let logger = Logger(label: "bitchat-swift-cli")
        
        logger.info("Starting BitChat Swift CLI")
        logger.info("Client name: \(name)")
        logger.info("Relay: \(relay)")
        
        // Initialize BitChat client
        let client = BitChatClient(
            name: name,
            relayURL: relay,
            logger: logger
        )
        
        // Start the client
        try await client.start()
        
        // Handle user input
        await handleUserInput(client: client, logger: logger)
    }
}

// MARK: - User Input Handling
func handleUserInput(client: BitChatClient, logger: Logger) async {
    print("\nBitChat Swift CLI ready. Type 'help' for commands or 'quit' to exit.")
    print("Commands:")
    print("  send <recipient> <message>  - Send a message")
    print("  connect <peer>              - Connect to a peer") 
    print("  list                        - List connected peers")
    print("  help                        - Show this help")
    print("  quit                        - Exit the application")
    print("")
    
    while true {
        print("> ", terminator: "")
        
        guard let input = readLine()?.trimmingCharacters(in: .whitespacesAndNewlines),
              !input.isEmpty else {
            continue
        }
        
        let components = input.split(separator: " ", maxSplits: 2).map(String.init)
        let command = components[0].lowercased()
        
        switch command {
        case "help":
            print("Available commands:")
            print("  send <recipient> <message>  - Send a message")
            print("  connect <peer>              - Connect to a peer")
            print("  list                        - List connected peers")
            print("  help                        - Show this help")
            print("  quit", "exit                - Exit the application")
            
        case "quit", "exit":
            logger.info("Shutting down...")
            await client.shutdown()
            return
            
        case "send":
            if components.count >= 3 {
                let recipient = components[1]
                let message = components[2]
                await client.sendMessage(to: recipient, message: message)
            } else {
                print("Usage: send <recipient> <message>")
            }
            
        case "connect":
            if components.count >= 2 {
                let peer = components[1]
                await client.connectToPeer(peer)
            } else {
                print("Usage: connect <peer>")
            }
            
        case "list":
            let peers = await client.getConnectedPeers()
            if peers.isEmpty {
                print("No connected peers")
            } else {
                print("Connected peers:")
                for peer in peers {
                    print("  - \(peer)")
                }
            }
            
        default:
            print("Unknown command: \(command). Type 'help' for available commands.")
        }
    }
}

// MARK: - BitChat Client Implementation
class BitChatClient {
    private let name: String
    private let relayURL: String
    private let logger: Logger
    private var connectedPeers: Set<String> = []
    private var isRunning = false
    
    init(name: String, relayURL: String, logger: Logger) {
        self.name = name
        self.relayURL = relayURL
        self.logger = logger
    }
    
    func start() async throws {
        logger.info("Connecting to relay: \(relayURL)")
        isRunning = true
        
        // Simulate connection delay
        try await Task.sleep(nanoseconds: 1_000_000_000) // 1 second
        
        logger.info("Connected to relay")
        logger.info("BitChat Swift client '\(name)' is ready")
        
        // Start background message processing
        Task {
            await processIncomingMessages()
        }
    }
    
    func shutdown() async {
        logger.info("Disconnecting from relay...")
        isRunning = false
        
        // Simulate cleanup
        try? await Task.sleep(nanoseconds: 500_000_000) // 0.5 seconds
        
        logger.info("Disconnected")
    }
    
    func sendMessage(to recipient: String, message: String) async {
        logger.info("Sending message to \(recipient): \(message)")
        
        // Simulate message sending
        try? await Task.sleep(nanoseconds: 100_000_000) // 0.1 seconds
        
        print("Message sent to \(recipient): \(message)")
        logger.debug("Message delivered to \(recipient)")
    }
    
    func connectToPeer(_ peer: String) async {
        logger.info("Initiating connection to \(peer)")
        
        // Simulate handshake
        logger.debug("Handshake initiated with \(peer)")
        try? await Task.sleep(nanoseconds: 500_000_000) // 0.5 seconds
        
        // Simulate handshake completion
        logger.debug("Handshake complete with \(peer)")
        connectedPeers.insert(peer)
        
        print("Connected to \(peer)")
        logger.info("Successfully connected to \(peer)")
    }
    
    func getConnectedPeers() async -> [String] {
        return Array(connectedPeers).sorted()
    }
    
    private func processIncomingMessages() async {
        logger.debug("Started message processing loop")
        
        while isRunning {
            // Simulate periodic message checking
            try? await Task.sleep(nanoseconds: 2_000_000_000) // 2 seconds
            
            // Simulate occasional incoming messages for testing
            if Int.random(in: 1...10) == 1 && !connectedPeers.isEmpty {
                let randomPeer = connectedPeers.randomElement()!
                let testMessage = "Hello from \(randomPeer)!"
                print("Message from \(randomPeer): \(testMessage)")
                logger.debug("Received message from \(randomPeer)")
            }
        }
        
        logger.debug("Message processing loop stopped")
    }
}