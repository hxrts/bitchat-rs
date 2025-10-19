# BitChat Hybrid Architecture Proposal

> **Deprecated**: This proposal described the legacy single-task core architecture. The current design is captured in `runtime-architecture-rfc.md` and supersedes the plan below. Sections referencing a monolithic `CoreLogicTask` are retained only for historical context.

**Document Version**: 1.0  
**Date**: October 2024  
**Status**: Proposal for Review  

## Executive Summary

This document proposes a hybrid architectural approach for BitChat that addresses current concurrency issues through a clean-slate implementation using CSP (Communicating Sequential Processes) channels as the integration mechanism. Rather than applying a single architectural pattern uniformly, this proposal strategically combines different patterns based on the specific requirements and constraints of each subsystem, connected through explicit channel-based communication.

The primary motivation stems from deadlock issues in the current shared state architecture, where multiple asynchronous tasks compete for access to a single application state mutex. The proposed solution eliminates these coordination problems through task isolation and channel-based communication, prioritizing clarity and correctness over backwards compatibility.

## Current Problem Analysis

### Primary Concurrency Issue

The existing BitChat CLI implementation suffers from a fundamental coordination problem where multiple asynchronous tasks attempt to acquire exclusive access to the same application state. This manifests as indefinite blocking when the message processing loop holds the application lock while command processing and event handling tasks wait for access.

The root cause lies in the architectural decision to share mutable state across concurrent tasks using a single mutex. While this approach appears straightforward, it creates a coordination bottleneck that becomes increasingly problematic as the number of concurrent operations grows.

### Secondary Architectural Challenges

Beyond the immediate deadlock issues, several additional challenges complicate the current architecture:

**WebAssembly Compatibility**: The threading patterns used in the native implementation do not translate well to WASM's single-threaded execution model, requiring significant adaptation for web deployment.

**Testing Complexity**: Shared mutable state makes unit testing difficult because tests must account for all possible interleavings of concurrent operations, leading to non-deterministic test behavior.

**Mobile Integration Friction**: The async coordination patterns in the Rust implementation create complexity when integrating with the more straightforward reactive patterns used in the Swift and Android clients.

**Transport Protocol Duplication**: Each transport mechanism (BLE and Nostr) requires separate handling logic, despite sharing similar patterns for peer discovery, message routing, and error recovery.

## Architectural Philosophy

### Selective Pattern Application

Rather than adopting a single architectural paradigm, this proposal applies different patterns based on the specific characteristics and requirements of each subsystem. This approach recognizes that different parts of a system face different challenges and benefit from different solutions.

The selection criteria for architectural patterns include:

**Data Characteristics**: Whether data is naturally immutable, frequently updated, or requires complex validation determines the most appropriate state management approach.

**Concurrency Requirements**: Subsystems with high contention benefit from different patterns than those with infrequent updates or read-heavy access patterns.

**Integration Constraints**: Components that must integrate closely with platform-specific APIs or existing codebases may require more traditional patterns for compatibility.

**Performance Sensitivity**: Critical path operations may justify different tradeoffs than infrequent administrative tasks.

### Pattern Selection Rationale

**Content-Addressed Storage** for messages leverages the natural immutability of message content while providing integrity guarantees and automatic deduplication. Messages represent perfect candidates for this pattern because they should never change after creation and benefit significantly from hash-based identity.

**Linear State Machines** for connection lifecycle management eliminate invalid state transitions while providing clear audit trails. Connection states have well-defined lifecycles with clear transition rules, making them ideal candidates for linear type enforcement.

**Effect-Based Coordination** for transport operations separates planning from execution, enabling comprehensive testing and replay capabilities. Transport coordination involves complex decision-making that benefits from pure functional planning followed by isolated effect execution.

**Traditional Shared State** for UI coordination maintains compatibility with existing mobile patterns while providing the reactive behavior expected by modern user interfaces. UI state typically involves frequent updates and established patterns that work well across platforms.

**Event-Driven Processing** for peer discovery naturally matches the asynchronous and unpredictable nature of network discovery protocols while providing clean separation between different transport mechanisms.

## Detailed Architecture Design

### Content-Addressed Message Layer

Messages form the core data structure in BitChat and exhibit characteristics that make them ideal candidates for content-addressed storage. Each message becomes immutable upon creation, with its identity derived from the cryptographic hash of its canonical representation.

