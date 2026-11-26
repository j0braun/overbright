//! Pieces of [`Hud::quads`], called in its order (see there).

use super::*;

impl Hud {
    /// 0x4DA640 / 0x4CA840: the minimap.
    pub(super) fn track_map(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { assets, package, input, lo, .. } = *f;
        let s = &mut self.state;
        // 0x4DA640: the minimap's filter, GPS search and 0x697D2BCA groups
        // stay hidden outside the GPS and filter states; 0x4CA840 frames
        // the track map on the player and places the markers.
        for h in [fe_ids::HASHED_0, fe_hash("GPS_Search_Group"), fe_ids::GPS] {
            s.hidden.insert(h);
        }
        if lo & 0x1 != 0 {
            match &input.map {
                Some(m) if m.texture.is_some() && m.size > 0.0 => minimap(s, package, &|t| assets.textures.contains_key(&t), m),
                _ => {
                    for e in ELEMENTS.iter().filter(|e| e.lo == 0x1 && e.objects.contains(&"Track2000_map")) {
                        for n in e.objects {
                            s.hidden.insert(fe_hash(n));
                        }
                    }
                }
            }
        }
    }

    /// 0x4CA090: the lap box and leader board groups.
    pub(super) fn lap_box_groups(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { lo, now, .. } = *f;
        let s = &mut self.state;
        // 0x4CA090: the lap box and leader board move with the features.
        let lap_info = if lo & 0x400 != 0 { "LAPINFOON" } else { "LAPINFOOFF" };
        let leader = if lo & 0x80_0000 != 0 { "LEADERON" } else { "LEADEROFF" };
        for (object, script) in
            [("LeaderBoardGroup", lap_info), ("Leader_bottom", lap_info), ("BestTimeCounter", leader)]
        {
            let h = fe_hash(object);
            if self.groups.get(&h) != Some(&script) {
                s.run(h, script, now);
                self.groups.insert(h, script);
            }
        }
        if lo & 0x400 != 0 {
            s.hidden.insert(fe_hash("Leader_bottom"));
        }
    }

    /// 0x4DA190 → 0x4C6F30: the leader board.
    pub(super) fn leader_board(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { assets, input, lo, now, .. } = *f;
        let s = &mut self.state;
        let text = |s: &mut ScreenState, name: &str, t: String| {
            s.text.insert(fe_hash(name), t);
        };
        let label = |h: u32| assets.strings.get(h).unwrap_or_default().to_string();
        // 0x4DA190 → 0x4C6F30: the leader board, a row per racer in race
        // order; rows past the racers are hidden. With one human it shows
        // the gaps (0x4AA9F0) for 10 s, then the names (0x4AA890) for 2 s,
        // timed on the sim clock 0x865188 from the element's construction
        // (0x4F63E7: +0x1BC = the clock, +0x1C0 = 0, gaps) at the race's
        // set-up, not from the green: after a start scene of ~10 s the
        // first flip falls at the green, so the board opens on the names
        // (**observed** under Wine, raceflow: circuit names at +0.3 s, gaps
        // from +3 s; sprint names at +0.05 s). The game's few hundred ms
        // between the clock's reset (4000 at the race load) and the
        // scene's start are load time the clone does not have.
        // The player's row runs SetPlayerColor and shows the name; the
        // others run Init and show the signed gap, or once done their
        // status (0x4AA7A0's colours are online only).
        if lo & 0x80_0000 != 0 {
            let t = input.race_age * 1000.0;
            let since = *self.board_since.get_or_insert(0.0);
            let hold = if self.board_names { 2000.0 } else { 10_000.0 };
            if t - since > hold {
                self.board_since = Some(t);
                self.board_names = !self.board_names;
            }
            // 0x4C6F30 → 0x4AAED0: which places the rows show (outside the
            // drift board, whose rows are its first four nodes).
            let drift = input.leaders.iter().any(|r| r.points.is_some());
            let places = match input.leaders.iter().position(|r| r.player) {
                Some(p) if !drift => board_rows(p, input.leaders.len(), 4),
                _ => (0..input.leaders.len().min(4)).collect(),
            };
            for row in 1..=4u32 {
                let names = [
                    format!("LBData_{row}"),
                    format!("LBTextBacking_{row}"),
                    format!("LeaderText_{row}"),
                    format!("LBBacking_{row}"),
                ];
                let place = places.get(row as usize - 1).copied();
                let Some(r) = place.and_then(|i| input.leaders.get(i)) else {
                    for n in &names {
                        s.hidden.insert(fe_hash(n));
                    }
                    continue;
                };
                let data = if let (Some(points), false) = (r.points, self.board_names) {
                    // "%$d" (0x78F744): grouped, "4,582" under Wine.
                    grouped(i64::from(points))
                } else if r.player || self.board_names {
                    r.name.clone()
                } else {
                    match r.done {
                        Some(status) => label(status),
                        None => signed_time_text(r.gap),
                    }
                };
                text(s, &names[0], data);
                text(s, &names[2], format!("{} :", place.unwrap_or(0) + 1));
                let script = if r.player { "SETPLAYERCOLOR" } else { "INIT" };
                // The drift board's rows (0x4AAC50 / 0x4AADC0) script the
                // backing (+0x1A4) alone, by the node's flag; LBData keeps
                // its own colour, so every row reads white (**observed**
                // under Wine, Quick Race drift). The other boards (0x4AA9F0
                // / 0x4AA890) script the backing and LBData.
                let scripted: &[&String] = if drift { &[&names[3]] } else { &[&names[3], &names[0]] };
                for &n in scripted {
                    let h = fe_hash(n);
                    if self.groups.get(&h) != Some(&script) {
                        s.run(h, script, now);
                        self.groups.insert(h, script);
                    }
                }
            }
        }
    }
}
