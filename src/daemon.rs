use tracing::{debug, info, warn};

use crate::cli::parse_zone_number;
use crate::deduped_devices;

use steelseries_gg::Result;
use steelseries_gg::config::Config;
use steelseries_gg::device_state::{DeviceId, DeviceStateStore, KeyboardState};
use steelseries_gg::devices::headsets::Headset;
use steelseries_gg::devices::keyboards::Keyboard;
use steelseries_gg::devices::{
    DeviceInfo, DeviceManager, DeviceType,
    discovery::{DeviceFingerprint, HotPlugEvent, print_device_summary},
};
use steelseries_gg::gamesense::GameSenseServer;
use steelseries_gg::profiles::ProfileManager;
use steelseries_gg::rgb::{Color, Effect, RgbController};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub(crate) async fn cmd_server(port: u16) -> Result<()> {
    info!("Starting GameSense server on port {}", port);

    let server = GameSenseServer::new("127.0.0.1", port)?;
    server.run().await?;

    Ok(())
}

/// Daemon state for managing connected devices and RGB controllers.
pub(crate) struct DaemonState {
    keyboards: HashMap<String, (Box<dyn Keyboard>, RgbController, DeviceInfo)>,
    headsets: HashMap<String, (Box<dyn Headset>, DeviceInfo)>,
    gamesense_overlays: Arc<HashMap<String, (Color, std::time::Instant)>>, // zone -> (color, expiry)
    /// Device fingerprints for tracking devices across reconnections
    device_fingerprints: HashMap<String, DeviceFingerprint>,
    /// Profile manager for applying settings to reconnected devices
    profile_manager: Option<ProfileManager>,
    /// State store for persisting device states
    state_store: DeviceStateStore,
}

impl DaemonState {
    async fn new() -> Result<Self> {
        let state_store = DeviceStateStore::new_async().await?;
        let profile_manager = ProfileManager::new().await.ok(); // Optional - don't fail if profiles unavailable

        Ok(Self {
            keyboards: HashMap::new(),
            headsets: HashMap::new(),
            gamesense_overlays: Arc::new(HashMap::new()),
            device_fingerprints: HashMap::new(),
            profile_manager,
            state_store,
        })
    }

    /// Handle device addition event
    async fn handle_device_added(
        &mut self,
        device_manager: &DeviceManager,
        fingerprint: &DeviceFingerprint,
        info: &DeviceInfo,
    ) -> Result<()> {
        let serial = info.serial_number.clone().unwrap_or_else(|| "unknown".to_string());

        if steelseries_gg::devices::is_detect_only(info.product_id) {
            info!("Hot-plug: {} detected (detect-only, not opened)", info.name);
            return Ok(());
        }

        match info.device_type {
            DeviceType::Keyboard => {
                info!("Hot-plug: Adding keyboard: {} ({})", info.name, fingerprint.to_id());

                // Open the keyboard device
                match device_manager.open_keyboard(info) {
                    Ok(mut keyboard) => {
                        // Initialize the device
                        if let Err(e) = keyboard.initialize() {
                            warn!("Failed to initialize keyboard {}: {}", info.name, e);
                            return Err(e);
                        }

                        let zone_count = keyboard.zone_count();
                        let mut rgb_controller = RgbController::new(zone_count);

                        // Try to restore state from state store
                        let device_id = DeviceId::from(info);
                        if let Some(device_state) = self.state_store.get(&device_id) {
                            if let Some(ref keyboard_state) = device_state.keyboard {
                                rgb_controller.set_effect(keyboard_state.effect.clone());
                                rgb_controller.set_brightness(keyboard_state.brightness as f32 / 100.0);

                                // Apply the restored effect if it's static
                                match &keyboard_state.effect {
                                    Effect::Static { color } => {
                                        if let Err(e) = keyboard.set_color(*color).await {
                                            warn!("Failed to apply restored color to {}: {}", info.name, e);
                                        }
                                    }
                                    Effect::Off => {
                                        if let Err(e) = keyboard.set_color(Color::BLACK).await {
                                            warn!("Failed to turn off {}: {}", info.name, e);
                                        }
                                    }
                                    _ => {
                                        info!(
                                            "Restored animated effect for {} (will be applied by animation loop)",
                                            info.name
                                        );
                                    }
                                }

                                info!(
                                    "Restored state for {}: brightness={}%, effect={:?}",
                                    info.name, keyboard_state.brightness, keyboard_state.effect
                                );
                            }
                        } else if let Some(ref profile_manager) = self.profile_manager {
                            // Try to apply default profile
                            Self::apply_default_profile(profile_manager, keyboard.as_mut(), &mut rgb_controller, info)
                                .await;
                        }

                        // Store device information
                        self.keyboards
                            .insert(serial.clone(), (keyboard, rgb_controller, info.clone()));
                        self.device_fingerprints.insert(serial, fingerprint.clone());

                        info!("Successfully added keyboard: {} (zones: {})", info.name, zone_count);
                    }
                    Err(e) => {
                        warn!("Failed to open keyboard {}: {}", info.name, e);
                        return Err(e);
                    }
                }
            }
            DeviceType::Headset => {
                info!("Hot-plug: Adding headset: {} ({})", info.name, fingerprint.to_id());

                match device_manager.open_headset(info) {
                    Ok(mut headset) => {
                        // Initialize the device
                        if let Err(e) = headset.initialize() {
                            warn!("Failed to initialize headset {}: {}", info.name, e);
                            return Err(e);
                        }

                        // Store device information
                        self.headsets.insert(serial.clone(), (headset, info.clone()));
                        self.device_fingerprints.insert(serial, fingerprint.clone());

                        info!("Successfully added headset: {}", info.name);
                    }
                    Err(e) => {
                        warn!("Failed to open headset {}: {}", info.name, e);
                        return Err(e);
                    }
                }
            }
            DeviceType::Mouse => {
                debug!("Hot-plug: Ignoring mouse (not managed by the daemon): {}", info.name);
            }
            DeviceType::Unknown => {
                debug!("Hot-plug: Ignoring unknown device: {}", info.name);
            }
        }

        Ok(())
    }