This approach provides several significant benefits. Automatic deduplication occurs when identical messages are received from multiple sources or transport mechanisms. Integrity verification becomes trivial since any modification to message content results in a different hash, making tampering immediately detectable. Concurrent access becomes safe by default since immutable data requires no coordination between readers.

The implementation centers around a message identifier that serves as both the storage key and integrity proof. Message content includes sender information, optional recipient specification, textual content, and timestamp information. A global message store maintains the mapping between identifiers and content while supporting efficient queries by conversation and time range.

Storage organization separates message content from conversation structure, allowing the same message to appear in multiple conversations without duplication. This separation also enables efficient indexing and query patterns while maintaining referential integrity through content addressing.

### Linear Connection State Machine

Connection management represents one of the most critical aspects of the BitChat system, involving complex state transitions that must maintain consistency across multiple transport mechanisms. The linear state machine approach eliminates entire classes of bugs by making invalid state transitions impossible through type system enforcement.

Each connection progresses through well-defined states: disconnected, discovering peers, establishing connections, maintaining active sessions, and handling error conditions. State transitions consume the current state and produce a new state along with a list of effects that describe what should happen as a result of the transition.

This design provides several key advantages. Invalid states become impossible because the type system prevents creation of inconsistent state combinations. Audit trails become automatic since every state change produces a clear record of what happened and why. Testing becomes straightforward because state transitions are pure functions that can be verified in isolation.

The implementation defines state variants for each phase of the connection lifecycle, with associated data that captures the relevant information for that phase. Transition functions take the current state and an event, returning the new state and any effects that should be executed. Effect types describe all possible side effects that can result from state transitions.

### Effect-Based Transport Coordination

Transport operations involve complex coordination between multiple protocols and error handling strategies. The effect-based approach separates the planning of what should happen from the execution of those plans, enabling comprehensive testing and flexible execution strategies.

Planning functions take the current application state and desired operation, returning an updated state and a list of effects that describe what should happen. These planning functions are pure, making them easy to test and reason about. Effect execution handles the complex real-world details of network operations, error handling, and retry logic.

The separation provides several benefits. Complex logic can be tested in isolation without requiring network connections or hardware. Error handling strategies can be applied uniformly across all transport mechanisms. Retry logic and circuit breaker patterns can be implemented once and applied to all operations.

Effect types encompass all possible side effects: sending messages via specific transports, initiating peer discovery, updating user interface elements, logging events, and scheduling future operations. Effect execution handles the translation from high-level effects to specific transport operations while managing error conditions and retry policies.

### Traditional UI Coordination

User interface coordination requires patterns that integrate well with existing mobile application frameworks and provide the reactive behavior expected by modern applications. The traditional shared state approach meets these requirements while avoiding the complexity of more advanced patterns in areas where they provide little benefit.

UI state encompasses elements that change frequently and require immediate propagation to user interface components: current conversation selection, typing indicators, notification queues, and connection status displays. This state uses conventional locking mechanisms but remains simple enough to avoid coordination problems.

The approach maintains compatibility with existing Swift and Android implementations while providing the reactive behavior required for responsive user interfaces. State updates trigger events that notify interested components, enabling loose coupling between UI elements and business logic.

Implementation uses standard concurrent data structures with well-understood locking patterns. Event broadcasting ensures that UI components receive timely updates without requiring complex coordination. The simplicity of this approach makes it easy to maintain and debug while providing the reliability required for user-facing functionality.

### Event-Driven Peer Discovery

Peer discovery operates on fundamentally asynchronous events that arrive unpredictably from multiple transport mechanisms. The event-driven approach naturally matches this problem domain while providing clean separation between different discovery protocols.

Discovery events flow through dedicated channels from transport-specific components to centralized processing logic. This design allows each transport mechanism to implement discovery according to its own patterns and constraints while providing unified handling of discovery results.

The event-driven architecture provides natural backpressure handling when discovery events arrive faster than they can be processed. Different transport mechanisms can operate independently without requiring coordination. The centralized processing ensures consistent handling of discovery results regardless of their source.

Event types capture the essential information from discovery operations: peer identification, transport mechanism, signal strength or reliability indicators, and capability information. Processing logic maintains the overall peer database while handling deduplication and conflict resolution between different transport mechanisms.

## Integration Strategy

### CSP Channel-Based Architecture

The various architectural layers integrate through explicit CSP (Communicating Sequential Processes) channels that connect isolated task-based subsystems. This approach eliminates the complexity of a monolithic central coordinator by making all inter-subsystem communication explicit and non-blocking.

