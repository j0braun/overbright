//! The swap chain's format for the SDR frame.
//!
//! Every renderer draws into the window's format (`config.format`) and
//! writes display-encoded values (the game does its colour maths in gamma
//! space, as the original's 8-bit back buffer), so the surface must be a
//! non-sRGB format the compositor shows as sRGB. The surface's own list is
//! in the driver's preference order, which on a colour-managed Wayland
//! desktop can start with a 16-bit format (16-bit float there is
//! configured as extended linear sRGB by default, which would show the
//! encoded values as light). The game therefore picks its format itself:
//! 8-bit first, then 10-bit, 16-bit float only when nothing narrower is
//! offered. HDR Output configures its own format and colour space on top
//! (`hdr_output::Signal`, docs/hdr.md); this is the SDR frame's.

/// The preference order: 8-bit BGRA / RGBA, 10-bit, 16-bit float (16-bit
/// unorm is no render target without a device feature the game does not
/// ask for).
pub const SDR_FORMAT_ORDER: [wgpu::TextureFormat; 4] = ORDER;
const ORDER: [wgpu::TextureFormat; 4] = [
    wgpu::TextureFormat::Bgra8Unorm,
    wgpu::TextureFormat::Rgba8Unorm,
    wgpu::TextureFormat::Rgb10a2Unorm,
    wgpu::TextureFormat::Rgba16Float,
];

/// The SDR frame's swap chain format and colour space from the surface's
/// capabilities: the first of [`ORDER`] the surface offers in the sRGB
/// colour space, else its first non-sRGB format, else its first format
/// (a surface offering only sRGB formats: the values are encoded once
/// more, as before this choice existed). The colour space is `Auto` (sRGB
/// for every format but 16-bit float) except for a 16-bit float format,
/// which is asked for as sRGB explicitly. None: the surface offers no
/// format (not supported by the adapter).
pub fn sdr_surface_format(caps: &wgpu::SurfaceCapabilities) -> Option<(wgpu::TextureFormat, wgpu::SurfaceColorSpace)> {
    let srgb = |f: wgpu::TextureFormat| {
        // `formats` holds only formats `Auto` can configure; the per-format
        // colour spaces, where reported, must include sRGB.
        caps.format_capabilities.iter().all(|c| c.format != f)
            || caps.color_spaces(f).contains(wgpu::SurfaceColorSpaces::SRGB)
    };
    let format = ORDER
        .into_iter()
        .find(|f| caps.formats.contains(f) && srgb(*f))
        .or_else(|| caps.formats.iter().copied().find(|f| !f.is_srgb()))
        .or_else(|| caps.formats.first().copied())?;
    let space = if format == wgpu::TextureFormat::Rgba16Float && srgb(format) {
        wgpu::SurfaceColorSpace::Srgb
    } else {
        wgpu::SurfaceColorSpace::Auto
    };
    Some((format, space))
}

