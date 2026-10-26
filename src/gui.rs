//! The server behind `ssgg-gui`: the profile editor in `tools/profile-gui` and the small lighting API its live panel
//! calls.
//!
//! Everything listens on 127.0.0.1 only. The GameSense server is deliberately not reused: it is a game-facing
//! endpoint with its own CORS rule, while this one needs no CORS at all because the page is served from the
//! same origin. A browser page on another origin cannot read the token, and a request that arrives under a
//! foreign `Host` (DNS rebinding) or `Origin` is refused before the token is looked at.
//!
//! `[EXPERIMENTAL]` The lighting calls are the ones `ssgg rgb` sends; only a solid colour on the wired Apex Pro
//! TKL 2023 (`0x1628`) is confirmed on hardware, so a keyboard that ignores a call still answers success.
//!
//! `[EXPERIMENTAL]` The headset calls are the ones `ssgg headset` sends, and a value the page asks to persist
//! goes to `[headset]` in `config.toml` like `--persistent` does.

use std::collections::hash_map::RandomState;
use std::fmt::Write as _;
use std::hash::{BuildHasher, Hasher};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use parking_lot::Mutex;
use serde::Deserialize;
use tracing::{info, warn};

use crate::config::{Config, HeadsetConfig};
use crate::devices::headsets::{Headset, HeadsetCapabilities};
use crate::devices::keyboards::Keyboard;
use crate::devices::{DeviceManager, DeviceType};
use crate::rgb::Color;
use crate::{Error, Result};

const INDEX_HTML: &str = include_str!("../tools/profile-gui/index.html");
const APP_JS: &str = include_str!("../tools/profile-gui/app.js");
const LIVE_JS: &str = include_str!("../tools/profile-gui/live.js");
const MAIN_JS: &str = include_str!("../tools/profile-gui/main.js");
const PRODUCTS_JS: &str = include_str!("../tools/profile-gui/products.generated.js");
const STYLE_CSS: &str = include_str!("../tools/profile-gui/style.css");

const CONTROL_JS: &str = include_str!("../tools/profile-gui/control.js");
const HEADSET_JS: &str = include_str!("../tools/profile-gui/headset.js");

/// Subcommands the control panel may run. Left out because they never return or reopen this server:
/// `gui`, `server`, `daemon`, `status`, `hid-logs`, `fuzz`; `remap` is allowed for `list` only (`run` grabs the keyboard).
const CLI_COMMANDS: &[&str] = &[
    "devices",
    "rgb",
    "actuation",
    "oled",
    "headset",
    "mouse",
    "profile",
    "audio",
    "sonar",
    "pollrate",
    "onboard",
    "validate",
    "performance",
    "bug-report",
    "test-device",
    "verify-performance",
];
const CLI_TIMEOUT: Duration = Duration::from_secs(60);
const CLI_OUTPUT_MAX: u64 = 256 * 1024;

const TOKEN_HEADER: &str = "x-ssgg-token";
const JS: &str = "text/javascript; charset=utf-8";

/// The keyboard the live panel drives. One handle for the whole run: the HID collection is opened once.
pub struct KeyboardHandle {
    name: String,
    device: Mutex<Box<dyn Keyboard>>,
}

impl KeyboardHandle {
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// The headset the headset tab drives, opened once for the whole run like [`KeyboardHandle`].
pub struct HeadsetHandle {
    name: String,
    product_id: u16,
    capabilities: HeadsetCapabilities,
    device: Mutex<Box<dyn Headset>>,
}

impl HeadsetHandle {
    pub fn name(&self) -> &str {
        &self.name
    }
}

struct GuiState {
    port: u16,
    token: String,
    /// `index.html` with the token in a `<meta>` tag: the page learns the token only by being served here.
    index: String,
    keyboard: Option<KeyboardHandle>,
    headset: Option<HeadsetHandle>,
}

#[derive(Deserialize)]
struct ColorBody {
    color: String,
}

#[derive(Deserialize)]
struct BrightnessBody {
    level: u8,
}

#[derive(Deserialize)]
struct HeadsetBody {
    value: u8,
    /// Keep the value in `config.toml`, where the daemon re-applies it whenever the headset powers on. `false`
    /// removes a saved value: the page's checkbox shows whether one is saved.
    #[serde(default)]
    persist: bool,
}

#[derive(Clone, Copy)]
enum HeadsetSetting {
    Sidetone,
    MicVolume,
    AutoOff,
}

impl HeadsetSetting {
    fn apply(self, headset: &mut dyn Headset, value: u8) -> Result<()> {
        match self {
            Self::Sidetone => headset.set_sidetone(value),
            Self::MicVolume => headset.set_mic_volume(value),
            Self::AutoOff => headset.set_auto_off(value),
        }
    }