The architecture consists of three primary task domains:

**Core Logic Task**: A single async task that owns the NoiseSessionManager, DeliveryTracker, and other core state. It receives Command enums from other tasks via channels and processes them using linear state machines and effect-based coordination internally. This task never blocks on external I/O or UI operations.

**Transport Tasks**: Each transport mechanism (BLE, Nostr) runs in its own spawned task. These tasks listen for network events and send them as Event enums to the Core Logic task via channels. They also receive Effect::SendPacket messages from the Core Logic task for outbound operations.

**UI Task**: The user interface runs in its own task, maintaining UI-specific state using traditional concurrent patterns (Arc<Mutex<T>> where appropriate). It sends user actions as Commands to the Core Logic task and receives AppEvents for display updates.

Channel-based integration provides several key benefits: elimination of shared mutable state between subsystems, explicit communication boundaries that prevent hidden dependencies, non-blocking operation between different architectural domains, and clear separation that enables independent testing and development of each subsystem.

### Cross-Platform Compatibility

The channel-based architecture translates naturally across different deployment targets while maintaining functional compatibility. The explicit communication boundaries and task isolation adapt well to various platform constraints.

**Native Implementation**: Uses tokio::spawn for task creation and tokio channels for communication. Each subsystem runs in true parallel tasks with optimal performance characteristics.

**WASM Implementation**: Adapts seamlessly using wasm-bindgen-futures::spawn_local for task creation. Channels work identically, maintaining the same communication patterns while operating within browser single-threaded constraints.

**Mobile Integration**: The event-driven nature maps perfectly to reactive patterns used in mobile frameworks. Commands from UI to Core Logic translate naturally to Swift Combine publishers or Kotlin Flow streams. AppEvents from Core Logic to UI integrate cleanly with SwiftUI state updates or Compose recomposition triggers.

## Performance Considerations

### Overhead Analysis

The CSP channel-based architecture introduces specific overhead characteristics that must be balanced against their benefits. Channel communication adds serialization/deserialization overhead for message passing. Task spawning introduces memory overhead for isolated task contexts. Bounded channels require careful sizing to balance memory usage against blocking behavior.

The analysis indicates acceptable overhead levels for BitChat's use cases. Channel communication overhead is minimal compared to network I/O operations and is offset by elimination of lock contention. Task isolation overhead is justified by the elimination of deadlock conditions and improved testability. Bounded channel overhead prevents uncontrolled memory growth while enabling graceful degradation under load.

Performance optimization focuses on the critical paths while accepting overhead in coordination areas. The Core Logic task optimizes for minimal latency in message processing, with the acknowledged trade-off that complex operations may create temporary bottlenecks. Transport tasks optimize for throughput while maintaining isolation boundaries. UI updates maintain responsiveness through non-blocking communication that degrades gracefully when the system is under load.

### Scalability Characteristics

The CSP channel-based architecture provides excellent scalability characteristics through task isolation and explicit communication boundaries, with the acknowledged limitation that the Core Logic task represents a serialization bottleneck. Content-addressed messages scale well with message volume through automatic deduplication. Transport tasks scale independently without requiring coordination between different mechanisms.

Bottleneck analysis identifies the Core Logic task as the primary scaling constraint since all core operations must be processed sequentially. This design prioritizes correctness over maximum throughput. If performance testing reveals bottlenecks, the Core Logic task can be decomposed into specialized sub-tasks (SessionManagerTask, FragmentationTask, DeliveryTrackerTask) that coordinate through additional channels while maintaining the overall architectural benefits.

## Risk Assessment and Mitigation

### Technical Risks

**Core Logic Bottleneck**: The primary technical risk is that the single Core Logic task may become a performance bottleneck under load, causing user interface lag or dropped network events. Mitigation includes performance benchmarking early in development and maintaining the option to decompose into specialized sub-tasks if needed.

**Channel Backpressure**: Bounded channels can cause UI freezing if not handled properly. This is mitigated by using `try_send` for UI-to-Core communication, allowing the UI to gracefully handle channel fullness by dropping non-critical commands or displaying system busy indicators.

**Effect/AppEvent Separation**: Risk of coupling core logic to UI concerns if the distinction between Effects and AppEvents is not maintained. Mitigation through strict interface definitions and code review processes that enforce the separation.

