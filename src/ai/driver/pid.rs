/// A mean of the last n samples (vtable 0x7A0610, constructor 0x579F30,
/// push 0x576980).
#[derive(Clone, Debug)]
pub struct RunningMean {
    pub(super) samples: Vec<f32>,
    pub(super) count: usize,
    pub(super) idx: usize,
    pub(super) sum: f32,
    /// +0x0C, the mean.
    pub mean: f32,
}

impl RunningMean {
    pub fn new(n: usize) -> RunningMean {
        RunningMean { samples: vec![0.0; n], count: 0, idx: 0, sum: 0.0, mean: 0.0 }
    }

    pub fn push(&mut self, v: f32) {
        if self.count < self.samples.len() {
            self.count += 1;
        }
        self.sum = (v + self.sum) - self.samples[self.idx];
        self.samples[self.idx] = v;
        self.mean = self.sum / self.count as f32;
        self.idx += 1;
        if self.idx >= self.samples.len() {
            self.idx = 0;
        }
    }

    /// 0x5769E0: every sample becomes `v`, the count 0, the sum n·v.
    pub(super) fn fill(&mut self, v: f32) {
        self.samples.iter_mut().for_each(|s| *s = v);
        self.count = 0;
        self.sum = self.samples.len() as f32 * v;
    }
}

/// A mean over the samples of the last `window` seconds (vtable 0x7A0618,
/// constructor 0x579F90: room for trunc(rate × window + 0.5) samples; push
/// 0x576AB0). A slot holding 0 at time 0 is empty.
#[derive(Clone, Debug)]
pub(super) struct WindowMean {
    pub(super) samples: Vec<f32>,
    pub(super) times: Vec<f32>,
    pub(super) count: usize,
    pub(super) idx: usize,
    pub(super) oldest: usize,
    pub(super) sum: f32,
    pub(super) mean: f32,
    pub(super) window: f32,
}

impl WindowMean {
    pub(super) fn new(window: f32, rate: f32) -> WindowMean {
        let n = (window * rate + 0.5) as usize;
        WindowMean {
            samples: vec![0.0; n],
            times: vec![0.0; n],
            count: 0,
            idx: 0,
            oldest: 0,
            sum: 0.0,
            mean: 0.0,
            window,
        }
    }

    pub(super) fn push(&mut self, v: f32, t: f32) {
        let i = self.idx;
        if self.samples[i] == 0.0 && self.times[i] == 0.0 {
            self.count += 1;
        } else {
            self.sum -= self.samples[i];
        }
        self.sum += v;
        self.samples[i] = v;
        self.times[i] = t;
        while self.window < t - self.times[self.oldest] {
            let o = self.oldest;
            if self.times[o] > 0.0 {
                self.sum -= self.samples[o];
                self.samples[o] = 0.0;
                self.times[o] = 0.0;
                self.count -= 1;
            }
            self.oldest = (o + 1) % self.samples.len();
        }
        self.idx += 1;
        self.mean = self.sum / self.count as f32;
        if self.idx >= self.samples.len() {
            self.idx = 0;
        }
    }
}

/// An error and its running integral and rate (0x84 bytes, constructor
/// 0x4016B0(n1, n2, k), update 0x576BC0).
#[derive(Clone, Debug)]
pub(super) struct Tracker {
    /// The frame times (n1), the trapezoid areas (n1), the rates (n2).
    pub(super) dts: RunningMean,
    pub(super) areas: RunningMean,
    pub(super) rates: RunningMean,
    pub(super) k: f32,
    pub(super) cur: f32,
    pub(super) prev: f32,
}

impl Tracker {
    pub(super) fn new(n1: usize, n2: usize, k: f32) -> Tracker {
        Tracker {
            dts: RunningMean::new(n1),
            areas: RunningMean::new(n1),
            rates: RunningMean::new(n2),
            k,
            cur: 0.0,
            prev: 0.0,
        }
    }

