//! Performance monitoring and metrics collection for RGB effects.
//!
//! This module provides comprehensive performance monitoring for RGB effects,
//! including timing metrics, memory usage tracking, and adaptive timing calculations.
//! Also includes intelligent performance optimizations:
//! - Advanced effect computation caching
//! - HID communication batching and queuing
//! - Memory pool management
//! - Adaptive refresh rates
//! - Background processing
//! - Smart invalidation strategies

use crate::rgb::{Color, Effect, PerKeyEffect};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// RGB timing metrics for performance monitoring as required by the performance foundation plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RgbTimingMetrics {
    /// Current frame time in milliseconds
    pub frame_time: f32,
    /// Target FPS for effects
    pub target_fps: f32,
    /// Actual achieved FPS
    pub actual_fps: f32,
    /// Number of dropped frames in current window
    pub dropped_frames: u32,
    /// Total frames processed
    pub total_frames: u64,
    /// Average computation time in microseconds
    pub avg_computation_time_us: f32,
    /// Current memory usage in bytes
    pub memory_usage_bytes: u64,
    /// Cache hit rate (0.0 to 1.0)
    pub cache_hit_rate: f32,
}

impl Default for RgbTimingMetrics {
    fn default() -> Self {
        Self {
            frame_time: 16.67, // ~60 FPS
            target_fps: 60.0,
            actual_fps: 0.0,
            dropped_frames: 0,
            total_frames: 0,
            avg_computation_time_us: 0.0,
            memory_usage_bytes: 0,
            cache_hit_rate: 0.0,
        }
    }
}

/// Performance monitor with ring buffer for timing history as required by the performance foundation plan.
pub struct PerformanceMonitor {
    /// Ring buffer for frame timing history (last 60 frames)
    timing_history: VecDeque<Duration>,
    /// Ring buffer for computation times (last 60 frames)
    computation_history: VecDeque<Duration>,
    /// Current metrics
    metrics: RgbTimingMetrics,
    /// Last update time
    last_update: Instant,
    /// Start time for FPS calculation
    fps_window_start: Instant,
    /// Frames in current FPS window
    frames_in_window: u32,
    /// Maximum history size
    max_history: usize,
    /// Exponential moving average alpha for smooth metric reporting
    ema_alpha: f32,
    /// Current effect complexity
    current_complexity: EffectComplexity,
    /// Cache hit counter
    cache_hits: u64,
    /// Cache miss counter
    cache_misses: u64,
    /// Dropped frame detection threshold
    drop_threshold_ms: f32,
}

/// Effect complexity scoring for adaptive timing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectComplexity {
    /// Simple static effects - can run at lower framerates
    Simple,
    /// Medium complexity like breathing, spectrum - standard framerate
    Medium,
    /// High complexity like wave, reactive - needs consistent timing
    High,
    /// Critical effects requiring minimal latency - highest framerate
    Critical,
}

impl EffectComplexity {
    /// Get recommended frame budget for this complexity level.
    pub fn frame_budget_ms(&self) -> f32 {
        match self {
            EffectComplexity::Simple => 33.0,   // 30 FPS
            EffectComplexity::Medium => 16.67,  // 60 FPS
            EffectComplexity::High => 8.33,     // 120 FPS
            EffectComplexity::Critical => 4.17, // 240 FPS
        }
    }

    /// Get target FPS for this complexity level.
    pub fn target_fps(&self) -> f32 {
        1000.0 / self.frame_budget_ms()
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceMonitor {
    /// Create a new performance monitor.
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            timing_history: VecDeque::with_capacity(60),
            computation_history: VecDeque::with_capacity(60),
            metrics: RgbTimingMetrics::default(),
            last_update: now,
            fps_window_start: now,
            frames_in_window: 0,
            max_history: 60,
            ema_alpha: 0.1, // Smooth averaging
            current_complexity: EffectComplexity::Medium,
            cache_hits: 0,
            cache_misses: 0,
            drop_threshold_ms: 20.0, // Frame time 20% above target considered dropped
        }
    }

