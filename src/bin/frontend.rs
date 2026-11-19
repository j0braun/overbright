//! `frontend` — the game's menus.
//!
//! `frontend --shot PACKAGE OUT.png [--time MS]` draws one package at a
//! moment of its INIT scripts; `frontend --list` names every package;
//! `frontend --tracks [-v|-q|-i]` counts the script tracks' kinds.

use anyhow::{Context, Result};
use nfsu2::frontend::{Assets, ScreenState, layout};
use nfsu2::render::Gpu;
use nfsu2::render::ui::UiRenderer;

fn shot(assets: &Assets, package: &str, out: &str, time: f32) -> Result<()> {
    let p = assets.package(package).with_context(|| format!("no package {package}"))?;
    let (width, height) = (1280u32, 960u32);
    let gpu = pollster::block_on(Gpu::new(None, Gpu::new_instance()))?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut ui = UiRenderer::new(&gpu, format);
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shot"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let state = ScreenState::default();
    let quads = layout(assets, p, &state, time);
    ui.render(&gpu, assets, &view, (width, height), &quads, true);
    let rgba = nfsu2::render::read_back(&gpu, &target, width, height);
    image::save_buffer(out, &rgba, width, height, image::ColorType::Rgba8)?;
    println!("wrote {out}: {} quads", quads.len());
    Ok(())
}

