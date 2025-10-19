//! Core Logic Module
//!
//! This module contains the core logic task implementation split into focused components:
//! - `state`: Core application state and statistics
//! - `handlers`: Command and event handlers
//! - `task`: Main CoreLogicTask implementation and coordination
//!
//! ## Architecture Design Trade-offs
//!
//! ### The CoreLogicTask Bottleneck
//!
//! **Current Design**: All core logic is serialized through a single `CoreLogicTask` that owns
//! all critical application state. This task processes commands from the UI and events from 
//! transport tasks sequentially in a single async event loop.
//!
//! **Benefits of This Approach:**
//! - **Eliminates Race Conditions**: No shared mutable state means no data races
//! - **Prevents Deadlocks**: Single-threaded access to critical state eliminates lock contention
//! - **Simplifies Reasoning**: Clear single point of truth for all application state
//! - **Easier Testing**: Deterministic behavior makes integration testing straightforward
//! - **Memory Safety**: No complex lifetime management or shared ownership patterns
//!
//! **Potential Performance Bottleneck:**
//! Under very high load (many active peers, high message volume, multiple transports),
//! this single-threaded approach could become a bottleneck because:
//! - Every command from every UI must be processed sequentially
//! - Every event from every transport must be processed sequentially  
//! - All cryptographic operations happen in this single task
//! - All message storage operations happen in this single task
//!
//! ### Future Decomposition Strategy
//!
//! The current architecture anticipates this potential bottleneck and is structured for
//! decomposition if performance benchmarks reveal it as necessary:
//!
//! **Already Separated Components in `CoreState`:**
//! - `SessionManager`: Handles cryptographic sessions - could become separate task
//! - `DeliveryTracker`: Manages message reliability - could become separate task  
//! - `MessageStore`: Content-addressed storage - could become separate task
//! - `connections`: Per-peer state - could be sharded across multiple tasks
//!
//! **Decomposition Approach (if needed):**
//! 1. **Message Processing Task**: Handle message encryption/decryption
//! 2. **Session Management Task**: Handle cryptographic session lifecycle
//! 3. **Storage Task**: Handle message persistence and retrieval
//! 4. **Connection Sharding**: Split peer management across multiple tasks
//!
//! **When to Consider Decomposition:**
//! - Benchmarks show >100ms p99 latency for command processing
//! - CPU profiling shows the CoreLogicTask is saturated
//! - Real-world deployments report responsiveness issues
//!
//! **Current Recommendation:**
//! Keep the current single-task design until measurements prove it's a bottleneck.
//! The correctness benefits far outweigh hypothetical performance concerns for most use cases.

pub mod state;
pub mod handlers;
pub mod task;

pub use state::{CoreState, CoreStats, SystemTimeSource, LoggerWrapper};
pub use handlers::CommandHandlers;
pub use task::CoreLogicTask;