    /// Record a frame timing measurement.
    pub fn record_frame_timing(&mut self, frame_duration: Duration, computation_time: Duration) {
        let now = Instant::now();

        // Add to history (maintain ring buffer size)
        if self.timing_history.len() >= self.max_history {
            self.timing_history.pop_front();
        }
        if self.computation_history.len() >= self.max_history {
            self.computation_history.pop_front();
        }

        self.timing_history.push_back(frame_duration);
        self.computation_history.push_back(computation_time);

        // Update frame counting and dropped frame detection
        self.frames_in_window += 1;
        self.metrics.total_frames += 1;

        // Check if this frame was dropped (significantly over target time)
        let frame_time_ms = frame_duration.as_secs_f32() * 1000.0;
        let target_frame_time = self.current_complexity.frame_budget_ms();
        if frame_time_ms > target_frame_time + self.drop_threshold_ms {
            self.metrics.dropped_frames += 1;
        }

        // Calculate actual FPS every second
        let fps_window_duration = now.duration_since(self.fps_window_start);
        if fps_window_duration >= Duration::from_secs(1) {
            self.metrics.actual_fps =
                self.frames_in_window as f32 / fps_window_duration.as_secs_f32();
            self.frames_in_window = 0;
            self.fps_window_start = now;
        }

        // Update metrics with exponential moving average
        let computation_time_us = computation_time.as_micros() as f32;

        if self.metrics.total_frames == 1 {
            // First measurement - initialize directly
            self.metrics.frame_time = frame_time_ms;
            self.metrics.avg_computation_time_us = computation_time_us;
        } else {
            // Exponential moving average for smooth metrics
            self.metrics.frame_time =
                self.metrics.frame_time * (1.0 - self.ema_alpha) + frame_time_ms * self.ema_alpha;
            self.metrics.avg_computation_time_us = self.metrics.avg_computation_time_us
                * (1.0 - self.ema_alpha)
                + computation_time_us * self.ema_alpha;
        }

        // Update target FPS based on current complexity
        self.metrics.target_fps = self.current_complexity.target_fps();

        // Update cache hit rate
        let total_cache_operations = self.cache_hits + self.cache_misses;
        self.metrics.cache_hit_rate = if total_cache_operations > 0 {
            self.cache_hits as f32 / total_cache_operations as f32
        } else {
            0.0
        };

        self.last_update = now;
    }

    /// Record cache hit/miss for performance tracking.
    pub fn record_cache_hit(&mut self, hit: bool) {
        if hit {
            self.cache_hits += 1;
        } else {
            self.cache_misses += 1;
        }
    }

    /// Set the current effect complexity level.
    pub fn set_effect_complexity(&mut self, complexity: EffectComplexity) {
        self.current_complexity = complexity;
        self.metrics.target_fps = complexity.target_fps();
    }

    /// Calculate optimal timing interval based on effect complexity and system performance.
    ///
    /// This function analyzes recent performance history and current system load to determine
    /// the optimal frame interval for smooth effects while preventing USB bus saturation.
    pub fn calculate_optimal_timing(&self) -> Duration {
        // Start with complexity-based recommendation
        let base_interval_ms = self.current_complexity.frame_budget_ms();

        // Analyze recent performance to detect system load
        let _avg_frame_time = if !self.timing_history.is_empty() {
            let sum: Duration = self.timing_history.iter().sum();
            sum.as_secs_f32() * 1000.0 / self.timing_history.len() as f32
        } else {
            base_interval_ms
        };

        let avg_computation_time = if !self.computation_history.is_empty() {
            let sum: Duration = self.computation_history.iter().sum();
            sum.as_micros() as f32 / (self.computation_history.len() as f32 * 1000.0) // Convert to ms
        } else {
            1.0 // 1ms default computation time
        };

        // Calculate system load factor
        let computation_ratio = avg_computation_time / base_interval_ms;
        let performance_factor = if computation_ratio > 0.8 {
            // System is under load - increase interval
            1.5
        } else if computation_ratio > 0.5 {
            // Moderate load - slight increase
            1.2
        } else if computation_ratio < 0.1 {
            // Very light load - can decrease interval for smoother effects
            0.8
        } else {
            // Normal load - use base timing
            1.0
        };

        // Check for dropped frames (frame time > target + tolerance)
        let target_frame_time = base_interval_ms;
        let tolerance = self.drop_threshold_ms;
        let dropped_frame_ratio = if !self.timing_history.is_empty() {
            let dropped = self
                .timing_history
                .iter()
                .filter(|&t| t.as_secs_f32() * 1000.0 > target_frame_time + tolerance)
                .count();
            dropped as f32 / self.timing_history.len() as f32
        } else {
            0.0
        };

        // Apply dropped frame penalty
        let drop_penalty = if dropped_frame_ratio > 0.1 {
            1.3 // Increase interval if dropping > 10% of frames
        } else {
            1.0
        };

        // Calculate final optimal interval
        let optimal_ms = base_interval_ms * performance_factor * drop_penalty;

        // Enforce absolute limits for USB bus protection and user experience
        let final_ms = optimal_ms.clamp(4.0, 100.0); // 4ms to 100ms range (250Hz to 10Hz)

        Duration::from_millis(final_ms as u64)
    }

