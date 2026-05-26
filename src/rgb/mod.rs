//! RGB lighting control and effects.

use crate::devices::key_mapping::{KeyId, KeyMapping};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// RGB color representation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    /// Create a new color from RGB values.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Create color from hex value (0xRRGGBB).
    pub const fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
        }
    }

    /// Convert to hex value.
    pub const fn to_hex(&self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Black (off).
    pub const BLACK: Color = Color::new(0, 0, 0);
    /// White.
    pub const WHITE: Color = Color::new(255, 255, 255);
    /// Red.
    pub const RED: Color = Color::new(255, 0, 0);
    /// Green.
    pub const GREEN: Color = Color::new(0, 255, 0);
    /// Blue.
    pub const BLUE: Color = Color::new(0, 0, 255);
    /// Cyan.
    pub const CYAN: Color = Color::new(0, 255, 255);
    /// Magenta.
    pub const MAGENTA: Color = Color::new(255, 0, 255);
    /// Yellow.
    pub const YELLOW: Color = Color::new(255, 255, 0);
    /// Orange.
    pub const ORANGE: Color = Color::new(255, 128, 0);
    /// Purple.
    pub const PURPLE: Color = Color::new(128, 0, 255);
    /// Pink.
    pub const PINK: Color = Color::new(255, 105, 180);

    /// Default 7-color sequence for wave effects.
    pub const DEFAULT_COLORS: [Color; 7] = [
        Color::RED,
        Color::ORANGE,
        Color::YELLOW,
        Color::GREEN,
        Color::CYAN,
        Color::BLUE,
        Color::PURPLE,
    ];

    /// Standard 8-color rainbow sequence.
    pub const RAINBOW_COLORS: [Color; 8] = [
        Color::RED,
        Color::ORANGE,
        Color::YELLOW,
        Color::GREEN,
        Color::CYAN,
        Color::BLUE,
        Color::PURPLE,
        Color::MAGENTA,
    ];

    /// Blend between two colors.
    #[inline]
    pub fn blend(a: Color, b: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let inv_t = 1.0 - t; // Pre-calculate inverse

        // Use more efficient blending with fewer operations
        Color {
            r: (a.r as f32 * inv_t + b.r as f32 * t) as u8,
            g: (a.g as f32 * inv_t + b.g as f32 * t) as u8,
            b: (a.b as f32 * inv_t + b.b as f32 * t) as u8,
        }
    }

    /// Scale brightness (0.0 = black, 1.0 = original).
    #[inline]
    pub fn scale(&self, factor: f32) -> Color {
        let factor = factor.clamp(0.0, 1.0);
        Color {
            r: (self.r as f32 * factor) as u8,
            g: (self.g as f32 * factor) as u8,
            b: (self.b as f32 * factor) as u8,
        }
    }

    /// Convert from HSV to RGB.
    /// Optimized version with reduced branching and fewer operations.
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Color {
        let h = h % 360.0;
        let s = s.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        let c = v * s;
        let h_sector = h / 60.0;
        let x = c * (1.0 - (h_sector % 2.0 - 1.0).abs());
        let m = v - c;

        // Use integer sector for faster branching
        let sector = h_sector as i32;
        let (r, g, b) = match sector {
            0 => (c, x, 0.0), // 0-60
            1 => (x, c, 0.0), // 60-120
            2 => (0.0, c, x), // 120-180
            3 => (0.0, x, c), // 180-240
            4 => (x, 0.0, c), // 240-300
            _ => (c, 0.0, x), // 300-360
        };

        // Single multiplication by 255.0 instead of per-component
        let scale = 255.0;
        Color {
            r: ((r + m) * scale) as u8,
            g: ((g + m) * scale) as u8,
            b: ((b + m) * scale) as u8,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

/// RGB lighting effect types.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum Effect {
    /// Static single color.
    Static { color: Color },

    /// Breathing effect (pulse).
    Breathing {
        color: Color,
        speed: f32, // Cycles per second
    },

    /// Color cycle through spectrum.
    Spectrum {
        speed: f32, // Cycles per second
    },

    /// Wave effect across zones.
    Wave {
        colors: Vec<Color>,
        speed: f32,
        direction: WaveDirection,
    },

    /// Reactive effect (responds to key presses).
    Reactive {
        color: Color,
        duration: f32, // Seconds
    },

    /// Gradient between two colors.
    Gradient { start: Color, end: Color },

    /// Custom per-zone colors.
    Custom { colors: Vec<Color> },

    /// Disabled (all LEDs off).
    Off,
}

/// Wave effect direction.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum WaveDirection {
    LeftToRight,
    RightToLeft,
    CenterOut,
    OutCenter,
}

impl Default for Effect {
    fn default() -> Self {
        Effect::Static { color: Color::WHITE }
    }
}

/// Per-key RGB lighting effect types.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PerKeyEffect {
    /// Static single color for all keys.
    Static { color: Color },

    /// Per-key breathing effect.
    Breathing {
        color: Color,
        speed: f32, // Cycles per second
    },

    /// Color cycle through spectrum (all keys synchronized).
    Spectrum {
        speed: f32, // Cycles per second
    },

    /// Wave effect across keyboard rows/columns.
    Wave {
        colors: Vec<Color>,
        speed: f32,
        direction: KeyWaveDirection,
    },

    /// Ripple effect from center or specific keys.
    Ripple {
        color: Color,
        speed: f32,
        center_keys: Vec<KeyId>, // Keys to start ripple from
    },

    /// Reactive effect (responds to specific key presses).
    Reactive {
        base_color: Color,
        highlight_color: Color,
        duration: f32,           // Seconds
        active_keys: Vec<KeyId>, // Currently highlighted keys
    },

    /// Gradient across keyboard layout.
    Gradient {
        start: Color,
        end: Color,
        direction: GradientDirection,
    },

    /// Custom per-key colors.
    Custom { key_colors: HashMap<KeyId, Color> },

    /// Gaming-specific zones (WASD, arrow keys, etc.).
    GameZone {
        wasd_color: Color,
        arrow_keys_color: Color,
        function_keys_color: Color,
        number_row_color: Color,
        default_color: Color,
    },

    /// Disabled (all keys off).
    Off,
}

