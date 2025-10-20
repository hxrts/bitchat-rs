# Simulator Refactoring Plan: Foundation for Deterministic Testing

**Date**: October 20, 2025  
**Status**: Proposal - Ready for Implementation  
**Goal**: Refactor existing simulator to prepare for advanced deterministic testing features

## Executive Summary

This document outlines a **preparatory refactoring** of the BitChat simulator that establishes clean abstractions without adding new features. The goal is to create a solid foundation for future enhancements (deterministic simulation, protocol-aware fault injection, RON specifications) as outlined in `simulation-system-evolution.md`.

**Key Principle**: "Make the change easy, then make the easy change"

## Current State Analysis

### Findings from Codebase Audit

**Time Dependencies** (Found in 15+ locations):
- `std::time::Instant::now()` - System time queries
- `tokio::time::sleep()` - Actual waiting
- `tokio::time::timeout()` - Real timeouts
- `tokio::time::interval()` - Timer-based polling

**Randomness Sources** (Found in `network_router.rs`):
- `self.rng.gen()` - Packet corruption
- `self.rng.gen_range()` - Latency jitter, packet loss decisions
- **Issue**: RNG seed not exposed or configurable

**Side Effects** (Process, I/O, Network):
- Process spawning (`tokio::process::Command`)
- stdin/stdout communication
- Network packet routing
- File operations (scenario loading)

**Architecture Strengths**:
- ✅ Good separation between scenario-runner and emulator-rig
- ✅ MockTransport abstraction already exists
- ✅ TOML-based configuration is clean
- ✅ Event-driven architecture (partially implemented)

**Architecture Gaps**:
- ❌ Time is directly coupled to system clock
- ❌ No trait-based abstraction for effects
- ❌ RNG not injectable or reproducible
- ❌ Hard-coded process spawning
- ❌ Direct `tokio` dependencies throughout test code

## Refactoring Strategy: Three-Phase Approach

### Phase 1: Extract Interfaces (Week 1)
**Goal**: Define trait boundaries without changing behavior

**Deliverables**:
1. `SimulationClock` trait
2. `RandomSource` trait  
3. `ProcessExecutor` trait
4. `TransportEffect` trait
5. `StorageEffect` trait

**Success Criteria**: Code compiles, all tests pass, zero behavior changes

---

### Phase 2: Inject Dependencies (Week 2)
**Goal**: Make implementations swappable

**Deliverables**:
1. Update `ScenarioRunner` to accept trait objects
2. Update `NetworkRouter` to accept trait objects
3. Update `EventOrchestrator` to accept trait objects
4. Provide "real" implementations (current behavior)
5. Add `TestContext` struct to bundle traits

**Success Criteria**: Can choose implementations at runtime, tests still pass

---

### Phase 3: Document & Clean (Week 3)
**Goal**: Polish the refactoring

**Deliverables**:
1. API documentation for all new traits
2. Migration guide for test writers
3. Remove dead code
4. Add integration tests for trait boundaries
5. Performance benchmarks (ensure no regression)

**Success Criteria**: Clean, well-documented foundation ready for advanced features

---

## Detailed Implementation Plan

### Phase 1.1: Clock Abstraction

**File**: `simulator/scenario-runner/src/time_abstraction.rs` (new)

```rust
/// Abstraction for time operations in simulation
pub trait SimulationClock: Send + Sync {
    /// Get current time
    fn now(&self) -> SimulationInstant;
    
    /// Sleep for a duration (returns immediately in simulated mode)
    async fn sleep(&self, duration: Duration);
    
    /// Create a timeout future
    async fn timeout<F>(&self, duration: Duration, future: F) -> Result<F::Output, TimeoutError>
    where
        F: Future + Send;
    
    /// Create an interval timer
    fn interval(&self, period: Duration) -> Box<dyn IntervalStream>;
}

/// Time instant that works in both real and simulated modes
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimulationInstant {
    inner: Duration, // Elapsed since epoch
}

impl SimulationInstant {
    pub fn elapsed(&self, now: SimulationInstant) -> Duration {
        now.inner.saturating_sub(self.inner)
    }
}

/// Real implementation using tokio::time
pub struct RealClock;

impl SimulationClock for RealClock {
    fn now(&self) -> SimulationInstant {
        // Use monotonic clock
        let now = std::time::Instant::now();
        // Implementation details...
    }
    
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await
    }
    
    // ... other methods
}
```

**Files to Update**:
- `scenario_runner.rs`: Replace `Instant::now()` → `clock.now()`
- `network_router.rs`: Replace `Instant::now()` → `clock.now()`
- All scenario files: Replace `tokio::time::sleep()` → `clock.sleep()`

