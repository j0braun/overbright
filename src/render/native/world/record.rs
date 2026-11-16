//! World view recording: draw passes, kept groups, the frame entry points (moved from render/mod.rs).

use crate::render::*;

impl WorldCore {
    /// The main view's group 0 as the scene's other draws (shadows,
    /// particles, glows, characters) bind it: with the lamps' buffers while
    /// they are on (its frame values then have them), else the native one.
    pub(crate) fn main_group(&self) -> wgpu::BindGroup {
        self.lamp_main.read().unwrap().clone().unwrap_or_else(|| self.main_frame.1.clone())
    }

    /// The main view's group 0 for the scene's pipelines
    /// ([`WorldCore::scene_pipelines`]): with Modern + RT's traced scene
    /// (binding 11) while they are made with that layout, else
    /// [`WorldCore::main_group`]. Pipelines of any other layout (the SSR
    /// mask, the cover shadow, the native ones) take `main_group`.
    pub(crate) fn scene_group(&self) -> wgpu::BindGroup {
        if self.scene_pipelines().mode.modern == 2
            && let Some(g) = self.lamp_main_rt.read().unwrap().clone()
        {
            return g;
        }
        self.main_group()
    }

    /// The scene's pipelines as they are now.
    pub(crate) fn scene_pipelines(&self) -> std::sync::Arc<ScenePipelines> {
        self.scene.read().unwrap().clone()
    }