    async fn apply_default_profile(
        profile_manager: &ProfileManager,
        keyboard: &mut dyn Keyboard,
        rgb_controller: &mut RgbController,
        info: &DeviceInfo,
    ) {
        let Some(default_profile) = profile_manager.get("default") else {
            return;
        };
        let Some(ref keyboard_profile) = default_profile.keyboard else {
            return;
        };

        rgb_controller.set_effect(keyboard_profile.effect.clone());
        rgb_controller.set_brightness(keyboard_profile.brightness as f32 / 100.0);

        match &keyboard_profile.effect {
            Effect::Static { color } => {
                if let Err(e) = keyboard.set_color(*color).await {
                    warn!("Failed to apply default profile color to {}: {}", info.name, e);
                }
            }
            Effect::Off => {
                if let Err(e) = keyboard.set_color(Color::BLACK).await {
                    warn!("Failed to turn off {} (default profile): {}", info.name, e);
                }
            }
            _ => {
                info!(
                    "Applied default animated profile for {} (will be handled by animation loop)",
                    info.name
                );
            }
        }

        info!(
            "Applied default profile to {}: brightness={}%, effect={:?}",
            info.name, keyboard_profile.brightness, keyboard_profile.effect
        );
    }

    /// Get current device count for monitoring
    fn device_count(&self) -> usize {
        self.keyboards.len() + self.headsets.len()
    }

    /// Get list of connected device names
    fn device_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .keyboards
            .values()
            .map(|(_, _, info)| info.name.to_string())
            .collect();

        names.extend(self.headsets.values().map(|(_, info)| info.name.to_string()));
        names
    }
}

