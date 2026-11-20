use super::*;

/// Once a frame, from the game: with Reload Packs When Changed on, keeps
/// the watcher running and puts the HD UI Art row (`hd_ui`) in force at
/// once. True when the map calibration must be applied again
/// (`frontend::map_pack::reapply`).
pub fn frame(hd_ui: bool) -> bool {
    static LAST_HD: OnceLock<Mutex<Option<bool>>> = OnceLock::new();
    // A Studio test run's live textures watch whatever the setting says.
    if !crate::qol::reload_packs() && live().is_none() {
        if RUNNING.load(Ordering::Relaxed) {
            STOP.store(true, Ordering::Relaxed);
        }
        return false;
    }
    if !RUNNING.load(Ordering::Relaxed) {
        spawn();
    }
    HD_REQUESTED.store(hd_ui, Ordering::Relaxed);
    let mut last = LAST_HD.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if *last != Some(hd_ui) {
        *last = Some(hd_ui);
        let before = crate::hd_ui_pack::active_root();
        crate::hd_ui_pack::set_live(hd_ui);
        if crate::hd_ui_pack::active_root() != before {
            log::info!("texture packs: HD UI Art {}", if hd_ui { "on" } else { "off" });
            publish(ALL, None);
            MAP_DIRTY.store(true, Ordering::Relaxed);
        }
    }
    MAP_DIRTY.swap(false, Ordering::Relaxed)
}

// ------------------------------------------------------------- world

impl crate::render::WorldRenderer {
    /// Reload Packs When Changed: the world textures whose pack file
    /// changed. With texture arrays, a replacement of the same shape is
    /// written over its layer; anything else (a new size, a texture gaining
    /// or losing its replacement, a whole-pack reload) lays the arrays out
    /// and uploads them again (geometry, draws and the native cache stay).
    /// Left for a later frame while a render thread holds the core. True
    /// when textures changed. The textures added later (car shadows, the
    /// race start's characters) and Surface Detail maps keep theirs.
    pub fn sync_texture_pack(&mut self, gpu: &crate::render::Gpu, world: &crate::world::World) -> bool {
        let (now, pending) = since(self.pack_seen, Domain::World);
        if !pending.any() {
            self.pack_seen = now;
            return false;
        }
        let sources: Vec<Option<&crate::formats::texture::Texture>> =
            self.pack_slots.iter().map(|h| h.and_then(|h| world.textures.get(&h))).collect();
        let hit: Vec<usize> = sources
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_some_and(|t| pending.hits(t.hash, &t.name)))
            .map(|(i, _)| i)
            .collect();
        if hit.is_empty() {
            self.pack_seen = now;
            return false;
        }
        let device = &gpu.device;
        let bc = device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let max = device.limits().max_texture_dimension_2d;
        // Same shapes: each layer rewritten in place.
        if let (Some(a), Pending::Some(_)) = (self.core.texture_arrays.as_ref(), &pending) {
            let layers: Vec<_> = hit
                .iter()
                .map(|&slot| (slot, crate::render::texture_layer_with_pack(Domain::World, sources[slot], bc, max)))
                .collect();
            let fits = layers.iter().all(|(slot, (key, _))| {
                let t = &a.arrays[a.slots[*slot].0 as usize];
                (t.format(), t.width(), t.height(), t.mip_level_count()) == *key
            });
            if fits {
                for (slot, (key, bytes)) in &layers {
                    let (array, layer) = a.slots[*slot];
                    let one = crate::render::prepared::single(*key, bytes);
                    for (mip, level) in one.levels.iter().enumerate() {
                        gpu.queue.write_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture: &a.arrays[array as usize],
                                mip_level: mip as u32,
                                origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
                                aspect: wgpu::TextureAspect::All,
                            },
                            &level.data,
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(level.bytes_per_row),
                                rows_per_image: Some(level.rows),
                            },
                            wgpu::Extent3d { width: level.extent.0, height: level.extent.1, depth_or_array_layers: 1 },
                        );
                    }
                }
                log::info!("texture packs: {} world texture(s) rewritten in place", layers.len());
                self.pack_seen = now;
                return true;
            }
        }
        let Some(core) = std::sync::Arc::get_mut(&mut self.core) else { return false };
        let sampler = &self.texture_sampler;
        let started = std::time::Instant::now();
        match core.texture_arrays.as_ref() {
            Some(_) => {
                let max_layers = device.limits().max_texture_array_layers.max(1) as usize;
                let arrays = crate::render::prepared::arrays_with_pack(&sources, bc, max_layers, max);
                let made = crate::render::TextureArrays::new(gpu, sampler, &arrays);
                let morph = crate::render::make_texture_layout(device, "skymorph texture", false);
                core.sky_groups = core
                    .sky_table
                    .iter()
                    .map(|&slot| crate::render::bind_texture(device, &morph, &made.view(slot), sampler))
                    .collect();
                core.texture_arrays = Some(made);
            }
            None => {
                for &slot in &hit {
                    let Some(t) = sources[slot] else { continue };
                    let view = crate::render::upload_texture_for(gpu, t, bc, Domain::World);
                    core.texture_bind_groups[slot] =
                        crate::render::bind_world_texture(device, &self.texture_layout, sampler, &view, &t.scroll);
                }
                core.sky_groups = core.sky_table.iter().map(|&slot| core.texture_bind_groups[slot].clone()).collect();
            }
        }
        log::info!(
            "texture packs: world textures uploaded again ({} changed) in {:.0} ms",
            hit.len(),
            started.elapsed().as_secs_f64() * 1000.0
        );
        self.pack_seen = now;
        true
    }
}
