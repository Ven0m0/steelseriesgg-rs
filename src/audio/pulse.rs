#[cfg(feature = "audio")]
use libpulse_binding as pulse;
#[cfg(feature = "audio")]
use pulse::context::{Context, FlagSet as ContextFlagSet};
#[cfg(feature = "audio")]
use pulse::mainloop::threaded::Mainloop;
#[cfg(feature = "audio")]
use pulse::volume::{ChannelVolumes, VolumeLinear};
#[cfg(feature = "audio")]
use parking_lot::Mutex;
#[cfg(feature = "audio")]
use std::sync::Arc;
#[cfg(feature = "audio")]
use std::time::Duration;

#[cfg(feature = "audio")]
use crate::{Error, Result};

#[cfg(feature = "audio")]
pub struct PulseHandler {
    mainloop: Mainloop,
    context: Arc<Mutex<Context>>,
}

#[cfg(feature = "audio")]
enum ListMsg {
    Item(u32, u8), // index, channels
    End,
}

#[cfg(feature = "audio")]
impl PulseHandler {
    pub fn connect() -> Result<Self> {
        let mut mainloop =
            Mainloop::new().ok_or(Error::Audio("Failed to create mainloop".into()))?;
        let mut context = Context::new(&mainloop, "SteelSeries GG Linux")
            .ok_or(Error::Audio("Failed to create context".into()))?;

        context
            .connect(None, ContextFlagSet::empty(), None)
            .map_err(|e| Error::Audio(format!("Failed to connect context: {}", e)))?;

        mainloop
            .start()
            .map_err(|e| Error::Audio(format!("Failed to start mainloop: {}", e)))?;

        // Wait for connection with timeout
        let start = std::time::Instant::now();
        loop {
            match context.get_state() {
                pulse::context::State::Ready => break,
                pulse::context::State::Failed | pulse::context::State::Terminated => {
                    mainloop.stop();
                    return Err(Error::Audio("Context connection failed".into()));
                }
                _ => {
                    if start.elapsed() > Duration::from_secs(5) {
                        mainloop.stop();
                        return Err(Error::Audio("Context connection timed out".into()));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }

        Ok(Self {
            mainloop,
            context: Arc::new(Mutex::new(context)),
        })
    }

    /// Set volume for the default sink (Master).
    pub fn set_master_volume(&mut self, volume: f32) -> Result<()> {
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
        let tx = Arc::new(Mutex::new(tx));

        self.mainloop.lock();
        {
            let context_guard = self.context.lock();
            let introspector = context_guard.introspect();
            let tx_clone = tx.clone();
            introspector.get_server_info(move |info| {
                let default_sink = info.default_sink_name.as_ref().map(|s| s.to_string());
                let _ = tx_clone.lock().send(default_sink);
            });
        }
        self.mainloop.unlock();

        let default_sink_name = rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| Error::Audio("Timeout getting server info".into()))?
            .ok_or(Error::Audio("Failed to get server info".into()))?;

        // default_sink_name is String
        self.set_sink_volume_by_name(&default_sink_name, volume)?;

        Ok(())
    }

    /// Set volume for a specific sink by name.
    fn set_sink_volume_by_name(&mut self, sink_name: &str, volume: f32) -> Result<()> {
        let (tx, rx) = std::sync::mpsc::channel::<Option<u8>>();
        let tx = Arc::new(Mutex::new(tx));

        self.mainloop.lock();
        {
            let context_guard = self.context.lock();
            let introspector = context_guard.introspect();
            let tx_clone = tx.clone();
            introspector.get_sink_info_by_name(sink_name, move |info| {
                match info {
                    pulse::callbacks::ListResult::Item(item) => {
                        let _ = tx_clone.lock().send(Some(item.channel_map.len()));
                    }
                    _ => {
                        // If it's End without Item, we didn't find it?
                        // get_by_name usually returns Item if found.
                    }
                }
            });
        }
        self.mainloop.unlock();

        // Note: we might receive nothing if not found.
        let channels = rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| Error::Audio("Timeout getting sink info".into()))?
            .ok_or(Error::Audio(format!("Sink {} not found", sink_name)))?;

        let vol_linear = VolumeLinear(volume as f64);
        let mut cv = ChannelVolumes::default();
        cv.set_len(channels);
        for i in 0..channels {
            cv.set(i, vol_linear.into());
        }

        self.mainloop.lock();
        {
            let context_guard = self.context.lock();
            let mut introspector = context_guard.introspect();
            introspector.set_sink_volume_by_name(sink_name, &cv, None);
        }
        self.mainloop.unlock();

        Ok(())
    }

    /// Set volume for default source (Mic).
    pub fn set_mic_volume(&mut self, volume: f32) -> Result<()> {
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
        let tx = Arc::new(Mutex::new(tx));

        self.mainloop.lock();
        {
            let context_guard = self.context.lock();
            let introspector = context_guard.introspect();
            let tx_clone = tx.clone();
            introspector.get_server_info(move |info| {
                let default_source = info.default_source_name.as_ref().map(|s| s.to_string());
                let _ = tx_clone.lock().send(default_source);
            });
        }
        self.mainloop.unlock();

        let default_source_name = rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| Error::Audio("Timeout getting server info".into()))?
            .ok_or(Error::Audio("Failed to get server info".into()))?;

        self.set_source_volume_by_name(&default_source_name, volume)?;

        Ok(())
    }

