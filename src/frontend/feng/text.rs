//! Text and boxes on the canvas: [`push_box`], [`string_quads`], the line counters and wrappers.

use glam::{Mat4, Vec2, Vec3};

use crate::frontend::assets::Assets;
use crate::frontend::feng::font::Font;
use crate::frontend::feng::state::{CANVAS, Quad};
use crate::render::display::Place;

/// Map the local box `lo`..`hi` through `world` onto the canvas.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_box(
    out: &mut Vec<Quad>,
    texture: Option<u32>,
    world: &Mat4,
    lo: Vec2,
    hi: Vec2,
    uv: [Vec2; 2],
    color: [f32; 4],
    z: f32,
) {
    let centre = world.transform_point3(((lo + hi) * 0.5).extend(0.0)).truncate() + CANVAS * 0.5;
    let mut ax = world.transform_vector3(Vec3::new(hi.x - lo.x, 0.0, 0.0)).truncate();
    let ay = world.transform_vector3(Vec3::new(0.0, hi.y - lo.y, 0.0)).truncate();
    // Unturned (up to float noise: a 180° turn about x or y through the
    // quaternion leaves ~1e-8 off the axes, which drew MC_Main's and
    // UI_PC_LAN_ServerSelect's x-mirrored top-right corners as turned by
    // −π, a pixel off with the texture wrapped — the "notch").
    let flat = |v: f32, len: f32| v.abs() <= len * 1e-5;
    let (half, angle) = if flat(ax.y, ax.x.abs()) && flat(ay.x, ay.y.abs()) {
        let (ax, ay) = (Vec2::new(ax.x, 0.0), Vec2::new(0.0, ay.y));
        // Unturned: keep the signs (a negative size mirrors).
        (Vec2::new(ax.x, ay.y) * 0.5, 0.0)
    } else {
        // Mirrored and turned: turned by the mirrored x axis, the box
        // mirrored in x (a negative half width), so each corner keeps its
        // own UV: the half-texel inset (0x5C5030) goes by the game's
        // vertex order (0x51A1C0: vertex 0 the local (−0.5, −0.5) corner
        // with (u0, v0)), inward in the object's UV rectangle. Swapping
        // the UVs instead pushed it outward and, the sampler wrapping,
        // showed the far edge's texels as a 1-pixel line at both sides of
        // mirrored corner pieces (the dialog boxes' corner notches).
        let mirrored = ax.perp_dot(ay) < 0.0;
        if mirrored {
            ax = -ax;
        }
        let sx = if mirrored { -ax.length() } else { ax.length() };
        (Vec2::new(sx, ay.length()) * 0.5, ax.y.atan2(ax.x))
    };
    out.push(Quad { texture, rect: [centre - half, centre + half], uv, color, z, angle, place: Place::CENTRE });
}

/// Glyph quads for one string at `scale` × font pixels with its origin at
/// `at` on the canvas (see [`string_quads`] for the justification bits).
#[allow(clippy::too_many_arguments)]
pub fn text_quads(
    assets: &Assets,
    font: &Font,
    text: &str,
    justify: u32,
    scale: Vec2,
    at: Vec2,
    color: [f32; 4],
    z: f32,
    out: &mut Vec<Quad>,
) {
    let world = Mat4::from_translation((at - CANVAS * 0.5).extend(0.0)) * Mat4::from_scale(scale.extend(1.0));
    string_quads(assets, font, text, justify, 0, None, &world, color, z, out);
}

