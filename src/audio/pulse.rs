use crate::{Error, Result};
use libpulse_binding::callbacks::ListResult;
use libpulse_binding::channelmap::Map as ChannelMap;
use libpulse_binding::context::{Context, FlagSet as ContextFlagSet, State as ContextState};
use libpulse_binding::mainloop::threaded::Mainloop;
use libpulse_binding::volume::{ChannelVolumes, Volume};
use std::sync::mpsc;

/// Describes a single PulseAudio sink input (an output audio stream routed to a sink).
///
/// A sink input typically corresponds to an application's playback stream, including
/// identifying metadata and its current routing and volume state.
pub struct SinkInput {
    /// Index of the sink input as assigned by PulseAudio.
    pub index: u32,
    /// Human-readable name of the stream (often set by the application).
    pub name: String,
    /// Name of the application that owns this stream, if available.
    pub app_name: Option<String>,
    /// Media role or category of the stream (e.g. "music", "game"), if provided.
    pub media_role: Option<String>,
    /// Channel map describing the layout of the stream's audio channels.
    pub channel_map: ChannelMap,
    /// Per-channel volume levels for this sink input.
    pub volume: ChannelVolumes,
    /// Whether this sink input is currently muted.
    pub muted: bool,
}

pub struct PulseHandler {
    mainloop: Mainloop,
    context: Context,
}

impl PulseHandler {
    pub fn new() -> Result<Self> {
        let mut mainloop = Mainloop::new().ok_or_else(|| Error::Audio("Failed to create mainloop".to_string()))?;
        let mut context = Context::new(&mainloop, "SteelSeries GG")
            .ok_or_else(|| Error::Audio("Failed to create context".to_string()))?;

        context
            .connect(None, ContextFlagSet::NOFLAGS, None)
            .map_err(|e| Error::Audio(format!("Failed to connect context: {}", e)))?;

        mainloop
            .start()
            .map_err(|e| Error::Audio(format!("Failed to start mainloop: {}", e)))?;

        // Wait for ready
        mainloop.lock();
        let result = loop {
            match context.get_state() {
                ContextState::Ready => break Ok(()),
                ContextState::Failed | ContextState::Terminated => {
                    break Err(Error::Audio("Context connection failed".to_string()));
                }
                _ => {
                    mainloop.wait();
                }
            }
        };
        mainloop.unlock();

        result?;

        Ok(Self { mainloop, context })
    }

    pub fn get_sink_inputs(&mut self) -> Result<Vec<SinkInput>> {
        let (tx, rx) = mpsc::channel();
        let tx = parking_lot::Mutex::new(tx);

        self.mainloop.lock();

        let introspector = self.context.introspect();
        let _op = introspector.get_sink_input_info_list(move |res| {
            let tx = tx.lock();
            match res {
                ListResult::Item(info) => {
                    let app_name = info
                        .proplist
                        .get_str("application.name")
                        .or_else(|| info.proplist.get_str("application.process.binary"))
                        .map(|s| s.to_string());

                    let media_role = info.proplist.get_str("media.role").map(|s| s.to_string());

                    let input = SinkInput {
                        index: info.index,
                        name: info.name.as_ref().map(|s| s.to_string()).unwrap_or_default(),
                        app_name,
                        media_role,
                        channel_map: info.channel_map,
                        volume: info.volume,
                        muted: info.mute,
                    };

                    let _ = tx.send(Ok(Some(input)));
                }
                ListResult::End => {
                    let _ = tx.send(Ok(None)); // End signal
                }
                ListResult::Error => {
                    let _ = tx.send(Err(Error::Audio("Failed to list sink inputs".to_string())));
                }
            }
        });

        self.mainloop.unlock();

        let mut inputs = Vec::new();
        loop {
            match rx.recv() {
                Ok(Ok(Some(input))) => inputs.push(input),
                Ok(Ok(None)) => break,
                Ok(Err(e)) => return Err(e),
                Err(_) => return Err(Error::Audio("Channel closed unexpectedly".to_string())),
            }
        }

        Ok(inputs)
    }

    pub fn set_volume(&mut self, index: u32, volume: f32, channel_map: &ChannelMap) -> Result<()> {
        let vol_linear = volume.clamp(0.0, 1.0);
        // Assuming Volume is u32 wrapper and NORMAL is 0x10000 (65536)
        let vol_val = Volume((vol_linear * 65536.0) as u32);

        let mut cv = ChannelVolumes::default();
        cv.set_len(channel_map.len());
        for i in 0..channel_map.len() {
            cv.set(i, vol_val);
        }

        let (tx, rx) = mpsc::channel();

        self.mainloop.lock();
        let _ = self.context.introspect().set_sink_input_volume(
            index,
            &cv,
            Some(Box::new(move |success| {
                let _ = tx.send(success);
            })),
        );
        self.mainloop.unlock();

        match rx.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::Audio("Failed to set sink input volume".to_string())),
            Err(_) => Err(Error::Audio("Volume setting timed out or channel closed".to_string())),
        }
    }

    pub fn set_mute(&mut self, index: u32, muted: bool) -> Result<()> {
        let (tx, rx) = mpsc::channel();

        self.mainloop.lock();
        let _ = self.context.introspect().set_sink_input_mute(
            index,
            muted,
            Some(Box::new(move |success| {
                let _ = tx.send(success);
            })),
        );
        self.mainloop.unlock();

        match rx.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::Audio("Failed to set sink input mute".to_string())),
            Err(_) => Err(Error::Audio("Mute setting timed out or channel closed".to_string())),
        }
    }
}

impl Drop for PulseHandler {
    fn drop(&mut self) {
        self.context.disconnect();
        self.mainloop.stop();
    }
}
