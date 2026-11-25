use super::*;

impl UiRenderer {
    /// [`UiRenderer::record`] without the encoding (wgpu's `finish`), for a
    /// render thread to finish ([`crate::render::Finishing`]): the same
    /// commands, so the same image.
    #[allow(clippy::too_many_arguments)]
    pub fn record_encoder(
        &mut self,
        gpu: &Gpu,
        assets: &Assets,
        target: &wgpu::TextureView,
        size: (u32, u32),
        quads: &[Quad],
        clear: bool,
        alpha: Alpha,
    ) -> wgpu::CommandEncoder {
        self.sync_pack(assets);
        for q in quads {
            if let Some(h) = q.texture {
                self.bind(gpu, assets, h);
            }
        }
        // The canvas covers the target, each axis scaled on its own
        // (**verified**): 0x5C5030 writes x / 320 − 1, 1 − y / 240 straight
        // into the vertices and fe.efx's vertex shader multiplies them by
        // WorldViewProj, which the front-end draw (0x5CC03B..0x5CC09D)
        // sets from identity view and world matrices (0x5BDDF0). So no
        // half-pixel correction. Under Direct3D 9's pixel-centre rule
        // (pixel i's centre on the window coordinate i) pixel i samples the
        // picture at i, where wgpu (centres at i + 0.5) samples it at
        // i + 0.5: the original's front end lands half a target pixel down
        // and right of where wgpu puts the same clip coordinates, so the
        // clone moves its vertices down and right by that half pixel.
        // **Observed** (2026-10-31, vanilla115, the original without the
        // widescreen fix at 1280 × 960): every FE edge of the main menu
        // and the scene letterbox bars (rows 0–119 and 840–959, all 1280
        // columns) land on the original's pixels; the earlier shift the
        // other way put them one pixel up and left (bars 0–118, the last
        // column uncovered).
        //
        // Where the canvas goes on the target (`display::UiFrame`): the
        // whole target, stretched, as in the game
        // (`AspectMode::Stretched`); else (agreed deviation) each quad at
        // its 4:3 size in the centred 4:3 box, moved towards the edge its
        // element belongs to (`Quad::place`, set by `frontend::layout`).
        // What lies off the canvas (off screen in the game) stays hidden:
        // each run is scissored to the canvas as its place moves it —
        // except fills (flat, or a small texture magnified: dims, fades,
        // letterbox bars, backings) spanning the canvas or touching their
        // anchored edge, which reach the target's edges (below).
        let frame = crate::render::display::UiFrame::new(size);
        // Clone (screens taller than 960 lines; native at 1280 × 960 and
        // below): hairlines grow to round(height / 960) pixels so they
        // scale with the screen. Every texture, glyphs included, is
        // sampled as fe.efx samples it (bilinear, wrap; see the sampler in
        // `UiRenderer::new`) at any size; only HD UI Art pack art drawn
        // magnified goes through `sharp` (below).

        let hairline = (frame.scale.0.min(frame.scale.1) * 0.5).round().max(1.0);
        let (tw, th) = (size.0.max(1) as f32, size.1.max(1) as f32);
        let half = glam::Vec2::new(0.5 / frame.scale.0, 0.5 / frame.scale.1);
        let to_clip = |p: glam::Vec2, place: crate::render::display::Place| {
            let p = frame.to_pixels(p + half, place);
            [(p.x / tw) * 2.0 - 1.0, 1.0 - (p.y / th) * 2.0]
        };
        let boxed =
            frame.origin != (0.0, 0.0) || frame.scale.0 * CANVAS.x < tw - 0.5 || frame.scale.1 * CANVAS.y < th - 0.5;
        let whole_target = (0, 0, size.0.max(1), size.1.max(1));
        let mut vertices = Vec::with_capacity(quads.len() * 6 + 12);
        let mut runs: Vec<(Option<u32>, usize, u32, u32, (u32, u32, u32, u32))> = Vec::new();
        // Each quad's blend and alpha test (**verified**): 0x5BCC40 fills
        // the BLENDSTATE array 0x86EAD8 = {alpha test on (entry byte 0x55
        // == 1), ref 95, blending on (byte 0x56 ≠ 0), src, dst} (0x5BCB80,
        // 0x5BCBD0) and hands it to fe.efx, whose pass binds its five
        // entries to D3DX states 4 / 10 / 13 / 6 / 7 (AlphaTestEnable,
        // AlphaRef, AlphaBlendEnable, SrcBlend, DestBlend; pass at blob
        // 0xB8C..0xBCC). ALPHAFUNC is the device's GREATER (5, set once at
        // 0x5C2480; the effect does not set it). So byte 0 draws unblended
        // and an alpha-tested texture keeps only alpha > 95 / 255: the
        // dialogs' DXT1 frame (U2_HIGHLIGHT_BOX, byte 0, test on) shows
        // under Wine as 121,194,194,194,121 across its edge, not the
        // alpha-blended 7,75,192,192,192,75,7 the clone drew. Flat colours
        // and rendered textures blend by alpha, untested.
        use crate::formats::texture::Blend;
        // Each quad's texture, looked up once (`tex_of`).
        let tex_of = |q: &Quad| q.texture.and_then(|h| assets.textures.get(&h));
        let blend_of = |t: Option<&crate::formats::texture::Texture>| match t.map(|t| t.blend) {
            Some(Blend::Additive) => 1,
            Some(Blend::DestAlpha) => 2,
            Some(Blend::DestAlphaAdditive) => 3,
            Some(Blend::None) => 5,
            _ => 0,
        };
        let tested = |t: Option<&crate::formats::texture::Texture>| t.is_some_and(|t| t.alpha_test);
        // Destination alpha: the frame function ends (0x5CBA9F..0x5CBAE6)
        // with COLORWRITEENABLE 8 (alpha alone) and a full-screen quad of
        // colour 0 (0x5C7150), so the front end draws over alpha 0 and its
        // masks (additive, black, e.g. Minimap_Mask, NOS_Mask) leave their
        // alpha for the destination-alpha textures after them. The clone
        // clears the alpha so only when such a texture is drawn.
        let masked = quads.iter().any(|q| matches!(blend_of(tex_of(q)), 2 | 3));
        // The same alpha 0 without the full-screen pass: a cleared target is
        // cleared to it (black, alpha 0, what the clear and the quad left),
        // and the post passes' last pass writes it when told to
        // (`alpha.zeroed`).
        let clear_colour = alpha_prelude(masked, clear, alpha, &mut vertices, &mut runs, whole_target);
        scene_bar_bands(size, tw, &mut vertices, &mut runs, whole_target);
        // NFSU2_QUAD_DUMP: every quad with its texture's blend, format and
        // alpha test on stderr (a debugging aid for the pixel comparisons).
        let dump = *crate::render::debug_env::once::quad_dump();
        for (qi, q) in quads.iter().enumerate() {
            let qt = tex_of(q);
            if dump {
                let t = qt;
                eprintln!(
                    "quad {:?} {:?} {:?} at{:?} {}x{} rect {:?} uv {:?} col {:?} ang {} place {:?}",
                    t.map(|t| t.name.as_str()),
                    t.map(|t| t.blend),
                    t.map(|t| t.format),
                    t.map(|t| t.alpha_test),
                    t.map_or(0, |t| t.width),
                    t.map_or(0, |t| t.height),
                    q.rect,
                    q.uv,
                    q.color,
                    q.angle,
                    q.place
                );
            }
            let (pieces, n, place, flat, spans) = quad_pieces(q, qt, &frame, boxed, hairline);
            let local = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
            let start = vertices.len() as u32;
            // HD UI Art (clone, opt-in): pack art that is not a fill is
            // sampled sharp-bilinear (ui.wgsl `sharp`) wherever it is drawn
            // magnified on both axes; alpha-blended and additive art
            // filters premultiplied. Everything else, and everything with
            // HD UI Art off, samples as fe.efx does.
            let blend = blend_of(qt);
            let mut mode = if tested(qt) { 2.0 } else { 0.0 };
            if !flat && q.texture.is_some_and(|h| self.hd.contains(&h)) {
                mode += if matches!(blend, 0 | 1) { SHARP_PREMULTIPLIED } else { SHARP };
            }
            // HUD colour-vision palette (`set_hud_palette`).
            let palette = hud_palette_of(qi);
            mode += 16.0 * f32::from(palette);
            for corners in &pieces[..n] {
                for i in [0, 1, 2, 0, 2, 3] {
                    let (p, uv) = corners[i];
                    vertices.push(Vertex {
                        position: to_clip(p, place),
                        uv: uv.into(),
                        color: q.color,
                        clip: [local[i][0], local[i][1], mode],
                    });
                }
            }
            let count = 6 * n as u32;
            let tex = q.texture.filter(|h| self.textures.contains_key(h));
            let blend = if palette != 0 && blend < 4 { blend + PALETTE_PIPELINES } else { blend };
            let scissor = if spans || !boxed { whole_target } else { frame.scissor(place) };
            #[cfg(feature = "dev-harness")]
            super::geom_dump::quad(qi, q, qt.map(|t| t.name.as_str()), flat, &vertices[start as usize..], scissor, size);
            match runs.last_mut() {
                Some(r) if r.0 == tex && r.1 == blend && r.4 == scissor => r.3 += count,
                _ => runs.push((tex, blend, start, count, scissor)),
            }
        }
        // Alpha back to 1 (the blends above lower it), so the frame stays
        // opaque.
        // Not needed where nothing reads the alpha: a swap chain presented
        // opaque (`alpha.opaque`).
        if !quads.is_empty() && !alpha.opaque {
            let start = vertices.len() as u32;
            full(&mut vertices, 1.0);
            runs.push((None, 4, start, 6, whole_target));
        }
        #[cfg(feature = "dev-harness")]
        super::geom_dump::finish(&frame, size);
        self.encode_runs(gpu, target, clear, clear_colour, boxed, vertices, runs)
    }

