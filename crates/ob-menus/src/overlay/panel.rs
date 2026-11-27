//! The overlay's drawing: the quads of Minimal / Standard / Full (the painter and the colours).
//!
//! The three layouts are one panel: the same backing, font size, label and
//! value colours, columns and spacing. Minimal is the panel's first rows
//! (the frame figures), the slowed-game warning and the deviations, without
//! the system rows and the graph.

use super::{DisplayInfo, Layout, Overlay, Settings, last_load_ms, stats};
use crate::frontend::font::Font;
use crate::frontend::{Assets, CANVAS, Place, Quad, text_quads};
use glam::Vec2;

/// MangoHud's default colours (RGB): text, GPU, CPU, VRAM, RAM, engine
/// (FPS), I/O, frame-time graph, background (alpha 0.5).
mod colour {
    const fn rgb(v: u32) -> [f32; 4] {
        [((v >> 16) & 0xFF) as f32 / 255.0, ((v >> 8) & 0xFF) as f32 / 255.0, (v & 0xFF) as f32 / 255.0, 1.0]
    }
    pub const TEXT: [f32; 4] = rgb(0xFFFFFF);
    pub const GPU: [f32; 4] = rgb(0x2E9762);
    pub const CPU: [f32; 4] = rgb(0x2E97CB);
    pub const VRAM: [f32; 4] = rgb(0xAD64C1);
    pub const RAM: [f32; 4] = rgb(0xC26693);
    pub const ENGINE: [f32; 4] = rgb(0xEB5B5B);
    pub const IO: [f32; 4] = rgb(0xA491D3);
    pub const GRAPH: [f32; 4] = rgb(0x00FF00);
    pub const BACKGROUND: [f32; 4] = [0.008, 0.008, 0.008, 0.5];
    pub const DIM: [f32; 4] = [0.75, 0.75, 0.75, 1.0];
    /// The warning that the game runs slower than real time.
    pub const WARN: [f32; 4] = [1.0, 0.35, 0.3, 1.0];
}

/// The panel's text size (× the font's), its padding and the gap between
/// the label and value columns, canvas pixels at 100 %.
const FS: f32 = 0.32;
const PAD: f32 = 4.0;
const GAP: f32 = 12.0;
/// The deviations' lines: at most about this many characters.
const DEVIATIONS_WRAP: usize = 64;

/// A panel row: the label's colour, the label, the value (drawn white).
type Row = ([f32; 4], String, String);

impl Overlay {
    /// The overlay's quads (none when hidden): rebuilt after a refresh or
    /// a settings change, copied otherwise.
    #[allow(clippy::too_many_arguments)]
    pub fn quads(
        &mut self,
        assets: &Assets,
        font: &Font,
        deviations: impl FnOnce() -> Vec<String>,
        display: impl FnOnce() -> DisplayInfo,
        s: &Settings,
        out: &mut Vec<Quad>,
    ) {
        let Some(layout) = self.shown else { return };
        let key = Some((self.shown, *s));
        if self.dirty || self.cache_key != key {
            self.cache.clear();
            let mut p = Painter::default();
            let devs = deviations();
            match layout {
                Layout::Minimal => self.paint_minimal(&mut p, font, &devs),
                _ => self.paint_panel(&mut p, font, &devs, &display(), layout == Layout::Full),
            }
            p.emit(assets, font, s, &mut self.cache);
            self.cache_key = key;
            self.dirty = false;
        }
        out.extend_from_slice(&self.cache);
    }

