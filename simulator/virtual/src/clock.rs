//! Time abstraction for deterministic simulation
//!
//! Provides a clean trait for time operations that can be swapped between
//! real time (SystemClock) and simulated time (VirtualClock).

#![allow(dead_code)] // Public API for deterministic testing

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::{sleep, timeout, Instant, Interval, interval};

/// Abstraction for time operations in simulation
pub trait SimulationClock: Send + Sync {
    /// Get current time instant
    fn now(&self) -> SimulationInstant;
    
    /// Sleep for a duration
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
    
    /// Create an interval timer
    fn interval(&self, period: Duration) -> Box<dyn IntervalStream + Send + Unpin>;
}

/// Time instant that works in both real and simulated modes
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimulationInstant {
    inner: Instant,
}

impl SimulationInstant {
    pub fn elapsed(&self, now: SimulationInstant) -> Duration {
        now.inner.duration_since(self.inner)
    }
    
    pub fn duration_since(&self, earlier: SimulationInstant) -> Duration {
        self.inner.duration_since(earlier.inner)
    }
}

/// Timeout error
#[derive(Debug, Clone, Copy)]
pub struct TimeoutError;

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation timed out")
    }
}

impl std::error::Error for TimeoutError {}

/// Trait for interval streams
pub trait IntervalStream {
    fn tick(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

struct TokioInterval {
    inner: Interval,
}

impl IntervalStream for TokioInterval {
    fn tick(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            self.inner.tick().await;
        })
    }
}

/// Real-time implementation using tokio::time
pub struct SystemClock;

impl SystemClock {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulationClock for SystemClock {
    fn now(&self) -> SimulationInstant {
        SimulationInstant {
            inner: Instant::now(),
        }
    }
    
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
        Box::pin(async move {
            sleep(duration).await;
        })
    }
    
    fn interval(&self, period: Duration) -> Box<dyn IntervalStream + Send + Unpin> {
        Box::new(TokioInterval {
            inner: interval(period),
        })
    }
}

impl SystemClock {
    /// Helper method for timeout (not part of trait to keep it object-safe)
    pub async fn timeout_future<F, T>(&self, duration: Duration, future: F) -> Result<T, TimeoutError>
    where
        F: Future<Output = T>,
    {
        timeout(duration, future)
            .await
            .map_err(|_| TimeoutError)
    }
}

// ============================================================================
// Virtual Clock - Deterministic time control for testing
// ============================================================================

/// Virtual clock for deterministic time control in tests
pub struct VirtualClock {
    state: Arc<Mutex<VirtualClockState>>,
}

struct VirtualClockState {
    current_time: Duration,
    waiters: Vec<(Duration, tokio::sync::oneshot::Sender<()>)>,
}

impl VirtualClock {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(VirtualClockState {
                current_time: Duration::ZERO,
                waiters: Vec::new(),
            })),
        }
    }

    /// Manually advance virtual time by the given duration
    pub fn advance(&self, duration: Duration) {
        let mut state = self.state.lock().unwrap();
        state.current_time += duration;
        let current = state.current_time;
        
        // Wake up any waiters whose time has come
        let mut i = 0;
        while i < state.waiters.len() {
            if state.waiters[i].0 <= current {
                let (_, sender) = state.waiters.swap_remove(i);
                let _ = sender.send(()); // Ignore errors if receiver dropped
            } else {
                i += 1;
            }
        }
    }

    /// Set virtual time to a specific value
    pub fn set_time(&self, time: Duration) {
        let mut state = self.state.lock().unwrap();
        state.current_time = time;
        let current = state.current_time;
        
        // Wake up all waiters whose time has passed
        let mut i = 0;
        while i < state.waiters.len() {
            if state.waiters[i].0 <= current {
                let (_, sender) = state.waiters.swap_remove(i);
                let _ = sender.send(());
            } else {
                i += 1;
            }
        }
    }

    /// Get current virtual time
    pub fn current_time(&self) -> Duration {
        self.state.lock().unwrap().current_time
    }
}

impl Default for VirtualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulationClock for VirtualClock {
    fn now(&self) -> SimulationInstant {
        let duration = self.state.lock().unwrap().current_time;
        // Create an instant that represents the virtual time
        // This is a bit of a hack - we create a fake Instant based on real time
        let base = Instant::now();
        SimulationInstant {
            inner: base.checked_sub(base.elapsed()).unwrap_or(base) + duration,
        }
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
        let state = Arc::clone(&self.state);
        
        Box::pin(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            
            let wake_time = {
                let mut s = state.lock().unwrap();
                let wake_time = s.current_time + duration;
                s.waiters.push((wake_time, tx));
                wake_time
            };
            
            // Wait for manual time advance or timeout as fallback
            let _ = tokio::time::timeout(Duration::from_secs(30), rx).await;
            
            // Check if we actually reached the target time
            let current = state.lock().unwrap().current_time;
            if current < wake_time {
                // Timeout occurred, this is a test failure scenario
                panic!("VirtualClock sleep timed out - did you forget to call advance()?");
            }
        })
    }

    fn interval(&self, period: Duration) -> Box<dyn IntervalStream + Send + Unpin> {
        // For VirtualClock, intervals need manual advancement
        Box::new(VirtualInterval {
            period,
            state: Arc::clone(&self.state),
            next_tick: self.state.lock().unwrap().current_time + period,
        })
    }
}

