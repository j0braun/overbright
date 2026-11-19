//! LOD Fade (clone deviation): per-instance fade state and its per-frame update (moved from render/mod.rs).

use crate::render::*;

/// LOD Fade (clone deviation, docs/formats.md "Graphics enhancements"):
/// what each instance slot of the main view's scenery shows. Each frame
/// [`WorldRenderer::update_fades`] sizes the scenery instances of the
/// drawn sections as the native cull does (0x5D64E0, without its behind
/// test, so turning the view fades nothing) and moves each one's fade
/// toward drawn (size at least the main view's need) or not (below
/// [`crate::render::RenderOptions::LOD_FADE_KEEP`] of it once drawn) by the frame's time
/// over [`crate::render::RenderOptions::LOD_FADE_SECONDS`]; an instance of a section that
/// was not drawn the frame before takes its state at once (the section
/// itself appears at once, as natively). The fades go to the GPU as a
/// per-instance vertex attribute (only the slots that changed).
pub(in crate::render) struct FadeState {
    /// One f32 per instance slot (the shown share; negative while fading
    /// out: the complementary screen-door pattern) (location 7 of the fade pipelines).
    pub(in crate::render) buffer: wgpu::Buffer,
    /// The fades as the GPU has them (1 for every slot not scenery).
    values: std::sync::Arc<Vec<f32>>,
    /// Drawn by the hysteresis' rule (the fade's target).
    on: Vec<bool>,
    /// The frame the slot was last sized in.
    stamp: Vec<u32>,
    frame: u32,
    /// The frame time the fades last moved at (`FrameParams::time`).
    time: f32,
    /// The sections the main view drew last frame (by number), and of
    /// those the ones no longer listed, still fading out.
    sections: Vec<bool>,
    lingering: Vec<u32>,
    /// The same for World Detail Extreme's far sections (their low LOD).
    far_sections: Vec<bool>,
    far_lingering: Vec<u32>,
    /// The eye last frame (a jump past [`TELEPORT`] snaps every fade).
    eye: Vec3,
    /// The buffer holds `values` (false after frames the option was off
    /// and only the statistics moved them).
    synced: bool,
    /// The groups the fades walk ([`FadeGroups`]), for the LOD and depth
    /// direction they were listed for.
    groups: Option<((u8, bool), FadeGroups)>,
    /// World Detail Ultra / Extreme — the sections (by number) whose
    /// swap began at this frame time, cross-faded over
    /// [`crate::render::RenderOptions::SECTION_FADE_SECONDS`] (main and far sets).
    swaps: HashMap<u32, f32>,
    /// The last world tick the fades stepped
    /// (`render::enh::tick_views`) and what that step handed the draw.
    gate: crate::render::enh::tick_views::TickGate,
    #[allow(clippy::type_complexity)]
    last_out: Option<(std::sync::Arc<Vec<f32>>, Vec<bool>, std::sync::Arc<Vec<bool>>, Option<Vec<bool>>)>,
}

/// What [`fade_slots`] sizes a frame's slots by
/// ([`WorldRenderer::update_fades`]).
pub(in crate::render) struct FadeCtx<'a> {
    pub(in crate::render) eye: Vec3,
    pub(in crate::render) scale: f32,
    pub(in crate::render) keep: f32,
    pub(in crate::render) need: f32,
    pub(in crate::render) frame: u32,
    pub(in crate::render) step: f32,
    pub(in crate::render) on: bool,
    pub(in crate::render) jumped: bool,
    pub(in crate::render) stats: bool,
    pub(in crate::render) in_view: &'a dyn Fn(Vec3, f32) -> bool,
    /// The fade's length (s), for the pop meter.
    pub(in crate::render) fade_seconds: f32,
    /// The frame's settings (the pop meter's output width).
    pub(in crate::render) opts: &'a RenderOptions,
}

