//! The light-streak buffer's constants and the overbright level (moved from render/postfx.rs).

use super::*;

/// The overbright level 0x5BC190 / 0x5BC110 keep at 0x876848, stepped once
/// a frame after the frame is drawn (0x5D2B6A), so a frame uses the level
/// of the frames before it. The target is `0.2 + 0.8·a` (0x7FF0FC,
/// 0x7FF100) with `a` 0.45 (0x7A20B0) in the front end and 0 (0x7D7D3C) in
/// game, or in game the main view's effect slot 4 value (+0x1E0, when its
/// enable +0x3C is set: camera-track bloom, not modelled), or 0.2
/// (0x7FF7C8) while the byte 0x870904 is set (never written).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverbrightLevel(pub f32);

impl Default for OverbrightLevel {
    /// 0x876848's initial value in .data.
    fn default() -> Self {
        OverbrightLevel(2.672_888_5e-5)
    }
}

impl OverbrightLevel {
    pub fn target(front_end: bool) -> f32 {
        let a = if front_end { 0.45 } else { 0.0 };
        a * (1.0 - 0.2) + 0.2
    }

    /// 0x5BC110: level = level·(1 − 0.2) + target·0.2 (0x800B90).
    pub fn step(&mut self, front_end: bool) {
        self.0 = (1.0 - 0.2) * self.0 + 0.2 * Self::target(front_end);
    }

    /// Tick form: the level tick n's frame draws with is the
    /// one before that tick's step (0x5D2B6A runs after the frame is drawn:
    /// "use, then step"), so S_n = the level as tick n starts. A frame at α
    /// between ticks `prev` (S_{n−1}) and `cur` (S_n) draws the
    /// interpolated level, `prev` exactly at α = 0; [`OverbrightLevel::add`]
    /// and [`OverbrightLevel::offset`] then truncate it as the pass does.
    pub fn drawn(prev: OverbrightLevel, cur: OverbrightLevel, alpha: f32) -> OverbrightLevel {
        OverbrightLevel(crate::render::native::fx::tick::lerp(prev.0, cur.0, alpha))
    }

    /// The per-frame path over the tick form: one step, then the draw at α
    /// = 0. -> the level this frame uses; `self` is stepped.
    pub fn frame(&mut self, front_end: bool) -> OverbrightLevel {
        let prev = *self;
        self.step(front_end);
        OverbrightLevel::drawn(prev, *self, 0.0)
    }

    /// 0x7FF114 = trunc(10 + level·(72 − 10)) (0x7FF118 / 0x7FF11C): the
    /// glow's add factor in game, out of 255.
    pub fn add(self) -> u8 {
        (10.0 + self.0 * (72.0 - 10.0)) as i32 as u8
    }

    /// 0x7FF10C = trunc(216 + level·(62 − 216)) (0x7FF120 / 0x7FF124),
    /// out of 255; OVERBRIGHTOFFSET is half of it (0x5C642B).
    pub fn offset(self) -> u8 {
        (216.0 + self.0 * (62.0 - 216.0)) as i32 as u8
    }
}

/// The streak buffer's per-frame decay (0x5CBB6A: BLENDFACTOR 0xFF785AAA,
/// ZERO / BLENDFACTOR): R 0x78, G 0x5A, B 0xAA.
pub(super) const STREAK_DECAY: wgpu::Color =
    wgpu::Color { r: 120.0 / 255.0, g: 90.0 / 255.0, b: 170.0 / 255.0, a: 1.0 };

/// The light-streak buffer's format, for the glow sprites drawn into it.
pub const STREAK_FORMAT: wgpu::TextureFormat = INTERMEDIATE;

/// The streak buffer's texture: the post chain's
/// ([`texture`]), copyable both ways for [`StreakFrame::scratch`].
pub(super) fn streak_texture(gpu: &Gpu, (width, height): (u32, u32), format: wgpu::TextureFormat) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("streaks"),
        size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// Tick form of the streak buffer (frame-rate independence):