/// Glyph quads for one string in its object's space (0x51C110). Lines
/// ("\n" or "^") step by the leading (`Sl`, × the font's leading scale,
/// 1 for the Latin fonts) plus the font's line height. Justification
/// (`Sj`): 1 centres each line on the origin, 2 ends it there; 4 centres
/// the block vertically (lines × step), else 8 ends it there (the height
/// is the steps between lines plus one line height, 0x50BF40). A glyph
/// sits at the pen plus its offsets, max(width, 4) × height font pixels
/// over its texture cell widened by a texel all round (0x51BFC0); the
/// pen moves by its advance plus the kerning after the previous
/// character (0x6F45F0; also in the width, 0x50BCE0).
#[allow(clippy::too_many_arguments)]
pub fn string_quads(
    assets: &Assets,
    font: &Font,
    text: &str,
    justify: u32,
    leading: i32,
    count: Option<usize>,
    world: &Mat4,
    color: [f32; 4],
    z: f32,
    out: &mut Vec<Quad>,
) {
    let texture = crate::formats::hash::upper_hash(&font.texture);
    let texture = assets.textures.contains_key(&texture).then_some(texture);
    let lines: Vec<&str> = text.trim_end_matches('\n').split(['^', '\n']).collect();
    let step = leading as f32 + font.line_height;
    let mut y = if justify & 5 != 0 {
        let height = step * count.unwrap_or(lines.len()) as f32;
        if justify & 4 != 0 {
            -height * 0.5
        } else if justify & 8 != 0 {
            -height
        } else {
            0.0
        }
    } else if justify & 8 != 0 {
        -(step * (lines.len() - 1) as f32 + font.line_height)
    } else {
        0.0
    };
    let ts = Vec2::from(font.texture_size);
    let mut previous = None;
    for line in lines {
        let width = font.measure(line);
        let mut x = if justify & 1 != 0 {
            -width * 0.5
        } else if justify & 2 != 0 {
            -width
        } else {
            0.0
        };
        for ch in line.chars() {
            let Some(g) = font.glyphs.get(&(ch as u32)) else {
                previous = Some(ch);
                continue;
            };
            if let Some(p) = previous {
                x += g.kerning(p);
            }
            if g.width > 0.0 && g.height > 0.0 {
                let p0 = Vec2::new(x + g.x_offset, y + g.y_offset);
                let p1 = p0 + Vec2::new(g.width.max(4.0), g.height);
                let uv =
                    [Vec2::new(g.x - 1.0, g.y - 1.0) / ts, Vec2::new(g.x + g.width + 1.0, g.y + g.height + 1.0) / ts];
                push_box(out, texture, world, p0, p1, uv, color, z);
            }
            x += g.advance;
            previous = Some(ch);
        }
        previous = Some('\n');
        y += step;
    }
}

/// 0x50C080 with the string's wrap width (`Sw`, +0x74): the line count
/// a `Sj` & 5 block is centred by. A "\n" / "^" counts as in
/// [`line_count`]; the width (advances, kerning after the line's first
/// character) breaks the line once it reaches the limit (limit <= width)
/// with more text to come and a space seen since the last break, the
/// scan resuming past the spaces there. The renderer's blank lines
/// ([`wrap_feng`]) are not counted.
pub fn line_count_wrapped(font: &Font, text: &str, limit: f32) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let (mut count, mut i, mut space, mut width, mut k, mut fresh) = (1, 0, None::<usize>, 0.0f32, 0usize, false);
    while i < chars.len() {
        let c = chars[i];
        if (c == '\n' || c == '^')
            && let Some(at) = space
        {
            i = at + 1;
            (count, width, space, fresh) = (count + 1, 0.0, None, true);
        } else if c == '\r' {
            i += 1;
        } else {
            if fresh {
                (k, fresh) = (0, false);
            }
            if c == ' ' {
                space = Some(i);
            }
            if let Some(g) = font.glyphs.get(&(c as u32)) {
                if k != 0 && i > 0 {
                    width += g.kerning(chars[i - 1]);
                }
                width += g.advance;
            }
            i += 1;
            if limit > 0.0
                && i < chars.len()
                && limit <= width
                && let Some(at) = space
            {
                i = at;
                while chars.get(i) == Some(&' ') {
                    i += 1;
                }
                (count, width, space, fresh) = (count + 1, 0.0, None, true);
            }
        }
        k += 1;
    }
    count
}

/// 0x50C080 with no wrap width: the line count the block height of a
/// `Sj` & 5 string takes (0x51C110). A "\n" or "^" counts only after a
/// space on its line (the scan then resumes after that space, so the
/// last word is read again on the new line); with none it is passed over
/// as a character, though the drawing still breaks there. So "Winner!\n
/// Your time: 10.12" counts one line and stands half a line lower than
/// "You finished second!\nYour time: …" (**verified**; **observed** under
/// Wine, finish151: "Winner!" 36 px lower at 1280×960).
pub fn line_count(text: &str) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let (mut count, mut i, mut space) = (1, 0, None);
    while i < chars.len() {
        let c = chars[i];
        if (c == '\n' || c == '^')
            && let Some(at) = space
        {
            i = at + 1;
            count += 1;
            space = None;
            continue;
        }
        if c == ' ' {
            space = Some(i);
        }
        i += 1;
    }
    count
}

