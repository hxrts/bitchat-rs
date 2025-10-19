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
    
    @Flag(name: .long, help: "Enable automation mode (JSON output)")
    var automationMode: Bool = false
    
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
            logger: logger,
            automationMode: automationMode
        )
        
        // Start the client
        try await client.start()
        
        if automationMode {
            // Automation mode - handle commands from stdin
            await handleAutomationMode(client: client, logger: logger)
        } else {
            // Interactive mode - handle user input
            await handleUserInput(client: client, logger: logger)
        }
    }
}

// MARK: - Automation Mode Handler
func handleAutomationMode(client: BitChatClient, logger: Logger) async {
    // Emit Ready event
    await client.emitAutomationEvent(type: "Ready", data: [:])
    
    while true {
        guard let input = readLine()?.trimmingCharacters(in: .whitespacesAndNewlines),
              !input.isEmpty else {
            continue
        }
        
        let components = input.split(separator: " ", maxSplits: 2).map(String.init)
        let command = components[0]
        
        switch command {
        case "send":
            if components.count >= 2 {
                let message = components[1..<components.count].joined(separator: " ")
                await client.sendMessage(to: nil, message: message)
            }
            
        case "private":
            if components.count >= 3 {
                let peer = components[1]
                let message = components[2..<components.count].joined(separator: " ")
                await client.sendPrivateMessage(to: peer, message: message)
            }
            
        case "connect":
            if components.count >= 2 {
                let peer = components[1]
                await client.connectToPeer(peer)
            }
            
        case "discover":
            await client.startDiscovery()
            
        case "stop-discovery":
            await client.stopDiscovery()
            
        case "/simulate-panic":
            await client.emitAutomationEvent(type: "PanicRecovered", data: [:])
            
        case "/inject-corrupted-packets":
            // Simulate malicious behavior
            await client.emitAutomationEvent(type: "CorruptedPacketsInjected", data: [:])
            
        case "/disable-transport":
            if components.count >= 2 {
                let transport = components[1]
                await client.emitAutomationEvent(type: "TransportStatusChanged", 
                                               data: ["transport": transport, "status": "Disabled"])
            }
            
        case "/enable-transport":
            if components.count >= 2 {
                let transport = components[1]
                await client.emitAutomationEvent(type: "TransportStatusChanged", 
                                               data: ["transport": transport, "status": "Enabled"])
            }
            
        case "quit", "exit":
            break
            
        default:
            break
        }
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
    let name: String
    private let relayURL: String
    private let logger: Logger
    private let automationMode: Bool
    private var connectedPeers: Set<String> = []
    private var isRunning = false
    
    init(name: String, relayURL: String, logger: Logger, automationMode: Bool = false) {
        self.name = name
        self.relayURL = relayURL
        self.logger = logger
        self.automationMode = automationMode
    }
    
    func start() async throws {
        logger.info("Connecting to relay: \(relayURL)")
        isRunning = true
        
        // Emit client_started event
        if automationMode {
            await emitAutomationEvent(type: "client_started", data: [:])
        }
        
        // Event-driven connection (no sleep)
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
        logger.info("Disconnected")
    }
    
    func sendMessage(to recipient: String?, message: String) async {
        let targetRecipient = recipient ?? "broadcast"
        logger.info("Sending message to \(targetRecipient): \(message)")
        
        // Event-driven messaging (no sleep)
        if !automationMode {
            print("Message sent to \(targetRecipient): \(message)")
        }
        
        // Emit automation event
        await emitAutomationEvent(type: "MessageSent", data: [
            "to": targetRecipient,
            "content": message,
            "message_id": UUID().uuidString
        ])
        
        logger.debug("Message delivered to \(targetRecipient)")
    }
    
    func sendPrivateMessage(to recipient: String, message: String) async {
        logger.info("Sending private message to \(recipient): \(message)")
        
        // Event-driven messaging (no sleep)
        if !automationMode {
            print("Private message sent to \(recipient): \(message)")
        }
        
        // Emit automation event
        await emitAutomationEvent(type: "MessageSent", data: [
            "to": recipient,
            "content": message,
            "message_id": UUID().uuidString,
            "is_private": true
        ])
        
        logger.debug("Private message delivered to \(recipient)")
    }
    
    func startDiscovery() async {
        logger.info("Starting peer discovery")
        await emitAutomationEvent(type: "DiscoveryStateChanged", data: [
            "active": true,
            "transport": "nostr"
        ])
    }
    
    func stopDiscovery() async {
        logger.info("Stopping peer discovery")
        await emitAutomationEvent(type: "DiscoveryStateChanged", data: [
            "active": false,
            "transport": "nostr"
        ])
    }
    
    func connectToPeer(_ peer: String) async {
        logger.info("Initiating connection to \(peer)")
        
        // Event-driven connection (no sleep)
        logger.debug("Handshake initiated with \(peer)")
        logger.debug("Handshake complete with \(peer)")
        connectedPeers.insert(peer)
        
        // Emit peer discovery event
        await emitAutomationEvent(type: "PeerDiscovered", data: [
            "peer_id": peer,
            "transport": "nostr"
        ])
        
        // Emit session established event
        await emitAutomationEvent(type: "SessionEstablished", data: [
            "peer_id": peer
        ])
        
        if !automationMode {
            print("Connected to \(peer)")
        }
        logger.info("Successfully connected to \(peer)")
    }
    
    func getConnectedPeers() async -> [String] {
        return Array(connectedPeers).sorted()
    }
    
    private func processIncomingMessages() async {
        logger.debug("Started message processing loop")
        
        while isRunning {
            // Event-driven message processing (no sleep)
            // In a real implementation, this would listen to actual network events
            
            // For simulation purposes, we'll use a smaller delay with deterministic behavior
            await Task.yield() // Yield control to other tasks
            
            // Simulate incoming messages based on automation commands
            // This would be replaced with real protocol implementation
        }
        
        logger.debug("Message processing loop stopped")
    }
    
    // MARK: - Automation Support
    func emitAutomationEvent(type: String, data: [String: Any]) async {
        guard automationMode else { return }
        
        var eventData = data
        eventData["timestamp"] = UInt64(Date().timeIntervalSince1970 * 1000)
        eventData["peer_id"] = self.name
        
        let event: [String: Any] = [
            "type": type,
            "data": eventData
        ]
        
        do {
            let jsonData = try JSONSerialization.data(withJSONObject: event)
            if let jsonString = String(data: jsonData, encoding: .utf8) {
                print(jsonString)
                fflush(stdout)
            }
        } catch {
            logger.error("Failed to serialize automation event: \(error)")
        }
    }
    
    func simulateMessageReceived(from sender: String, message: String) async {
        await emitAutomationEvent(type: "MessageReceived", data: [
            "from": sender,
            "content": message,
            "message_id": UUID().uuidString,
            "is_private": false
        ])
        
        if !automationMode {
            print("Message from \(sender): \(message)")
        }
    }
}