/// One line for the log: the formats offered in order, the chosen one
/// marked.
pub fn surface_formats_line(caps: &wgpu::SurfaceCapabilities, chosen: wgpu::TextureFormat) -> String {
    let list: Vec<String> =
        caps.formats.iter().map(|f| if *f == chosen { format!("[{f:?}]") } else { format!("{f:?}") }).collect();
    format!("{} offered: {}", list.len(), list.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::{SurfaceColorSpaces as S, SurfaceFormatCapabilities, TextureFormat as F};

    fn caps(list: &[(F, S)]) -> wgpu::SurfaceCapabilities {
        wgpu::SurfaceCapabilities {
            formats: list.iter().filter(|(_, s)| s.contains(S::SRGB)).map(|(f, _)| *f).collect(),
            format_capabilities: list
                .iter()
                .map(|&(format, color_spaces)| SurfaceFormatCapabilities { format, color_spaces })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn the_sdr_frame_takes_an_8_bit_format_before_wider_ones() {
        let auto = wgpu::SurfaceColorSpace::Auto;
        // A colour-managed desktop's list: wide formats first.
        let wide_first = caps(&[
            (F::Rgba16Float, S::SRGB | S::EXTENDED_SRGB_LINEAR),
            (F::Rgba16Unorm, S::SRGB),
            (F::Rgb10a2Unorm, S::SRGB | S::BT2100_PQ),
            (F::Bgra8UnormSrgb, S::SRGB),
            (F::Bgra8Unorm, S::SRGB),
            (F::Rgba8Unorm, S::SRGB),
        ]);
        assert_eq!(sdr_surface_format(&wide_first), Some((F::Bgra8Unorm, auto)));
        let rgba_only = caps(&[(F::Rgba16Float, S::SRGB), (F::Rgba8UnormSrgb, S::SRGB), (F::Rgba8Unorm, S::SRGB)]);
        assert_eq!(sdr_surface_format(&rgba_only), Some((F::Rgba8Unorm, auto)));
        // 10-bit only (and its sRGB-only HDR twin): 10-bit.
        let ten = caps(&[(F::Rgba16Float, S::EXTENDED_SRGB_LINEAR), (F::Rgb10a2Unorm, S::SRGB)]);
        assert_eq!(sdr_surface_format(&ten), Some((F::Rgb10a2Unorm, auto)));
        // 16-bit float only: taken, asked for as sRGB (not extended linear).
        let half = caps(&[(F::Rgba16Float, S::SRGB | S::EXTENDED_SRGB_LINEAR)]);
        assert_eq!(sdr_surface_format(&half), Some((F::Rgba16Float, wgpu::SurfaceColorSpace::Srgb)));
        // A list without per-format colour spaces counts as sRGB.
        let bare = wgpu::SurfaceCapabilities { formats: vec![F::Rgba16Float, F::Bgra8Unorm], ..Default::default() };
        assert_eq!(sdr_surface_format(&bare), Some((F::Bgra8Unorm, auto)));
        // sRGB formats only: the first one, as wgpu's default.
        let srgb_only = caps(&[(F::Bgra8UnormSrgb, S::SRGB), (F::Rgba8UnormSrgb, S::SRGB)]);
        assert_eq!(sdr_surface_format(&srgb_only), Some((F::Bgra8UnormSrgb, auto)));
        // Something outside the order: the first non-sRGB one.
        let odd = caps(&[(F::Rgba8UnormSrgb, S::SRGB), (F::Rg11b10Ufloat, S::SRGB)]);
        assert_eq!(sdr_surface_format(&odd), Some((F::Rg11b10Ufloat, auto)));
        assert_eq!(sdr_surface_format(&caps(&[])), None);
        assert_eq!(
            surface_formats_line(&wide_first, F::Bgra8Unorm),
            "6 offered: Rgba16Float, Rgba16Unorm, Rgb10a2Unorm, Bgra8UnormSrgb, [Bgra8Unorm], Rgba8Unorm"
        );
    }
}

/// The file in the cache folder that holds the window's format as the
/// game's window last chose it on this machine (`game --prepare` makes its
/// programs for it: it has no window to ask).
fn window_format_file() -> Option<std::path::PathBuf> {
    Some(crate::cache::dir()?.join("window-format.txt"))
}

/// Keeps `format` as the window's ([`recorded_window_format`]); written
/// only when it changed.
pub fn record_window_format(format: wgpu::TextureFormat) {
    let Some(path) = window_format_file() else { return };
    let text = format!("{format:?}\n");
    if std::fs::read_to_string(&path).is_ok_and(|t| t == text) {
        return;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, text);
}

/// The window's format the game's window last chose on this machine, if
/// it is one of the formats the window ever picks.
pub fn recorded_window_format() -> Option<wgpu::TextureFormat> {
    let text = std::fs::read_to_string(window_format_file()?).ok()?;
    let name = text.trim();
    ORDER
        .into_iter()
        .chain([wgpu::TextureFormat::Bgra8UnormSrgb, wgpu::TextureFormat::Rgba8UnormSrgb])
        .find(|f| format!("{f:?}") == name)
}

/// What a prepare run was made for: this build, the adapter and its
/// driver (`game --prepare` writes it last; `game --probe` reports whether
/// it still holds, and the launcher offers the step again when not), and
/// the settings' program key (`graphics_table::program_key`) on its own
/// last line.
pub fn prepared_stamp(info: &wgpu::AdapterInfo, programs: u64) -> String {
    format!("{}programs={programs:016x}\n", prepared_machine(info))
}

/// The stamp's lines for the build, the adapter and its driver.
fn prepared_machine(info: &wgpu::AdapterInfo) -> String {
    format!(
        "build={:016x}\nadapter={:04x}:{:04x} {}\ndriver={} {}\n",
        crate::cache::build_id(),
        info.vendor,
        info.device,
        info.name,
        info.driver,
        info.driver_info
    )
}

fn prepared_file() -> Option<std::path::PathBuf> {
    Some(crate::cache::dir()?.join("prepared.txt"))
}

/// Writes [`prepared_stamp`] into the cache folder.
pub fn write_prepared_stamp(info: &wgpu::AdapterInfo, programs: u64) {
    let Some(path) = prepared_file() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Whole or not at all (`cache::replace_file`).
    let _ = crate::cache::replace_file(&path, prepared_stamp(info, programs).as_bytes());
}

/// The program key of the cache folder's prepare stamp when the stamp is
/// for this build, adapter and driver (None: no stamp, or for another).
pub fn prepared_programs(info: &wgpu::AdapterInfo) -> Option<u64> {
    let text = prepared_file().and_then(|p| std::fs::read_to_string(p).ok())?;
    let rest = text.strip_prefix(&prepared_machine(info))?;
    u64::from_str_radix(rest.strip_prefix("programs=")?.strip_suffix('\n')?, 16).ok()
}

/// Whether the cache folder holds a prepare run's stamp for this build,
/// adapter, driver and the settings' program key `programs`.
pub fn is_prepared(info: &wgpu::AdapterInfo, programs: u64) -> bool {
    prepared_programs(info) == Some(programs)
}