Implementation complexity represents an additional risk, as the hybrid approach requires understanding multiple architectural patterns. Mitigation strategies include comprehensive documentation, clear interface definitions between subsystems, and parallel development that allows each pattern to be implemented and tested independently.

### Business Risks

Development timeline risks are mitigated through the focused clean-slate approach that eliminates the complexity of migration logic and compatibility layers. The clear architectural boundaries allow parallel development of different subsystems while reducing coordination overhead.

Team adoption risks are addressed through comprehensive documentation and clear separation of concerns. Each architectural pattern operates independently, allowing team members to focus on one pattern at a time while building expertise gradually.

Integration risks are minimized through explicit channel-based communication that provides clear interfaces between subsystems. The CSP approach makes all communication explicit and eliminates the hidden coordination problems that cause deadlocks in shared-state architectures.

## Implementation Strategy

### Clean-Slate CSP Architecture

The implementation strategy abandons incremental migration in favor of a focused clean-slate implementation built around CSP channel communication. This approach prioritizes architectural clarity and eliminates deadlock issues through task isolation and explicit communication boundaries.

### Task-Based Implementation

**Core Logic Task**: The brain of the system runs as a single async task that owns all core state including NoiseSessionManager, DeliveryTracker, and content-addressed message storage. This task receives Commands via channels and processes them using:
- Linear state machines for connection management that make invalid state transitions impossible
- Effect-based coordination for transport operations that separates planning from execution  
- Content-addressed message storage with automatic deduplication and integrity verification

**Performance Trade-off**: This design prioritizes correctness and eliminates deadlocks by serializing all core logic through a single task. While this ensures consistency, it creates a potential performance bottleneck where every command and network event must be processed sequentially. Under extreme load, complex operations like large message reassembly could delay processing of user commands, making the application feel unresponsive. This trade-off is acceptable for initial implementation, with the option to split into specialized sub-tasks (SessionManagerTask, FragmentationTask) if performance benchmarks indicate bottlenecks.

**Transport Tasks**: Each transport mechanism (BLE, Nostr) operates in dedicated spawned tasks that:
- Listen for network events and convert them to Event enums sent to Core Logic
- Receive Effect::SendPacket messages from Core Logic for outbound operations
- Handle transport-specific discovery and connection management independently
- Maintain transport-specific state without requiring coordination with other subsystems

**UI Task**: The user interface operates in its own task with:
- Traditional concurrent patterns (Arc<Mutex<T>>) for UI state management where appropriate
- Command generation from user actions sent to Core Logic via bounded channels using `try_send`
- AppEvent processing from Core Logic for display updates
- Non-blocking communication that prevents UI freezes when Core Logic is under load
- Platform-specific reactive patterns optimized for each deployment target

### Channel Communication Protocol

The system uses typed channels to ensure correct communication between tasks:

```rust
// Commands from UI and external systems to Core Logic
enum Command {
    SendMessage { recipient: PeerId, content: String },
    ConnectToPeer { peer_id: PeerId },
    StartDiscovery,
    StopDiscovery,
}

// Events from Transport tasks to Core Logic  
enum Event {
    PeerDiscovered { peer_id: PeerId, transport: TransportType },
    MessageReceived { from: PeerId, content: String },
    ConnectionEstablished { peer_id: PeerId },
    ConnectionLost { peer_id: PeerId },
}

// Effects from Core Logic to Transport tasks
// Effects describe side effects related to external systems only
enum Effect {
    SendPacket { peer_id: PeerId, data: Vec<u8> },
    InitiateConnection { peer_id: PeerId },
    StartListening,
    WriteToStorage { key: String, data: Vec<u8> },
    ScheduleRetry { delay: Duration, command: Command },
}

// Application events from Core Logic to UI
// AppEvents describe state changes that UI components need to know about
enum AppEvent {
    MessageReceived { from: PeerId, content: String },
    PeerStatusChanged { peer_id: PeerId, status: ConnectionStatus },
    DiscoveryStateChanged { active: bool },
    ConversationUpdated { peer_id: PeerId, message_count: usize },
    SystemBusy { reason: String },
}
```

### Platform Adaptation

**Native Implementation**: Direct tokio::spawn task creation with bounded tokio::sync::mpsc channels for communication. Uses `try_send` for UI-to-Core communication to prevent blocking. Full performance characteristics with true parallelism.

