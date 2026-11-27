//! docs/notes/graphics/resolutions.md: a developer-harness-only dump of where the
//! UI pass put things, for `scripts/checks/resolutions.sh`. Not native, no
//! effect on any image: with `NFSU2_UI_GEOM=<file>` set, every
//! [`super::UiRenderer::record_encoder`] call writes (overwriting) one JSON
//! object to `<file>`: the target size, the aspect mode, the UI frame
//! (`display::UiFrame`: scale, canvas origin, reaches), the front-end scene
//! bars, each quad's canvas rectangle, place, texture and the pixel box its
//! vertices cover (continuations included) with the scissor it is drawn
//! under, the mouse's recorded rectangles with a hit test at the centre of
//! what was drawn for each, and the game-FOV projections the 3D views used
//! since the last dump. The last call of a shot is its final frame.
//! Without the variable nothing is collected.

use std::fmt::Write as _;
use std::sync::{Mutex, OnceLock};

use crate::draw::quad::Quad;
use crate::render::display::{AspectMode, Place, UiFrame};

/// One drawn object's rectangle on the canvas and its placement (the mouse's record of the layout).
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub lo: glam::Vec2,
    pub hi: glam::Vec2,
    pub place: Place,
}

/// The mouse's side of the dump, installed by the menus layer (the renderer does not name it).
pub struct MouseHooks {
    /// Start recording this layout's rectangles.
    pub begin_frame: fn(),
    /// The rectangles recorded, sorted.
    pub rects: fn() -> Vec<((String, u32), Rect)>,
    /// Whether the cursor (target pixels) lies in the rectangle.
    pub inside: fn(&UiFrame, glam::Vec2, &Rect) -> bool,
}

static MOUSE: OnceLock<MouseHooks> = OnceLock::new();

/// The menus layer's mouse module hands over its hooks (once, before the first shot lays out).
pub fn install_mouse(hooks: MouseHooks) {
    let _ = MOUSE.set(hooks);
}

struct DrawnQuad {
    index: usize,
    texture: Option<String>,
    flat: bool,
    rect: [glam::Vec2; 2],
    angle: f32,
    place: Place,
    /// Pixel box of the vertices (x0, y0, x1, y1).
    px: [f32; 4],
    scissor: (u32, u32, u32, u32),
}

#[derive(Default)]
struct State {
    quads: Vec<DrawnQuad>,
    /// (aspect, fov, hw, across, m00, m11, count).
    projections: Vec<(f32, f32, f32, f32, f32, f32, u32)>,
    fov_consts: (f32, f32),
    frames: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// The dump's file (`NFSU2_UI_GEOM`), read once.
fn target() -> Option<&'static std::path::Path> {
    crate::render::debug_env::once::ui_geom().as_deref()
}

fn with(f: impl FnOnce(&mut State)) {
    if target().is_none() {
        return;
    }
    if let Ok(mut s) = STATE.lock() {
        f(s.get_or_insert_with(State::default));
    }
}

/// Before a shot lays its screen out: with the dump on, the mouse records
/// this layout's rectangles (`mouse::begin_frame(true)`; the quads are the
/// same either way), so [`finish`] can hit-test them.
pub fn begin_layout() {
    if target().is_some() {
        if let Some(m) = MOUSE.get() {
            (m.begin_frame)();
        }
    }
}

/// One quad as `record_encoder` built it: `vertices` its own (all its
/// pieces), in clip space on a `size` target.
#[allow(clippy::too_many_arguments)]
pub(super) fn quad(
    index: usize,
    q: &Quad,
    texture: Option<&str>,
    flat: bool,
    vertices: &[super::Vertex],
    scissor: (u32, u32, u32, u32),
    size: (u32, u32),
) {
    with(|s| {
        let (tw, th) = (size.0.max(1) as f32, size.1.max(1) as f32);
        let mut px = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for v in vertices {
            let (x, y) = ((v.position[0] + 1.0) * 0.5 * tw, (1.0 - v.position[1]) * 0.5 * th);
            px = [px[0].min(x), px[1].min(y), px[2].max(x), px[3].max(y)];
        }
        s.quads.push(DrawnQuad {
            index,
            texture: texture.map(str::to_string),
            flat,
            rect: q.rect,
            angle: q.angle,
            place: q.place,
            px,
            scissor,
        });
    });
}

fn place_json(p: Place) -> String {
    format!("{{\"x\":{},\"y\":{},\"fill\":{},\"stretch\":{},\"hud\":{}}}", p.x, p.y, p.fill, p.stretch, p.hud)
}

fn esc(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            c if (c as u32) < 0x20 => vec![' '],
            c => vec![c],
        })
        .collect()
}