pub(crate) async fn cmd_daemon(mut manager: DeviceManager) -> Result<()> {
    info!("Starting SteelSeries GG daemon");

    let config = Config::load_async().await?;

    // Apply saved polling rates
    apply_saved_poll_rates(&config).await;

    // Initialize daemon state
    let daemon_state = Arc::new(RwLock::new(DaemonState::new().await?));

    // Set up and start hot-plug monitoring
    let hotplug_stop_tx = setup_hotplug_monitoring(&mut manager, daemon_state.clone()).await?;

    // Discover and open devices initially
    initialize_devices(&manager, daemon_state.clone()).await;

    // Start GameSense server in background if enabled
    start_gamesense_server(&config, daemon_state.clone());

    // Use provided device manager
    print_device_summary(&manager);

    // Load default profile if configured
    load_default_profile(&config, daemon_state.clone()).await;

    info!("Daemon running. Press Ctrl+C to stop.");

    // Spawn RGB animation loop with adaptive timing and performance monitoring
    let daemon_state_anim = daemon_state.clone();
    let animation_task = tokio::spawn(run_animation_loop(daemon_state_anim));

    // Set up graceful shutdown on SIGTERM (systemd stop) and SIGINT (Ctrl+C)
    wait_for_shutdown().await?;

    // Stop hot-plug monitoring
    if let Err(e) = hotplug_stop_tx.send(()).await {
        debug!("Hot-plug monitoring task already stopped: {}", e);
    } else {
        info!("Stopped hot-plug monitoring");
    }

    // Abort animation task
    animation_task.abort();

    // Save final device states
    save_final_device_states(daemon_state).await;

    info!("Daemon stopped.");
    Ok(())
}

async fn apply_saved_poll_rates(config: &Config) {
    use steelseries_gg::pollrate::{DeviceType, PollRate, set_poll_rate};

    if let Some(mouse_hz) = config.poll_rate.mouse_hz {
        match PollRate::from_hz(mouse_hz) {
            Ok(rate) => match set_poll_rate(DeviceType::Mouse, rate).await {
                Ok(()) => info!("Applied mouse poll rate: {} Hz", mouse_hz),
                Err(e) => tracing::warn!("Failed to set mouse poll rate: {}", e),
            },
            Err(e) => {
                tracing::warn!(
                    "Configured mouse poll rate {} Hz is invalid or unsupported: {}",
                    mouse_hz,
                    e
                );
            }
        }
    }

    if let Some(keyboard_hz) = config.poll_rate.keyboard_hz {
        match PollRate::from_hz(keyboard_hz) {
            Ok(rate) => match set_poll_rate(DeviceType::Keyboard, rate).await {
                Ok(()) => info!("Applied keyboard poll rate: {} Hz", keyboard_hz),
                Err(e) => tracing::warn!("Failed to set keyboard poll rate: {}", e),
            },
            Err(e) => {
                tracing::warn!(
                    "Configured keyboard poll rate {} Hz is invalid or unsupported: {}",
                    keyboard_hz,
                    e
                );
            }
        }
    }
}

async fn setup_hotplug_monitoring(
    manager: &mut DeviceManager,
    daemon_state: Arc<RwLock<DaemonState>>,
) -> Result<tokio::sync::mpsc::Sender<()>> {
    let hotplug_daemon_state = daemon_state;
    manager.set_hotplug_callback(move |event| {
        let _state_clone = hotplug_daemon_state.clone();
        tokio::spawn(async move {
            // Handle hot-plug events without blocking
            match event {
                HotPlugEvent::DeviceAdded {
                    fingerprint,
                    info,
                    timestamp,
                } => {
                    info!(
                        "Hot-plug event: Device added at {:.3}s: {} ({})",
                        timestamp.elapsed().as_secs_f64(),
                        info.name,
                        fingerprint.to_id()
                    );

                    // Note: We can't access the DeviceManager here, so we'll log the event
                    // and let the periodic refresh handle actual device initialization
                    debug!("Deferring device initialization to refresh cycle");
                }
                HotPlugEvent::DeviceRemoved {
                    fingerprint,
                    last_seen,
                    timestamp,
                } => {
                    info!(
                        "Hot-plug event: Device removed at {:.3}s: {} (last seen {:.3}s ago)",
                        timestamp.elapsed().as_secs_f64(),
                        fingerprint.to_id(),
                        timestamp.duration_since(last_seen).as_secs_f64()
                    );

                    // Note: Similar to device addition, we rely on the periodic refresh cycle
                    // to reconcile state after device removal, instead of mutating daemon state
                    // directly from this hot-plug callback.
                    debug!("Deferring device removal handling to refresh cycle");
                }
            }
        });
    });

    // Start hot-plug monitoring
    manager.start_hotplug_monitoring().await
}