    fn set_source_volume_by_name(&mut self, source_name: &str, volume: f32) -> Result<()> {
        let (tx, rx) = std::sync::mpsc::channel::<Option<u8>>();
        let tx = Arc::new(Mutex::new(tx));

        self.mainloop.lock();
        {
            let context_guard = self.context.lock();
            let introspector = context_guard.introspect();
            let tx_clone = tx.clone();
            introspector.get_source_info_by_name(source_name, move |info| {
                match info {
                    pulse::callbacks::ListResult::Item(item) => {
                        let _ = tx_clone.lock().send(Some(item.channel_map.len()));
                    }
                    _ => {}
                }
            });
        }
        self.mainloop.unlock();

        let channels = rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| Error::Audio("Timeout getting source info".into()))?
            .ok_or(Error::Audio(format!("Source {} not found", source_name)))?;

        let vol_linear = VolumeLinear(volume as f64);
        let mut cv = ChannelVolumes::default();
        cv.set_len(channels);
        for i in 0..channels {
            cv.set(i, vol_linear.into());
        }

        self.mainloop.lock();
        {
            let context_guard = self.context.lock();
            let mut introspector = context_guard.introspect();
            introspector.set_source_volume_by_name(source_name, &cv, None);
        }
        self.mainloop.unlock();

        Ok(())
    }

    /// Apply volume to all sink inputs.
    pub fn set_all_sink_inputs_volume(&mut self, volume: f32) -> Result<()> {
        let (tx, rx) = std::sync::mpsc::channel::<ListMsg>();
        let tx = Arc::new(Mutex::new(tx));

        self.mainloop.lock();
        {
            let context_guard = self.context.lock();
            let introspector = context_guard.introspect();
            let tx_clone = tx.clone();

            introspector.get_sink_input_info_list(move |result| {
                match result {
                    pulse::callbacks::ListResult::Item(item) => {
                        let _ = tx_clone.lock().send(ListMsg::Item(item.index, item.channel_map.len()));
                    }
                    pulse::callbacks::ListResult::End => {
                        let _ = tx_clone.lock().send(ListMsg::End);
                    }
                    _ => {}
                }
            });
        }
        self.mainloop.unlock();

        let mut inputs = Vec::new();
        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(ListMsg::Item(index, channels)) => {
                    inputs.push((index, channels));
                }
                Ok(ListMsg::End) => break,
                Err(_) => break, // Timeout or disconnected
            }
        }

        if !inputs.is_empty() {
            let vol_linear = VolumeLinear(volume as f64);
            self.mainloop.lock();
            {
                let context_guard = self.context.lock();
                let mut introspector = context_guard.introspect();

                for (index, channels) in inputs {
                    let mut cv = ChannelVolumes::default();
                    cv.set_len(channels);
                    for i in 0..channels {
                        cv.set(i, vol_linear.into());
                    }
                    introspector.set_sink_input_volume(index, &cv, None);
                }
            }
            self.mainloop.unlock();
        }

        Ok(())
    }
}
