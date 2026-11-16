//! Shared transient render targets (docs/perf-gpu.md "Render-target
//! memory"). A pass that opts in asks the device's pool for a target by a
//! [`Slot`] and its descriptor (size, format, samples, usage) instead of
//! making its own; every pass asking for the same slot and descriptor gets
//! the same texture. So a slot's users must never hold its contents across
//! another user's passes: each writes all of it before reading it, and
//! reads it only before the next user's first write, in queue order (wgpu
//! puts the barriers between them). The slots and their users, with the
//! frame order that makes them safe, are listed at [`Slot`]. Only clone
//! deviations opt in (the Original look's targets stay its own).
//!
//! A pooled target lives while one user holds it (the pool keeps a weak
//! reference): a resize or a tier switched off frees it once its last user
//! lets go. `NFSU2_RT_POOL=0` gives every request a texture of its own (A/B).
use std::sync::{Arc, Mutex, Weak};

/// Which users share a target (all on one frame's queue, in this order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// A scene-sized copy of the scene, rewritten in full before each read:
    /// Enhanced Glass's copy (the car pass), the motion blur / depth of
    /// field copy (after the cars, before the particles), Screen-Space
    /// Reflections' copy (the post's pre-passes).
    SceneCopy,
    /// A second scene-sized image beside [`Slot::SceneCopy`], cleared before
    /// each use: the motion blur's velocity / the depth of field's circle
    /// of confusion, then Screen-Space Reflections' glow sprites.
    SceneAux,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    slot: Slot,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    samples: u32,
    usage: wgpu::TextureUsages,
}

/// A pooled target: the texture and its whole view (derefs to the view).
pub struct Target {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl std::ops::Deref for Target {
    type Target = wgpu::TextureView;
    fn deref(&self) -> &wgpu::TextureView {
        &self.view
    }
}

#[derive(Default)]
pub struct TargetPool {
    live: Mutex<Vec<(Key, Weak<Target>)>>,
}

/// Whether requests share (`NFSU2_RT_POOL=0`: no; read once).
pub fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| crate::render::debug_env::nfsu2_rt_pool())
}

impl TargetPool {
    /// The slot's target of this descriptor, made if no user holds one.
    pub fn get(
        &self,
        device: &wgpu::Device,
        slot: Slot,
        label: &str,
        size: (u32, u32),
        format: wgpu::TextureFormat,
        samples: u32,
        usage: wgpu::TextureUsages,
    ) -> Arc<Target> {
        let key = Key { slot, size: (size.0.max(1), size.1.max(1)), format, samples, usage };
        let mut live = self.live.lock().unwrap();
        live.retain(|(_, w)| w.strong_count() > 0);
        if enabled()
            && let Some(t) = live.iter().find(|(k, _)| *k == key).and_then(|(_, w)| w.upgrade())
        {
            return t;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: key.size.0, height: key.size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let t = Arc::new(Target { texture, view });
        live.push((key, Arc::downgrade(&t)));
        t
    }

    /// Live pooled targets (tests).
    pub fn live(&self) -> usize {
        let live = self.live.lock().unwrap();
        live.iter().filter(|(_, w)| w.strong_count() > 0).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same slot and descriptor: one texture; another slot, size or format:
    /// another; freed with its last holder.
    #[test]
    #[ignore = "requires a GPU adapter; run serially with --ignored"]
    fn shares_by_slot_and_descriptor_and_frees_with_last_holder() {
        let gpu = pollster::block_on(crate::render::Gpu::new(
            None,
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
        ))
        .unwrap();
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let f = wgpu::TextureFormat::Rgba16Float;
        let pool = TargetPool::default();
        let a = pool.get(&gpu.device, Slot::SceneCopy, "a", (64, 32), f, 1, usage);
        let b = pool.get(&gpu.device, Slot::SceneCopy, "b", (64, 32), f, 1, usage);
        assert!(Arc::ptr_eq(&a, &b));
        let c = pool.get(&gpu.device, Slot::SceneAux, "c", (64, 32), f, 1, usage);
        let d = pool.get(&gpu.device, Slot::SceneCopy, "d", (64, 33), f, 1, usage);
        let e = pool.get(&gpu.device, Slot::SceneCopy, "e", (64, 32), wgpu::TextureFormat::Rgba8Unorm, 1, usage);
        assert!(!Arc::ptr_eq(&a, &c) && !Arc::ptr_eq(&a, &d) && !Arc::ptr_eq(&a, &e));
        assert_eq!(pool.live(), 4);
        drop((a, c, d, e));
        assert_eq!(pool.live(), 1);
        drop(b);
        assert_eq!(pool.live(), 0);
    }
}
