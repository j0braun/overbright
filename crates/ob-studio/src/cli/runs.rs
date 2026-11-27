//! The Runs workspace on the command line (`runs list`, `show`, `telemetry`, `compare`, `rename`, `delete`): each
//! reads or changes the run files of the mod (`.editor/runs/`) with the functions the window's Runs workspace
//! uses ([`crate::runs`]).

use serde_json::{Value, json};

use super::{Done, FAILED, Mod, OK, USAGE, args, fail};
use crate::runs::format::{CarKind, EventKind};
use crate::runs::library::{self, Entry};
use crate::runs::record::Recording;
use crate::runs::telemetry::{self, Line};

pub(super) fn run(name: &str, line: &args::Line, m: &Mod) -> Done {
    let dir = m.edir.join("runs");
    match name {
        "runs.list" => list(&dir),
        "runs.show" => with_run(line, &dir, "run", |n, rec| show(n, rec)),
        "runs.telemetry" => with_run(line, &dir, "run", |n, rec| telemetry(line, m, n, rec)),
        "runs.compare" => compare(line, &dir),
        "runs.rename" => rename(line, &dir),
        _ => delete(line, &dir),
    }
}

fn arg<'a>(line: &'a args::Line, key: &str) -> &'a str {
    line.args.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn car_of(line: &args::Line) -> u32 {
    line.args.get("car").and_then(Value::as_u64).unwrap_or(0) as u32
}

fn secs(t: f32) -> String {
    format!("{t:.1} s")
}

fn time_of(t: std::time::SystemTime) -> Option<u64> {
    t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_millis() as u64)
}

fn entry_json(e: &Entry) -> Value {
    json!({
        "run": e.name, "what": e.what, "seed": e.seed, "end": e.end, "bytes": e.bytes,
        "time_ms": e.modified.and_then(time_of),
    })
}

fn list(dir: &std::path::Path) -> Done {
    let runs = library::list(dir);
    let mut text: Vec<String> = runs
        .iter()
        .map(|e| {
            format!(
                "{:<28} {:<12} seed {:<6} {}",
                e.name,
                e.what,
                e.seed.map_or("?".to_string(), |s| s.to_string()),
                e.end.as_deref().unwrap_or("not ended")
            )
        })
        .collect();
    if runs.is_empty() {
        text.push("No runs yet".into());
    }
    Done { code: OK, text, doc: json!({ "count": runs.len(), "runs": runs.iter().map(entry_json).collect::<Vec<_>>() }) }
}

/// Read the run `--<key>` and hand it to `f`; the same checks as the window (a name that is not one, a run that
/// is not there).
fn with_run(line: &args::Line, dir: &std::path::Path, key: &str, f: impl FnOnce(&str, &Recording) -> Done) -> Done {
    let name = arg(line, key);
    match read(dir, name, key) {
        Ok(rec) => f(name, &rec),
        Err(d) => d,
    }
}

fn read(dir: &std::path::Path, name: &str, key: &str) -> Result<Recording, Done> {
    library::check_name(name).map_err(|e| fail(USAGE, format!("--{key}: {e}")))?;
    let path = library::path(dir, name);
    let text = std::fs::read_to_string(&path).map_err(|_| {
        let mut d = fail(FAILED, format!("no run called {name} (see `runs list`)"));
        d.doc["error"]["reason"] = json!("no_run");
        d
    })?;
    Ok(Recording::read(&text))
}

fn show(name: &str, rec: &Recording) -> Done {
    let (what, seed) = rec.header.as_ref().map_or(("?".to_string(), None), |h| (h.race.clone(), Some(h.seed)));
    let span = rec.span();
    let end_time = span.map_or(0.0, |s| s.1);
    let standings = rec.standings(end_time);
    let problems = rec.problems();
    let mut text = vec![format!(
        "{name}: {what}, seed {}, {}, {}",
        seed.map_or("?".into(), |s| s.to_string()),
        span.map_or("no frames".to_string(), |s| format!("{} to {}", secs(s.0), secs(s.1))),
        rec.end.as_deref().unwrap_or("not ended")
    )];
    for s in &standings {
        text.push(format!(
            "  {:>3}  {:<16} {}",
            if s.dnf { "dnf".to_string() } else { s.place.to_string() },
            rec.name(s.car),
            s.time.map_or(String::new(), secs)
        ));
    }
    for p in &problems {
        text.push(format!("  {}", p.text()));
    }
    let doc = json!({
        "run": name, "what": what, "seed": seed, "end": rec.end,
        "span": span.map(|s| json!({ "from": s.0, "to": s.1 })),
        "frames": rec.frames.len(),
        "cars": { "count": rec.cars.len(), "racers": rec.racers() },
        "standings": standings.iter().map(|s| json!({
            "car": s.car, "name": rec.name(s.car), "place": s.place, "time": s.time, "best": s.best,
            "laps": s.laps, "dnf": s.dnf,
        })).collect::<Vec<_>>(),
        "problems": problems.iter().map(|p| json!({
            "kind": kind_word(p.kind), "checkpoint": p.checkpoint, "cars": p.cars, "time": p.time, "text": p.text(),
        })).collect::<Vec<_>>(),
    });
    Done { code: OK, text, doc }
}

fn kind_word(k: EventKind) -> String {
    format!("{k:?}").to_ascii_lowercase()
}