async fn initialize_devices(manager: &DeviceManager, daemon_state: Arc<RwLock<DaemonState>>) {
    // Deduplicated, not manager.devices(): the raw list has one entry per HID collection (up to
    // 10 for a single Apex Pro TKL 2023 on Windows), which would open the same physical control
    // endpoint multiple times and register that many independent RgbController/DeviceId entries.
    let devices_info = deduped_devices(manager);
    if devices_info.is_empty() {
        info!("No SteelSeries devices found initially");
    } else {
        let mut state = daemon_state.write().await;
        for info in devices_info {
            let fingerprint = DeviceFingerprint::from_device_info(info);
            if let Err(e) = state.handle_device_added(manager, &fingerprint, info).await {
                warn!("Failed to add initial device {}: {}", info.name, e);
            }
        }
        let device_count = state.device_count();
        let device_names = state.device_names();
        info!("Initialized {} device(s): {}", device_count, device_names.join(", "));
    }
}

fn start_gamesense_server(config: &Config, daemon_state: Arc<RwLock<DaemonState>>) {
    if config.gamesense.enabled {
        let gs_bind = config.gamesense.bind_address.clone();
        let gs_port = config.gamesense.port;
        let daemon_state_clone = daemon_state.clone();

        tokio::spawn(async move {
            match GameSenseServer::new(&gs_bind, gs_port) {
                Ok(server) => {
                    // Set RGB callback to update overlays with optimized async handling
                    server
                        .set_rgb_callback(move |zone: &str, r: u8, g: u8, b: u8| {
                            let state = &daemon_state_clone; // Use reference instead of double-cloning
                            let zone_owned = zone.to_string();

                            // Use a more efficient approach - avoid spawning tasks for simple operations
                            let color = Color::new(r, g, b);
                            let expiry = std::time::Instant::now() + Duration::from_secs(30);

                            // Use try_write to avoid blocking if the lock is busy
                            if let Ok(mut state_guard) = state.try_write() {
                                Arc::make_mut(&mut state_guard.gamesense_overlays)
                                    .insert(zone_owned.clone(), (color, expiry));
                                tracing::debug!("GameSense overlay: {} = {:?}", zone_owned, color);
                            } else {
                                // If we can't get the lock immediately, spawn a task
                                let state_clone = daemon_state_clone.clone();
                                let zone_deferred = zone_owned.clone();
                                tokio::spawn(async move {
                                    let mut state = state_clone.write().await;
                                    Arc::make_mut(&mut state.gamesense_overlays)
                                        .insert(zone_deferred.clone(), (color, expiry));
                                    tracing::debug!("GameSense overlay (deferred): {} = {:?}", zone_deferred, color);
                                });
                            }
                        })
                        .await;

                    if let Err(e) = server.run().await {
                        tracing::error!("GameSense server error: {}", e);
                    }
                }
                Err(e) => tracing::error!("Failed to create GameSense server: {}", e),
            }
        });

        info!(
            "GameSense server starting on {}:{}",
            config.gamesense.bind_address, config.gamesense.port
        );
    } else {
        info!("GameSense server disabled in config");
    }
}

async fn load_default_profile(config: &Config, daemon_state: Arc<RwLock<DaemonState>>) {
    if let Some(ref profile_name) = config.default_profile
        && let Ok(profile_manager) = ProfileManager::new().await
        && let Some(profile) = profile_manager.get(profile_name)
    {
        info!("Loading default profile: {}", profile.name);

        // Apply keyboard settings if present
        if let Some(ref keyboard_profile) = profile.keyboard {
            let keyboard_state = KeyboardState {
                effect: keyboard_profile.effect.clone(),
                brightness: keyboard_profile.brightness,
            };

            // Collect device info first to avoid borrow conflicts
            let device_infos: Vec<DeviceInfo> = {
                let mut state = daemon_state.write().await;
                let mut infos = Vec::new();

                for (_keyboard, controller, info) in state.keyboards.values_mut() {
                    controller.set_effect(keyboard_profile.effect.clone());
                    controller.set_brightness(keyboard_profile.brightness as f32 / 100.0);
                    infos.push(info.clone());
                    info!("Applied profile to keyboard: {}", info.name);
                }

                infos
            };

            // Update state store separately
            {
                let state = daemon_state.read().await;
                for info in device_infos {
                    let device_id = DeviceId::from(&info);
                    let _ = state.state_store.update_keyboard(device_id, keyboard_state.clone());
                }
            }
        }
    }
}