/// [`WorldRenderer::update_fades`] for one group's instance slots
/// `slots`: each slot sized and its fade moved, the slot's state, the
/// group's part-way flag, the changed values and (with the statistics) the
/// pops (appeared, vanished, their sizes summed in slot order) updated.
/// The per-slot arithmetic as before, over slices.
#[allow(clippy::too_many_arguments)]
#[inline]
pub(in crate::render) fn fade_slots(
    cull: &[[f32; 4]],
    stamp: &mut [u32],
    on_: &mut [bool],
    values: &mut [f32],
    slots: std::ops::Range<u32>,
    ctx: &FadeCtx,
    is_leaving: bool,
    any_left: &mut bool,
    mid: &mut bool,
    changed: &mut Vec<(u32, f32)>,
    pops: &mut (u32, u32, f32),
) {
    let (lo, hi) = (slots.start as usize, slots.end as usize);
    let FadeCtx { eye, scale, keep, need, frame, step, on, jumped, stats, in_view, .. } = *ctx;
    for ((((slot, c), stamp), on_k), value) in
        slots.zip(&cull[lo..hi]).zip(&mut stamp[lo..hi]).zip(&mut on_[lo..hi]).zip(&mut values[lo..hi])
    {
        let r = c[3];
        if r < 0.0 {
            continue;
        }
        let p = Vec3::new(c[0], c[1], c[2]);
        // Section geometry (radius 0) has no size cull; scenery is sized
        // as 0x5D64E0 without its behind test.
        let size = if r == 0.0 {
            f32::MAX
        } else {
            let gap = p.distance(eye) - r;
            let s = if gap <= r { scale } else { scale / gap * r };
            // Below one unit (the pixel rule) the size is not
            // truncated (ftol would make every need under 1 a need of 1).
            if need < 1.0 { s } else { trunc_fast(s) }
        };
        let seen = *stamp + 1 == frame;
        let target_on = !is_leaving && if seen && *on_k { size >= keep } else { size >= need };
        // Pop meter: the cull's decision flipped for an instance in the
        // view (statistics only).
        if stats && seen && !is_leaving && r > 0.0 && target_on != *on_k && in_view(p, r) {
            if pop_meter::verbose() {
                eprintln!(
                    "pop scenery frame {frame} slot {slot} at {p:.0} r {r:.1} d {:.0} size {size:.2} need {need:.2} keep {keep:.2} -> {target_on} was {:.2} fade {} leaving {is_leaving}",
                    p.distance(eye),
                    *value,
                    ctx.fade_seconds
                );
            }
            pop_meter::event(pop_meter::Source::Scenery, pop_meter::px(size, ctx.opts), if on { ctx.fade_seconds } else { 0.0 });
        }
        let target = if target_on { 1.0 } else { 0.0 };
        // Not drawn the frame before: a whole section coming in. At once
        // natively (and on a jump); with the option it fades in from
        // nothing.
        let was = if seen {
            value.abs()
        } else if on && !jumped {
            0.0
        } else {
            target
        };
        let v = if target > was {
            (was + step).min(1.0)
        } else if target < was {
            (was - step).max(0.0)
        } else {
            was
        };
        *on_k = target_on;
        *stamp = frame;
        *any_left |= v > 0.0;
        // A jump of half or more in one frame of an instance in the view:
        // a pop (whole sections are counted as blocks).
        if stats && seen && r > 0.0 && (v - was).abs() >= 0.5 && in_view(p, r) {
            if v > was {
                pops.0 += 1;
            } else {
                pops.1 += 1;
            }
            pops.2 += size;
        }
        // Fading out: the other half of the screen-door pattern
        // (negative), so a section going out and the one taking its place
        // share the pixels between them.
        let stored = if target_on || v == 0.0 { v } else { -v };
        if v > 0.0 && v < 1.0 {
            *mid = true;
        }
        if stored != *value {
            *value = stored;
            changed.push((slot, stored));
        }
    }
}

/// Per section (index into `WorldCore::sections`), the groups
/// [`WorldRenderer::update_fades`] sizes, in the section's group order
/// (the same groups the per-frame tests kept, listed once instead of
/// tested every frame; they depend only on the group's fixed key, the main
/// LOD and the depth direction): `main` the main view's at the main LOD,
/// `far` World Detail Extreme's at `LOW_LOD`, each flagged when the main
/// LOD has it too (left out while the main list draws its section).
/// Entries: group, first and end instance slot.
#[derive(Default)]
struct FadeGroups {
    main: Vec<Vec<(u32, u32, u32)>>,
    far: Vec<Vec<(u32, u32, u32, bool)>>,
}

impl FadeGroups {
    fn new(core: &WorldCore, main_lod: u8, reversed: bool) -> FadeGroups {
        let mut out = FadeGroups::default();
        for groups in &core.section_groups {
            let mut main = Vec::new();
            let mut far = Vec::new();
            for &g in groups {
                let (is_sky, flags, lods, ref instances) = core.group_key[g as usize];
                if is_sky || WorldView::Main.culls(flags, reversed) {
                    continue;
                }
                if lods & main_lod != 0 {
                    main.push((g, instances.start, instances.end));
                }
                if lods & LOW_LOD != 0 {
                    far.push((g, instances.start, instances.end, lods & main_lod != 0));
                }
            }
            out.main.push(main);
            out.far.push(far);
        }
        out
    }
}