/// Wave direction for per-key effects.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum KeyWaveDirection {
    LeftToRight,
    RightToLeft,
    TopToBottom,
    BottomToTop,
    CenterOut,
    OutCenter,
    Diagonal,
}

/// Gradient direction for per-key effects.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum GradientDirection {
    Horizontal, // Left to right
    Vertical,   // Top to bottom
    Diagonal,   // Top-left to bottom-right
    Radial,     // Center outward
}

/// Color blending mode for row/column effects.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum BlendMode {
    Add,      // Add colors together (clamped)
    Multiply, // Multiply colors
    Average,  // Average the colors
    Overlay,  // Row color overlays column
}

impl Default for PerKeyEffect {
    fn default() -> Self {
        PerKeyEffect::Static { color: Color::WHITE }
    }
}

/// Timing mode for effect rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TimingMode {
    /// Fixed timing interval (legacy mode)
    Fixed,
    /// Adaptive timing based on effect complexity and system performance
    #[default]
    Adaptive,
    /// High performance mode with optimizations for consistent timing
    HighPerformance,
}

/// RGB effect engine that computes colors over time with adaptive timing support.
pub struct EffectEngine {
    effect: Effect,
    start_time: Instant,
    zone_count: usize,
    /// Cached colors from last computation
    cached_colors: Vec<Color>,
    /// Last time colors were computed
    last_compute_time: Duration,
    /// Timing mode for adaptive performance
    timing_mode: TimingMode,
    /// Dynamic cache threshold based on effect complexity
    cache_threshold: Duration,
    /// Base cache threshold (16ms = ~60 FPS)
    base_cache_threshold: Duration,
}

impl EffectEngine {
    /// Create a new effect engine with default adaptive timing.
    pub fn new(effect: Effect, zone_count: usize) -> Self {
        let mut engine = Self {
            effect,
            start_time: Instant::now(),
            zone_count,
            cached_colors: Vec::with_capacity(zone_count),
            last_compute_time: Duration::ZERO,
            timing_mode: TimingMode::default(),
            cache_threshold: Duration::from_millis(16), // Base threshold
            base_cache_threshold: Duration::from_millis(16),
        };

        // Calculate initial frame budget based on effect complexity
        engine.update_cache_threshold();
        engine
    }

    /// Create a new effect engine with specific timing mode.
    pub fn with_timing_mode(effect: Effect, zone_count: usize, timing_mode: TimingMode) -> Self {
        let mut engine = Self {
            effect,
            start_time: Instant::now(),
            zone_count,
            cached_colors: vec![Color::BLACK; zone_count],
            last_compute_time: Duration::ZERO,
            timing_mode,
            cache_threshold: Duration::from_millis(16),
            base_cache_threshold: Duration::from_millis(16),
        };

        engine.update_cache_threshold();
        engine
    }

