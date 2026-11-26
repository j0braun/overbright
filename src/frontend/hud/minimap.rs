use crate::frontend::fe_ids;
use super::*;

/// 0x4CA840 for the north-up map (custom-HUD option 1; option 2 turns it
/// with the car): the map's texture window is 0.3 / zoom of the map wide,
/// centred on the player; markers sit at their offset from the player in
/// window units times the map's size, pulled in to 0.55 and dropped past 1.
pub(super) fn minimap(s: &mut ScreenState, package: &super::super::feng::Package, has_texture: &dyn Fn(u32) -> bool, m: &MapView) {
    let window = 0.3 / m.zoom.max(0.001);
    let to_map = |p: glam::Vec2| glam::Vec2::new((p.x - m.left) / m.size, (m.top - p.y) / m.size);
    let centre = to_map(m.player);
    let map = fe_hash("Track2000_map");
    if let Some(t) = m.texture {
        s.image.insert(map, t);
    }
    s.uv.insert(
        map,
        [centre.x - window * 0.5, centre.y - window * 0.5, centre.x + window * 0.5, centre.y + window * 0.5],
    );
    let Some(o) = package.objects.iter().find(|o| o.hash == map) else { return };
    let (at, size) = (o.attributes.position(), o.attributes.size());
    // 0x4CA840: with map mode 2 the turn a = 0x43E6D0(dir y, dir x) =
    // 90° − heading, else 0; the map, its ring/mask and the north
    // indicator get −a (0x51CEE0, FEng angles turn clockwise), each
    // marker its own direction's angle − a, its offset turned by a.
    let a = if m.rotate { (90.0 - m.heading.to_degrees()).to_radians() } else { 0.0 };
    // The minimap object (0x4EC950) holds +8 Track2000_map (0x90FE80F3),
    // +0xC Minimap_Mask (0x40CDC515) and +0x10 Minimap_North_indicator
    // (0xF2A54430): the frame ring (TrackMapTargetRing, four
    // Minimap_Border quarters) does not turn, so its seams stay at the
    // four compass points.
    s.angle.insert(map, -a);
    s.angle.insert(fe_hash("Minimap_Mask"), -a);
    s.angle.insert(fe_hash("Minimap_North_indicator"), -a);
    // The player's arrow: its direction's angle 90° − heading, less a.
    let player = fe_hash("PlayerCarIndicator_1");
    s.position.insert(player, glam::Vec2::new(at.x, at.y));
    s.angle.insert(player, (90.0 - m.heading.to_degrees()).to_radians() - a);
    let (sin, cos) = a.sin_cos();
    let mut used: crate::fxhash::FxHashMap<&str, u32> = Default::default();
    for (base, world, heading, sponsor) in &m.icons {
        let n = used.entry(base).or_insert(0);
        let h = fe_hash(&format!("{base}{n}"));
        *n += 1;
        let f = (to_map(*world) - centre) / window;
        let mut d = glam::Vec2::new(cos * f.x + sin * f.y, cos * f.y - sin * f.x);
        let dist = d.length();
        if dist >= m.range {
            continue;
        }
        if dist > 0.55 {
            d *= 0.55 / dist;
        }
        s.position.insert(h, glam::Vec2::new(at.x + size.x * d.x, at.y + size.y * d.y));
        // 0x4AC460 (per item, from 0x4CA840): an event marker (types 12-18)
        // whose event has +0x34 = 1, a sponsor race, takes texture
        // 0xB90F50BB, the cross the World Map shows.
        if *sponsor && has_texture(fe_ids::BOX_OFF) {
            s.image.insert(h, fe_ids::BOX_OFF);
        }
        // Every shown item turns (0x4CA840: 0x43E6D0(dir y, dir x) − a,
        // 0x43E6D0(y, x) being atan2(x, y), so 90° − the direction's
        // angle): a car's (0x4AC2B0 cases 3 / 4: (cos h, sin h) of car
        // +0x40C) like the player's arrow; a marker's (cases 5–18: (cos θ,
        // sin θ) of the item's +0x14, 0 for the route triggers 0x4EC8C0
        // adds) by 90° − a.
        // 0x4EC8C0 → 0x4AC620 (again from 0x533320 via 0x4CB110): a marker
        // table entry's item runs "Static", "Normal" or "Dramatic" by the
        // entry's +0x14 (0 a shop, milestone or world challenge, 1 an
        // event; 0x533320), when its object has it (0x51CF80): of the
        // indicators only SHOWCASEINDICATOR_n (and AIRACERINDICATOR_n, not
        // a table entry) carry them, so the photo-shoot / challenge icon
        // takes Static's colour.
        if heading.is_none() {
            let script = if base.ends_with("EVENTINDICATOR_") { "Normal" } else { "Static" };
            let name = fe_hash(script);
            if package.objects.iter().any(|o| o.hash == h && o.scripts.iter().any(|sc| sc.name == name)) {
                s.run(h, script, 0.0);
            }
        }
        let heading = heading.unwrap_or(0.0);
        s.angle.insert(h, (90.0 - heading.to_degrees()).to_radians() - a);
    }
    // The GPS (0x4C8410, from 0x4DA640 with the GPS engaged in a career's
    // free roam): 0x697D2BCA shows in every state but off (4) and sits at
    // the target's map point (0x4CAE60: the markers' window and turn,
    // kept within 10 windows while the GPS is on, pulled in to 0.55,
    // else x = 512, off the map). GPS_Search_Group (+0x24) and its error
    // text (+0x2C: 0x155409E8 in state 1, 0x31139B65 in state 2) follow
    // the route search: the group in states 0–2 while the player's
    // follower holds no route, the error in states 1 and 2.
    if m.gps_searching {
        for n in ["GPS_Search_Group", "GPS_SEARCH_Text"] {
            s.hidden.remove(&fe_hash(n));
            s.shown.insert(fe_hash(n));
        }
    }
    match &m.gps_error {
        Some(t) => {
            let h = fe_hash("GPS_SEARCH_ERROR");
            s.hidden.remove(&h);
            s.shown.insert(h);
            s.text.insert(h, t.clone());
        }
        None => {
            s.hidden.insert(fe_hash("GPS_SEARCH_ERROR"));
        }
    }
    if let Some(g) = m.gps {
        const GPS: u32 = fe_ids::GPS;
        let f = (to_map(g) - centre) / window;
        let mut d = glam::Vec2::new(cos * f.x + sin * f.y, cos * f.y - sin * f.x);
        let dist = d.length();
        if dist < 10.0 {
            if dist > 0.55 {
                d *= 0.55 / dist;
            }
            s.hidden.remove(&GPS);
            s.shown.insert(GPS);
            s.position.insert(GPS, glam::Vec2::new(at.x + size.x * d.x, at.y + size.y * d.y));
        }
    }
}