**WASM Implementation**: Uses wasm-bindgen-futures::spawn_local for task creation. Channel communication works identically, maintaining architecture while adapting to single-threaded browser execution. Bounded channels prevent memory issues in constrained browser environments.

**Mobile Bindings**: Event-driven architecture maps directly to reactive mobile patterns. Commands translate to Swift Combine or Kotlin Flow publishers, while AppEvents integrate with SwiftUI or Compose state management.

### Implementation Refinements

**Channel Strategy**: Use bounded channels throughout the system to prevent memory exhaustion. UI tasks use `try_send()` for non-blocking communication, gracefully handling channel fullness by either dropping non-critical updates or showing user feedback about system load.

**Effect vs AppEvent Separation**: Maintain strict separation where Effects describe external side effects (network, storage) and AppEvents describe state changes for UI consumption. The Core Logic task's update function returns `(NewState, Vec<Effect>, Vec<AppEvent>)` to enforce this separation.

**Sub-Task Decomposition Strategy**: Begin with a single Core Logic task for simplicity, but design internal boundaries that allow splitting into specialized tasks if performance testing indicates bottlenecks:
- SessionManagerTask: Handle connection state and cryptographic operations
- FragmentationTask: Manage large message assembly/disassembly  
- DeliveryTrackerTask: Handle message delivery confirmation and retry logic

### CLI Wrapper Design for Integration Testing

**Stateless Design**: CLI wrappers for Swift and Kotlin implementations are designed to be stateless for reliable integration testing. Each test run initializes wrappers with specific identity keys and configuration from the Test Runner, ensuring hermetic tests that don't depend on state from previous runs.

**Minimal Dependencies**: CLI wrappers avoid dependencies on platform-specific persistent storage (Core Data, Room) to enable headless operation in test environments. All necessary state is provided through initialization parameters or maintained in memory only for the duration of the test.

## Success Criteria

### Primary Objectives

Elimination of deadlock issues represents the most critical success criterion, measured through comprehensive testing under high concurrency conditions. The new architecture must demonstrate zero deadlock incidents during stress testing and provide clear audit trails for all state transitions.

Clean architectural boundaries ensure that each subsystem can be developed, tested, and maintained independently. WASM deployment must achieve functional parity with native implementations while meeting browser performance requirements through appropriate pattern adaptations.

### Secondary Objectives

Improved testability measured through increased test coverage and reduced test complexity represents a key secondary objective. The pure functional components should achieve near-complete test coverage with deterministic, fast-running tests.

Developer productivity improvements measured through reduced debugging time and clearer error messages provide ongoing benefits. The architectural separation should make it easier to understand and modify system behavior while reducing the likelihood of introducing bugs.

## Conclusion

This hybrid architectural proposal addresses BitChat's immediate concurrency issues through a clean-slate CSP channel-based implementation that establishes a solid foundation for future development. The selective application of different architectural patterns connected through explicit channel communication provides the benefits of advanced patterns while eliminating the deadlock issues that plague shared-state architectures.

The approach prioritizes architectural clarity and correctness through task isolation and explicit communication boundaries. Channel-based communication eliminates hidden coordination points while enabling independent development and testing of each subsystem. The pragmatic use of well-understood CSP patterns reduces cognitive overhead while providing robust concurrency guarantees.

The proposal provides a clear path forward that solves current technical challenges while positioning BitChat for continued evolution and improvement. The CSP foundation translates naturally across deployment targets and provides the testability and maintainability that make BitChat effective across diverse scenarios from native applications to WASM and mobile integrations.

## Implementation Recommendation

This CSP channel-based hybrid architecture directly addresses the core deadlock issues while providing a robust foundation for future development. The design prioritizes correctness and testability over maximum performance, with explicit acknowledgment of trade-offs and clear paths for optimization if needed.

**Go-Forward Plan**:
1. **Start with Single Core Logic Task**: Begin implementation with a unified Core Logic task, monitoring performance and preparing for decomposition if benchmarks indicate bottlenecks
2. **Enforce Strict Separation**: Maintain clear boundaries between Effects (external side effects) and AppEvents (UI state changes) to prevent coupling
3. **Use Bounded Channels**: Implement bounded channels with `try_send` for UI communication to prevent memory issues and UI freezing
4. **Design Stateless CLI Wrappers**: Create hermetic test environments through stateless mobile CLI wrappers initialized per test run

This architecture represents the optimal balance between implementation complexity and system robustness for BitChat's requirements, providing a maintainable foundation that will scale with the project's growth.