    /// Get current performance metrics.
    pub fn metrics(&self) -> &RgbTimingMetrics {
        &self.metrics
    }

    /// Update memory usage tracking.
    pub fn update_memory_usage(&mut self, bytes: u64) {
        self.metrics.memory_usage_bytes = bytes;
    }

    /// Check if performance is degraded.
    pub fn is_performance_degraded(&self) -> bool {
        // Consider performance degraded if:
        // 1. Actual FPS is significantly below target (< 80% of target)
        // 2. Frame time is consistently high
        // 3. High number of recent dropped frames

        let fps_ratio = if self.metrics.target_fps > 0.0 {
            self.metrics.actual_fps / self.metrics.target_fps
        } else {
            1.0
        };

        let high_frame_time =
            self.metrics.frame_time > self.current_complexity.frame_budget_ms() * 1.5;
        let low_fps = fps_ratio < 0.8;
        let high_cache_miss =
            self.metrics.cache_hit_rate < 0.5 && (self.cache_hits + self.cache_misses) > 10;

        high_frame_time || low_fps || high_cache_miss
    }

    /// Reset all performance counters.
    pub fn reset(&mut self) {
        self.timing_history.clear();
        self.computation_history.clear();
        self.metrics = RgbTimingMetrics::default();
        let now = Instant::now();
        self.last_update = now;
        self.fps_window_start = now;
        self.frames_in_window = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.metrics.dropped_frames = 0;
    }

    /// Get performance summary string for debugging.
    pub fn performance_summary(&self) -> String {
        format!(
            "FPS: {:.1}/{:.1} | Frame: {:.1}ms | Compute: {:.0}μs | Cache: {:.1}% | Dropped: {} | Complexity: {:?}",
            self.metrics.actual_fps,
            self.metrics.target_fps,
            self.metrics.frame_time,
            self.metrics.avg_computation_time_us,
            self.metrics.cache_hit_rate * 100.0,
            self.metrics.dropped_frames,
            self.current_complexity
        )
    }
}

/// Determine effect complexity for adaptive timing.
pub fn calculate_effect_complexity(effect: &Effect) -> EffectComplexity {
    match effect {
        Effect::Static { .. } => EffectComplexity::Simple,
        Effect::Off => EffectComplexity::Simple,
        Effect::Custom { .. } => EffectComplexity::Simple,

        Effect::Breathing { .. } => EffectComplexity::Medium,
        Effect::Spectrum { .. } => EffectComplexity::Medium,
        Effect::Gradient { .. } => EffectComplexity::Medium,

        Effect::Wave { colors, .. } => {
            if colors.len() > 4 {
                EffectComplexity::High
            } else {
                EffectComplexity::Medium
            }
        }

        Effect::Reactive { .. } => EffectComplexity::Critical,
    }
}

/// Memory usage estimation for daemon stability monitoring.
pub fn estimate_memory_usage() -> u64 {
    // Use basic process memory estimation
    // In a production system, this would integrate with system APIs
    // For now, estimate based on typical RGB daemon usage

    let base_daemon_memory = 8 * 1024 * 1024; // 8MB base
    let rgb_buffers = 1024 * 1024; // 1MB for RGB buffers
    let cache_memory = 512 * 1024; // 512KB for effect caches

    base_daemon_memory + rgb_buffers + cache_memory
}

/// Performance metrics and statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    /// Total effect computations performed
    pub total_computations: u64,
    /// Cache hits vs total requests
    pub cache_hit_rate: f64,
    /// Average computation time in microseconds
    pub avg_computation_time_us: f64,
    /// Average HID communication time in microseconds
    pub avg_hid_time_us: f64,
    /// Number of HID operations batched
    pub hid_operations_batched: u64,
    /// Memory allocations saved through pooling
    pub allocations_saved: u64,
    /// Current memory pool utilization percentage
    pub memory_pool_utilization: f64,
    /// Adaptive refresh rate adjustments made
    pub refresh_rate_adjustments: u64,
    /// Current effective refresh rate (Hz)
    pub current_refresh_rate: f64,
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            total_computations: 0,
            cache_hit_rate: 0.0,
            avg_computation_time_us: 0.0,
            avg_hid_time_us: 0.0,
            hid_operations_batched: 0,
            allocations_saved: 0,
            memory_pool_utilization: 0.0,
            refresh_rate_adjustments: 0,
            current_refresh_rate: 60.0,
        }
    }
}