    /// Minimal: Standard's frame rows, the slowed-game warning (red, across
    /// both columns) and Standard's deviation rows. Its rows are a part of
    /// Standard's and the warning is narrower than Standard's ticks row, so
    /// in the same corner Minimal covers a part of what Standard covers.
    fn paint_minimal(&self, p: &mut Painter, font: &Font, deviations: &[String]) {
        let figures = frame_rows(&self.summary);
        let slowed = slowed_text(&self.summary);
        let devs = deviation_rows(deviations, |line| line.len() <= DEVIATIONS_WRAP);
        let label_w = figures.iter().chain(&devs).map(|r| font.measure(&r.1) * FS).fold(0.0, f32::max);
        let value_x = label_w + GAP;
        let mut width =
            figures.iter().chain(&devs).map(|r| value_x + measure_tabular(font, &r.2) * FS).fold(0.0, f32::max);
        if let Some(s) = &slowed {
            width = width.max(measure_tabular(font, s) * FS);
        }
        let step = font.line_height * FS;
        let backing = p.rect(Vec2::ZERO, Vec2::ZERO, colour::BACKGROUND);
        let mut y = PAD;
        let mut row = |p: &mut Painter, (c, label, value): &Row| {
            p.text(font, Vec2::new(PAD, y), label, FS, *c);
            p.text(font, Vec2::new(PAD + value_x, y), value, FS, colour::TEXT);
            y += step;
        };
        figures.iter().for_each(|r| row(p, r));
        if let Some(s) = slowed {
            row(p, &(colour::WARN, s, String::new()));
        }
        devs.iter().for_each(|r| row(p, r));
        p.set_rect(backing, Vec2::ZERO, Vec2::new(width + 2.0 * PAD, y + PAD));
    }