    fn slot(self, config: &mut HeadsetConfig) -> &mut Option<u8> {
        match self {
            Self::Sidetone => &mut config.sidetone,
            Self::MicVolume => &mut config.mic_volume,
            Self::AutoOff => &mut config.auto_off,
        }
    }

    /// Sidetone and mic volume are 0-100 levels; auto-off is minutes, any byte.
    fn accepts(self, value: u8) -> bool {
        matches!(self, Self::AutoOff) || value <= 100
    }
}

enum LightingOp {
    Color(Color),
    Brightness(u8),
}

impl LightingOp {
    async fn apply(self, keyboard: &mut dyn Keyboard) -> Result<()> {
        match self {
            Self::Color(color) => {
                keyboard.set_color(color).await?;
                keyboard.apply().await
            }
            Self::Brightness(level) => keyboard.set_brightness(level).await,
        }
    }
}

/// Open the first keyboard for the live lighting panel; `None` leaves the panel hidden.
pub fn open_first_keyboard(manager: &DeviceManager) -> Option<KeyboardHandle> {
    let info = manager.first_device_of_type(DeviceType::Keyboard)?;
    let mut device = match manager.open_keyboard(info) {
        Ok(device) => device,
        Err(e) => {
            warn!("live lighting unavailable, could not open {}: {e}", info.name);
            return None;
        }
    };
    if let Err(e) = device.initialize() {
        warn!("live lighting unavailable, could not initialise {}: {e}", info.name);
        return None;
    }
    Some(KeyboardHandle {
        name: info.name.to_string(),
        device: Mutex::new(device),
    })
}

/// Open the first headset for the headset tab; `None` leaves the tab hidden.
pub fn open_first_headset(manager: &DeviceManager) -> Option<HeadsetHandle> {
    let info = manager.first_device_of_type(DeviceType::Headset)?;
    let mut device = match manager.open_headset(info) {
        Ok(device) => device,
        Err(e) => {
            warn!("headset tab unavailable, could not open {}: {e}", info.name);
            return None;
        }
    };
    if let Err(e) = device.initialize() {
        warn!("headset tab unavailable, could not initialise {}: {e}", info.name);
        return None;
    }
    Some(HeadsetHandle {
        name: info.name.to_string(),
        product_id: info.product_id,
        capabilities: device.capabilities(),
        device: Mutex::new(device),
    })
}

/// Serve the editor, the lighting API and the headset API on `listener` until the task is dropped or the
/// process stops.
pub async fn serve(
    listener: tokio::net::TcpListener,
    keyboard: Option<KeyboardHandle>,
    headset: Option<HeadsetHandle>,
) -> Result<()> {
    let port = listener.local_addr()?.port();
    let token = new_token();
    let meta = format!("<meta name=\"ssgg-live-token\" content=\"{token}\">\n</head>");
    let state = Arc::new(GuiState {
        port,
        index: INDEX_HTML.replacen("</head>", &meta, 1),
        token,
        keyboard,
        headset,
    });
    info!("profile editor on 127.0.0.1:{port}");
    axum::serve(listener, router(state)).await?;
    Ok(())
}

fn router(state: Arc<GuiState>) -> Router {
    Router::new()
        .route("/api/status", get(status))
        .route("/api/rgb/color", post(color))
        .route("/api/rgb/brightness", post(brightness))
        .route("/api/rgb/off", post(off))
        .route("/api/headset", get(headset_info))
        .route("/api/headset/status", get(headset_status))
        .route("/api/headset/sidetone", post(headset_sidetone))
        .route("/api/headset/mic-volume", post(headset_mic_volume))
        .route("/api/headset/auto-off", post(headset_auto_off))
        .route("/api/cli", post(cli))
        .fallback_service(get(asset).with_state(state.clone()))
        .with_state(state)
}

/// 128 bits from the per-process random keys of std's `RandomState`.
///
/// shortcut: not a CSPRNG; enough to stop a web page from guessing a loopback token, swap for `getrandom` if the
/// token ever guards more than lighting.
fn new_token() -> String {
    let mut token = String::with_capacity(32);
    for salt in 0u64..2 {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(salt);
        // Writing to a String cannot fail.
        let _ = write!(token, "{:016x}", hasher.finish());
    }
    token
}

/// The request names this server, under this port, and was not sent from another origin.
fn same_origin(headers: &HeaderMap, port: u16) -> bool {
    let allowed = |host: &str| {
        host.eq_ignore_ascii_case(&format!("127.0.0.1:{port}"))
            || host.eq_ignore_ascii_case(&format!("localhost:{port}"))
    };
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    if !allowed(host) {
        return false;
    }
    match headers.get(header::ORIGIN) {
        None => true,
        Some(origin) => origin
            .to_str()
            .ok()
            .and_then(|o| o.strip_prefix("http://"))
            .is_some_and(allowed),
    }
}

fn token_matches(headers: &HeaderMap, token: &str) -> bool {
    let Some(given) = headers.get(TOKEN_HEADER).map(|v| v.as_bytes()) else {
        return false;
    };
    let expected = token.as_bytes();
    given.len() == expected.len() && given.iter().zip(expected).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

fn authorized(state: &GuiState, headers: &HeaderMap) -> bool {
    same_origin(headers, state.port) && token_matches(headers, &state.token)
}

fn error_response(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(serde_json::json!({ "error": message.into() }))).into_response()
}

async fn asset(State(state): State<Arc<GuiState>>, headers: HeaderMap, uri: Uri) -> Response {
    if !same_origin(&headers, state.port) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let (content_type, body) = match uri.path() {
        "/" | "/index.html" => ("text/html; charset=utf-8", state.index.as_str()),
        "/app.js" => (JS, APP_JS),
        "/control.js" => (JS, CONTROL_JS),
        "/headset.js" => (JS, HEADSET_JS),
        "/live.js" => (JS, LIVE_JS),
        "/main.js" => (JS, MAIN_JS),
        "/products.generated.js" => (JS, PRODUCTS_JS),
        "/style.css" => ("text/css; charset=utf-8", STYLE_CSS),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    // The index carries the token, so nothing here may be cached.
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store"),
            // The page holds the lighting token: no frames, plugins or <base> rewrites inside it.
            (
                header::CONTENT_SECURITY_POLICY,
                "frame-src 'none'; object-src 'none'; base-uri 'none'",
            ),
        ],
        body.to_owned(),
    )
        .into_response()
}

