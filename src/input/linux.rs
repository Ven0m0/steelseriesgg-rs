//! Linux layer: grab SteelSeries keyboard evdev nodes, run events through the [`Remapper`] and
//! re-emit them through uinput. [EXPERIMENTAL] Never run against real hardware.
//!
//! Safety model: every grab lives in a [`Grabbed`] whose `Drop` ungrabs, and the session releases
//! every virtual key and ungrabs on each exit path (stop flag, escape chord, error, drop). Grabs
//! are also released by the kernel when the file descriptor closes, e.g. if the process dies.
//! Hot-plug is not handled: a vanished device ends the session with an error.

use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, Device, EventType, InputEvent, KeyCode};
use rustix::event::{PollFd, PollFlags, Timespec};
use tracing::{info, warn};

use super::binding::BindingSet;
use super::keys::InputKey;
use super::remapper::{Effect, KeyEvent, KeyValue, Remapper, RemapperConfig};
use crate::STEELSERIES_VENDOR_ID;
use crate::error::{Error, Result};

pub const VIRTUAL_DEVICE_NAME: &str = "ssgg virtual keyboard";

/// Longest the loop sleeps in poll, so the stop flag is noticed promptly.
const POLL_SLICE: Duration = Duration::from_millis(100);
/// How long to wait for the user to let go of keys (e.g. Enter that launched the command) before grabbing.
const RELEASE_WAIT: Duration = Duration::from_secs(5);
/// Time for the desktop to notice the new virtual device before the first event is emitted.
const VIRTUAL_DEVICE_SETTLE: Duration = Duration::from_millis(300);

/// True when a device is a SteelSeries keyboard node we should grab.
fn is_target(vendor: u16, product: u16, has_letter_keys: bool, product_filter: Option<u16>) -> bool {
    vendor == STEELSERIES_VENDOR_ID && has_letter_keys && product_filter.is_none_or(|p| p == product)
}

/// Translate a raw evdev event into a key event; anything but a key press, release or repeat is dropped.
fn translate(event_type: u16, code: u16, value: i32) -> Option<KeyEvent> {
    if event_type != EventType::KEY.0 {
        return None;
    }
    Some(KeyEvent {
        key: InputKey::from_code(code),
        value: KeyValue::from_evdev(value)?,
    })
}

fn to_input_events(effects: &[Effect]) -> Vec<InputEvent> {
    effects
        .iter()
        .map(|e| InputEvent::new(EventType::KEY.0, e.key.code(), e.value.to_evdev()))
        .collect()
}

fn comm_error(context: &str, e: io::Error) -> Error {
    if e.kind() == io::ErrorKind::PermissionDenied {
        Error::PermissionDenied(format!(
            "{context}: {e} (install assets/70-steelseries.rules and assets/ssgg-uinput.conf, and join the input group)"
        ))
    } else {
        Error::DeviceCommunication(format!("{context}: {e}"))
    }
}

/// A grabbed input device; ungrabs on drop.
struct Grabbed {
    path: PathBuf,
    device: Device,
}

impl Drop for Grabbed {
    fn drop(&mut self) {
        if let Err(e) = self.device.ungrab() {
            warn!(
                "cannot ungrab {}: {e} (released when the device closes)",
                self.path.display()
            );
        }
    }
}

struct Session {
    devices: Vec<Grabbed>,
    virt: VirtualDevice,
    remapper: Remapper,
}

impl Session {
    fn emit(&mut self, effects: &[Effect]) -> Result<()> {
        if effects.is_empty() {
            return Ok(());
        }
        // emit() terminates the batch with SYN_REPORT.
        self.virt
            .emit(&to_input_events(effects))
            .map_err(|e| comm_error("writing to the virtual device", e))
    }

    fn pump(&mut self, stop: &AtomicBool) -> Result<()> {
        while !stop.load(Ordering::SeqCst) && self.remapper.is_active() {
            let timeout = self.remapper.next_deadline().map_or(POLL_SLICE, |d| {
                d.saturating_duration_since(Instant::now()).min(POLL_SLICE)
            });
            for index in poll_ready(&self.devices, timeout)? {
                let mut effects = Vec::new();
                let path = self.devices[index].path.clone();
                let events = self.devices[index]
                    .device
                    .fetch_events()
                    .map_err(|e| comm_error(&format!("reading {}", path.display()), e))?;
                let now = Instant::now();
                for event in events {
                    if let Some(key_event) = translate(event.event_type().0, event.code(), event.value()) {
                        effects.extend(self.remapper.handle(key_event, now));
                    }
                }
                self.emit(&effects)?;
            }
            let fired = self.remapper.tick(Instant::now());
            self.emit(&fired)?;
            if !self.remapper.is_active() {
                info!("escape chord held: stopping");
            }
        }
        Ok(())
    }