    fn paint_panel(&self, p: &mut Painter, font: &Font, deviations: &[String], d: &DisplayInfo, full: bool) {
        let f = &self.summary;
        let snap = self.snapshot.clone().unwrap_or_default();
        let na = || "n/a".to_string();
        let mut rows: Vec<Row> = frame_rows(f);
        rows.push((
            if slowed_text(f).is_some() { colour::WARN } else { colour::TEXT },
            "Ticks".into(),
            if f.frames > 0 { ticks_text(f) } else { na() },
        ));
        rows.push((
            colour::CPU,
            "CPU".into(),
            parts([
                snap.cpu.map(|c| format!("{c:.0}%")),
                snap.cpu_mhz
                    .map(|m| format!("{} MHz", if full { format!("{}/{}", m.0, m.1) } else { m.0.to_string() })),
                snap.cpu_temp.map(|t| format!("{t:.0}\u{B0}C")),
                snap.cpu_power.map(|w| format!("{w:.0} W")),
                snap.process_cpu.map(|c| format!("game {c:.0}%")),
            ]),
        ));
        if full && !snap.cores.is_empty() {
            for (i, chunk) in snap.cores.chunks(8).enumerate() {
                let label = if i == 0 { "Cores" } else { "" };
                rows.push((
                    colour::CPU,
                    label.into(),
                    chunk.iter().map(|c| format!("{c:>3.0}")).collect::<Vec<_>>().join(" "),
                ));
            }
        }
        let g = &snap.gpu;
        rows.push((
            colour::GPU,
            "GPU".into(),
            parts([
                g.busy.map(|b| format!("{b:.0}%")),
                g.core_mhz.map(|c| format!("{c}{} MHz", g.mem_mhz.map(|m| format!("/{m}")).unwrap_or_default())),
                g.temp_c
                    .map(|t| format!("{t:.0}\u{B0}C{}", g.hotspot_c.map(|h| format!(" ({h:.0})")).unwrap_or_default())),
                g.power_w.map(|w| format!("{w:.0} W")),
                snap.own_gpu.filter(|_| full).map(|o| format!("game {o:.0}%")),
            ]),
        ));
        let gb = |b: u64| b as f64 / (1024.0 * 1024.0 * 1024.0);
        rows.push((
            colour::VRAM,
            "VRAM".into(),
            parts([
                g.vram_used.map(|u| {
                    format!("{:.2}{} GB", gb(u), g.vram_total.map(|t| format!("/{:.1}", gb(t))).unwrap_or_default())
                }),
                g.gtt_used.map(|u| format!("GTT {:.2}", gb(u))),
                snap.own_vram.map(|o| {
                    format!(
                        "game {:.2}{}",
                        gb(o),
                        snap.own_gtt.filter(|_| full).map(|t| format!(" + {:.2}", gb(t))).unwrap_or_default()
                    )
                }),
            ]),
        ));
        rows.push((
            colour::RAM,
            "RAM".into(),
            parts([
                snap.mem_used.map(|u| {
                    format!("{:.1}{} GB", gb(u), snap.mem_total.map(|t| format!("/{:.1}", gb(t))).unwrap_or_default())
                }),
                snap.rss.map(|r| format!("game {:.2}", gb(r))),
                snap.swap_total
                    .filter(|t| *t > 0)
                    .map(|t| format!("swap {:.1}/{:.1}", gb(snap.swap_used.unwrap_or(0)), gb(t))),
            ]),
        ));
        let mbs = |v: f64| v / (1024.0 * 1024.0);
        if full {
            rows.push((
                colour::IO,
                "Game I/O".into(),
                parts([
                    snap.io_read.zip(snap.io_write).map(|(r, w)| format!("disk {:.1}/{:.1} MB/s", mbs(r), mbs(w))),
                    snap.io_rchar.zip(snap.io_wchar).map(|(r, w)| format!("all {:.1}/{:.1}", mbs(r), mbs(w))),
                ]),
            ));
            rows.push((
                colour::IO,
                "Disks".into(),
                parts([snap
                    .disk_read
                    .zip(snap.disk_write)
                    .map(|(r, w)| format!("{:.1}/{:.1} MB/s read/write", mbs(r), mbs(w)))]),
            ));
            rows.push((
                colour::IO,
                "Last load".into(),
                last_load_ms().map_or_else(|| "none yet".into(), |m| format!("{m:.0} ms")),
            ));
        }
        rows.push((
            colour::TEXT,
            "Display".into(),
            parts([
                Some(format!(
                    "{}{}",
                    d.present,
                    if d.latency > 0 { format!(" ({} queued)", d.latency) } else { String::new() }
                )),
                Some(format!("low latency {}", if d.low_latency { "on" } else { "off" })),
                Some(if d.frame_cap > 0 { format!("cap {}", d.frame_cap) } else { "uncapped".into() }),
                (full && d.size.0 > 0).then(|| format!("{}x{}", d.size.0, d.size.1)),
            ]),
        ));
        if full && let Some(v) = &snap.vrr {
            rows.push((colour::TEXT, "VRR".into(), v.clone()));
        }
        if full {
            if !d.adapter.is_empty() {
                rows.push((colour::DIM, "Adapter".into(), d.adapter.clone()));
            }
            for (i, (name, pct)) in snap.threads.iter().filter(|t| t.1 >= 0.05).take(8).enumerate() {
                rows.push((
                    colour::CPU,
                    if i == 0 { "Threads".into() } else { String::new() },
                    format!("{pct:>5.1}%  {name}"),
                ));
            }
            match &self.passes {
                Some(v) => {
                    rows.push((colour::GPU, "GPU passes".into(), "ms per GPU frame (a light-trail render is one)".into()));
                    for (name, ms) in v.iter().take(10) {
                        let name: String = name.chars().take(34).collect();
                        rows.push((colour::GPU, String::new(), format!("{ms:>6.3} ms  {name}")));
                    }
                }
                None => rows.push((
                    colour::GPU,
                    "GPU passes".into(),
                    match self.gpu_timing {
                        Some(false) if crate::prof::gpu::ready() => "timed by the bench".into(),
                        Some(false) => "n/a (no GPU timestamps)".into(),
                        _ => "starting".into(),
                    },
                )),
            }
        }
        // The deviations, wrapped.
        rows.extend(deviation_rows(deviations, |line| line.len() <= DEVIATIONS_WRAP));
        // Two columns: labels, then values.
        let step = font.line_height * FS;
        let label_w = rows.iter().map(|r| font.measure(&r.1) * FS).fold(0.0, f32::max);
        let value_x = label_w + GAP;
        let pad = PAD;
        let mut y = pad;
        let mut width = rows.iter().map(|r| value_x + measure_tabular(font, &r.2) * FS).fold(0.0, f32::max);
        let graph_h = 28.0;
        let graph_w = 200.0f32.max(width - 2.0 * pad);
        width = width.max(graph_w);
        // The panel's backing first (filled once the height is known).
        let backing = p.rect(Vec2::ZERO, Vec2::ZERO, colour::BACKGROUND);
        for (c, label, value) in &rows {
            p.text(font, Vec2::new(pad, y), label, FS, *c);
            p.text(font, Vec2::new(pad + value_x, y), value, FS, colour::TEXT);
            y += step;
        }
        // The frame-time graph: the last frames, scaled to twice the mean
        // (or the longest), with the mean's line.
        y += 3.0;
        let graph: Vec<f32> = self.stats.graph().copied().collect();
        let top = graph.iter().copied().fold(f.avg_ms * 2.0, f32::max).max(1.0);
        // A long frame spent mostly waiting for a swap-chain image
        // says so (the reported 1007 / 1052 ms stalls of 2026-11-06 were
        // wgpu's 1 s acquire timeout: the maintainer's private notes).
        let (worst_ms, worst_acquire) = self.stats.worst();
        let label = if worst_ms >= 100.0 && worst_acquire >= worst_ms * 0.5 {
            format!(
                "Frametime, last {} frames  max {worst_ms:.1} ms ({worst_acquire:.0} ms waiting for a display image)",
                graph.len()
            )
        } else {
            format!("Frametime, last {} frames  max {:.1} ms", graph.len(), graph.iter().copied().fold(0.0, f32::max))
        };
        p.text(font, Vec2::new(pad, y), &label, FS * 0.85, colour::GRAPH);
        y += step * 0.9;
        let n = stats::GRAPH_LEN as f32;
        let bar = graph_w / n;
        let base = y + graph_h;
        p.rect(Vec2::new(pad, y), Vec2::new(pad + graph_w, base), [0.0, 0.0, 0.0, 0.35]);
        let start = stats::GRAPH_LEN - graph.len();
        // A line: each frame a segment from the previous frame's height
        // to its own (MangoHud draws a line graph).
        let height = |ms: f32| base - (ms / top).min(1.0) * graph_h;
        let mut last = graph.first().map(|ms| height(*ms));
        for (i, ms) in graph.iter().enumerate() {
            let x = pad + (start + i) as f32 * bar;
            let y1 = height(*ms);
            let y0 = last.unwrap_or(y1);
            last = Some(y1);
            p.rect(Vec2::new(x, y0.min(y1) - 0.4), Vec2::new(x + bar.max(0.5), y0.max(y1) + 0.4), colour::GRAPH);
        }
        let mean_y = base - (f.avg_ms / top).min(1.0) * graph_h;
        p.rect(Vec2::new(pad, mean_y - 0.25), Vec2::new(pad + graph_w, mean_y + 0.25), [1.0, 1.0, 1.0, 0.5]);
        y = base + 3.0;
        if full {
            // The pacing histogram: the share of frames per distance from
            // the median frame time.
            p.text(font, Vec2::new(pad, y), "Pacing (|frame - median|)", FS * 0.85, colour::GRAPH);
            y += step * 0.9;
            let labels = ["0.25", "0.5", "1", "2", "4", ">4 ms"];
            let col = graph_w / stats::PACING_BINS as f32;
            let h = 20.0;
            for (i, share) in self.pacing.iter().enumerate() {
                let x = pad + i as f32 * col;
                let bh = share.clamp(0.0, 1.0) * h;
                p.rect(Vec2::new(x + 2.0, y + h - bh), Vec2::new(x + col - 2.0, y + h), colour::GRAPH);
                p.text(
                    font,
                    Vec2::new(x + 2.0, y + h + 1.0),
                    &format!("{}  {:.0}%", labels[i], share * 100.0),
                    FS * 0.75,
                    colour::TEXT,
                );
            }
            y += h + step;
        }
        p.set_rect(backing, Vec2::ZERO, Vec2::new(width + 2.0 * pad, y + pad));
    }
}