fn main() -> Result<()> {
    // A developer tool: the developer cache, never the player's (docs/developing/asset-cache.md).
    nfsu2::platform::dirs::developer_program();
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let assets = Assets::load(&nfsu2::game_root())?;
    if args.first().map(String::as_str) == Some("--list") {
        let mut names: Vec<_> = assets.packages.keys().collect();
        names.sort();
        for n in names {
            let p = &assets.packages[n];
            println!("{n}: {} objects, {} resources", p.objects.len(), p.resources.len());
        }
        println!("{} textures, {} fonts, {} strings", assets.textures.len(), assets.fonts.len(), assets.strings.map.len());
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--objects") {
        // One package's objects: hash, kind, parent, position, size, text
        // or label, and the scripts' name hashes.
        let p = assets.package(&args[1]).context("no such package")?;
        for o in &p.objects {
            let a = &o.attributes;
            let scripts: Vec<String> = o.scripts.iter().map(|s| format!("{:08X}", s.name)).collect();
            let text = o.label.and_then(|l| assets.strings.get(l)).unwrap_or(&o.text);
            println!(
                "{:08X} {:?} parent {:08X?} pos {:?} size {:?} flags {:08X} res {} {:?} scripts [{}]",
                o.hash,
                o.kind,
                o.parent,
                a.position(),
                a.size(),
                o.flags,
                o.resource,
                text,
                scripts.join(" ")
            );
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--scripts") {
        // One object's scripts: each track's attribute offset, kind, loop
        // mode, base and keys (as floats).
        let p = assets.package(&args[1]).context("no such package")?;
        let h = u32::from_str_radix(args[2].trim_start_matches("0x"), 16)?;
        let o = p.objects.iter().find(|o| o.hash == h).context("no such object")?;
        let f = |v: &[u32]| v.iter().map(|w| format!("{:.3}", f32::from_bits(*w))).collect::<Vec<_>>().join(",");
        println!("SA {}", f(&o.attributes.words));
        for m in &o.messages {
            println!("msg {:08X} {:08X?}", m.message, m.entries);
        }
        for s in &o.scripts {
            println!("script {:08X} len {} flags {:X} chain {:08X?} events {:08X?}", s.name, s.length, s.flags, s.chain, s.events.iter().map(|e| (e.message, e.target, e.time)).collect::<Vec<_>>());
            for t in &s.tracks {
                let g = |v: &[u32]| if t.param == 6 { v.iter().map(|w| (*w as i32).to_string()).collect::<Vec<_>>().join(",") } else { f(v) };
                println!("  to {} p{} i{} f{} len {} base [{}] keys {:?}", t.offset, t.param, t.interpolation, t.flags, t.length, g(&t.base), t.keys.iter().map(|k| (k.0, g(&k.1))).collect::<Vec<_>>());
            }
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--chunk") {
        // Dump a career sub-chunk (GlobalB 0x80034A10) as hex rows.
        let id = u32::from_str_radix(args[1].trim_start_matches("0x"), 16)?;
        let size: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0x40);
        let file = nfsu2::game_fs::read(&nfsu2::game_root(), "GLOBAL/GlobalB.lzc")?;
        let data = nfsu2::formats::jdlz::decompress(&file)?;
        let career = nfsu2::formats::bundle::chunks(&data, 0).find(|c| c.id == 0x8003_4A10).context("career")?;
        let c = career.child(id).context("no such chunk")?;
        for (i, r) in c.body.chunks(size).enumerate() {
            let hex: Vec<String> = r.iter().map(|b| format!("{b:02x}")).collect();
            println!("{i:3}: {}", hex.join(" "));
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--tracks") {
        // Script track statistics: (param, interpolation, flags) counts,
        // first-key times, and script flags / chains.
        let mut kinds: std::collections::BTreeMap<(u8, u8, u8), usize> = Default::default();
        let mut first: std::collections::BTreeMap<i64, usize> = Default::default();
        let mut sflags: std::collections::BTreeMap<u32, usize> = Default::default();
        for p in assets.packages.values() {
            for o in &p.objects {
                for s in &o.scripts {
                    *sflags.entry(s.flags).or_default() += 1;
                    for t in &s.tracks {
                        *kinds.entry((t.param, t.interpolation, t.flags)).or_default() += 1;
                        if let Some(k) = t.keys.first() {
                            *first.entry(k.0 as i32 as i64).or_default() += 1;
                        }
                        if args.get(1).is_some_and(|a| a == "-q") && t.param == 5 && !t.keys.is_empty() {
                            let f = |v: &[u32]| v.iter().map(|w| format!("{:.3}", f32::from_bits(*w))).collect::<Vec<_>>().join(",");
                            println!("{} {:08X} script {:08X} len {} base [{}] keys {:?}", p.name, o.hash, s.name, s.length, f(&t.base), t.keys.iter().map(|k| (k.0, f(&k.1))).collect::<Vec<_>>());
                        }
                        if args.get(1).is_some_and(|a| a == "-v") && (t.interpolation != 1 || t.flags != 0) {
                            println!("{} {:08X} script {:08X} len {} track p{} i{} f{} len {} keys {:?}", p.name, o.hash, s.name, s.length, t.param, t.interpolation, t.flags, t.length, t.keys.iter().map(|k| k.0).collect::<Vec<_>>());
                        }
                    }
                }
            }
        }
        // Tracks the loader leaves inactive (bit 7) although they have
        // keys, and INIT's keyless tracks whose base differs from `SA`.
        let (mut keyed_off, mut init_differs) = (0, 0);
        for p in assets.packages.values() {
            for o in &p.objects {
                for s in &o.scripts {
                    for t in &s.tracks {
                        keyed_off += (t.flags & 0x80 != 0 && !t.keys.is_empty()) as usize;
                        if s.name == 0x1744B3 && t.keys.is_empty() && t.base.iter().enumerate().any(|(i, w)| o.attributes.words.get(t.offset + i).is_some_and(|a| a != w)) {
                            init_differs += 1;
                            if args.get(1).is_some_and(|a| a == "-i") {
                                println!("{} {:08X} offset {} base {:?} SA {:?}", p.name, o.hash, t.offset, t.base, &o.attributes.words[t.offset..(t.offset + t.base.len()).min(o.attributes.words.len())]);
                            }
                        }
                    }
                }
            }
        }
        println!("keyed tracks with bit 7: {keyed_off}; INIT keyless tracks off SA: {init_differs}");
        println!("(param, interpolation, flags): {kinds:?}\nfirst key times: {first:?}\nscript flags: {sflags:?}");
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--career") {
        let root = nfsu2::game_root();
        let data = nfsu2::frontend::career::CareerData::load(&root)?;
        println!("{} events, {} tracks", data.events.len(), data.tracks.len());
        let mut numbers: Vec<_> = data.tracks.keys().copied().collect();
        numbers.sort();
        for n in numbers.iter().take(12) {
            let t = &data.tracks[n];
            println!("  track {n} {} {} map {} {} {} {}", t.name, t.region, t.map_left, t.map_top, t.map_size, t.map_zoom);
        }
        for h in [0x1185_EF08u32, 0x5E80_2BCE, 0xAB7A_688E, 0x04CC_E3E1, 0xDD60_E403] {
            for e in data.events.iter().filter(|e| e.unlock == h) {
                println!("  final {h:08X} {} {} stage {} {:?} {:?}", e.index, e.name, e.stage, e.kind, e.category);
            }
        }
        for e in data.events.iter().filter(|e| e.stage == 0) {
            let t = data.tracks.get(&e.track);
            println!("  stage0 {:3} {:22} {:?} {:?} laps {} ${} track {} {:?} car? flags", e.index, e.name, e.kind, e.category, e.laps, e.reward, e.track, t.map(|t| t.title()));
        }
        for stage in 1..=5 {
            let events = data.stage_events(&root, stage);
            println!("stage {stage}: {} events", events.len());
            for e in events.iter().take(40) {
                let t = &data.tracks[&e.track];
                let route = nfsu2::frontend::career::load_route(&root, t, e.variant == 1).map(|r| (r.checkpoints.len(), r.loops, r.length() as u32, r.grid.len(), r.grid.first().map(|g| (g.0, g.1.to_degrees() as i32)))).map_err(|e| e.to_string());
                println!("  {:3} {:22} {:?} {:?} laps {} ${} {} route {:?}", e.index, e.name, e.kind, e.category, e.laps, e.reward, t.title(), route);
            }
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--trackmap") {
        let mut assets = assets;
        let n: u16 = args[1].parse()?;
        let h = assets.track_map(&nfsu2::game_root(), n);
        let t = h.and_then(|h| assets.textures.get(&h));
        println!("{h:X?} {:?}", t.map(|t| (&t.name, t.width, t.height, t.format, t.blend, t.alpha_test, t.mip_count)));
        if let Some(t) = t {
            let rgba = t.decode_rgba();
            image::save_buffer(&args[2], &rgba, t.width, t.height, image::ColorType::Rgba8)?;
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--has") {
        for h in &args[1..] {
            let h = u32::from_str_radix(h.trim_start_matches("0x"), 16)?;
            println!("{h:08X} {:?}", assets.textures.get(&h).map(|t| (&t.name, t.width, t.height, t.format, t.blend)));
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--str") {
        // Look up language strings by hash (hex) or list those whose text matches.
        for a in &args[1..] {
            match u32::from_str_radix(a.trim_start_matches("0x"), 16) {
                Ok(h) if a.starts_with("0x") => println!("{h:08X} {:?}", assets.strings.get(h)),
                _ => {
                    for (h, t) in &assets.strings.map {
                        if t == a {
                            println!("{h:08X} {t:?}");
                        }
                    }
                }
            }
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--visible") {
        // Objects whose own colour is not transparent at 5 s, with their
        // parents and text.
        let p = assets.package(&args[1]).context("no such package")?;
        let state = nfsu2::frontend::ScreenState::default();
        for o in &p.objects {
            let a = nfsu2::frontend::animate(o, &state, 5000.0);
            if a.color()[3] > 0.0 {
                let text = o.label.and_then(|h| assets.strings.get(h)).map_or(o.text.clone(), str::to_string);
                let res = p.resources.get(o.resource as usize).map_or(String::new(), |r| r.name.clone());
                println!("{:08X} {:?} guid {:X} parent {:X?} {:?} {}", o.hash, o.kind, o.guid, o.parent, text, res);
            }
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--find") {
        let p = assets.package(&args[1]).context("no such package")?;
        for name in &args[2..] {
            let h = u32::from_str_radix(name.trim_start_matches("0x"), 16).ok().filter(|_| name.starts_with("0x")).unwrap_or_else(|| nfsu2::formats::hash::fe_hash(name));
            match p.objects.iter().find(|o| o.hash == h) {
                Some(o) => {
                    println!("{name:28} {h:08X} {:?} parent {:X?} flags {:08X} pos {:?} size {:?} color {:?}", o.kind, o.parent, o.flags, o.attributes.position(), o.attributes.size(), o.attributes.color());
                    for sc in &o.scripts {
                        println!("    script {:08X} len {} flags {} chain {:X?}", sc.name, sc.length, sc.flags, sc.chain);
                        for t in &sc.tracks {
                            println!("      track off {} param {} base {:X?} keys {:X?}", t.offset, t.param, t.base, t.keys);
                        }
                    }
                }
                None => println!("{name:28} {h:08X} missing"),
            }
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--dump") {
        let p = assets.package(&args[1]).context("no such package")?;
        for r in &p.resources {
            println!("res {:?} {}", r.kind, r.name);
        }
        for o in &p.objects {
            let a = o.attributes;
            let text = o.label.and_then(|h| assets.strings.get(h)).map_or(o.text.clone(), str::to_string);
            println!(
                "{:08X} {:?} guid {:X} parent {:X?} flags {:08X} res {} pos {:?} size {:?} rot {:?} color {:?} just {} text {:?} scripts {:?}",
                o.hash, o.kind, o.guid, o.parent, o.flags, o.resource as i32, a.position(), a.size(), a.rotation(), a.color(), o.justify, text,
                o.scripts.iter().map(|s| format!("{:X}", s.name)).collect::<Vec<_>>()
            );
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("--shot") {
        let time = args.iter().position(|a| a == "--time").and_then(|i| args.get(i + 1)).and_then(|t| t.parse().ok()).unwrap_or(5000.0);
        return shot(&assets, &args[1], &args[2], time);
    }
    println!("usage: frontend --list | --shot PACKAGE OUT.png [--time MS]");
    Ok(())
}
