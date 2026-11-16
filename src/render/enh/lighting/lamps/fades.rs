use super::*;
/// Pop meter: a world lamp (not a car's light) in the view and in
/// reach whose added light changed by half or more in one frame (its
/// section joined or left the drawn set at once, or the light cap moved);
/// a change that begins a fade counts as gradual. `share(i)`: the share of
/// lamp `i`'s light drawn this frame (0 not kept).
pub(in crate::render) fn meter_lights(
    lamps: &[Lamp],
    share: &dyn Fn(usize) -> f32,
    view_proj: Mat4,
    eye: Vec3,
    forward: Vec3,
    opts: &crate::render::RenderOptions,
) {
    use std::collections::HashMap;
    static PREV: std::sync::Mutex<Option<HashMap<[u32; 3], f32>>> = std::sync::Mutex::new(None);
    let planes = super_planes(view_proj);
    let scale = crate::render::cull_scale(70f32.to_radians());
    let mut now = HashMap::new();
    let mut prev = PREV.lock().unwrap();
    for (i, l) in lamps.iter().enumerate() {
        if l.section == u32::MAX {
            continue;
        }
        let p = l.light.position;
        let r = l.light.outer;
        let dist = (p - eye).length();
        let inside = dist - r <= REACH
            && (p - eye).dot(forward) >= -r
            && !planes.iter().any(|pl| pl.truncate().dot(p) + pl.w < -r);
        if !inside {
            continue;
        }
        let key = p.to_array().map(f32::to_bits);
        let v = share(i);
        if let Some(&was) = prev.as_ref().and_then(|m| m.get(&key)) {
            let px = crate::render::pop_meter::px(scale * r / (dist - r).max(r), opts);
            if (v - was).abs() >= 0.5 {
                crate::render::pop_meter::event(crate::render::pop_meter::Source::LampLight, px, 0.0);
            } else if (was == 0.0) != (v == 0.0) && was <= v {
                crate::render::pop_meter::event(
                    crate::render::pop_meter::Source::LampLight,
                    px,
                    crate::render::RenderOptions::SECTION_FADE_SECONDS,
                );
            }
        }
        now.insert(key, v);
    }
    *prev = Some(now);
}

/// World Detail Ultra / Extreme with LOD Fade; docs/pop-in.md: the
/// lamps' light follows their section's cross-fade (a share per section
/// moving over `qol::SECTION_FADE_SECONDS`), and a lamp's shadow fades in
/// over [`SHADOW_FADE_SECONDS`] from the frame its layer is handed out.
#[derive(Default)]
pub(in crate::render) struct PopFades {
    time: f32,
    eye: Vec3,
    /// Last frame's drawn sections.
    visible: Vec<bool>,
    /// The sections part-way in or out (section → share).
    pub(in crate::render) sections: std::collections::HashMap<u32, f32>,
}

/// Lamp index → the frame time its shadow layer was handed out.
pub(in crate::render) static POP_SHADOWS: std::sync::Mutex<Option<std::collections::HashMap<usize, f32>>> =
    std::sync::Mutex::new(None);

pub(in crate::render) static POP_FADES: std::sync::Mutex<Option<PopFades>> = std::sync::Mutex::new(None);
/// A lamp shadow's fade-in (s).
pub const SHADOW_FADE_SECONDS: f32 = 0.6;

impl PopFades {
    /// This frame's section shares toward `visible` (a first frame or an
    /// eye jump over 30 m: at once).
    pub(super) fn step(&mut self, visible: &[bool], time: f32, eye: Vec3) {
        let snap = self.visible.is_empty() || self.eye.distance(eye) > 30.0 || time < self.time;
        let dt = (time - self.time).clamp(0.0, 0.1);
        self.time = time;
        self.eye = eye;
        if snap {
            self.sections.clear();
            self.visible = visible.to_vec();
            return;
        }
        for (s, &on) in visible.iter().enumerate() {
            if on != self.visible.get(s).copied().unwrap_or(false) {
                self.sections.entry(s as u32).or_insert(if on { 0.0 } else { 1.0 });
            }
        }
        let step = dt / crate::render::RenderOptions::SECTION_FADE_SECONDS;
        self.sections.retain(|&s, v| {
            let on = visible.get(s as usize).copied().unwrap_or(false);
            *v = if on { (*v + step).min(1.0) } else { (*v - step).max(0.0) };
            if on { *v < 1.0 } else { *v > 0.0 }
        });
        self.visible = visible.to_vec();
    }

    /// The time the shares last moved at (the shadows' fade-in reads it).
    pub(in crate::render) fn time(&self) -> f32 {
        self.time
    }

    /// A section's lamp share now.
    pub(super) fn share(&self, s: u32) -> f32 {
        self.sections
            .get(&s)
            .copied()
            .unwrap_or_else(|| if self.visible.get(s as usize).copied().unwrap_or(false) { 1.0 } else { 0.0 })
    }
}