/// The panel's first rows, Minimal's figures: FPS, the last second's mean
/// frame time and the last 10 s's lows; its 99th percentile and longest
/// frame; what a frame was spent on (the main thread's work, the waits);
/// the GPU's time per frame.
pub fn frame_rows(f: &stats::FrameSummary) -> Vec<Row> {
    let fps = if f.frames > 0 {
        format!("{}   {} ms   1% {:.0}   0.1% {:.0}", num(f.fps, 3, 0), num(f.avg_ms, 6, 2), f.low1_fps, f.low01_fps)
    } else {
        "n/a".into()
    };
    vec![
        (colour::ENGINE, "FPS".into(), fps),
        (colour::ENGINE, "Frame".into(), format!("99% {}   max {} ms", num(f.p99_ms, 5, 1), num(f.max_ms, 5, 1))),
        (colour::TEXT, "CPU time".into(), cpu_text(f)),
        (colour::TEXT, "Waits".into(), wait_text(f)),
        (colour::GPU, "GPU time".into(), gpu_text(f)),
    ]
}

/// The deviations on ("none" when none), broken at commas into the lines
/// `fits` accepts, the first labelled.
fn deviation_rows(deviations: &[String], fits: impl Fn(&str) -> bool) -> Vec<Row> {
    let devs = if deviations.is_empty() { "none".to_string() } else { deviations.join(", ") };
    wrap_by(&devs, fits)
        .into_iter()
        .enumerate()
        .map(|(i, line)| (colour::DIM, if i == 0 { "Deviations".into() } else { String::new() }, line))
        .collect()
}

