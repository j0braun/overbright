use super::*;

/// Rows per statistic (elevation for the main texture, radius for the cap).
pub const ROWS: usize = 16;
/// Sky table pairs (0x8A07A8: four main/cap pairs).
pub const PAIRS: usize = 4;

/// The Sky row in force (Original: nothing is made or drawn).
pub fn enabled(opts: &crate::render::RenderOptions) -> bool {
    opts.sky != 0
}

static RAIN: AtomicU32 = AtomicU32::new(0);

/// The weather's rain amount (0..1) this frame, from the game loop.
pub fn set_rain(amount: f32) {
    RAIN.store(amount.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
}

/// The rain amount the extras see (the game's, or NFSU2_NIGHT_SKY_RAIN).
pub fn rain_amount() -> f32 {
    rain()
}

pub(super) fn rain() -> f32 {
    crate::render::debug_env::nfsu2_night_sky_rain().unwrap_or_else(|| f32::from_bits(RAIN.load(Ordering::Relaxed)))
}

/// Per-row colour statistics of one sky texture: clear (10th luma percentile), mean, lit (90th), each
/// rgb + the row's mean alpha, in 0..1.
pub type Stats = [[[f32; 4]; 3]; ROWS];

fn luma(p: &[u8]) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

/// Statistics of a set of pixels (RGBA8): the 10th / 90th luma percentile pixels' average colour around
/// each percentile (a 5 % window) and the mean; alpha-weighted when `weighted` (the cap's disc).
fn stats_of(mut px: Vec<[u8; 4]>, weighted: bool) -> [[f32; 4]; 3] {
    if px.is_empty() {
        return [[0.0; 4]; 3];
    }
    let n = px.len() as f32;
    let alpha = px.iter().map(|p| p[3] as f32).sum::<f32>() / n / 255.0;
    if weighted {
        px.retain(|p| p[3] > 32);
        if px.is_empty() {
            return [[0.0, 0.0, 0.0, alpha]; 3];
        }
    }
    px.sort_by(|a, b| luma(a).total_cmp(&luma(b)));
    let avg = |lo: f32, hi: f32| {
        let (a, b) = (
            (lo * px.len() as f32) as usize,
            ((hi * px.len() as f32) as usize).max((lo * px.len() as f32) as usize + 1).min(px.len()),
        );
        let s = px[a..b].iter().fold([0f32; 3], |s, p| [s[0] + p[0] as f32, s[1] + p[1] as f32, s[2] + p[2] as f32]);
        let k = (b - a) as f32 * 255.0;
        [s[0] / k, s[1] / k, s[2] / k, alpha]
    };
    [avg(0.075, 0.125), avg(0.0, 1.0), avg(0.875, 0.925)]
}

/// The main texture's rows (top v = 0 .. bottom v = 1, the dome's zenith side .. horizon), `ROWS` bands.
pub fn main_stats(rgba: &[u8], w: u32, h: u32) -> Stats {
    std::array::from_fn(|r| {
        let (y0, y1) =
            (r as u32 * h / ROWS as u32, ((r as u32 + 1) * h / ROWS as u32).max(r as u32 * h / ROWS as u32 + 1));
        let px = (y0..y1)
            .flat_map(|y| (0..w).map(move |x| ((y * w + x) * 4) as usize))
            .map(|i| [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]])
            .collect();
        stats_of(px, false)
    })
}

/// The cap's rings (centre .. rim at uv radius 0.5), `ROWS` bands, alpha-weighted colours with the ring's
/// mean alpha.
pub fn cap_stats(rgba: &[u8], w: u32, h: u32) -> Stats {
    let mut rings: Vec<Vec<[u8; 4]>> = vec![Vec::new(); ROWS];
    for y in 0..h {
        for x in 0..w {
            let (u, v) = ((x as f32 + 0.5) / w as f32 - 0.5, (y as f32 + 0.5) / h as f32 - 0.5);
            let r = (u * u + v * v).sqrt() / 0.5;
            if r < 1.0 {
                let i = ((y * w + x) * 4) as usize;
                rings[((r * ROWS as f32) as usize).min(ROWS - 1)].push([
                    rgba[i],
                    rgba[i + 1],
                    rgba[i + 2],
                    rgba[i + 3],
                ]);
            }
        }
    }
    std::array::from_fn(|r| stats_of(std::mem::take(&mut rings[r]), true))
}

/// The sky table's palettes ([`crate::sky::SKY_TABLE`] order: main, cap per pair).
#[derive(Clone)]
pub struct Palette {
    pub main: [Stats; PAIRS],
    pub cap: [Stats; PAIRS],
}

impl Palette {
    /// From the world's sky table textures (missing ones stay black).
    pub fn from_world(world: &crate::world::World) -> Palette {
        let get = |name: &str| {
            world.textures.get(&crate::formats::hash::bin_hash(name)).map(|t| (t.decode_rgba(), t.width, t.height))
        };
        let table = crate::sky::SKY_TABLE;
        Palette {
            main: std::array::from_fn(|p| {
                get(table[2 * p]).map(|(d, w, h)| main_stats(&d, w, h)).unwrap_or([[[0.0; 4]; 3]; ROWS])
            }),
            cap: std::array::from_fn(|p| {
                get(table[2 * p + 1]).map(|(d, w, h)| cap_stats(&d, w, h)).unwrap_or([[[0.0; 4]; 3]; ROWS])
            }),
        }
    }
}