/// LOD Fade: an eye move past this many metres in a frame (a teleport, a
/// new event's start) shows everything at once.
const TELEPORT: f32 = 30.0;

impl WorldRenderer {
    /// LOD Fade's main-view pipelines at the scene's sample count, built
    /// on first use (a few dozen pipelines, from the disk cache after the
    /// first run).
    fn ensure_fade_pipelines(&self, gpu: &Gpu) {
        let scene = self.scene_pipelines();
        if scene.fade.is_some() {
            return;
        }
        let fade = [false, true].map(|d| {
            gpu.world_shared().fade_pipelines(
                &gpu.device,
                self.arrays,
                scene.mode,
                self.scene_format,
                &PipelineKey::all(),
                d,
            )
        });
        // The pre-pass draws' EQUAL pipelines with the fades too.
        let depth = scene.depth.as_ref().map(|d| {
            let [e0, ..] = &d.equal;
            let [e1, e2] = [1, 2].map(|f| {
                gpu.world_shared().equal_pipelines(&gpu.device, self.arrays, scene.mode, self.scene_format, f)
            });
            let [d0, ..] = &d.equal_dry;
            let [d1, d2] = [1, 2].map(|f| {
                DepthPrepass::dry_for(gpu.world_shared(), &gpu.device, self.arrays, scene.mode, self.scene_format, f)
            });
            std::sync::Arc::new(DepthPrepass {
                depth: d.depth.clone(),
                equal: [e0.clone(), e1, e2],
                equal_dry: [d0.clone(), d1, d2],
            })
        });
        let built = ScenePipelines {
            mode: scene.mode,
            pipelines: scene.pipelines.clone(),
            glow: scene.glow.clone(),
            glow_streak: scene.glow_streak.clone(),
            fade: Some(fade),
            car_shadow: scene.car_shadow.clone(),
            depth,
            characters: scene.characters.clone(),
            drops: scene.drops.clone(),
        };
        *self.scene.write().unwrap() = std::sync::Arc::new(built);
    }

    /// LOD Fade (clone deviation): this frame's per-instance fades
    /// ([`FadeState`]), uploaded where they changed, and the sections the
    /// main view draws (the frame's, and those still fading out); None
    /// with the option off (the native path: nothing here runs unless the
    /// pop statistics are kept, and then nothing is uploaded or drawn
    /// differently).
    ///
    /// docs/original/uncapped-fps.md, "Enhancement effects on ticks": with
    /// the game's ticks published (`render::enh::tick_views`) the fades
    /// step once per world tick, each with that tick's camera, sections and
    /// time, and a frame between two ticks draws what the last step left;
    /// the fades at tick n are then the same at every display rate.
    /// Without them, one step per draw with the draw's inputs (as before).
    #[allow(clippy::type_complexity)]
    pub(in crate::render) fn update_fades(
        &mut self,
        gpu: &Gpu,
        params: &FrameParams,
        visible: &[bool],
        far: Option<&[bool]>,
        view_proj: Mat4,
        scale: f32,
    ) -> Option<(std::sync::Arc<Vec<f32>>, Vec<bool>, std::sync::Arc<Vec<bool>>, Option<Vec<bool>>)> {
        use crate::render::enh::tick_views::Due;
        if !params.opts.lod_fade && !pop_stats_enabled() {
            return None;
        }
        let frame = crate::render::enh::tick_views::frame();
        let due = match self.fade.as_mut() {
            Some(f) => f.gate.due(frame.as_deref()),
            None => Due::Draw,
        };
        let views = match due {
            // The tick drawn at α = 1: the frame's own inputs.
            Due::Ticks(v) if v.len() == 1 && v[0].is_frame(params.camera, params.visible_sections, params.far_sections) => {
                // The tick's own LOD Fade and pop-in fixes (its step).
                let opts = RenderOptions { lod_fade: v[0].opts.lod_fade, pop_free: v[0].opts.pop_free, ..*params.opts };
                let p = FrameParams { time: v[0].time, opts: &opts, ..*params };
                let out = self.fades_step(gpu, &p, visible, far, view_proj, scale);
                if let Some(f) = self.fade.as_mut() {
                    f.last_out = out.clone();
                }
                return out;
            }
            Due::Draw => {
                let out = self.fades_step(gpu, params, visible, far, view_proj, scale);
                if let Some(f) = self.fade.as_mut() {
                    // The first step (the state made): the published
                    // ticks count as stepped.
                    f.gate.due(frame.as_deref());
                    f.last_out = out.clone();
                }
                return out;
            }
            Due::Ticks(v) => v,
        };
        if views.is_empty() {
            return self.fade.as_ref().and_then(|f| f.last_out.clone()).filter(|_| params.opts.lod_fade);
        }
        let mut out = None;
        for v in &views {
            // The tick's camera (the frame's jitter on it: a tick drawn at
            // α = 1 is the frame's camera bit for bit).
            let camera = Camera { jitter: params.camera.jitter, ..v.camera };
            // The tick's own LOD Fade and pop-in fixes (its step).
            let opts = RenderOptions { lod_fade: v.opts.lod_fade, pop_free: v.opts.pop_free, ..*params.opts };
            let p = FrameParams {
                camera: &camera,
                opts: &opts,
                time: v.time,
                visible_sections: v.sections.as_deref(),
                far_sections: v.far.as_deref(),
                ..*params
            };
            let vis = crate::render::native::world::plan::visible_of(p.visible_sections);
            let far = crate::render::native::world::plan::far_sections(&p, &vis);
            let vp = camera.view_projection(p.aspect);
            out = self.fades_step(gpu, &p, &vis, far.as_deref(), vp, scale);
        }
        if let Some(f) = self.fade.as_mut() {
            f.last_out = out.clone();
        }
        out
    }

