//! Placements moved, reset and hidden (moved from render/mod.rs).

use crate::render::*;

impl WorldRenderer {
    /// An instance slot's group has left its bounds.
    fn mark_moved(&self, slot: u32) {
        if let Some(&g) = self.slot_group.get(slot as usize) {
            self.group_moved[g as usize].store(true, std::sync::atomic::Ordering::Relaxed);
            self.section_moved[self.group_section[g as usize] as usize]
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Move one placement (a knocked-over prop) to `transform`.
    pub fn move_placement(&self, gpu: &Gpu, placement: usize, transform: Mat4) {
        self.move_placements(gpu, std::iter::once((placement, transform)));
    }

    /// [`WorldRenderer::move_placement`] for many: the locks taken once
    /// and the instances written in runs of adjacent slots.
    pub fn move_placements(&self, gpu: &Gpu, moves: impl IntoIterator<Item = (usize, Mat4)>) {
        let mut writes: Vec<(u32, [[f32; 4]; 4])> = Vec::new();
        {
            let mut moved = self.moved.lock().unwrap();
            let mut cull = self.instance_cull.write().unwrap();
            let mut spheres = self.slot_spheres.write().unwrap();
            for (placement, transform) in moves {
                let Some(slots) = self.placement_slots.get(placement) else { continue };
                // Written there already: the same matrix, cull values,
                // spheres and moved marks would be written again.
                let bits = transform.to_cols_array().map(f32::to_bits);
                if moved.insert(placement, bits) == Some(bits) {
                    continue;
                }
                let radius = self.placement_radius.get(placement).copied().unwrap_or(0.0);
                let hidden = self.placement_hidden.get(placement).copied().unwrap_or(false);
                let bytes = instance_matrix(transform, true, radius, hidden);
                for &slot in slots {
                    self.mark_moved(slot);
                    cull[slot as usize] = instance_cull(&bytes);
                    spheres[slot as usize] =
                        instance_spheres(self.group_local[self.slot_group[slot as usize] as usize], transform, true);
                    writes.push((slot, bytes));
                }
            }
        }
        write_slots(gpu, &self.instances, writes);
    }

    /// Every placement back as `world` has it, as a new renderer of it
    /// would start (for a load that keeps the world and its renderer, after
    /// [`World::reset_to_loaded`]): the instances rewritten in one copy and
    /// no group marked moved, and the world's emitters made the loaded
    /// ones. Construction-time state only.
    pub fn reset_placements(&mut self, gpu: &Gpu, world: &World) {
        self.reset_instances(gpu, world);
        modern_light_bold::load_emitters(world);
    }

    /// [`WorldRenderer::reset_placements`] but for the loaded emitters (a
    /// process-wide table the frames read): this renderer's own state,
    /// which a car change can make ready before the next drive's load
    /// (that load then sets the emitters, [`modern_light_bold::set_loaded_emitters`]).
    pub fn reset_instances(&mut self, gpu: &Gpu, world: &World) {
        self.moved.get_mut().unwrap().clear();
        let mut instance_data = vec![[[0.0f32; 4]; 4]; self.slot_group.len()];
        let mut spheres = self.slot_spheres.write().unwrap();
        for (i, p) in world.placements.iter().enumerate() {
            let m =
                instance_matrix(p.transform, p.flags & NO_MATRIX_FLAG == 0, p.cull_radius, p.flags & HIDDEN_FLAG != 0);
            for &slot in self.placement_slots.get(i).map_or(&[][..], |s| &s[..]) {
                if let Some(d) = instance_data.get_mut(slot as usize) {
                    *d = m;
                }
                // Only a moved group's spheres can have changed.
                let g = self.slot_group[slot as usize] as usize;
                if self.group_moved[g].load(std::sync::atomic::Ordering::Relaxed) {
                    spheres[slot as usize] =
                        instance_spheres(self.group_local[g], p.transform, p.flags & NO_MATRIX_FLAG == 0);
                }
            }
        }
        drop(spheres);
        gpu.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instance_data));
        *self.instance_cull.write().unwrap() = instance_data.iter().map(instance_cull).collect();
        self.placement_hidden = world.placements.iter().map(|p| p.flags & HIDDEN_FLAG != 0).collect();
        self.placement_radius = world.placements.iter().map(|p| p.cull_radius).collect();
        for m in self.group_moved.iter().chain(self.section_moved.iter()) {
            m.store(false, std::sync::atomic::Ordering::Relaxed);
        }
        self.lamps = lamps::lamps_of(&world.light_packs);
        self.lamp = None;
    }

