use crate::audio::fe::Sample;

/// Where the car sounds go: the device mixer or a headless [`Mix`].
pub trait Sink {
    fn one_shot(&mut self, sample: &Sample, gain: f32, pitch: u16);
    fn start_loop(&mut self, sample: &Sample, gain: f32, pitch: u16) -> u32;
    fn gain(&mut self, id: u32, gain: f32);
    fn end(&mut self, id: u32);
    /// A one-shot with a handle (its gain can be changed); ends by itself.
    fn start_once(&mut self, sample: &Sample, gain: f32, pitch: u16) -> u32;
    fn sounding(&mut self, id: u32) -> bool;
    /// A handled voice's pitch (0x1000 = the recorded rate).
    fn pitch(&mut self, id: u32, pitch: u16);
    /// A handled voice's azimuth word (kind 3; [`crate::audio::mixer::pan_weights`]).
    fn pan(&mut self, id: u32, azimuth: u16);
    /// A handled voice's reverb send gain (bus 0): the send byte / 127 ×
    /// its volume (0x723BC1 → 0x729E24); none by default.
    fn send(&mut self, _id: u32, _gain: f32) {}
}

impl Sink for crate::audio::mixer::Mix {
    fn pitch(&mut self, id: u32, pitch: u16) {
        self.set_pitch(id, pitch);
    }
    fn send(&mut self, id: u32, gain: f32) {
        self.set_send(id, gain);
    }
    fn pan(&mut self, id: u32, azimuth: u16) {
        self.set_pan(id, azimuth);
    }
    fn start_once(&mut self, sample: &Sample, gain: f32, pitch: u16) -> u32 {
        self.play_handle(sample, gain, pitch)
    }
    fn sounding(&mut self, id: u32) -> bool {
        self.playing(id)
    }
    fn one_shot(&mut self, sample: &Sample, gain: f32, pitch: u16) {
        self.play_after(sample, gain, pitch, 0.0);
    }
    fn start_loop(&mut self, sample: &Sample, gain: f32, pitch: u16) -> u32 {
        self.play_loop(sample, gain, pitch)
    }
    fn gain(&mut self, id: u32, gain: f32) {
        self.set_gain(id, gain);
    }
    fn end(&mut self, id: u32) {
        self.stop(id);
    }
}

impl Sink for crate::audio::mixer::Mixer {
    fn pitch(&mut self, id: u32, pitch: u16) {
        self.set_pitch(id, pitch);
    }
    fn send(&mut self, id: u32, gain: f32) {
        self.set_send(id, gain);
    }
    fn pan(&mut self, id: u32, azimuth: u16) {
        self.set_pan(id, azimuth);
    }
    fn start_once(&mut self, sample: &Sample, gain: f32, pitch: u16) -> u32 {
        self.play_handle(sample, gain, pitch)
    }
    fn sounding(&mut self, id: u32) -> bool {
        self.playing(id)
    }
    fn one_shot(&mut self, sample: &Sample, gain: f32, pitch: u16) {
        self.play_after(sample, gain, pitch, 0.0);
    }
    fn start_loop(&mut self, sample: &Sample, gain: f32, pitch: u16) -> u32 {
        self.play_loop(sample, gain, pitch)
    }
    fn gain(&mut self, id: u32, gain: f32) {
        self.set_gain(id, gain);
    }
    fn end(&mut self, id: u32) {
        self.stop(id);
    }
}