/// Break an FEng string as its renderer does (0x51C110, the line widths
/// from 0x50BCE0, the count 0x50C080), at the string's own wrap width
/// (`Sw`, the FEString's +0x74, read at 0x53504E; no margin).
///
/// **Verified** (code): 0x51C110 measures each line before
/// drawing it (0x50BCE0 from the line's first character): the advances
/// (+0xE) plus, after the line's first character, the kerning after the
/// previous character (0x6F45F0), up to a "\n" / "^" (trailing spaces
/// included) or the end; once that width reaches the limit (>=) with
/// more text to come the measure stops and returns the width before the
/// last space seen (when above 0), and raises the string's wrap flag.
/// The flag is cleared once per string and never again. The drawing
/// adds the same advances, but its kerning counter (local_2c) runs on
/// over the whole string, so a line's first character is kerned against
/// the character before it (the "^" or the space the wrap skipped).
/// After each drawn character, with the flag up, a drawn width that has
/// reached the measured one (measured <= drawn) ends the line: the
/// spaces after it are skipped and the next line measured. A "\n" /
/// "^" then still breaks once more. So a line that ends at a "^" after
/// the first wrap leaves a blank line when its drawn width reaches its
/// measure before the "^", i.e. unless its first character kerns
/// negatively against the character before it: the graphics shop's
/// "Vinyls. ^" (" V" kerns −1 in the dialog font) has no blank line,
/// the performance shop's "Brakes performance upgrades. ^" has one
/// (observed under Wine in a stage-4 playthrough; the lobby-cancel
/// question's blank line, round 3C). A line without a space runs on.
pub fn wrap_feng(font: &Font, text: &str, limit: f32) -> String {
    let chars: Vec<char> = text.chars().collect();
    let advance = |i: usize, kern: bool| -> f32 {
        font.glyphs.get(&(chars[i] as u32)).map_or(0.0, |g| {
            if kern && i > 0 { g.kerning(chars[i - 1]) + g.advance } else { g.advance }
        })
    };
    // 0x50BCE0 from `start`: the measured width and, when the limit
    // stopped it, the index of the last space (the drawn line's end).
    let measure = |start: usize, flag: &mut bool| -> (f32, Option<usize>) {
        let (mut width, mut before_space, mut space) = (0.0f32, 0.0f32, None);
        let mut i = start;
        while i < chars.len() {
            let c = chars[i];
            if c == '\n' || c == '^' {
                break;
            }
            if c != '\r' {
                if c == ' ' {
                    before_space = width;
                    space = Some(i);
                }
                width += advance(i, i != start);
            }
            i += 1;
            if c != '\r' && limit > 0.0 && i < chars.len() && limit <= width {
                *flag = true;
                if before_space > 0.0 {
                    return (before_space, space);
                }
                return (width, None);
            }
        }
        (width, None)
    };
    let mut flag = false;
    let mut out = String::new();
    let (mut measured, mut cut) = measure(0, &mut flag);
    let (mut line_start, mut drawn, mut i) = (0usize, 0.0f32, 0usize);
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' || c == '^' {
            out.extend(&chars[line_start..i]);
            out.push('\n');
            i += 1;
            if i >= chars.len() {
                return out;
            }
            (measured, cut) = measure(i, &mut flag);
            (line_start, drawn) = (i, 0.0);
            continue;
        }
        // The drawing's kerning counter is 0 only at the string's start.
        drawn += advance(i, i > 0);
        i += 1;
        if limit > 0.0 && flag && measured <= drawn {
            // A measure cut at a space draws to that space (the measured
            // width, which the line is centred by, ends there).
            let end = cut.filter(|&at| at >= line_start && at <= i).unwrap_or(i);
            out.extend(&chars[line_start..end]);
            out.push('\n');
            while chars.get(i) == Some(&' ') {
                i += 1;
            }
            (measured, cut) = measure(i, &mut flag);
            (line_start, drawn) = (i, 0.0);
        }
    }
    out.extend(&chars[line_start..]);
    out
}

/// Break `text` into lines no wider than `width` font pixels, at the last
/// space before the limit (the sponsor terms' breaker, 0x520080: the
/// row's box width less 16; "\n" and "^" always break).
pub fn wrap(font: &Font, text: &str, width: f32) -> String {
    let mut out = String::new();
    for (i, line) in text.split(['\n', '^']).enumerate() {
        if i > 0 {
            out.push('\n');
        }
        // Leading spaces are kept (UI_Status_Career indents its rows with
        // four, 0x797670, and the game draws them indented): only a word
        // after a break starts a line without its space.
        let mut current: Option<String> = None;
        for word in line.split(' ') {
            let candidate = match &current {
                None => word.to_string(),
                Some(c) => format!("{c} {word}"),
            };
            if font.measure(&candidate) > width && current.as_ref().is_some_and(|c| !c.trim().is_empty()) {
                out.push_str(current.as_deref().unwrap_or(""));
                out.push('\n');
                current = Some(word.to_string());
            } else {
                current = Some(candidate);
            }
        }
        out.push_str(current.as_deref().unwrap_or(""));
    }
    out
}
