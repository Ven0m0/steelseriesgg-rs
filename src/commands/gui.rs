//! `ssgg gui`: serve the profile editor for a browser. The windowed app is `ssgg-gui` (feature `desktop`).

use steelseries_gg::Result;
use steelseries_gg::devices::DeviceManager;
use steelseries_gg::gui::{open_first_keyboard, serve};

pub(crate) async fn cmd_gui(manager: &DeviceManager, port: u16) -> Result<()> {
    if manager
        .first_device_of_type(steelseries_gg::devices::DeviceType::Keyboard)
        .is_some()
    {
        crate::warn_if_gg_engine_running();
    }
    let keyboard = open_first_keyboard(manager);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    println!("Profile editor: http://127.0.0.1:{}/", listener.local_addr()?.port());
    match keyboard.as_ref() {
        Some(k) => println!("Live lighting drives: {}", k.name()),
        None => println!("No keyboard found: the editor works, the live lighting panel stays hidden"),
    }
    serve(listener, keyboard).await
}