/// The main thread's work per frame: the simulation's ticks, the drawing
/// and, when there were any, the light-trail renders.
fn cpu_text(f: &stats::FrameSummary) -> String {
    let trails = if f.probes_per_s > 0.0 { format!("   trails {}", num(f.probe_ms, 5, 2)) } else { String::new() };
    format!("update {}   draw {}{trails} ms", num(f.update_ms, 5, 2), num(f.draw_ms, 5, 2))
}

/// The waits per frame: the display's, and the frame limiter's when it slept.
fn wait_text(f: &stats::FrameSummary) -> String {
    let cap = if f.cap_ms >= 0.005 { format!("   cap {}", num(f.cap_ms, 5, 2)) } else { String::new() };
    format!("display {}{cap} ms", num(f.wait_ms, 5, 2))
}

/// The GPU's busy time per frame and its share of the time, or why there
/// is none.
fn gpu_text(f: &stats::FrameSummary) -> String {
    match (f.gpu_ms, f.gpu_state) {
        (Some(ms), _) => match f.gpu_load {
            Some(l) => format!("{} ms   busy {:.0}%", num(ms, 5, 2), l * 100.0),
            None => format!("{} ms", num(ms, 5, 2)),
        },
        (None, stats::GpuState::Untimed) => "n/a (no GPU timestamps)".into(),
        (None, _) => "...".into(),
    }
}

/// The ticks per second, the simulation's time per tick, the game's speed
/// and the light trails' form.
fn ticks_text(f: &stats::FrameSummary) -> String {
    let form = match f.streak_exact {
        Some(e) if e >= 0.995 => "   trails exact".to_string(),
        Some(e) if e <= 0.005 => "   trails cheap".to_string(),
        Some(e) => format!("   trails exact {:.0}%", e * 100.0),
        None => String::new(),
    };
    format!("{}/s   {} ms each   speed {:.0}%{form}", num(f.ticks_per_s, 2, 0), num(f.tick_ms, 5, 2), (f.speed * 100.0).floor())
}

/// The warning when the game ran slower than real time in the last
/// second (frames longer than the ticks one frame may take: 3/60 s, 2/30 s
/// in a scene); None at full speed.
pub fn slowed_text(f: &stats::FrameSummary) -> Option<String> {
    (f.frames > 0 && f.speed < 0.995)
        .then(|| format!("game slowed to {:.0}% speed (frames too long)", (f.speed * 100.0).floor()))
}

/// The figure space: a blank as wide as a digit (drawn by [`Painter`]).
const FIGURE_SPACE: char = '\u{2007}';

/// `v` with `prec` decimals, padded on the left with figure spaces to
/// `width` characters, so a number keeps its place as it changes.
pub fn num(v: f32, width: usize, prec: usize) -> String {
    let s = format!("{v:.prec$}");
    let pad = width.saturating_sub(s.chars().count());
    std::iter::repeat_n(FIGURE_SPACE, pad).chain(s.chars()).collect()
}

/// The widest digit's advance: every digit (and the figure space) is
/// drawn in a cell this wide, centred, so digits do not jitter.
fn digit_cell(font: &Font) -> f32 {
    ('0'..='9').filter_map(|c| font.glyphs.get(&u32::from(c))).map(|g| g.advance).fold(0.0, f32::max)
}

fn tabular(c: char) -> bool {
    c.is_ascii_digit() || c == FIGURE_SPACE
}