    /// [`UiRenderer::record_encoder`]'s last part: the vertices uploaded, the
    /// runs encoded into one pass over `target`.
    #[allow(clippy::too_many_arguments)]
    fn encode_runs(
        &mut self,
        gpu: &Gpu,
        target: &wgpu::TextureView,
        clear: bool,
        clear_colour: wgpu::Color,
        boxed: bool,
        vertices: Vec<Vertex>,
        runs: Vec<Run>,
    ) -> wgpu::CommandEncoder {
        let bytes: &[u8] = bytemuck::cast_slice(&vertices);
        if !bytes.is_empty() && self.vertices.as_ref().is_none_or(|b| b.size() < bytes.len() as u64) {
            self.vertices = Some(gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ui vertices"),
                size: (bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let buffer = (!bytes.is_empty()).then(|| {
            let b = self.vertices.as_ref().unwrap();
            gpu.queue.write_buffer(b, 0, bytes);
            b
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        crate::prof::gpu::mark(&mut encoder, crate::prof::gpu::GAP);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if clear { wgpu::LoadOp::Clear(clear_colour) } else { wgpu::LoadOp::Load },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(buffer) = buffer {
                pass.set_vertex_buffer(0, buffer.slice(..bytes.len() as u64));
                for (tex, blend, start, count, scissor) in runs {
                    if scissor.2 == 0 || scissor.3 == 0 {
                        continue;
                    }
                    if boxed {
                        pass.set_scissor_rect(scissor.0, scissor.1, scissor.2, scissor.3);
                    }
                    pass.set_pipeline(&self.pipelines[blend]);
                    let group = tex.and_then(|h| self.textures.get(&h)).unwrap_or(&self.white);
                    pass.set_bind_group(0, group, &[]);
                    pass.draw(start..start + count, 0..1);
                }
            }
        }
        crate::prof::gpu::mark(&mut encoder, "ui");
        encoder
    }
}

/// A run of quads drawn together: (texture, blend, first vertex, vertex
/// count, scissor).
type Run = (Option<u32>, usize, u32, u32, (u32, u32, u32, u32));
/// A quad's four corners: (canvas position, uv).
type Corners = [(glam::Vec2, glam::Vec2); 4];

/// A full-screen quad of colour 0 and alpha `a`.
fn full(vertices: &mut Vec<Vertex>, a: f32) {
        for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            vertices.push(Vertex {
                position: [x, y],
                uv: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, a],
                clip: [0.0, 0.0, 0.0],
            });
        }
}

/// Destination alpha's prelude: the alpha cleared (the full-screen pass, when
/// not cleared otherwise). -> the target's clear colour.
fn alpha_prelude(
    masked: bool,
    clear: bool,
    alpha: Alpha,
    vertices: &mut Vec<Vertex>,
    runs: &mut Vec<Run>,
    whole_target: (u32, u32, u32, u32),
) -> wgpu::Color {
    let clear_colour = if masked { wgpu::Color::TRANSPARENT } else { wgpu::Color::BLACK };
    if masked && (clear || alpha.zeroed) {
        // Listed among the bench's parts: the frames that took this way.
        let _counted = crate::prof::acc::span("ui: alpha 0 without its pass");
    }
    if masked && !clear && !alpha.zeroed {
        full(vertices, 0.0);
        runs.push((None, 4, 0, 6, whole_target));
    }
    clear_colour
}

/// The side bars of a front-end 3D scene capped at 21:9.
fn scene_bar_bands(
    size: (u32, u32),
    tw: f32,
    vertices: &mut Vec<Vertex>,
    runs: &mut Vec<Run>,
    whole_target: (u32, u32, u32, u32),
) {
    // A front-end 3D scene capped at 21:9 on a wider screen
    // (`display::scene_bars`): black bars at the sides, fading to
    // clear over their inner edge, under every quad.
    if crate::render::display::scene_capped()
        && let Some((x, fade)) = crate::render::display::scene_bars(size)
    {
        let start = vertices.len() as u32;
        let cx = |px: f32| (px / tw) * 2.0 - 1.0;
        let mut band = |x0: f32, x1: f32, a0: f32, a1: f32| {
            let v = |x: f32, y: f32, a: f32| Vertex {
                position: [cx(x), y],
                uv: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, a],
                clip: [0.0, 0.0, 0.0],
            };
            for (px, y, a) in
                [(x0, 1.0, a0), (x1, 1.0, a1), (x1, -1.0, a1), (x0, 1.0, a0), (x1, -1.0, a1), (x0, -1.0, a0)]
            {
                vertices.push(v(px, y, a));
            }
        };
        band(0.0, x - fade, 1.0, 1.0);
        band(x - fade, x, 1.0, 0.0);
        band(tw - x, tw - x + fade, 0.0, 1.0);
        band(tw - x + fade, tw, 1.0, 1.0);
        runs.push((None, 0, start, vertices.len() as u32 - start, whole_target));
    }
}

/// A quad's pieces on the canvas (the quad and its wide-screen
/// continuations, at most 3 × 3). -> (pieces, their count, its place, a
/// fill, spans the target).
fn quad_pieces(
    q: &Quad,
    qt: Option<&crate::formats::texture::Texture>,
    frame: &crate::render::display::UiFrame,
    boxed: bool,
    hairline: f32,
) -> ([Corners; 9], usize, crate::render::display::Place, bool, bool) {
        let [a, b] = q.rect;
        let [ua, ub] = q.uv;
        let mut corners = [
            (glam::Vec2::new(a.x, a.y), glam::Vec2::new(ua.x, ua.y)),
            (glam::Vec2::new(b.x, a.y), glam::Vec2::new(ub.x, ua.y)),
            (glam::Vec2::new(b.x, b.y), glam::Vec2::new(ub.x, ub.y)),
            (glam::Vec2::new(a.x, b.y), glam::Vec2::new(ua.x, ub.y)),
        ];
        // Half-texel inset (**verified**, 0x5C5030, the draw every
        // front-end polygon goes through via 0x51A5D0 → 0x5C7EB0 with a
        // mode of 0 or 1, never 0x61, the one mode that skips it): with
        // a texture, by 0.5 / the entry's width (+0x44) and height
        // (+0x46), vertex 0 (upper left) u + du, v + dv; 1 (upper right)
        // u − du, v + dv; 2 (lower right) u − du, v − dv; 3 (lower
        // left) u + du, v − dv. It goes by vertex, so a mirrored UV
        // range is pushed outward, as in the game.
        if let Some(t) = qt {
            let d = glam::Vec2::new(0.5 / t.width.max(1) as f32, 0.5 / t.height.max(1) as f32);
            for ((_, uv), s) in corners.iter_mut().zip([[1.0, 1.0], [-1.0, 1.0], [-1.0, -1.0], [1.0, -1.0]]) {
                *uv += d * glam::Vec2::from(s);
            }
        }
        // A *fill* (see the wide-screen continuation below): flat
        // (untextured, or `Place::fill`) or its texture magnified at
        // least `FILL_MAGNIFY` times on both axes.
        let flat = q.texture.is_none()
            || q.place.fill
            || qt.is_some_and(|t| {
                let texels = (ub - ua).abs() * glam::Vec2::new(t.width as f32, t.height as f32);
                let size = (b - a).abs();
                size.x >= texels.x * FILL_MAGNIFY && size.y >= texels.y * FILL_MAGNIFY
            });
        if q.angle != 0.0 {
            // FEng turns by the row-vector matrix [[cos, sin], [−sin,
            // cos]] (0x51CEE0) on a y-down canvas: a positive angle is
            // clockwise on screen (UI_Sponsorship_new's −90° arrow at
            // the top of its scroll bar points up).
            let c = (a + b) * 0.5;
            let rot = glam::Mat2::from_angle(q.angle);
            for (p, _) in &mut corners {
                *p = c + rot * (*p - c);
            }
        }
        // Full-canvas overlays and side backings on a boxed canvas
        // (clone deviation, wide screens; docs/formats.md "Display modes
        // and aspect"). A *fill* is an unturned quad that is flat
        // (untextured but not a hairline, or `Place::fill`) or magnifies
        // its texture at least `FILL_MAGNIFY` times on both axes (a dim,
        // a backing, a gradient: U2_Menu_Box, White16x16, never a
        // picture such as PC_load.tga or a movie frame). A fill that
        // spans the canvas's width or height, or that touches the
        // canvas edge its element is anchored to, is not clipped to the
        // canvas, and each side of it on a canvas edge it touches goes
        // on to the target's edge, continued with its edge texels (its
        // own picture inside stays as it is): fades, dims, letterbox
        // bars (and the top / bottom at 5:4), the help bar's backing.
        let mut place = q.place;
        if place.stretch && q.angle == 0.0 {
            // Both ends with their edges (`Place::stretch`).
            let d = frame.reach_x(place) / frame.scale.0;
            let lo_x = a.x.min(b.x);
            for (p, _) in &mut corners {
                p.x += if p.x <= lo_x { -d } else { d };
            }
            place.x = 0;
        }
    let (mut pieces, mut n, spans) = fill_continuation(frame, place, &corners, boxed, flat, q);
    hairline_corners(q, frame, hairline, &mut corners);
        if n == 0 {
            pieces[0] = corners;
            n = 1;
        }
    (pieces, n, place, flat, spans)
}

/// A fill's continuations to the target's edges. -> (pieces, their count
/// (0: none), spans the target).
fn fill_continuation(
    frame: &crate::render::display::UiFrame,
    place: crate::render::display::Place,
    corners: &Corners,
    boxed: bool,
    flat: bool,
    q: &Quad,
) -> ([Corners; 9], usize, bool) {
        // At most 3 × 3 pieces: the quad and its continuations.
        let mut pieces = [[(glam::Vec2::ZERO, glam::Vec2::ZERO); 4]; 9];
        let mut n = 0;
        let mut spans = place.stretch;
        if boxed && flat && q.angle == 0.0 && q.uv != crate::draw::quad::HAIRLINE_UV {
            let edges = frame.edges(place);
            let (c0, c2) = (corners[0], corners[2]);
            let (lo, hi) = (c0.0.min(c2.0), c0.0.max(c2.0));
            let across = lo.x <= 0.0 && hi.x >= CANVAS.x;
            let down = lo.y <= 0.0 && hi.y >= CANVAS.y;
            let wide = across || down;
            let ext = [
                lo.x <= 0.0 && (wide || place.x < 0),
                hi.x >= CANVAS.x && (wide || place.x > 0),
                lo.y <= 0.0 && (wide || place.y < 0),
                hi.y >= CANVAS.y && (wide || place.y > 0),
            ];
            if wide || ext.iter().any(|&e| e) {
                spans = true;
                // Per axis: (from, to, uv from, uv to) pieces, in order.
                let axis = |p0: f32, t0: f32, p1: f32, t1: f32, e_lo: bool, e_hi: bool, s_lo: f32, s_hi: f32| {
                    let ((lo, tl), (hi, th)) = if p0 <= p1 { ((p0, t0), (p1, t1)) } else { ((p1, t1), (p0, t0)) };
                    let mut v = Vec::with_capacity(3);
                    if e_lo && s_lo < lo {
                        v.push((s_lo, lo, tl, tl));
                    }
                    v.push((lo, hi, tl, th));
                    if e_hi && s_hi > hi {
                        v.push((hi, s_hi, th, th));
                    }
                    v
                };
                let xs = axis(c0.0.x, c0.1.x, c2.0.x, c2.1.x, ext[0], ext[1], edges.0.x, edges.1.x);
                let ys = axis(c0.0.y, c0.1.y, c2.0.y, c2.1.y, ext[2], ext[3], edges.0.y, edges.1.y);
                if xs.len() * ys.len() > 1 {
                    for &(x0, x1, u0, u1) in &xs {
                        for &(y0, y1, v0, v1) in &ys {
                            let c = |x: f32, y: f32, u: f32, v: f32| (glam::Vec2::new(x, y), glam::Vec2::new(u, v));
                            pieces[n] =
                                [c(x0, y0, u0, v0), c(x1, y0, u1, v0), c(x1, y1, u1, v1), c(x0, y1, u0, v1)];
                            n += 1;
                        }
                    }
                }
            }
        }
    (pieces, n, spans)
}

/// A hairline's corners: a line one target pixel wide (`hairline` pixels
/// on tall screens) along the rectangle's middle.
fn hairline_corners(q: &Quad, frame: &crate::render::display::UiFrame, hairline: f32, corners: &mut Corners) {
        // A hairline (no texture, [`crate::draw::quad::HAIRLINE_UV`]) is a
        // line one target pixel wide along the rectangle's middle:
        // the dyno graph's lines (0x50DCD0 → 0x5C5760) are a
        // DrawPrimitiveUP line strip (type 3, one line), one pixel at
        // any resolution (**verified**: 1 px at 1280 × 960 under Wine,
        // menus114 ch14). A quad one pixel across stands in for the
        // line's rasterisation (diamond rule; tiny differences at
        // the ends).
        if q.texture.is_none() && q.uv == crate::draw::quad::HAIRLINE_UV {
            let scale = glam::Vec2::new(frame.scale.0, frame.scale.1);
            let (p0, p1) = ((corners[0].0 + corners[3].0) * 0.5, (corners[1].0 + corners[2].0) * 0.5);
            let d = (p1 - p0) * scale;
            let n = glam::Vec2::new(-d.y, d.x).normalize_or_zero() * 0.5 * hairline / scale;
            corners[0].0 = p0 - n;
            corners[1].0 = p1 - n;
            corners[2].0 = p1 + n;
            corners[3].0 = p0 + n;
        }
}