    /// Release every virtual key, then every grab. Safe to call more than once.
    fn shutdown(&mut self) {
        let released = self.remapper.release_all();
        if let Err(e) = self.emit(&released) {
            warn!("cannot release keys on the virtual device: {e}");
        }
        // Dropping a Grabbed ungrabs it.
        self.devices.clear();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Indices of devices with data to read. A hung-up or errored device is an error.
fn poll_ready(devices: &[Grabbed], timeout: Duration) -> Result<Vec<usize>> {
    let mut fds: Vec<PollFd<'_>> = devices.iter().map(|g| PollFd::new(&g.device, PollFlags::IN)).collect();
    let timespec = Timespec::try_from(timeout).map_err(|e| Error::Other(format!("invalid poll timeout: {e}")))?;
    match rustix::event::poll(&mut fds, Some(&timespec)) {
        Ok(_) => {}
        Err(rustix::io::Errno::INTR) => return Ok(Vec::new()),
        Err(e) => return Err(comm_error("polling input devices", io::Error::from(e))),
    }
    let mut ready = Vec::new();
    for (i, fd) in fds.iter().enumerate() {
        let revents = fd.revents();
        if revents.intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) {
            return Err(Error::DeviceCommunication(format!(
                "{} went away",
                devices[i].path.display()
            )));
        }
        if !revents.is_empty() {
            ready.push(i);
        }
    }
    Ok(ready)
}

fn find_targets(product_filter: Option<u16>) -> Vec<(PathBuf, Device)> {
    evdev::enumerate()
        .filter(|(_, device)| {
            let id = device.input_id();
            let has_letters = device
                .supported_keys()
                .is_some_and(|keys| keys.contains(KeyCode::KEY_A) && keys.contains(KeyCode::KEY_Z));
            is_target(id.vendor(), id.product(), has_letters, product_filter)
        })
        .collect()
}

fn wait_until_no_keys_held(device: &Device, path: &std::path::Path, stop: &AtomicBool) -> Result<()> {
    let start = Instant::now();
    loop {
        let state = device
            .get_key_state()
            .map_err(|e| comm_error(&format!("reading key state of {}", path.display()), e))?;
        if state.iter().next().is_none() {
            return Ok(());
        }
        if stop.load(Ordering::SeqCst) || start.elapsed() >= RELEASE_WAIT {
            return Err(Error::DeviceCommunication(format!(
                "keys are still held on {}; release them and run again",
                path.display()
            )));
        }
        sleep(Duration::from_millis(20));
    }
}

fn build_virtual_device(targets: &[(PathBuf, Device)], bindings: &BindingSet) -> Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    for (_, device) in targets {
        if let Some(supported) = device.supported_keys() {
            for key in supported.iter() {
                keys.insert(key);
            }
        }
    }
    for key in bindings.output_keys() {
        keys.insert(KeyCode::new(key.code()));
    }
    VirtualDevice::builder()
        .and_then(|b| b.name(VIRTUAL_DEVICE_NAME).with_keys(&keys))
        .and_then(|b| b.build())
        .map_err(|e| comm_error("creating the uinput device (is the uinput module loaded?)", e))
}

/// Grab the matching keyboards and remap until `stop` is set, the escape chord fires or an error
/// occurs. Blocks; run it on a dedicated thread. Every grab is released before it returns.
pub fn run(bindings: &BindingSet, product_filter: Option<u16>, stop: &AtomicBool) -> Result<()> {
    let remapper = Remapper::new(bindings, RemapperConfig::default())?;

    let targets = find_targets(product_filter);
    if targets.is_empty() {
        return Err(Error::DeviceNotFound(
            "no readable SteelSeries keyboard input node (check the udev rules and the input group)".to_string(),
        ));
    }

    let virt = build_virtual_device(&targets, bindings)?;
    sleep(VIRTUAL_DEVICE_SETTLE);

    let mut session = Session {
        devices: Vec::new(),
        virt,
        remapper,
    };
    for (path, mut device) in targets {
        wait_until_no_keys_held(&device, &path, stop)?;
        device.grab().map_err(|e| {
            comm_error(
                &format!("grabbing {} (is another program capturing it?)", path.display()),
                e,
            )
        })?;
        info!("grabbed {} ({})", device.name().unwrap_or("unnamed"), path.display());
        session.devices.push(Grabbed { path, device });
    }

    let result = session.pump(stop);
    session.shutdown();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_needs_vendor_letters_and_matching_product() {
        assert!(is_target(0x1038, 0x1628, true, None));
        assert!(is_target(0x1038, 0x1628, true, Some(0x1628)));
        assert!(!is_target(0x1038, 0x1628, true, Some(0x1612)));
        assert!(!is_target(0x1038, 0x1628, false, None));
        assert!(!is_target(0x046d, 0x1628, true, None));
    }

    #[test]
    fn translate_keeps_only_key_events_with_known_values() {
        let ev = translate(EventType::KEY.0, 30, 1).unwrap();
        assert_eq!(ev.key.code(), 30);
        assert_eq!(ev.value, KeyValue::Press);
        assert_eq!(translate(EventType::KEY.0, 30, 2).unwrap().value, KeyValue::Repeat);
        assert!(translate(EventType::KEY.0, 30, 7).is_none());
        assert!(translate(EventType::MISC.0, 4, 30).is_none());
        assert!(translate(EventType::SYNCHRONIZATION.0, 0, 0).is_none());
    }

    #[test]
    fn effects_become_key_input_events() {
        let effects = [
            KeyEvent {
                key: InputKey::from_code(1),
                value: KeyValue::Press,
            },
            KeyEvent {
                key: InputKey::from_code(1),
                value: KeyValue::Release,
            },
        ];
        let events = to_input_events(&effects);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type(), EventType::KEY);
        assert_eq!((events[0].code(), events[0].value()), (1, 1));
        assert_eq!((events[1].code(), events[1].value()), (1, 0));
    }
}