**Validation**: Run `cargo test` - all tests should pass

---

### Phase 1.2: Random Source Abstraction

**File**: `simulator/scenario-runner/src/random_abstraction.rs` (new)

```rust
/// Abstraction for random number generation
pub trait RandomSource: Send + Sync {
    /// Generate random value of type T
    fn gen<T>(&mut self) -> T
    where
        T: Distribution<Standard>;
    
    /// Generate random value in range
    fn gen_range<T, R>(&mut self, range: R) -> T
    where
        T: SampleUniform,
        R: SampleRange<T>;
    
    /// Get a seedable clone for deterministic branching
    fn fork(&mut self) -> Box<dyn RandomSource>;
}

/// Real implementation using thread_rng
pub struct SystemRandom {
    rng: ThreadRng,
}

impl RandomSource for SystemRandom {
    fn gen<T>(&mut self) -> T
    where
        T: Distribution<Standard>
    {
        self.rng.gen()
    }
    
    // ... other methods
}

/// Deterministic implementation (for future use)
pub struct SeededRandom {
    rng: StdRng,
    seed: u64,
}

impl RandomSource for SeededRandom {
    fn gen<T>(&mut self) -> T
    where
        T: Distribution<Standard>
    {
        self.rng.gen()
    }
    
    fn fork(&mut self) -> Box<dyn RandomSource> {
        // Create a new seeded RNG derived from current state
        let new_seed = self.rng.gen();
        Box::new(SeededRandom {
            rng: StdRng::seed_from_u64(new_seed),
            seed: new_seed,
        })
    }
}
```

**Files to Update**:
- `network_router.rs`: Add `rng: Box<dyn RandomSource>` field
- Network profile methods: Use `self.rng.gen()` interface

**Validation**: Run network tests with fixed seed, verify reproducibility

---

### Phase 1.3: Process Executor Abstraction

**File**: `simulator/scenario-runner/src/process_abstraction.rs` (new)

```rust
/// Abstraction for process execution
#[async_trait]
pub trait ProcessExecutor: Send + Sync {
    /// Spawn a process with given command and args
    async fn spawn(
        &self,
        command: &str,
        args: &[&str],
        config: ProcessConfig,
    ) -> Result<Box<dyn ProcessHandle>>;
}

/// Handle to a running process
#[async_trait]
pub trait ProcessHandle: Send + Sync {
    /// Send input to process stdin
    async fn send_input(&mut self, data: &str) -> Result<()>;
    
    /// Read next line from stdout
    async fn read_output(&mut self) -> Result<Option<String>>;
    
    /// Wait for process to exit
    async fn wait(&mut self) -> Result<ExitStatus>;
    
    /// Terminate the process
    async fn kill(&mut self) -> Result<()>;
}

pub struct ProcessConfig {
    pub stdin: bool,
    pub stdout: bool,
    pub stderr: bool,
    pub kill_on_drop: bool,
}

/// Real implementation using tokio::process
pub struct TokioProcessExecutor;

#[async_trait]
impl ProcessExecutor for TokioProcessExecutor {
    async fn spawn(
        &self,
        command: &str,
        args: &[&str],
        config: ProcessConfig,
    ) -> Result<Box<dyn ProcessHandle>> {
        let mut cmd = tokio::process::Command::new(command);
        cmd.args(args);
        
        if config.stdin {
            cmd.stdin(std::process::Stdio::piped());
        }
        // ... configure other streams
        
        let process = cmd.spawn()?;
        Ok(Box::new(TokioProcessHandle { process, /* ... */ }))
    }
}
```

**Files to Update**:
- `event_orchestrator.rs`: Accept `Box<dyn ProcessExecutor>` in constructor
- All client spawning code: Use trait methods

**Benefits**:
- Future: Can mock process execution entirely
- Future: Can run multiple "clients" in same process
- Future: Can simulate process crashes deterministically

---

### Phase 1.4: Transport Effect Abstraction

**File**: `simulator/scenario-runner/src/transport_abstraction.rs` (new)

```rust
/// Effects for network transport operations
#[async_trait]
pub trait TransportEffect: Send + Sync {
    /// Send packet to peer
    async fn send_packet(
        &self,
        from: PeerId,
        to: PeerId,
        data: Vec<u8>,
    ) -> Result<()>;
    
    /// Receive next packet (if available)
    async fn recv_packet(&self, peer: PeerId) -> Result<Option<NetworkPacket>>;
    
    /// Check if peer is connected
    async fn is_connected(&self, peer: PeerId) -> bool;
}

/// Real implementation using actual networking
pub struct RealTransportEffect {
    // ... existing mock transport infrastructure
}

/// In-memory implementation for fast testing
pub struct InMemoryTransportEffect {
    packets: Arc<Mutex<VecDeque<NetworkPacket>>>,
    connections: Arc<RwLock<HashSet<PeerId>>>,
}
```