    /// Set a new effect and update timing accordingly.
    pub fn set_effect(&mut self, effect: Effect) {
        self.effect = effect;
        self.start_time = Instant::now();
        self.last_compute_time = Duration::ZERO; // Force recompute on next call
        self.update_cache_threshold(); // Recalculate timing based on new effect
    }

    /// Set the timing mode.
    pub fn set_timing_mode(&mut self, mode: TimingMode) {
        self.timing_mode = mode;
        self.update_cache_threshold();
    }

    /// Get the current timing mode.
    pub fn timing_mode(&self) -> TimingMode {
        self.timing_mode
    }

    /// Calculate frame budget based on effect complexity and timing mode.
    pub fn calculate_frame_budget(&self) -> Duration {
        match self.timing_mode {
            TimingMode::Fixed => self.base_cache_threshold,
            TimingMode::Adaptive => {
                let complexity = self.calculate_effect_complexity();
                match complexity {
                    crate::performance::EffectComplexity::Simple => Duration::from_millis(33), // 30 FPS
                    crate::performance::EffectComplexity::Medium => Duration::from_millis(16), // 60 FPS
                    crate::performance::EffectComplexity::High => Duration::from_millis(8),    // 120 FPS
                    crate::performance::EffectComplexity::Critical => Duration::from_millis(4), // 240 FPS
                }
            }
            TimingMode::HighPerformance => {
                // Aggressive timing for consistent performance
                let complexity = self.calculate_effect_complexity();
                match complexity {
                    crate::performance::EffectComplexity::Simple => Duration::from_millis(12), // 83 FPS
                    crate::performance::EffectComplexity::Medium => Duration::from_millis(8),  // 120 FPS
                    crate::performance::EffectComplexity::High => Duration::from_millis(4),    // 240 FPS
                    crate::performance::EffectComplexity::Critical => Duration::from_millis(2), // 500 FPS
                }
            }
        }
    }

    /// Calculate effect complexity for the current effect.
    fn calculate_effect_complexity(&self) -> crate::performance::EffectComplexity {
        crate::performance::calculate_effect_complexity(&self.effect)
    }

    /// Update cache threshold based on current effect and timing mode.
    fn update_cache_threshold(&mut self) {
        self.cache_threshold = self.calculate_frame_budget();
    }

    /// Get the current effect.
    pub fn effect(&self) -> &Effect {
        &self.effect
    }

    /// Compute current colors for all zones.
    /// Uses caching to avoid unnecessary recomputation.
    #[inline]
    pub fn compute(&mut self) -> &[Color] {
        let elapsed = self.start_time.elapsed();

        // Return cached colors if delta is below threshold (but always compute on first call)
        if self.last_compute_time != Duration::ZERO
            && elapsed.saturating_sub(self.last_compute_time) < self.cache_threshold
        {
            return &self.cached_colors;
        }

        let elapsed_secs = elapsed.as_secs_f32();

        // Reuse cached vector to avoid allocations - use truncate instead of clear + resize
        self.cached_colors.truncate(0);

        match &self.effect {
            Effect::Static { color } => {
                // Use extend with repeat iterator for better performance than resize
                self.cached_colors.extend(std::iter::repeat_n(*color, self.zone_count));
            }

            Effect::Breathing { color, speed } => {
                // Sine wave for breathing effect - use optimized sin approximation for better performance
                let t = (elapsed_secs * speed * 2.0 * std::f32::consts::PI).sin();
                let brightness = (t + 1.0) * 0.5; // Use multiplication instead of division
                let scaled_color = color.scale(brightness);
                self.cached_colors
                    .extend(std::iter::repeat_n(scaled_color, self.zone_count));
            }

            Effect::Spectrum { speed } => {
                // Cycle through hue
                let hue = (elapsed_secs * speed * 360.0) % 360.0;
                let color = Color::from_hsv(hue, 1.0, 1.0);
                self.cached_colors.extend(std::iter::repeat_n(color, self.zone_count));
            }

            Effect::Wave {
                colors,
                speed,
                direction,
            } => {
                if colors.is_empty() {
                    self.cached_colors
                        .extend(std::iter::repeat_n(Color::BLACK, self.zone_count));
                } else {
                    let phase = elapsed_secs * speed;
                    let zone_count_f32 = self.zone_count as f32;
                    let colors_len_f32 = colors.len() as f32;
                    let center = zone_count_f32 * 0.5; // Pre-calculate center

                    // Pre-allocate capacity to avoid reallocations
                    self.cached_colors.reserve(self.zone_count);

                    for i in 0..self.zone_count {
                        let i_f32 = i as f32;
                        let zone_offset = match direction {
                            WaveDirection::LeftToRight => i_f32 / zone_count_f32,
                            WaveDirection::RightToLeft => 1.0 - (i_f32 / zone_count_f32),
                            WaveDirection::CenterOut => (i_f32 - center).abs() / center,
                            WaveDirection::OutCenter => 1.0 - (i_f32 - center).abs() / center,
                        };

                        let t = (phase + zone_offset) % 1.0;
                        let color_pos = t * colors_len_f32;
                        let color_index = color_pos as usize % colors.len();
                        let next_index = (color_index + 1) % colors.len();
                        let blend_t = color_pos % 1.0;

                        // Safe indexing - indices are guaranteed valid by modulo operations
                        self.cached_colors
                            .push(Color::blend(colors[color_index], colors[next_index], blend_t));
                    }
                }
            }

            Effect::Reactive { color, .. } => {
                // Base state - actual reactivity handled by input events
                let scaled_color = color.scale(0.2);
                self.cached_colors
                    .extend(std::iter::repeat_n(scaled_color, self.zone_count));
            }

            Effect::Gradient { start, end } => {
                // Pre-allocate capacity and calculate division once
                self.cached_colors.reserve(self.zone_count);
                let divisor = ((self.zone_count - 1).max(1)) as f32;

                for i in 0..self.zone_count {
                    let t = i as f32 / divisor;
                    self.cached_colors.push(Color::blend(*start, *end, t));
                }
            }

            Effect::Custom { colors } => {
                let copy_len = colors.len().min(self.zone_count);
                self.cached_colors.extend_from_slice(&colors[..copy_len]);
                if self.zone_count > copy_len {
                    let remaining = self.zone_count - copy_len;
                    self.cached_colors.extend(std::iter::repeat_n(Color::BLACK, remaining));
                }
            }

            Effect::Off => {
                self.cached_colors
                    .extend(std::iter::repeat_n(Color::BLACK, self.zone_count));
            }
        }

        // Update last compute time
        self.last_compute_time = elapsed;

        &self.cached_colors
    }

