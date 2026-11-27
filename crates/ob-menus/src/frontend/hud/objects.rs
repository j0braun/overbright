//! Pieces of [`Hud::quads`], called in its order (see there).

use super::*;

impl Hud {
    /// The elements' objects in this package, found again when the package or drag tach changes.
    pub(super) fn element_objects(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { package, input, drag, now, .. } = *f;
        let has = |h: u32| package.objects.iter().any(|o| o.hash == h);
        let same = |e: &ElementObjects| {
            e.package.0 == package.name
                && e.package.1 == package.objects.as_ptr() as usize
                && e.package.2 == package.objects.len()
                && e.drag == drag
                && e.turbo == input.turbo
        };
        if !self.elements.as_ref().is_some_and(same) {
            let key = (package.name.clone(), package.objects.as_ptr() as usize, package.objects.len());
            let mut elements = Vec::new();
            for (i, e) in ELEMENTS.iter().enumerate() {
                let e = if i == 0 && drag { &DRAG_TACH } else { e };
                let mut hashes: Vec<u32> = e.objects.iter().map(|n| fe_hash(n)).collect();
                for base in e.numbered {
                    for n in 0..=16 {
                        let h = fe_hash(&format!("{base}{n}"));
                        if has(h) {
                            hashes.push(h);
                        }
                    }
                }
                if e.objects.contains(&"Angle_Indicator_DATA") {
                    // 0x4F6C00: the angle's backing and the texts' place by
                    // the car's turbo ([`DRIFT_ANGLE_BACKING`]).
                    let (backing, script) = DRIFT_ANGLE_BACKING[input.turbo as usize];
                    let h = fe_hash(backing);
                    if has(h) {
                        hashes.push(h);
                    }
                    let name = fe_hash(script);
                    for text in DRIFT_ANGLE_TEXTS {
                        let o = fe_hash(text);
                        // 0x51CF80(…, 1): a script the object lacks is not run.
                        if package.objects.iter().any(|x| x.hash == o && x.scripts.iter().any(|sc| sc.name == name)) {
                            self.state.run(o, script, now);
                        }
                    }
                }
                elements.push((e.lo, e.hi, hashes));
            }
            for (elo, ehi, hashes) in HASHED {
                elements.push((*elo, *ehi, hashes.to_vec()));
            }
            let mut children = crate::fxhash::FxHashMap::<u32, Vec<(u32, u32)>>::default();
            for object in &package.objects {
                if let Some(parent) = object.parent {
                    children.entry(parent).or_default().push((object.hash, object.guid));
                }
            }
            self.elements =
                Some(ElementObjects { package: key, drag, turbo: input.turbo, elements, children, hidden: None });
        }
    }

    /// Show or hide each element's objects by its mask (0x4C6690), cached per mask.
    pub(super) fn element_masks(&mut self, f: &QuadsFrame<'_>) {
        let QuadsFrame { input, package, lo, hi, drag, .. } = *f;
        let s = &mut self.state;
        let drift_race = input.race == crate::race::RaceMode::Drift;
        let mask_key = (lo, hi, drift_race);
        #[cfg(test)]
        let cached = !self.reference_hierarchy;
        #[cfg(not(test))]
        let cached = true;
        if cached
            && let Some((k, h)) = &self.elements.as_ref().unwrap().hidden
            && *k == mask_key
        {
            s.hidden.clone_from(h);
        } else {
            let mut shown = HashSet::default();
            let mut hidden = HashSet::default();
            for (elo, ehi, hashes) in &self.elements.as_ref().unwrap().elements {
                let on = elo & lo != 0 || ehi & hi != 0;
                for &h in hashes {
                    if on {
                        shown.insert(h)
                    } else {
                        hidden.insert(h)
                    };
                }
            }
            // 0x4CA090 shows Leader_bottom (0x50CA50) while the lap info is off,
            // outside a drift race (0x89E7D9 / 0x890104).
            if !drift_race && lo & 0x400 == 0 {
                shown.insert(fe_hash("Leader_bottom"));
            }
            // The HUD's constructor (0x4F8BF0), outside a drag race (0x89E7D8 /
            // 0x890108; 0x89E7E5 and 0x836474 not traced, taken as clear), first
            // hides every object of the package: 0x52CFD0(package, 0) visits them
            // all, down every group (0x504880 / 0x504830), with 0x51CEB0 →
            // FEObject_Hide 0x50CA00. Only the elements' objects show again
            // (0x4C6690 → 0x50CA50, which shows a group's objects too), so one
            // no element claims stays hidden: HUD_Short_Track's "Wrong Way"
            // string (WrongWayText, 0x159ABF82; no code names it), which the
            // clone drew through every Street X race, and the "Circuit Stats"
            // panel. The hidden bit is the object's own (a
            // hidden group's shown objects draw: read in the original's memory,
            // HUD_Short_Track's LeaderBoardGroup hidden, its rows shown), so only
            // objects that draw (not groups) are hidden here.
            if !drag {
                let mut open: Vec<u32> =
                    package.objects.iter().filter(|o| shown.contains(&o.hash)).map(|o| o.guid).collect();
                let mut under = HashSet::default();
                while let Some(g) = open.pop() {
                    #[cfg(test)]
                    if self.reference_hierarchy {
                        // Frozen pre-index traversal, for complete HUD-output comparison.
                        for o in package.objects.iter().filter(|o| o.parent == Some(g)) {
                            if under.insert(o.hash) {
                                open.push(o.guid);
                            }
                        }
                        continue;
                    }
                    if let Some(children) = self.elements.as_ref().unwrap().children.get(&g) {
                        for &(hash, guid) in children {
                            if under.insert(hash) {
                                open.push(guid);
                            }
                        }
                    }
                }
                for o in &package.objects {
                    if o.kind != crate::frontend::feng::Kind::Group
                        && !shown.contains(&o.hash)
                        && !under.contains(&o.hash)
                    {
                        hidden.insert(o.hash);
                    }
                }
            }
            // The online and leader-tab objects the constructor hides (0x4F8BF0).
            for n in ["Online_DATA_01", "Online_DATA_02", "Online_Timer"] {
                hidden.insert(fe_hash(n));
            }
            hidden.extend(UNTRACED);
            s.hidden = hidden.difference(&shown).copied().collect();
            self.elements.as_mut().unwrap().hidden = Some((mask_key, s.hidden.clone()));
        }
    }
}