    /// Record a frame's world views ([`WorldRenderer::plan_views`]).
    fn record_views(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        plan: &ViewsPlan,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        reflected: &mut dyn FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView, &wgpu::TextureView),
    ) {
        // The sky panorama's rows before the first view that samples it.
        if let Some(n) = self.sky() {
            n.update(encoder);
        }
        // View 3 first (0x5CAE4B, after the cube faces and before view 4):
        // 0x5BA3D0(record 3, 1) clears it to opaque black, then the world
        // (0x5D6AE0(view 3, 1)); no SKYDOME (0x60F890 is called only for
        // the cube faces and the main view, 0x5CAD50 / 0x5CB237), and the
        // target being off the back buffer turns every non-car effect into
        // worldnofog (0x5C5940).
        if let Some((cull, target_color, target_depth)) = &plan.rear {
            let order = self.group_order(&self.kept_groups(
                &plan.visible,
                cull,
                WorldView::RearView,
                plan.reversed,
                false,
                None,
                LOW_LOD,
            ));
            self.draw_pass(
                encoder,
                target_color,
                target_depth,
                wgpu::Color::BLACK,
                &self.rear_frame.1,
                &self.no_reflection,
                &order,
                plan.time,
                None,
                WorldView::RearView,
                (0, 0),
            );
            crate::prof::gpu::mark(encoder, "world: rear view");
        }
        if let Some((cull, target_color, target_depth)) = &plan.reflection {
            let order = self.group_order(&self.kept_groups(
                &plan.visible,
                cull,
                WorldView::RoadReflection,
                plan.reversed,
                false,
                None,
                plan.reflection_lod,
            ));
            let view = if cull.mirrored { WorldView::RoadReflection } else { WorldView::PlanarRoadReflection };
            self.draw_pass(
                encoder,
                target_color,
                target_depth,
                wgpu::Color::BLACK,
                &self.reflection_frame.1,
                &self.no_reflection,
                &order,
                plan.time,
                None,
                view,
                (plan.pool_count, plan.reflection_glow_count),
            );
            reflected(encoder, target_color, target_depth);
            crate::prof::gpu::mark(encoder, "world: road reflection");
        }
        self.record_lamp_shadows(encoder, &plan.lamp_shadows, &plan.visible, plan.reversed, plan.time, plan.main_lod);
        let main_visible = plan.main_visible.as_deref().unwrap_or(&plan.visible);
        let mut kept =
            self.kept_groups(main_visible, &plan.main, WorldView::Main, plan.reversed, true, None, plan.main_lod);
        if let Some(far) = &plan.far {
            kept.extend(self.kept_far_groups(far, main_visible, &plan.main, plan.reversed, plan.main_lod));
        }
        let mut order = self.group_order(&kept);
        // The animation mask's draws, the water's among them (drawn
        // by `render::water` with Animated Water).
        let animated: Vec<(u32, AnimMotion)> = match &plan.anim_mask {
            Some(m) => order
                .iter()
                .filter_map(|&i| self.anim_motion(i, m.ripples, plan.time, m.last_time).map(|k| (i, k)))
                .collect(),
            None => Vec::new(),
        };
        if plan.hide_water {
            // Animated Water (docs/water.md): `render::water` draws them.
            order.retain(|&i| !self.draws[i as usize].water);
        }
        let from_main = self.draw_pass_faded(
            encoder,
            color,
            depth,
            plan.clear,
            plan.lamp_frame.as_ref().unwrap_or(&self.main_frame.1),
            &plan.reflection_group,
            &order,
            plan.time,
            Some(plan.sky),
            WorldView::Main,
            (plan.pool_count, 0),
            None,
            plan.fade.as_ref().map(|(b, m)| (b, &m[..])),
            plan.rt_shadow.as_ref(),
            plan.wet_mask.as_ref(),
            plan.dry_road,
        );
        crate::prof::gpu::mark(encoder, "world: main view");
        if let Some(m) = &plan.wet_mask {
            self.draw_wet_mask(encoder, m, &plan.reflection_group, &order, plan.time, from_main, depth);
            crate::prof::gpu::mark(encoder, "graphics: ssr mask");
        }
        if let Some(m) = &plan.anim_mask {
            self.draw_anim_mask(encoder, m, &animated, plan.time);
            crate::prof::gpu::mark(encoder, "graphics: fsr2 animation mask");
        }
    }

    /// How draw `i`'s texture moves this frame (FSR 2's animation
    /// mask), None for a still texture: a flip-book or a scroll of kind 1
    /// or 2 ([`scroll_motion`]), a water texture, or with `ripples` the wet
    /// road.
    fn anim_motion(&self, i: u32, ripples: bool, time: f32, last: Option<f32>) -> Option<AnimMotion> {
        let d = &self.draws[i as usize];
        match d.texture {
            TextureRef::Animated(a) => {
                let fps = self.animations[a].fps;
                Some(if last.is_none_or(|l| flip_stepped(fps, time, l)) {
                    AnimMotion::Stepped
                } else {
                    AnimMotion::Still
                })
            }
            TextureRef::Fixed(t) => {
                self.texture_scrolls.get(t).and_then(|s| scroll_motion(s, time, last)).or_else(|| {
                    if d.water {
                        Some(AnimMotion::Continuous)
                    } else if ripples && d.key.effect == Effect::Reflect {
                        Some(AnimMotion::Ripples)
                    } else {
                        None
                    }
                })
            }
            TextureRef::Sky(_) => None,
        }
    }

    /// FSR 2's animation mask ([`AnimMask`]): `draws` (the main
    /// view's, in its order, with their motion) again, with the main
    /// view's vertex shaders, each scaled to its values ([`anim_values`],
    /// the blend constant).
    fn draw_anim_mask(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        m: &AnimMaskPlan,
        draws: &[(u32, AnimMotion)],
        time: f32,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("fsr2 animation mask"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &m.color,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &m.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(m.clear_depth),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if draws.is_empty() {
            return;
        }
        pass.set_bind_group(0, &self.main_group(), &[]);
        pass.set_bind_group(2, &self.no_reflection, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        let (mut bound, mut current, mut current_texture, mut constant) = (usize::MAX, usize::MAX, usize::MAX, None);
        for &(i, motion) in draws {
            let d = &self.draws[i as usize];
            let texture = match d.texture {
                TextureRef::Fixed(t) => t,
                TextureRef::Animated(a) => {
                    let anim = &self.animations[a];
                    anim.frames[(time * anim.fps) as usize % anim.frames.len()]
                }
                TextureRef::Sky(_) => continue,
            };
            // An additive draw's light between its steps; at a step (or
            // moving) its whole surface (the old frame's light is gone).
            let light = motion == AnimMotion::Still
                && matches!(d.key.blend, Blend::Additive | Blend::DestAlphaAdditive)
                && !d.key.alpha_test;
            let p = 3 * usize::from(d.key.effect == Effect::Reflect)
                + if light { 2 } else { usize::from(d.key.alpha_test) };
            if p != current {
                pass.set_pipeline(&m.pipelines[p]);
                current = p;
                if self.texture_arrays.is_some() {
                    current_texture = usize::MAX;
                }
            }
            if constant != Some(motion) {
                let [r, g] = anim_values(motion);
                pass.set_blend_constant(wgpu::Color { r, g, b: 0.0, a: 0.0 });
                constant = Some(motion);
            }
            if texture != current_texture {
                self.set_world_texture(&mut pass, texture, &mut bound);
                current_texture = texture;
            }
            pass.draw_indexed(d.first_index..d.first_index + d.index_count, d.base_vertex, d.instances.clone());
        }
    }

    /// Screen-Space Reflections' mask ([`WetMask`]): the main view's
    /// worldreflect draws in `order` again.
    fn draw_wet_mask(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        m: &WetMaskPlan,
        reflection: &wgpu::BindGroup,
        order: &[u32],
        time: f32,
        // The main view wrote the road's term and surface (loaded here; the
        // road then draws its depth and kind only, and the road the main
        // view hides its term and surface in a pass against `main_depth`).
        from_main: bool,
        main_depth: &wgpu::TextureView,
    ) {
        let clear =
            wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store };
        let written =
            if from_main { wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } } else { clear };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ssr mask"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: &m.term,
                    depth_slice: None,
                    resolve_target: None,
                    ops: written,
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &m.surface,
                    depth_slice: None,
                    resolve_target: None,
                    ops: written,
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &m.tint,
                    depth_slice: None,
                    resolve_target: None,
                    ops: clear,
                }),
            ],
            // Kept: the cars' mask (car_ssr.wgsl) is tested against it.
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &m.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(m.clear_depth),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        // The overlays the main view drew (the GPS arrow): flagged in the
        // mask (tint.a 1, their view depth), so no traced ray takes their
        // colour (ssr.wgsl `overlay_hit`); drawn first, writing depth, so
        // the road and the cars behind them leave them flagged.
        let overlays: Vec<u32> =
            order.iter().copied().filter(|i| self.overlay_draws.binary_search(i).is_ok()).collect();
        if !m.road && overlays.is_empty() {
            return;
        }
        pass.set_bind_group(0, &self.main_group(), &[]);
        pass.set_bind_group(2, reflection, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        let mut bound = usize::MAX;
        if !overlays.is_empty() {
            pass.set_pipeline(&m.pipelines[2]);
            for d in overlays.iter().map(|&i| &self.draws[i as usize]) {
                let texture = match d.texture {
                    TextureRef::Fixed(t) => t,
                    TextureRef::Animated(a) => self.animations[a].frames[0],
                    TextureRef::Sky(_) => continue,
                };
                self.set_world_texture(&mut pass, texture, &mut bound);
                pass.draw_indexed(d.first_index..d.first_index + d.index_count, d.base_vertex, d.instances.clone());
            }
        }
        if !m.road {
            return;
        }
        let (mut current, mut current_texture) = (None, usize::MAX);
        for d in order.iter().map(|&i| &self.draws[i as usize]).filter(|d| d.key.effect == Effect::Reflect) {
            let texture = match d.texture {
                TextureRef::Fixed(t) => t,
                TextureRef::Animated(a) => {
                    let anim = &self.animations[a];
                    anim.frames[(time * anim.fps) as usize % anim.frames.len()]
                }
                TextureRef::Sky(_) => continue,
            };
            if current != Some(d.key.alpha_test) {
                pass.set_pipeline(match (&m.mrt, from_main) {
                    (Some(mrt), true) => &mrt.kind[usize::from(d.key.alpha_test)],
                    _ => &m.pipelines[usize::from(d.key.alpha_test)],
                });
                current = Some(d.key.alpha_test);
                if self.texture_arrays.is_some() {
                    current_texture = usize::MAX;
                }
            }
            if texture != current_texture {
                self.set_world_texture(&mut pass, texture, &mut bound);
                current_texture = texture;
            }
            pass.draw_indexed(d.first_index..d.first_index + d.index_count, d.base_vertex, d.instances.clone());
        }
        drop(pass);
        let Some(mrt) = m.mrt.as_ref().filter(|_| from_main) else { return };
        // The road the main view hides: its term and surface, as above
        // before the main view wrote them (the SSR filter's taps beside an
        // occluder read them); only fragments behind the main view's depth
        // reach the shader, which keeps the road's frontmost (the mask's
        // own depth, read).
        let load = wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store };
        let attachments = [&m.term, &m.surface, &m.tint]
            .map(|view| Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: load }));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ssr mask hidden road"),
            color_attachments: &attachments,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: main_depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.main_group(), &[]);
        pass.set_bind_group(2, reflection, &[]);
        pass.set_bind_group(3, &m.depth_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        let (mut current, mut current_texture, mut bound) = (None, usize::MAX, usize::MAX);
        for d in order.iter().map(|&i| &self.draws[i as usize]).filter(|d| d.key.effect == Effect::Reflect) {
            let texture = match d.texture {
                TextureRef::Fixed(t) => t,
                TextureRef::Animated(a) => {
                    let anim = &self.animations[a];
                    anim.frames[(time * anim.fps) as usize % anim.frames.len()]
                }
                TextureRef::Sky(_) => continue,
            };
            if current != Some(d.key.alpha_test) {
                pass.set_pipeline(&mrt.hidden[usize::from(d.key.alpha_test)]);
                current = Some(d.key.alpha_test);
                if self.texture_arrays.is_some() {
                    current_texture = usize::MAX;
                }
            }
            if texture != current_texture {
                self.set_world_texture(&mut pass, texture, &mut bound);
                current_texture = texture;
            }
            pass.draw_indexed(d.first_index..d.first_index + d.index_count, d.base_vertex, d.instances.clone());
        }
    }

    /// Record the cube's faces ([`WorldRenderer::plan_environment`]).
    /// The Next-Gen sky while its row is on.
    fn sky(&self) -> Option<&sky_ng::NightSky> {
        self.sky_ng.get().filter(|_| self.sky_on.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// The groups of the visible sections a view draws: the draws a group
    /// has share its flags, LOD and instances, so they are tested
    /// together — by the view's flags and LOD, by the view's planes
    /// ([`PassCull`]; a whole section outside them is passed over, unless
    /// it holds the dome or a moved group), and by the scenery cull of
    /// each instance. A group left out puts no pixel on screen. `sky`:
    /// the view draws the dome (always kept).
    #[allow(clippy::too_many_arguments)]
    pub(in crate::render) fn kept_groups(
        &self,
        visible: &[bool],
        cull: &PassCull,
        view: WorldView,
        reversed: bool,
        sky: bool,
        within: Option<&[u32]>,
        lod: u8,
    ) -> Vec<u32> {
        let instance_cull = self.instance_cull.read().unwrap();
        let slot_spheres = self.slot_spheres.read().unwrap();
        let keeps = |g: u32| {
            let (is_sky, flags, lods, ref instances) = self.group_key[g as usize];
            if (is_sky && !sky) || lods & lod == 0 || view.culls(flags, reversed) {
                return false;
            }
            // The dome follows the camera, so it is never left out; moved
            // groups have left their bounds.
            if is_sky {
                return true;
            }
            // A moved group's bounds no longer hold: each instance is
            // tested by its own sphere as it stands now.
            if self.group_moved[g as usize].load(std::sync::atomic::Ordering::Relaxed) {
                let side = usize::from(cull.mirrored);
                return instances.clone().any(|i| {
                    let (centre, radius) = slot_spheres[i as usize][side];
                    !outside(&cull.planes, centre, radius) && !cull.drops_slot(i as usize, instance_cull[i as usize])
                });
            }
            let (centre, radius) =
                if cull.mirrored { self.mirrored_bounds[g as usize] } else { self.group_bounds[g as usize] };
            if outside(&cull.planes, centre, radius) {
                return false;
            }
            instances.clone().any(|i| !cull.drops_slot(i as usize, instance_cull[i as usize]))
        };
        if let Some(groups) = within {
            return groups.iter().copied().filter(|&g| keeps(g)).collect();
        }
        let mut kept = Vec::new();
        for (i, (section, _, _)) in self.sections.iter().enumerate() {
            if !visible.get(*section as usize).copied().unwrap_or(true) {
                continue;
            }
            let (bounds, mirrored, has_sky) = self.section_bounds[i];
            let (centre, radius) = if cull.mirrored { mirrored } else { bounds };
            if !has_sky
                && !self.section_moved[i].load(std::sync::atomic::Ordering::Relaxed)
                && outside(&cull.planes, centre, radius)
            {
                continue;
            }
            kept.extend(self.section_groups[i].iter().copied().filter(|&g| keeps(g)));
        }
        kept
    }

    /// World Detail Extreme: the groups of the far sections at their lowest
    /// LOD (`LOW_LOD`) in the main view; in a section the main list draws
    /// too (a cross-fade with LOD Fade) only those the main LOD leaves.
    fn kept_far_groups(
        &self,
        far: &[bool],
        main_visible: &[bool],
        cull: &PassCull,
        reversed: bool,
        main_lod: u8,
    ) -> Vec<u32> {
        let mut kept = self.kept_groups(far, cull, WorldView::Main, reversed, false, None, LOW_LOD);
        let both: std::collections::HashSet<u32> = self
            .sections
            .iter()
            .enumerate()
            .filter(|(_, (s, _, _))| {
                far.get(*s as usize).copied().unwrap_or(false)
                    && main_visible.get(*s as usize).copied().unwrap_or(false)
            })
            .flat_map(|(i, _)| self.section_groups[i].iter().copied())
            .filter(|&g| self.group_key[g as usize].2 & main_lod != 0)
            .collect();
        if !both.is_empty() {
            kept.retain(|g| !both.contains(g));
        }
        kept
    }

    /// The groups' draws in the draw order.
    pub(in crate::render) fn group_order(&self, groups: &[u32]) -> Vec<u32> {
        let mut order: Vec<u32> = groups.iter().flat_map(|&g| self.group_draws[g as usize].iter().copied()).collect();
        order.sort_unstable();
        order
    }

    /// Bind world texture slot `slot` for the next draws. `bound` is the
    /// array bound now (usize::MAX for none), kept up to date.
    pub(in crate::render) fn set_world_texture(&self, pass: &mut wgpu::RenderPass<'_>, slot: usize, bound: &mut usize) {
        match &self.texture_arrays {
            Some(a) => {
                let (group, index, layer) = a.group_of(slot);
                if *bound != group {
                    pass.set_bind_group(1, &a.groups[group], &[]);
                    *bound = group;
                }
                // Surface Detail (clone deviation): the slot's map layer and
                // material in the padding, read by the lamps' module only;
                // the last word the array's index with bound arrays.
                let [surface_layer, surface_material, _] = self.surface_slots.pad(slot);
                let imm = TextureImmediates {
                    scroll: self.texture_scrolls[slot],
                    layer,
                    _pad: [surface_layer, surface_material, index],
                };
                pass.set_immediates(0, bytemuck::bytes_of(&imm));
            }
            None => pass.set_bind_group(1, &self.texture_bind_groups[slot], &[]),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::render) fn draw_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        clear: wgpu::Color,
        frame: &wgpu::BindGroup,
        reflection: &wgpu::BindGroup,
        order: &[u32],
        time: f32,
        sky: Option<SkyFrame>,
        view: WorldView,
        counts: (u32, u32),
    ) {
        self.draw_pass_into(encoder, color, depth, clear, frame, reflection, order, time, sky, view, counts, None);
    }

    /// [`WorldCore::draw_pass`], with `multiview`'s pipelines into all six
    /// layers of `color` and `depth` at once when given.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::render) fn draw_pass_into(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        clear: wgpu::Color,
        frame: &wgpu::BindGroup,
        reflection: &wgpu::BindGroup,
        // The draws, in the draw order ([`WorldRenderer::kept_groups`],
        // [`WorldRenderer::group_order`]).
        order: &[u32],
        time: f32,
        // The main view's sky; `None` for the reflection and environment
        // views, which 0x60F890 gives no SKYDOME (they get the scaled-down
        // solid 0x72923C81, which Bayview does not have, in views 0x10–0x15
        // only).
        sky: Option<SkyFrame>,
        view: WorldView,
        // The road reflection's headlight pools and flare quads this frame.
        (pool_count, glow_count): (u32, u32),
        multiview: Option<&HashMap<PipelineKey, wgpu::RenderPipeline>>,
    ) {
        self.draw_pass_faded(
            encoder,
            color,
            depth,
            clear,
            frame,
            reflection,
            order,
            time,
            sky,
            view,
            (pool_count, glow_count),
            multiview,
            None,
            None,
            None,
            false,
        );
    }

    /// The main view's depth pre-pass: `depth` cleared, then the depth
    /// of the opaque draws of `order` ([`in_depth_prepass`]) as their
    /// colour pipelines would write it (LOD Fade's fades and dither from
    /// `fade` when given).
    #[allow(clippy::too_many_arguments)]
    fn draw_depth_prepass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        depth: &wgpu::TextureView,
        clear_depth: f32,
        frame: &wgpu::BindGroup,
        reflection: &wgpu::BindGroup,
        order: &[u32],
        time: f32,
        pipelines: &HashMap<DepthKey, wgpu::RenderPipeline>,
        coverage: bool,
        fade: Option<(&wgpu::Buffer, &[bool])>,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("depth pre-pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_depth),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, frame, &[]);
        pass.set_bind_group(2, reflection, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        if let Some((buffer, _)) = fade {
            pass.set_vertex_buffer(2, buffer.slice(..));
        }
        let (mut current_key, mut current_texture, mut bound) = (None, usize::MAX, usize::MAX);
        for d in order.iter().map(|&i| &self.draws[i as usize]) {
            if !in_depth_prepass(d.key, d.sky, coverage) {
                continue;
            }
            let texture = match d.texture {
                TextureRef::Fixed(t) => t,
                TextureRef::Animated(a) => {
                    let anim = &self.animations[a];
                    anim.frames[(time * anim.fps) as usize % anim.frames.len()]
                }
                TextureRef::Sky(_) => continue,
            };
            let fade_mode = match fade {
                Some((_, mid)) => 1 + u8::from(mid.get(d.group as usize).copied().unwrap_or(false)),
                None => 0,
            };
            let key =
                DepthKey { reflect: d.key.effect == Effect::Reflect, alpha_test: d.key.alpha_test, fade: fade_mode };
            if current_key != Some(key) {
                pass.set_pipeline(&pipelines[&key]);
                current_key = Some(key);
            }
            if texture != current_texture {
                self.set_world_texture(&mut pass, texture, &mut bound);
                current_texture = texture;
            }
            pass.draw_indexed(d.first_index..d.first_index + d.index_count, d.base_vertex, d.instances.clone());
        }
    }

    /// [`WorldCore::draw_pass_into`], the main view with LOD Fade's
    /// pipelines and per-instance fades (`fade`) when given.
    #[allow(clippy::too_many_arguments)]
    fn draw_pass_faded(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        clear: wgpu::Color,
        frame: &wgpu::BindGroup,
        reflection: &wgpu::BindGroup,
        order: &[u32],
        time: f32,
        sky: Option<SkyFrame>,
        view: WorldView,
        (pool_count, glow_count): (u32, u32),
        multiview: Option<&HashMap<PipelineKey, wgpu::RenderPipeline>>,
        fade: Option<(&wgpu::Buffer, &[bool])>,
        rt: Option<&rt_shadow::Plan>,
        // The SSR mask, whose term and surface the main view's road then
        // writes where it can ([`super::plan::MaskMrt`]). -> whether it did.
        mask: Option<&super::plan::WetMaskPlan>,
        // The road dry (`wet` ≤ 0.5): its dry-road pipelines where it has them.
        dry_road: bool,
    ) -> bool {
        let is_reflection = sky.is_none();
        let scene = self.scene_pipelines();
        // The main view's depth as its pipelines test it; the other views
        // keep the native range.
        let clear_depth = if view == WorldView::Main { scene.mode.clear_depth() } else { 1.0 };
        // The main view's opaque depth first (enhanced lighting only);
        // the colour pass then loads it and keeps its own pipelines, so the
        // same draw wins every pixel and the hidden layers skip their
        // shading (docs/perf-gpu.md, depth pre-pass).
        let prepass = match &scene.depth {
            Some(d) if view == WorldView::Main && !is_reflection && multiview.is_none() => {
                let mid = match (fade, &scene.fade) {
                    (Some((buffer, mid)), Some(_)) => Some((buffer, mid)),
                    _ => None,
                };
                self.draw_depth_prepass(
                    encoder,
                    depth,
                    clear_depth,
                    frame,
                    reflection,
                    order,
                    time,
                    &d.depth,
                    scene.mode.coverage,
                    mid,
                );
                crate::prof::gpu::mark(encoder, "world: depth pre-pass");
                // Modern + RT's traced shadows from that depth.
                if let Some(r) = rt {
                    r.record(encoder, depth);
                    crate::prof::gpu::mark(encoder, "world: rt shadows");
                }
                Some(d)
            }
            _ => None,
        };
        // NFSU2_DEPTH_PREPASS=2 (measurements): the colour pass's own pipelines.
        let equal = prepass.filter(|_| depth_prepass_env() != Some(2));
        let depth_load = if prepass.is_some() { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(clear_depth) };
        // The mask from the main view (docs/wet-roads-ssr.md): the order's
        // opaque worldreflect draws, one run of it with no blended one
        // anywhere (the order sorts them so: the opaque road last before
        // the first blended draw), drawn in a pass of their own whose
        // second and third targets are the mask's; the draws before and
        // after them as before, in the same order.
        let split = match (mask, equal) {
            (Some(m), Some(_)) if view == WorldView::Main && !is_reflection && multiview.is_none() => {
                m.mrt.as_ref().and_then(|mrt| {
                    let road = |i: &u32| {
                        let d = &self.draws[*i as usize];
                        d.key.effect == Effect::Reflect && !d.sky
                    };
                    let a = order.iter().position(road)?;
                    let b = a + order[a..].iter().take_while(|i| road(i)).count();
                    let opaque = order[a..b].iter().all(|&i| self.draws[i as usize].key.blend == Blend::None);
                    let one_run = !order[b..].iter().any(road);
                    let faded_ok = fade.is_none() || scene.fade.is_none() || !mrt.road[1].is_empty();
                    (opaque && one_run && faded_ok).then_some((a, b, m, mrt))
                })
            }
            _ => None,
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(if is_reflection { "reflection" } else { "world" }),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
            })],
            // The cube's depth is read by nothing after its face's pass
            // (load/store hygiene: no write-back on tile-based GPUs).
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: depth_load,
                    store: if view == WorldView::Environment { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store },
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: multiview.map(|_| std::num::NonZeroU32::new(ENV_VIEWS).unwrap()),
        });
        pass.set_bind_group(0, frame, &[]);
        pass.set_bind_group(2, reflection, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);

        let faded = match (fade, &scene.fade) {
            (Some((buffer, mid)), Some(p)) if !is_reflection && multiview.is_none() => {
                pass.set_vertex_buffer(2, buffer.slice(..));
                Some((p, mid))
            }
            _ => None,
        };
        let native = multiview.unwrap_or(if is_reflection { &self.reflection_pipelines } else { &scene.pipelines });
        let (mut current_key, mut current_texture, mut bound) = (None, usize::MAX, usize::MAX);
        // Sky Next-Gen (docs/atmosphere.md): in the main view after the opaque draws (before the first
        // blended one, or at the end), depth-tested so it shades only the dome's visible pixels.
        let mut night = self.sky().filter(|_| view == WorldView::Main && !is_reflection && multiview.is_none());
        let sky_key = scene.mode;
        let fade_buffer = fade.filter(|_| faded.is_some()).map(|(b, _)| b);
        let mut i = 0;
        while i < order.len() {
            if let Some((a, b, m, mrt)) = split.filter(|&(a, ..)| a == i) {
                drop(pass);
                self.draw_road_with_mask(
                    encoder,
                    color,
                    depth,
                    frame,
                    reflection,
                    &order[a..b],
                    time,
                    m,
                    mrt,
                    faded.map(|(_, mid)| mid),
                    fade_buffer,
                );
                pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("world"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth,
                        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_bind_group(0, frame, &[]);
                pass.set_bind_group(2, reflection, &[]);
                pass.set_vertex_buffer(0, self.vertices.slice(..));
                pass.set_vertex_buffer(1, self.instances.slice(..));
                pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
                if let Some(buffer) = fade_buffer {
                    pass.set_vertex_buffer(2, buffer.slice(..));
                }
                (current_key, current_texture, bound) = (None, usize::MAX, usize::MAX);
                i = b;
                continue;
            }
            let d = &self.draws[order[i] as usize];
            i += 1;
            if !d.sky
                && d.key.blend != Blend::None
                && let Some(n) = night.take()
                && n.draw(&mut pass, sky_key)
            {
                pass.set_bind_group(0, frame, &[]);
                (current_key, current_texture, bound) = (None, usize::MAX, usize::MAX);
            }
            // LOD Fade: the group's instances all in or out read their
            // fades only; one part-way dithers.
            let (pipelines, dither) = match faded {
                Some((p, mid)) => {
                    let dither = mid.get(d.group as usize).copied().unwrap_or(false);
                    (&p[usize::from(dither)], dither)
                }
                None => (native, false),
            };
            // A draw the pre-pass drew tests EQUAL against its depth (the
            // road's on a dry road: its dry set, `DepthPrepass::equal_dry`).
            let (pipelines, is_equal) = match equal {
                Some(e) if in_depth_prepass(d.key, d.sky, scene.mode.coverage) => {
                    let f = if faded.is_some() { 1 + usize::from(dither) } else { 0 };
                    let dry = &e.equal_dry[f];
                    (if dry_road && dry.contains_key(&d.key) { dry } else { &e.equal[f] }, true)
                }
                _ => (pipelines, false),
            };
            let mut key = if is_reflection { d.key.in_reflection() } else { d.key };
            if view == WorldView::PlanarRoadReflection {
                key.effect = Effect::PlanarNoFog;
            }
            let texture = match d.texture {
                TextureRef::Fixed(t) => t,
                TextureRef::Animated(a) => {
                    let anim = &self.animations[a];
                    anim.frames[(time * anim.fps) as usize % anim.frames.len()]
                }
                TextureRef::Sky(part) => {
                    let sky = sky.unwrap_or_default();
                    // 0x5CF687: the pair faded from goes to SKYMORPHTEXTURE.
                    if let Some((from, _)) = sky.morph {
                        key.effect = Effect::SkyMorph;
                        pass.set_bind_group(3, &self.sky_groups[from + part], &[]);
                    }
                    self.sky_table[sky.set + part]
                }
            };
            if current_key != Some((key, dither, is_equal)) {
                pass.set_pipeline(&pipelines[&key]);
                current_key = Some((key, dither, is_equal));
                // A change of pipeline layout (skymorph's) clears the
                // immediates.
                if self.texture_arrays.is_some() {
                    current_texture = usize::MAX;
                }
            }
            if texture != current_texture {
                self.set_world_texture(&mut pass, texture, &mut bound);
                current_texture = texture;
            }
            pass.draw_indexed(d.first_index..d.first_index + d.index_count, d.base_vertex, d.instances.clone());
        }
        if let Some(n) = night {
            n.draw(&mut pass, sky_key);
            pass.set_bind_group(0, frame, &[]);
            bound = usize::MAX;
        }
        // The cube faces (one by one) show the sky where they drew nothing (their clear black natively),
        // before their pools and flares. The road reflections keep their native clear black: what they leave
        // undrawn is not only sky (a tunnel's or a bridge's underside is not in them), and on a wet road the
        // mirrored overcast lit the whole road from below (rain: the road box ahead of the car +11..+26 luma,
        // the blacks +2.7 against the same settings without it).
        if let Some(n) = self.sky().filter(|_| {
            is_reflection && multiview.is_none() && matches!(view, WorldView::Environment)
        }) {
            n.draw_reflection(&mut pass);
            bound = usize::MAX;
        }

        // Ambient Detail: the street props last, over the road and its decals.
        if view == WorldView::Main && !is_reflection && multiview.is_none() {
            if let Some(plan) = self.ambient_draw.lock().unwrap().as_ref() {
                plan.draw(&mut pass);
            }
        }
        // These effects already carry their authored reflection placement.
        // Keep their original projection when enhanced world geometry uses a
        // local planar capture, avoiding a second mirror of lamps/pools.
        if view.is_road_reflection() {
            pass.set_bind_group(0, &self.reflection_effect_frame.1, &[]);
        }
        // View 4 then draws the headlight pools (0x5CAC17, after the world,
        // 0x566410, 0x631260 and 0x630DF0) with world.efx itself bound
        // (0x5CABDF: effect 0, 0x86EB28), not through 0x5C5940's remap, and
        // the texture's own blend (0x5BCC40; additive).
        if view.is_road_reflection() && pool_count > 0 {
            if let Some((texture, key)) = self.pool_texture {
                pass.set_pipeline(&native[&key]);
                self.set_world_texture(&mut pass, texture, &mut bound);
                pass.set_vertex_buffer(0, self.pool_vertices.slice(..));
                pass.set_vertex_buffer(1, self.pool_instances.slice(..));
                pass.set_index_buffer(self.pool_indices.slice(..), wgpu::IndexFormat::Uint16);
                for i in 0..pool_count {
                    pass.draw_indexed(0..6, 4 * i as i32, i..i + 1);
                }
            }
        }
        // Then the flares (0x5CAC9C: 0x491860(view 4, 2)), into the same
        // image (**inferred**: the sprite batch's render state in view 4 —
        // taken as the glow pass's, depth-tested, not depth-written).
        if view.is_road_reflection() && glow_count > 0 {
            if let Some((texture, key)) = self.reflection_glow_texture {
                pass.set_pipeline(&native[&key]);
                self.set_world_texture(&mut pass, texture, &mut bound);
                pass.set_vertex_buffer(0, self.reflection_glow_vertices.slice(..));
                pass.set_vertex_buffer(1, self.reflection_glow_instance.slice(..));
                pass.set_index_buffer(self.reflection_glow_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..glow_count * 6, 0, 0..1);
            }
        }
        if split.is_some() {
            super::plan::MASK_FROM_MAIN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        split.is_some()
    }

    /// The main view's opaque road (`draws`, one run of the order) in a pass
    /// of its own: the scene's colour and depth loaded (EQUAL after the
    /// pre-pass), and the SSR mask's term and surface cleared and written
    /// by the same draws ([`super::plan::MaskMrt`]'s pipelines, the `_mrt` entries
    /// of world_masks.wgsl); LOD Fade's state as the main pass has it.
    #[allow(clippy::too_many_arguments)]
    fn draw_road_with_mask(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        frame: &wgpu::BindGroup,
        reflection: &wgpu::BindGroup,
        draws: &[u32],
        time: f32,
        m: &super::plan::WetMaskPlan,
        mrt: &super::plan::MaskMrt,
        fade_mid: Option<&[bool]>,
        fade_buffer: Option<&wgpu::Buffer>,
    ) {
        let clear =
            wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("world road + ssr mask"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &m.term,
                    depth_slice: None,
                    resolve_target: None,
                    ops: clear,
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: &m.surface,
                    depth_slice: None,
                    resolve_target: None,
                    ops: clear,
                }),
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, frame, &[]);
        pass.set_bind_group(2, reflection, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        if let Some(buffer) = fade_buffer {
            pass.set_vertex_buffer(2, buffer.slice(..));
        }
        let (mut current, mut current_texture, mut bound) = (None, usize::MAX, usize::MAX);
        for d in draws.iter().map(|&i| &self.draws[i as usize]) {
            let dither = fade_mid.map(|mid| mid.get(d.group as usize).copied().unwrap_or(false));
            let pipelines = &mrt.road[match dither {
                Some(dither) => 1 + usize::from(dither),
                None => 0,
            }];
            let texture = match d.texture {
                TextureRef::Fixed(t) => t,
                TextureRef::Animated(a) => {
                    let anim = &self.animations[a];
                    anim.frames[(time * anim.fps) as usize % anim.frames.len()]
                }
                TextureRef::Sky(_) => continue,
            };
            if current != Some((d.key, dither)) {
                pass.set_pipeline(&pipelines[&d.key]);
                current = Some((d.key, dither));
                if self.texture_arrays.is_some() {
                    current_texture = usize::MAX;
                }
            }
            if texture != current_texture {
                self.set_world_texture(&mut pass, texture, &mut bound);
                current_texture = texture;
            }
            pass.draw_indexed(d.first_index..d.first_index + d.index_count, d.base_vertex, d.instances.clone());
        }
    }
}

