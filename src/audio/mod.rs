//! Audio mixer functionality (Sonar replacement).
//!
//! Provides multi-channel audio mixing with support for:
//! - Game, Chat, Media, Aux, and Mic channels
//! - Per-application audio routing
//! - Streamer mode with separate streaming/monitoring sliders

#[cfg(feature = "sonar")]
pub mod sonar;

#[cfg(feature = "audio")]
pub mod pulse;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{Error, Result};

#[cfg(feature = "sonar")]
pub use sonar::{SonarChannel, SonarClient};

#[cfg(feature = "audio")]
use self::pulse::PulseHandler;

// Channel types are used by both audio and sonar features
#[cfg(any(feature = "audio", feature = "sonar"))]
/// Audio channel identifier.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Channel {
    /// Master volume (affects all channels)
    Master,
    /// Game audio
    Game,
    /// Voice chat audio
    Chat,
    /// Music/media audio
    Media,
    /// Auxiliary audio
    Aux,
    /// Microphone input
    Mic,
}

#[cfg(any(feature = "audio", feature = "sonar"))]
impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Channel::Master => write!(f, "Master"),
            Channel::Game => write!(f, "Game"),
            Channel::Chat => write!(f, "Chat"),
            Channel::Media => write!(f, "Media"),
            Channel::Aux => write!(f, "Aux"),
            Channel::Mic => write!(f, "Mic"),
        }
    }
}

#[cfg(feature = "audio")]
/// Channel state.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChannelState {
    /// Volume level (0.0 - 1.0)
    pub volume: f32,
    /// Whether the channel is muted
    pub muted: bool,
}

#[cfg(feature = "audio")]
impl Default for ChannelState {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
        }
    }
}

#[cfg(feature = "audio")]
/// Audio mixer state.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MixerState {
    /// Channel states
    pub channels: HashMap<Channel, ChannelState>,

    /// Streamer mode enabled
    pub streamer_mode: bool,

    /// Streaming slider state (when in streamer mode)
    pub streaming: HashMap<Channel, ChannelState>,

    /// Monitoring slider state (when in streamer mode)
    pub monitoring: HashMap<Channel, ChannelState>,

    /// Chat mix balance (-1.0 = full game, 1.0 = full chat)
    pub chat_mix: f32,
}

#[cfg(feature = "audio")]
impl Default for MixerState {
    fn default() -> Self {
        let mut channels = HashMap::new();
        for channel in [
            Channel::Master,
            Channel::Game,
            Channel::Chat,
            Channel::Media,
            Channel::Aux,
            Channel::Mic,
        ] {
            channels.insert(channel, ChannelState::default());
        }

        Self {
            channels,
            streamer_mode: false,
            streaming: HashMap::new(),
            monitoring: HashMap::new(),
            chat_mix: 0.0,
        }
    }
}

#[cfg(feature = "audio")]
/// Audio mixer for controlling channel volumes.
pub struct AudioMixer {
    state: MixerState,
    // PulseAudio/PipeWire connection
    #[cfg(feature = "audio")]
    pulse: Option<PulseHandler>,
}

#[cfg(feature = "audio")]
impl AudioMixer {
    /// Create a new audio mixer.
    pub fn new() -> Result<Self> {
        let pulse = match PulseHandler::connect() {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!("Failed to connect to PulseAudio: {}", e);
                None
            }
        };

