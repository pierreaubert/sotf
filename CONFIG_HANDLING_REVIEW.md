# Configuration Handling Review - manager_thread.rs

## Executive Summary

The `manager_thread.rs` configuration handling system has a solid foundation with queue-based serialization and crossfade support. However, there are **critical bugs**, performance inefficiencies, and opportunities for improved robustness.

**Critical Issue**: Signal handlers are not actually registered (line 228-239).

---

## Architecture Overview

### Current Flow

```
Config Sources → ConfigUpdateQueue → apply_plugin_update() → ProcessingThread → Crossfade → Swap
                                                            ↓
                                                    PlaybackThread (if channel change)
```

**Config Sources:**
1. File watcher (notify crate) - detects file changes
2. Unix signals (SIGHUP/SIGTERM/SIGINT) - *currently broken*
3. Direct commands (ManagerCommand::UpdatePluginChain)

**Queue System:**
- VecDeque with max 5 pending updates
- Drops oldest when full
- One-at-a-time processing

**Update Process:**
1. Build new PluginHost in processing thread
2. Start crossfade (100ms) or immediate swap (if channel count changes)
3. Update playback thread if channels changed
4. Update shared state

---

## Critical Issues

### 1. **Signal Handlers Not Implemented** ⚠️ HIGH PRIORITY

**Location:** `manager_thread.rs:228-239`

**Problem:**
```rust
fn setup_signal_handler() -> Result<SignalFlags, String> {
    log::debug!("[Config Watcher] Setting up signal handlers (SIGHUP, SIGTERM, SIGINT)");

    let shutdown = Arc::new(AtomicBool::new(false));
    let reload = Arc::new(AtomicBool::new(false));

    log::debug!("[Config Watcher] Signal handlers registered successfully");

    Ok(SignalFlags { shutdown, reload })
}
```

The function creates `AtomicBool` flags but **never registers actual signal handlers**. Signals like SIGHUP, SIGTERM, SIGINT are completely ignored.

**Impact:**
- Unix signal handling doesn't work at all
- Graceful shutdown via SIGTERM fails
- Hot-reload via SIGHUP fails
- Only file watching actually works

**Fix Required:**
Use the `signal-hook` crate to register handlers. Example:

```rust
#[cfg(unix)]
fn setup_signal_handler() -> Result<SignalFlags, String> {
    use signal_hook::consts::signal::*;
    use signal_hook::flag;

    let shutdown = Arc::new(AtomicBool::new(false));
    let reload = Arc::new(AtomicBool::new(false));

    // Register signal handlers
    flag::register(SIGTERM, Arc::clone(&shutdown))
        .map_err(|e| format!("Failed to register SIGTERM handler: {}", e))?;
    flag::register(SIGINT, Arc::clone(&shutdown))
        .map_err(|e| format!("Failed to register SIGINT handler: {}", e))?;
    flag::register(SIGHUP, Arc::clone(&reload))
        .map_err(|e| format!("Failed to register SIGHUP handler: {}", e))?;

    log::debug!("[Config Watcher] Signal handlers registered successfully");

    Ok(SignalFlags { shutdown, reload })
}
```

**Dependencies:**
Add to `Cargo.toml`:
```toml
[target.'cfg(unix)'.dependencies]
signal-hook = "0.3"
```

---

### 2. **ManagerCommand::ReloadConfig Not Implemented**

**Location:** `manager_thread.rs:845-848`

**Problem:**
```rust
ManagerCommand::ReloadConfig => {
    log::debug!("[Manager] Reload config (not implemented)");
    // TODO: Reload config from file
    ManagerResponse::Ok
}
```

**Impact:**
- API command exists but does nothing
- Confusing for users - returns Ok without actually reloading
- File watching works, but manual reload doesn't

**Fix:**
Either remove the command or implement it properly by calling the same logic as file watcher events.

---

## Performance Issues

### 3. **No Debouncing for File Watch Events**

**Location:** `config_watcher.rs:176-196`

**Problem:**
Every file modification event immediately triggers a config reload. Editors often write files multiple times (temp files, atomic writes), causing rapid-fire reloads.

