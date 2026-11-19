//! GPU timings and memory reports (wgpu): the renderer's side of `prof`
//! (see `prof::gpu` there).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// What the timestamps need: queries, and writes between passes.
pub const FEATURES: wgpu::Features =
    wgpu::Features::TIMESTAMP_QUERY.union(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS);
/// The label of an encoder's first mark.
pub const GAP: &str = "(between submissions)";
const MAX: u32 = 256;
/// Frames whose timestamps can be in flight at once.
pub const RING: usize = 4;

static ON: AtomicBool = AtomicBool::new(false);
/// Ask for [`FEATURES`] when the device is made (`render::Gpu::new`).
static WANTED: AtomicBool = AtomicBool::new(false);

struct Slot {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    /// The marks written into `set`, in the order made.
    labels: Vec<&'static str>,
    /// Being read back: the frame's number, its submission, and set
    /// once the buffer is mapped.
    pending: Option<(u64, wgpu::SubmissionIndex, Arc<AtomicBool>)>,
}

struct State {
    slots: Vec<Slot>,
    /// The slot this frame's marks go to.
    current: usize,
    /// Frames ended so far.
    frame: u64,
    /// ns per tick.
    period: f64,
    /// Per label: total ms, frames.
    totals: Vec<(&'static str, f64, u64)>,
    /// Per frame read back: its number, the GPU's busy time (the
    /// parts, without the gaps between submissions), ms.
    frames: Vec<(u64, f64)>,
    /// The parts of the frame read last.
    last: Vec<&'static str>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

pub fn want(on: bool) {
    WANTED.store(on, Ordering::Relaxed);
}

pub fn wanted() -> bool {
    WANTED.load(Ordering::Relaxed)
}

/// Start timing on `device` (made with [`FEATURES`]); false if it lacks them.
pub fn init(device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
    if !device.features().contains(FEATURES) {
        return false;
    }
    let size = u64::from(MAX) * 8;
    let slots = (0..RING)
        .map(|_| Slot {
            set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("gpu timing"),
                ty: wgpu::QueryType::Timestamp,
                count: MAX,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu timing"),
                size,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            read: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu timing"),
                size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            labels: Vec::new(),
            pending: None,
        })
        .collect();
    *STATE.lock().unwrap() = Some(State {
        slots,
        current: 0,
        frame: 0,
        period: f64::from(queue.get_timestamp_period()),
        totals: Vec::new(),
        frames: Vec::new(),
        last: Vec::new(),
    });
    ON.store(true, Ordering::Relaxed);
    true
}

/// A timestamp after what `encoder` holds so far, closing part `label`.
pub fn mark(encoder: &mut wgpu::CommandEncoder, label: &'static str) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let mut state = STATE.lock().unwrap();
    let Some(s) = state.as_mut() else { return };
    let slot = &mut s.slots[s.current];
    let i = slot.labels.len() as u32;
    if i < MAX {
        encoder.write_timestamp(&slot.set, i);
        slot.labels.push(label);
    }
}

/// After the frame's last submission: resolve its timestamps and start
/// reading them back, without waiting; add the frames read back by
/// now to the totals. Returns the parts of the latest frame read.
pub fn end_frame(device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<&'static str> {
    if !ON.load(Ordering::Relaxed) {
        return Vec::new();
    }
    let mut state = STATE.lock().unwrap();
    let Some(s) = state.as_mut() else { return Vec::new() };
    let frame = s.frame;
    s.frame += 1;
    let cur = s.current;
    let n = s.slots[cur].labels.len() as u32;
    if n > 0 {
        let slot = &mut s.slots[cur];
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.resolve_query_set(&slot.set, 0..n, &slot.resolve, 0);
        encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.read, 0, u64::from(n) * 8);
        let index = queue.submit([encoder.finish()]);
        let done = Arc::new(AtomicBool::new(false));
        let flag = done.clone();
        slot.read.slice(..u64::from(n) * 8).map_async(wgpu::MapMode::Read, move |r| {
            if r.is_ok() {
                flag.store(true, Ordering::Release);
            }
        });
        slot.pending = Some((frame, index, done));
    }
    // The next frame's set: free once its read-back is in (wait for it
    // only if not yet).
    s.current = (cur + 1) % RING;
    let _ = device.poll(wgpu::PollType::Poll);
    if let Some((_, index, done)) = &s.slots[s.current].pending
        && !done.load(Ordering::Acquire)
    {
        let _ = device.poll(wgpu::PollType::Wait { submission_index: Some(index.clone()), timeout: None });
    }
    harvest(s);
    s.slots[s.current].labels.clear();
    s.last.clone()
}

/// Wait for every frame's read-back still under way and add them in.
pub fn finish(device: &wgpu::Device) {
    if !ON.load(Ordering::Relaxed) {
        return;
    }
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    let mut state = STATE.lock().unwrap();
    if let Some(s) = state.as_mut() {
        harvest(s);
    }
}

/// Reads every slot whose map has completed, oldest frame first.
fn harvest(s: &mut State) {
    let mut ready: Vec<usize> = (0..s.slots.len())
        .filter(|&i| s.slots[i].pending.as_ref().is_some_and(|p| p.2.load(Ordering::Acquire)))
        .collect();
    ready.sort_by_key(|&i| s.slots[i].pending.as_ref().unwrap().0);
    for i in ready {
        let slot = &mut s.slots[i];
        let (frame, _, _) = slot.pending.take().unwrap();
        let n = slot.labels.len();
        let ticks: Vec<u64> = slot
            .read
            .slice(..n as u64 * 8)
            .get_mapped_range()
            .expect("mapped timestamps")
            .chunks_exact(8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
            .collect();
        slot.read.unmap();
        // In the order the GPU wrote them (marks are made on several
        // threads, into command buffers submitted in order).
        let mut marks: Vec<(u64, &'static str)> = ticks.iter().copied().zip(std::mem::take(&mut slot.labels)).collect();
        marks.sort_by_key(|m| m.0);
        let mut parts: Vec<&'static str> = marks.iter().map(|m| m.1).filter(|&l| l != GAP).collect();
        parts.dedup();
        // Per label, this frame's sum (a label may close several parts).
        let mut sums: Vec<(&'static str, f64)> = Vec::new();
        for i in 1..marks.len() {
            let ms = marks[i].0.saturating_sub(marks[i - 1].0) as f64 * s.period / 1e6;
            match sums.iter_mut().find(|e| e.0 == marks[i].1) {
                Some(e) => e.1 += ms,
                None => sums.push((marks[i].1, ms)),
            }
        }
        let busy = sums.iter().filter(|e| e.0 != GAP).map(|e| e.1).sum();
        s.frames.push((frame, busy));
        for (label, ms) in sums {
            match s.totals.iter_mut().find(|e| e.0 == label) {
                Some(e) => {
                    e.1 += ms;
                    e.2 += 1;
                }
                None => s.totals.push((label, ms, 1)),
            }
        }
        s.last = parts;
    }
}

/// Per part: its total ms and the frames it ran in, cleared.
pub fn take() -> Vec<(&'static str, f64, u64)> {
    STATE.lock().unwrap().as_mut().map(|s| std::mem::take(&mut s.totals)).unwrap_or_default()
}

/// Per frame read back since the last call: the GPU's busy time, ms,
/// in frame order.
pub fn take_frames() -> Vec<f64> {
    STATE
        .lock()
        .unwrap()
        .as_mut()
        .map(|s| {
            let mut f = std::mem::take(&mut s.frames);
            f.sort_by_key(|e| e.0);
            f.into_iter().map(|e| e.1).collect()
        })
        .unwrap_or_default()
}

/// Drop the current frame's marks unread (e.g. frames not timed).
pub fn discard() {
    if let Some(s) = STATE.lock().unwrap().as_mut() {
        let cur = s.current;
        s.slots[cur].labels.clear();
    }
}

/// the performance overlay's Full layout: timing was started
/// ([`init`]) by someone.
pub fn ready() -> bool {
    STATE.lock().unwrap().is_some()
}

/// The marks are written ([`init`] made, not paused).
pub fn active() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Pause or resume timing after [`init`] (the overlay's Full layout
/// hidden / shown): paused, `mark` and `end_frame` do nothing. On
/// resuming, the marks left from the frame paused in are dropped.
pub fn set_active(on: bool) {
    if on && ready() {
        discard();
        ON.store(true, Ordering::Relaxed);
    } else if !on {
        ON.store(false, Ordering::Relaxed);
    }
}

/// docs/perf-gpu.md "Render-target memory": the device's allocator
/// report (every live texture and buffer by its label, as wgpu-hal's
/// gpu-allocator holds them) grouped by label, largest first: count, MiB,
/// then the totals. `NFSU2_VRAM_REPORT=file` writes it at a bench's end; a
/// measuring aid, read once, nothing when unset. None where the backend
/// keeps no report.
pub fn allocator_report(device: &wgpu::Device) -> Option<String> {
    let r = device.generate_allocator_report()?;
    let mut by: Vec<(String, u64, u64)> = Vec::new();
    for a in &r.allocations {
        match by.iter_mut().find(|e| e.0 == a.name) {
            Some(e) => {
                e.1 += 1;
                e.2 += a.size;
            }
            None => by.push((a.name.clone(), 1, a.size)),
        }
    }
    by.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    let mib = |b: u64| b as f64 / (1024.0 * 1024.0);
    let now = super::gpu_memory()
        .map_or(String::new(), |m| format!(", process VRAM now {} MiB, GTT {} MiB", m.0 / 1024, m.1 / 1024));
    let mut out = format!(
        "# allocated {:.1} MiB in {} allocations, reserved {:.1} MiB in {} blocks{now}\n",
        mib(r.total_allocated_bytes),
        r.allocations.len(),
        mib(r.total_reserved_bytes),
        r.blocks.len()
    );
    for (name, n, bytes) in by {
        out.push_str(&format!("{:10.2}\t{n}\t{name}\n", mib(bytes)));
    }
    Some(out)
}