        Ok(Self {
            state: MixerState::default(),
            pulse,
        })
    }

    /// Get the current mixer state.
    pub fn state(&self) -> &MixerState {
        &self.state
    }

    /// Set volume for a channel.
    pub fn set_volume(&mut self, channel: Channel, volume: f32) -> Result<()> {
        let volume = volume.clamp(0.0, 1.0);

        if let Some(state) = self.state.channels.get_mut(&channel) {
            state.volume = volume;
            self.apply_channel(channel)?;
        }

        Ok(())
    }

    /// Get volume for a channel.
    pub fn get_volume(&self, channel: Channel) -> f32 {
        self.state
            .channels
            .get(&channel)
            .map(|s| s.volume)
            .unwrap_or(1.0)
    }

    /// Mute or unmute a channel.
    pub fn set_mute(&mut self, channel: Channel, muted: bool) -> Result<()> {
        if let Some(state) = self.state.channels.get_mut(&channel) {
            state.muted = muted;
            self.apply_channel(channel)?;
        }

        Ok(())
    }

    /// Check if a channel is muted.
    pub fn is_muted(&self, channel: Channel) -> bool {
        self.state
            .channels
            .get(&channel)
            .map(|s| s.muted)
            .unwrap_or(false)
    }

    /// Toggle mute for a channel.
    pub fn toggle_mute(&mut self, channel: Channel) -> Result<bool> {
        let muted = !self.is_muted(channel);
        self.set_mute(channel, muted)?;
        Ok(muted)
    }

    /// Set chat mix balance.
    pub fn set_chat_mix(&mut self, balance: f32) -> Result<()> {
        self.state.chat_mix = balance.clamp(-1.0, 1.0);

        // Calculate volume factors for game/chat balance
        let (game_factor, chat_factor) = self.calculate_balance_factors(balance);

        // Store calculated factors for potential audio system integration
        // TODO: Integrate with PulseAudio/PipeWire to actually apply these factors
        // Implementation roadmap:
        // 1. Use libpulse-binding to enumerate sink inputs by application
        // 2. Apply game_factor to game audio sink inputs
        // 3. Apply chat_factor to communication app sink inputs
        // 4. Handle errors if audio system is unavailable
        tracing::debug!(
            "Chat mix set to {:.2}: game_factor={:.2}, chat_factor={:.2}",
            balance,
            game_factor,
            chat_factor
        );

        Ok(())
    }

    /// Get chat mix balance.
    pub fn chat_mix(&self) -> f32 {
        self.state.chat_mix
    }

    /// Calculate volume factors for game and chat based on balance.
    ///
    /// Balance ranges from -1.0 (game only) to 1.0 (chat only), with 0.0 being balanced.
    /// Returns (game_factor, chat_factor) where each factor is between 0.0 and 1.0.
    fn calculate_balance_factors(&self, balance: f32) -> (f32, f32) {
        let balance = balance.clamp(-1.0, 1.0);

        let game_factor = if balance <= 0.0 {
            1.0 // Full game volume when balance is negative or neutral
        } else {
            1.0 - balance // Reduce game volume as we go toward chat
        };

        let chat_factor = if balance >= 0.0 {
            1.0 // Full chat volume when balance is positive or neutral
        } else {
            1.0 + balance // Reduce chat volume as we go toward game (balance is negative)
        };

        (game_factor, chat_factor)
    }

    /// Populate a channel map from the source channels.
    fn populate_channel_map(
        source: &HashMap<Channel, ChannelState>,
        target: &mut HashMap<Channel, ChannelState>,
    ) {
        const CHANNELS: &[Channel] = &[
            Channel::Master,
            Channel::Game,
            Channel::Chat,
            Channel::Media,
            Channel::Aux,
            Channel::Mic,
        ];

        for &channel in CHANNELS {
            if let Some(channel_state) = source.get(&channel) {
                target.insert(channel, channel_state.clone());
            }
        }
    }

    /// Enable or disable streamer mode.
    pub fn set_streamer_mode(&mut self, enabled: bool) -> Result<()> {
        self.state.streamer_mode = enabled;

        if enabled {
            // Initialize streaming/monitoring only if empty (lazy initialization)
            // Values will be populated on first access via or_default()
            // This avoids unnecessary HashMap cloning
            if self.state.streaming.is_empty() {
                Self::populate_channel_map(&self.state.channels, &mut self.state.streaming);
            }
            if self.state.monitoring.is_empty() {
                Self::populate_channel_map(&self.state.channels, &mut self.state.monitoring);
            }
        } else {
            // When disabling streamer mode, clear streaming/monitoring state so that
            // it will be re-initialized from current channel values next time.
            self.state.streaming.clear();
            self.state.monitoring.clear();
        }

        Ok(())
    }

    /// Check if streamer mode is enabled.
    pub fn is_streamer_mode(&self) -> bool {
        self.state.streamer_mode
    }

    /// Set volume for streaming slider (streamer mode).
    pub fn set_streaming_volume(&mut self, channel: Channel, volume: f32) -> Result<()> {
        if !self.state.streamer_mode {
            return Err(Error::Audio("Streamer mode not enabled".to_string()));
        }

        let volume = volume.clamp(0.0, 1.0);
        self.state.streaming.entry(channel).or_default().volume = volume;

        Ok(())
    }

    /// Set volume for monitoring slider (streamer mode).
    pub fn set_monitoring_volume(&mut self, channel: Channel, volume: f32) -> Result<()> {
        if !self.state.streamer_mode {
            return Err(Error::Audio("Streamer mode not enabled".to_string()));
        }

        let volume = volume.clamp(0.0, 1.0);
        self.state.monitoring.entry(channel).or_default().volume = volume;

        Ok(())
    }

    /// Apply channel settings to the audio system.
    fn apply_channel(&mut self, channel: Channel) -> Result<()> {
        // Validate channel state before potential audio system integration
        let (volume, muted) = {
            let channel_state = self
                .state
                .channels
                .get(&channel)
                .ok_or_else(|| Error::Audio(format!("Channel {:?} not found", channel)))?;

            // Validate the volume is in acceptable range
            if !(0.0..=1.0).contains(&channel_state.volume) {
                return Err(Error::Audio(format!(
                    "Invalid volume {:.2} for channel {:?} (must be 0.0-1.0)",
                    channel_state.volume,
                    channel
                )));
            }

            (channel_state.volume, channel_state.muted)
        };

        tracing::debug!(
            "Applying channel {:?}: volume={:.2}, mute={}",
            channel,
            volume,
            muted
        );

        if let Some(pulse) = &mut self.pulse {
            let effective_volume = if muted { 0.0 } else { volume };

            match channel {
                Channel::Master => pulse.set_master_volume(effective_volume)?,
                Channel::Mic => pulse.set_mic_volume(effective_volume)?,
                // For other channels, we currently apply to all sink inputs as a placeholder
                // until per-application routing is implemented
                _ => pulse.set_all_sink_inputs_volume(effective_volume)?,
            }
        }

        Ok(())
    }

    /// Get all channel states.
    pub fn all_channels(&self) -> &HashMap<Channel, ChannelState> {
        &self.state.channels
    }
}

