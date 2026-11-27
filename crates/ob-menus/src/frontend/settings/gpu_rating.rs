//! The game's graphics-card rating and the first-run detail estimate.

/// The graphics cards the game rates (0x8005C0, 12-byte rows: PCI vendor,
/// device, score 0–5; 0x5C0CE0 takes the first match, 1 for any other).
static CARD_SCORES: crate::exe_tables::Table<[(u32, u32, u8); 120]> = crate::exe_tables::Table::new(|t| &t.card_scores);

/// The detail level the game picks on a fresh run and on "Default
/// Settings" (0x5CECA0 → 0x5CDE50): 0.2 × the card's score + 0.8 × the
/// CPU's tier (0x5CDD00's clock, MHz: below 1400 1, 1800 2, 2400 3, else
/// 4), truncated; 0 on XGI (0x18CA), Intel (0x8086) and the ATI 0x5834;
/// then at most 3 on the NVIDIA 0x0322 / 0x0312, and 0 on the SiS 0x6330.
pub fn estimate_detail(vendor: u32, device: u32, cpu_mhz: u32) -> u8 {
    let card = CARD_SCORES.iter().find(|r| r.0 == vendor && r.1 == device).map_or(1.0, |r| f32::from(r.2));
    let cpu = match cpu_mhz {
        m if m < 1400 => 1.0,
        m if m < 1800 => 2.0,
        m if m < 2400 => 3.0,
        _ => 4.0,
    };
    let excluded = vendor == 0x18CA || vendor == 0x8086 || (vendor == 0x1002 && device == 0x5834);
    let mut detail = if excluded { 0 } else { (card * 0.2 + cpu * 0.8) as u8 };
    if vendor == 0x10DE && (device == 0x0322 || device == 0x0312) && detail > 3 {
        detail = 3;
    }
    if vendor == 0x1039 && device == 0x6330 {
        detail = 0;
    }
    detail
}

/// A card's score in the game's table (0x5C0CE0: the first match, 1 for
/// any other card).
pub fn card_score(vendor: u32, device: u32) -> u8 {
    CARD_SCORES.iter().find(|r| r.0 == vendor && r.1 == device).map_or(1, |r| r.2)
}

/// The CPU clock the estimate reads. The game times the processor
/// (0x5CDD00 → 0x5C0AE0: RDTSC ticks over a Sleep(50) by the
/// QueryPerformanceCounter time, so the TSC rate, the base clock on
/// current CPUs, not the boost clock) and only for the CPUID vendors
/// "GenuineIntel" / "AuthenticAMD" — any other leaves 0, tier 1
/// (A-frontend-1 3-002 (d), certain for the code). The clone reads Linux
/// cpufreq: `base_frequency` first, else the maximum clock; with neither,
// prov: inferred frontend.settings.cpu_mhz
/// the top tier (**inferred**; how close either file is to the TSC rate
/// depends on the CPU and its driver).
pub fn cpu_mhz() -> u32 {
    let vendor = std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|s| {
        s.lines().find(|l| l.starts_with("vendor_id")).and_then(|l| l.split(':').nth(1)).map(|v| v.trim().to_string())
    });
    if vendor.as_deref().is_some_and(|v| v != "GenuineIntel" && v != "AuthenticAMD") {
        return 0;
    }
    let khz = |f: &str| {
        std::fs::read_to_string(format!("/sys/devices/system/cpu/cpu0/cpufreq/{f}")).ok().and_then(|s| s.trim().parse::<u32>().ok())
    };
    khz("base_frequency").or_else(|| khz("cpuinfo_max_freq")).map_or(2400, |khz| khz / 1000)
}