    /// One step of LOD Fade's per-instance fades ([`WorldRenderer::update_fades`]).
    #[allow(clippy::type_complexity)]
    fn fades_step(
        &mut self,
        gpu: &Gpu,
        params: &FrameParams,
        visible: &[bool],
        far: Option<&[bool]>,
        view_proj: Mat4,
        scale: f32,
    ) -> Option<(std::sync::Arc<Vec<f32>>, Vec<bool>, std::sync::Arc<Vec<bool>>, Option<Vec<bool>>)> {
        let on = params.opts.lod_fade;
        let stats = pop_stats_enabled();
        if !on && !stats {
            return None;
        }
        let _t = crate::prof::acc::span("world: LOD fade");
        if on {
            self.ensure_fade_pipelines(gpu);
        }
        let slots = self.instance_cull.read().unwrap().len();
        let eye = params.camera.position;
        let fade = self.fade.get_or_insert_with(|| {
            let buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("world instance fades"),
                contents: bytemuck::cast_slice(&vec![1.0f32; slots.max(1)]),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
            FadeState {
                buffer,
                values: std::sync::Arc::new(vec![1.0; slots]),
                on: vec![true; slots],
                stamp: vec![0; slots],
                frame: 1,
                time: params.time,
                sections: Vec::new(),
                lingering: Vec::new(),
                far_sections: Vec::new(),
                far_lingering: Vec::new(),
                eye,
                synced: true,
                groups: None,
                swaps: HashMap::new(),
                gate: Default::default(),
                last_out: None,
            }
        });
        fade.frame += 1;
        let frame = fade.frame;
        let dt = (params.time - fade.time).clamp(0.0, 0.1);
        fade.time = params.time;
        // A first frame or a jump of the eye (a teleport, a new event):
        // everything takes its state at once.
        let jumped = frame == 2 || fade.eye.distance(eye) > TELEPORT;
        fade.eye = eye;
        // Native when off (statistics only): drawn exactly at the size
        // need, at once, sections at once.
        let step = if on { dt / crate::render::RenderOptions::LOD_FADE_SECONDS } else { 1.0 };
        let need = main_min_size(params.opts);
        let keep = if on { need * crate::render::RenderOptions::LOD_FADE_KEEP } else { need };
        let planes = side_planes(view_proj);
        let forward = params.camera.forward();
        let in_view = |c: Vec3, r: f32| !outside(&planes, c, r) && (c - eye).dot(forward) > -r;
        // The sections drawn: this frame's, and with the option those
        // drawn before that are still fading out.
        let mut drawn = visible.to_vec();
        let before = std::mem::take(&mut fade.sections);
        let leaving: Vec<u32> = if on && !jumped {
            let mut l: Vec<u32> =
                fade.lingering.iter().copied().filter(|&s| !visible.get(s as usize).copied().unwrap_or(true)).collect();
            l.extend(
                before
                    .iter()
                    .enumerate()
                    .filter(|&(s, &was)| was && !visible.get(s).copied().unwrap_or(true))
                    .map(|(s, _)| s as u32),
            );
            l.sort_unstable();
            l.dedup();
            l
        } else {
            Vec::new()
        };
        for &s in &leaving {
            if let Some(v) = drawn.get_mut(s as usize) {
                *v = true;
            }
        }
        // World Detail Extreme's far sections (their lowest LOD): the same
        // bookkeeping, so a far section handed to the main list (or
        // dropped) cross-fades instead of popping.
        let far_now: Vec<bool> = far.map_or_else(|| vec![false; visible.len()], <[bool]>::to_vec);
        let far_before = std::mem::take(&mut fade.far_sections);
        let mut far_drawn = far_now.clone();
        if on && !jumped {
            let gone = |s: usize| !far_now.get(s).copied().unwrap_or(false);
            for s in fade
                .far_lingering
                .iter()
                .map(|&s| s as usize)
                .chain(far_before.iter().enumerate().filter(|(_, w)| **w).map(|(s, _)| s))
            {
                if gone(s)
                    && let Some(v) = far_drawn.get_mut(s)
                {
                    *v = true;
                }
            }
        }
        let mut still_far_leaving = Vec::new();
        let core = &self.core;
        let key = (self.main_lod, params.reversed);
        if fade.groups.as_ref().is_none_or(|(k, _)| *k != key) {
            fade.groups = Some((key, FadeGroups::new(core, self.main_lod, params.reversed)));
        }
        let lists = &fade.groups.as_ref().unwrap().1;
        let cull = core.instance_cull.read().unwrap();
        let values = std::sync::Arc::make_mut(&mut fade.values);
        let mut changed: Vec<(u32, f32)> = Vec::new();
        let (mut appeared, mut vanished, mut size_sum, mut blocks) = (0u32, 0u32, 0.0f32, 0u32);
        let mut still_leaving = Vec::new();
        let mut block_dist = Vec::new();
        // Per group: an instance part-way in or out (drawn with the
        // dithering pipelines; the others keep early depth).
        let mut mid = vec![false; core.group_key.len()];
        // How far a section's nearest group in the view is (None: none in
        // the view), for the statistics' blocks.
        let block_distance = |i: usize| -> Option<f32> {
            core.section_groups[i]
                .iter()
                .filter(|&&g| !core.group_key[g as usize].0)
                .filter_map(|&g| {
                    let (c, r) = core.group_bounds[g as usize];
                    in_view(c, r).then(|| (c.distance(eye) - r).max(0.0))
                })
                .reduce(f32::min)
        };
        // (section index, LOD drawn, far) for the main sections, then the
        // far ones.
        let work: Vec<(usize, u8, bool)> = (0..core.sections.len())
            .map(|i| (i, self.main_lod, false))
            .chain(
                (0..core.sections.len())
                    .filter(|&i| far_drawn.get(core.sections[i].0 as usize).copied().unwrap_or(false))
                    .map(|i| (i, LOW_LOD, true)),
            )
            .collect();
        // Pop meter: a section's biggest group in the view (output
        // pixels of radius; None: none in the view).
        let block_px = |i: usize| -> Option<f32> {
            core.section_groups[i]
                .iter()
                .filter(|&&g| !core.group_key[g as usize].0)
                .filter_map(|&g| {
                    let (c, r) = core.group_bounds[g as usize];
                    in_view(c, r).then(|| {
                        let gap = c.distance(eye) - r;
                        pop_meter::px(if gap <= r { scale } else { scale / gap * r }, params.opts)
                    })
                })
                .reduce(f32::max)
        };
        // The swaps that begin this frame (a section joining or leaving
        // the main or far set) take the slow cross-fade.
        let slow_swaps = on && params.opts.pop_free && !jumped;
        if slow_swaps {
            let now = params.time;
            fade.swaps.retain(|_, t| now - *t < crate::render::RenderOptions::SECTION_FADE_SECONDS);
            for i in 0..core.sections.len() {
                let n = core.sections[i].0;
                let sec = n as usize;
                let now_v = visible.get(sec).copied().unwrap_or(false);
                let was_v = before.get(sec).copied().unwrap_or(false) && !fade.lingering.contains(&n);
                let now_f = far_now.get(sec).copied().unwrap_or(false);
                let was_f = far_before.get(sec).copied().unwrap_or(false) && !fade.far_lingering.contains(&n);
                if now_v != was_v || now_f != was_f {
                    fade.swaps.insert(n, now);
                }
            }
            // Still fading out of one set: the swap goes on (and a group
            // handed from the main set to the far one, or back, when the
            // fade-out ends, takes the slow fade too).
            for &n in fade.lingering.iter().chain(&fade.far_lingering) {
                fade.swaps.insert(n, now);
            }
        } else {
            fade.swaps.clear();
        }
        let slow_step = dt / crate::render::RenderOptions::SECTION_FADE_SECONDS;
        if stats && !jumped {
            let secs = if on {
                if slow_swaps { crate::render::RenderOptions::SECTION_FADE_SECONDS } else { crate::render::RenderOptions::LOD_FADE_SECONDS }
            } else {
                0.0
            };
            for i in 0..core.sections.len() {
                let sec = core.sections[i].0 as usize;
                let now_v = visible.get(sec).copied().unwrap_or(false);
                let was_v = before.get(sec).copied().unwrap_or(false) && !fade.lingering.contains(&core.sections[i].0);
                let now_f = far_now.get(sec).copied().unwrap_or(false);
                let was_f =
                    far_before.get(sec).copied().unwrap_or(false) && !fade.far_lingering.contains(&core.sections[i].0);
                if (now_v != was_v || (now_f != was_f && !now_v && !was_v))
                    && let Some(px) = block_px(i)
                {
                    pop_meter::event(pop_meter::Source::Section, px, secs);
                }
            }
        }
        let ctx = FadeCtx {
            eye,
            scale,
            keep,
            need,
            frame,
            step,
            on,
            jumped,
            stats,
            in_view: &in_view,
            fade_seconds: crate::render::RenderOptions::LOD_FADE_SECONDS,
            opts: params.opts,
        };
        let slow_ctx = FadeCtx { step: slow_step, fade_seconds: crate::render::RenderOptions::SECTION_FADE_SECONDS, ..ctx };
        let mut pops = (0u32, 0u32, 0.0f32);
        for (i, lod, is_far) in work {
            let section = &core.sections[i].0;
            let sec = *section as usize;
            if is_far {
                let is_leaving = !far_now.get(sec).copied().unwrap_or(false);
                let shared = drawn.get(sec).copied().unwrap_or(false);
                let mut any_left = false;
                debug_assert_eq!(lod, LOW_LOD);
                let c = if fade.swaps.contains_key(section) { &slow_ctx } else { &ctx };
                if pop_meter::verbose() {
                    eprintln!(
                        "pop far section {section} leaving {is_leaving} shared {shared} slow {}",
                        fade.swaps.contains_key(section)
                    );
                }
                for &(g, first, end, also_main) in &lists.far[i] {
                    if shared && also_main {
                        continue;
                    }
                    fade_slots(
                        &cull,
                        &mut fade.stamp,
                        &mut fade.on,
                        values,
                        first..end,
                        c,
                        is_leaving,
                        &mut any_left,
                        &mut mid[g as usize],
                        &mut changed,
                        &mut pops,
                    );
                }
                if is_leaving && any_left {
                    still_far_leaving.push(*section);
                }
                continue;
            }
            if !drawn.get(sec).copied().unwrap_or(true) {
                // A section gone from the drawn set at once, with a group
                // in the view: a block vanishing (statistics only).
                if stats
                    && !on
                    && before.get(sec).copied().unwrap_or(false)
                    && !jumped
                    && let Some(d) = block_distance(i)
                {
                    blocks += 1;
                    block_dist.push(d);
                }
                continue;
            }
            let is_leaving = !visible.get(sec).copied().unwrap_or(true);
            let new_section = !before.get(sec).copied().unwrap_or(false);
            // A section new to the drawn set, with a group in the view,
            // shown at once: a block appearing (statistics only).
            if stats
                && !on
                && new_section
                && !jumped
                && let Some(d) = block_distance(i)
            {
                blocks += 1;
                block_dist.push(d);
            }
            let mut any_left = false;
            let c = if fade.swaps.contains_key(section) { &slow_ctx } else { &ctx };
            for &(g, first, end) in &lists.main[i] {
                fade_slots(
                    &cull,
                    &mut fade.stamp,
                    &mut fade.on,
                    values,
                    first..end,
                    c,
                    is_leaving,
                    &mut any_left,
                    &mut mid[g as usize],
                    &mut changed,
                    &mut pops,
                );
            }
            if is_leaving && any_left {
                still_leaving.push(*section);
            }
        }
        (appeared, vanished, size_sum) = (appeared + pops.0, vanished + pops.1, size_sum + pops.2);
        drop(cull);
        let mut now_drawn = visible.to_vec();
        for &s in &still_leaving {
            if let Some(v) = now_drawn.get_mut(s as usize) {
                *v = true;
            }
        }
        fade.sections = now_drawn.clone();
        fade.lingering = still_leaving;
        let mut far_drawn_now = far_now.clone();
        for &s in &still_far_leaving {
            if let Some(v) = far_drawn_now.get_mut(s as usize) {
                *v = true;
            }
        }
        fade.far_sections = far_drawn_now.clone();
        fade.far_lingering = still_far_leaving;
        if stats {
            pop_meter::end_frame(params.opts);
            let cars = CAR_POPS.swap(0, std::sync::atomic::Ordering::Relaxed);
            let mut all = POP_STATS.lock().unwrap();
            let st = all.get_or_insert_with(PopStats::default);
            st.frames += 1;
            let pops = appeared + vanished;
            if pops + blocks + cars > 0 {
                st.pop_frames += 1;
            }
            st.appeared += u64::from(appeared);
            st.vanished += u64::from(vanished);
            st.sections += u64::from(blocks);
            st.car_pops += u64::from(cars);
            st.popped_size += f64::from(size_sum);
            st.worst = st.worst.max(pops);
            st.per_frame.push((appeared, vanished, blocks, size_sum, cars));
            st.block_distances.extend(block_dist);
        }
        if !on {
            fade.synced = false;
            return None;
        }
        if !fade.synced {
            gpu.queue.write_buffer(&fade.buffer, 0, bytemuck::cast_slice(&fade.values[..]));
            fade.synced = true;
            changed.clear();
        }
        // The changed slots, a copy per run of adjacent ones.
        changed.sort_unstable_by_key(|c| c.0);
        let mut i = 0;
        while i < changed.len() {
            let mut j = i + 1;
            while j < changed.len() && changed[j].0 == changed[j - 1].0 + 1 {
                j += 1;
            }
            let run: Vec<f32> = changed[i..j].iter().map(|c| c.1).collect();
            gpu.queue.write_buffer(&fade.buffer, u64::from(changed[i].0) * 4, bytemuck::cast_slice(&run));
            i = j;
        }
        Some((
            fade.values.clone(),
            now_drawn,
            std::sync::Arc::new(mid),
            far_drawn_now.iter().any(|&f| f).then_some(far_drawn_now),
        ))
    }

