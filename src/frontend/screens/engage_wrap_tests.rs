//! The engage dialogs' message
//! boxes broken as 0x51C110 draws them (`wrap_feng`).

use crate::frontend::career::CareerData;
use crate::frontend::{Assets, wrap_feng};

fn message_box<'a>(assets: &'a Assets, package: &str, hash: u32) -> (&'a crate::frontend::font::Font, f32) {
    let p = assets.package(package).unwrap();
    let o = p.object(hash).unwrap();
    let r = p.resources.get(o.resource as usize).unwrap();
    (assets.font_for(&r.name).unwrap(), o.max_width as f32)
}

fn blank_lines(wrapped: &str) -> usize {
    wrapped.trim_end_matches('\n').split('\n').filter(|l| l.trim().is_empty()).count()
}

/// 0x51C110: a line ending at "^" after the first wrap gets a blank line
/// after it only when its drawn width reaches its measure before the "^";
/// the draw kerns the line's first character against the one before it
/// (local_2c runs over the whole string), the measure (0x50BCE0) does
/// not. Under Wine (the original's career at stage 4): the Coal Harbor East graphics
/// shop's text has no blank line before "All parts…" (" V" kerns), the
/// performance shop's has one before "All parts…" (" B" does not).
#[test]
#[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
fn shop_engage_blank_line_follows_the_draws_kerning() {
    let root = crate::testkit::game_root();
    let assets = Assets::load(&root).unwrap();
    let (font, width) =
        message_box(&assets, "UI_EngageShopDialog", crate::frontend::fe_ids::SHOP_ENGAGE_MESSAGE);
    let gfx = assets.strings.label("IND_GRAPHICS_BODY");
    let perf = assets.strings.label("IND_PERF_SHOP_2_BODY");
    let g = wrap_feng(font, &gfx, width);
    let p = wrap_feng(font, &perf, width);
    assert_eq!(blank_lines(&g), 0, "graphics shop (Wine: no blank line):\n{g}");
    assert_eq!(blank_lines(&p), 1, "performance shop (Wine: one blank line):\n{p}");
    assert!(g.contains("\nVinyls. \nAll parts"), "{g}");
    assert!(p.contains("upgrades. \n\nAll parts"), "{p}");
}

/// Every shop's first-visit text and every race / challenge engage text,
/// broken as the original draws them: prints the ones with a blank line
/// (diagnostic, run with `--ignored --nocapture`).
#[test]
#[ignore = "diagnostic: lists the engage texts that get a blank line"]
fn list_engage_blank_lines() {
    let root = crate::testkit::game_root();
    let assets = Assets::load(&root).unwrap();
    let data = CareerData::load(&root).unwrap();
    let (font, width) =
        message_box(&assets, "UI_EngageShopDialog", crate::frontend::fe_ids::SHOP_ENGAGE_MESSAGE);
    for shop in &data.shops {
        let label = format!("{}_BODY", shop.name);
        if let Some(t) = assets.strings.get(crate::formats::hash::bin_hash(&label)) {
            let w = wrap_feng(font, t, width);
            println!("shop {label}: blank {} lines {}", blank_lines(&w), w.trim_end().split('\n').count());
        }
    }
    if let Some(t) = assets.strings.get(0xC4DF_3B44) {
        let w = wrap_feng(font, t, width);
        println!("retry C4DF3B44: blank {}", blank_lines(&w));
    }
    let (font, width) = message_box(&assets, "UI_EngageRaceDialog", crate::frontend::fe_ids::ENGAGE_MESSAGE);
    let mut labels: Vec<String> = data.events.iter().map(|e| format!("ENGAGE_{}", e.name)).collect();
    for (set, n) in [("URL", 12), ("CIRCUIT", 18), ("SPRINT", 15), ("STREET", 14), ("DRAG", 12), ("DRIFT", 13)] {
        labels.extend((1..=n).map(|i| format!("ENGAGE_{set}_RANDOM_{i}")));
    }
    for label in labels {
        if let Some(t) = assets.strings.get(crate::formats::hash::bin_hash(&label)) {
            let w = wrap_feng(font, t, width);
            let b = blank_lines(&w);
            if b > 0 || t.contains('^') {
                println!("race {label}: blank {b}");
            }
        }
    }
}