#[cfg(feature = "audio")]
/// Application audio routing entry.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppRoute {
    /// Application name or executable.
    pub app_name: String,

    /// Target channel.
    pub channel: Channel,

    /// Override volume (None = use channel volume).
    pub volume_override: Option<f32>,
}

#[cfg(feature = "audio")]
/// Audio router for per-application routing.
pub struct AudioRouter {
    routes: Vec<AppRoute>,
}

#[cfg(feature = "audio")]
impl AudioRouter {
    /// Create a new audio router.
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Add a route for an application.
    pub fn add_route(&mut self, route: AppRoute) {
        // Remove existing route for this app
        self.routes.retain(|r| r.app_name != route.app_name);
        self.routes.push(route);
    }

    /// Remove a route.
    pub fn remove_route(&mut self, app_name: &str) {
        self.routes.retain(|r| r.app_name != app_name);
    }

    /// Get the route for an application.
    pub fn get_route(&self, app_name: &str) -> Option<&AppRoute> {
        self.routes.iter().find(|r| r.app_name == app_name)
    }

    /// Get all routes.
    pub fn routes(&self) -> &[AppRoute] {
        &self.routes
    }
}

#[cfg(feature = "audio")]
impl Default for AudioRouter {
    fn default() -> Self {
        Self::new()
    }
}