impl WorldRenderer {
    /// Record the world into `color`/`depth` (both `_size` pixels; the
    /// reflection has its own fixed size).
    pub fn render(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        size: (u32, u32),
        params: &FrameParams,
    ) {
        self.render_with_reflection(gpu, encoder, color, depth, size, params, &mut |_, _, _| {});
    }

    /// [`WorldRenderer::render`], calling `reflected` with the reflection
    /// image (colour, depth) after the world is drawn into it and before
    /// the main view samples it: the front end draws its car there
    /// mirrored (see `GarageScene`).
    #[allow(clippy::too_many_arguments)]
    pub fn render_with_reflection(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        size: (u32, u32),
        params: &FrameParams,
        reflected: &mut dyn FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView, &wgpu::TextureView),
    ) {
        self.view_size = size;
        let plan = self.plan_views(gpu, params);
        self.core.record_views(encoder, &plan, color, depth, reflected);
    }

    /// [`WorldRenderer::render`] into a command buffer of its own, recorded
    /// and encoded on a render thread (the uniforms are written here, so it
    /// goes to the GPU as if recorded now).
    /// The scene's sample count (the anti-aliasing, 0x870D08): its
    /// pipelines (main view, shadows, particles, glows) rebuilt at `samples`
    /// when it changes (from the device's shared cache).
    /// With the Graphics page's Reversed-Z or MSAA on, the pipelines also
    /// follow those ([`SceneMode::current`]).
    pub fn set_samples(&self, gpu: &Gpu, samples: u32, opts: &RenderOptions) {
        self.set_mode(gpu, SceneMode::current(opts, samples), opts.lod_fade);
    }

    /// The main view's pipelines a drive's first frame takes
    /// ([`WorldRenderer::set_samples`] at `samples` with the temporal pass
    /// as the drive will run it, [`SceneMode::driving`]), built now: at
    /// the drive's load, behind its loading screen, not in that frame. The
    /// same pipelines (the device's shared cache); the first frame's
    /// `set_samples` then finds them in place.
    pub fn warm(&self, gpu: &Gpu, samples: u32, opts: &RenderOptions) {
        self.set_mode(gpu, SceneMode::driving(opts, samples), opts.lod_fade);
    }

    /// `lod_fade`: LOD Fade in force (its fade and equal-depth pipelines).
    fn set_mode(&self, gpu: &Gpu, mode: SceneMode, lod_fade: bool) {
        if self.scene_pipelines().mode == mode {
            return;
        }
        if crate::render::debug_env::nfsu2_pipeline_log() {
            eprintln!("world scene pipelines: {:?} -> {mode:?}", self.scene_pipelines().mode);
        }
        let built = scene_pipelines_for(gpu, self.arrays, self.scene_format, &self.glow_keys, mode, lod_fade);
        *self.scene.write().unwrap() = std::sync::Arc::new(built);
    }

    /// The scene's size for [`WorldRenderer::render_async`] (the road
    /// reflection's size follows it when Road Reflection Res is not
    /// native).
    pub fn set_view_size(&mut self, size: (u32, u32)) {
        self.view_size = size;
    }

    /// The output's size the scene is shown at: Modern + RT's traced
    /// shadows take their blocks from it when the scene is supersampled
    /// (`rt_shadow::grid`).
    pub fn set_screen_size(&mut self, size: (u32, u32)) {
        self.screen_size = size;
    }

    pub fn render_async(
        &mut self,
        gpu: &Gpu,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        params: &FrameParams,
    ) -> Finishing {
        let plan = self.plan_views(gpu, params);
        let core = self.core.clone();
        let (color, depth) = (color.clone(), depth.clone());
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        crate::prof::gpu::mark(&mut encoder, crate::prof::gpu::GAP);
        Finishing::spawn(move || {
            core.record_views(&mut encoder, &plan, &color, &depth, &mut |_, _, _| {});
            encoder.finish()
        })
    }
}

