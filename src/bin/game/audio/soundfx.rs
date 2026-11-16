use crate::*;

/// The race state the SoundFX requests read.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SoundFxIn {
    pub(crate) flow: u32,
    pub(crate) nitrous: f32,
    pub(crate) blown: bool,
}

// prov: inferred game.soundfx_request
/// NisAudio_Request (0x484510) for a named SoundFX stream: GB_NOS
/// (SoundFX_Select (1, 4)) unless the race flow is in state 2, 3 or 7 or a
/// CollisionStream plays (0x82BB2B, cleared at each request unless the
/// view mode 0x82BB48 is 3 — never in the clone), then 0x82BB29 = 1;
/// CollisionStream (2, 8) sets 0x82BB2B; BLOWNENGINE (8, 0x10) 0x82BB2C;
/// each queued with its request kind (1, 5, 8: 0x484510's first
/// argument, passed on to 0x46CA60). **Inferred**: the queued
/// line replaces the playing one (the kind table 0x484B74's priorities
/// are not modelled).
pub(crate) fn soundfx_request(a: &mut Audio, ask: SoundFx, flow: u32) {
    use nfsu2::audio::speech::nis::EVENT_SOUNDFX;
    a.soundfx.collision = false;
    // The request kinds (0x48509F GB_NOS 1, 0x486154 CollisionStream 5,
    // 0x484F0A BLOWNENGINE 8), queued as the stream's kind (0x46CA60).
    let kind = match ask {
        SoundFx::GbNos => 1,
        SoundFx::Collision => 5,
        SoundFx::BlownEngine => 8,
    };
    let tuple: [u32; 2] = match ask {
        SoundFx::GbNos => {
            if matches!(flow, 2 | 3 | 7) || a.soundfx.collision {
                return;
            }
            a.soundfx.gb_nos = true;
            [1, 4]
        }
        SoundFx::Collision => {
            a.soundfx.collision = true;
            [2, 8]
        }
        SoundFx::BlownEngine => {
            a.soundfx.blown = true;
            [8, 0x10]
        }
    };
    let played = a.streams.play_nis(&a.mixer, EVENT_SOUNDFX, &tuple, kind);
    if debug_env().audio_debug {
        eprintln!("audio: SoundFX {ask:?} {tuple:x?} {}", if played { "plays" } else { "fits no line" });
    }
}

/// The SoundFX streams' flags (see `Audio::soundfx`).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SoundFxGate {
    pub(crate) gb_nos: bool,
    pub(crate) collision: bool,
    blown: bool,
    pub(crate) blown_asked: bool,
    pub(crate) crash_camera: bool,
}

/// NisAudio_Request's named SoundFX streams (0x484608..0x48471E).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SoundFx {
    GbNos,
    Collision,
    BlownEngine,
}
