//! SteelSeries Sonar API client (GGSonarRev integration).
//!
//! Provides control over the SteelSeries Sonar audio device through the
//! reverse-engineered HTTP API. Based on https://github.com/PrzemekkkYT/GGSonarRev

use serde::{Deserialize, Serialize};
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

impl SonarClient {
    /// Create a new Sonar client by discovering the dynamic port.
    pub async fn new() -> Result<Self> {
        let port = Self::discover_port().await?;
        let base_url = format!("http://127.0.0.1:{}", port);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| Error::Audio(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { base_url, client })
    }

    /// Create a Sonar client with a specific port (for testing or manual override).
    pub fn with_port(port: u16) -> Result<Self> {
        let base_url = format!("http://127.0.0.1:{}", port);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| Error::Audio(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { base_url, client })
    }

    /// Discover the dynamic Sonar port.
    ///
    /// The Sonar API runs on a different port after each restart. This method
    /// queries the configuration endpoint to find the current port.
    async fn discover_port() -> Result<u16> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| Error::Audio(format!("Failed to create HTTP client: {}", e)))?;

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
        // The response format is typically JSON containing port information
        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| Error::Audio(format!("Failed to parse port discovery response: {}", e)))?;

        // Look for the Sonar port in the response
        // The exact structure depends on the API response format
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

        // If we can't find the port in the structured response, try common ports
        for port in [37330, 37331, 37332, 37333, 37334, 37335] {
            if Self::test_port(&client, port).await {
                return Ok(port);
            }
        }

        Err(Error::Audio(
            "Could not discover Sonar port. Is SteelSeries Sonar running?".to_string(),
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
    // Helper methods
    // ========================================================================

    /// Perform a GET request and deserialize the response.
    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Audio(format!("GET request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Audio(format!(
                "GET request failed with status: {}",
                response.status()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| Error::Audio(format!("Failed to parse response: {}", e)))
    }

    /// Perform a PUT request.
    async fn put(&self, url: &str) -> Result<()> {
        let response = self
            .client
            .put(url)
            .send()
            .await
            .map_err(|e| Error::Audio(format!("PUT request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Audio(format!(
                "PUT request failed with status: {}",
                response.status()
            )));
        }

        Ok(())
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