async fn run_animation_loop(daemon_state_anim: Arc<RwLock<DaemonState>>) {
    // We only need the monitor inside the animation loop
    let mut performance_monitor = steelseries_gg::performance::PerformanceMonitor::new();

    // Start with base interval (50ms for compatibility, will be adapted)
    let mut interval = tokio::time::interval(Duration::from_millis(50));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Frame timing tracking
    let mut last_frame_start = Instant::now();
    let mut frames_processed = 0u64;

    // Reusable buffer for RGB data to avoid allocations every frame
    // Stores (serial, colors) for each keyboard
    let mut frame_data_buffer: Vec<(String, Vec<Color>)> = Vec::new();

    loop {
        let frame_start = Instant::now();
        interval.tick().await;

        // Determine current effect complexity by examining active effects
        let current_complexity = {
            let state = daemon_state_anim.read().await;
            let mut max_complexity = steelseries_gg::performance::EffectComplexity::Simple;

            for (_, controller, _) in state.keyboards.values() {
                let effect = controller.effect();
                let effect_complexity = steelseries_gg::performance::calculate_effect_complexity(effect);
                if effect_complexity as u8 > max_complexity as u8 {
                    max_complexity = effect_complexity;
                }
            }
            max_complexity
        };

        // Update performance monitor with current complexity
        performance_monitor.set_effect_complexity(current_complexity);

        // Track frame timing from previous iteration
        if frames_processed > 0 {
            let frame_duration = frame_start.duration_since(last_frame_start);
            // Estimate computation time (we'll measure actual computation below)
            let computation_start = Instant::now();

            // Update memory usage periodically
            if frames_processed.is_multiple_of(60) {
                performance_monitor.update_memory_usage(steelseries_gg::performance::estimate_memory_usage());
            }

            // Record performance metrics
            let computation_time = computation_start.elapsed(); // Will be updated with actual time
            performance_monitor.record_frame_timing(frame_duration, computation_time);

            // Log performance summary periodically
            if frames_processed.is_multiple_of(300) {
                // Every 5 seconds at 60fps
                tracing::debug!("RGB Performance: {}", performance_monitor.performance_summary());
            }
        }

        let computation_start = Instant::now();

        // Split the work: read operations first, then write operations
        let (processed_count, overlays, _now) = {
            let mut state = daemon_state_anim.write().await;

            // Clean up expired overlays while we have the lock
            let now = std::time::Instant::now();
            let has_expired = state.gamesense_overlays.iter().any(|(_, (_, expiry))| *expiry <= now);
            if has_expired {
                Arc::make_mut(&mut state.gamesense_overlays).retain(|_, (_, expiry)| *expiry > now);
            }

            // Collect data needed for RGB computation to minimize lock time
            // Reuse frame_data_buffer to avoid allocations
            let needed_capacity = state.keyboards.len();
            if frame_data_buffer.len() < needed_capacity {
                frame_data_buffer.resize(needed_capacity, (String::new(), Vec::new()));
            }

            let mut count = 0;
            for (serial, (_, controller, _)) in state.keyboards.iter_mut() {
                // Safety check, though resize above ensures capacity
                if count >= frame_data_buffer.len() {
                    break;
                }

                let (buf_serial, buf_colors) = &mut frame_data_buffer[count];

                // Update serial (reusing String allocation)
                buf_serial.clear();
                buf_serial.push_str(serial);

                // Update colors (reusing Vec allocation)
                // controller.compute_colors() returns &[Color]
                let colors = controller.compute_colors();
                buf_colors.clear();
                buf_colors.extend_from_slice(colors);

                count += 1;
            }

            // Fast atomic clone of the Arc
            let overlays = Arc::clone(&state.gamesense_overlays);

            (count, overlays, now)
        };

        // Process RGB updates for each keyboard without holding the lock
        for (serial, colors) in frame_data_buffer.iter_mut().take(processed_count) {
            // Apply GameSense overlays using simple zone mapping
            if !overlays.is_empty() {
                let zone_count = colors.len();
                for (zone, (overlay_color, _)) in overlays.iter() {
                    match parse_zone_number(zone) {
                        None => {
                            // Apply to all zones
                            for c in colors.iter_mut() {
                                *c = *overlay_color;
                            }
                        }
                        Some(idx) if idx < zone_count => {
                            // Apply to specific zone
                            colors[idx] = *overlay_color;
                        }
                        Some(_) => {
                            // Index out of bounds, ignore
                        }
                    }
                }
            }

            // Apply colors to hardware without holding the global state lock across I/O
            // First, remove the keyboard entry from the map while holding the lock
            let keyboard_entry = {
                let mut state = daemon_state_anim.write().await;
                state.keyboards.remove_entry(serial.as_str())
            };

            if let Some((owned_serial, (mut keyboard, controller, other))) = keyboard_entry {
                if let Err(e) = keyboard.set_zone_colors(colors).await {
                    tracing::warn!("Failed to update keyboard {}: {}", serial, e);
                }

                // Reinsert the keyboard entry after the async operation completes
                let mut state = daemon_state_anim.write().await;
                state.keyboards.insert(owned_serial, (keyboard, controller, other));
            }
        }

        // Record actual computation time
        let computation_time = computation_start.elapsed();
        performance_monitor.record_frame_timing(frame_start.elapsed(), computation_time);

        // Calculate optimal timing for next iteration
        let optimal_interval = performance_monitor.calculate_optimal_timing();

        // Update interval if it has changed significantly (>2ms difference)
        let current_interval_ms = interval.period().as_millis() as u64;
        let optimal_interval_ms = optimal_interval.as_millis() as u64;

        if optimal_interval_ms.abs_diff(current_interval_ms) > 2 {
            // Create new interval with optimal timing
            interval = tokio::time::interval(optimal_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            tracing::debug!(
                "Adapted RGB timing: {}ms -> {}ms (complexity: {:?})",
                current_interval_ms,
                optimal_interval_ms,
                current_complexity
            );
        }

        // Performance degradation detection and recovery
        if performance_monitor.is_performance_degraded() {
            tracing::warn!(
                "RGB performance degraded, applying recovery measures: {}",
                performance_monitor.performance_summary()
            );

            // Temporary graceful degradation - increase interval by 50%
            let degraded_interval = optimal_interval.mul_f32(1.5);
            interval = tokio::time::interval(degraded_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        }

        last_frame_start = frame_start;
        frames_processed += 1;
    }
}

async fn wait_for_shutdown() -> Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;

        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down gracefully...");
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, shutting down gracefully...");
            }
        }
    }

    #[cfg(not(unix))]
    {
        use tokio::signal;
        signal::ctrl_c().await?;
        info!("Received Ctrl+C, shutting down gracefully...");
    }

    Ok(())
}

