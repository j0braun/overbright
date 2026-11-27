//! Motion blur's screen passes (moved from render/postfx.rs).

use super::*;

impl PostFx {
    /// Motion blur (0x5CC470) over the scene as drawn so far: the scene to
    /// screen copy 0, blurred into copy 1 and back with 4× the shift, then
    /// lerped over the scene by the blend factor.
    pub fn motion_blur(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder, blur: crate::motion_blur::Blur) {
        if self.targets.is_none() {
            return;
        }
        let pass = |scale: f32| {
            let mut u = PassUniform {
                weights: [[0.25; 4]; 4],
                offsets: [[0.0; 4]; 4],
                params: [4.0, 0.0, 0.0, 0.0],
                grey: [0.0; 4],
                tint: [0.0; 4],
            };
            u.offsets[0] = blur.shift.map(|v| v * scale);
            u
        };
        let copy = PassUniform {
            weights: [[1.0; 4]; 4],
            offsets: [[0.0; 4]; 4],
            params: [1.0, 0.0, 0.0, 0.0],
            grey: [0.0; 4],
            tint: [0.0; 4],
        };
        // The three slots (MOTION1, MOTION2, MOTION_COPY: consecutive) in
        // one write.
        let mut data = vec![0u8; (SLOT * 3) as usize];
        for (i, u) in [pass(1.0), pass(crate::motion_blur::SECOND_PASS), copy].iter().enumerate() {
            let o = i * SLOT as usize;
            data[o..o + std::mem::size_of::<PassUniform>()].copy_from_slice(bytemuck::bytes_of(u));
        }
        const _: () = assert!(MOTION2 == MOTION1 + 1 && MOTION_COPY == MOTION1 + 2);
        gpu.queue.write_buffer(&self.uniforms, MOTION1 as u64 * SLOT, &data);
        let t = self.targets.as_ref().unwrap();
        let v = &t.views;
        let (scene, full0, full1) = (v.scene.clone(), v.full[0].clone(), v.full[1].clone());
        // The game blurs a StretchRect copy of the back buffer (screen copy
        // 0); the first blur reads the scene itself, the same 8-bit
        // texels, so the copy is left out (multisampled, the scene is
        // resolved into it first, as the StretchRect resolves).
        self.resolve(encoder);
        let inter = self.inter();
        self.draw(gpu, encoder, &full1, inter, "fs_motion", Blend::Replace, Src::Scene, MOTION1, None, None);
        self.draw(gpu, encoder, &full0, inter, "fs_motion", Blend::Replace, Src::Full1, MOTION2, None, None);
        let f = f64::from(blur.factor) / 255.0;
        let out = self.scene_format();
        let constant = Some(wgpu::Color { r: f, g: f, b: f, a: f });
        match self.msaa.as_ref().filter(|_| self.samples > 1).map(|m| m.color.clone()) {
            // Over the multisampled scene, each sample lerped.
            Some(color) => {
                let p = self.pipeline_ms(gpu, "fs_filter", out, Blend::LerpConstant, self.samples);
                let t = self.targets.as_ref().unwrap();
                full_pass(
                    encoder,
                    "fs_filter",
                    &self.pipelines[p].1,
                    &[&color],
                    &t.groups[Src::Full0 as usize],
                    MOTION_COPY,
                    constant,
                    None,
                );
            }
            None => self.draw(
                gpu,
                encoder,
                &scene,
                out,
                "fs_filter",
                Blend::LerpConstant,
                Src::Full0,
                MOTION_COPY,
                constant,
                None,
            ),
        }
    }
}