async fn status(State(state): State<Arc<GuiState>>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    Json(serde_json::json!({
        "keyboard": state.keyboard.as_ref().map(|k| k.name.as_str()),
        "headset": state.headset.as_ref().map(|h| h.name.as_str()),
    }))
    .into_response()
}

const NO_HEADSET: &str = "no headset is connected";

/// The headset's model, what it supports and the values saved for the daemon.
async fn headset_info(State(state): State<Arc<GuiState>>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(headset) = state.headset.as_ref() else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, NO_HEADSET);
    };
    let saved = match Config::load_async().await {
        Ok(config) => Some(config.headset),
        Err(e) => {
            warn!("could not read the saved headset settings: {e}");
            None
        }
    };
    Json(serde_json::json!({
        "name": headset.name,
        "product_id": headset.product_id,
        "capabilities": headset.capabilities,
        "saved": saved,
    }))
    .into_response()
}

async fn headset_status(State(state): State<Arc<GuiState>>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if state.headset.is_none() {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, NO_HEADSET);
    }
    let outcome = tokio::task::spawn_blocking(move || {
        let headset = state
            .headset
            .as_ref()
            .ok_or_else(|| Error::DeviceNotFound(NO_HEADSET.to_string()))?;
        headset.device.lock().status()
    })
    .await;
    match outcome {
        Ok(Ok(status)) => Json(status).into_response(),
        Ok(Err(e)) => error_response(StatusCode::BAD_GATEWAY, e.to_string()),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("headset task failed: {e}")),
    }
}

async fn headset_sidetone(state: State<Arc<GuiState>>, headers: HeaderMap, body: Json<HeadsetBody>) -> Response {
    set_headset(state, headers, body, HeadsetSetting::Sidetone).await
}

