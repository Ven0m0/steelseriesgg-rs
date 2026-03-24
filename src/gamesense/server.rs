//! GameSense HTTP server implementation.

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, Method, StatusCode, header},
    routing::{get, post},
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::net::SocketAddr;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{debug, info};

use super::*;
use crate::{Error, Result};

/// Type alias for RGB callback functions.
type RgbCallback = Box<dyn Fn(&str, u8, u8, u8) + Send + Sync>;

/// Shared server state.
struct ServerState {
    /// Registered games - pre-allocate capacity for typical usage
    games: HashMap<String, GameMetadata>,

    /// Event bindings per game - use nested HashMap with better initial capacity
    bindings: HashMap<String, HashMap<String, EventBinding>>,

    /// Last event values - optimize for frequent access
    event_values: HashMap<String, HashMap<String, i32>>,

    /// Callback for RGB updates.
    rgb_callback: Option<RgbCallback>,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            // Pre-allocate reasonable capacity for typical gaming scenarios
            games: HashMap::with_capacity(8),
            bindings: HashMap::with_capacity(8),
            event_values: HashMap::with_capacity(8),
            rgb_callback: None,
        }
    }
}

/// GameSense-compatible HTTP server.
pub struct GameSenseServer {
    state: Arc<RwLock<ServerState>>,
    bind_addr: SocketAddr,
}

impl GameSenseServer {
    pub fn new(host: &str, port: u16) -> Result<Self> {
        let addr: SocketAddr = format!("{}:{}", host, port)
            .parse()
            .map_err(|e| Error::GameSense(format!("Invalid bind address: {}", e)))?;

        Ok(Self {
            state: Arc::new(RwLock::new(ServerState::default())),
            bind_addr: addr,
        })
    }

    /// Set a callback for RGB color changes.
    pub async fn set_rgb_callback<F>(&self, callback: F)
    where
        F: Fn(&str, u8, u8, u8) + Send + Sync + 'static,
    {
        let mut state = self.state.write();
        state.rgb_callback = Some(Box::new(callback));
    }

    /// Build the router.
    fn router(&self) -> Router {
        let state = self.state.clone();

        Router::new()
            // GameSense SDK endpoints
            .route("/game_metadata", post(register_game))
            .route("/bind_game_event", post(bind_event))
            .route("/register_game_event", post(bind_event))
            .route("/game_event", post(game_event))
            .route("/game_heartbeat", post(heartbeat))
            .route("/remove_game", post(remove_game))
            .route("/remove_game_event", post(remove_event))
            // Info endpoint
            .route("/", get(server_info))
            .layer(
                CorsLayer::new()
                    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                    .allow_headers([header::CONTENT_TYPE])
                    .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _parts: &_| {
                        let origin_bytes = origin.as_bytes();
                        origin_bytes == b"http://localhost"
                            || origin_bytes.starts_with(b"http://localhost:")
                            || origin_bytes == b"https://localhost"
                            || origin_bytes.starts_with(b"https://localhost:")
                            || origin_bytes == b"http://127.0.0.1"
                            || origin_bytes.starts_with(b"http://127.0.0.1:")
                            || origin_bytes == b"https://127.0.0.1"
                            || origin_bytes.starts_with(b"https://127.0.0.1:")
                    })),
            )
            .with_state(state)
    }

    /// Start the server.
    pub async fn run(&self) -> Result<()> {
        let router = self.router();

        info!("GameSense server listening on {}", self.bind_addr);

        // Write the coreProps.json file for game discovery
        self.write_core_props().await?;

        let listener = tokio::net::TcpListener::bind(self.bind_addr).await?;
        axum::serve(listener, router).await?;

        Ok(())
    }

    /// Write coreProps.json for game SDK discovery.
    async fn write_core_props(&self) -> Result<()> {
        let props = serde_json::json!({
            "address": format!("127.0.0.1:{}", self.bind_addr.port())
        });

        // Standard location for SteelSeries Engine
        #[cfg(target_os = "linux")]
        let path = std::path::Path::new("/tmp/steelseries-engine/coreProps.json");

        #[cfg(target_os = "windows")]
        let path = {
            let programdata = std::env::var("PROGRAMDATA").unwrap_or_default();
            std::path::PathBuf::from(programdata)
                .join("SteelSeries")
                .join("SteelSeries Engine 3")
                .join("coreProps.json")
        };

        #[cfg(target_os = "macos")]
        let path = std::path::Path::new("/Library/Application Support/SteelSeries Engine 3/coreProps.json");

        Self::write_secure_json(path, &props)
    }

    /// Securely write JSON content to a file, ensuring correct permissions and ownership.
    fn write_secure_json(path: &std::path::Path, content: &serde_json::Value) -> Result<()> {
        #[cfg(unix)]
        {
            // Secure directory creation for /tmp usage
            if let Some(parent) = path.parent() {
                if parent.exists() {
                    let metadata = fs::symlink_metadata(parent)?;

                    // Verify it is a directory and not a symlink
                    if !metadata.is_dir() {
                        return Err(Error::GameSense(format!(
                            "Security error: {:?} is not a directory",
                            parent
                        )));
                    }

                    // Verify ownership (must be owned by us)
                    if metadata.uid() != rustix::process::getuid().as_raw() {
                        return Err(Error::GameSense(format!(
                            "Security error: {:?} is not owned by the current user",
                            parent
                        )));
                    }
                } else {
                    // Create with strict permissions (rwx------)
                    fs::DirBuilder::new().recursive(true).mode(0o700).create(parent)?;
                }
            }

            // Secure file write
            let mut options = OpenOptions::new();
            options.write(true).create(true).truncate(true);

            // Set file creation mode to 600 (rw-------)
            options.mode(0o600);

            // Do not follow symlinks for the file itself
            options.custom_flags(libc::O_NOFOLLOW);

            let file = options.open(path)?;

            // Ensure file permissions are 600 (rw-------) even if file already existed
            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o600);
            file.set_permissions(perms)?;

            // Write content
            serde_json::to_writer_pretty(file, content)?;
            debug!("Wrote secure JSON to {:?}", path);
        }

        #[cfg(not(unix))]
        {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(path, serde_json::to_string_pretty(content)?)?;
            debug!("Wrote JSON to {:?}", path);
        }

        Ok(())
    }

    /// Get the server address.
    pub fn address(&self) -> SocketAddr {
        self.bind_addr
    }
}

