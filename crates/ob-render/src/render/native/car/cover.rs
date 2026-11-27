//! The reward-cover queue order (moved from car.rs).

use super::*;

impl CarRenderer {
    /// Reward-cover queue: car effect 5 and shadow effect 13 share the
    /// native (late, order) buckets. Keep texture groups in first-use
    /// order; draw the shadow after car order 2, before higher/late buckets.
    #[allow(clippy::too_many_arguments)]
    pub fn render_cover(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        view_proj: Mat4,
        view: Mat4,
        lighting: &CarLighting,
        world: Mat4,
        wheels: &[WheelPose; 4],
        shadow: impl FnOnce(&mut wgpu::CommandEncoder),
        opts: &crate::render::RenderOptions,
    ) {
        self.write_uniforms(gpu, view_proj, view, lighting, world, wheels, opts);
        let mut submitted: Vec<_> = (0..self.draws.len()).collect();
        submitted.sort_by_key(|&i| self.draws[i].submission);
        let keys: Vec<_> = submitted
            .iter()
            .map(|&i| {
                let d = &self.draws[i];
                self.lod
                    .filter(|&lod| self.draw_visible(d, lod, lighting.is_frontend()))
                    .map(|_| (d.cover_bucket.0, d.cover_bucket.1, d.texture))
            })
            .collect();
        let (before, after) = cover_draw_order(&keys);
        let record = |encoder: &mut wgpu::CommandEncoder, indices: &[usize]| {
            let Some(lod) = self.lod else { return };
            let mut pass = begin_pass(encoder, color, depth);
            self.record_lod_draws(
                &self.pipelines,
                &mut pass,
                lighting.is_frontend(),
                &self.frame_group,
                &self.draw_group,
                0,
                lod,
                indices.iter().map(|&i| {
                    let i = submitted[i];
                    (i, &self.draws[i])
                }),
            );
        };
        record(encoder, &before);
        shadow(encoder);
        record(encoder, &after);
    }
}

/// 0x5CEB40 / 0x5CF430: non-late first, order, effect, first-use texture.
/// All entries here use car effect 5; SHADOWFE uses (false, 2, 13).
pub(super) fn cover_draw_order(keys: &[Option<(bool, u8, usize)>]) -> (Vec<usize>, Vec<usize>) {
    // Input is original submission order, with excluded LOD/scene draws
    // removed before registering each bucket's first-use texture groups.
    let mut groups = HashMap::new();
    let mut order = Vec::new();
    for (i, key) in keys.iter().enumerate() {
        let Some((late, bucket, texture)) = *key else { continue };
        let rank = groups.len();
        let rank = *groups.entry((late, bucket, texture)).or_insert(rank);
        order.push((i, late, bucket, rank));
    }
    order.sort_by_key(|&(_, late, bucket, rank)| (late, bucket, rank));
    let (before, after): (Vec<_>, Vec<_>) = order.into_iter().partition(|&(_, late, bucket, _)| !late && bucket <= 2);
    (before.into_iter().map(|v| v.0).collect(), after.into_iter().map(|v| v.0).collect())
}
