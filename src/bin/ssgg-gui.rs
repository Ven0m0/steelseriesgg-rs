//! `ssgg-gui`: the profile editor in a native window (feature `desktop`).
//!
//! The editor and its lighting API come from `steelseries_gg::gui`; this binary runs that server on an ephemeral
//! loopback port and points a webview at it: WebView2 on Windows, WebKitGTK on Linux (GTK picks Wayland itself).

// A console window would sit next to the app when launched from a shortcut.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::time::Duration;

use anyhow::{Context, Result};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::platform::run_return::EventLoopExtRunReturn;
use tao::window::WindowBuilder;
use tracing::{error, info};
use wry::{NewWindowResponse, WebViewBuilder};

use steelseries_gg::config::Config;
use steelseries_gg::devices::DeviceManager;
use steelseries_gg::gui::{open_first_headset, open_first_keyboard, serve};

/// The page, or a `blob:` URL the page made itself (the "Save a copy" download). Anything else stays out of a
/// window that holds the live-lighting token.
fn is_own_origin(uri: &str, origin: &str) -> bool {
    let uri = uri.strip_prefix("blob:").unwrap_or(uri);
    uri.strip_prefix(origin)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '?', '#']))
}

fn main() -> Result<()> {
    // shortcut: a windowed Windows build has no console, so start-up failures reach only this subscriber's
    // output, which nobody sees; add a native error dialog if that proves a problem.
    tracing::subscriber::set_global_default(tracing_subscriber::FmtSubscriber::new())
        .context("could not install the log subscriber")?;

    // The event loop must own the main thread (GTK requires it), so no `#[tokio::main]`.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("could not start the async runtime")?;

    let mut manager = DeviceManager::new().context("could not enumerate devices")?;
    if let Ok(config) = Config::load() {
        manager.set_device_override(config.device);
    }
    let keyboard = open_first_keyboard(&manager);
    let headset = open_first_headset(&manager);

    let listener = runtime
        .block_on(tokio::net::TcpListener::bind(("127.0.0.1", 0)))
        .context("could not bind a loopback port")?;
    let port = listener.local_addr()?.port();
    runtime.spawn(async move {
        if let Err(e) = serve(listener, keyboard, headset).await {
            error!("editor server stopped: {e}");
        }
    });
    let origin = format!("http://127.0.0.1:{port}");
    info!("editor at {origin}");

    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("ssgg")
        .with_inner_size(LogicalSize::new(1400.0, 900.0))
        .build(&event_loop)
        .context("could not create the window")?;

    let builder = WebViewBuilder::new()
        .with_url(format!("{origin}/"))
        .with_navigation_handler(move |uri| is_own_origin(&uri, &origin))
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        // wry turns devtools on in debug builds; they would show the token.
        .with_devtools(false);
    // `build(&window)` is X11-only on Linux; the GTK container works on Wayland and X11.
    #[cfg(target_os = "linux")]
    let _webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().context("the window has no GTK container")?;
        builder.build_gtk(vbox)
    }
    .context("could not create the webview")?;
    #[cfg(not(target_os = "linux"))]
    let _webview = builder.build(&window).context("could not create the webview")?;

    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });

    // Drops the keyboard handle with the server task; the HID writes in flight are short.
    runtime.shutdown_timeout(Duration::from_secs(2));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_own_origin;

    const ORIGIN: &str = "http://127.0.0.1:8765";

    #[test]
    fn only_the_own_origin_and_its_blobs_are_allowed() {
        assert!(is_own_origin("http://127.0.0.1:8765/", ORIGIN));
        assert!(is_own_origin("http://127.0.0.1:8765", ORIGIN));
        assert!(is_own_origin("http://127.0.0.1:8765/api/status?x=1", ORIGIN));
        assert!(is_own_origin("blob:http://127.0.0.1:8765/1b2c", ORIGIN));

        assert!(!is_own_origin("http://127.0.0.1:87650/", ORIGIN));
        assert!(!is_own_origin("http://127.0.0.1:8765.evil.example/", ORIGIN));
        assert!(!is_own_origin("http://127.0.0.1:8765@evil.example/", ORIGIN));
        assert!(!is_own_origin("https://example.com/", ORIGIN));
        assert!(!is_own_origin("blob:https://example.com/1b2c", ORIGIN));
        assert!(!is_own_origin("file:///etc/passwd", ORIGIN));
    }
}