    /// Reset the effect timer.
    pub fn reset(&mut self) {
        self.start_time = Instant::now();
    }
}

/// Per-key RGB effect engine that computes colors for individual keys.
pub struct PerKeyEffectEngine {
    effect: PerKeyEffect,
    start_time: Instant,
    key_mapping: KeyMapping,
    /// Cached colors from last computation
    cached_key_colors: HashMap<KeyId, Color>,
    /// Last time colors were computed
    last_compute_time: Duration,
    /// Minimum time between recomputes (16ms = ~60 FPS)
    cache_threshold: Duration,
    /// Reactive effect state
    reactive_state: HashMap<KeyId, f32>, // Key -> time remaining
}

impl PerKeyEffectEngine {
    /// Get the current cached colors map.
    pub fn get_cached_colors(&self) -> &HashMap<KeyId, Color> {
        &self.cached_key_colors
    }

    /// Create a new per-key effect engine.
    pub fn new(effect: PerKeyEffect, key_mapping: KeyMapping) -> Self {
        Self {
            effect,
            start_time: Instant::now(),
            key_mapping,
            cached_key_colors: HashMap::with_capacity(128),
            last_compute_time: Duration::ZERO,
            cache_threshold: Duration::from_millis(16), // ~60 FPS
            reactive_state: HashMap::with_capacity(32),
        }
    }

    /// Set a new effect.
    pub fn set_effect(&mut self, effect: PerKeyEffect) {
        self.effect = effect;
        self.start_time = Instant::now();
        self.last_compute_time = Duration::ZERO; // Force recompute on next call
    }

    /// Get the current effect.
    pub fn effect(&self) -> &PerKeyEffect {
        &self.effect
    }

    /// Trigger reactive effect for specific keys.
    pub fn trigger_reactive(&mut self, keys: &[KeyId], duration: f32) {
        for &key in keys {
            self.reactive_state.insert(key, duration);
        }
    }

    /// Update reactive effect state.
    fn update_reactive_state(&mut self, delta_time: f32) {
        self.reactive_state.retain(|_, time_remaining| {
            *time_remaining -= delta_time;
            *time_remaining > 0.0
        });
    }

    /// Static helper to get keyboard center without borrowing self
    fn get_keyboard_center_static() -> (f32, f32) {
        (0.5, 0.5) // Center of normalized coordinate space
    }

