//! The fuzz targets: one function per parser entry point that takes bytes
//! from a file the player (or a mod, a download) controls. Shared by
//! `fuzz/fuzz_targets/*.rs` (cargo-fuzz, nightly) and `tests/fuzz_smoke.rs`
//! (stable, a mutation engine of its own); see docs/fuzzing.md.
//!
//! A target must return for every input: no panic, no hang, no allocation
//! beyond `Limits::max_alloc`. It never reads game files itself (the seeds,
//! which the smoke test takes from the player's installed game, are named in
//! [`Seed`]; nothing from the game is committed).
//!
//! Chunk-level targets start with one byte, the chunk header's file offset
//! modulo 128 (alignment rules of the readers depend on it), then the chunk.
#![allow(dead_code)]

use nfsu2::formats::bundle::{self, Chunk, id};
use nfsu2::formats::*;
use std::hint::black_box;

/// Where the smoke test finds real seeds for a target.
#[derive(Clone, Copy)]
pub enum Seed {
    /// Chunks (any depth) of these ids in a bundle, each as [offset%128] + chunk.
    Chunks(&'static str, &'static [u32]),
    /// The first chunk of each id of a bundle, concatenated (16-aligned).
    Concat(&'static str, &'static [u32]),
    /// The whole file (decompressed if JDLZ), cut to 2 MB.
    File(&'static str),
    /// Entries of the first archive that begin with this magic (<= 256 KB each).
    Viv(&'static str, &'static [u8]),
    /// The smallest compressed block (magic) of the texture packs of a file.
    Block(&'static str, &'static [u8; 4]),
    /// The top-level chunks of a bundle, each whole (header and body).
    Top(&'static str),
    /// The bodies of chunks of these ids (depth any; an embedded JDLZ/HUFF
    /// block also decompressed).
    Bodies(&'static str, &'static [u32]),
    /// Text lines the target understands.
    Text(&'static str),
}

pub struct Target {
    pub name: &'static str,
    pub seeds: &'static [Seed],
    pub run: fn(&[u8]),
}

/// The chunk at the start of `b[1..]`, its file offset `b[0] & 0x7F`.
pub fn chunk(b: &[u8]) -> Option<Chunk<'_>> {
    let base = (*b.first()? & 0x7F) as usize;
    bundle::chunks(&b[1..], base).next()
}

pub fn bundle_walk(b: &[u8]) {
    bundle::walk(bundle::chunks(b, 0), &mut |c| {
        black_box(c.payload().len());
    });
}

pub fn texture_pack(b: &[u8]) {
    if let Some(c) = chunk(b)
        && let Ok(t) = texture::parse_texture_pack(c)
    {
        black_box(t.iter().map(|t| t.decode_rgba().len()).sum::<usize>());
    }
}

pub fn viv_big(b: &[u8]) {
    black_box(viv::Viv::parse_header(std::path::PathBuf::new(), b).map(|v| v.entries.len()));
    // `Viv::open` on a file with this content (header size from file fields).
    let p = std::env::temp_dir().join(format!("nfsu2-fuzz-viv-{}-{:?}.viv", std::process::id(), std::thread::current().id()));
    if std::fs::write(&p, b).is_ok() {
        if let Some(v) = viv::Viv::open(&p) {
            for e in v.entries.iter().take(4) {
                black_box(v.read_range(e, 0, 64).map(|r| r.len()));
                black_box(v.read(&e.name).map(|r| r.len()));
            }
            black_box(v.find("a/b.c"));
        }
        let _ = std::fs::remove_file(&p);
    }
}

pub fn huff(b: &[u8]) {
    black_box(huff::decompress(b).ok());
}

pub fn jdlz(b: &[u8]) {
    black_box(jdlz::decompress(b).ok());
}

pub fn tpf(b: &[u8]) {
    if let Ok(p) = tpf::read(b) {
        black_box(p.entries.len());
    }
    black_box(tpf::parse_def(&String::from_utf8_lossy(b)).len());
}

pub fn track_sections(b: &[u8]) {
    black_box(track::parse_streaming_sections(b).map(|v| v.len()).ok());
}

pub fn scenery(b: &[u8]) {
    if let Some(c) = chunk(b) {
        black_box(scenery::parse_scenery_section(c).map(|s| s.instances.len()).ok());
    }
}

pub fn solid(b: &[u8]) {
    if let Some(c) = chunk(b) {
        black_box(solid::parse_solid_list(c).map(|s| s.len()).ok());
        black_box(solid::solid_markers(c, 0).len());
    }
}

pub fn collision(b: &[u8]) {
    if let Some(c) = chunk(b) {
        black_box(collision::parse_collision_pack(c).1.len());
        black_box(collision::parse_collision_grid(c).map(|g| g.cells.len()));
        black_box(collision::parse_barrier_groups(c).len());
        black_box(collision::parse_collision_volumes(c).len());
    }
}

pub fn visibility(b: &[u8]) {
    black_box(visibility::Visibility::parse(b).map(|v| v.lists.len()).ok());
}

pub fn lights(b: &[u8]) {
    if let Some(c) = chunk(b) {
        let p = lights::LightPack::parse(c);
        black_box(p.leaf_lights(glam::Vec3::ZERO).map(|l| l.count()));
        black_box(lights::FlarePack::parse(c).map(|p| p.flares.len()));
    }
}

pub fn roads(b: &[u8]) {
    let net = roads::RoadNet::parse(b);
    black_box(net.segments.len());
    road_queries(&net);
}

/// What the game does with a parsed network: nearest node, routes, the
/// race road (ranges the parse must have made consistent).
fn road_queries(net: &roads::RoadNet) {
    let pts: Vec<glam::Vec2> = net.nodes.iter().take(64).map(|n| n.0).collect();
    for w in pts.windows(2).take(16) {
        black_box(net.nearest_node(w[0]));
        for loops in [false, true] {
            if let Some(route) = net.route(w, loops) {
                let r = net.race_road(&route, loops);
                black_box((r.point_at(r.length * 0.5), r.direction(r.length * 0.5), r.segment_at(1.0)));
            }
        }
    }
    // The locator on whatever route a race road could be built for.
    if let Some(route) = net.route(&pts[..pts.len().min(2)], false) {
        let r = net.race_road(&route, false);
        let mut loc = roads::Locator::default();
        for p in pts.iter().take(8) {
            r.locate(&mut loc, *p + glam::Vec2::splat(3.0));
        }
        black_box(r.project(glam::Vec2::ZERO, Some(5.0), 20.0));
    }
    if pts.len() >= 3 {
        black_box(net.route(&pts[..3.min(pts.len())], true).map(|r| net.race_road(&r, true).length));
    }
}

pub fn ai_routes(b: &[u8]) {
    let mut r = ai_routes::RouteFile::parse(b);
    black_box(r.records.len());
    r.finalize_free_roam(&[]);
}

pub fn parts(b: &[u8]) {
    black_box(parts::PartDb::load(b).map(|d| d.parts.len()).ok());
    black_box(parts::CostTable::parse(b.get(8..).unwrap_or_default()).categories.len());
}

pub fn world_anim(b: &[u8]) {
    black_box(world_anim::WorldAnimations::parse(b).animations.len());
}

pub fn texture_anim(b: &[u8]) {
    if let Some(c) = chunk(b)
        && let Ok(a) = texture_anim::parse_texture_animations(c)
    {
        black_box(a.iter().map(|a| a.frame_at(1.0)).sum::<u32>());
    }
}

pub fn camera_tracks(b: &[u8]) {
    if let Some(c) = chunk(b) {
        black_box(camera_tracks::parse_camera_tracks(c).map(|g| g.len()).ok());
    }
}

pub fn parameter_map(b: &[u8]) {
    black_box(parameter_map::parse_parameter_maps(b).map(|m| m.len()).ok());
}

pub fn smackable(b: &[u8]) {
    if let Some(c) = chunk(b) {
        black_box(smackable::parse_scenery_groups(c).len());
        black_box(smackable::parse_scenery_links(c).len());
        black_box(smackable::parse_infos(c).len());
        black_box(smackable::parse_instances(c).len());
    }
}

pub fn eagl(b: &[u8]) {
    if let Some(c) = chunk(b) {
        black_box(eagl::parse_bank(c.aligned(16)).map(|v| v.len()).ok());
    }
    black_box(eagl::parse_bank(b).map(|v| v.len()).ok());
}

pub fn zam(b: &[u8]) {
    black_box(zam::PartAnimations::parse(b, "COROLLA").map(|a| a.pivots.len()).ok());
}

pub fn magazines(b: &[u8]) {
    black_box(magazines::Magazines::parse(b.get(8..).unwrap_or_default()).map(|m| m.setups.len()));
    black_box(magazines::Showcase::parse(b.get(8..).unwrap_or_default()).map(|m| m.setups.len()));
}

pub fn native_save(b: &[u8]) {
    if let Ok(s) = native_save::NativeSave::parse(b) {
        black_box((s.name(), s.visits(), s.to_bytes().len()));
    }
    black_box(native_save::read_picture(b).map(|p| p.len()).ok());
    // The payload behind a valid header and checksum.
    if let Ok(s) = native_save::NativeSave::parse(&native_save::container(b)) {
        black_box((s.name(), s.visits(), (0..8).map(|k| s.stage(k)).sum::<usize>()));
    }
}

/// A VP6 key frame, then inter frames (the next bytes, cut in the middle),
/// each into one decoder; the frame converted to RGBA.
pub fn vp6(b: &[u8]) {
    let mut d = vp6::Decoder::new();
    let mid = b.len() / 2;
    for part in [b, &b[..mid], &b[mid..]] {
        if d.decode(part).is_ok() {
            let mut out = Vec::new();
            vp6::to_rgba(d.frame(), &mut out);
            black_box(out.len());
        }
    }
}

/// The movie container (chunks), the audio header, and the EA-XA decode of
/// the first block with the header's sample count.
pub fn movie(b: &[u8]) {
    if let Ok(m) = nfsu2::movie::Movie::parse(b.to_vec()) {
        black_box(m.header.width);
        for (_, c) in m.frames.iter().take(2) {
            black_box(m.payload(c).len());
        }
        if let Some(a) = &m.audio {
            for c in m.blocks.iter().take(2) {
                black_box(nfsu2::movie::decode_block(a, &mut [Default::default(); 8], m.payload(c)).len());
            }
        }
    }
    black_box(nfsu2::movie::parse_audio_header(b));
}

pub fn feng_package(b: &[u8]) {
    black_box(nfsu2::frontend::feng::parse(b).is_ok());
    black_box(nfsu2::frontend::feng::parse_responses(b).len());
}

pub fn feng_strings(b: &[u8]) {
    if let Ok(s) = nfsu2::frontend::feng::strings::Strings::parse(b) {
        black_box(s.map.len());
    }
}

pub fn feng_font(b: &[u8]) {
    black_box(nfsu2::frontend::feng::font::Font::parse(b).is_ok());
}

pub fn options_text(b: &[u8]) {
    let s = nfsu2::frontend::options::Settings::from_text(&String::from_utf8_lossy(b));
    black_box(s.options_text().len());
}

/// The player's text files: the legacy career.txt, the SMS inbox lines, the
/// memory-card language table.
pub fn profile_text(b: &[u8]) {
    let text = String::from_utf8_lossy(b);
    let mut inbox = nfsu2::frontend::sms::Inbox::default();
    for l in text.lines().take(512) {
        inbox.load_line(l.strip_prefix("sms ").unwrap_or(l));
    }
    black_box(inbox.unread());
    black_box(nfsu2::frontend::profiles::parse_loc(b).len());
    let p = std::env::temp_dir().join(format!("nfsu2-fuzz-career-{}-{:?}.txt", std::process::id(), std::thread::current().id()));
    if std::fs::write(&p, b).is_ok() {
        black_box(nfsu2::frontend::career::Profile::load_text(&p).map(|p| p.cash));
        let _ = std::fs::remove_file(&p);
    }
}

pub fn globalb(b: &[u8]) {
    black_box(nfsu2::car::parse_car_types(b).map(|v| v.len()).ok());
    black_box(nfsu2::car::parse_presets(b).len());
    black_box(nfsu2::car::parse_light_materials(b).len());
    black_box(nfsu2::drift::parse_tricks(b).len());
    black_box(nfsu2::opponents::Bounds::parse(b).is_some());
    black_box(nfsu2::particles::parse_descriptions(b).len());
    black_box(nfsu2::tuning::parse(b).len());
}

pub fn dds(b: &[u8]) {
    black_box(nfsu2::upscale::parse_dds(b).is_some());
}

pub fn abk(b: &[u8]) {
    if let Some(bank) = nfsu2::audio::abk::Bank::parse(b.to_vec()) {
        for i in 0..bank.samples.len().min(6) {
            black_box(bank.decode(i).map(|(_, p)| p.len()));
        }
        for m in bank.modules.iter().take(8) {
            black_box(bank.sound_list(m).len());
        }
        for (name, _) in bank.events.iter().take(8) {
            black_box(bank.module(name).is_some());
        }
    }
}

pub fn schl(b: &[u8]) {
    black_box(nfsu2::audio::schl::decode(b).map(|p| p.samples.len()));
    black_box(nfsu2::audio::codec::schl::stream_len(b));
}

/// Byte 0: channels (1..=2); the rest: EALayer3 frames.
pub fn ealayer3(b: &[u8]) {
    let Some((&ch, d)) = b.split_first() else { return };
    let mut dec = nfsu2::audio::ealayer3::Decoder::new();
    let mut out = Vec::new();
    let mut at = 0;
    for _ in 0..16 {
        let Some(n) = dec.frame(&d[at.min(d.len())..], 1 + (ch & 1) as usize, &mut out) else { break };
        at += n.max(1);
    }
    black_box(out.len());
}

/// Byte 0 bit 0: the "rev3" flavour; the rest: a MicroTalk stream.
pub fn utk(b: &[u8]) {
    let Some((&f, d)) = b.split_first() else { return };
    let mut u = nfsu2::audio::codec::utk::Utk::new(d, f & 1 != 0);
    for _ in 0..8 {
        black_box(u.frame().len());
    }
    u.begin(d, f & 2 != 0);
    black_box(u.frame().len());
}

pub fn gin(b: &[u8]) {
    if let Some(g) = nfsu2::audio::gin::Gin::parse(b) {
        black_box((g.rpm_position(3000.0), g.cycle_at(100), g.cycle_length(2.0), g.cycle_position(5.0)));
        let mut gr = nfsu2::audio::gin::Granular::new(g);
        gr.set_rpm(4000.0, 10.0);
        let mut out = vec![0i16; gr.block()];
        for _ in 0..4 {
            gr.fill(&mut out);
        }
    }
}

/// A speech bank file (`.evt`, `.idx` or `.csi`), read as each kind; the
/// events are also requested through the library against the index read
/// from the same bytes.
pub fn speech_bank(d: &[u8]) {
    use nfsu2::audio::speech::{self, Bank, BankId, Library};
    let k = d.len() as u8;
    let (events, event_switches) = speech::parse_evt_switches(d);
    let bank = Bank {
        entries: speech::parse_idx(d),
        events,
        switches: speech::parse_csi(d),
        event_switches,
        ..Bank::default()
    };
    let mut lib = Library::default();
    for ev in bank.events.iter().take(16) {
        let values = [1u32 << (k & 31), u32::from(k), 0x20, 3];
        if let Some(p) = lib.request(&bank, BankId::Speech, ev, &values) {
            lib.played(&bank, BankId::Speech, p);
        }
        black_box(bank.select(ev.id, &values).len());
    }
    black_box(bank.fit(&speech::lines::outrun(8, 1)));
}

/// SPEED2.EXE's PE section table and every table address read through it
/// (`exe_tables`: a modified or broken executable is read before it is
/// refused).
pub fn pe_sections(b: &[u8]) {
    nfsu2::exe_tables::fuzz_probe(b);
}

macro_rules! t {
    ($n:ident, $($s:expr),*) => { Target { name: stringify!($n), seeds: &[$($s),*], run: $n } };
}

const BUN_STREAM: &str = "TRACKS/STREAML4RA.BUN";
const BUN_TRACK: &str = "TRACKS/L4RA.BUN";

pub const TARGETS: &[Target] = &[
    t!(bundle_walk, Seed::Chunks(BUN_STREAM, &[id::SCENERY_SECTION])),
    t!(texture_pack, Seed::Chunks(BUN_STREAM, &[id::TEXTURE_PACK]), Seed::Chunks("TRACKS/LOC4DYNTEX.BIN", &[id::TEXTURE_PACK])),
    t!(viv_big, Seed::File("SDATA/sdat.viv")),
    t!(huff, Seed::Block("TRACKS/LOC4DYNTEX.BIN", huff::MAGIC)),
    t!(jdlz, Seed::Block("TRACKS/LOC4DYNTEX.BIN", jdlz::MAGIC), Seed::File("GLOBAL/GlobalB.lzc")),
    t!(tpf, Seed::Text("tpf")),
    t!(track_sections, Seed::Concat(BUN_TRACK, &[id::TRACK_STREAMING_SECTIONS])),
    t!(scenery, Seed::Chunks(BUN_STREAM, &[id::SCENERY_SECTION])),
    t!(solid, Seed::Chunks(BUN_STREAM, &[id::SOLID_LIST]), Seed::Chunks("CARS/COROLLA/GEOMETRY.BIN", &[id::SOLID_LIST])),
    t!(collision, Seed::Chunks(BUN_STREAM, &[collision::COLLISION_PACK]), Seed::Chunks(BUN_TRACK, &[collision::BARRIER_GROUPS]), Seed::Chunks("CARS/COROLLA/GEOMETRY.BIN", &[collision::COLLISION_VOLUMES])),
    t!(visibility, Seed::Concat(BUN_TRACK, &[visibility::SECTION_INFO, visibility::DRIVABLE_BOUNDARIES, visibility::VISIBLE_SECTIONS])),
    t!(lights, Seed::Chunks(BUN_STREAM, &[lights::LIGHT_PACK, lights::FLARE_PACK])),
    t!(roads, Seed::Concat("TRACKS/ROUTESL4RA/Paths4001.bin", &[0x34148, 0x34149, 0x3414D])),
    t!(ai_routes, Seed::Concat("TRACKS/ROUTESL4RA/Routes4176F.bin", &[0x34121, 0x34122, 0x34123])),
    t!(parts, Seed::Concat("GLOBAL/GLOBALB.BUN", &[0x8003_4602, 0x34607]), Seed::Concat("GLOBAL/GLOBALB.BUN", &[0x34A1C])),
    t!(world_anim, Seed::Concat(BUN_TRACK, &[world_anim::ENTITY, world_anim::INSTANCE, world_anim::BANK])),
    t!(texture_anim, Seed::Chunks(BUN_STREAM, &[texture_anim::TEXTURE_ANIMATIONS])),
    t!(camera_tracks, Seed::Chunks(BUN_TRACK, &[camera_tracks::TYPED_CAMERAS, camera_tracks::ROUTE_CAMERAS])),
    t!(parameter_map, Seed::Concat(BUN_TRACK, &[parameter_map::PARAMETER_MAPS])),
    t!(smackable, Seed::Chunks(BUN_TRACK, &[smackable::SCENERY_GROUPS, smackable::SCENERY_LINKS, smackable::OBJECT_INFOS]), Seed::Chunks(BUN_STREAM, &[smackable::OBJECT_INSTANCES])),
    t!(eagl, Seed::Chunks(BUN_TRACK, &[world_anim::BANK])),
    t!(zam, Seed::File("CARS/COROLLA/PARTS_ANIMATIONS.bin")),
    t!(magazines, Seed::File("FRONTEND/MAGAZINES_FRONTEND.BIN"), Seed::File("FRONTEND/MAGAZINES_SHOWCASE.BIN")),
    t!(native_save, Seed::Text("save")),
    t!(vp6, Seed::Text("vp6")),
    t!(movie, Seed::File("MOVIES/blank.vp6"), Seed::File("MOVIES/DragTut_en.vp6")),
    t!(feng_package, Seed::Bodies("FRONTEND/FrontB.lzc", &[0x30203, 0x30210]), Seed::Bodies("GLOBAL/InGameCommon.lzc", &[0x30203, 0x30210])),
    t!(feng_strings, Seed::File("LANGUAGES/English.bin")),
    t!(feng_font, Seed::Bodies("FRONTEND/FrontB.lzc", &[0x30201]), Seed::Bodies("GLOBAL/GlobalB.lzc", &[0x30201])),
    t!(options_text, Seed::Text("options")),
    t!(profile_text, Seed::Text("profile")),
    t!(globalb, Seed::File("GLOBAL/GLOBALB.BUN"), Seed::Top("GLOBAL/GLOBALB.BUN")),
    t!(dds, Seed::Text("dds")),
    t!(abk, Seed::File("SOUND/NOS/Nitrous_00_MB.abk"), Seed::File("SOUND/FE/FE_MB.abk")),
    t!(schl, Seed::Viv("SDATA/sdat.viv", b"SCHl")),
    t!(ealayer3, Seed::Viv("SDATA/sdat.viv", b"SCHl")),
    t!(utk, Seed::Viv("SDATA/sdat.viv", b"SCHl")),
    t!(gin, Seed::File("SOUND/ENGINE/GIN_Mazda_RX7_DCL.gin")),
    t!(pe_sections, Seed::File("SPEED2.EXE")),
    t!(speech_bank, Seed::Viv("SDATA/sdat.viv", b"\x03\x11\x3c\x07"), Seed::Viv("SDATA/sdat.viv", b"MOIR"), Seed::Viv("SDATA/sdat.viv", b"\x01\x00\x00\x00")),
];
