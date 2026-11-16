//! HDR Output in headless shots: the faked HDR surface and the HDR
//! readback of a shot's frame.
use crate::*;

/// `NFSU2_HDR_FAKE=pq|scrgb`: headless runs act as on a surface offering
/// that HDR signal (HDR Output's path without an HDR display).
pub(crate) fn fake_hdr() {
    if let Some(signal) =
        std::env::var("NFSU2_HDR_FAKE").ok().as_deref().and_then(nfsu2::render::hdr_output::Signal::parse)
    {
        nfsu2::qol::set_hdr_output_support(Some(signal));
    }
    if std::env::var("NFSU2_HDR_PATTERN").is_ok_and(|v| v == "1") {
        nfsu2::qol::set_hdr_pattern(true);
    }
}

/// The display transform of a shot's frame `sdr` into `signal`, read back
/// and written next to `out` (see `shot`).
pub(crate) fn hdr_shot(
    gpu: &Gpu,
    sdr: &wgpu::TextureView,
    unclipped: Option<&wgpu::TextureView>,
    signal: nfsu2::render::hdr_output::Signal,
    (width, height): (u32, u32),
    out: &str,
) -> Result<()> {
    use nfsu2::render::hdr_output::{self, Signal};
    let cal = nfsu2::qol::hdr_calibration();
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hdr shot"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: signal.format(),
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut t = hdr_output::DisplayTransform::new(gpu);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    t.run(gpu, &mut encoder, sdr, unclipped, &target.create_view(&Default::default()), signal, &cal, (width, height));
    gpu.queue.submit([encoder.finish()]);
    let nits = nfsu2::render::hdr_output::read_nits(gpu, &target, signal, (width, height));
    let pq16: Vec<u16> = nits
        .iter()
        .flat_map(|&c| {
            hdr_output::bt709_to_bt2020(c).map(|n| (hdr_output::pq_encode(n.max(0.0)) * 65535.0).round() as u16)
        })
        .collect();
    let base = out.strip_suffix(".png").unwrap_or(out);
    let bytes: Vec<u8> = pq16.iter().flat_map(|v| v.to_ne_bytes()).collect();
    image::save_buffer(format!("{base}.hdr.png"), &bytes, width, height, image::ColorType::Rgb16)?;
    let preview: Vec<u8> = nits.iter().flat_map(|&c| hdr_output::preview(c, &cal)).collect();
    image::save_buffer(format!("{base}.hdr-preview.png"), &preview, width, height, image::ColorType::Rgb8)?;
    let peak = nits.iter().map(|c| c[0].max(c[1]).max(c[2])).fold(0.0f32, f32::max);
    let above = nits.iter().filter(|c| c[0].max(c[1]).max(c[2]) > cal.paper * 1.01).count();
    println!(
        "wrote {base}.hdr.png ({}, paper {} nits, peak setting {} nits): brightest {peak:.0} nits, {above} pixels above paper white ({:.2} %)",
        if signal == Signal::Pq { "HDR10 PQ" } else { "scRGB, stored as PQ" },
        cal.paper,
        cal.peak,
        above as f64 * 100.0 / (width * height) as f64
    );
    Ok(())
}