type AppState = Arc<RwLock<ServerState>>;

/// Server info endpoint.
async fn server_info() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "address": "127.0.0.1",
        "encrypted_address": null,
        "sse_address": null,
        "version": "4.0.0"
    }))
}

/// Register a game.
async fn register_game(
    State(state): State<AppState>,
    Json(metadata): Json<GameMetadata>,
) -> (StatusCode, Json<ApiResponse>) {
    let mut state = state.write();

    info!("Registering game: {} ({})", metadata.game_display_name, metadata.game);

    state.games.insert(metadata.game.clone(), metadata);

    (StatusCode::OK, Json(ApiResponse::success()))
}

/// Bind or register an event handler.
async fn bind_event(
    State(state): State<AppState>,
    Json(binding): Json<EventBinding>,
) -> (StatusCode, Json<ApiResponse>) {
    let mut state = state.write();

    debug!("Binding event: {}:{}", binding.game, binding.event);

    let game_bindings = state.bindings.entry(binding.game.clone()).or_default();
    game_bindings.insert(binding.event.clone(), binding);

    (StatusCode::OK, Json(ApiResponse::success()))
}

/// Handle a game event.
async fn game_event(State(state): State<AppState>, Json(event): Json<GameEvent>) -> (StatusCode, Json<ApiResponse>) {
    debug!("Game event: {}:{} = {}", event.game, event.event, event.data.value);

    // Store the event value with write lock (brief critical section)
    {
        let mut state_write = state.write();
        state_write
            .event_values
            .entry(event.game.clone())
            .or_default()
            .insert(event.event.clone(), event.data.value);
    } // Write lock released here

    // Process handlers with read lock to allow concurrent event handling
    let state_read = state.read();
    if let Some(game_bindings) = state_read.bindings.get(&event.game) {
        if let Some(binding) = game_bindings.get(&event.event) {
            // Process handlers
            for handler in &binding.handlers {
                process_handler(handler, event.data.value, &state_read);
            }
        }
    }

    (StatusCode::OK, Json(ApiResponse::success()))
}

/// Process a handler with the given value.
fn process_handler(handler: &Handler, value: i32, state: &ServerState) {
    match handler {
        Handler::RgbPerKeyZones { zone, color, .. } | Handler::Keyboard { zone, color, .. } => {
            if let Some((r, g, b)) = compute_color(color, value) {
                debug!("Setting {} to RGB({}, {}, {})", zone, r, g, b);
                if let Some(ref callback) = state.rgb_callback {
                    callback(zone, r, g, b);
                }
            }
        }
        Handler::Screen { zone, datas } => {
            debug!("Screen update for zone {}: {:?}", zone, datas);
        }
        Handler::Tactile { zone, mode } => {
            debug!("Tactile event for zone {}: {:?}", zone, mode);
        }
    }
}

/// Compute color from handler and value.
fn compute_color(color: &ColorHandler, value: i32) -> Option<(u8, u8, u8)> {
    match color {
        ColorHandler::Static { red, green, blue } => Some((*red, *green, *blue)),

        ColorHandler::Gradient { gradient } => {
            let t = (value as f32 / 100.0).clamp(0.0, 1.0);
            let r = (gradient.zero.red as f32 * (1.0 - t) + gradient.hundred.red as f32 * t) as u8;
            let g = (gradient.zero.green as f32 * (1.0 - t) + gradient.hundred.green as f32 * t) as u8;
            let b = (gradient.zero.blue as f32 * (1.0 - t) + gradient.hundred.blue as f32 * t) as u8;
            Some((r, g, b))
        }

        ColorHandler::Range { color: ranges } => {
            for range in ranges {
                if value >= range.low && value <= range.high {
                    return Some((range.color.red, range.color.green, range.color.blue));
                }
            }
            None
        }
    }
}

