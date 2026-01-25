//! SteelSeries Sonar API client (GGSonarRev integration).
//!
//! Provides control over the SteelSeries Sonar audio device through the
//! reverse-engineered HTTP API. Based on:
//! - https://github.com/wex/sonar-rev
//! - https://github.com/PrzemekkkYT/GGSonarRev
//!
//! See `docs/development/SONAR_PROTOCOL.md` for complete API documentation.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::{Error, Result};

/// Sonar API client for controlling the Sonar audio device.
pub struct SonarClient {
    /// Base URL for the Sonar API (with dynamic port)
    base_url: String,
    /// HTTP client
    client: reqwest::Client,
}

/// Audio device information
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: String,
}

/// Audio configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioConfig {
    pub id: String,
    pub name: String,
    pub selected: bool,
}

/// Volume settings for classic mode
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClassicVolume {
    pub master: f32,
    pub game: f32,
    pub chat: f32,
    pub media: f32,
    pub aux: f32,
}

/// Volume settings for streamer mode
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StreamerVolume {
    pub monitoring: ChannelVolumes,
    pub streaming: ChannelVolumes,
}

/// Individual channel volumes
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChannelVolumes {
    pub master: f32,
    pub game: f32,
    pub chat: f32,
    pub media: f32,
    pub aux: f32,
    #[serde(rename = "chatCapture")]
    pub chat_capture: f32,
}

/// Chat mix settings
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatMix {
    pub value: f32,
}

/// Sonar operational mode
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SonarMode {
    Classic,
    Streamer,
}

/// Stream redirection configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StreamRedirection {
    pub game: bool,
    pub chat: bool,
    pub media: bool,
    pub aux: bool,
}

/// Sonar audio channel identifier
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SonarChannel {
    Master,
    Game,
    Chat,
    Media,
    Aux,
    ChatCapture,
}

impl SonarChannel {
    /// Get the string identifier used in Sonar API endpoints
    pub fn as_str(&self) -> &'static str {
        match self {
            SonarChannel::Master => "master",
            SonarChannel::Game => "game",
            SonarChannel::Chat => "chat",
            SonarChannel::Media => "media",
            SonarChannel::Aux => "aux",
            SonarChannel::ChatCapture => "chatCapture",
        }
    }
}

impl SonarClient {
    /// Create a new Sonar client by discovering the dynamic port.
    pub async fn new() -> Result<Self> {
        let port = Self::discover_port().await?;
        let base_url = format!("http://127.0.0.1:{}", port);

        let client = Self::create_http_client()?;

        Ok(Self { base_url, client })
    }

    /// Create a Sonar client with a specific port (for testing or manual override).
    pub fn with_port(port: u16) -> Result<Self> {
        let base_url = format!("http://127.0.0.1:{}", port);
        let client = Self::create_http_client()?;

        Ok(Self { base_url, client })
    }

    /// Create HTTP client configured for Sonar API.
    ///
    /// The Sonar API uses HTTPS with self-signed certificates that are regenerated
    /// on each restart. This client is configured to accept these certificates while
    /// still maintaining local-only communication security.
    fn create_http_client() -> Result<reqwest::Client> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .danger_accept_invalid_certs(true) // Sonar uses self-signed certificates
            .build()
            .map_err(|e| Error::Audio(format!("Failed to create HTTP client: {}", e)))?;

        tracing::warn!(
            "Sonar HTTP client accepts self-signed certificates (Sonar regenerates certs on restart)"
        );