**Example Scenario:**
```
vim saves config.yaml:
  1. Write to .config.yaml.swp
  2. Write to config.yaml~
  3. Rename to config.yaml
→ Result: 3 config reload attempts in <100ms
```

**Impact:**
- Queue fills with duplicate updates
- CPU waste processing identical configs
- Poor user experience during editing

**Suggested Fix:**
Add debouncing with configurable delay (default 300ms):

```rust
struct FileWatcherState {
    last_event_time: Arc<Mutex<std::time::Instant>>,
    debounce_ms: u64,
    pending_event: Arc<Mutex<Option<ConfigEvent>>>,
}

impl FileWatcherState {
    fn should_trigger(&mut self, event: ConfigEvent) -> bool {
        let mut last_time = self.last_event_time.lock().unwrap();
        let now = std::time::Instant::now();

        if now.duration_since(*last_time).as_millis() < self.debounce_ms as u128 {
            // Update pending event, don't trigger yet
            *self.pending_event.lock().unwrap() = Some(event);
            false
        } else {
            // Trigger and reset
            *last_time = now;
            *self.pending_event.lock().unwrap() = None;
            true
        }
    }
}
```

---

### 4. **No Config Validation Before Application**

**Location:** `manager_thread.rs:476-487`

**Problem:**
Config files are parsed but not validated before being queued:

```rust
match load_config_file(config_path) {
    Ok(new_config) => {
        log::debug!("[Manager] Config loaded, enqueuing plugin update");
        config_queue.enqueue(new_config.plugins);  // ← No validation!
    }
    ...
}
```

Invalid configs are only detected when `build_plugin_host()` fails in the processing thread, **after** they've been queued and removed valid updates from the queue.

**Impact:**
- Invalid configs waste queue slots
- No early feedback to users
- Processing thread does the heavy lifting
- Failed updates can't be retried

**Suggested Fix:**
Add validation layer before queuing:

```rust
fn validate_plugin_configs(configs: &[PluginConfig]) -> Result<(), String> {
    for (i, config) in configs.iter().enumerate() {
        // Check plugin type is recognized
        match config.plugin_type.as_str() {
            "EQ" | "gain" | "upmixer" | "compressor" | "gate" |
            "limiter" | "loudness_compensation" | "matrix" |
            "convolution" | "crossover" | "delay" => {},
            unknown => return Err(format!(
                "Unknown plugin type '{}' at index {}", unknown, i
            )),
        }

        // Validate parameters exist
        if config.parameters.is_null() {
            return Err(format!("Plugin {} missing parameters", i));
        }

        // Type-specific validation (e.g., EQ filters well-formed)
        // ...
    }
    Ok(())
}

// In handle_config_event:
match load_config_file(config_path) {
    Ok(new_config) => {
        // Validate before queuing
        if let Err(e) = validate_plugin_configs(&new_config.plugins) {
            log::warn!("[Manager] Invalid config file: {}", e);
            return Ok(false);
        }
        config_queue.enqueue(new_config.plugins);
    }
    ...
}
```

---

### 5. **Full Plugin Chain Rebuild on Every Update**

**Location:** `processing_thread.rs:441-456`

**Problem:**
Every config update rebuilds the **entire** plugin chain, even if only one parameter changed:

```rust
ProcessingCommand::UpdatePlugins(configs) => {
    match build_plugin_host(&configs, sample_rate, channels) {
        Ok(new_host) => {
            state.start_reload(new_host);  // ← Full rebuild
            ...
        }
    }
}
```

**Impact:**
- Expensive plugin re-initialization (SOFA loading, FFT setup, etc.)
- Unnecessary audio glitches from crossfade
- Poor responsiveness for simple parameter tweaks

**Suggested Fix:**
Add config diffing to determine if full rebuild is needed:

```rust
fn needs_full_rebuild(old: &[PluginConfig], new: &[PluginConfig]) -> bool {
    // Different number of plugins
    if old.len() != new.len() {
        return true;
    }

    // Check if plugin types or order changed
    for (o, n) in old.iter().zip(new.iter()) {
        if o.plugin_type != n.plugin_type {
            return true;
        }
        // Check structural parameters (e.g., filter count for EQ)
        if plugin_structure_changed(o, n) {
            return true;
        }
    }

    false
}
```

Then use `ProcessingCommand::SetParameter` for incremental updates when possible.

---

## Robustness Issues

### 6. **No Rollback on Failed Updates**

**Location:** `manager_thread.rs:509-574`

**Problem:**
If a plugin update fails, there's no rollback to the previous working config:

```rust
fn apply_plugin_update(...) -> Result<(), String> {
    processing.send_command(ProcessingCommand::UpdatePlugins(plugins))?;

    match response {
        ProcessingResponse::PluginChainUpdated { ... } => Ok(()),
        ProcessingResponse::Error(e) => {
            Err(format!("Plugin update error: {}", e))
            // ← No rollback! Audio engine might be in broken state
        }
    }
}
```

**Impact:**
- Failed updates leave engine without working plugins
- No way to recover automatically
- Audio might be silent or distorted

**Suggested Fix:**
Keep reference to last working config:

```rust
struct ConfigUpdateQueue {
    queue: VecDeque<PendingConfigUpdate>,
    update_in_progress: bool,
    last_working_config: Option<Vec<PluginConfig>>,  // ← Add this
}

fn apply_plugin_update(...) -> Result<(), String> {
    // ... attempt update ...

    match response {
        ProcessingResponse::PluginChainUpdated { ... } => {
            // Success - save as last working config
            config_queue.last_working_config = Some(plugins.clone());
            Ok(())
        }
        ProcessingResponse::Error(e) => {
            log::error!("[Manager] Plugin update failed: {}", e);

            // Attempt rollback to last working config
            if let Some(ref working) = config_queue.last_working_config {
                log::warn!("[Manager] Rolling back to last working config");
                processing.send_command(
                    ProcessingCommand::UpdatePlugins(working.clone())
                )?;
                // Wait for confirmation...
            }

            Err(format!("Plugin update error: {}", e))
        }
    }
}
```

---

### 7. **Queue Overflow Strategy Too Aggressive**

**Location:** `manager_thread.rs:56-77`

**Problem:**
When queue is full (5 items), oldest update is dropped:

```rust
if self.queue.len() >= MAX_CONFIG_QUEUE_SIZE {
    log::warn!(...);
    self.queue.pop_front(); // ← Drops oldest update
}
```

**Issues:**
- Oldest might be important (e.g., safety-critical gain reduction)
- No prioritization mechanism
- Loss of config state

**Suggested Fix:**
Implement smarter queue management:

```rust
enum ConfigUpdatePriority {
    UserDirect,      // From API/command - highest priority
    SignalReload,    // From SIGHUP - medium priority
    FileWatch,       // From file watcher - lowest priority
}

struct PendingConfigUpdate {
    plugins: Vec<PluginConfig>,
    timestamp: std::time::Instant,
    priority: ConfigUpdatePriority,  // ← Add priority
}

fn enqueue(&mut self, plugins: Vec<PluginConfig>, priority: ConfigUpdatePriority) -> bool {
    if self.queue.len() >= MAX_CONFIG_QUEUE_SIZE {
        // Find lowest priority item to drop
        let min_priority_idx = self.queue
            .iter()
            .enumerate()
            .min_by_key(|(_, u)| u.priority as u8)
            .map(|(i, _)| i);

        if let Some(idx) = min_priority_idx {
            if (self.queue[idx].priority as u8) < (priority as u8) {
                // Drop lower priority item
                self.queue.remove(idx);
            } else {
                // Can't drop anything, reject new update
                log::warn!("Config queue full, rejecting low-priority update");
                return false;
            }
        }
    }

    self.queue.push_back(PendingConfigUpdate {
        plugins,
        timestamp: std::time::Instant::now(),
        priority,
    });
    true
}
```

---

### 8. **Timeout Configuration Too Rigid**