/// Memory pool for reusing RGB color vectors.
pub struct ColorVectorPool {
    /// Pool of pre-allocated color vectors
    pool: Arc<Mutex<Vec<Vec<Color>>>>,
    /// Statistics tracking
    stats: Arc<Mutex<PoolStats>>,
}

#[derive(Debug, Default)]
struct PoolStats {
    allocations_saved: u64,
    peak_utilization: usize,
    current_size: usize,
}

impl ColorVectorPool {
    /// Create a new color vector pool with initial capacity.
    pub fn new(initial_capacity: usize) -> Self {
        let mut pool = Vec::with_capacity(initial_capacity);

        // Pre-allocate some vectors of common sizes
        for size in [1, 9, 87, 104] {
            // Single key, zones, TKL keys, full keyboard keys
            for _ in 0..(initial_capacity / 4) {
                pool.push(vec![Color::BLACK; size]);
            }
        }

        let initial_size = pool.len();

        Self {
            pool: Arc::new(Mutex::new(pool)),
            stats: Arc::new(Mutex::new(PoolStats {
                current_size: initial_size,
                ..Default::default()
            })),
        }
    }

    /// Get a vector from the pool, or allocate a new one if needed.
    pub fn get(&self, size: usize) -> Vec<Color> {
        let mut pool = self.pool.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();

        // Try to find a vector of suitable size
        for i in 0..pool.len() {
            if pool[i].len() >= size {
                let mut vec = pool.swap_remove(i);
                vec.truncate(size);
                vec.fill(Color::BLACK); // Reset colors
                stats.allocations_saved += 1;
                stats.current_size = pool.len();
                return vec;
            }
        }

        // No suitable vector found, allocate new one
        stats.current_size = pool.len();
        vec![Color::BLACK; size]
    }

    /// Return a vector to the pool for reuse.
    pub fn return_vector(&self, mut vec: Vec<Color>) {
        let mut pool = self.pool.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();

        // Only keep reasonably sized vectors
        if vec.len() <= 256 && pool.len() < 50 {
            vec.clear();
            pool.push(vec);
            stats.current_size = pool.len();
            stats.peak_utilization = stats.peak_utilization.max(pool.len());
        }
    }

    /// Get pool utilization statistics.
    pub fn get_utilization(&self) -> f64 {
        let stats = self.stats.lock().unwrap();
        if stats.peak_utilization > 0 {
            stats.current_size as f64 / stats.peak_utilization as f64
        } else {
            0.0
        }
    }

    /// Get number of allocations saved.
    pub fn allocations_saved(&self) -> u64 {
        self.stats.lock().unwrap().allocations_saved
    }
}

/// Advanced effect computation cache with smart invalidation.
pub struct EffectComputationCache {
    /// Cache storage for computed effects
    cache: HashMap<EffectCacheKey, EffectCacheEntry>,
    /// Maximum cache size
    max_size: usize,
    /// Cache hit/miss statistics
    hits: u64,
    misses: u64,
    /// Time-based invalidation threshold
    max_age: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct EffectCacheKey {
    effect_hash: u64,
    time_bucket: u64, // Rounded to reduce cache fragmentation
    key_count: usize,
}

#[derive(Debug, Clone)]
struct EffectCacheEntry {
    colors: Vec<Color>,
    timestamp: Instant,
    #[allow(dead_code)]
    computation_time: Duration,
    access_count: u32,
}

/// Simple FNV-1a 64-bit hasher for performance.
struct FnvHasher {
    state: u64,
}

impl FnvHasher {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self {
            state: Self::OFFSET_BASIS,
        }
    }
}

impl std::hash::Hasher for FnvHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= byte as u64;
            self.state = self.state.wrapping_mul(Self::PRIME);
        }
    }

    fn finish(&self) -> u64 {
        self.state
    }
}

impl EffectComputationCache {
    /// Create a new effect computation cache.
    pub fn new(max_size: usize, max_age: Duration) -> Self {
        Self {
            cache: HashMap::with_capacity(max_size),
            max_size,
            hits: 0,
            misses: 0,
            max_age,
        }
    }