**Files to Update**:
- `MockTransport` becomes implementation of `TransportEffect`
- `NetworkRouter` uses `TransportEffect` trait internally

---

### Phase 2: Dependency Injection

**File**: `simulator/scenario-runner/src/test_context.rs` (new)

```rust
/// Context for running tests with configurable implementations
pub struct TestContext {
    pub clock: Arc<dyn SimulationClock>,
    pub random: Arc<Mutex<dyn RandomSource>>,
    pub process_executor: Arc<dyn ProcessExecutor>,
    pub transport: Arc<dyn TransportEffect>,
}

impl TestContext {
    /// Create context with real implementations (current behavior)
    pub fn real() -> Self {
        Self {
            clock: Arc::new(RealClock),
            random: Arc::new(Mutex::new(SystemRandom::new())),
            process_executor: Arc::new(TokioProcessExecutor),
            transport: Arc::new(RealTransportEffect::new()),
        }
    }
    
    /// Create context with deterministic implementations (future)
    pub fn deterministic(seed: u64) -> Self {
        Self {
            clock: Arc::new(SimulatedClock::new()),
            random: Arc::new(Mutex::new(SeededRandom::new(seed))),
            process_executor: Arc::new(MockProcessExecutor::new()),
            transport: Arc::new(InMemoryTransportEffect::new()),
        }
    }
}
```

**Files to Update**:

```rust
// scenario_runner.rs - BEFORE
pub struct ScenarioRunner {
    config: ScenarioConfig,
    harnesses: HashMap<String, TestHarness>,
    state: Arc<RwLock<ScenarioState>>,
    // ... other fields
}

impl ScenarioRunner {
    pub async fn new(config: ScenarioConfig) -> Result<Self> {
        // ... initialization
    }
}

// scenario_runner.rs - AFTER
pub struct ScenarioRunner {
    config: ScenarioConfig,
    harnesses: HashMap<String, TestHarness>,
    state: Arc<RwLock<ScenarioState>>,
    context: Arc<TestContext>, // NEW: Injected dependencies
}

impl ScenarioRunner {
    pub async fn new(config: ScenarioConfig, context: Arc<TestContext>) -> Result<Self> {
        // ... initialization using context
    }
    
    // Convenience method for backward compatibility
    pub async fn new_with_defaults(config: ScenarioConfig) -> Result<Self> {
        Self::new(config, Arc::new(TestContext::real())).await
    }
}
```

**Migration Path**:
1. Add new constructors with `context` parameter
2. Keep old constructors calling new ones with `TestContext::real()`
3. Update internal code to use `self.context.clock.now()` etc.
4. Gradually migrate tests to use explicit context
5. Eventually deprecate old constructors

---

### Phase 3: Documentation & Cleanup

**File**: `simulator/README.md` (update)

Add section:
```markdown
## Test Infrastructure

### Abstraction Layers

BitChat's simulator uses dependency injection to enable both real and simulated testing:

**Clock Abstraction** (`SimulationClock` trait):
- Enables fast-forwarding time in tests
- Eliminates flaky timing-based tests
- See: `src/time_abstraction.rs`

**Random Source** (`RandomSource` trait):
- Enables reproducible "random" behavior
- Tests can use fixed seeds
- See: `src/random_abstraction.rs`

**Process Execution** (`ProcessExecutor` trait):
- Enables mocking process spawning
- Can run multiple "processes" in-memory
- See: `src/process_abstraction.rs`

**Transport Effects** (`TransportEffect` trait):
- Enables in-memory networking
- Fast test execution
- See: `src/transport_abstraction.rs`

### Writing Tests

```rust
// For fast, deterministic tests:
let context = TestContext::deterministic(42); // Fixed seed
let mut runner = ScenarioRunner::new(config, Arc::new(context)).await?;

// For integration tests with real behavior:
let context = TestContext::real();
let mut runner = ScenarioRunner::new(config, Arc::new(context)).await?;
```
```

**File**: `simulator/docs/migration-guide.md` (new)

```markdown
# Migration Guide: Updated Test Infrastructure

## For Test Writers

### Before
```rust
let mut runner = ScenarioRunner::new(config).await?;
```

### After  
```rust
// Option 1: Use defaults (same behavior as before)
let mut runner = ScenarioRunner::new_with_defaults(config).await?;

