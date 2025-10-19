//! Supervisor Task
//!
//! Manages the lifecycle of decomposed runtime tasks, providing:
//! - Task spawning and monitoring
//! - Failure detection and restart logic  
//! - Channel monitoring and diagnostics
//! - Graceful shutdown coordination

use crate::tasks::{
    InternalCommand, InternalEvent, MessageIngressTask, SessionManagerTask, StorageDeliveryTask,
    TaskHealthStatus,
};
use alloc::string::String;
use bitchat_core::{
    config::{DeliveryConfig, SessionConfig},
    internal::{AppEventSender, CommandReceiver, EffectSender, EventReceiver, TaskId},
    BitchatResult, PeerId,
};
use std::collections::HashMap;
use tokio::{
    sync::mpsc,
    task::JoinHandle,
    time::{interval, Duration, Instant},
};

#[cfg(not(feature = "std"))]
use log::{debug, error, info, warn};
#[cfg(feature = "std")]
use tracing::{debug, error, info, warn};

// ----------------------------------------------------------------------------
// Supervisor Task
// ----------------------------------------------------------------------------

/// Supervises and coordinates the decomposed runtime tasks
pub struct SupervisorTask {
    // Task handles
    ingress_handle: Option<JoinHandle<BitchatResult<()>>>,
    session_handle: Option<JoinHandle<BitchatResult<()>>>,
    storage_handle: Option<JoinHandle<BitchatResult<()>>>,

    // Inter-task communication channels
    session_command_sender: mpsc::UnboundedSender<InternalCommand>,
    storage_command_sender: mpsc::UnboundedSender<InternalCommand>,
    internal_event_receiver: mpsc::UnboundedReceiver<InternalEvent>,

    // Health monitoring
    task_health: HashMap<TaskId, TaskHealthInfo>,
    monitoring_interval: Duration,
    last_health_check: Instant,

    // Configuration
    peer_id: PeerId,
    #[allow(dead_code)]
    restart_failed_tasks: bool,
    #[allow(dead_code)]
    max_restart_attempts: u32,

    // State
    running: bool,
    shutdown_requested: bool,
}

#[derive(Debug, Clone)]
struct TaskHealthInfo {
    status: TaskHealthStatus,
    last_update: Instant,
    #[allow(dead_code)]
    restart_count: u32,
    last_error: Option<String>,
}

impl SupervisorTask {
    /// Create a new supervisor with the given configuration
    pub fn new(
        peer_id: PeerId,
        monitoring_interval: Duration,
        restart_failed_tasks: bool,
        max_restart_attempts: u32,
    ) -> Self {
        let (session_command_sender, _) = mpsc::unbounded_channel();
        let (storage_command_sender, _) = mpsc::unbounded_channel();
        let (_, internal_event_receiver) = mpsc::unbounded_channel();

        Self {
            ingress_handle: None,
            session_handle: None,
            storage_handle: None,
            session_command_sender,
            storage_command_sender,
            internal_event_receiver,
            task_health: HashMap::new(),
            monitoring_interval,
            last_health_check: Instant::now(),
            peer_id,
            restart_failed_tasks,
            max_restart_attempts,
            running: false,
            shutdown_requested: false,
        }
    }