    /// Try to get cached effect computation.
    pub fn get(
        &mut self,
        effect: &PerKeyEffect,
        elapsed_time: Duration,
        key_count: usize,
    ) -> Option<Vec<Color>> {
        let key = EffectCacheKey {
            effect_hash: self.hash_effect(effect),
            time_bucket: self.time_bucket(elapsed_time),
            key_count,
        };

        if let Some(entry) = self.cache.get_mut(&key) {
            // Check if entry is still valid
            if entry.timestamp.elapsed() <= self.max_age {
                entry.access_count += 1;
                self.hits += 1;
                return Some(entry.colors.clone());
            } else {
                // Entry expired, remove it
                self.cache.remove(&key);
            }
        }

        self.misses += 1;
        None
    }

    /// Store computed effect in cache.
    pub fn put(
        &mut self,
        effect: &PerKeyEffect,
        elapsed_time: Duration,
        colors: Vec<Color>,
        computation_time: Duration,
    ) {
        // Enforce cache size limit
        if self.cache.len() >= self.max_size {
            self.evict_lru();
        }

        let key = EffectCacheKey {
            effect_hash: self.hash_effect(effect),
            time_bucket: self.time_bucket(elapsed_time),
            key_count: colors.len(),
        };

        let entry = EffectCacheEntry {
            colors,
            timestamp: Instant::now(),
            computation_time,
            access_count: 1,
        };

        self.cache.insert(key, entry);
    }