// Option 2: Explicit context for control
let context = TestContext::real();
let mut runner = ScenarioRunner::new(config, Arc::new(context)).await?;
```

No other changes required! The internal implementation now uses injected dependencies.

## For Framework Developers

When adding new features that involve time, randomness, or I/O:

1. Use `self.context.clock` instead of `tokio::time`
2. Use `self.context.random` instead of `rand::thread_rng()`
3. Use trait methods instead of direct tokio/std APIs

This enables future deterministic simulation without refactoring.
```

---

## File Structure After Refactoring

```
simulator/scenario-runner/src/
├── time_abstraction.rs       (NEW: Clock trait + implementations)
├── random_abstraction.rs     (NEW: RNG trait + implementations)
├── process_abstraction.rs    (NEW: Process trait + implementations)
├── transport_abstraction.rs  (NEW: Transport trait + implementations)
├── test_context.rs           (NEW: Dependency injection bundle)
├── scenario_runner.rs        (MODIFIED: Accepts TestContext)
├── network_router.rs         (MODIFIED: Uses traits)
├── event_orchestrator.rs     (MODIFIED: Uses traits)
└── ... (other files with minimal changes)

simulator/docs/
├── migration-guide.md        (NEW: How to update tests)
└── architecture-diagram.md   (NEW: Visual of abstraction layers)
```

---

## Testing Strategy

### Phase 1 Validation
After each abstraction:
```bash
# All tests must pass
cargo test

# No behavior changes
cargo test --release -- --nocapture > before.log
# (apply refactoring)
cargo test --release -- --nocapture > after.log
diff before.log after.log  # Should be identical
```

### Phase 2 Validation
After dependency injection:
```bash
# Test with real context
cargo test --features real_context

# Test that we can create deterministic context (doesn't have to work yet)
cargo test test_context_creation
```

### Phase 3 Validation
```bash
# All tests pass
cargo test

# Documentation builds
cargo doc --no-deps --open

# Performance benchmarks
cargo bench
```

---

## Risk Mitigation

**Risk**: Abstraction overhead affects performance  
**Mitigation**: 
- Use trait objects with Arc (single indirection)
- Benchmark before/after
- Accept small overhead for massive testability gains

**Risk**: Complex trait APIs are hard to use  
**Mitigation**:
- Provide high-level `TestContext` bundle
- Keep trait methods minimal
- Extensive documentation with examples

**Risk**: Incomplete abstraction requires later changes  
**Mitigation**:
- Review with simulation-system-evolution.md requirements
- Over-abstract rather than under-abstract
- Plan for extension points

**Risk**: Team confusion during transition  
**Mitigation**:
- Backward-compatible constructors during migration
- Pair programming sessions
- Clear migration guide

---

## Success Metrics

### Quantitative
- [ ] Zero test failures after refactoring
- [ ] <5% performance regression (acceptable)
- [ ] 100% of time operations go through clock trait
- [ ] 100% of random operations go through random trait
- [ ] All tests can optionally use explicit context

### Qualitative
- [ ] Code review approval from 2+ team members
- [ ] Documentation is clear and helpful
- [ ] New abstractions feel "natural" to use
- [ ] Team confident in making future changes

---

## Timeline

**Week 1** (Phase 1 - Extract Interfaces):
- Day 1-2: Clock abstraction + update call sites
- Day 3: Random abstraction + update call sites
- Day 4: Process abstraction + update call sites
- Day 5: Transport abstraction review

**Week 2** (Phase 2 - Inject Dependencies):
- Day 1-2: Create TestContext + update ScenarioRunner
- Day 3: Update NetworkRouter and EventOrchestrator
- Day 4-5: Update all scenario files

**Week 3** (Phase 3 - Document & Polish):
- Day 1-2: Write documentation
- Day 3: Code cleanup and dead code removal
- Day 4: Integration testing
- Day 5: Team review and finalization

**Total**: 3 weeks, ready for advanced features

---

## Next Steps

After this refactoring, the simulator will be ready for:

1. **Phase 1 of simulation-system-evolution.md**: Implement deterministic clock/RNG
2. **Phase 2**: Protocol-aware fault injection using transport abstraction
3. **Phase 3**: Full algebraic effects with mock implementations
4. **Phase 4**: RON specifications and code generation
5. **Phase 5**: Branching and time-travel debugging

But those are future features. This refactoring just prepares the foundation.

---

## Questions for Discussion

1. Do these abstractions cover all necessary extension points?
2. Are the trait APIs too complex or just right?
3. Should we refactor all 3 phases together or validate after each phase?
4. Any other sources of non-determinism we missed?
5. Is 3 weeks a reasonable timeline?

---

**Document Status**: Ready for Review and Implementation