async fn headset_mic_volume(state: State<Arc<GuiState>>, headers: HeaderMap, body: Json<HeadsetBody>) -> Response {
    set_headset(state, headers, body, HeadsetSetting::MicVolume).await
}

async fn headset_auto_off(state: State<Arc<GuiState>>, headers: HeaderMap, body: Json<HeadsetBody>) -> Response {
    set_headset(state, headers, body, HeadsetSetting::AutoOff).await
}

/// Send one setting to the headset, then save or forget it for the daemon. `config.toml` changes only after the
/// headset took the value, so it never holds a value the headset refused, and is written only when it changes.
async fn set_headset(
    State(state): State<Arc<GuiState>>,
    headers: HeaderMap,
    Json(body): Json<HeadsetBody>,
    setting: HeadsetSetting,
) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !setting.accepts(body.value) {
        return error_response(
            StatusCode::BAD_REQUEST,
            format!("{} is out of range (0-100)", body.value),
        );
    }
    if state.headset.is_none() {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, NO_HEADSET);
    }
    let outcome = tokio::task::spawn_blocking(move || -> std::result::Result<(), (StatusCode, String)> {
        let headset = state
            .headset
            .as_ref()
            .ok_or((StatusCode::SERVICE_UNAVAILABLE, NO_HEADSET.to_string()))?;
        setting
            .apply(headset.device.lock().as_mut(), body.value)
            .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
        let wanted = body.persist.then_some(body.value);
        let save = Config::load().and_then(|mut config| {
            let slot = setting.slot(&mut config.headset);
            if *slot == wanted {
                return Ok(());
            }
            *slot = wanted;
            config.save()
        });
        save.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("the headset took the value, but updating the saved value in config failed: {e}"),
            )
        })?;
        Ok(())
    })
    .await;
    match outcome {
        Ok(Ok(())) => Json(serde_json::json!({ "ok": true, "saved": body.persist })).into_response(),
        Ok(Err((status, message))) => error_response(status, message),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("headset task failed: {e}")),
    }
}

async fn color(State(state): State<Arc<GuiState>>, headers: HeaderMap, Json(body): Json<ColorBody>) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match Color::parse(body.color.trim().trim_start_matches('#')) {
        Some(color) => run_lighting(state, LightingOp::Color(color)).await,
        None => error_response(
            StatusCode::BAD_REQUEST,
            Error::Other(format!("Invalid color: {}", body.color)).to_string(),
        ),
    }
}

async fn brightness(
    State(state): State<Arc<GuiState>>,
    headers: HeaderMap,
    Json(body): Json<BrightnessBody>,
) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    run_lighting(state, LightingOp::Brightness(body.level.min(100))).await
}

async fn off(State(state): State<Arc<GuiState>>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    run_lighting(state, LightingOp::Color(Color::BLACK)).await
}

/// Run one lighting call on the keyboard's own thread of the blocking pool: the HID writes block, and the
/// keyboard is behind a lock because a call takes `&mut`.
async fn run_lighting(state: Arc<GuiState>, op: LightingOp) -> Response {
    if state.keyboard.is_none() {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "no keyboard is connected");
    }
    let runtime = tokio::runtime::Handle::current();
    let outcome = tokio::task::spawn_blocking(move || -> Result<()> {
        let keyboard = state
            .keyboard
            .as_ref()
            .ok_or_else(|| Error::DeviceNotFound("no keyboard is connected".to_string()))?;
        let mut device = keyboard.device.lock();
        runtime.block_on(op.apply(device.as_mut()))
    })
    .await;
    match outcome {
        Ok(Ok(())) => Json(serde_json::json!({ "ok": true })).into_response(),
        Ok(Err(e)) => error_response(StatusCode::BAD_GATEWAY, e.to_string()),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("lighting task failed: {e}")),
    }
}

#[derive(Deserialize)]
struct CliBody {
    args: Vec<String>,
}

fn cli_allowed(args: &[String]) -> bool {
    match args.first().map(String::as_str) {
        Some("remap") => args.get(1).map(String::as_str) == Some("list"),
        Some(command) => CLI_COMMANDS.contains(&command),
        None => false,
    }
}

