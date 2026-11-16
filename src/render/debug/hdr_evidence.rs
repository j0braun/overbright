//! HDR Output evidence: preview, readback, f16 decode (moved from hdr_output.rs).

use crate::render::Gpu;
use crate::render::hdr_output::{Calibration, Signal, pq_decode};

/// A tone-mapped SDR preview of HDR light (for the evidence images; not
/// used in the game): light up to 0.8 × paper white as SDR shows it, the
/// range above compressed into the last 20 % (white at the peak), on the
/// max channel; then the transfer's inverse at 8 bits.
pub fn preview(nits: [f32; 3], cal: &Calibration) -> [u8; 3] {
    let rel = nits.map(|n| n.max(0.0) / cal.paper);
    let m = rel[0].max(rel[1]).max(rel[2]);
    let h = (cal.peak / cal.paper).max(1.0);
    let scale = if m > 0.8 {
        let x = m - 0.8;
        (0.8 + 0.2 * (1.0 - (-3.0 * x / (h - 0.8).max(0.2)).exp())) / m
    } else {
        1.0
    };
    let inv = 1.0 / cal.transfer.exponent();
    rel.map(|v| ((v * scale).clamp(0.0, 1.0).powf(inv) * 255.0).round() as u8)
}

/// BT.2020 to BT.709 (the inverse of [`BT709_TO_BT2020`]).
pub const BT2020_TO_BT709: [[f32; 3]; 3] = [
    [1.660_491, -0.587_641_1, -0.072_849_9],
    [-0.124_550_5, 1.132_899_9, -0.008_349_4],
    [-0.018_150_8, -0.100_578_9, 1.118_729_7],
];

/// A half float's value.
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = i32::from((h >> 10) & 0x1F);
    let m = f32::from(h & 0x3FF);
    sign * match e {
        0 => m * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

/// Decodes one texel of `signal`'s format to light (nits, BT.709).
pub fn decode(texel: &[u8], signal: Signal) -> [f32; 3] {
    match signal {
        Signal::Pq => {
            let v = u32::from_le_bytes([texel[0], texel[1], texel[2], texel[3]]);
            let c = [v & 0x3FF, (v >> 10) & 0x3FF, (v >> 20) & 0x3FF].map(|x| pq_decode(x as f32 / 1023.0));
            let m = BT2020_TO_BT709;
            [0, 1, 2].map(|i| m[i][0] * c[0] + m[i][1] * c[1] + m[i][2] * c[2])
        }
        Signal::ScRgb => [0, 1, 2].map(|i| f16_to_f32(u16::from_le_bytes([texel[2 * i], texel[2 * i + 1]])) * 80.0),
    }
}

/// Reads an HDR target of `signal`'s format back as light (nits, BT.709).
pub fn read_nits(gpu: &Gpu, target: &wgpu::Texture, signal: Signal, (width, height): (u32, u32)) -> Vec<[f32; 3]> {
    let texel = signal.format().block_copy_size(None).unwrap_or(4);
    let bytes_per_row = (width * texel).div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("hdr readback"),
        size: u64::from(bytes_per_row * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: None },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    let mapped = slice.get_mapped_range().expect("mapped hdr readback");
    let mut out = Vec::with_capacity((width * height) as usize);
    for row in mapped.chunks(bytes_per_row as usize).take(height as usize) {
        for t in row[..(width * texel) as usize].chunks_exact(texel as usize) {
            out.push(decode(t, signal));
        }
    }
    out
}