    /// Get cache hit rate.
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }

    /// Clear expired entries from cache.
    pub fn cleanup(&mut self) {
        let now = Instant::now();
        self.cache
            .retain(|_, entry| now.duration_since(entry.timestamp) <= self.max_age);
    }

    /// Hash an effect for caching.
    fn hash_effect(&self, effect: &PerKeyEffect) -> u64 {
        use std::hash::{Hash, Hasher};

        let mut hasher = FnvHasher::new();

        // Hash effect type and key parameters
        match effect {
            PerKeyEffect::Static { color } => {
                "static".hash(&mut hasher);
                color.to_hex().hash(&mut hasher);
            }
            PerKeyEffect::Breathing { color, speed } => {
                "breathing".hash(&mut hasher);
                color.to_hex().hash(&mut hasher);
                ((speed * 1000.0) as u32).hash(&mut hasher);
            }
            PerKeyEffect::Spectrum { speed } => {
                "spectrum".hash(&mut hasher);
                ((speed * 1000.0) as u32).hash(&mut hasher);
            }
            // Add more effect types as needed...
            _ => {
                "other".hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    /// Convert time to discrete bucket for cache key.
    fn time_bucket(&self, elapsed_time: Duration) -> u64 {
        // Round to 16ms buckets (60fps) to improve cache hit rate
        (elapsed_time.as_millis() / 16) as u64
    }

    /// Evict least recently used cache entry.
    fn evict_lru(&mut self) {
        if let Some((key_to_remove, _)) = self
            .cache
            .iter()
            .min_by_key(|(_, entry)| (entry.access_count, entry.timestamp))
        {
            let key_to_remove = key_to_remove.clone();
            self.cache.remove(&key_to_remove);
        }
    }
}

/// HID operation batching system to reduce communication overhead.
pub struct HidBatchProcessor {
    /// Pending operations to batch
    pending_operations: Vec<HidOperation>,
    /// Maximum batch size
    max_batch_size: usize,
    /// Maximum time to wait before processing batch
    max_batch_delay: Duration,
    /// Last batch processing time
    last_batch_time: Instant,
    /// Statistics
    operations_batched: u64,
}

#[derive(Debug, Clone)]
struct HidOperation {
    operation_type: HidOpType,
    data: Vec<u8>,
    timestamp: Instant,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum HidOpType {
    SetZoneColors,
    SetPerKeyColors,
    SetBrightness,
    Apply,
}

impl HidBatchProcessor {
    /// Create a new HID batch processor.
    pub fn new(max_batch_size: usize, max_batch_delay: Duration) -> Self {
        Self {
            pending_operations: Vec::with_capacity(max_batch_size),
            max_batch_size,
            max_batch_delay,
            last_batch_time: Instant::now(),
            operations_batched: 0,
        }
    }

    /// Add an operation to the batch.
    pub fn add_operation(&mut self, op_type: HidOpType, data: Vec<u8>) -> bool {
        let operation = HidOperation {
            operation_type: op_type,
            data,
            timestamp: Instant::now(),
        };

        self.pending_operations.push(operation);

        // Check if batch should be processed
        self.should_process_batch()
    }

    /// Check if the batch should be processed now.
    pub fn should_process_batch(&self) -> bool {
        self.pending_operations.len() >= self.max_batch_size
            || self.last_batch_time.elapsed() >= self.max_batch_delay
            || (!self.pending_operations.is_empty() && self.has_urgent_operation())
    }

    /// Process the current batch and return operations to execute.
    pub fn process_batch(&mut self) -> Vec<Vec<u8>> {
        if self.pending_operations.is_empty() {
            return Vec::new();
        }

        // Optimize the batch by removing redundant operations
        let optimized_ops = self.optimize_operations();

        self.operations_batched += optimized_ops.len() as u64;
        self.last_batch_time = Instant::now();
        self.pending_operations.clear();

        optimized_ops.into_iter().map(|op| op.data).collect()
    }

    /// Get number of operations that have been batched.
    pub fn operations_batched(&self) -> u64 {
        self.operations_batched
    }

    /// Check if any operation in the batch is urgent.
    fn has_urgent_operation(&self) -> bool {
        self.pending_operations.iter().any(|op| {
            matches!(op.operation_type, HidOpType::Apply)
                || op.timestamp.elapsed() > Duration::from_millis(50)
        })
    }

    /// Optimize operations by removing redundant ones.
    fn optimize_operations(&self) -> Vec<HidOperation> {
        let mut optimized = Vec::new();
        let mut last_per_key: Option<&HidOperation> = None;
        let mut last_zone: Option<&HidOperation> = None;
        let mut last_brightness: Option<&HidOperation> = None;
        let mut has_apply = false;

        // Process operations in chronological order
        for op in &self.pending_operations {
            match op.operation_type {
                HidOpType::SetPerKeyColors => last_per_key = Some(op),
                HidOpType::SetZoneColors => last_zone = Some(op),
                HidOpType::SetBrightness => last_brightness = Some(op),
                HidOpType::Apply => has_apply = true,
            }
        }

        // Add the most recent operation of each type
        if let Some(op) = last_brightness {
            optimized.push(op.clone());
        }
        if let Some(op) = last_zone {
            optimized.push(op.clone());
        }
        if let Some(op) = last_per_key {
            optimized.push(op.clone());
        }
        if has_apply {
            optimized.push(HidOperation {
                operation_type: HidOpType::Apply,
                data: vec![],
                timestamp: Instant::now(),
            });
        }

        optimized
    }
}

/// Adaptive refresh rate controller.
pub struct AdaptiveRefreshController {
    /// Current refresh rate in Hz
    current_rate: f64,
    /// Minimum allowed refresh rate
    min_rate: f64,
    /// Maximum allowed refresh rate
    max_rate: f64,
    /// Recent computation times
    computation_times: Vec<Duration>,
    /// Recent HID communication times
    hid_times: Vec<Duration>,
    /// Number of adjustments made
    adjustments: u64,
    /// Last adjustment time
    last_adjustment: Instant,
}

impl AdaptiveRefreshController {
    /// Create a new adaptive refresh controller.
    pub fn new(initial_rate: f64, min_rate: f64, max_rate: f64) -> Self {
        Self {
            current_rate: initial_rate,
            min_rate,
            max_rate,
            computation_times: Vec::with_capacity(10),
            hid_times: Vec::with_capacity(10),
            adjustments: 0,
            last_adjustment: Instant::now(),
        }
    }

    /// Record timing measurements and potentially adjust refresh rate.
    pub fn record_timing(&mut self, computation_time: Duration, hid_time: Duration) -> f64 {
        // Keep rolling window of recent times
        self.computation_times.push(computation_time);
        self.hid_times.push(hid_time);

        if self.computation_times.len() > 10 {
            self.computation_times.remove(0);
        }
        if self.hid_times.len() > 10 {
            self.hid_times.remove(0);
        }

        // Don't adjust too frequently
        if self.last_adjustment.elapsed() < Duration::from_millis(500) {
            return self.current_rate;
        }

        // Calculate average total cycle time
        if self.computation_times.len() >= 5 {
            let avg_computation = self.computation_times.iter().sum::<Duration>()
                / self.computation_times.len() as u32;
            let avg_hid = self.hid_times.iter().sum::<Duration>() / self.hid_times.len() as u32;
            let total_cycle_time = avg_computation + avg_hid;

            let ideal_rate = if total_cycle_time.as_millis() > 0 {
                1000.0 / total_cycle_time.as_millis() as f64
            } else {
                self.max_rate
            };

            let target_rate = ideal_rate * 0.8; // Leave 20% headroom
            let new_rate = target_rate.clamp(self.min_rate, self.max_rate);

            // Only adjust if the change is significant
            if (new_rate - self.current_rate).abs() > 5.0 {
                self.current_rate = new_rate;
                self.adjustments += 1;
                self.last_adjustment = Instant::now();
            }
        }

        self.current_rate
    }

    /// Get current refresh rate.
    pub fn current_rate(&self) -> f64 {
        self.current_rate
    }

    /// Get number of adjustments made.
    pub fn adjustments(&self) -> u64 {
        self.adjustments
    }

    /// Get target frame time for current refresh rate.
    pub fn frame_time(&self) -> Duration {
        Duration::from_millis((1000.0 / self.current_rate) as u64)
    }
}

/// Performance optimization manager that coordinates all optimization systems.
pub struct PerformanceManager {
    /// Memory pool for color vectors
    color_pool: ColorVectorPool,
    /// Effect computation cache
    effect_cache: EffectComputationCache,
    /// HID operation batching
    hid_batcher: HidBatchProcessor,
    /// Adaptive refresh rate control
    refresh_controller: AdaptiveRefreshController,
    /// Performance statistics
    stats: PerformanceStats,
}

impl PerformanceManager {
    /// Create a new performance manager with default settings.
    pub fn new() -> Self {
        Self {
            color_pool: ColorVectorPool::new(20),
            effect_cache: EffectComputationCache::new(100, Duration::from_secs(5)),
            hid_batcher: HidBatchProcessor::new(5, Duration::from_millis(16)),
            refresh_controller: AdaptiveRefreshController::new(60.0, 15.0, 120.0),
            stats: PerformanceStats::default(),
        }
    }

    /// Create a performance manager with custom settings.
    pub fn with_config(
        pool_size: usize,
        cache_size: usize,
        batch_size: usize,
        initial_refresh_rate: f64,
    ) -> Self {
        Self {
            color_pool: ColorVectorPool::new(pool_size),
            effect_cache: EffectComputationCache::new(cache_size, Duration::from_secs(5)),
            hid_batcher: HidBatchProcessor::new(batch_size, Duration::from_millis(16)),
            refresh_controller: AdaptiveRefreshController::new(initial_refresh_rate, 15.0, 120.0),
            stats: PerformanceStats::default(),
        }
    }

    /// Get a color vector from the memory pool.
    pub fn get_color_vector(&self, size: usize) -> Vec<Color> {
        self.color_pool.get(size)
    }

    /// Return a color vector to the memory pool.
    pub fn return_color_vector(&self, vec: Vec<Color>) {
        self.color_pool.return_vector(vec);
    }

    /// Try to get cached effect computation.
    pub fn get_cached_effect(
        &mut self,
        effect: &PerKeyEffect,
        elapsed_time: Duration,
        key_count: usize,
    ) -> Option<Vec<Color>> {
        self.effect_cache.get(effect, elapsed_time, key_count)
    }

    /// Store computed effect in cache.
    pub fn cache_effect(
        &mut self,
        effect: &PerKeyEffect,
        elapsed_time: Duration,
        colors: Vec<Color>,
        computation_time: Duration,
    ) {
        self.effect_cache
            .put(effect, elapsed_time, colors, computation_time);
    }

    /// Add HID operation to batch.
    pub fn add_hid_operation(&mut self, data: Vec<u8>) -> Option<Vec<Vec<u8>>> {
        if self
            .hid_batcher
            .add_operation(HidOpType::SetZoneColors, data)
        {
            Some(self.hid_batcher.process_batch())
        } else {
            None
        }
    }

    /// Force process HID batch.
    pub fn flush_hid_batch(&mut self) -> Vec<Vec<u8>> {
        self.hid_batcher.process_batch()
    }

    /// Record performance timing and adjust refresh rate.
    pub fn record_timing(&mut self, computation_time: Duration, hid_time: Duration) -> f64 {
        self.stats.total_computations += 1;

        // Update running averages
        let alpha = 0.1; // Smoothing factor
        let comp_us = computation_time.as_micros() as f64;
        let hid_us = hid_time.as_micros() as f64;

        self.stats.avg_computation_time_us =
            self.stats.avg_computation_time_us * (1.0 - alpha) + comp_us * alpha;
        self.stats.avg_hid_time_us = self.stats.avg_hid_time_us * (1.0 - alpha) + hid_us * alpha;

        // Update other stats
        self.stats.cache_hit_rate = self.effect_cache.hit_rate();
        self.stats.hid_operations_batched = self.hid_batcher.operations_batched();
        self.stats.allocations_saved = self.color_pool.allocations_saved();
        self.stats.memory_pool_utilization = self.color_pool.get_utilization();

        let new_rate = self
            .refresh_controller
            .record_timing(computation_time, hid_time);
        if new_rate != self.stats.current_refresh_rate {
            self.stats.refresh_rate_adjustments += 1;
            self.stats.current_refresh_rate = new_rate;
        }

        new_rate
    }

    /// Get current performance statistics.
    pub fn get_stats(&self) -> &PerformanceStats {
        &self.stats
    }

    /// Clean up expired cache entries.
    pub fn cleanup(&mut self) {
        self.effect_cache.cleanup();
    }

    /// Get optimal frame time for current refresh rate.
    pub fn frame_time(&self) -> Duration {
        self.refresh_controller.frame_time()
    }
}

impl Default for PerformanceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rgb::Color;

    #[test]
    fn test_color_vector_pool() {
        let pool = ColorVectorPool::new(10);

        // Get vectors of different sizes
        let vec1 = pool.get(5);
        let vec2 = pool.get(10);

        assert_eq!(vec1.len(), 5);
        assert_eq!(vec2.len(), 10);

        // Return them to pool
        pool.return_vector(vec1);
        pool.return_vector(vec2);

        // Get again - should reuse from pool
        let vec3 = pool.get(5);
        assert_eq!(vec3.len(), 5);

        // Check that allocations were saved
        assert!(pool.allocations_saved() > 0);
    }

    #[test]
    fn test_effect_cache() {
        let mut cache = EffectComputationCache::new(10, Duration::from_secs(1));

        let effect = PerKeyEffect::Static { color: Color::RED };
        let elapsed = Duration::from_millis(100);
        let colors = vec![Color::RED; 5];
        let comp_time = Duration::from_micros(500);

        // Should miss on first access
        assert!(cache.get(&effect, elapsed, 5).is_none());

        // Store in cache
        cache.put(&effect, elapsed, colors.clone(), comp_time);

        // Should hit on second access
        let cached_colors = cache.get(&effect, elapsed, 5);
        assert!(cached_colors.is_some());
        assert_eq!(cached_colors.unwrap().len(), 5);

        // Check hit rate
        assert!(cache.hit_rate() > 0.0);
    }

    #[test]
    fn test_hid_batch_processor() {
        let mut batcher = HidBatchProcessor::new(3, Duration::from_millis(100));

        // Add operations one by one
        assert!(!batcher.add_operation(HidOpType::SetZoneColors, vec![1, 2, 3]));
        assert!(!batcher.add_operation(HidOpType::SetBrightness, vec![4, 5, 6]));

        // Third operation should trigger batch processing
        assert!(batcher.add_operation(HidOpType::Apply, vec![]));

        let batch = batcher.process_batch();
        assert!(!batch.is_empty());
        assert!(batcher.operations_batched() > 0);
    }

    #[test]
    fn test_adaptive_refresh_controller() {
        let mut controller = AdaptiveRefreshController::new(60.0, 15.0, 120.0);

        assert_eq!(controller.current_rate(), 60.0);

        // Simulate fast operations
        for _ in 0..6 {
            controller.record_timing(Duration::from_micros(100), Duration::from_micros(200));
        }

        // Should have adjusted rate upward due to fast operations
        // (Need to wait for adjustment delay to pass)
        std::thread::sleep(Duration::from_millis(600));
        controller.record_timing(Duration::from_micros(100), Duration::from_micros(200));

        assert!(controller.current_rate() >= 60.0);
    }

    #[test]
    fn test_performance_manager() {
        let mut manager = PerformanceManager::new();

        // Test color vector pooling
        let vec = manager.get_color_vector(10);
        assert_eq!(vec.len(), 10);
        manager.return_color_vector(vec);

        // Test effect caching
        let effect = PerKeyEffect::Static { color: Color::BLUE };
        let elapsed = Duration::from_millis(200);

        assert!(manager.get_cached_effect(&effect, elapsed, 5).is_none());

        let colors = vec![Color::BLUE; 5];
        manager.cache_effect(&effect, elapsed, colors.clone(), Duration::from_micros(300));

        assert!(manager.get_cached_effect(&effect, elapsed, 5).is_some());

        // Test timing recording
        let rate = manager.record_timing(Duration::from_micros(500), Duration::from_millis(1));
        assert!(rate > 0.0);

        let stats = manager.get_stats();
        assert!(stats.total_computations > 0);
    }

    #[test]
    fn test_performance_stats_defaults() {
        let stats = PerformanceStats::default();
        assert_eq!(stats.total_computations, 0);
        assert_eq!(stats.cache_hit_rate, 0.0);
        assert_eq!(stats.current_refresh_rate, 60.0);
    }
}