/// what one drawn frame does to it. The buffer C is tick-committed,
/// C_n = sat(q(k⊙C_{n−1}) + G_n) once per tick with that tick's glows; a
/// frame at α = 0 shows the last commit, a frame between ticks shows the
/// scratch B(α) = sat(q(k⊙C_{n−1}) + G(lerp state)), which is not kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreakFrame {
    /// Ticks committed in this frame, in order: each one decay plus that
    /// tick's glows ([`StreakGlow::Commit`]); 0, 1 or several.
    pub commits: u32,
    /// The frame lies between ticks (α > 0): after the commits, B(α) from
    /// the frame's own glows ([`StreakGlow::Frame`]) is shown, then C is
    /// restored.
    pub scratch: bool,
    /// A still of a running game (a shot's warm-up): the overbright level
    /// starts settled.
    pub still: bool,
}

impl StreakFrame {
    /// Today's per-frame path ([`PostFx::apply`]): `frames` commits of this
    /// frame's glows (`frames` > 1: a still), shown as committed.
    pub fn per_frame(frames: u32) -> StreakFrame {
        StreakFrame { commits: frames.max(1), scratch: false, still: frames > 1 }
    }

    /// A drawn frame in the tick form: `ticks` ticks were run since the last
    /// drawn frame (uncommitted), and the frame is drawn at `alpha`
    /// (snapped: 0 at a tick).
    pub fn tick(ticks: u32, alpha: f32) -> StreakFrame {
        StreakFrame { commits: ticks, scratch: alpha > 0.0, still: false }
    }
}

/// Which glow list a pass into the streak buffer draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreakGlow {
    /// The `i`th tick committed in this frame ([`StreakFrame::commits`]):
    /// that tick's glow-sprite list, from its snapshot S_m.
    Commit(u32),
    /// The drawn frame's own glows (its interpolated state): the scratch
    /// B(α), and the reflections' glow copy.
    Frame,
}

/// What a commit adds after the glow sprites.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct StreakAdd {
    /// The overbright pass's A · factor (0x5CCB9A), when it runs.
    pub(super) overbright: Option<f64>,
    /// min(·, 1) after the adds (High Precision Post's float buffer, as
    /// the 8-bit writes saturate).
    pub(super) saturate: bool,
}

impl PostFx {
    /// Tick form: the overbright level the next [`PostFx::apply`] /
    /// [`PostFx::apply_ticks`] draws with, from the game's tick-stepped
    /// level ([`OverbrightLevel::drawn`]); the post's own per-frame step is
    /// skipped for that frame.
    pub fn set_tick_level(&mut self, level: OverbrightLevel) {
        self.tick_level = Some(level);
    }

    /// Dev harness (the streak tests): the streak buffer, to read back.
    #[cfg(feature = "dev-harness")]
    pub fn dev_streak_texture(&self) -> Option<&wgpu::Texture> {
        self.targets.as_ref().map(|t| &t.streak)
    }

    /// One commit into `target` (0x5CBB10): the decay
    /// (BLENDFACTOR (120, 90, 170) / 255, ZERO / BLENDFACTOR), the glow
    /// list `which`, then A · factor and the saturation as `add` says.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn streak_commit(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        add: StreakAdd,
        which: StreakGlow,
        glow: &mut impl FnMut(&Self, StreakGlow, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) {
        self.draw(gpu, encoder, target, format, "fs_zero", Blend::ScaleDest, Src::Scene, COPY, Some(STREAK_DECAY), None);
        glow(self, which, encoder, target);
        if let Some(f) = add.overbright {
            let c = wgpu::Color { r: f, g: f, b: f, a: f };
            self.draw(gpu, encoder, target, format, "fs_filter", Blend::AddConstant, Src::A, COPY, Some(c), None);
        }
        if add.saturate {
            self.draw(gpu, encoder, target, format, "fs_one", Blend::Min, Src::Scene, COPY, None, None);
        }
    }

