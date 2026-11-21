//! `game --probe` (docs/launcher.md): the facts the launcher needs before
//! the game's first run — the graphics adapter the game would pick, the
//! display modes it would offer, the display values its first start would
//! choose for this hardware, its version — as stable `key=value` lines.
//! The game fills a [`Probe`] and prints [`Probe::to_text`]; the launcher
//! runs it in the background (`launcher::probe::spawn`) and reads it back
//! with [`Probe::parse`]. One definition of the format for both sides.
//!
//! ```text
//! probe=1
//! version=Overbright 0.5.1 (commit unknown, linux-x86_64)
//! adapter.name=Example GPU (driver name)
//! adapter.backend=Vulkan
//! adapter.type=DiscreteGpu
//! adapter.pci=1002:1234
//! adapter.driver=radv Mesa 25.2.0
//! adapter.vram=17163091968
//! gpu.class=High
//! monitor=2560x1440@143.912
//! modes=640x480,800x600,…,2560x1440
//! settings.file=0
//! first_run.resolution=1280x960
//! first_run.fullscreen=0
//! first_run.detail=4
//! first_run.custom=3,1,2,…
//! ok=1
//! ```
//! Unknown keys are ignored (later fields stay readable by older
//! launchers). `monitor.modes` (the monitor's own video modes) and
//! `monitor.exclusive` (0 on Wayland: no exclusive full screen) give the
//! launcher the facts the game's Resolution row is built from
//! (`display::modes_for`); `first_run.*` gives the first start's display
//! values (`first_run_display`, from the engine's graphics card table). Without an adapter the probe prints `error=<text>` and
//! `ok=0` and exits 1; without a display `monitor=none` (the modes are then
//! the fixed list).

/// The format's version (`probe=`).
pub const FORMAT: u32 = 1;

/// The adapter the game would pick (`Gpu::adapter_options`, no surface).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Adapter {
    pub name: String,
    /// wgpu's backend name (`Vulkan`, `Dx12`, `Metal`, `Gl`).
    pub backend: String,
    /// wgpu's device type (`DiscreteGpu`, `IntegratedGpu`, `Cpu`, …).
    pub kind: String,
    /// PCI vendor and device ids.
    pub vendor: u32,
    pub device: u32,
    pub driver: String,
    /// Video memory in bytes (0 = unknown).
    pub vram: u64,
}

/// What the game's first start would choose (`Settings::default()` +
/// `first_run_display(card)`; 0x5CECA0's detail estimate).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FirstRun {
    pub resolution: (u32, u32),
    pub fullscreen: bool,
    pub detail: u8,
    pub custom: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Probe {
    pub version: String,
    pub adapter: Option<Adapter>,
    /// The presets' GPU class name (`qol::GpuClass::name`).
    pub gpu_class: String,
    /// The primary monitor's size and refresh rate (mHz), if any.
    pub monitor: Option<(u32, u32, u32)>,
    /// The monitor's own video modes (sizes, sorted, any refresh rate).
    pub monitor_modes: Vec<(u32, u32)>,
    /// Exclusive full screen exists (false on Wayland).
    pub exclusive: bool,
    /// The Resolution row's list (`display::mode_list`).
    pub modes: Vec<(u32, u32)>,
    /// options.txt existed when the probe ran.
    pub settings_file: bool,
    /// Whether the cache folder holds a `game --prepare` run for this
    /// build, adapter and driver (None: a game that does not say).
    pub prepared: Option<bool>,
    /// The settings' program key in that stamp (`graphics_table::program_key`;
    /// None: no stamp for this build, adapter and driver), to compare with
    /// the settings after the launcher's Graphics row changed them.
    pub prepared_programs: Option<u64>,
    pub first_run: Option<FirstRun>,
    pub error: Option<String>,
}

/// The display values the game's first start chooses for `card` (PCI
/// vendor, device): `Settings::default()` + `first_run_display(card)`, as
/// `create_window` does without options.txt. The probe prints these; the
/// launcher's first write (`model::save_choices`) starts from the same.
pub fn first_run(card: (u32, u32)) -> FirstRun {
    let mut s = crate::frontend::options::Settings::default();
    s.first_run_display(card);
    FirstRun { resolution: s.resolution, fullscreen: s.fullscreen, detail: s.detail, custom: s.custom.to_vec() }
}

fn size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

/// One line's value: no line breaks (they would end the line).
fn clean(s: &str) -> String {
    s.chars().map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect()
}

impl Probe {
    /// The PCI ids the first-run estimate takes.
    pub fn card(&self) -> Option<(u32, u32)> {
        self.adapter.as_ref().map(|a| (a.vendor, a.device))
    }