/// The end of a `record_encoder` call: write the dump.
pub(super) fn finish(frame: &UiFrame, size: (u32, u32)) {
    let Some(path) = target() else { return };
    // The game-FOV projections are recorded by the camera (`draw::camera::projections`).
    let views = crate::draw::camera::projections::take();
    let Some(mut s) = STATE.lock().ok().and_then(|mut g| g.take()).or_else(|| views.is_some().then(State::default)) else {
        return;
    };
    if let Some((list, consts)) = views {
        s.projections = list;
        s.fov_consts = consts;
    }
    s.frames += 1;
    let mode = match crate::render::display::aspect_mode() {
        AspectMode::Wide => "wide",
        AspectMode::Stretched => "stretched",
    };
    let canvas = crate::render::display::canvas_rect(size);
    let bars = crate::render::display::scene_bars(size).filter(|_| crate::render::display::scene_capped());
    let mut o = String::with_capacity(64 * 1024);
    let _ = write!(
        o,
        "{{\"size\":[{},{}],\"aspect_mode\":\"{mode}\",\"hud_full_width\":{},\"canvas\":[{},{},{},{}],\"scale\":[{},{}],\"origin\":[{},{}],\"reach\":[{},{}],\"hud_reach\":{},\"scene_bars\":{},",
        size.0,
        size.1,
        crate::render::display::hud_full_width(),
        canvas.0,
        canvas.1,
        canvas.2,
        canvas.3,
        frame.scale.0,
        frame.scale.1,
        frame.origin.0,
        frame.origin.1,
        frame.reach.0,
        frame.reach.1,
        frame.hud_reach,
        bars.map_or("null".to_string(), |(x, f)| format!("[{x},{f}]")),
    );
    o.push_str("\"quads\":[");
    for (i, q) in s.quads.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let _ = write!(
            o,
            "{{\"i\":{},\"tex\":{},\"flat\":{},\"rect\":[{},{},{},{}],\"angle\":{},\"place\":{},\"px\":[{},{},{},{}],\"scissor\":[{},{},{},{}]}}",
            q.index,
            q.texture.as_deref().map_or("null".to_string(), |t| format!("\"{}\"", esc(t))),
            q.flat,
            q.rect[0].x,
            q.rect[0].y,
            q.rect[1].x,
            q.rect[1].y,
            q.angle,
            place_json(q.place),
            q.px[0],
            q.px[1],
            q.px[2],
            q.px[3],
            q.scissor.0,
            q.scissor.1,
            q.scissor.2,
            q.scissor.3,
        );
    }
    o.push_str("],\"mouse\":[");
    // The mouse's rectangles of this frame's layout (when it recorded):
    // per object, the pixel box of the quads drawn inside its canvas
    // rectangle with its place, and the game's own hit test
    // (`mouse::inside` on the frame the mouse uses) at that box's centre.
    let mouse_frame = UiFrame::new(size);
    for (i, ((package, hash), r)) in MOUSE.get().map(|m| (m.rects)()).unwrap_or_default().into_iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let eps = 0.01;
        let mut px = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for q in &s.quads {
            let (lo, hi) = (q.rect[0].min(q.rect[1]), q.rect[0].max(q.rect[1]));
            let same = q.place.x == r.place.x && q.place.y == r.place.y && q.place.hud == r.place.hud;
            if same
                && !q.place.stretch
                && lo.x >= r.lo.x - eps
                && lo.y >= r.lo.y - eps
                && hi.x <= r.hi.x + eps
                && hi.y <= r.hi.y + eps
                && hi.x - lo.x > eps
                && hi.y - lo.y > eps
            {
                px = [px[0].min(q.px[0]), px[1].min(q.px[1]), px[2].max(q.px[2]), px[3].max(q.px[3])];
            }
        }
        let drawn = px[0] <= px[2];
        let (hit, expect) = if drawn {
            let c = glam::Vec2::new((px[0] + px[2]) * 0.5, (px[1] + px[3]) * 0.5);
            let at = mouse_frame.to_pixels((r.lo + r.hi) * 0.5, r.place);
            (MOUSE.get().map_or(false, |m| (m.inside)(&mouse_frame, c, &r)).to_string(), format!("[{},{}]", at.x, at.y))
        } else {
            ("null".to_string(), "null".to_string())
        };
        let _ = write!(
            o,
            "{{\"pkg\":\"{}\",\"hash\":{},\"lo\":[{},{}],\"hi\":[{},{}],\"place\":{},\"drawn_px\":{},\"hit\":{hit},\"mouse_centre_px\":{expect}}}",
            esc(&package),
            hash,
            r.lo.x,
            r.lo.y,
            r.hi.x,
            r.hi.y,
            place_json(r.place),
            if drawn { format!("[{},{},{},{}]", px[0], px[1], px[2], px[3]) } else { "null".to_string() },
        );
    }
    o.push_str("],\"projections\":[");
    for (i, p) in s.projections.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let _ = write!(
            o,
            "{{\"aspect\":{},\"fov\":{},\"hw\":{},\"across\":{},\"m00\":{},\"m11\":{},\"n\":{}}}",
            p.0, p.1, p.2, p.3, p.4, p.5, p.6
        );
    }
    let _ = write!(o, "],\"fov_consts\":[{},{}],\"ui_calls\":{}}}", s.fov_consts.0, s.fov_consts.1, s.frames);
    o.push('\n');
    let _ = std::fs::write(path, o);
    // The count of calls carries over; quads and projections start anew.
    if let Ok(mut g) = STATE.lock() {
        *g = Some(State { frames: s.frames, fov_consts: s.fov_consts, ..State::default() });
    }
}