fn telemetry(line: &args::Line, m: &Mod, name: &str, rec: &Recording) -> Done {
    let car = car_of(line);
    if !rec.cars.contains_key(&car) {
        let mut d = fail(FAILED, format!("run {name} has no car {car}"));
        d.doc["error"]["reason"] = json!("no_car");
        return d;
    }
    // The racing line comes from the race the run was made on, when the mod has it (as in the window).
    let race = rec.header.as_ref().map(|h| h.race.clone()).filter(|r| !r.is_empty());
    let map = crate::history::History::open_read(
        &m.dir,
        &m.edir,
        crate::MOD_DOCS,
        std::sync::Arc::new(crate::history::disk::Real),
    )
    .ok()
    .and_then(|h| ob_map::tools::map::format::MapMod::parse(h.doc(ob_map::tools::map::store::FILE)?).ok());
    let line_of = race
        .as_ref()
        .and_then(|r| map.as_ref()?.races.iter().find(|c| &c.id == r))
        .and_then(Line::of);
    let t = telemetry::of(rec, car, line_of.as_ref());
    let series = |s: &[(f32, f32)]| {
        let (lo, hi) = telemetry::range(s).unzip();
        let mean = (!s.is_empty()).then(|| s.iter().map(|p| p.1).sum::<f32>() / s.len() as f32);
        json!({ "samples": s.len(), "min": lo, "max": hi, "mean": mean })
    };
    let mut text = vec![format!("{name}, car {car} ({})", rec.name(car))];
    if let Some((lo, hi)) = telemetry::range(&t.speed) {
        text.push(format!("  speed {lo:.1} to {hi:.1} m/s over {} frames", t.speed.len()));
    }
    if let Some((lo, hi)) = t.lateral.as_deref().and_then(telemetry::range) {
        text.push(format!("  off the racing line {lo:.1} to {hi:.1} m"));
    }
    if let Some((lo, hi)) = t.gear.as_deref().and_then(telemetry::range) {
        text.push(format!("  gear {} to {}", gear_word(lo), gear_word(hi)));
    }
    if let Some((_, hi)) = t.throttle.as_deref().and_then(telemetry::range) {
        let full = t.throttle.as_deref().map_or(0, |s| s.iter().filter(|p| p.1 > 0.95).count());
        text.push(format!("  throttle up to {:.0} % ({full} of {} frames flat out)", hi * 100.0, t.speed.len()));
    }
    if let Some((_, hi)) = t.brake.as_deref().and_then(telemetry::range) {
        text.push(format!("  brake up to {:.0} %", hi * 100.0));
    }
    for (time, kind, cp) in &t.events {
        text.push(format!("  {:>7} {} (checkpoint {cp})", secs(*time), kind_word(*kind)));
    }
    let doc = json!({
        "run": name, "car": car, "name": rec.name(car), "player": rec.cars[&car].kind == CarKind::Player,
        "speed": series(&t.speed),
        "lateral": t.lateral.as_deref().map(series),
        "gear": t.gear.as_deref().map(series),
        "rpm": t.rpm.as_deref().map(series),
        "throttle": t.throttle.as_deref().map(series),
        "brake": t.brake.as_deref().map(series),
        "steer": t.steer.as_deref().map(series),
        "events": t.events.iter().map(|(time, kind, cp)| json!({
            "time": time, "kind": kind_word(*kind), "checkpoint": cp,
        })).collect::<Vec<_>>(),
        "series": {
            "speed": t.speed, "lateral": t.lateral, "gear": t.gear, "rpm": t.rpm, "throttle": t.throttle,
            "brake": t.brake, "steer": t.steer,
        },
    });
    Done { code: OK, text, doc }
}

fn gear_word(v: f32) -> String {
    match v.round() as i32 {
        -1 => "R".into(),
        0 => "N".into(),
        g => g.to_string(),
    }
}

fn compare(line: &args::Line, dir: &std::path::Path) -> Done {
    let car = car_of(line);
    let (a, b) = (arg(line, "run"), arg(line, "with"));
    let (ra, rb) = match (read(dir, a, "run"), read(dir, b, "with")) {
        (Ok(x), Ok(y)) => (x, y),
        (Err(d), _) | (_, Err(d)) => return d,
    };
    let p = telemetry::parting(&ra, &rb, car);
    let same = p.hash_tick.is_none() && p.apart.is_none();
    let text = vec![
        format!("{a} and {b}, car {car}"),
        match p.hash_tick {
            Some(t) => format!("  parts at tick {t}{}", p.time.map_or(String::new(), |t| format!(" (about {})", secs(t)))),
            None => "  the state hashes never differ".to_string(),
        },
        match p.apart {
            Some(t) => format!("  over a metre apart from {}", secs(t)),
            None => "  never over a metre apart".to_string(),
        },
    ];
    let doc = json!({
        "run": a, "with": b, "car": car, "same": same,
        "hash_tick": p.hash_tick, "time": p.time, "apart": p.apart,
    });
    Done { code: OK, text, doc }
}

fn rename(line: &args::Line, dir: &std::path::Path) -> Done {
    let (from, to) = (arg(line, "run"), arg(line, "to"));
    if let Err(e) = library::check_name(from).and_then(|_| library::check_name(to)) {
        return fail(USAGE, e);
    }
    match library::rename(dir, from, to) {
        Ok(()) => Done { code: OK, text: vec![format!("Renamed {from} to {to}")], doc: json!({ "run": from, "to": to }) },
        Err(e) => fail(FAILED, e),
    }
}

fn delete(line: &args::Line, dir: &std::path::Path) -> Done {
    let name = arg(line, "run");
    if let Err(e) = library::check_name(name) {
        return fail(USAGE, e);
    }
    if !library::path(dir, name).is_file() {
        return fail(FAILED, format!("no run called {name} (see `runs list`)"));
    }
    match library::delete(dir, name) {
        Ok(()) => Done { code: OK, text: vec![format!("Deleted {name}")], doc: json!({ "run": name }) },
        Err(e) => fail(FAILED, e),
    }
}
