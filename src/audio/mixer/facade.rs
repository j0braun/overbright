use super::*;
use crate::audio::fe::Sample;
use crate::audio::gin::Granular;
use std::sync::{Arc, Mutex};

/// The device stream and the mix it plays.
pub struct Mixer {
    pub(super) mix: Arc<Mutex<Mix>>,
    /// The device's stream and its watchdog; None for an offline mixer
    /// ([`Mixer::offline`]).
    pub(super) device: Option<Mutex<Device>>,
}

impl Mixer {
    /// Dev harness (rate gate, `state_hash` `aud`): what has been posted
    /// to the mix, as bits: the voices started so far (the next handle),
    /// each live voice's source, handle, gain, pitch step, pan and send
    /// (not its play position, which follows the rendering), the engines
    /// and tracks set.
    #[cfg(feature = "dev-harness")]
    pub fn dev_posted(&self) -> Vec<u64> {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).dev_posted()
    }

    /// An offline mixer ([`Mixer::offline`]: rendered by its caller, no
    /// device).
    pub fn is_offline(&self) -> bool {
        self.device.is_none()
    }

    pub fn play(&self, sample: &Sample, volume: u8) {
        if !self.rendering() {
            return;
        }
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).play(sample, volume, 0x1000);
    }

    /// Whether the mix is being rendered: offline always, on a device
    /// while its stream runs. One-shots started while it does not (the
    /// sound server still opening, a watchdog gap) are dropped, not queued
    /// to all sound together when the stream comes back.
    fn rendering(&self) -> bool {
        self.device
            .as_ref()
            .is_none_or(|d| d.lock().unwrap_or_else(std::sync::PoisonError::into_inner).stream.is_some())
    }

    /// See [`Mix::play_after`].
    pub fn play_after(&self, sample: &Sample, gain: f32, pitch: u16, delay: f32) {
        if !self.rendering() {
            return;
        }
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).play_after(sample, gain, pitch, delay);
    }

    /// See [`Mix::play_handle_after`].
    pub fn play_handle_after(&self, sample: &Sample, gain: f32, pitch: u16, delay: f32) -> u32 {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).play_handle_after(sample, gain, pitch, delay)
    }

    /// See [`Mix::play_loop`].
    pub fn play_loop(&self, sample: &Sample, gain: f32, pitch: u16) -> u32 {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).play_loop(sample, gain, pitch)
    }

    /// See [`Mix::play_handle`].
    pub fn play_handle(&self, sample: &Sample, gain: f32, pitch: u16) -> u32 {
        // A one-shot with no stream running is dropped (handle 0: never
        // playing), as [`Mixer::play`].
        if !self.rendering() {
            return 0;
        }
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).play_handle(sample, gain, pitch)
    }

    /// See [`Mix::playing`].
    pub fn playing(&self, id: u32) -> bool {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).playing(id)
    }

    pub fn set_gain(&self, id: u32, gain: f32) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).set_gain(id, gain);
    }

    /// See [`Mix::set_send`].
    pub fn set_send(&self, id: u32, gain: f32) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).set_send(id, gain);
    }

    pub fn stop(&self, id: u32) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).stop(id);
    }

    /// See [`Mix::set_pitch`].
    pub fn set_pitch(&self, id: u32, pitch: u16) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).set_pitch(id, pitch);
    }

    pub fn set_pan(&self, id: u32, azimuth: u16) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).set_pan(id, azimuth);
    }

    pub fn engine_pan(&self, slot: usize, azimuth: u16) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).engine_pan(slot, azimuth);
    }

    pub fn set_engine(&self, slot: usize, player: Option<Granular>) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).set_engine(slot, player);
    }

    pub fn engine(&self, slot: usize, rpm: f32, volume: f32) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).engine(slot, rpm, volume);
    }

    /// See [`Mix::set_stream`].
    pub fn engine_send(&self, slot: usize, send: u8, dry: u8) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).engine_send(slot, send, dry);
    }

    pub fn set_fx(&self, graph: Option<crate::audio::reverb::Graph>) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).set_fx(graph);
    }

    pub fn fx_running(&self) -> bool {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).fx_running()
    }

    /// The largest |wet sample| (16-bit range) since the last call.
    pub fn take_fx_peak(&self) -> f32 {
        std::mem::take(&mut self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).fx_peak)
    }

    /// The mix rate (the FX graph's sample rate, 0x8B44C6).
    pub fn rate(&self) -> u32 {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).rate
    }

    pub fn set_stream(&self, format: Option<(u8, u32)>) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).set_stream(format);
    }

    /// See [`Mix::set_stream_gain`].
    pub fn set_stream_gain(&self, gain: f32) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).set_stream_gain(gain);
    }

    /// See [`Mix::push_stream`].
    pub fn push_stream(&self, samples: &[f32]) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push_stream(samples);
    }

    /// See [`Mix::play_track`].
    pub fn play_track(&self, buf: TrackBuf, gain: f32, kind: usize) -> u32 {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).play_track(buf, gain, kind)
    }

    /// The mix's state (voices, tracks, engines, levels) for a debug line.
    pub fn state(&self) -> String {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).debug_state()
    }

    /// The option volumes the mix applies now ([`Levels`]).
    pub fn levels(&self) -> Levels {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).levels
    }

    pub fn set_levels(&self, levels: Levels) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).levels = levels;
    }

    pub fn set_track_gain(&self, id: u32, gain: f32) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).set_track_gain(id, gain);
    }

    pub fn stop_track(&self, id: u32) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).stop_track(id);
    }

    /// See [`Mix::hold`].
    pub fn hold_all(&self, held: bool) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).hold(held);
    }

    /// Held by [`Mixer::hold_all`].
    pub fn is_held(&self) -> bool {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).held()
    }

    pub fn pause_track(&self, id: u32, paused: bool) {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).pause_track(id, paused);
    }

    pub fn track_playing(&self, id: u32) -> bool {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).track_playing(id)
    }

    pub fn track_pos(&self, id: u32) -> Option<(f64, bool)> {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).track_pos(id)
    }

    /// See [`Mix::voices`].
    pub fn voices(&self) -> usize {
        self.mix.lock().unwrap_or_else(std::sync::PoisonError::into_inner).voices()
    }

    /// See [`Mix::take_stops`].
    /// The sources' output energies since the last call ([`stems`]): (name,
    /// sum of squares of both channels in −1..1 units).
    pub fn take_stems(&self) -> Vec<(String, f64)> {
        let mut m = self.mix.lock().unwrap();
        let e = std::mem::take(&mut m.stem_energy);
        e.into_iter()
            .enumerate()
            .filter(|e| e.1 > 0.0)
            .map(|(i, x)| (stems::name(i), x / (32768.0 * 32768.0)))
            .collect()
    }
}