/// The scene's pipelines for `mode` from the device's shared sets
/// ([`crate::render::Gpu::world_shared`]): what a world renderer with these
/// texture arrays (`arrays`), scene format and glow keys takes for its
/// frames. Built where the sets do not hold them yet, so a call with no
/// renderer at hand (a drive's programs made ahead,
/// [`warm_scene_programs`]) leaves them there for the renderers.
pub(in crate::render) fn scene_pipelines_for(
    gpu: &Gpu,
    arrays: bool,
    scene_format: wgpu::TextureFormat,
    glow_keys: &[PipelineKey],
    mode: SceneMode,
    lod_fade: bool,
) -> ScenePipelines {
    let shared = gpu.world_shared();
    let device = &gpu.device;
    let all = PipelineKey::all();
    ScenePipelines {
        mode,
        pipelines: shared.pipelines_for(
            device,
            arrays,
            false,
            mode,
            scene_format,
            wgpu::ColorWrites::ALL,
            &all,
        ),
        glow: shared.pipelines_for(
            device,
            arrays,
            false,
            mode,
            scene_format,
            wgpu::ColorWrites::COLOR,
            glow_keys,
        ),
        glow_streak: shared.pipelines_for(
            device,
            arrays,
            false,
            mode,
            crate::render::postfx::STREAK_FORMAT,
            wgpu::ColorWrites::COLOR,
            glow_keys,
        ),
        fade: lod_fade.then(|| {
            [false, true].map(|d| shared.fade_pipelines(device, arrays, mode, scene_format, &all, d))
        }),
        car_shadow: if mode.lamps {
            shared.pipelines_for(
                device,
                arrays,
                false,
                mode,
                scene_format,
                wgpu::ColorWrites::ALL,
                &car_shadow_keys(),
            )
        } else {
            HashMap::new()
        },
        depth: depth_prepass(mode).then(|| {
            std::sync::Arc::new(DepthPrepass {
                depth: shared.depth_pipelines(device, arrays, mode),
                equal: [0, 1, 2].map(|f| {
                    if f > 0 && !lod_fade {
                        HashMap::new()
                    } else {
                        shared.equal_pipelines(device, arrays, mode, scene_format, f)
                    }
                }),
                equal_dry: [0, 1, 2].map(|f| {
                    if f > 0 && !lod_fade {
                        HashMap::new()
                    } else {
                        DepthPrepass::dry_for(shared, device, arrays, mode, scene_format, f)
                    }
                }),
            })
        }),
        characters: if mode.modern == 2 {
            let keys: Vec<PipelineKey> = all.iter().copied().filter(|k| k.effect == Effect::World).collect();
            shared.pipelines_for(
                device,
                arrays,
                false,
                SceneMode { lamp_features: mode.lamp_features | LF_ML_CHAR, ..mode },
                scene_format,
                wgpu::ColorWrites::ALL,
                &keys,
            )
        } else {
            HashMap::new()
        },
    }
}

