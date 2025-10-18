package com.bitchat.cli

import com.github.ajalt.clikt.core.CliktCommand
import com.github.ajalt.clikt.parameters.options.default
import com.github.ajalt.clikt.parameters.options.flag
import com.github.ajalt.clikt.parameters.options.option
import kotlinx.coroutines.*
import mu.KotlinLogging
import kotlin.system.exitProcess

/**
 * BitChat Kotlin CLI - A command-line interface for BitChat messaging
 */
class BitChatKotlinCLI : CliktCommand(
    name = "bitchat-kotlin-cli",
    help = "BitChat Kotlin CLI - A command-line interface for BitChat messaging"
) {
    private val relay by option("-r", "--relay", help = "Nostr relay URL")
        .default("ws://localhost:8080")
    
    private val name by option("-n", "--name", help = "Client name/identifier")
        .default("kotlin-client")
    
    private val verbose by option("-v", "--verbose", help = "Enable verbose logging")
        .flag()

    override fun run() {
        // Setup logging
        val logger = KotlinLogging.logger {}
        
        // Set log level based on verbose flag
        if (verbose) {
            System.setProperty("org.slf4j.simpleLogger.defaultLogLevel", "debug")
        } else {
            System.setProperty("org.slf4j.simpleLogger.defaultLogLevel", "info")
        }
        
        runBlocking {
            logger.info { "Starting BitChat Kotlin CLI" }
            logger.info { "Client name: $name" }
            logger.info { "Relay: $relay" }
            
            // Initialize BitChat client
            val client = BitChatClient(name, relay, logger)
            
            try {
                // Start the client
                client.start()
                
                // Handle user input
                handleUserInput(client, logger)
            } catch (e: Exception) {
                logger.error(e) { "Error in BitChat client" }
                exitProcess(1)
            } finally {
                client.shutdown()
            }
        }
    }
}

/**
 * Handle user input and command processing
 */
suspend fun handleUserInput(client: BitChatClient, logger: mu.KLogger) {
    println("\nBitChat Kotlin CLI ready. Type 'help' for commands or 'quit' to exit.")
    println("Commands:")
    println("  send <recipient> <message>  - Send a message")
    println("  connect <peer>              - Connect to a peer")
    println("  list                        - List connected peers")
    println("  help                        - Show this help")
    println("  quit                        - Exit the application")
    println()
    
    while (true) {
        print("> ")
        val input = readlnOrNull()?.trim() ?: break
        
        if (input.isEmpty()) continue
        
        val components = input.split(" ", limit = 3)
        val command = components[0].lowercase()
        
        when (command) {
            "help" -> {
                println("Available commands:")
                println("  send <recipient> <message>  - Send a message")
                println("  connect <peer>              - Connect to a peer")
                println("  list                        - List connected peers")
                println("  help                        - Show this help")
                println("  quit, exit                  - Exit the application")
            }
            
            "quit", "exit" -> {
                logger.info { "Shutting down..." }
                break
            }
            
            "send" -> {
                if (components.size >= 3) {
                    val recipient = components[1]
                    val message = components[2]
                    client.sendMessage(recipient, message)
                } else {
                    println("Usage: send <recipient> <message>")
                }
            }
            
            "connect" -> {
                if (components.size >= 2) {
                    val peer = components[1]
                    client.connectToPeer(peer)
                } else {
                    println("Usage: connect <peer>")
                }
            }
            
            "list" -> {
                val peers = client.getConnectedPeers()
                if (peers.isEmpty()) {
                    println("No connected peers")
                } else {
                    println("Connected peers:")
                    peers.forEach { peer ->
                        println("  - $peer")
                    }
                }
            }
            
            else -> {
                println("Unknown command: $command. Type 'help' for available commands.")
            }
        }
    }
}

/**
 * BitChat Client Implementation
 */
class BitChatClient(
    private val name: String,
    private val relayURL: String,
    private val logger: mu.KLogger
) {
    private val connectedPeers = mutableSetOf<String>()
    private var isRunning = false
    private var messageProcessingJob: Job? = null
    
    suspend fun start() {
        logger.info { "Connecting to relay: $relayURL" }
        isRunning = true
        
        // Simulate connection delay
        delay(1000)
        
        logger.info { "Connected to relay" }
        logger.info { "BitChat Kotlin client '$name' is ready" }
        
        // Start background message processing
        messageProcessingJob = CoroutineScope(Dispatchers.IO).launch {
            processIncomingMessages()
        }
    }
    
    suspend fun shutdown() {
        logger.info { "Disconnecting from relay..." }
        isRunning = false
        
        messageProcessingJob?.cancel()
        messageProcessingJob?.join()
        
        // Simulate cleanup
        delay(500)
        
        logger.info { "Disconnected" }
    }
    
    suspend fun sendMessage(recipient: String, message: String) {
        logger.info { "Sending message to $recipient: $message" }
        
        // Simulate message sending
        delay(100)
        
        println("Message sent to $recipient: $message")
        logger.debug { "Message delivered to $recipient" }
    }
    
    suspend fun connectToPeer(peer: String) {
        logger.info { "Initiating connection to $peer" }
        
        // Simulate handshake
        logger.debug { "Handshake initiated with $peer" }
        delay(500)
        
        // Simulate handshake completion
        logger.debug { "Handshake complete with $peer" }
        connectedPeers.add(peer)
        
        println("Connected to $peer")
        logger.info { "Successfully connected to $peer" }
    }
    
    fun getConnectedPeers(): List<String> {
        return connectedPeers.sorted()
    }
    
    private suspend fun processIncomingMessages() {
        logger.debug { "Started message processing loop" }
        
        while (isRunning && !Thread.currentThread().isInterrupted) {
            try {
                // Simulate periodic message checking
                delay(2000)
                
                // Simulate occasional incoming messages for testing
                if ((1..10).random() == 1 && connectedPeers.isNotEmpty()) {
                    val randomPeer = connectedPeers.random()
                    val testMessage = "Hello from $randomPeer!"
                    println("Message from $randomPeer: $testMessage")
                    logger.debug { "Received message from $randomPeer" }
                }
            } catch (e: InterruptedException) {
                break
            } catch (e: Exception) {
                logger.error(e) { "Error in message processing" }
            }
        }
        
        logger.debug { "Message processing loop stopped" }
    }
}

/**
 * Main entry point
 */
fun main(args: Array<String>) {
    BitChatKotlinCLI().main(args)
}