/// The `ssgg` binary: the sibling of `ssgg-gui` in the desktop app.
fn ssgg_exe() -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?;
    if me.file_stem()? == "ssgg" {
        return Some(me);
    }
    let sibling = me
        .with_file_name("ssgg")
        .with_extension(std::env::consts::EXE_EXTENSION);
    sibling.is_file().then_some(sibling)
}

fn drain(pipe: impl std::io::Read + Send + 'static) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        // A read error keeps what arrived so far; the exit status carries the verdict.
        let _ = pipe.take(CLI_OUTPUT_MAX).read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// Run `ssgg <args>` to completion or kill it after `CLI_TIMEOUT`.
fn run_cli(exe: &Path, args: &[String]) -> std::io::Result<serde_json::Value> {
    let mut command = Command::new(exe);
    command
        .args(args)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        // CREATE_NO_WINDOW: the windowed desktop app must not flash a console per call.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().map(drain);
    let stderr = child.stderr.take().map(drain);
    let started = Instant::now();
    let (code, timed_out) = loop {
        if let Some(status) = child.try_wait()? {
            break (status.code(), false);
        }
        if started.elapsed() >= CLI_TIMEOUT {
            // Already exited between the check and the kill is fine: wait() reaps it either way.
            let _ = child.kill();
            let _ = child.wait();
            break (None, true);
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let text = |handle: Option<std::thread::JoinHandle<String>>| handle.and_then(|h| h.join().ok()).unwrap_or_default();
    Ok(serde_json::json!({
        "ok": code == Some(0),
        "code": code,
        "timed_out": timed_out,
        "stdout": text(stdout),
        "stderr": text(stderr),
    }))
}

/// Run one `ssgg` command for the control panel, so the GUI offers whatever the CLI does.
async fn cli(State(state): State<Arc<GuiState>>, headers: HeaderMap, Json(body): Json<CliBody>) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !cli_allowed(&body.args) {
        return error_response(StatusCode::BAD_REQUEST, "that command is not available from the GUI");
    }
    let Some(exe) = ssgg_exe() else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "the ssgg binary was not found");
    };
    match tokio::task::spawn_blocking(move || run_cli(&exe, &body.args)).await {
        Ok(Ok(result)) => Json(result).into_response(),
        Ok(Err(e)) => error_response(StatusCode::BAD_GATEWAY, format!("could not run ssgg: {e}")),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("command task failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_listed_subcommands_run_and_remap_only_lists() {
        let args = |parts: &[&str]| parts.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(cli_allowed(&args(&["actuation", "set", "1.0"])));
        assert!(cli_allowed(&args(&["remap", "list"])));
        assert!(!cli_allowed(&args(&["remap", "run"])));
        for blocked in ["gui", "daemon", "server", "status", "hid-logs", "fuzz", "--debug"] {
            assert!(!cli_allowed(&args(&[blocked])), "{blocked}");
        }
        assert!(!cli_allowed(&[]));
    }

    use axum::http::HeaderValue;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, HeaderValue::from_str(value).expect("header value"));
        }
        map
    }

    #[test]
    fn same_origin_accepts_only_this_server_under_this_port() {
        assert!(same_origin(&headers(&[("host", "127.0.0.1:8765")]), 8765));
        assert!(same_origin(
            &headers(&[("host", "localhost:8765"), ("origin", "http://localhost:8765")]),
            8765
        ));
        assert!(!same_origin(&headers(&[]), 8765), "no Host");
        assert!(
            !same_origin(&headers(&[("host", "evil.example:8765")]), 8765),
            "DNS rebinding"
        );
        assert!(
            !same_origin(&headers(&[("host", "127.0.0.1:9999")]), 8765),
            "another port"
        );
        assert!(!same_origin(
            &headers(&[("host", "127.0.0.1:8765"), ("origin", "https://evil.example")]),
            8765
        ));
        assert!(
            !same_origin(&headers(&[("host", "127.0.0.1:8765"), ("origin", "null")]), 8765),
            "file:// page"
        );
    }

    #[test]
    fn token_must_match_exactly() {
        assert!(token_matches(&headers(&[(TOKEN_HEADER, "abc")]), "abc"));
        assert!(!token_matches(&headers(&[(TOKEN_HEADER, "abd")]), "abc"));
        assert!(!token_matches(&headers(&[(TOKEN_HEADER, "ab")]), "abc"));
        assert!(!token_matches(&headers(&[]), "abc"));
    }

    #[test]
    fn tokens_are_32_hex_digits_and_differ() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 32);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    fn request(port: u16, head: &str, body: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let text = format!(
            "{head}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(text.as_bytes()).expect("send");
        let mut reply = String::new();
        stream.read_to_string(&mut reply).expect("read");
        reply
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_api_needs_the_token_the_page_is_served_with_and_a_keyboard() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        tokio::spawn(serve(listener, None, None));
        let host = format!("Host: 127.0.0.1:{port}");

        let (page, foreign, no_token, ok_token, bad_color) = tokio::task::spawn_blocking(move || {
            let page = request(port, &format!("GET / HTTP/1.1\r\n{host}"), "");
            let token = page
                .split("name=\"ssgg-live-token\" content=\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .unwrap_or_default()
                .to_string();
            let json = "Content-Type: application/json";
            let foreign = request(port, "GET /api/status HTTP/1.1\r\nHost: evil.example", "");
            let no_token = request(port, &format!("POST /api/rgb/off HTTP/1.1\r\n{host}\r\n{json}"), "");
            let ok_token = request(
                port,
                &format!("POST /api/rgb/off HTTP/1.1\r\n{host}\r\n{json}\r\n{TOKEN_HEADER}: {token}"),
                "",
            );
            let bad_color = request(
                port,
                &format!("POST /api/rgb/color HTTP/1.1\r\n{host}\r\n{json}\r\n{TOKEN_HEADER}: {token}"),
                "{\"color\":\"not-a-colour\"}",
            );
            (page, foreign, no_token, ok_token, bad_color)
        })
        .await
        .expect("requests");

        assert!(page.starts_with("HTTP/1.1 200"), "{page}");
        assert!(page.contains("ssgg-live-token"));
        assert!(foreign.starts_with("HTTP/1.1 403"), "{foreign}");
        assert!(no_token.starts_with("HTTP/1.1 403"), "{no_token}");
        assert!(
            ok_token.starts_with("HTTP/1.1 503"),
            "authorised, but no keyboard: {ok_token}"
        );
        assert!(bad_color.starts_with("HTTP/1.1 400"), "{bad_color}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_headset_api_needs_the_token_a_level_in_range_and_a_headset() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        tokio::spawn(serve(listener, None, None));
        let host = format!("Host: 127.0.0.1:{port}");

        let replies = tokio::task::spawn_blocking(move || {
            let page = request(port, &format!("GET / HTTP/1.1\r\n{host}"), "");
            let token = page
                .split("name=\"ssgg-live-token\" content=\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .unwrap_or_default()
                .to_string();
            let json = "Content-Type: application/json";
            let authed = format!("{host}\r\n{json}\r\n{TOKEN_HEADER}: {token}");
            [
                request(
                    port,
                    &format!("POST /api/headset/sidetone HTTP/1.1\r\n{host}\r\n{json}"),
                    "{\"value\":0}",
                ),
                request(
                    port,
                    &format!("POST /api/headset/sidetone HTTP/1.1\r\n{authed}"),
                    "{\"value\":101}",
                ),
                request(
                    port,
                    &format!("POST /api/headset/mic-volume HTTP/1.1\r\n{authed}"),
                    "{\"value\":50}",
                ),
                request(
                    port,
                    &format!("POST /api/headset/auto-off HTTP/1.1\r\n{authed}"),
                    "{\"value\":200}",
                ),
                request(port, &format!("GET /api/headset HTTP/1.1\r\n{authed}"), ""),
                request(port, &format!("GET /api/headset/status HTTP/1.1\r\n{authed}"), ""),
                request(port, &format!("GET /api/status HTTP/1.1\r\n{authed}"), ""),
            ]
        })
        .await
        .expect("requests");
        let [no_token, too_loud, mic, auto_off, info, status, overview] = replies;

        assert!(no_token.starts_with("HTTP/1.1 403"), "{no_token}");
        assert!(too_loud.starts_with("HTTP/1.1 400"), "sidetone is 0-100: {too_loud}");
        for reply in [&mic, &auto_off, &info, &status] {
            assert!(reply.starts_with("HTTP/1.1 503"), "authorised, but no headset: {reply}");
        }
        assert!(overview.contains("\"headset\":null"), "{overview}");
    }
}
