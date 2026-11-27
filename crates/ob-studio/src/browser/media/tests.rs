use super::*;

fn sound(bank: &str, name: &str, samples: &[u32]) -> Sound {
    Sound { bank: bank.into(), name: name.into(), samples: samples.to_vec(), rate: 22050, frames: 22050, playable: true }
}

fn lines() -> Vec<Line> {
    vec![
        Line { hash: bin_hash("DEFAULT_STRING"), text: "Default".into() },
        Line { hash: 0x1234, text: "Press START to Race".into() },
        Line { hash: 0x5678, text: "Race Again".into() },
    ]
}

#[test]
fn a_wav_has_the_riff_header_and_16_bit_samples() {
    let w = wav(&Pcm { rate: 22050, samples: vec![0.0, 1000.4, -40000.0, 40000.0] });
    assert_eq!(&w[..4], b"RIFF");
    assert_eq!(&w[8..16], b"WAVEfmt ");
    assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()) as usize, w.len() - 8);
    assert_eq!(u16::from_le_bytes(w[22..24].try_into().unwrap()), 1, "one channel");
    assert_eq!(u32::from_le_bytes(w[24..28].try_into().unwrap()), 22050);
    assert_eq!(&w[36..40], b"data");
    assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()), 8);
    let s: Vec<i16> = w[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
    assert_eq!(s, [0, 1000, -32768, 32767], "rounded and clamped");
}

#[test]
fn sounds_are_found_by_every_word_of_their_bank_or_name() {
    let all = vec![
        sound("SOUND/FE/FE_MB.abk", "FE_MENU_CLICK", &[3]),
        sound("SOUND/IG_GLOBAL/FX_MAIN_MEM_MB.abk", "FX_TURBO_01", &[0, 1]),
        sound("SOUND/IG_GLOBAL/FX_MAIN_MEM_MB.abk", "FX_BLOWOFF", &[2]),
    ];
    assert_eq!(find_sounds(&all, "turbo"), [1]);
    assert_eq!(find_sounds(&all, "fx main"), [1, 2]);
    assert_eq!(find_sounds(&all, "FE click"), [0]);
    assert_eq!(find_sounds(&all, ""), [0, 1, 2]);
    assert!(find_sounds(&all, "zebra").is_empty());
}

#[test]
fn strings_are_found_by_words_hash_or_label() {
    let all = lines();
    assert_eq!(find_lines(&all, "race"), [1, 2]);
    assert_eq!(find_lines(&all, "race again"), [2]);
    assert_eq!(find_lines(&all, "0x1234"), [1]);
    assert_eq!(find_lines(&all, "default_string"), [0]);
    assert_eq!(find_lines(&all, ""), [0, 1, 2]);
    assert!(find_lines(&all, "zebra").is_empty());
}

#[test]
fn a_language_is_picked_by_name_or_a_unique_start() {
    let l = ["English".to_string(), "German".to_string(), "Spanish".to_string(), "Swedish".to_string()];
    assert_eq!(pick_language(&l, "german").as_deref(), Some("German"));
    assert_eq!(pick_language(&l, "eng").as_deref(), Some("English"));
    assert_eq!(pick_language(&l, "s"), None, "two start with s");
    assert_eq!(pick_language(&l, ""), None);
}

#[test]
fn the_wav_name_is_a_safe_file_name() {
    assert_eq!(wav_name(&sound("SOUND/FE/FE_MB.abk", "FE/CLICK 1", &[0])), "FE_MB-FE_CLICK_1.wav");
}

#[test]
fn given_media_answers_what_it_holds() {
    let m = GivenMedia {
        sounds: vec![sound("b.abk", "A", &[0])],
        pcm: vec![("b.abk".into(), 0, Pcm { rate: 8000, samples: vec![1.0] })],
        languages: vec![("English".into(), lines())],
    };
    assert_eq!(m.pcm("b.abk", 0).unwrap().rate, 8000);
    assert!(m.pcm("b.abk", 1).is_none());
    assert_eq!(m.languages(), ["English"]);
    assert_eq!(m.strings("english").unwrap().len(), 3);
    assert!(m.strings("Klingon").is_none());
}
