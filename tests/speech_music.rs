//! SDATA/sdat.viv: the archive, a speech line (MicroTalk), the career
//! scene's NIS line (EA-XA) and a song segment (EALayer3) decoded from
//! the install (ignored without NFSU2_ROOT).
mod common;

use nfsu2::audio::{music::Mpf, schl, speech};
use nfsu2::formats::viv::Viv;

fn viv() -> Viv {
    let root = common::game_root();
    Viv::mount_game(&root).expect("mounting the game's archives")
}

fn rms(s: &[f32]) -> f32 {
    (s.iter().map(|x| x * x).sum::<f32>() / s.len().max(1) as f32).sqrt()
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn archive_entries() {
    let v = viv();
    assert_eq!(v.entries.len(), 10);
    let e = v.find("SPEECH/ug2_speech.BIG").unwrap();
    assert_eq!((e.offset, e.size), (0x1B40, 56_835_840));
    assert_eq!(v.find("pfdata/MusicSFx.mus").unwrap().size, 125_873_408);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn speech_line_microtalk() {
    let v = viv();
    let e = v.find("speech/UG2_Speech.big").unwrap();
    let d = v.read_range(e, 0, 0x20000).unwrap();
    let h = schl::header(&d).unwrap();
    assert_eq!((h.codec, h.channels, h.rate, h.samples), (0x16, 1, 22050, 296_250));
    let len = schl::stream_len(&d).unwrap();
    assert_eq!(len, 0x1D2EC);
    let pcm = schl::decode(&d[..len]).unwrap();
    assert_eq!(pcm.samples.len(), 296_250);
    let r = rms(&pcm.samples);
    let peak = pcm.samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    // Speech: well above silence, not blown up.
    assert!(r > 0.05 && r < 0.3 && peak < 1.2, "rms {r} peak {peak}");
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn career_nis_line() {
    let v = viv();
    let bank = speech::Bank::load(&v, "nisaudio").unwrap();
    let sel = bank.select(speech::nis::EVENT_END, &speech::nis::CAREER);
    assert_eq!(sel, vec![0]);
    let pcm = bank.decode(&v, 0x4024, 0).unwrap();
    assert_eq!((pcm.channels, pcm.rate), (2, 32000));
    let secs = pcm.samples.len() as f32 / 2.0 / 32000.0;
    assert!((secs - 13.24).abs() < 0.01, "{secs} s");
    assert!(rms(&pcm.samples) > 0.01);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn song_segment_ealayer3() {
    let v = viv();
    let mpf = Mpf::parse(&v.read("pfdata/MusicSFx.mpf").unwrap()).unwrap();
    assert_eq!(mpf.nodes.len(), 162);
    assert_eq!(mpf.samples.len(), 43);
    assert_eq!(mpf.events.len(), 0x4F);
    // Event i starts at node 3i (0x7399E0 → the event record's +0x1C).
    assert!((0..27).all(|i| mpf.events[i] == 3 * i));
    // Song 0, "Riders on the Storm (Fredwreck Remix)": node 2's stream id
    // 39 → table entry 38, 373.6 s (matched under Wine against the
    // original's output).
    let s = mpf.song_sample(0).unwrap();
    assert_eq!(s, 38);
    assert_eq!(mpf.song_sample(1), Some(3));
    let (off, samples) = mpf.samples[s];
    assert!((samples as f32 / 32000.0 - 373.6).abs() < 0.1, "{samples}");
    let e = v.find("pfdata/MusicSFx.mus").unwrap();
    // The first ~120 blocks (about 8 s).
    let d = v.read_range(e, off, 0x40000).unwrap();
    let h = schl::header(&d).unwrap();
    assert_eq!((h.codec, h.channels, h.rate), (0x17, 2, 32000));
    let mut dec = nfsu2::audio::ealayer3::Decoder::new();
    let mut out = Vec::new();
    for (o, size) in schl::blocks(&d, &h) {
        let mut p = o + 0xC + 8;
        while p < o + size {
            match dec.frame(&d[p..o + size], 2, &mut out) {
                Some(n) if n > 0 => p += n,
                _ => break,
            }
        }
    }
    assert!(out.len() > 32000 * 2 * 5, "{} samples", out.len());
    let tail = &out[out.len() - 32000 * 2..];
    let r = rms(tail);
    assert!(r > 0.02 && r < 0.6, "rms {r}");
    // Stereo music: the channels are correlated.
    let (l, rr): (Vec<f32>, Vec<f32>) = tail.chunks(2).map(|c| (c[0], c[1])).unzip();
    let dot: f32 = l.iter().zip(&rr).map(|(a, b)| a * b).sum();
    assert!(dot > 0.0);
}

#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn rachels_lines_fit_one_event_each() {
    use speech::lines;
    let v = viv();
    let b = speech::Bank::load(&v, "UG2_Speech").unwrap();
    // (tuple, entry, records): 0x53E0C0's and 0x5243E0's lines.
    let cases: Vec<(Vec<u32>, u32, Vec<usize>)> = vec![
        (lines::career(0x2000, 0x20), 0x27, vec![0]),
        (lines::career(1, 0x20), 0x27, vec![11, 12, 13]),
        (lines::career(0x100_0000, 0x20), 0x27, vec![7]),
        (lines::career(0x80_0000, 0x10), 0x27, vec![14]),
        (lines::career(0x800, 0x20), 0x27, vec![38]),
        (lines::time(1), 0x23, vec![0]),
        (lines::time(4), 0x23, vec![1]),
        (lines::time(8), 0x23, vec![2]),
        (lines::intro_events(0x40_0000, 8), 0x37, vec![39]),
        (lines::intro_events(0x8000, 1), 0x37, vec![35]),
        (lines::intro_area(2, 0x20), 0x36, vec![5]),
        (lines::dvd(0x8, 1), 0x2C, vec![0]),
        (lines::magazine(0x4, 1), 0x2E, vec![6]),
    ];
    for (t, entry, recs) in cases {
        assert_eq!(b.fit(&t), Some((entry, recs)), "{t:x?}");
    }
    // The front-end lines fit FE_Screen's entry 0x2D.
    assert_eq!(b.fit(&lines::fe_screen(0x100_0000, 0x20)).map(|f| f.0), Some(0x2D));
    assert_eq!(b.fit(&lines::fe_screen(0x10_0000, 0x10)).map(|f| f.0), Some(0x2D));
}