/// Whether this device's world renderers keep their textures in arrays
/// (`build::textures`): the form their pipelines are made for.
pub(in crate::render) fn texture_arrays_in_use(device: &wgpu::Device) -> bool {
    device.features().contains(wgpu::Features::IMMEDIATES) && !crate::render::debug_env::nfsu2_no_texture_arrays()
}

/// The world's programs a drive's frames take that depend on the settings
/// and the window's format alone, made with no world at hand (the menus,
/// the prepare mode): the scene's pipelines for every key at the drive's
/// mode ([`SceneMode::driving`]; the glows' are a world's own keys and
/// stay with its renderer), the lamps' shadow layers', Wet Road
/// Interaction's trail map's, the traced shadows' and the weather
/// passes'. They go into the device's shared sets and its cache, where a
/// renderer's own warm-up ([`WorldRenderer::warm_drive`]) and frames then
/// find them: the same objects from the same descriptors.
pub fn warm_scene_programs(gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, opts: &RenderOptions) {
    let arrays = texture_arrays_in_use(&gpu.device);
    let mode = SceneMode::driving(opts, samples);
    drop(scene_pipelines_for(gpu, arrays, format, &[], mode, opts.lod_fade));
    if opts.lamp_shadows != 0 || modern_light::district(opts).is_some() {
        let _ = crate::render::lamps::shadow_pipelines(gpu, arrays);
    }
    if opts.wet_interaction != 0 {
        drop(crate::render::wet_road::WetRoad::new(gpu, opts.wet_interaction));
    }
    if mode.modern == 2 {
        rt_shadow::RtShadow::new(gpu).warm(gpu, mode.samples > 1);
    }
    if weather_passes(opts) {
        crate::render::weather_fx::WeatherFx::new(gpu).warm(mode.target(format), mode.samples);
    }
}

/// Whether `opts`' frames draw the rain's and spray's passes (their
/// programs made with the first rain).
fn weather_passes(opts: &RenderOptions) -> bool {
    opts.rain_detail != 0 || opts.spray || opts.wet_interaction != 0
}

/// The programs a front-end scene's world (the main menu's and the shops'
/// garage scenes, `WorldRenderer::set_samples` at the frame's count) takes
/// beyond [`warm_scene_programs`]'s: the weather passes for its own target
/// and `samples` ([`SceneMode::current`] with the temporal pass off, as
/// those frames run). A garage's renderer is made in its visit's first
/// tick and its weather passes with the first rain drawn there; these go
/// into the device's cache, where its [`crate::render::weather_fx::WeatherFx`]
/// then finds them (a drive's at the same target and count are found made).
pub fn warm_front_scene_programs(gpu: &Gpu, format: wgpu::TextureFormat, samples: u32, opts: &RenderOptions) {
    if !weather_passes(opts) {
        return;
    }
    let front = SceneMode::current(&RenderOptions { temporal_frame: false, ..*opts }, samples);
    crate::render::weather_fx::WeatherFx::new(gpu).warm(front.target(format), front.samples);
}