    /// LOD Fade: a placement's fade in the main view (its drawn slots' most;
    /// None when no slot of it was sized this frame or the option is off):
    /// the scenery lamps' flares follow it.
    pub fn placement_fade(&self, placement: usize, opts: &RenderOptions) -> Option<f32> {
        let fade = self.fade.as_ref().filter(|_| opts.lod_fade)?;
        self.placement_slots
            .get(placement)?
            .iter()
            .filter(|&&s| fade.stamp.get(s as usize).is_some_and(|&t| t == fade.frame))
            .map(|&s| fade.values[s as usize].abs())
            .reduce(f32::max)
    }
}

#[cfg(test)]
mod fade_slot_tests {
    use crate::render::*;

    /// The per-slot body `update_fades` had before it was parallelised, verbatim but for the
    /// names: the reference [`fade_slots`] must match bit for bit.
    #[allow(clippy::too_many_arguments)]
    fn reference(cull: &[[f32; 4]], stamp: &mut [u32], on_: &mut [bool], values: &mut [f32], slots: std::ops::Range<u32>, c: &FadeCtx, is_leaving: bool, any_left: &mut bool, mid: &mut bool, changed: &mut Vec<(u32, f32)>, pops: &mut (u32, u32, f32)) {
        let FadeCtx { eye, scale, keep, need, frame, step, on, jumped, stats, in_view, .. } = *c;
        for slot in slots {
            let k = slot as usize;
            let cc = cull[k];
            let r = cc[3];
            if r < 0.0 {
                continue;
            }
            let p = Vec3::new(cc[0], cc[1], cc[2]);
            let size = if r == 0.0 { f32::MAX } else {
                let gap = p.distance(eye) - r;
                trunc_fast(if gap <= r { scale } else { scale / gap * r })
            };
            let seen = stamp[k] + 1 == frame;
            let target_on = !is_leaving && if seen && on_[k] { size >= keep } else { size >= need };
            let target = if target_on { 1.0 } else { 0.0 };
            let was = if seen { values[k].abs() } else if on && !jumped { 0.0 } else { target };
            let v = if target > was { (was + step).min(1.0) } else if target < was { (was - step).max(0.0) } else { was };
            on_[k] = target_on;
            stamp[k] = frame;
            *any_left |= v > 0.0;
            if stats && seen && r > 0.0 && (v - was).abs() >= 0.5 && in_view(p, r) {
                if v > was { pops.0 += 1; } else { pops.1 += 1; }
                pops.2 += size;
            }
            let stored = if target_on || v == 0.0 { v } else { -v };
            if v > 0.0 && v < 1.0 {
                *mid = true;
            }
            if stored != values[k] {
                values[k] = stored;
                changed.push((slot, stored));
            }
        }
    }

