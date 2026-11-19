use crate::*;

/// One frame of the loading screen `loading` at `now` (ms), presented
/// at once (the game's loop keeps drawing the front end during a track
/// load).
/// How long a track load runs before PC_Loading goes up (ms). The original
/// shows it for every track load; the clone skips it for a load that is
/// over sooner (a world kept in memory, a cached load): an agreed
/// deviation (docs/playing/native-deviations.md "Loading screens"; docs/formats.md, "Track loads and their loading screen").
pub(crate) const LOADING_DELAY_MS: f32 = 200.0;

/// PC_Loading over a load in progress, drawn once the load has run
/// [`LOADING_DELAY_MS`]; its animation starts from its first frame.
pub(crate) struct LoadingGate {
    pub(crate) loading: Option<screens::Loading>,
    pub(crate) shown: bool,
    pub(crate) since: Instant,
}

/// Boot loading screen (docs/original/loading-image.md): PC_Loading over the game start. The
/// original's WinMain makes the front end's load its first load
/// (0x580E6C: the load handler 0x865484 = 0x57FF20), and that handler
/// opens PC_Loading.fng (0x57FF95 → 0x555E90, unless 0x52CF70 finds it
/// up) before it reads Global\FrontEndMemoryFile.bin; the boot screens
/// (0x4D9580) come after. The clone follows [`LOADING_DELAY_MS`] here too
/// (every loading screen, the start's included): the screen goes up only once the start's work still running
/// with the window open (`resumed`: window, audio device) has taken
/// 200 ms, and a start done sooner presents no PC_Loading frame at all.
/// `enh.boot_loading_min_ms` (opt-in; 0 = the 200 ms rule) instead shows
/// it from the window's first frame for at least that long.
pub(crate) struct BootLoading {
    pub(crate) loading: screens::Loading,
    pub(crate) since: Instant,
    /// The start's work is done (`resumed`).
    pub(crate) ready: bool,
    /// A PC_Loading frame was presented.
    pub(crate) shown: bool,
}

impl BootLoading {
    pub(crate) fn new(now: f32) -> BootLoading {
        BootLoading {
            loading: screens::Loading { package: screens::Loading::PC, opened: now },
            since: Instant::now(),
            ready: false,
            shown: false,
        }
    }

    /// Whether it comes down now: the start's work done and the minimum
    /// time (`min_ms`) run.
    pub(crate) fn done(&self, min_ms: u16) -> bool {
        self.ready && self.since.elapsed() >= std::time::Duration::from_millis(min_ms.into())
    }

    /// The PC_Loading frame to present at `now` (ms), if any: none once
    /// it is [`Self::done`], none while the start has run under
    /// [`LOADING_DELAY_MS`] (unless `min_ms` asks for the screen); its
    /// animation starts from the first frame presented.
    pub(crate) fn frame(&mut self, now: f32, min_ms: u16) -> Option<&screens::Loading> {
        if self.done(min_ms) || (min_ms == 0 && self.since.elapsed().as_secs_f32() * 1000.0 < LOADING_DELAY_MS) {
            return None;
        }
        if !self.shown {
            self.loading.opened = now;
            self.shown = true;
        }
        Some(&self.loading)
    }
}

/// The load's time for the performance overlay ("Last load").
impl Drop for LoadingGate {
    fn drop(&mut self) {
        nfsu2::overlay::record_load(self.since.elapsed().as_secs_f32() * 1000.0);
    }
}

impl LoadingGate {
    pub(crate) fn new(loading: Option<screens::Loading>) -> LoadingGate {
        LoadingGate { loading, shown: false, since: Instant::now() }
    }

    pub(crate) fn tick(&mut self, window: &mut Option<WindowState>, assets: &Assets, now: f32) {
        let Some(l) = self.loading.as_mut() else { return };
        if !self.shown {
            if self.since.elapsed().as_secs_f32() * 1000.0 < LOADING_DELAY_MS {
                return;
            }
            l.opened = now;
            self.shown = true;
        }
        present_loading(window, assets, l, now);
    }
}

pub(crate) fn present_loading(window: &mut Option<WindowState>, assets: &Assets, loading: &screens::Loading, now: f32) {
    let quads = loading.quads(assets, now);
    let Some(w) = window.as_mut() else { return };
    w.settle();
    let Some(surface) = w.surface.as_ref() else { return };
    let frame = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        _ => return,
    };
    let view = frame.texture.create_view(&Default::default());
    let size = (w.config.width, w.config.height);
    let gpu = w.gpu.clone();
    // HDR Output: the swap chain is in the signal's format; the screen is
    // drawn into the SDR frame texture and the display transform writes
    // the swap chain image (as the game's frames, `render::frame`).
    if let Some(sdr) = w.hdr_frame(size) {
        w.ui.render(&gpu, assets, &sdr, size, &quads, true);
        let h = w.hdr.as_mut().expect("hdr_frame comes with HDR Output on");
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        h.transform.run(&gpu, &mut encoder, &sdr, None, &view, h.signal, &nfsu2::qol::hdr_calibration(), size);
        gpu.queue.submit([encoder.finish()]);
    } else {
        w.ui.render(&gpu, assets, &view, size, &quads, true);
    }
    gpu.queue.present(frame);
}

/// The HD UI pack's first HUD frame: the UI
/// textures `textures` bound now, at the load, as the HUD's first frame
/// would bind them (`UiRenderer::render` binds each quad's texture: the HD
/// UI pack's art read, decoded and uploaded), drawn into a throwaway 1×1
/// target. The same objects the frame would make; nothing on screen changes.
pub(crate) fn prefetch_ui_textures(window: &mut Option<WindowState>, assets: &Assets, textures: &[u32]) {
    let Some(w) = window.as_mut() else { return };
    if textures.is_empty() {
        return;
    }
    let target = w.gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ui prefetch"),
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: w.config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let quads: Vec<Quad> = textures
        .iter()
        .map(|&h| Quad {
            texture: Some(h),
            rect: [Vec2::ZERO, Vec2::ONE],
            uv: [Vec2::ZERO, Vec2::ONE],
            color: [0.0; 4],
            z: 0.0,
            angle: 0.0,
            place: Default::default(),
        })
        .collect();
    w.ui.render(&w.gpu, assets, &view, (1, 1), &quads, true);
}

pub(crate) fn now_ms(started: Instant) -> f32 {
    started.elapsed().as_secs_f32() * 1000.0
}
