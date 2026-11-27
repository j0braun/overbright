//! The HDR Output values the settings store holds and the display transform reads (the signal, the SDR
//! transfer, the calibration); the transform itself is `render::hdr_output`.

/// The HDR signal of the swap chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// BT.2100 PQ, BT.2020 primaries, 10 bits (HDR10).
    Pq,
    /// Extended linear BT.709, half float, 1.0 = 80 nits.
    ScRgb,
}

impl Signal {
    pub fn format(self) -> wgpu::TextureFormat {
        match self {
            Signal::Pq => wgpu::TextureFormat::Rgb10a2Unorm,
            Signal::ScRgb => wgpu::TextureFormat::Rgba16Float,
        }
    }

    pub fn color_space(self) -> wgpu::SurfaceColorSpace {
        match self {
            Signal::Pq => wgpu::SurfaceColorSpace::Bt2100Pq,
            Signal::ScRgb => wgpu::SurfaceColorSpace::ExtendedSrgbLinear,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Signal::Pq => "HDR10 PQ",
            Signal::ScRgb => "scRGB",
        }
    }

    /// `NFSU2_HDR_SIGNAL` / `NFSU2_HDR_FAKE` values.
    pub fn parse(s: &str) -> Option<Signal> {
        match s {
            "pq" | "hdr10" => Some(Signal::Pq),
            "scrgb" => Some(Signal::ScRgb),
            _ => None,
        }
    }
}

/// The SDR transfer the frame is shown with (the calibration's gamma).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    /// Pure gamma 2.2 (what most SDR monitors show; default).
    Gamma22,
    /// The sRGB piecewise curve (Windows' SDR-in-HDR mapping).
    Srgb,
    /// Gamma 2.4 (BT.1886, dim rooms).
    Gamma24,
}

impl Transfer {
    pub fn from_index(i: u8) -> Transfer {
        match i {
            1 => Transfer::Srgb,
            2 => Transfer::Gamma24,
            _ => Transfer::Gamma22,
        }
    }

    pub fn index(self) -> f32 {
        match self {
            Transfer::Gamma22 => 0.0,
            Transfer::Srgb => 1.0,
            Transfer::Gamma24 => 2.0,
        }
    }

    /// SDR signal to light relative to paper white (CPU reference of
    /// hdr_output.wgsl `eotf1`).
    pub fn eotf(self, v: f32) -> f32 {
        let x = v.max(0.0);
        match self {
            Transfer::Gamma22 => x.powf(2.2),
            Transfer::Gamma24 => x.powf(2.4),
            Transfer::Srgb => {
                if x <= 0.04045 {
                    x / 12.92
                } else {
                    ((x + 0.055) / 1.055).powf(2.4)
                }
            }
        }
    }

    /// The power law used above white.
    pub fn exponent(self) -> f32 {
        if self == Transfer::Gamma24 { 2.4 } else { 2.2 }
    }
}

/// The calibration (Display page: Peak Brightness, Paper White, SDR Gamma).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Calibration {
    pub peak: f32,
    pub paper: f32,
    pub transfer: Transfer,
    /// The calibration pattern's rectangle (fractions of the output: left,
    /// top, right, bottom) while the HDR Calibration page is open.
    pub pattern: Option<[f32; 4]>,
}

/// BT.2408's reference white and a common HDR10 peak, the defaults when the
/// display reports nothing.
pub const DEFAULT_PAPER: f32 = 203.0;
pub const DEFAULT_PEAK: f32 = 1000.0;

impl Default for Calibration {
    fn default() -> Calibration {
        Calibration { peak: DEFAULT_PEAK, paper: DEFAULT_PAPER, transfer: Transfer::Gamma22, pattern: None }
    }
}