/// Handle heartbeat.
async fn heartbeat(State(_state): State<AppState>, Json(hb): Json<Heartbeat>) -> (StatusCode, Json<ApiResponse>) {
    debug!("Heartbeat from: {}", hb.game);
    (StatusCode::OK, Json(ApiResponse::success()))
}

/// Remove a game.
async fn remove_game(State(state): State<AppState>, Json(req): Json<RemoveGame>) -> (StatusCode, Json<ApiResponse>) {
    let mut state = state.write();

    info!("Removing game: {}", req.game);

    state.games.remove(&req.game);
    state.bindings.remove(&req.game);
    state.event_values.remove(&req.game);

    (StatusCode::OK, Json(ApiResponse::success()))
}

/// Remove an event.
async fn remove_event(State(state): State<AppState>, Json(req): Json<RemoveEvent>) -> (StatusCode, Json<ApiResponse>) {
    let mut state = state.write();

    debug!("Removing event: {}:{}", req.game, req.event);

    if let Some(game_bindings) = state.bindings.get_mut(&req.game) {
        game_bindings.remove(&req.event);
    }

    (StatusCode::OK, Json(ApiResponse::success()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_color_static() {
        let handler = ColorHandler::Static {
            red: 255,
            green: 0,
            blue: 0,
        };

        let (r, g, b) = compute_color(&handler, 50).unwrap();
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_compute_color_gradient() {
        let handler = ColorHandler::Gradient {
            gradient: GradientSpec {
                zero: ColorSpec {
                    red: 255,
                    green: 0,
                    blue: 0,
                },
                hundred: ColorSpec {
                    red: 0,
                    green: 255,
                    blue: 0,
                },
            },
        };

        // At value 0, should be red
        let (r, g, b) = compute_color(&handler, 0).unwrap();
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);

        // At value 100, should be green
        let (r, g, b) = compute_color(&handler, 100).unwrap();
        assert_eq!(r, 0);
        assert_eq!(g, 255);
        assert_eq!(b, 0);

        // At value 50, should be blend
        let (r, g, b) = compute_color(&handler, 50).unwrap();
        assert!(r > 100 && r < 150);
        assert!(g > 100 && g < 150);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_compute_color_range() {
        let handler = ColorHandler::Range {
            color: vec![
                RangeColor {
                    low: 0,
                    high: 25,
                    color: ColorSpec {
                        red: 255,
                        green: 0,
                        blue: 0,
                    },
                },
                RangeColor {
                    low: 26,
                    high: 75,
                    color: ColorSpec {
                        red: 255,
                        green: 255,
                        blue: 0,
                    },
                },
                RangeColor {
                    low: 76,
                    high: 100,
                    color: ColorSpec {
                        red: 0,
                        green: 255,
                        blue: 0,
                    },
                },
            ],
        };

        // Test first range
        let (r, g, b) = compute_color(&handler, 10).unwrap();
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);

        // Test second range
        let (r, g, b) = compute_color(&handler, 50).unwrap();
        assert_eq!(r, 255);
        assert_eq!(g, 255);
        assert_eq!(b, 0);

        // Test third range
        let (r, g, b) = compute_color(&handler, 90).unwrap();
        assert_eq!(r, 0);
        assert_eq!(g, 255);
        assert_eq!(b, 0);

        // Test value outside ranges
        assert!(compute_color(&handler, 150).is_none());
    }

    #[test]
    #[cfg(unix)]
    fn test_secure_json_write() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_gamesense_secure");
        let target_path = temp_dir.join("coreProps.json");

        // Cleanup
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir)?;
        }

        // Write content
        let props = serde_json::json!({ "test": true });

        // This should create the directory securely and write the file
        GameSenseServer::write_secure_json(&target_path, &props)?;

        // Verify directory
        let metadata = std::fs::symlink_metadata(&temp_dir)?;
        assert!(metadata.is_dir());
        assert_eq!(metadata.uid(), rustix::process::getuid().as_raw());
        // Verify secure permissions (0o700)
        assert_eq!(metadata.permissions().mode() & 0o777, 0o700);

        // Verify file
        let file_metadata = std::fs::symlink_metadata(&target_path)?;
        // Verify secure permissions (0o600)
        assert_eq!(file_metadata.permissions().mode() & 0o777, 0o600);
        assert!(!file_metadata.file_type().is_symlink());

        // Cleanup
        std::fs::remove_dir_all(&temp_dir)?;

        Ok(())
    }
}