    /// Static helper to get key position from HID code.
    /// Returns normalized coordinates (0.0-1.0) based on HID code.
    fn get_key_position_static(hid_code: u8) -> (f32, f32) {
        // Map HID code to approximate keyboard position
        let row = match hid_code {
            0x04..=0x1D => 0, // A-Z keys (top letter row)
            0x1E..=0x27 => 1, // Number row
            0x28..=0x2F => 2, // Enter, shift row
            0x30..=0x35 => 3, // Bottom row (space, etc.)
            0x36..=0x3F => 4, // Function keys area
            _ => 2,           // Default to middle
        };
        let col = (hid_code % 10) as f32 / 10.0;
        (col, row as f32 / 5.0)
    }

    /// Static helper to calculate distance between two keys by their HID codes.
    fn key_distance_static(hid1: u8, hid2: u8) -> f32 {
        let (x1, y1) = Self::get_key_position_static(hid1);
        let (x2, y2) = Self::get_key_position_static(hid2);
        ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt()
    }

    /// Compute colors for all keys.
    #[inline]
    pub fn compute(&mut self) -> &HashMap<KeyId, Color> {
        let elapsed = self.start_time.elapsed();

        // Return cached colors if delta is below threshold (but always compute on first call)
        if self.last_compute_time != Duration::ZERO
            && elapsed.saturating_sub(self.last_compute_time) < self.cache_threshold
        {
            return &self.cached_key_colors;
        }

        let elapsed_secs = elapsed.as_secs_f32();
        let delta_time = if self.last_compute_time == Duration::ZERO {
            0.0
        } else {
            elapsed.saturating_sub(self.last_compute_time).as_secs_f32()
        };

        // Update reactive state
        self.update_reactive_state(delta_time);

        // Optimization: Handle both population (first run) and updates (subsequent runs)
        // cleanly without clearing the map or duplicating logic.
        if self.cached_key_colors.is_empty() {
            // First run: Iterate all keys from mapping and populate
            let keys = self.key_mapping.get_all_keys();
            let mut new_colors = HashMap::with_capacity(keys.len());

            if let PerKeyEffect::Static { color } = self.effect {
                for key in keys {
                    new_colors.insert(*key, color);
                }
            } else {
                for key in keys {
                    let mut color = Color::BLACK;
                    Self::apply_effect_static(
                        &self.effect,
                        *key,
                        &mut color,
                        &self.key_mapping,
                        &self.reactive_state,
                        elapsed_secs,
                    );
                    new_colors.insert(*key, color);
                }
            }
            self.cached_key_colors = new_colors;
        } else {
            // Fast path: Update existing entries in place
            // NOTE: This assumes the key set doesn't change, which is true for a fixed keyboard layout
            for (key, color) in self.cached_key_colors.iter_mut() {
                Self::apply_effect_static(
                    &self.effect,
                    *key,
                    color,
                    &self.key_mapping,
                    &self.reactive_state,
                    elapsed_secs,
                );
            }
        }

        // Update last compute time
        self.last_compute_time = elapsed;

        &self.cached_key_colors
    }