    /// The monitor as the game's Resolution row sees it
    /// (`display::modes_for`), if the probe found one.
    pub fn monitor_info(&self) -> Option<crate::render::display::MonitorInfo> {
        self.monitor.map(|(w, h, _)| crate::render::display::MonitorInfo {
            name: None,
            desktop: (w, h),
            modes: self.monitor_modes.clone(),
            exclusive: self.exclusive,
        })
    }

    pub fn ok(&self) -> bool {
        self.error.is_none() && self.adapter.is_some()
    }

    /// The launcher's GPU line: "Example GPU, Vulkan".
    pub fn gpu_line(&self) -> Option<String> {
        self.adapter.as_ref().map(|a| {
            let kind = match a.kind.as_str() {
                "IntegratedGpu" => ", built-in graphics",
                "Cpu" => ", software: slow",
                _ => "",
            };
            // The driver's name often ends in its own brackets
            // ("… (RADV NAVI32)"): the backend after a comma.
            format!("{}, {}{kind}", a.name, a.backend)
        })
    }

    pub fn to_text(&self) -> String {
        let mut s = format!("probe={FORMAT}\nversion={}\n", clean(&self.version));
        if let Some(a) = &self.adapter {
            s += &format!(
                "adapter.name={}\nadapter.backend={}\nadapter.type={}\nadapter.pci={:04x}:{:04x}\nadapter.driver={}\nadapter.vram={}\n",
                clean(&a.name),
                a.backend,
                a.kind,
                a.vendor,
                a.device,
                clean(&a.driver),
                a.vram
            );
        }
        if !self.gpu_class.is_empty() {
            s += &format!("gpu.class={}\n", self.gpu_class);
        }
        match self.monitor {
            Some((w, h, mhz)) => {
                s += &format!("monitor={w}x{h}@{}.{:03}\n", mhz / 1000, mhz % 1000);
                let mm: Vec<String> = self.monitor_modes.iter().map(|(w, h)| format!("{w}x{h}")).collect();
                s += &format!("monitor.modes={}\nmonitor.exclusive={}\n", mm.join(","), self.exclusive as u8);
            }
            None => s += "monitor=none\n",
        }
        let modes: Vec<String> = self.modes.iter().map(|(w, h)| format!("{w}x{h}")).collect();
        s += &format!("modes={}\nsettings.file={}\n", modes.join(","), self.settings_file as u8);
        if let Some(f) = &self.first_run {
            let custom: Vec<String> = f.custom.iter().map(u8::to_string).collect();
            s += &format!(
                "first_run.resolution={}x{}\nfirst_run.fullscreen={}\nfirst_run.detail={}\nfirst_run.custom={}\n",
                f.resolution.0,
                f.resolution.1,
                f.fullscreen as u8,
                f.detail,
                custom.join(",")
            );
        }
        if let Some(p) = self.prepared {
            s += &format!("prepared={}\n", p as u8);
        }
        if let Some(k) = self.prepared_programs {
            s += &format!("prepared.programs={k:016x}\n");
        }
        if let Some(e) = &self.error {
            s += &format!("error={}\n", clean(e));
        }
        s += &format!("ok={}\n", self.ok() as u8);
        s
    }