    /// Start all supervised tasks
    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        &mut self,
        command_receiver: CommandReceiver,
        event_receiver: EventReceiver,
        effect_sender: EffectSender,
        app_event_sender: AppEventSender,
        session_config: SessionConfig,
        delivery_config: DeliveryConfig,
        rate_limit_config: bitchat_core::internal::RateLimitConfig,
    ) -> BitchatResult<()> {
        info!("Starting supervisor and decomposed tasks");

        // Create inter-task channels
        let (session_command_sender, session_command_receiver) = mpsc::unbounded_channel();
        let (storage_command_sender, storage_command_receiver) = mpsc::unbounded_channel();
        let (internal_event_sender, internal_event_receiver) = mpsc::unbounded_channel();

        self.session_command_sender = session_command_sender.clone();
        self.storage_command_sender = storage_command_sender.clone();
        self.internal_event_receiver = internal_event_receiver;

        // Initialize health tracking
        self.task_health.insert(
            TaskId::CoreLogic,
            TaskHealthInfo {
                status: TaskHealthStatus::Healthy,
                last_update: Instant::now(),
                restart_count: 0,
                last_error: None,
            },
        );
        self.task_health.insert(
            TaskId::SessionManager,
            TaskHealthInfo {
                status: TaskHealthStatus::Healthy,
                last_update: Instant::now(),
                restart_count: 0,
                last_error: None,
            },
        );
        self.task_health.insert(
            TaskId::DeliveryManager,
            TaskHealthInfo {
                status: TaskHealthStatus::Healthy,
                last_update: Instant::now(),
                restart_count: 0,
                last_error: None,
            },
        );

        // Start Message Ingress Task
        let mut ingress_task = MessageIngressTask::new(
            self.peer_id,
            command_receiver,
            event_receiver,
            effect_sender.clone(),
            app_event_sender,
            session_command_sender,
            storage_command_sender,
            rate_limit_config,
        );

        self.ingress_handle = Some(tokio::spawn(async move { ingress_task.run().await }));

        // Start Session Manager Task
        let mut session_task = SessionManagerTask::new(
            self.peer_id,
            session_command_receiver,
            internal_event_sender.clone(),
            effect_sender,
            session_config,
        )?;

        self.session_handle = Some(tokio::spawn(async move { session_task.run().await }));

        // Start Storage/Delivery Task
        let mut storage_task = StorageDeliveryTask::new(
            storage_command_receiver,
            internal_event_sender,
            delivery_config,
        )?;

        self.storage_handle = Some(tokio::spawn(async move { storage_task.run().await }));

        self.running = true;
        info!("All decomposed tasks started successfully");
        Ok(())
    }

    /// Run the supervisor main loop
    pub async fn run(&mut self) -> BitchatResult<()> {
        info!("Supervisor task starting");

        let mut health_check_interval = interval(self.monitoring_interval);

        while self.running && !self.shutdown_requested {
            tokio::select! {
                // Process internal events from tasks
                Some(event) = self.internal_event_receiver.recv() => {
                    self.handle_internal_event(event).await;
                }

                // Periodic health monitoring
                _ = health_check_interval.tick() => {
                    self.perform_health_check().await;
                }

                // Check for task completion
                _ = async {
                    if let Some(handle) = &mut self.ingress_handle {
                        if handle.is_finished() {
                            return Some("ingress");
                        }
                    }
                    if let Some(handle) = &mut self.session_handle {
                        if handle.is_finished() {
                            return Some("session");
                        }
                    }
                    if let Some(handle) = &mut self.storage_handle {
                        if handle.is_finished() {
                            return Some("storage");
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    None::<&str>
                } => {
                    warn!("Task monitoring task completed unexpectedly");
                }
            }
        }

        info!("Supervisor task stopped");
        Ok(())
    }

    /// Request graceful shutdown of all tasks
    pub async fn shutdown(&mut self) -> BitchatResult<()> {
        info!("Supervisor shutdown requested");
        self.shutdown_requested = true;

        // Send shutdown commands to all tasks
        let _ = self.session_command_sender.send(InternalCommand::Shutdown);
        let _ = self.storage_command_sender.send(InternalCommand::Shutdown);

        // Wait for tasks to complete
        let mut shutdown_timeout = interval(Duration::from_secs(5));
        let shutdown_deadline = Instant::now() + Duration::from_secs(10);

        while Instant::now() < shutdown_deadline {
            let mut all_finished = true;

            if let Some(handle) = &self.ingress_handle {
                if !handle.is_finished() {
                    all_finished = false;
                }
            }
            if let Some(handle) = &self.session_handle {
                if !handle.is_finished() {
                    all_finished = false;
                }
            }
            if let Some(handle) = &self.storage_handle {
                if !handle.is_finished() {
                    all_finished = false;
                }
            }

            if all_finished {
                break;
            }

            shutdown_timeout.tick().await;
        }

        // Force abort any remaining tasks
        if let Some(handle) = self.ingress_handle.take() {
            handle.abort();
        }
        if let Some(handle) = self.session_handle.take() {
            handle.abort();
        }
        if let Some(handle) = self.storage_handle.take() {
            handle.abort();
        }

        self.running = false;
        info!("All tasks shut down");
        Ok(())
    }

    async fn handle_internal_event(&mut self, event: InternalEvent) {
        match event {
            InternalEvent::TaskHealth {
                task_id,
                status,
                message,
            } => {
                self.update_task_health(task_id, status, Some(message));
            }

            InternalEvent::SessionEstablished { peer_id, transport } => {
                debug!(
                    "Session established with peer {:?} via {:?}",
                    peer_id, transport
                );
            }

            InternalEvent::SessionFailed { peer_id, reason } => {
                warn!("Session failed with peer {:?}: {}", peer_id, reason);
            }

            InternalEvent::MessageDecrypted { message_id, .. } => {
                debug!("Message decrypted: {:?}", message_id);
            }

            InternalEvent::MessageStored { message_id, .. } => {
                debug!("Message stored: {:?}", message_id);
            }

            InternalEvent::DeliveryConfirmed {
                message_id,
                peer_id,
            } => {
                debug!(
                    "Delivery confirmed for message {:?} to peer {:?}",
                    message_id, peer_id
                );
            }
        }
    }

    async fn perform_health_check(&mut self) {
        let now = Instant::now();
        let stale_threshold = Duration::from_secs(60);

        for (task_id, health_info) in &mut self.task_health {
            // Check if task health is stale
            if now.duration_since(health_info.last_update) > stale_threshold {
                warn!("Task {:?} health is stale, marking as degraded", task_id);
                health_info.status = TaskHealthStatus::Degraded;
                health_info.last_update = now;
            }
        }

        self.last_health_check = now;
    }

    fn update_task_health(
        &mut self,
        task_id: TaskId,
        status: TaskHealthStatus,
        message: Option<String>,
    ) {
        let health_info = self
            .task_health
            .entry(task_id)
            .or_insert_with(|| TaskHealthInfo {
                status: TaskHealthStatus::Healthy,
                last_update: Instant::now(),
                restart_count: 0,
                last_error: None,
            });

        health_info.status = status.clone();
        health_info.last_update = Instant::now();

        if let Some(msg) = message {
            health_info.last_error = Some(msg);
        }

        match status {
            TaskHealthStatus::Failed => {
                error!("Task {:?} reported failure", task_id);
                // Could trigger restart logic here if enabled
            }
            TaskHealthStatus::Degraded => {
                warn!("Task {:?} is degraded", task_id);
            }
            TaskHealthStatus::Healthy => {
                debug!("Task {:?} is healthy", task_id);
            }
        }
    }

    #[allow(dead_code)]
    async fn handle_task_failure(&mut self, task_name: &str) {
        error!("Task {} failed", task_name);

        if !self.restart_failed_tasks {
            warn!("Task restart disabled, not restarting {}", task_name);
            return;
        }

        // Task restart logic would go here
        // For now, just log the failure
        warn!("Task restart not yet implemented for {}", task_name);
    }

    /// Get current health status of all tasks
    pub fn get_health_summary(&self) -> HashMap<TaskId, TaskHealthStatus> {
        self.task_health
            .iter()
            .map(|(id, info)| (*id, info.status.clone()))
            .collect()
    }

    /// Check if all tasks are healthy
    pub fn is_healthy(&self) -> bool {
        self.task_health
            .values()
            .all(|info| matches!(info.status, TaskHealthStatus::Healthy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_supervisor_creation() {
        let supervisor = SupervisorTask::new(
            PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]),
            Duration::from_secs(30),
            true,
            3,
        );

        assert!(!supervisor.running);
        assert!(!supervisor.shutdown_requested);
        assert!(supervisor.is_healthy()); // Should start healthy
    }

    #[test]
    fn test_health_tracking() {
        let mut supervisor = SupervisorTask::new(
            PeerId::new([1, 2, 3, 4, 5, 6, 7, 8]),
            Duration::from_secs(30),
            true,
            3,
        );

        // Initially healthy
        assert!(supervisor.is_healthy());

        // Mark a task as failed
        supervisor.update_task_health(
            TaskId::CoreLogic,
            TaskHealthStatus::Failed,
            Some("Test failure".to_string()),
        );

        assert!(!supervisor.is_healthy());

        let summary = supervisor.get_health_summary();
        assert!(matches!(
            summary.get(&TaskId::CoreLogic),
            Some(TaskHealthStatus::Failed)
        ));
    }
}