        Ok(client)
    }

    /// Discover the dynamic Sonar port.
    ///
    /// The Sonar API runs on a different port after each restart. This method
    /// uses a fallback chain to find the current port:
    /// 1. Environment variable STEELSERIES_SONAR_PORT
    /// 2. coreProps.json file (Windows/Linux)
    /// 3. HTTP API query to GG core service
    /// 4. Port scanning common Sonar ports
    async fn discover_port() -> Result<u16> {
        // Priority 1: Environment variable override
        if let Ok(port_str) = std::env::var("STEELSERIES_SONAR_PORT") {
            if let Ok(port) = port_str.parse::<u16>() {
                tracing::debug!("Using Sonar port from STEELSERIES_SONAR_PORT: {}", port);
                return Ok(port);
            }
        }

        // Priority 2: Try coreProps.json file discovery
        if let Ok(port) = Self::discover_port_from_config_file() {
            tracing::debug!("Discovered Sonar port from coreProps.json: {}", port);
            return Ok(port);
        }

        // Priority 3: HTTP API query to GG core service
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| Error::Audio(format!("Failed to create HTTP client: {}", e)))?;

        if let Ok(port) = Self::discover_port_from_http_api(&client).await {
            tracing::debug!("Discovered Sonar port from HTTP API: {}", port);
            return Ok(port);
        }

        // Priority 4: Port scanning fallback
        tracing::debug!("Falling back to port scanning");
        for port in [37330, 37331, 37332, 37333, 37334, 37335] {
            if Self::test_port(&client, port).await {
                tracing::debug!("Found Sonar on port {} via scanning", port);
                return Ok(port);
            }
        }

        Err(Error::Audio(
            "Could not discover Sonar port. Is SteelSeries Sonar running?".to_string(),
        ))
    }

    /// Discover Sonar port from coreProps.json configuration file.
    ///
    /// Searches for coreProps.json in platform-specific locations:
    /// - Linux: ~/.config/SteelSeries/GG/coreProps.json
    /// - Windows: C:\ProgramData\SteelSeries\GG\coreProps.json
    fn discover_port_from_config_file() -> Result<u16> {
        use std::fs;

        let config_paths = Self::get_config_file_paths();

        for path in config_paths {
            if !path.exists() {
                continue;
            }

            let contents = fs::read_to_string(&path).map_err(|e| {
                Error::Audio(format!(
                    "Failed to read coreProps.json at {:?}: {}",
                    path, e
                ))
            })?;

            let json: serde_json::Value = serde_json::from_str(&contents).map_err(|e| {
                Error::Audio(format!(
                    "Failed to parse coreProps.json at {:?}: {}",
                    path, e
                ))
            })?;

            // Extract port from coreProps.json structure
            // Structure: { "ggEncryptedAddress": "https://127.0.0.1:PORT" }
            // or { "address": "https://127.0.0.1:PORT" }
            if let Some(address) = json
                .get("ggEncryptedAddress")
                .or_else(|| json.get("address"))
                .and_then(|v| v.as_str())
            {
                if let Some(port_str) = address.split(':').next_back() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        return Ok(port);
                    }
                }
            }
        }

        Err(Error::Audio(
            "coreProps.json not found or invalid".to_string(),
        ))
    }

    /// Get platform-specific paths to coreProps.json.
    fn get_config_file_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // Linux paths
        if cfg!(target_os = "linux") {
            if let Some(home) = std::env::var_os("HOME") {
                let home_path = PathBuf::from(home);
                paths.push(home_path.join(".config/SteelSeries/GG/coreProps.json"));
                paths.push(home_path.join(".local/share/SteelSeries/GG/coreProps.json"));
            }
            paths.push(PathBuf::from("/etc/SteelSeries/GG/coreProps.json"));
        }

        // Windows paths
        if cfg!(target_os = "windows") {
            if let Some(programdata) = std::env::var_os("PROGRAMDATA") {
                paths.push(PathBuf::from(programdata).join("SteelSeries/GG/coreProps.json"));
            }
        }

        paths
    }

    /// Discover Sonar port via HTTP API query to GG core service.
    async fn discover_port_from_http_api(client: &reqwest::Client) -> Result<u16> {
        // Query the subApps endpoint to find the Sonar port
        let url = "http://127.0.0.1:6327/subApps";
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Audio(format!("Failed to discover Sonar port: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Audio(format!(
                "Port discovery failed with status: {}",
                response.status()
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| Error::Audio(format!("Failed to read port discovery response: {}", e)))?;

        // Parse the response to extract the Sonar port
        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| Error::Audio(format!("Failed to parse port discovery response: {}", e)))?;

        // Look for the Sonar port in the response
        if let Some(apps) = json.as_array() {
            for app in apps {
                if let Some(name) = app.get("name").and_then(|n| n.as_str()) {
                    if name.to_lowercase().contains("sonar") {
                        if let Some(port) = app.get("port").and_then(|p| p.as_u64()) {
                            return Ok(port as u16);
                        }
                    }
                }
            }
        }

        Err(Error::Audio(
            "Sonar port not found in API response".to_string(),
        ))
    }

    /// Test if a port is the Sonar API port.
    async fn test_port(client: &reqwest::Client, port: u16) -> bool {
        let url = format!("http://127.0.0.1:{}/mode", port);
        client.get(&url).send().await.is_ok()
    }

    // ========================================================================
    // GET Endpoints
    // ========================================================================

    /// Get all audio configurations.
    pub async fn get_configs(&self) -> Result<Vec<AudioConfig>> {
        let url = format!("{}/configs", self.base_url);
        self.get(&url).await
    }

    /// Get the selected audio configuration.
    pub async fn get_selected_config(&self) -> Result<AudioConfig> {
        let url = format!("{}/configs/selected", self.base_url);
        self.get(&url).await
    }

    /// Get all audio devices.
    pub async fn get_audio_devices(&self) -> Result<Vec<AudioDevice>> {
        let url = format!("{}/audioDevices", self.base_url);
        self.get(&url).await
    }

    /// Get chat mix settings.
    pub async fn get_chat_mix(&self) -> Result<ChatMix> {
        let url = format!("{}/chatMix", self.base_url);
        self.get(&url).await
    }

    /// Get current operational mode (classic or streamer).
    pub async fn get_mode(&self) -> Result<SonarMode> {
        let url = format!("{}/mode", self.base_url);
        self.get(&url).await
    }

    /// Get volume settings for classic mode.
    pub async fn get_classic_volumes(&self) -> Result<ClassicVolume> {
        let url = format!("{}/volumeSettings/classic", self.base_url);
        self.get(&url).await
    }

    /// Get volume settings for streamer mode.
    pub async fn get_streamer_volumes(&self) -> Result<StreamerVolume> {
        let url = format!("{}/volumeSettings/streamer", self.base_url);
        self.get(&url).await
    }

    /// Get classic mode redirections.
    pub async fn get_classic_redirections(&self) -> Result<serde_json::Value> {
        let url = format!("{}/classicRedirections", self.base_url);
        self.get(&url).await
    }

    /// Get stream mode redirections.
    pub async fn get_stream_redirections(&self) -> Result<StreamRedirection> {
        let url = format!("{}/streamRedirections", self.base_url);
        self.get(&url).await
    }

    // ========================================================================
    // PUT Endpoints
    // ========================================================================

    /// Select an audio configuration.
    pub async fn select_config(&self, config_id: &str) -> Result<()> {
        let url = format!("{}/configs/{}/select", self.base_url, config_id);
        self.put(&url).await
    }

    /// Set master volume (classic mode).
    pub async fn set_classic_master_volume(&self, volume: f32) -> Result<()> {
        self.set_volume("classic", "master", volume).await
    }

    /// Set game volume (classic mode).
    pub async fn set_classic_game_volume(&self, volume: f32) -> Result<()> {
        self.set_volume("classic", "game", volume).await
    }

    /// Set chat volume (classic mode).
    pub async fn set_classic_chat_volume(&self, volume: f32) -> Result<()> {
        self.set_volume("classic", "chat", volume).await
    }

    /// Set media volume (classic mode).
    pub async fn set_classic_media_volume(&self, volume: f32) -> Result<()> {
        self.set_volume("classic", "media", volume).await
    }

    /// Set aux volume (classic mode).
    pub async fn set_classic_aux_volume(&self, volume: f32) -> Result<()> {
        self.set_volume("classic", "aux", volume).await
    }

    /// Set monitoring master volume (streamer mode).
    pub async fn set_monitoring_master_volume(&self, volume: f32) -> Result<()> {
        self.set_volume("streamer/monitoring", "master", volume)
            .await
    }

    /// Set monitoring game volume (streamer mode).
    pub async fn set_monitoring_game_volume(&self, volume: f32) -> Result<()> {
        self.set_volume("streamer/monitoring", "game", volume).await
    }

    /// Set monitoring chat volume (streamer mode).
    pub async fn set_monitoring_chat_volume(&self, volume: f32) -> Result<()> {
        self.set_volume("streamer/monitoring", "chat", volume).await
    }

    /// Set streaming master volume (streamer mode).
    pub async fn set_streaming_master_volume(&self, volume: f32) -> Result<()> {
        self.set_volume("streamer/streaming", "master", volume)
            .await
    }

    /// Toggle stream redirection for a channel.
    pub async fn toggle_stream_redirection(&self, channel: &str, enabled: bool) -> Result<()> {
        let url = format!(
            "{}/streamRedirections/{}/toggle/{}",
            self.base_url, channel, enabled
        );
        self.put(&url).await
    }

    /// Stop recording audio samples.
    pub async fn stop_audio_sample_recording(&self) -> Result<()> {
        let url = format!("{}/audioSamples/stopRecord", self.base_url);
        self.put(&url).await
    }

    // ========================================================================
    // Channel-specific GET/SET operations
    // ========================================================================

    /// Get volume for a specific channel (classic mode).
    pub async fn get_channel_volume(&self, channel: SonarChannel) -> Result<f32> {
        let url = format!(
            "{}/volumeSettings/classic/{}/volume",
            self.base_url,
            channel.as_str()
        );
        self.get(&url).await
    }

    /// Set volume for a specific channel (classic mode).
    pub async fn set_channel_volume(&self, channel: SonarChannel, volume: f32) -> Result<()> {
        self.set_volume("classic", channel.as_str(), volume).await
    }

    /// Get mute state for a specific channel (classic mode).
    pub async fn get_channel_mute(&self, channel: SonarChannel) -> Result<bool> {
        let url = format!(
            "{}/volumeSettings/classic/{}/mute",
            self.base_url,
            channel.as_str()
        );
        self.get(&url).await
    }

    /// Set mute state for a specific channel (classic mode).
    pub async fn set_channel_mute(&self, channel: SonarChannel, muted: bool) -> Result<()> {
        let url = format!(
            "{}/volumeSettings/classic/{}/mute/{}",
            self.base_url,
            channel.as_str(),
            muted
        );
        self.put(&url).await
    }

    /// Get volume for a specific channel (streamer monitoring).
    pub async fn get_monitoring_channel_volume(&self, channel: SonarChannel) -> Result<f32> {
        let url = format!(
            "{}/volumeSettings/streamer/monitoring/{}/volume",
            self.base_url,
            channel.as_str()
        );
        self.get(&url).await
    }

    /// Set volume for a specific channel (streamer monitoring).
    pub async fn set_monitoring_channel_volume(
        &self,
        channel: SonarChannel,
        volume: f32,
    ) -> Result<()> {
        self.set_volume("streamer/monitoring", channel.as_str(), volume)
            .await
    }

    /// Get volume for a specific channel (streamer streaming).
    pub async fn get_streaming_channel_volume(&self, channel: SonarChannel) -> Result<f32> {
        let url = format!(
            "{}/volumeSettings/streamer/streaming/{}/volume",
            self.base_url,
            channel.as_str()
        );
        self.get(&url).await
    }

    /// Set volume for a specific channel (streamer streaming).
    pub async fn set_streaming_channel_volume(
        &self,
        channel: SonarChannel,
        volume: f32,
    ) -> Result<()> {
        self.set_volume("streamer/streaming", channel.as_str(), volume)
            .await
    }

    // ========================================================================
    // Helper methods
    // ========================================================================

    /// Perform a GET request and deserialize the response.
    ///
    /// Retries transient failures (connection errors, timeouts) up to 2 times.
    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        const MAX_RETRIES: u32 = 2;

        let mut last_error = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                tracing::debug!(
                    "Retrying GET request (attempt {}/{})",
                    attempt + 1,
                    MAX_RETRIES + 1
                );
                tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
            }

            let response = match self.client.get(url).send().await {
                Ok(r) => r,
                Err(e) => {
                    if Self::is_transient_error(&e) && attempt < MAX_RETRIES {
                        last_error = Some(e);
                        continue;
                    }
                    return Err(Error::Audio(format!("GET request failed: {}", e)));
                }
            };

            if !response.status().is_success() {
                return Err(Error::Audio(format!(
                    "GET request failed with status: {}",
                    response.status()
                )));
            }

            return response
                .json()
                .await
                .map_err(|e| Error::Audio(format!("Failed to parse response: {}", e)));
        }

        Err(Error::Audio(format!(
            "GET request failed after {} retries: {}",
            MAX_RETRIES,
            last_error.unwrap()
        )))
    }

    /// Perform a PUT request.
    ///
    /// Retries transient failures (connection errors, timeouts) up to 2 times.
    async fn put(&self, url: &str) -> Result<()> {
        const MAX_RETRIES: u32 = 2;

        let mut last_error = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                tracing::debug!(
                    "Retrying PUT request (attempt {}/{})",
                    attempt + 1,
                    MAX_RETRIES + 1
                );
                tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
            }

            let response = match self.client.put(url).send().await {
                Ok(r) => r,
                Err(e) => {
                    if Self::is_transient_error(&e) && attempt < MAX_RETRIES {
                        last_error = Some(e);
                        continue;
                    }
                    return Err(Error::Audio(format!("PUT request failed: {}", e)));
                }
            };

            if !response.status().is_success() {
                return Err(Error::Audio(format!(
                    "PUT request failed with status: {}",
                    response.status()
                )));
            }

            return Ok(());
        }

        Err(Error::Audio(format!(
            "PUT request failed after {} retries: {}",
            MAX_RETRIES,
            last_error.unwrap()
        )))
    }

    /// Check if an HTTP error is transient and should be retried.
    fn is_transient_error(error: &reqwest::Error) -> bool {
        error.is_timeout() || error.is_connect() || error.is_request()
    }

    /// Generic helper to set volume for a specific path.
    ///
    /// Clamps volume to [0.0, 1.0] and constructs the URL path.
    async fn set_volume(&self, mode: &str, channel: &str, volume: f32) -> Result<()> {
        let volume = volume.clamp(0.0, 1.0);
        let url = format!(
            "{}/volumeSettings/{}/{}/volume/{}",
            self.base_url, mode, channel, volume
        );
        self.put(&url).await
    }

    /// Get the base URL for the Sonar API.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_port_discovery() {
        // This test requires SteelSeries Sonar to be running
        if let Ok(client) = SonarClient::new().await {
            assert!(!client.base_url.is_empty());
        }
    }
}