/// The width of `line` (no line breaks) as [`Painter`] draws it, font pixels.
fn measure_line(font: &Font, line: &str, cell: f32) -> f32 {
    let mut w = 0.0;
    for (digits, run) in runs(line) {
        w += if digits { cell * run.chars().count() as f32 } else { font.measure(run) };
    }
    w
}

/// The widest line of `text` ('^' breaks lines), font pixels.
pub fn measure_tabular(font: &Font, text: &str) -> f32 {
    let cell = digit_cell(font);
    text.split('^').map(|l| measure_line(font, l, cell)).fold(0.0, f32::max)
}

/// `line` cut into runs of digits / figure spaces and runs of the rest.
fn runs(line: &str) -> impl Iterator<Item = (bool, &str)> {
    let mut rest = line;
    std::iter::from_fn(move || {
        let first = rest.chars().next()?;
        let kind = tabular(first);
        let end = rest.find(|c: char| tabular(c) != kind).unwrap_or(rest.len());
        let (run, tail) = rest.split_at(end);
        rest = tail;
        Some((kind, run))
    })
}

/// The known parts joined, or "n/a".
fn parts<const N: usize>(v: [Option<String>; N]) -> String {
    let s: Vec<String> = v.into_iter().flatten().collect();
    if s.is_empty() { "n/a".into() } else { s.join("   ") }
}

/// `text` broken at ", " into lines of at most about `width` characters.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    wrap_by(text, |line| line.len() <= width)
}

/// `text` broken at ", " into lines `fits` accepts (an item it does not
/// accept alone gets a line of its own).
fn wrap_by(text: &str, fits: impl Fn(&str) -> bool) -> Vec<String> {
    let mut lines = vec![String::new()];
    for (i, item) in text.split(", ").enumerate() {
        let last = lines.last_mut().unwrap();
        if !last.is_empty() && !fits(&format!("{last}, {item}")) {
            last.push(',');
            lines.push(item.to_string());
        } else {
            if i > 0 {
                last.push_str(", ");
            }
            last.push_str(item);
        }
    }
    lines
}

enum Prim {
    Text { at: Vec2, text: String, scale: f32, color: [f32; 4] },
    Rect { min: Vec2, max: Vec2, color: [f32; 4] },
}

/// The overlay's drawing in its own space (origin its top left, canvas
/// pixels at 100 %), placed in its corner and scaled when emitted.
#[derive(Default)]
struct Painter {
    prims: Vec<Prim>,
    size: Vec2,
}

impl Painter {
    fn text(&mut self, font: &Font, at: Vec2, text: &str, scale: f32, color: [f32; 4]) {
        if text.is_empty() {
            return;
        }
        let w = measure_tabular(font, text) * scale;
        let h = text.split('^').count() as f32 * font.line_height * scale;
        self.size = self.size.max(at + Vec2::new(w, h));
        self.prims.push(Prim::Text { at, text: text.to_string(), scale, color });
    }

    fn rect(&mut self, min: Vec2, max: Vec2, color: [f32; 4]) -> usize {
        self.size = self.size.max(max);
        self.prims.push(Prim::Rect { min, max, color });
        self.prims.len() - 1
    }

    fn set_rect(&mut self, i: usize, min: Vec2, max: Vec2) {
        self.size = self.size.max(max);
        if let Some(Prim::Rect { min: a, max: b, .. }) = self.prims.get_mut(i) {
            (*a, *b) = (min, max);
        }
    }

    /// The quads, in the settings' corner ([`POSITION_NAMES`]), 8 canvas
    /// pixels from its edges, at its scale.
    fn emit(&self, assets: &Assets, font: &Font, s: &Settings, out: &mut Vec<Quad>) {
        let k = f32::from(s.scale) / 100.0;
        let right = s.position & 1 != 0;
        let bottom = s.position & 2 != 0;
        let size = self.size * k;
        let origin = Vec2::new(
            if right { CANVAS.x - 8.0 - size.x } else { 8.0 },
            if bottom { CANVAS.y - 8.0 - size.y } else { 8.0 },
        );
        let place = Place::new(if right { 1 } else { -1 }, if bottom { 1 } else { -1 });
        let start = out.len();
        for p in &self.prims {
            match p {
                Prim::Text { at, text, scale, color } => {
                    tabular_quads(assets, font, text, scale * k, origin + *at * k, *color, out);
                }
                Prim::Rect { min, max, color } => out.push(Quad {
                    texture: None,
                    rect: [origin + *min * k, origin + *max * k],
                    uv: [Vec2::ZERO, Vec2::ONE],
                    color: *color,
                    z: -1.0,
                    angle: 0.0,
                    place,
                }),
            }
        }
        for q in &mut out[start..] {
            q.place = place;
        }
    }
}