**Location:** `manager_thread.rs:16-18, 519`

**Problem:**
Fixed timeouts don't account for plugin complexity:

```rust
const PLUGIN_INIT_TIMEOUT_MS: u64 = 10000;  // Used for initial load
...
let timeout = std::time::Duration::from_millis(500);  // Used for hot-reload
```

**Issues:**
- Complex plugins (SOFA loading, large convolution) might need >500ms
- Simple parameter changes waste time waiting
- No dynamic adjustment

**Suggested Fix:**
Make timeouts adaptive based on config complexity:

```rust
fn estimate_update_timeout(configs: &[PluginConfig]) -> Duration {
    let mut timeout_ms = 100; // Base timeout

    for config in configs {
        timeout_ms += match config.plugin_type.as_str() {
            "convolution" => 2000,  // SOFA/IR loading is slow
            "upmixer" => 500,       // FFT setup
            "EQ" => 50,             // Fast
            "gain" => 10,           // Very fast
            _ => 100,
        };
    }

    Duration::from_millis(timeout_ms.min(10000))  // Cap at 10s
}
```

---

### 9. **No Update Metrics or Observability**

**Location:** Throughout `manager_thread.rs`

**Problem:**
Limited visibility into config update performance:
- No tracking of update success/failure rates
- No latency measurements
- No queue depth monitoring

**Impact:**
- Hard to debug production issues
- Can't detect degradation
- No alerting on failures

**Suggested Fix:**
Add metrics structure:

```rust
#[derive(Default, Debug, Clone)]
struct ConfigUpdateMetrics {
    total_updates: u64,
    successful_updates: u64,
    failed_updates: u64,
    total_update_time_ms: u64,
    max_queue_depth: usize,
    last_update_timestamp: Option<Instant>,
}

impl ConfigUpdateMetrics {
    fn record_success(&mut self, duration: Duration) {
        self.total_updates += 1;
        self.successful_updates += 1;
        self.total_update_time_ms += duration.as_millis() as u64;
        self.last_update_timestamp = Some(Instant::now());
    }

    fn record_failure(&mut self) {
        self.total_updates += 1;
        self.failed_updates += 1;
    }

    fn update_queue_depth(&mut self, depth: usize) {
        self.max_queue_depth = self.max_queue_depth.max(depth);
    }

    fn success_rate(&self) -> f64 {
        if self.total_updates == 0 {
            return 1.0;
        }
        self.successful_updates as f64 / self.total_updates as f64
    }

    fn avg_update_time_ms(&self) -> f64 {
        if self.successful_updates == 0 {
            return 0.0;
        }
        self.total_update_time_ms as f64 / self.successful_updates as f64
    }
}

// Add to ConfigUpdateQueue
struct ConfigUpdateQueue {
    queue: VecDeque<PendingConfigUpdate>,
    update_in_progress: bool,
    metrics: ConfigUpdateMetrics,  // ← Add this
}
```

Expose via new command:
```rust
ManagerCommand::GetMetrics => {
    ManagerResponse::Metrics(config_queue.metrics.clone())
}
```

---

### 10. **Race Condition: File Watcher vs Direct Commands**

**Location:** `manager_thread.rs:338-369`

**Problem:**
File watcher events and direct commands use same queue with no coordination:

```
Thread 1 (User):    UpdatePluginChain(A) →
Thread 2 (Watcher):                        ConfigChanged → Load(B) → Queue(B)
                                                                              ↓
                    Queue(A) → Process(A) → Process(B)  ← Unexpected order!
```

User expects config A to be active, but file watcher overwrites it with B.

**Impact:**
- User changes can be overwritten
- Non-deterministic behavior
- Debugging nightmare

**Suggested Fix:**
Add config source tracking and conflict resolution:

```rust
enum ConfigSource {
    FileWatcher { path: PathBuf },
    DirectCommand,
    SignalReload,
}

struct PendingConfigUpdate {
    plugins: Vec<PluginConfig>,
    timestamp: std::time::Instant,
    source: ConfigSource,  // ← Add source tracking
}

fn handle_config_event(...) -> Result<bool, String> {
    match event {
        ConfigEvent::ConfigChanged(path) => {
            // Check if there are pending direct commands
            if config_queue.has_direct_command_pending() {
                log::info!(
                    "[Manager] Skipping file watcher update - direct command pending"
                );
                return Ok(false);
            }

            // Load and queue with FileWatcher source
            ...
        }
    }
}
```

---

## Additional Improvements

### 11. **Add Config Versioning**

Prevent stale updates from being applied:

```rust
struct EngineConfig {
    // ... existing fields ...
    version: u64,  // Increment on each change
}

// Reject outdated configs
if new_config.version <= current_config.version {
    log::warn!("Rejecting stale config (version {} <= {})",
               new_config.version, current_config.version);
    return;
}
```

### 12. **Async Config Loading**

Move file I/O off the manager thread:

```rust
// Spawn a task to load config asynchronously
std::thread::spawn(move || {
    match load_config_file(&path) {
        Ok(config) => {
            event_tx.send(ConfigEvent::ConfigLoaded(config)).ok();
        }
        Err(e) => {
            log::error!("Failed to load config: {}", e);
        }
    }
});
```

### 13. **Better Error Types**

Replace `String` errors with structured types:

```rust
#[derive(Debug, Clone)]
enum ConfigError {
    ParseError { path: PathBuf, reason: String },
    ValidationError { plugin_index: usize, reason: String },
    TimeoutError { waited_ms: u64 },
    ChannelDisconnected,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::ParseError { path, reason } =>
                write!(f, "Failed to parse {:?}: {}", path, reason),
            Self::ValidationError { plugin_index, reason } =>
                write!(f, "Plugin {} invalid: {}", plugin_index, reason),
            Self::TimeoutError { waited_ms } =>
                write!(f, "Update timeout after {}ms", waited_ms),
            Self::ChannelDisconnected =>
                write!(f, "Thread communication channel disconnected"),
        }
    }
}
```

---

## Summary of Recommendations

### Must Fix (High Priority)
1. ✅ **Implement signal handlers** - Currently completely broken
2. ✅ **Add config validation** - Prevent invalid configs from breaking system
3. ✅ **Implement rollback** - Recover from failed updates

### Should Fix (Medium Priority)
4. ✅ **Add debouncing** - Prevent rapid-fire file watcher updates
5. ✅ **Priority-based queue** - Don't drop important updates
6. ✅ **Adaptive timeouts** - Handle complex plugins properly
7. ✅ **Add metrics** - Essential for production monitoring

### Nice to Have (Low Priority)
8. ✅ **Config diffing** - Optimize incremental updates
9. ✅ **Config versioning** - Prevent stale updates
10. ✅ **Source tracking** - Resolve conflicts between update sources
11. ✅ **Better error types** - Improve debugging

---

## Testing Recommendations

Add tests for:
1. **Signal handling** - Verify SIGHUP/SIGTERM work
2. **Queue overflow** - Test priority-based dropping
3. **Rollback** - Simulate failures and verify recovery
4. **Debouncing** - Rapid file changes should coalesce
5. **Race conditions** - File watcher vs direct commands
6. **Invalid configs** - Should be rejected early
7. **Timeout scenarios** - Both success and timeout paths

---

## Performance Metrics to Track

Monitor these metrics in production:
- Config update success rate (should be >99%)
- Average update latency (should be <200ms for simple configs)
- Queue depth (should rarely exceed 1-2)
- Update failures per hour
- Rollback frequency
- Timeout frequency

---

## Conclusion

The config handling system has good bones but needs attention in several critical areas:

1. **Critical bugs** (signal handlers) must be fixed immediately
2. **Validation and rollback** will dramatically improve robustness
3. **Performance optimizations** (debouncing, diffing) will improve UX
4. **Observability** (metrics, better errors) will ease debugging

Estimated effort:
- Critical fixes: 1-2 days
- Medium priority: 2-3 days
- Low priority: 2-3 days
- Testing: 1-2 days

**Total: ~1.5 weeks for full implementation**