    pub(super) fn update(&mut self, v: f32, dt: f32, no_rate: bool, no_area: bool) {
        self.prev = self.cur;
        let d = v - self.prev;
        self.cur = v;
        let area = if no_area { 0.0 } else { (d * 0.5 + self.prev) * dt };
        let rate = if no_rate { 0.0 } else { d / dt };
        self.dts.push(dt);
        self.areas.push(area);
        self.rates.push(rate);
    }

    /// The integral term: the areas' sum × their count over the times'
    /// sum × k (0 before the first area).
    pub(super) fn integral(&self) -> f32 {
        if self.areas.count == 0 {
            return 0.0;
        }
        self.areas.sum * self.areas.count as f32 / (self.dts.sum * self.k)
    }

    pub(super) fn rate(&self) -> f32 {
        self.rates.mean
    }

    /// 0x57A020: the value 0, the previous `v`, areas and rates filled
    /// with `v` and emptied.
    pub(super) fn reset(&mut self, v: f32) {
        self.cur = 0.0;
        self.prev = v;
        self.areas.fill(v);
        self.rates.fill(v);
    }

    /// 0x576C50: as [`Tracker::reset`] but the areas kept.
    pub(super) fn reset_rate(&mut self, v: f32) {
        self.cur = 0.0;
        self.prev = v;
        self.rates.fill(v);
    }
}

/// The game's self-tuning PID (AdaptivePIDControllerBase, vtable
/// 0x7845E8, constructor 0x409B50(mode 0, period)): output 0x409DA0 =
/// Σ Kᵢ·clamp(termᵢ); every `period` one gain in turn (round robin) is
/// stepped by −q·rate·x, x being how far the controlled behaviour is from
/// what it should be and q its sensitivity to that gain (0x4182C0,
/// 0x411390, 0x409EA0).
#[derive(Clone, Debug)]
pub(super) struct AdaptivePid {
    pub(super) period: f32,
    pub(super) term_limits: [(f32, f32); 3],
    pub(super) bounds: [(f32, f32); 3],
    pub(super) thresholds: [f32; 3],
    pub(super) rates: [f32; 3],
    pub(super) gains: [f32; 3],
    /// Each gain's recent steps, and the rate of x (windows of `period`
    /// with room for 45 samples a second).
    pub(super) steps: [WindowMean; 3],
    pub(super) last_x: f32,
    pub(super) dx: WindowMean,
    pub(super) terms: [f32; 3],
}

impl AdaptivePid {
    pub(super) fn new(period: f32) -> AdaptivePid {
        let window = || WindowMean::new(period, 45.0);
        AdaptivePid {
            period,
            term_limits: [(-1e5, 1e5); 3],
            bounds: [(0.0, 1.0); 3],
            thresholds: [0.01; 3],
            rates: [5e-5; 3],
            gains: [0.0; 3],
            steps: [window(), window(), window()],
            last_x: 0.0,
            dx: window(),
            terms: [0.0; 3],
        }
    }

    pub(super) fn output(&self) -> f32 {
        (0..3).map(|i| self.gains[i] * self.terms[i].clamp(self.term_limits[i].0, self.term_limits[i].1)).sum()
    }

    pub(super) fn adapt(&mut self, x: f32, dt: f32, now: f64) {
        let t = now as f32;
        // (A negative race clock — the countdown — still picks 0..3.)
        let i = ((now / self.period as f64) as i64).rem_euclid(3) as usize;
        let mut g = 0.0;
        if self.terms[i].abs() >= self.thresholds[i] {
            // 0x409EA0: how x moves with this gain lately.
            let (m, s) = (self.dx.mean, self.steps[i].mean);
            let q = if s.abs() > 1e-9 {
                (m / s).clamp(-1000.0, 1000.0)
            } else if m.abs() < 0.001 {
                0.0
            } else {
                m.signum()
            };
            g = -q * self.rates[i] * x;
            self.gains[i] = (self.gains[i] + g * dt).max(self.bounds[i].0).min(self.bounds[i].1);
        }
        for j in 0..3 {
            self.steps[j].push(if j == i { g } else { 0.0 }, t);
        }
        self.dx.push((x - self.last_x) / dt.max(0.001), t);
        self.last_x = x;
    }
}