    /// The slice form gives the same states, values, flags, changed
    /// list and statistics as the old per-slot loop, frame after frame,
    /// for slots in and out of range, fading in and out, with and without
    /// the option, jumps and the statistics.
    #[test]
    fn fade_slots_match_the_old_loop() {
        let mut seed = 0x2545_F491u32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        let n = 600;
        let cull: Vec<[f32; 4]> = (0..n).map(|i| {
            let r = match i % 7 { 0 => 0.0, 1 => -1.0, _ => rnd() * 6.0 };
            [rnd() * 800.0 - 400.0, rnd() * 800.0 - 400.0, rnd() * 20.0, r]
        }).collect();
        let in_view = |p: Vec3, _r: f32| p.x > 0.0;
        let mut a = (vec![0u32; n], vec![true; n], vec![1.0f32; n]);
        let mut b = a.clone();
        let mut eye = Vec3::ZERO;
        for frame in 2..80u32 {
            eye += Vec3::new(rnd() * 12.0 - 4.0, rnd() * 12.0 - 4.0, 0.0);
            let on = frame % 23 != 0;
            let ctx = FadeCtx { eye, scale: 900.0, keep: 17.0 * 0.8, need: 17.0, frame, step: rnd() * 0.3, on, jumped: frame % 31 == 0, stats: frame % 3 == 0, in_view: &in_view, fade_seconds: 0.25, opts: &RenderOptions::default() };
            // Groups of varied sizes; some skipped some frames (not seen).
            let mut lo = 0u32;
            while (lo as usize) < n {
                let hi = (lo + 1 + (rnd() * 40.0) as u32).min(n as u32);
                if rnd() > 0.2 {
                    let leaving = rnd() > 0.85;
                    let (mut la, mut lb) = ((false, false, Vec::new(), (0, 0, 0.0f32)), (false, false, Vec::new(), (0, 0, 0.0f32)));
                    fade_slots(&cull, &mut a.0, &mut a.1, &mut a.2, lo..hi, &ctx, leaving, &mut la.0, &mut la.1, &mut la.2, &mut la.3);
                    reference(&cull, &mut b.0, &mut b.1, &mut b.2, lo..hi, &ctx, leaving, &mut lb.0, &mut lb.1, &mut lb.2, &mut lb.3);
                    assert_eq!((la.0, la.1, la.3.0, la.3.1, la.3.2.to_bits()), (lb.0, lb.1, lb.3.0, lb.3.1, lb.3.2.to_bits()));
                    assert_eq!(la.2.iter().map(|c| (c.0, c.1.to_bits())).collect::<Vec<_>>(), lb.2.iter().map(|c| (c.0, c.1.to_bits())).collect::<Vec<_>>());
                }
                lo = hi;
            }
            assert_eq!((&a.0, &a.1), (&b.0, &b.1));
            assert_eq!(a.2.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), b.2.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
        }
        // Something moved: not a vacuous comparison.
        assert!(a.2.iter().any(|&v| v != 1.0) && a.2.iter().any(|&v| v < 0.0));
    }
}

impl FadeState {
    /// The fades' state as a digest (the rate tests compare it
    /// at equal ticks).
    pub(in crate::render) fn digest(&self, h: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        self.values.iter().map(|v| v.to_bits()).collect::<Vec<_>>().hash(h);
        self.on.hash(h);
        self.lingering.hash(h);
        self.far_lingering.hash(h);
        self.sections.hash(h);
        let mut swaps: Vec<(u32, u32)> = self.swaps.iter().map(|(k, v)| (*k, v.to_bits())).collect();
        swaps.sort_unstable();
        swaps.hash(h);
    }
}