struct VirtualInterval {
    period: Duration,
    state: Arc<Mutex<VirtualClockState>>,
    next_tick: Duration,
}

impl IntervalStream for VirtualInterval {
    fn tick(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let state = Arc::clone(&self.state);
        let wake_time = self.next_tick;
        self.next_tick += self.period;
        
        Box::pin(async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            
            {
                let mut s = state.lock().unwrap();
                if s.current_time >= wake_time {
                    // Already past the wake time
                    return;
                }
                s.waiters.push((wake_time, tx));
            }
            
            // Wait for time to advance
            let _ = tokio::time::timeout(Duration::from_secs(30), rx).await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_system_clock_now() {
        let clock = SystemClock::new();
        let t1 = clock.now();
        tokio::time::sleep(Duration::from_millis(10)).await;
        let t2 = clock.now();
        
        assert!(t2 > t1);
        assert!(t1.elapsed(t2) >= Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_system_clock_sleep() {
        let clock = SystemClock::new();
        let start = clock.now();
        clock.sleep(Duration::from_millis(50)).await;
        let elapsed = start.elapsed(clock.now());
        
        assert!(elapsed >= Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_system_clock_timeout_success() {
        let clock = SystemClock::new();
        let future = async { 42 };
        
        let result = clock.timeout_future(Duration::from_millis(100), future).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_system_clock_timeout_expires() {
        let clock = SystemClock::new();
        let future = async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            42
        };
        
        let result = clock.timeout_future(Duration::from_millis(50), future).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_system_clock_interval() {
        let clock = SystemClock::new();
        let mut interval = clock.interval(Duration::from_millis(10));
        
        let start = clock.now();
        interval.tick().await;
        interval.tick().await;
        let elapsed = start.elapsed(clock.now());
        
        assert!(elapsed >= Duration::from_millis(10));
    }

    // ========================================================================
    // VirtualClock Tests
    // ========================================================================

    #[tokio::test]
    async fn test_virtual_clock_manual_time_control() {
        let clock = VirtualClock::new();
        
        // Time starts at zero
        assert_eq!(clock.current_time(), Duration::ZERO);
        
        // Advance time manually
        clock.advance(Duration::from_secs(10));
        assert_eq!(clock.current_time(), Duration::from_secs(10));
        
        // Advance again
        clock.advance(Duration::from_secs(5));
        assert_eq!(clock.current_time(), Duration::from_secs(15));
    }

    #[tokio::test]
    async fn test_virtual_clock_set_time() {
        let clock = VirtualClock::new();
        
        clock.set_time(Duration::from_secs(100));
        assert_eq!(clock.current_time(), Duration::from_secs(100));
        
        clock.set_time(Duration::from_secs(50));
        assert_eq!(clock.current_time(), Duration::from_secs(50));
    }

    #[tokio::test]
    async fn test_virtual_clock_sleep_with_advance() {
        let clock = Arc::new(VirtualClock::new());
        let clock_clone = Arc::clone(&clock);
        
        // Spawn a task that sleeps
        let sleep_task = tokio::spawn(async move {
            clock_clone.sleep(Duration::from_secs(10)).await;
            42
        });
        
        // Give the sleep task a moment to register its waiter
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Advance time to wake it up
        clock.advance(Duration::from_secs(10));
        
        // Task should complete
        let result = tokio::time::timeout(Duration::from_secs(1), sleep_task).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_virtual_clock_multiple_waiters() {
        let clock = Arc::new(VirtualClock::new());
        
        // Spawn multiple tasks with different sleep durations
        let clock1 = Arc::clone(&clock);
        let task1 = tokio::spawn(async move {
            clock1.sleep(Duration::from_secs(5)).await;
            "task1"
        });
        
        let clock2 = Arc::clone(&clock);
        let task2 = tokio::spawn(async move {
            clock2.sleep(Duration::from_secs(10)).await;
            "task2"
        });
        
        let clock3 = Arc::clone(&clock);
        let task3 = tokio::spawn(async move {
            clock3.sleep(Duration::from_secs(15)).await;
            "task3"
        });
        
        // Give tasks time to register
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Advance to 5 seconds - task1 should complete
        clock.advance(Duration::from_secs(5));
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(task1.is_finished());
        assert!(!task2.is_finished());
        assert!(!task3.is_finished());
        
        // Advance to 10 seconds - task2 should complete
        clock.advance(Duration::from_secs(5));
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(task2.is_finished());
        assert!(!task3.is_finished());
        
        // Advance to 15 seconds - task3 should complete
        clock.advance(Duration::from_secs(5));
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(task3.is_finished());
    }

    #[tokio::test]
    async fn test_virtual_clock_now_advances_with_time() {
        let clock = VirtualClock::new();
        
        let t1 = clock.now();
        clock.advance(Duration::from_secs(100));
        let t2 = clock.now();
        
        let elapsed = t1.elapsed(t2);
        assert!(elapsed >= Duration::from_secs(100));
    }

    #[tokio::test]
    async fn test_virtual_clock_deterministic() {
        // Same seed should produce identical behavior
        let clock1 = VirtualClock::new();
        let clock2 = VirtualClock::new();
        
        clock1.advance(Duration::from_secs(42));
        clock2.advance(Duration::from_secs(42));
        
        assert_eq!(clock1.current_time(), clock2.current_time());
    }
}

