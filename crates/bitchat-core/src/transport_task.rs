//! Transport Task Trait Definition
//!
//! Defines the common interface for transport tasks in the BitChat hybrid architecture.
//! Concrete implementations live in their respective crates (bitchat-ble, bitchat-nostr).

use crate::{
    channel::{ChannelTransportType, EffectReceiver, EventSender},
    Result as BitchatResult,
};

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

// ----------------------------------------------------------------------------
// Transport Task Trait
// ----------------------------------------------------------------------------

/// Common interface for transport tasks
///
/// Transport tasks are independent async tasks that handle network communication
/// for specific transport protocols (BLE, Nostr, etc.). They communicate with
/// the Core Logic task via CSP channels and execute effects received from it.
///
/// ## Architecture
///
/// Each transport task:
/// - Runs independently with its own async event loop via the `run()` method
/// - Receives effects from Core Logic via `EffectReceiver` channel
/// - Sends events to Core Logic via `EventSender` channel
/// - Manages transport-specific network operations
/// - Maintains no shared state with other tasks
/// - Lifecycle (spawning/aborting) is managed by `BitchatRuntime`
///
/// ## Implementations
///
/// Concrete implementations are provided in separate crates:
/// - `BleTransportTask` in `bitchat-ble` crate
/// - `NostrTransportTask` in `bitchat-nostr` crate
#[async_trait::async_trait]
pub trait TransportTask: Send + Sync {
    /// Attach CSP channels created by the runtime
    ///
    /// Transport implementations must store these handles internally and use them
    /// for all communication with the Core Logic task.
    fn attach_channels(
        &mut self,
        event_sender: EventSender,
        effect_receiver: EffectReceiver,
    ) -> BitchatResult<()>;

    /// Run the transport's main event loop
    ///
    /// This future should run until the transport is shut down. The implementation
    /// should handle initialization, establish necessary connections, process effects
    /// from the Core Logic task, and perform cleanup when the future is cancelled.
    ///
    /// The `BitchatRuntime` is responsible for spawning this as a task and managing
    /// its lifecycle (including cancellation).
    async fn run(&mut self) -> BitchatResult<()>;

    /// Get the transport type identifier
    ///
    /// Used by the Core Logic task to identify which transport this task handles.
    fn transport_type(&self) -> ChannelTransportType;
}

// ----------------------------------------------------------------------------
// Note: Concrete implementations should be in their respective crates:
// - BleTransportTask in bitchat-ble crate
// - NostrTransportTask in bitchat-nostr crate
// - StubTransportTask in testing::mocks module for testing
// ----------------------------------------------------------------------------