    /// Static helper to apply effect to a single key.
    /// Used by both initial population and update loops to avoid code duplication.
    fn apply_effect_static(
        effect: &PerKeyEffect,
        key: KeyId,
        color: &mut Color,
        key_mapping: &KeyMapping,
        reactive_state: &HashMap<KeyId, f32>,
        elapsed_secs: f32,
    ) {
        match effect {
            PerKeyEffect::Static { color: c } => *color = *c,

            PerKeyEffect::Breathing { color: c, speed } => {
                let t = (elapsed_secs * speed * 2.0 * std::f32::consts::PI).sin();
                let brightness = (t + 1.0) * 0.5;
                *color = c.scale(brightness);
            }

            PerKeyEffect::Spectrum { speed } => {
                let hue = (elapsed_secs * speed * 360.0) % 360.0;
                *color = Color::from_hsv(hue, 1.0, 1.0);
            }

            PerKeyEffect::Wave {
                colors,
                speed,
                direction,
            } => {
                if colors.is_empty() {
                    *color = Color::BLACK;
                } else if let Some(address) = key_mapping.get_key_address(key) {
                    let (x, y) = Self::get_key_position_static(address.hid_code);
                    let wave_pos = match direction {
                        KeyWaveDirection::LeftToRight => x,
                        KeyWaveDirection::RightToLeft => 1.0 - x,
                        KeyWaveDirection::TopToBottom => y,
                        KeyWaveDirection::BottomToTop => 1.0 - y,
                        KeyWaveDirection::Diagonal => (x + y) * 0.5,
                        KeyWaveDirection::CenterOut => {
                            let (cx, cy) = Self::get_keyboard_center_static();
                            ((x - cx).powi(2) + (y - cy).powi(2)).sqrt()
                        }
                        KeyWaveDirection::OutCenter => {
                            let (cx, cy) = Self::get_keyboard_center_static();
                            1.0 - ((x - cx).powi(2) + (y - cy).powi(2)).sqrt()
                        }
                    };

                    let phase = elapsed_secs * speed;
                    let t = (phase + wave_pos) % 1.0;
                    let color_pos = t * colors.len() as f32;
                    let color_index = color_pos as usize % colors.len();
                    let next_index = (color_index + 1) % colors.len();
                    let blend_t = color_pos % 1.0;

                    *color = Color::blend(colors[color_index], colors[next_index], blend_t);
                } else {
                    *color = Color::BLACK;
                }
            }

            PerKeyEffect::Ripple {
                color: c,
                speed,
                center_keys,
            } => {
                if center_keys.is_empty() {
                    if let Some(address) = key_mapping.get_key_address(key) {
                        let (x, y) = Self::get_key_position_static(address.hid_code);
                        let (cx, cy) = Self::get_keyboard_center_static();
                        let distance = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                        let ripple_pos = (elapsed_secs * speed - distance * 3.0).rem_euclid(2.0);
                        let brightness = if ripple_pos > 0.0 && ripple_pos < 1.0 {
                            (ripple_pos * std::f32::consts::PI).sin()
                        } else {
                            0.0
                        };
                        *color = c.scale(brightness);
                    } else {
                        *color = Color::BLACK;
                    }
                } else {
                    let mut max_brightness = 0.0f32;
                    let key_hid = key_mapping.get_key_address(key).map(|a| a.hid_code).unwrap_or(0);
                    for &center_key in center_keys {
                        let center_hid = key_mapping.get_key_address(center_key).map(|a| a.hid_code).unwrap_or(0);
                        let distance = Self::key_distance_static(key_hid, center_hid);
                        if distance != f32::INFINITY {
                            let ripple_pos = (elapsed_secs * speed - distance * 5.0).rem_euclid(2.0);
                            let brightness = if ripple_pos > 0.0 && ripple_pos < 1.0 {
                                (ripple_pos * std::f32::consts::PI).sin()
                            } else {
                                0.0
                            };
                            max_brightness = max_brightness.max(brightness);
                        }
                    }
                    *color = c.scale(max_brightness);
                }
            }

            PerKeyEffect::Reactive {
                base_color,
                highlight_color,
                duration: _,
                active_keys,
            } => {
                *color = if active_keys.contains(&key) || reactive_state.contains_key(&key) {
                    *highlight_color
                } else {
                    *base_color
                };
            }

            PerKeyEffect::Gradient { start, end, direction } => {
                if let Some(address) = key_mapping.get_key_address(key) {
                    let (x, y) = Self::get_key_position_static(address.hid_code);
                    let t = match direction {
                        GradientDirection::Horizontal => x,
                        GradientDirection::Vertical => y,
                        GradientDirection::Diagonal => (x + y) * 0.5,
                        GradientDirection::Radial => {
                            let (cx, cy) = Self::get_keyboard_center_static();
                            ((x - cx).powi(2) + (y - cy).powi(2)).sqrt().min(1.0)
                        }
                    };
                    *color = Color::blend(*start, *end, t);
                } else {
                    *color = Color::BLACK;
                }
            }

            PerKeyEffect::Custom { key_colors } => {
                *color = key_colors.get(&key).copied().unwrap_or(Color::BLACK);
            }

            PerKeyEffect::GameZone {
                wasd_color,
                arrow_keys_color,
                function_keys_color,
                number_row_color,
                default_color,
            } => {
                *color = match key {
                    // WASD cluster
                    KeyId::W | KeyId::A | KeyId::S | KeyId::D => *wasd_color,

                    // Arrow keys
                    KeyId::ArrowUp | KeyId::ArrowDown | KeyId::ArrowLeft | KeyId::ArrowRight => *arrow_keys_color,

                    // Function keys
                    KeyId::F1
                    | KeyId::F2
                    | KeyId::F3
                    | KeyId::F4
                    | KeyId::F5
                    | KeyId::F6
                    | KeyId::F7
                    | KeyId::F8
                    | KeyId::F9
                    | KeyId::F10
                    | KeyId::F11
                    | KeyId::F12 => *function_keys_color,

                    // Number row
                    KeyId::Key1
                    | KeyId::Key2
                    | KeyId::Key3
                    | KeyId::Key4
                    | KeyId::Key5
                    | KeyId::Key6
                    | KeyId::Key7
                    | KeyId::Key8
                    | KeyId::Key9
                    | KeyId::Key0 => *number_row_color,

                    // Default for all other keys
                    _ => *default_color,
                };
            }

            PerKeyEffect::Off => {
                *color = Color::BLACK;
            }
        }
    }