async fn save_final_device_states(daemon_state: Arc<RwLock<DaemonState>>) {
    let state = daemon_state.read().await;

    let update_result = state.state_store.update_states_with(|states| {
        let mut changed = false;
        for (_keyboard, controller, info) in state.keyboards.values() {
            let brightness = (controller.brightness() * 100.0) as u8;
            let effect = controller.effect();

            // Find existing entry or create a new one. We only clone info to DeviceId if
            // we really need to insert a new entry, or if we find it easier to use the entry API.
            // Using loose match to avoid unnecessary DeviceId clones when possible.
            let device_id = DeviceId::from(info);
            let device_state = states.entry(device_id).or_default();

            if let Some(ref mut k_state) = device_state.keyboard {
                if k_state.brightness != brightness || &k_state.effect != effect {
                    k_state.brightness = brightness;
                    k_state.effect = effect.clone();
                    changed = true;
                }
            } else {
                device_state.keyboard = Some(KeyboardState {
                    effect: effect.clone(),
                    brightness,
                });
                changed = true;
            }
        }
        changed
    });

    if let Err(e) = update_result {
        warn!("Failed to batch update final states: {}", e);
    } else {
        debug!(
            "Successfully batched final states for {} device(s)",
            state.device_count()
        );
    }

    // Explicitly await the save operation to ensure data is persisted before shutdown.
    // This addresses the issue of the background task being aborted on drop.
    if let Err(e) = state.state_store.save().await {
        warn!("Failed to persist final states to disk: {}", e);
    } else {
        info!(
            "Successfully persisted final states for {} device(s)",
            state.device_count()
        );
    }
}