    /// Reads the probe's output (lines other than `key=value` and unknown
    /// keys are skipped: the game may log to stdout before). Err when the
    /// output holds no `probe=` line or a newer format.
    pub fn parse(text: &str) -> Result<Probe, String> {
        let mut p = Probe::default();
        let mut seen = false;
        let mut first = FirstRun::default();
        let mut has_first = false;
        let mut has_exclusive = false;
        for line in text.lines() {
            let Some((k, v)) = line.trim_end().split_once('=') else { continue };
            let adapter = || -> Adapter { Adapter::default() };
            match k {
                "probe" => {
                    let n: u32 = v.parse().map_err(|_| format!("probe format {v:?}"))?;
                    if n > FORMAT {
                        return Err(format!("the game's probe format {n} is newer than this launcher's {FORMAT}"));
                    }
                    seen = true;
                }
                "version" => p.version = v.into(),
                "adapter.name" => p.adapter.get_or_insert_with(adapter).name = v.into(),
                "adapter.backend" => p.adapter.get_or_insert_with(adapter).backend = v.into(),
                "adapter.type" => p.adapter.get_or_insert_with(adapter).kind = v.into(),
                "adapter.driver" => p.adapter.get_or_insert_with(adapter).driver = v.into(),
                "adapter.vram" => p.adapter.get_or_insert_with(adapter).vram = v.parse().unwrap_or(0),
                "adapter.pci" => {
                    let (a, b) = v.split_once(':').ok_or(format!("adapter.pci {v:?}"))?;
                    let ad = p.adapter.get_or_insert_with(adapter);
                    ad.vendor = u32::from_str_radix(a, 16).map_err(|_| format!("adapter.pci {v:?}"))?;
                    ad.device = u32::from_str_radix(b, 16).map_err(|_| format!("adapter.pci {v:?}"))?;
                }
                "gpu.class" => p.gpu_class = v.into(),
                "monitor" if v == "none" => p.monitor = None,
                "monitor" => {
                    let (sz, hz) = v.split_once('@').unwrap_or((v, "0"));
                    let (w, h) = size(sz).ok_or(format!("monitor {v:?}"))?;
                    let (i, f) = hz.split_once('.').unwrap_or((hz, "0"));
                    let f = format!("{f:0<3}");
                    let mhz = i.parse::<u32>().unwrap_or(0) * 1000 + f[..3].parse::<u32>().unwrap_or(0);
                    p.monitor = Some((w, h, mhz));
                }
                "monitor.modes" => p.monitor_modes = v.split(',').filter_map(size).collect(),
                "monitor.exclusive" => {
                    p.exclusive = v == "1";
                    has_exclusive = true;
                }
                "modes" => p.modes = v.split(',').filter_map(size).collect(),
                "settings.file" => p.settings_file = v == "1",
                "prepared" => p.prepared = Some(v == "1"),
                "prepared.programs" => p.prepared_programs = u64::from_str_radix(v, 16).ok(),
                "first_run.resolution" => {
                    first.resolution = size(v).ok_or(format!("first_run.resolution {v:?}"))?;
                    has_first = true;
                }
                "first_run.fullscreen" => first.fullscreen = v == "1",
                "first_run.detail" => first.detail = v.parse().map_err(|_| format!("first_run.detail {v:?}"))?,
                "first_run.custom" => {
                    first.custom = v.split(',').filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect()
                }
                "error" => p.error = Some(v.into()),
                _ => {}
            }
        }
        if !seen {
            return Err("the game printed no probe result".into());
        }
        if has_first {
            p.first_run = Some(first);
        }
        // An older game's probe (no monitor facts): the fixed list's
        // monitor sizes, exclusive full screen assumed (not on Wayland).
        if !has_exclusive {
            p.exclusive = true;
            if p.monitor_modes.is_empty() {
                p.monitor_modes = p.modes.clone();
            }
        }
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The output of a real `game --probe` (Xvfb, lavapipe), kept as text.
    const CAPTURED: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/launcher/probe-sample.txt"));

    #[test]
    fn a_captured_probe_parses() {
        let p = Probe::parse(CAPTURED).unwrap();
        assert!(p.ok(), "{p:?}");
        let a = p.adapter.as_ref().unwrap();
        assert!(!a.name.is_empty() && !a.backend.is_empty());
        assert!(p.card().is_some_and(|(v, _)| v != 0));
        assert!(!p.modes.is_empty());
        let f = p.first_run.as_ref().unwrap();
        assert_eq!(f.custom.len(), 22);
        assert!(p.gpu_line().unwrap().contains(&a.name));
    }

    #[test]
    fn written_and_read_back() {
        let p = Probe {
            version: format!("Overbright {} (commit unknown, linux-x86_64)", env!("CARGO_PKG_VERSION")),
            adapter: Some(Adapter {
                name: "Some GPU\nwith a break".into(),
                backend: "Vulkan".into(),
                kind: "DiscreteGpu".into(),
                vendor: 0x1002,
                device: 0x1234,
                driver: "drv 1.0".into(),
                vram: 17 << 30,
            }),
            gpu_class: "High".into(),
            monitor: Some((2560, 1440, 143_912)),
            monitor_modes: vec![(1920, 1080), (2560, 1440)],
            exclusive: false,
            modes: vec![(640, 480), (2560, 1440)],
            settings_file: false,
            prepared: Some(false),
            prepared_programs: Some(0x0123_4567_89ab_cdef),
            first_run: Some(FirstRun { resolution: (1280, 960), fullscreen: false, detail: 4, custom: vec![1; 22] }),
            error: None,
        };
        let text = format!("some log line\n{}", p.to_text());
        let back = Probe::parse(&text).unwrap();
        let mut want = p.clone();
        want.adapter.as_mut().unwrap().name = "Some GPU with a break".into();
        assert_eq!(back, want);
        // No adapter: an error, not ok.
        let e = Probe { error: Some("no GPU adapter".into()), ..Default::default() };
        let back = Probe::parse(&e.to_text()).unwrap();
        assert!(!back.ok() && back.error.as_deref() == Some("no GPU adapter") && back.monitor.is_none());
        // Not a probe, or a newer one.
        assert!(Probe::parse("hello\n").is_err());
        assert!(Probe::parse("probe=99\n").is_err());
    }
}