    /// Reset the effect timer.
    pub fn reset(&mut self) {
        self.start_time = Instant::now();
    }

    /// Get current colors as a vector of (KeyId, Color) pairs.
    pub fn get_key_colors(&mut self) -> Vec<(KeyId, Color)> {
        let colors = self.compute();
        colors.iter().map(|(&key, &color)| (key, color)).collect()
    }

    /// Get color for a specific key.
    pub fn get_key_color(&mut self, key: KeyId) -> Color {
        let colors = self.compute();
        colors.get(&key).copied().unwrap_or(Color::BLACK)
    }
}

/// RGB controller for managing device lighting.
pub struct RgbController {
    engine: EffectEngine,
    brightness: f32,
    scaled_colors: Vec<Color>,
}

/// Per-key RGB controller for managing individual key lighting.
pub struct PerKeyRgbController {
    engine: PerKeyEffectEngine,
    brightness: f32,
    performance_manager: Option<crate::performance::PerformanceManager>,
    /// Buffer for scaled colors to avoid reallocations
    scaled_key_colors: Vec<(KeyId, Color)>,
}

impl PerKeyRgbController {
    /// Create a new per-key RGB controller.
    pub fn new(key_mapping: KeyMapping) -> Self {
        let key_count = key_mapping.get_all_keys().len();
        Self {
            engine: PerKeyEffectEngine::new(PerKeyEffect::default(), key_mapping),
            brightness: 1.0,
            performance_manager: None,
            scaled_key_colors: Vec::with_capacity(key_count),
        }
    }

    /// Create a new per-key RGB controller with performance optimization enabled.
    pub fn new_with_performance(key_mapping: KeyMapping) -> Self {
        let key_count = key_mapping.get_all_keys().len();
        Self {
            engine: PerKeyEffectEngine::new(PerKeyEffect::default(), key_mapping),
            brightness: 1.0,
            performance_manager: Some(crate::performance::PerformanceManager::new()),
            scaled_key_colors: Vec::with_capacity(key_count),
        }
    }

    /// Enable performance optimizations.
    pub fn enable_performance_optimization(&mut self) {
        if self.performance_manager.is_none() {
            self.performance_manager = Some(crate::performance::PerformanceManager::new());
        }
    }

    /// Disable performance optimizations.
    pub fn disable_performance_optimization(&mut self) {
        self.performance_manager = None;
    }

    /// Set the lighting effect.
    pub fn set_effect(&mut self, effect: PerKeyEffect) {
        self.engine.set_effect(effect);
    }

    /// Set brightness (0.0 to 1.0).
    pub fn set_brightness(&mut self, brightness: f32) {
        self.brightness = brightness.clamp(0.0, 1.0);
    }

    /// Get current brightness.
    pub fn brightness(&self) -> f32 {
        self.brightness
    }

    /// Trigger reactive effect for specific keys.
    pub fn trigger_reactive(&mut self, keys: &[KeyId], duration: f32) {
        self.engine.trigger_reactive(keys, duration);
    }