    /// Take a placement's new flags and transform (a scenery group
    /// switched, or the race-only scenery mirrored: `World::take_changed`).
    /// Only [`HIDDEN_FLAG`] may change among the flags the views test.
    pub fn update_placement(&mut self, gpu: &Gpu, placement: usize, transform: Mat4, flags: u16) {
        self.moved.get_mut().unwrap().remove(&placement);
        let Some(slots) = self.placement_slots.get(placement) else { return };
        let radius = self.placement_radius.get(placement).copied().unwrap_or(0.0);
        let hidden = flags & HIDDEN_FLAG != 0;
        let bytes = instance_matrix(transform, flags & NO_MATRIX_FLAG == 0, radius, hidden);
        // Lamp Shadows' layers drawn with the placement as it was (its
        // instances' bounds before and after, the unmirrored ones the
        // layers' views test), when the layers draw it at all and it was
        // or is shown.
        let was_hidden = self.placement_hidden.get(placement).copied().unwrap_or(hidden);
        let layers = self.lamp.as_ref().is_some_and(|s| s.maps.is_some())
            && !(was_hidden && hidden)
            && self.casts_lamp_shadows(slots);
        let mut bounds = Vec::new();
        let mut moved = false;
        let mut cull = self.instance_cull.write().unwrap();
        let mut spheres = self.slot_spheres.write().unwrap();
        for &slot in slots {
            self.mark_moved(slot);
            cull[slot as usize] = instance_cull(&bytes);
            let now = instance_spheres(
                self.group_local[self.slot_group[slot as usize] as usize],
                transform,
                flags & NO_MATRIX_FLAG == 0,
            );
            if layers {
                moved |= spheres[slot as usize][0] != now[0];
                bounds.extend([spheres[slot as usize][0], now[0]]);
            }
            spheres[slot as usize] = now;
            gpu.queue.write_buffer(&self.instances, slot as u64 * 64, bytemuck::cast_slice(&bytes));
        }
        drop((cull, spheres));
        if let Some(h) = self.placement_hidden.get_mut(placement) {
            *h = hidden;
        }
        // (A placement a set-up hid and showed again, or the other way,
        // is as the layers drew it.)
        if !bounds.is_empty() && (moved || was_hidden != hidden) {
            self.release_lamp_layers_seeing(&bounds);
        }
    }

    /// Whether the lamps' shadow layers draw one of `slots`: a draw of
    /// its group that is not blended and not the sky
    /// (`WorldCore::record_lamp_shadows`). Blended scenery (the barriers'
    /// tape, the start lines' graphics) is in no layer, so its switch keeps
    /// them.
    fn casts_lamp_shadows(&self, slots: &[u32]) -> bool {
        slots.iter().any(|&slot| {
            self.slot_group.get(slot as usize).and_then(|&g| self.group_draws.get(g as usize)).is_some_and(|draws| {
                draws.iter().any(|&d| {
                    let d = &self.draws[d as usize];
                    !d.sky && d.key.blend == crate::formats::texture::Blend::None
                })
            })
        })
    }

    /// Placement `i`'s instances' bounds as the lamps' shadow layers test
    /// them (unmirrored), when the layers draw it; none when they do not
    /// (the tests).
    pub fn lamp_shadow_bounds(&self, i: usize) -> Vec<(Vec3, f32)> {
        let slots = self.placement_slots.get(i).map_or(&[][..], |s| &s[..]);
        if !self.casts_lamp_shadows(slots) {
            return Vec::new();
        }
        let spheres = self.slot_spheres.read().unwrap();
        slots.iter().map(|&slot| spheres[slot as usize][0]).collect()
    }

    /// Whether placement `i` is hidden (flag 0x10: no view draws it).
    pub fn placement_hidden(&self, i: usize) -> bool {
        self.placement_hidden.get(i).copied().unwrap_or(false)
    }
}