    /// One drawn frame's streak work ([`StreakFrame`]): its commits, then
    /// for a frame between ticks C_{n−1} kept and B(α) made in its place
    /// (restored by [`PostFx::streak_restore`] once the frame is composed).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn streak_frame(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        add: StreakAdd,
        streak: StreakFrame,
        glow: &mut impl FnMut(&Self, StreakGlow, &mut wgpu::CommandEncoder, &wgpu::TextureView),
    ) {
        for i in 0..streak.commits {
            self.streak_commit(gpu, encoder, target, format, add, StreakGlow::Commit(i), glow);
        }
        if streak.scratch {
            self.streak_keep(gpu, encoder);
            self.streak_commit(gpu, encoder, target, format, add, StreakGlow::Frame, glow);
        }
    }

    /// After a frame between ticks: the committed buffer back.
    pub(super) fn streak_restore(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let (Some(t), Some(keep)) = (self.targets.as_ref(), self.streak_keep.as_ref()) {
            encoder.copy_texture_to_texture(keep.as_image_copy(), t.streak.as_image_copy(), t.streak.size());
        }
    }

    /// The streak buffer C_{n−1} into the keep, made at its size and
    /// format on first use.
    fn streak_keep(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder) {
        let Some(t) = self.targets.as_ref() else { return };
        let (size, format) = (t.streak.size(), t.streak.format());
        if self.streak_keep.as_ref().is_none_or(|k| k.size() != size || k.format() != format) {
            self.streak_keep = Some(streak_texture(gpu, (size.width, size.height), format));
        }
        let (t, keep) = (self.targets.as_ref().unwrap(), self.streak_keep.as_ref().unwrap());
        encoder.copy_texture_to_texture(t.streak.as_image_copy(), keep.as_image_copy(), size);
    }
}

#[cfg(test)]
mod tick_tests {
    use super::*;
    use crate::render::native::fx::tick::{testkit, within};

    /// 600 ticks stepped = 600 frames of the per-frame code (verbatim:
    /// use the level, then step it), bit for bit, through the front end
    /// and the game; the draw at α = 0 is the old level, its truncations
    /// the old constants; between ticks the level lies within the two.
    #[test]
    fn overbright_tick_form_is_the_per_frame_path() {
        let mut legacy = OverbrightLevel::default();
        let mut per_frame = OverbrightLevel::default();
        let mut ticked = OverbrightLevel::default();
        let mut prev = ticked;
        for i in 0..600 {
            let front_end = (i / 120) % 2 == 0;
            // The per-frame code before the tick form.
            let used = legacy;
            legacy.step(front_end);
            // `PostFx::prepare_targets` now.
            let level = per_frame.frame(front_end);
            assert_eq!((level.0.to_bits(), per_frame.0.to_bits()), (used.0.to_bits(), legacy.0.to_bits()), "{i}");
            // The tick form: S_i is the level as tick i starts.
            let cur = ticked;
            ticked.step(front_end);
            assert_eq!(cur, used);
            let drawn = OverbrightLevel::drawn(prev, cur, 0.0);
            if i > 0 {
                assert_eq!(drawn, prev);
            }
            assert_eq!(OverbrightLevel::drawn(cur, ticked, 0.0).0.to_bits(), used.0.to_bits());
            assert_eq!((OverbrightLevel::drawn(cur, ticked, 0.0).add(), OverbrightLevel::drawn(cur, ticked, 0.0).offset()), (used.add(), used.offset()));
            for a in testkit::ALPHAS {
                let d = OverbrightLevel::drawn(prev, cur, a);
                assert!(within(d.0, prev.0, cur.0));
                let (lo, hi) = if prev.0 <= cur.0 { (prev, cur) } else { (cur, prev) };
                assert!(d.add() >= lo.add() && d.add() <= hi.add());
                assert!(d.offset() <= lo.offset() && d.offset() >= hi.offset());
            }
            prev = cur;
        }
    }

    #[test]
    fn streak_frames_of_the_per_frame_path_and_the_ticks() {
        assert_eq!(StreakFrame::per_frame(0), StreakFrame { commits: 1, scratch: false, still: false });
        assert_eq!(StreakFrame::per_frame(1), StreakFrame { commits: 1, scratch: false, still: false });
        assert_eq!(StreakFrame::per_frame(16), StreakFrame { commits: 16, scratch: false, still: true });
        assert_eq!(StreakFrame::tick(2, 0.0), StreakFrame { commits: 2, scratch: false, still: false });
        assert_eq!(StreakFrame::tick(0, 0.5), StreakFrame { commits: 0, scratch: true, still: false });
    }
}

/// The streak buffer's exact-path tests on the device (`streak_gpu_tests.rs`).
#[cfg(test)]
#[path = "streak_gpu_tests.rs"]
mod gpu_tests;