    /// Compute current colors for all keys with brightness applied.
    #[inline]
    pub fn compute_key_colors(&mut self) -> &[(KeyId, Color)] {
        let start_time = std::time::Instant::now();

        // Split borrows to avoid cloning the HashMap
        let engine = &mut self.engine;
        let perf_mgr_opt = &mut self.performance_manager;
        let scaled_colors = &mut self.scaled_key_colors;
        let brightness = self.brightness;

        // Reset buffer
        scaled_colors.clear();

        // Helper to apply brightness
        let apply_brightness = |color: Color, b: f32| -> Color {
            if (b - 1.0).abs() < f32::EPSILON {
                color
            } else {
                color.scale(b)
            }
        };

        if let Some(perf_mgr) = perf_mgr_opt {
            let elapsed = engine.start_time.elapsed();
            // Get key count without holding a borrow to keys slice
            let key_count = engine.key_mapping.total_keys;

            // Try cache first
            if let Some(cached_colors) = perf_mgr.get_cached_effect(engine.effect(), elapsed, key_count) {
                // Cache hit - populate directly from cache
                // keys and cached_colors should correspond 1:1 if cached_keys is stable
                let keys = engine.key_mapping.get_all_keys();
                for (i, &key) in keys.iter().enumerate() {
                    if i < cached_colors.len() {
                        let color = cached_colors[i];
                        scaled_colors.push((key, apply_brightness(color, brightness)));
                    }
                }
            } else {
                // Cache miss - compute and cache

                // 1. Force computation/update of internal cache
                // We ignore the return value to drop the mutable borrow immediately
                engine.compute();

                let computation_time = start_time.elapsed();

                // 2. Now safe to borrow keys and cache immutably together
                let keys = engine.key_mapping.get_all_keys();
                let computed_map = engine.get_cached_colors(); // New getter

                let mut colors_for_cache = Vec::with_capacity(keys.len());

                for &key in keys {
                    let color = computed_map.get(&key).copied().unwrap_or(Color::BLACK);
                    colors_for_cache.push(color);
                    scaled_colors.push((key, apply_brightness(color, brightness)));
                }

                // Cache the result
                perf_mgr.cache_effect(engine.effect(), elapsed, colors_for_cache, computation_time);

                // Record timing
                perf_mgr.record_timing(computation_time, Duration::from_micros(0));
            }
        } else {
            // No performance optimizations

            // 1. Force computation
            engine.compute();

            // 2. Borrow immutably
            let computed_map = engine.get_cached_colors();
            let keys = engine.key_mapping.get_all_keys();

            for &key in keys {
                let color = computed_map.get(&key).copied().unwrap_or(Color::BLACK);
                scaled_colors.push((key, apply_brightness(color, brightness)));
            }
        }

        scaled_colors
    }

    /// Get color for a specific key with brightness applied.
    pub fn get_key_color(&mut self, key: KeyId) -> Color {
        let color = self.engine.get_key_color(key);
        if (self.brightness - 1.0).abs() < f32::EPSILON {
            color
        } else {
            color.scale(self.brightness)
        }
    }

    /// Reset the effect timer.
    pub fn reset(&mut self) {
        self.engine.reset();
    }

    /// Get the current effect.
    pub fn effect(&self) -> &PerKeyEffect {
        self.engine.effect()
    }

    /// Get current performance statistics (if performance optimization is enabled).
    pub fn get_performance_stats(&self) -> Option<&crate::performance::PerformanceStats> {
        self.performance_manager.as_ref().map(|pm| pm.get_stats())
    }

    /// Get optimal frame time for current adaptive refresh rate.
    pub fn get_frame_time(&self) -> Option<Duration> {
        self.performance_manager.as_ref().map(|pm| pm.frame_time())
    }

    /// Force cleanup of performance caches.
    pub fn cleanup_performance_caches(&mut self) {
        if let Some(ref mut perf_mgr) = self.performance_manager {
            perf_mgr.cleanup();
        }
    }
}

#[cfg(test)]
mod tests;

impl RgbController {
    /// Create a new RGB controller.
    pub fn new(zone_count: usize) -> Self {
        Self {
            engine: EffectEngine::new(Effect::default(), zone_count),
            brightness: 1.0,
            scaled_colors: Vec::with_capacity(zone_count),
        }
    }

    /// Set the lighting effect.
    pub fn set_effect(&mut self, effect: Effect) {
        self.engine.set_effect(effect);
    }

    /// Set brightness (0.0 to 1.0).
    pub fn set_brightness(&mut self, brightness: f32) {
        self.brightness = brightness.clamp(0.0, 1.0);
    }

    /// Get current brightness.
    pub fn brightness(&self) -> f32 {
        self.brightness
    }

    /// Get the current effect.
    pub fn effect(&self) -> &Effect {
        self.engine.effect()
    }

    /// Get the current timing mode.
    pub fn timing_mode(&self) -> TimingMode {
        self.engine.timing_mode()
    }

    /// Set the timing mode.
    pub fn set_timing_mode(&mut self, mode: TimingMode) {
        self.engine.set_timing_mode(mode);
    }

    /// Calculate frame budget for the current effect and timing mode.
    pub fn calculate_frame_budget(&self) -> Duration {
        self.engine.calculate_frame_budget()
    }

    /// Compute current colors with brightness applied.
    /// This method reuses an internal buffer to avoid allocations.
    #[inline]
    pub fn compute_colors(&mut self) -> &[Color] {
        let colors = self.engine.compute();

        // If brightness is 1.0, we can return the colors directly without scaling
        if (self.brightness - 1.0).abs() < f32::EPSILON {
            return colors;
        }

        // Apply brightness scaling reusing internal buffer
        self.scaled_colors.clear();
        self.scaled_colors
            .extend(colors.iter().map(|c| c.scale(self.brightness)));
        &self.scaled_colors
    }
}