/// `text` ('^' breaks lines) drawn at `at`, `scale`, its digits and figure
/// spaces in cells of one width ([`digit_cell`]).
fn tabular_quads(assets: &Assets, font: &Font, text: &str, scale: f32, at: Vec2, color: [f32; 4], out: &mut Vec<Quad>) {
    let cell = digit_cell(font);
    let mut buf = [0u8; 4];
    for (i, line) in text.split('^').enumerate() {
        let y = at.y + i as f32 * font.line_height * scale;
        let mut x = at.x;
        for (digits, run) in runs(line) {
            if !digits {
                text_quads(assets, font, run, 0, Vec2::splat(scale), Vec2::new(x, y), color, -1.0, out);
                x += font.measure(run) * scale;
                continue;
            }
            for c in run.chars() {
                if c != FIGURE_SPACE {
                    let advance = font.glyphs.get(&u32::from(c)).map_or(cell, |g| g.advance);
                    let at = Vec2::new(x + (cell - advance) * 0.5 * scale, y);
                    text_quads(assets, font, c.encode_utf8(&mut buf), 0, Vec2::splat(scale), at, color, -1.0, out);
                }
                x += cell * scale;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::font::Glyph;

    /// A font of printable ASCII: letters 5 wide, digits 6.
    fn font() -> Font {
        let glyph = |advance: f32| Glyph {
            width: advance,
            height: 10.0,
            x: 0.0,
            y: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
            advance,
            kerning: Vec::new(),
        };
        let glyphs = (32u8..127).map(|c| (u32::from(c), glyph(if c.is_ascii_digit() { 6.0 } else { 5.0 }))).collect();
        Font { name: "t".into(), texture: "t".into(), line_height: 30.0, texture_size: [64.0, 64.0], glyphs }
    }

    fn summary() -> stats::FrameSummary {
        stats::FrameSummary {
            frames: 60,
            fps: 60.0,
            avg_ms: 16.67,
            p99_ms: 16.7,
            max_ms: 17.0,
            low1_fps: 55.0,
            low01_fps: 40.0,
            update_ms: 0.8,
            draw_ms: 4.0,
            wait_ms: 11.6,
            gpu_ms: Some(3.1),
            gpu_load: Some(0.186),
            gpu_state: stats::GpuState::Timed,
            speed: 1.0,
            ..Default::default()
        }
    }

    fn painted(layout: Layout, summary: stats::FrameSummary, devs: &[String]) -> Painter {
        let o = Overlay { shown: Some(layout), summary, ..Overlay::default() };
        let mut p = Painter::default();
        match layout {
            Layout::Minimal => o.paint_minimal(&mut p, &font(), devs),
            _ => o.paint_panel(&mut p, &font(), devs, &DisplayInfo::default(), layout == Layout::Full),
        }
        p
    }

    /// The text drawn: where, what, how big, in which colour.
    fn texts(p: &Painter) -> Vec<(Vec2, String, f32, [f32; 4])> {
        p.prims
            .iter()
            .filter_map(|p| match p {
                Prim::Text { at, text, scale, color } => Some((*at, text.clone(), *scale, *color)),
                Prim::Rect { .. } => None,
            })
            .collect()
    }

    /// The panel's backing (the first quad, behind everything).
    fn backing(p: &Painter) -> (Vec2, Vec2, [f32; 4]) {
        match p.prims.first() {
            Some(Prim::Rect { min, max, color }) => (*min, *max, *color),
            _ => panic!("no backing"),
        }
    }

    fn many_deviations() -> Vec<String> {
        (0..24).map(|i| format!("deviation number {i}")).collect()
    }

    /// Minimal is Standard's and Full's panel: the same backing, text
    /// size, colours and columns; its frame rows are theirs, drawn where
    /// they draw them.
    #[test]
    fn minimal_is_the_panels_first_rows() {
        let devs = vec!["wide aspect".to_string(), "vsync-off mailbox".to_string()];
        let minimal = painted(Layout::Minimal, summary(), &devs);
        let standard = painted(Layout::Standard, summary(), &devs);
        assert_eq!(backing(&minimal).2, colour::BACKGROUND);
        assert_eq!(backing(&standard).2, colour::BACKGROUND);
        let (m, s) = (texts(&minimal), texts(&standard));
        assert_eq!(m[..10], s[..10]);
        assert!(m.iter().all(|t| t.2 == FS), "{m:?}");
        assert!(m.iter().all(|t| [colour::ENGINE, colour::TEXT, colour::GPU, colour::DIM].contains(&t.3)), "{m:?}");
        // The deviations: Standard's label and colours.
        let devs_row = m.iter().position(|t| t.1 == "Deviations").unwrap();
        assert_eq!(m[devs_row].3, colour::DIM);
        assert_eq!(devs_row, 10);
        assert_eq!(deviation_lines(&m), ["wide aspect, vsync-off mailbox"]);
        assert_eq!(m[devs_row + 1].0.x, m[1].0.x);
    }

    /// The deviations' lines drawn (the value column after "Deviations").
    fn deviation_lines(t: &[(Vec2, String, f32, [f32; 4])]) -> Vec<String> {
        let first = t.iter().position(|t| t.1 == "Deviations").unwrap();
        t[first + 1..]
            .iter()
            .take_while(|l| l.0.x == t[first + 1].0.x && l.3 == colour::TEXT)
            .map(|l| l.1.clone())
            .collect()
    }

    /// Minimal stays the smallest layout, whatever it shows: in the same
    /// corner it covers a part of what Standard covers (never wider, always
    /// shorter), all its text on its backing; the deviations are Standard's
    /// lines.
    #[test]
    fn minimal_is_no_bigger_than_standard() {
        let slowed = stats::FrameSummary { speed: 0.42, probes_per_s: 30.0, probe_ms: 1.25, cap_ms: 2.5, ..summary() };
        let none: Vec<String> = Vec::new();
        let many = many_deviations();
        for (f, devs) in [(summary(), &none), (summary(), &many), (slowed, &none), (slowed, &many)] {
            let minimal = painted(Layout::Minimal, f, devs);
            let standard = painted(Layout::Standard, f, devs);
            let (min, max, _) = backing(&minimal);
            assert_eq!((min, max), (Vec2::ZERO, minimal.size), "text outside the backing");
            assert!(
                minimal.size.x <= standard.size.x && minimal.size.y < standard.size.y,
                "{} {}",
                minimal.size,
                standard.size
            );
            assert_eq!(deviation_lines(&texts(&minimal)), deviation_lines(&texts(&standard)));
        }
        let lines = deviation_lines(&texts(&painted(Layout::Minimal, summary(), &many)));
        assert!(lines.len() > 3 && lines.iter().all(|l| l.len() <= DEVIATIONS_WRAP + 1), "{lines:?}");
        assert_eq!(lines.join(" "), many.join(", "));
        // No deviations: "none".
        assert_eq!(deviation_lines(&texts(&painted(Layout::Minimal, summary(), &none))), ["none"]);
    }

    /// A slowed game: the warning in red across both columns, between the
    /// frame rows and the deviations.
    #[test]
    fn minimal_warns_in_red_when_slowed() {
        let f = stats::FrameSummary { speed: 0.75, ..summary() };
        let m = texts(&painted(Layout::Minimal, f, &[]));
        let warn: Vec<_> = m.iter().filter(|t| t.3 == colour::WARN).collect();
        assert_eq!(warn.len(), 1);
        assert_eq!(warn[0].1, "game slowed to 75% speed (frames too long)");
        assert_eq!(warn[0].0.x, m[0].0.x);
        assert_eq!(m[10].1, warn[0].1);
        assert_eq!(m[11].1, "Deviations");
        assert!(texts(&painted(Layout::Minimal, summary(), &[])).iter().all(|t| t.3 != colour::WARN));
    }
}
