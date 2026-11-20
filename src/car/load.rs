//! The load pipeline: `Car::load*` and `Car::load_tuned`.

use super::*;

/// The wide body (slot 6, ride +0x5A8).
pub(super) const WIDE_BODY: usize = 6;
pub(super) const FRONT_WHEELS: usize = 0x1D;
pub(super) const REAR_WHEELS: usize = 0x1E;

impl Car {
    pub fn load(game_root: &Path, name: &str) -> Result<Car> {
        Car::load_with(game_root, name, &std::collections::BTreeMap::new())
    }

    /// Load a car with parts fitted over the stock ones: by slot, the
    /// part's hash.
    pub fn load_with(game_root: &Path, name: &str, installed: &std::collections::BTreeMap<u8, u32>) -> Result<Car> {
        Car::load_upgraded(game_root, name, installed, &[], &|_| false)
    }

    /// A car with its parts and its performance parts fitted.
    pub fn load_upgraded(
        game_root: &Path,
        name: &str,
        installed: &std::collections::BTreeMap<u8, u32>,
        packages: &[crate::performance::Package],
        fitted: &dyn Fn(u8) -> bool,
    ) -> Result<Car> {
        Car::load_tuned(game_root, name, installed, packages, fitted, None)
    }

    /// A car with its parts, performance parts and dyno tuning.
    /// The car for the physics alone ([`Car::load_tuned`] without its
    /// models, textures or paint): the car type and the collision hull.
    pub fn load_physics(
        game_root: &Path,
        name: &str,
        packages: &[crate::performance::Package],
        fitted: &dyn Fn(u8) -> bool,
        tune: Tune,
    ) -> Result<Car> {
        // The run's shared copy, as `load_tuned` reads it (before,
        // a dyno pull read the whole 8 MB file again).
        let globalb = shared_file(&crate::game_fs::path(game_root, "GLOBAL/GLOBALB.BUN")).context("reading GLOBALB.BUN")?;
        let car_type = parse_car_type_tuned(&globalb, name, packages, fitted, tune)?;
        let geometry =
            crate::game_fs::read(game_root, &format!("CARS/{name}/GEOMETRY.BIN")).context("reading car geometry")?;
        let (hull, hull_links) = collision_hull(&geometry, name);
        Ok(Car {
            name: name.to_string(),
            car_type,
            solids: Vec::new(),
            parts: Vec::new(),
            textures: HashMap::new(),
            // No lights part: the default pool colour.
            headlight_pool_color: [0x90, 0xAA, 0x90],
            animations: None,
            part_styles: Default::default(),
            neon: Default::default(),
            neon_tubes: Vec::new(),
            engine_neon: None,
            trunk_neon: None,
            part_paints: [None; 7],
            doorline: String::new(),
            light_materials: HashMap::new(),
            texture_remap: HashMap::new(),
            light_material_remap: HashMap::new(),
            rim_material: None,
            audio_material: None,
            mirror_material: None,
            brake_material: None,
            camber: camber_angle(fitted),
            hull_reach: HullReach::of(&hull),
            hull,
            hull_links,
            // No models: an empty box (no shadow).
            shadow_box: (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            light_flares: Vec::new(),
            exhausts: Vec::new(),
            wheel_shift: [0.0; 4],
            purge: None,
            look_parts: Default::default(),
        })
    }

    pub fn load_tuned(
        game_root: &Path,
        name: &str,
        installed: &std::collections::BTreeMap<u8, u32>,
        packages: &[crate::performance::Package],
        fitted: &dyn Fn(u8) -> bool,
        tune: Tune,
    ) -> Result<Car> {
        let _t = crate::prof::span("Car::load_tuned");
        let globalb_path = crate::game_fs::path(game_root, "GLOBAL/GLOBALB.BUN");
        let globalb = shared_file(&globalb_path).context("reading GLOBALB.BUN")?;
        let globalb: &[u8] = &globalb;
        let Sources {
            car_type,
            camber,
            light_materials,
            t_solids,
            car_dir,
            geometry,
            solids,
            mut textures,
            globalb_textures,
        } = Sources::load(game_root, name, &globalb_path, globalb, packages, fitted, tune)?;
        let global_textures = Sources::globalb_rest(&globalb_textures, &mut textures);
        let db = Sources::part_db(globalb, t_solids)?;
        let installed = &perf_visual_parts(installed, &db, name, packages, fitted);
        let mut b = CarBuild::new(name, &db, &geometry, globalb, installed, solids);
        let _t_shared = b.shared_parts(game_root, &light_materials, &mut textures, global_textures)?;
        b.index_solids();
        let looks = b.lights_and_bounds();
        let neons = b.neons();
        let flags = b.slot_flags(&car_type);
        let mut parts = b.body_parts(&flags);
        let purge = b.purge(&car_type);
        let exhausts = b.exhaust_and_plate(&flags, &mut parts);
        b.trunk_audio(&flags, &mut parts);
        let (part_styles, wheel_setup) = b.part_styles_and_wheel_setup(&car_type);
        let wheel_shift = wheel_setup.wheel_shift;
        b.wheels(&wheel_setup, &mut parts);
        let parts = b.lod_variants(parts);
        let mut car = b.assemble(
            game_root,
            car_type,
            parts,
            textures,
            light_materials,
            camber,
            looks,
            neons,
            flags,
            exhausts,
            wheel_shift,
            purge,
            part_styles,
        );
        b.wheel_paint(&mut car);
        b.skin(&mut car, &car_dir, installed);
        car.look_parts = installed.clone();
        Ok(car)
    }
}

/// [`Sources::load`]'s files: the car type, GLOBALB's light materials, the
/// car's solids and texture packs.
struct Sources {
    car_type: CarType,
    camber: f32,
    light_materials: HashMap<u32, LightMaterial>,
    t_solids: Option<crate::prof::Span>,
    car_dir: std::path::PathBuf,
    geometry: crate::cache::SourceBytes,
    solids: Vec<Solid>,
    textures: HashMap<u32, Texture>,
    globalb_textures: std::sync::Arc<Vec<Texture>>,
}

impl Sources {
    /// The GLOBALB record, the light materials, the car's solids and its
    /// texture packs with the shared ones.
    fn load(
        game_root: &Path,
        name: &str,
        globalb_path: &Path,
        globalb: &[u8],
        packages: &[crate::performance::Package],
        fitted: &dyn Fn(u8) -> bool,
        tune: Tune,
    ) -> Result<Sources> {
        let car_type = parse_car_type_tuned(&globalb, name, packages, fitted, tune)?;
        let camber = camber_angle(fitted);
        let light_materials = parse_light_materials(&globalb);
        let t_solids = crate::prof::span("Car::load_tuned: solids, textures");

        let car_dir = crate::game_fs::path(game_root, &format!("CARS/{name}"));
        let geometry =
            crate::cache::SourceBytes::open(&crate::game_fs::path(&car_dir, "GEOMETRY.BIN")).context("reading car geometry")?;
        let solids: Vec<Solid> = crate::cache::file_cached(crate::BUILD_ID, &crate::game_fs::path(&car_dir, "GEOMETRY.BIN"), "solids", || {
            let mut solids = Vec::new();
            for list in bundle::chunks(&geometry, 0).filter(|c| c.id == id::SOLID_LIST) {
                solids.extend(parse_solid_list(list)?);
            }
            Ok(solids)
        })?;

        let mut textures = HashMap::new();
        // The car's pack, then the shared ones the game keeps loaded with
        // the cars (table 0x7FA978): CARS\TEXTURES.BIN and the rims'
        // CARS\WHEELS\TEXTURES.BIN (BRAKES and PLATES are empty).
        let file = crate::game_fs::path(&car_dir, "TEXTURES.BIN");
        let own = crate::cache::file_cached(crate::BUILD_ID, &file, "textures", || {
            let data = crate::cache::SourceBytes::open(&file).with_context(|| format!("reading {}", file.display()))?;
            texture_packs(&data)
        })?;
        for t in own {
            textures.entry(t.hash).or_insert(t);
        }
        for file in [crate::game_fs::path(game_root, "CARS/TEXTURES.BIN"), crate::game_fs::path(game_root, "CARS/WHEELS/TEXTURES.BIN")] {
            for t in shared_textures(&file)?.iter() {
                textures.entry(t.hash).or_insert_with(|| t.clone());
            }
        }

        // The window glass (0x615290): each of the six WINDOW_* slots is
        // remapped by its damage state (0x802740: WINDOW, WINDOW_DAMAGE0..2)
        // to a GLOBALB texture. The intact rear also keeps the GLOBALB
        // REAR_DEFROSTER (entry +0x768 targets 0xD195BE56 at state 0), which
        // the hatch glass of cars like the 240SX uses.
        let globalb_textures = shared_textures(&globalb_path)?;
        Ok(Sources {
            car_type,
            camber,
            light_materials,
            t_solids,
            car_dir,
            geometry,
            solids,
            textures,
            globalb_textures,
        })
    }

    /// GLOBALB's window textures into the car's; the rest kept at hand.
    fn globalb_rest<'t>(
        globalb_textures: &'t std::sync::Arc<Vec<Texture>>,
        textures: &mut HashMap<u32, Texture>,
    ) -> HashMap<u32, &'t Texture> {
        // The rest of GLOBALB's textures stay at hand for the shared parts
        // (the post mirrors' CHROME glass): GLOBALB is always loaded.
        let mut global_textures: HashMap<u32, &Texture> = HashMap::new();
        for t in globalb_textures.iter() {
            if t.hash == bin_hash(WINDOW_TEXTURES[0]) || t.hash == bin_hash(REAR_DEFROSTER) {
                textures.entry(t.hash).or_insert_with(|| t.clone());
            } else {
                global_textures.entry(t.hash).or_insert(t);
            }
        }
        global_textures
    }

    /// The part database (ends the solids/textures span).
    fn part_db(globalb: &[u8], t_solids: Option<crate::prof::Span>) -> Result<PartDb> {
        drop(t_solids);
        // The stock parts from the part database (0x637050), at LOD A:
        // each model group whose solid the car has.
        let t_db = crate::prof::span("Car::load_tuned: part database");
        let db = PartDb::load(&globalb)?;
        drop(t_db);
        Ok(db)
    }
}

/// What every stage of [`Car::load_tuned`] after the part database reads:
/// the fitted part per slot, the car's solids (with the shared parts') and
/// their index, the base part's mount markers.
pub(super) struct CarBuild<'a> {
    name: &'a str,
    db: &'a PartDb,
    geometry: &'a [u8],
    /// Which slot models the part animations move (chunk 0x34608).
    anim_table: Vec<i8>,
    /// The part in each slot 0..0xAA.
    fitted_slots: Vec<Option<&'a crate::formats::parts::Part>>,
    solids: Vec<Solid>,
    /// The car's own solids (before the shared parts'); set by
    /// [`CarBuild::shared_parts`].
    own_count: usize,
    /// Solid index by hash; set by [`CarBuild::index_solids`].
    by_hash: HashMap<u32, usize>,
    /// The base part's mount markers; set by [`CarBuild::index_solids`].
    markers: Vec<(u32, [f32; 16])>,
}

/// [`CarBuild::lights_and_bounds`]'s results.
struct CarLooks {
    window_tint: Option<u32>,
    shadow_box: (Vec3, Vec3),
    light_flares: Vec<crate::glow::CarFlare>,
    rim_material: Option<u32>,
    audio_material: Option<u32>,
    brake_material: Option<u32>,
    headlight_pool_color: [u8; 3],
}

/// [`CarBuild::neons`]'s results.
struct CarNeon {
    neon: crate::shadow::Neon,
    neon_tubes: Vec<[Vec3; 2]>,
    engine_neon: Option<(crate::shadow::Neon, Vec<[Vec3; 2]>)>,
    trunk_neon: Option<(crate::shadow::Neon, Vec<[Vec3; 2]>)>,
}

/// [`CarBuild::slot_flags`]'s results.
struct SlotFlags {
    audio_level: u8,
    part_paints: [Option<[u8; 4]>; 7],
    doorline: String,
    shown: [bool; 0xAA],
    carbon_hood: bool,
    vertsplit: bool,
    type_index: usize,
}

/// [`CarBuild::part_styles_and_wheel_setup`]'s wheel values.
struct WheelSetup {
    wide_body: bool,
    brake_marker: [f32; 2],
    wheel_shift: [f32; 4],
    drawn_wheels: [Wheel; 4],
}

impl<'a> CarBuild<'a> {
    fn new(
        name: &'a str,
        db: &'a PartDb,
        geometry: &'a [u8],
        globalb: &[u8],
        installed: &std::collections::BTreeMap<u8, u32>,
        solids: Vec<Solid>,
    ) -> CarBuild<'a> {
        // Which slot models the part animations move (chunk 0x34608).
        let anim_table: Vec<i8> = bundle::chunks(&globalb, 0)
            .find(|c| c.id == ANIMATED)
            .map(|c| c.body.iter().map(|&b| b as i8).collect())
            .unwrap_or_default();
        // The stock car (0x637050) leaves the vinyl slots 0x40–0x43, the
        // decal slots 0x61–0x98 and slot 0x19 empty.
        let stocked = |slot: usize| !((0x40..=0x43).contains(&slot) || (0x61..=0x98).contains(&slot) || slot == 0x19);
        // Looked up once a slot (the database search is linear; no part
        // has a slot past 0xA9).
        let fitted_slots: Vec<Option<&crate::formats::parts::Part>> = (0..0xAA)
            .map(|slot| match installed.get(&(slot as u8)) {
                // Hash 0: a slot the ride empties (ride +0x590 + 4·slot =
                // 0; a preset's 0, the AI looks' misses).
                Some(&0) => None,
                Some(&h) => {
                    db.find(name, slot, Some(h), None).or_else(|| db.stock(name, slot).filter(|_| stocked(slot)))
                }
                None => db.stock(name, slot).filter(|_| stocked(slot)),
            })
            .collect();
        CarBuild {
            name,
            db,
            geometry,
            anim_table,
            fitted_slots,
            solids,
            own_count: 0,
            by_hash: HashMap::new(),
            markers: Vec::new(),
        }
    }

    fn fitted(&self, slot: usize) -> Option<&'a crate::formats::parts::Part> {
        self.fitted_slots.get(slot).copied().flatten()
    }

    fn lod_a(&self, slot: usize) -> Vec<Option<usize>> {
        self.fitted(slot).map_or_else(Vec::new, |p| {
            self.db.solids(p, self.name).iter().map(|g| g.first().and_then(|h| self.by_hash.get(h).copied())).collect()
        })
    }

    fn marker(&self, hash: u32) -> Option<Mat4> {
        self.markers.iter().find(|m| m.0 == hash).map(|m| Mat4::from_cols_array(&m.1))
    }

    // Per-part paint (0x638DE8): the part in `paint_slot` names its
    // light material by attribute 0x6BA02C05, used only when the part in
    // `gate` is above level 0 (part byte +5 >> 5 ≥ 1). The rims' paint
    // (slot 0x49) waits on the front rim (0x1D) → +0x390; the brakes'
    // (0x46) on the front brakes (0x1B) → +0x398.
    fn part_paint(&self, paint_slot: usize, gate: usize) -> Option<u32> {
        self.fitted(gate).filter(|p| p.level >= 1)?;
        self.fitted(paint_slot).and_then(|p| self.db.attribute(p, 0x6BA0_2C05))
    }

    fn above_stock(&self, s: usize) -> bool {
        self.fitted(s).is_some_and(|p| p.level > 0)
    }

    /// The shared part files' solids, the model fixes and smoothing, and
    /// the GLOBALB textures the solids name.
    fn shared_parts(
        &mut self,
        game_root: &Path,
        light_materials: &HashMap<u32, LightMaterial>,
        textures: &mut HashMap<u32, Texture>,
        mut global_textures: HashMap<u32, &Texture>,
    ) -> Result<Option<crate::prof::Span>> {
        let mut solids = std::mem::take(&mut self.solids);
        let (name, db) = (self.name, self.db);
        let fitted = |slot: usize| self.fitted(slot);
        // Parts whose models are not in the car's own file come from the
        // shared part files the game keeps loaded (0x57F040): the solids
        // are found by hash in all of them alike (0x4908E0).
        let own: std::collections::HashSet<u32> = solids.iter().map(|s| s.hash).collect();
        let own_count = solids.len();
        let wanted: std::collections::HashSet<u32> = (0..0xAA)
            .filter_map(|s| fitted(s))
            .flat_map(|p| db.solids(p, name))
            // Every LOD's model (the car draw picks one per frame).
            .flatten()
            // The trunk audio components' in-game placeholders (0x635729).
            .chain(AUDIO_COMP_EMPTY.iter().flatten().map(|n| bin_hash(n)))
            .filter(|h| !own.contains(h))
            .collect();
        let _t_shared = crate::prof::span("Car::load_tuned: shared parts and the rest");
        if !wanted.is_empty() {
            for file in shared_part_files(game_root, &db) {
                add_shared_part_solids(&file, &wanted, &mut solids)?;
            }
        }
        // Fix Original Model Errors (opt-in; docs/notes/graphics/model-fixes.md): no-op
        // when off.
        crate::model_fixes::apply(&mut solids, &|h| light_materials.get(&h).map(|l| l.name.clone()));
        // Smooth Car Shapes (opt-in; docs/notes/graphics/assets-distance.md): no-op when
        // off; after the fixes, which work on the files' vertices.
        if crate::car_smooth::enabled() {
            // Only the fitted parts' models (every LOD's hash; the pass
            // keeps to `_A`): the file holds every kit.
            let used: std::collections::HashSet<u32> =
                (0..0xAA).filter_map(|s| fitted(s)).flat_map(|p| db.solids(p, name)).flatten().collect();
            let st = crate::car_smooth::apply_where(&mut solids, &|s| used.contains(&s.hash));
            if std::env::var_os("NFSU2_SMOOTH_STATS").is_some() {
                eprintln!("smooth {name}: {st:?}");
            }
        }
        // GLOBALB's textures stay loaded (its pack is resident), so a
        // texture none of the car's packs has comes from there — the shared
        // parts' CHROME, and the car's own brake BRAKE_STYLE00 (0xC6578694).
        for s in &solids {
            for t in &s.textures {
                if !textures.contains_key(t)
                    && let Some(g) = global_textures.remove(t)
                {
                    textures.insert(*t, g.clone());
                }
            }
        }
        // The carbon-fibre and empty-decal targets (0x636850, 0x61FF50)
        // are GLOBALB textures too.
        for h in [CARBONFIBRE, bin_hash(DEFAULT_ALPHA)] {
            if !textures.contains_key(&h)
                && let Some(g) = global_textures.remove(&h)
            {
                textures.insert(h, g.clone());
            }
        }
        self.solids = solids;
        self.own_count = own_count;
        Ok(_t_shared)
    }

    /// The solid index and the base part's mount markers.
    fn index_solids(&mut self) {
        let (name, db, geometry, solids) = (self.name, self.db, self.geometry, &self.solids);
        let fitted = |slot: usize| self.fitted(slot);
        let by_hash: HashMap<u32, usize> = solids.iter().enumerate().map(|(i, s)| (s.hash, i)).collect();
        // The mount markers (0x634F66): on the LOD A solid of the base
        // part (slot 0), SPOILER → render info +0x2C04, ROOF_SCOOP →
        // +0x2C08, 0x9B971511 (left mirror) → +0x2C0C, 0x0EE87EC4 (right
        // mirror) → +0x2C10.
        let markers: Vec<(u32, [f32; 16])> = fitted(0)
            .and_then(|p| db.solids(p, name).first().and_then(|g| g.first().copied()))
            .map(|base| {
                bundle::chunks(&geometry, 0)
                    .filter(|c| c.id == id::SOLID_LIST)
                    .flat_map(|l| crate::formats::solid::solid_markers(l, base))
                    .collect()
            })
            .unwrap_or_default();
        self.by_hash = by_hash;
        self.markers = markers;
    }

    /// The window tint, the shadow box, the light flares, the part paints'
    /// materials and the headlight pool colour.
    fn lights_and_bounds(&self) -> CarLooks {
        let (db, geometry, solids) = (self.db, self.geometry, &self.solids);
        let fitted = |slot: usize| self.fitted(slot);
        let lod_a = |slot: usize| self.lod_a(slot);
        let part_paint = |paint_slot: usize, gate: usize| self.part_paint(paint_slot, gate);
        // The window tint's light material (0x638DC5: render info +0x38C =
        // attribute 0x6BA02C05 of the slot 0x99 part, else WINDSHIELD).
        let window_tint = fitted(WINDOW_TINT).and_then(|p| db.attribute(p, 0x6BA0_2C05));
        let shadow_box = {
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for slot in SHADOW_BOX_SLOTS {
                for group in lod_a(slot).into_iter().take(2) {
                    let (a, b) = group.map_or((Vec3::ZERO, Vec3::ZERO), |s| {
                        (Vec3::from(solids[s].bbox_min), Vec3::from(solids[s].bbox_max))
                    });
                    lo = lo.min(a);
                    hi = hi.max(b);
                }
            }
            (lo - Vec3::splat(0.1), hi + Vec3::splat(0.1))
        };
        let light_flares = {
            let lists: Vec<_> = bundle::chunks(&geometry, 0).filter(|c| c.id == id::SOLID_LIST).collect();
            let solid_of = |slot: usize| lod_a(slot).first().copied().flatten().map(|s| solids[s].hash);
            let offset = solid_of(BODY_SLOT)
                .and_then(|h| lists.iter().find_map(|&l| crate::formats::solid::solid_translation(l, h)))
                .unwrap_or(Vec3::ZERO);
            light_flares(
                (0..FLARE_SLOTS)
                    .filter_map(solid_of)
                    .flat_map(|h| lists.iter().flat_map(move |&l| crate::formats::solid::solid_markers(l, h))),
                offset,
            )
        };
        let rim_material = part_paint(0x49, FRONT_WHEELS);
        let audio_material = fitted(AUDIO)
            .filter(|p| p.level >= 3)
            .and_then(|_| fitted(0x48))
            .and_then(|p| db.attribute(p, 0x6BA0_2C05));
        let brake_material = part_paint(0x46, FRONT_BRAKES);
        let headlight_pool_color = fitted(LIGHTS).map_or([0x90, 0xAA, 0x90], |p| {
            let get = |k: &str| db.attribute(p, bin_hash(k)).unwrap_or(0) as u8;
            [get("RED"), get("GREEN"), get("BLUE")]
        });
        CarLooks {
            window_tint,
            shadow_box,
            light_flares,
            rim_material,
            audio_material,
            brake_material,
            headlight_pool_color,
        }
    }

    /// The underglow, its tubes, and the engine and trunk neon.
    fn neons(&self) -> CarNeon {
        let (db, geometry, solids) = (self.db, self.geometry, &self.solids);
        let fitted = |slot: usize| self.fitted(slot);
        let lod_a = |slot: usize| self.lod_a(slot);
        // 0x61FC30: render info +0x34C = the slot 0x9A part's RED, GREEN,
        // BLUE, ALPHA (each the attribute's low byte), +0x350 = RED2 − RED
        // … ALPHA2 − ALPHA (bytes, wrapping); zeros with no part. The pulse
        // (0x6227DE) runs only when slot 0x9A, 0x9B or 0x9D holds a part
        // above level 0 (part byte +5 >> 5, ride +0x7F8 / +0x7FC / +0x804).
        let neon = {
            let mut n = crate::shadow::Neon {
                lit: [0x9A, 0x9B, 0x9D].iter().any(|&s| fitted(s).is_some_and(|p| p.level > 0)),
                ..Default::default()
            };
            if let Some(p) = fitted(0x9A) {
                let get = |k: &str| db.attribute(p, bin_hash(k)).unwrap_or(0) as u8;
                for (i, (a, b)) in [("RED", "RED2"), ("GREEN", "GREEN2"), ("BLUE", "BLUE2"), ("ALPHA", "ALPHA2")]
                    .into_iter()
                    .enumerate()
                {
                    n.base[i] = get(a);
                    n.delta[i] = get(b).wrapping_sub(get(a));
                }
            }
            n
        };
        // 0x6357A0: per prefix k, the model of slot (1, 0xB, 2)[k] (all
        // slot 6 with a wide body above level 0), its markers
        // <prefix><i>_START / _END for i in 0..3 (0x488FA0 by bin hash),
        // each pair found kept in order.
        let neon_tubes = if fitted(0x9A).is_some() {
            let lists: Vec<_> = bundle::chunks(&geometry, 0).filter(|c| c.id == id::SOLID_LIST).collect();
            let wide = fitted(crate::shadow::NEON_TUBE_WIDEBODY_SLOT).is_some_and(|p| p.level > 0);
            let mut tubes = Vec::new();
            for (k, prefix) in crate::shadow::NEON_TUBE_PREFIXES.iter().enumerate() {
                let slot =
                    if wide { crate::shadow::NEON_TUBE_WIDEBODY_SLOT } else { crate::shadow::NEON_TUBE_SLOTS[k] };
                let Some(h) = lod_a(slot).first().copied().flatten().map(|s| solids[s].hash) else { continue };
                let markers: Vec<(u32, [f32; 16])> =
                    lists.iter().flat_map(|&l| crate::formats::solid::solid_markers(l, h)).collect();
                let at = |name: String| {
                    markers.iter().find(|m| m.0 == bin_hash(&name)).map(|m| Vec3::new(m.1[12], m.1[13], m.1[14]))
                };
                for i in 0..3 {
                    if let (Some(a), Some(b)) = (at(format!("{prefix}{i}_START")), at(format!("{prefix}{i}_END"))) {
                        tubes.push([a, b]);
                    }
                }
            }
            tubes
        } else {
            Vec::new()
        };
        // The engine and trunk neon (0x6357A0, 0x615A20): the slot's
        // colours with the underglow's pulse gate (`lit`), and the marker
        // pairs found in the models' solids.
        let pairs = |models: Vec<usize>, start: &str, end: &str, count: usize| -> Vec<[Vec3; 2]> {
            let lists: Vec<_> = bundle::chunks(&geometry, 0).filter(|c| c.id == id::SOLID_LIST).collect();
            let markers: Vec<(u32, [f32; 16])> = models
                .into_iter()
                .map(|s| solids[s].hash)
                .flat_map(|h| lists.iter().flat_map(move |&l| crate::formats::solid::solid_markers(l, h)))
                .collect();
            let at = |name: String| {
                markers.iter().find(|m| m.0 == bin_hash(&name)).map(|m| Vec3::new(m.1[12], m.1[13], m.1[14]))
            };
            (0..count).filter_map(|i| Some([at(format!("{start}{i}"))?, at(format!("{end}{i}"))?])).collect()
        };
        let slot_neon = |slot: usize| {
            fitted(slot).map(|p| {
                let get = |k: &str| db.attribute(p, bin_hash(k)).unwrap_or(0) as u8;
                let mut n = crate::shadow::Neon { lit: neon.lit, ..Default::default() };
                for (i, (a, b)) in [("RED", "RED2"), ("GREEN", "GREEN2"), ("BLUE", "BLUE2"), ("ALPHA", "ALPHA2")]
                    .into_iter()
                    .enumerate()
                {
                    n.base[i] = get(a);
                    n.delta[i] = get(b).wrapping_sub(get(a));
                }
                n
            })
        };
        let engine_neon = slot_neon(0x9B).map(|n| {
            (
                n,
                pairs(
                    lod_a(0x0D).first().copied().flatten().into_iter().collect(),
                    "ENGINE_NEON_START_",
                    "ENGINE_NEON_END_",
                    5,
                ),
            )
        });
        let trunk_neon = slot_neon(0x9D).map(|n| {
            (
                n,
                pairs(
                    lod_a(AUDIO).into_iter().flatten().collect(),
                    "MARKER_TRUNK_NEON_START_",
                    "MARKER_TRUNK_NEON_END_",
                    4,
                ),
            )
        });
        CarNeon { neon, neon_tubes, engine_neon, trunk_neon }
    }

    /// The per-part paints, the doorline, the shown slots and the flags
    /// the body parts read.
    fn slot_flags(&self, car_type: &CarType) -> SlotFlags {
        let (name, db) = (self.name, self.db);
        let fitted = |slot: usize| self.fitted(slot);
        // 0x61D7A0: marker k takes slot k's paint part, but slot 0x48 only
        // with a level-3 (part +5 >> 5) audio part in slot 0x22 (ride
        // +0x618); otherwise the body paint.
        let audio_level = fitted(0x22).map_or(0, |p| p.level);
        let part_paints = PART_PAINT_SLOTS.map(|slot| {
            let p = fitted(slot).filter(|_| slot != 0x48 || audio_level >= 3)?;
            let get = |k: &str| db.attribute(p, bin_hash(k)).unwrap_or(0) as u8;
            Some([get("RED"), get("GREEN"), get("BLUE"), get("GLOSS")])
        });
        // 0x612B80: slot 6 (ride +0x5A8) above level 0, else slots 1, 2
        // and 0xB (the loop stops before the table's fourth entry, 5).
        let above_stock = |s: usize| self.above_stock(s);
        let doorline = format!(
            "{name}{}",
            if above_stock(6) {
                "_DOORLINE_WIDEBODY"
            } else if [1, 2, 0xB].into_iter().any(above_stock) {
                "_DOORLINE_KIT"
            } else {
                "_DOORLINE"
            }
        );
        let shown = shown_slots(&|s| fitted(s).map(|p| p.level), &|s| {
            fitted(s).map_or_else(Vec::new, |p| {
                db.attributes(p).iter().filter(|(k, _)| *k == 0xE80A_3B62).map(|&(_, v)| v).collect()
            })
        });
        // 0x634C95: the hood's CARBONFIBRE attribute (its value non-zero).
        let carbon_hood = fitted(9).and_then(|p| db.attribute(p, CARBONFIBRE)).is_some_and(|v| v != 0);
        // 0x6367D1: render info +0x2D48 = the hood's (ride +0x5B4) VERTSPLIT
        // attribute non-zero.
        let vertsplit = fitted(9).and_then(|p| db.attribute(p, bin_hash("VERTSPLIT"))).is_some_and(|v| v != 0);
        let type_index = car_type.handling.type_index;
        SlotFlags { audio_level, part_paints, doorline, shown, carbon_hood, vertsplit, type_index }
    }

    /// The generic draw's parts (spoiler, scoop and mirrors at their
    /// markers; the doors' and trunk's posed models).
    fn body_parts(&self, flags: &SlotFlags) -> Vec<CarPart> {
        let (db, geometry, solids, own_count) = (self.db, self.geometry, &self.solids, self.own_count);
        let (anim_table, markers) = (&self.anim_table, &self.markers);
        let (audio_level, shown, carbon_hood, vertsplit, type_index) =
            (flags.audio_level, &flags.shown, flags.carbon_hood, flags.vertsplit, flags.type_index);
        let fitted = |slot: usize| self.fitted(slot);
        let lod_a = |slot: usize| self.lod_a(slot);
        let marker = |hash: u32| self.marker(hash);
        let part_paint = |paint_slot: usize, gate: usize| self.part_paint(paint_slot, gate);
        let above_stock = |s: usize| self.above_stock(s);
        // The generic part draw (0x6230D5, table 0x630D3C) skips slots 3,
        // 4, 7, 0xC, 0xD, 0x10 and 0x1B–0x2E, which have their own draws;
        // the others it draws only while their ride +0x838 flag is set.
        let own_draw =
            |slot: usize| (3..=0x2E).contains(&slot) && !matches!(slot, 5 | 6 | 8..=0xB | 0xE | 0xF | 0x11..=0x1A);
        let decal_attr = |slot: usize, key: u32| fitted(slot).map(|p| db.attribute(p, key));
        // The trunk's and the doors' posed models as the generic draw left
        // them (0x6243F4: slot 0xA → [esp+0xC4], 0x11 → [esp+0x60], 0x12 →
        // [esp+0x68]; the last animated group's): its group and origin.
        let mut posed_by: [Option<(u8, Vec3)>; 3] = [None; 3];
        let mut parts = Vec::new();
        for slot in (0..0xAA).filter(|&s| draws_slot(s) && !(FRONT_BRAKES..=REAR_WHEELS).contains(&s)) {
            // The car draw 0x620AF0: the spoiler's two model groups at the
            // SPOILER marker (0x625AAB, else in car space at 0x626334), the
            // roof scoop's at ROOF_SCOOP (0x626039, else 0x6263B9); mirrors
            // above level 0 (ride +0x610, 0x62645D) group 0 at the left
            // marker and group 1 at the right one (its y axis un-flipped),
            // stock mirrors in car space (0x626F7F).
            let mount = |group: usize| -> Option<Mat4> {
                match slot {
                    SPOILER => Some(marker(bin_hash("SPOILER")).unwrap_or(Mat4::IDENTITY)),
                    ROOF_SCOOP => Some(marker(bin_hash("ROOF_SCOOP")).unwrap_or(Mat4::IDENTITY)),
                    MIRRORS if fitted(slot).is_some_and(|p| p.level > 0) => match group {
                        0 => marker(LEFT_MIRROR_MARKER),
                        // 0x626A4C: the right marker's copy with its +0x14
                        // (the y axis's y) negated, so the right model is
                        // not mirrored back.
                        _ => markers.iter().find(|m| m.0 == RIGHT_MIRROR_MARKER).map(|m| {
                            let mut a = m.1;
                            a[5] = -a[5];
                            Mat4::from_cols_array(&a)
                        }),
                    },
                    _ => Some(Mat4::IDENTITY),
                }
            };
            // The exhaust and the trunk audio components are drawn at their
            // markers below.
            // The engine shows only while the hood is not shut (below).
            if slot == EXHAUST
                || (AUDIO_COMP_FIRST..AUDIO_COMP_FIRST + AUDIO_COMP_MARKERS).contains(&slot)
                || (!own_draw(slot) && slot != HOOD_UNDER && !shown[slot])
            {
                continue;
            }
            let placed = matches!(slot, SPOILER | ROOF_SCOOP | MIRRORS);
            let scoop_level = fitted(ROOF_SCOOP).map_or(0, |p| p.level);
            for (group, solid) in lod_a(slot).into_iter().enumerate() {
                if solid.is_some_and(|s| s >= own_count) && !placed {
                    continue;
                }
                if !top_group_drawn(slot, group, type_index, scoop_level) {
                    continue;
                }
                if let (Some(solid), Some(transform)) = (solid, mount(group)) {
                    // The generic draw's animated models (chunk 0x34608),
                    // the engine gated on the hood, and the engine's and
                    // trunk audio's group 0 lit by their channel's value.
                    let origin = |solid: usize| {
                        bundle::chunks(&geometry, 0)
                            .filter(|l| l.id == id::SOLID_LIST)
                            .find_map(|l| crate::formats::solid::solid_translation(l, solids[solid].hash))
                            .unwrap_or(Vec3::ZERO)
                    };
                    let (channel, pose_group) = if own_draw(slot) {
                        (None, group as u8)
                    } else {
                        animated_channel(
                            slot,
                            group,
                            anim_table.get(slot * 2 + group).copied().unwrap_or(-1),
                            type_index,
                            vertsplit,
                        )
                    };
                    let mut anim = channel.map(|c| (c, origin(solid)));
                    let mut anim_group = pose_group;
                    if let (Some(a), Some(k)) = (anim, [0x0A, 0x11, 0x12].iter().position(|&s| s == slot)) {
                        posed_by[k] = Some((pose_group, a.1));
                    }
                    // Parts the draw puts on a posed model's matrix: the
                    // spoiler's group 0 at its marker × the trunk's
                    // (0x625AE1: only with the SPOILER marker; group 1 and
                    // the markerless path 0x626334 stay on the car); the
                    // aftermarket mirrors at their markers × the left /
                    // right door's (0x626515 / 0x626A86), the stock mirrors'
                    // groups 0 / 1 on the left / right door's (0x626FA6,
                    // prov: inferred car.load_tuned
                    // only while the left door is posed: **inferred** to be
                    // the same in the clone, which opens both doors
                    // together).
                    let follows = match (slot, group) {
                        (SPOILER, 0) if marker(bin_hash("SPOILER")).is_some() => posed_by[0].map(|p| (1, p)),
                        (MIRRORS, g) if g < 2 => posed_by[1 + g].map(|p| (2 + g as u8, p)),
                        _ => None,
                    };
                    if let Some((c, (g, o))) = follows {
                        anim = Some((c, o));
                        anim_group = g;
                    }
                    let harmonic_anim = match (slot, group) {
                        (ENGINE, 0) => Some((0, 0.6)),
                        (AUDIO, 0) => Some((1, 0.9)),
                        _ => None,
                    };
                    let harmonic_glow = match (slot, group) {
                        (ENGINE, 0) if fitted(ENGINE).is_some_and(|p| p.level == 3) => Some(0.6),
                        (AUDIO, 0) if audio_level == 3 => Some(1.5),
                        _ => None,
                    };
                    parts.push(CarPart {
                        solid,
                        transform,
                        carbon: carbon_for(slot, &above_stock, carbon_hood),
                        carbon_kit: slot < 0x3F && !own_draw(slot) && above_stock(KIT_CARBON),
                        decals: (0x34..=0x3E)
                            .contains(&slot)
                            .then(|| decal_block(slot, &decal_attr, fitted(9).is_some())),
                        harmonic_scale: if slot == AUDIO && group == 0 { 0.0 } else { 1.0 },
                        audio_paint: if slot == AUDIO { AudioPaint::TrunkAudio } else { AudioPaint::None },
                        mirror_paint: slot == MIRRORS && fitted(slot).is_some_and(|p| p.level > 0),
                        slot_paint: match slot {
                            ROOF_SCOOP => part_paint(0x4B, ROOF_SCOOP).map(|m| (CARSKIN_HASH, m)),
                            SPOILER => part_paint(0x45, SPOILER).map(|m| (CARSKIN_HASH, m)),
                            _ => None,
                        },
                        anim,
                        anim_group,
                        hood_gate: slot == ENGINE || slot == HOOD_UNDER,
                        harmonic_anim,
                        harmonic_glow,
                        ..Default::default()
                    });
                }
            }
        }
        parts
    }

    /// The purge's attributes and markers.
    #[allow(clippy::let_and_return)]
    fn purge(&self, car_type: &CarType) -> Option<PurgeFx> {
        let (name, db, geometry) = (self.name, self.db, self.geometry);
        let fitted = |slot: usize| self.fitted(slot);
        // The purge (0x634810): the slot 0xA2 part's attributes.
        let purge = fitted(0xA2).map(|p| {
            let attr = |k: &str| db.attribute(p, bin_hash(k)).unwrap_or(0);
            let f = |k: &str| f32::from_bits(attr(k));
            let mut fx = PurgeFx {
                groups: [attr("HOODEMITTER"), attr("WHEELEMITTER")],
                hsl: [[f("HOODHUE"), f("HOODSAT"), f("HOODLUM")], [f("TIREHUE"), f("TIRESAT"), f("TIRELUM")]],
                markers: Default::default(),
            };
            // HOODLEFT / HOODRIGHT: the markers of the slot 0 LOD A model
            // (render +0x17A8; 0x48D8F0 → 0x488FA0), their translation
            // (marker +0x40).
            let base: Vec<(u32, [f32; 16])> = fitted(0)
                .and_then(|b| db.solids(b, name).first().and_then(|g| g.first().copied()))
                .map(|h| {
                    bundle::chunks(&geometry, 0)
                        .filter(|c| c.id == id::SOLID_LIST)
                        .flat_map(|l| crate::formats::solid::solid_markers(l, h))
                        .collect()
                })
                .unwrap_or_default();
            for (key, marker, mirror) in [("HOODLEFT", PURGE_HOOD_LEFT, false), ("HOODRIGHT", PURGE_HOOD_RIGHT, true)] {
                if attr(key) != 0
                    && let Some(m) = base.iter().find(|m| m.0 == marker)
                {
                    fx.markers[0].push((Vec3::new(m.1[12], m.1[13], m.1[14]), [false, mirror, false]));
                }
            }
            // WHEELLEFT / WHEELRIGHT: 0x5B5360(1) on the ride's physics
            // block, +0x10 / +0x14 / +0x18 and +0x20 (+0x40… / +0x50 for
            // prov: inferred car.load_tuned-2
            // the right); **inferred**: that block runs as the car type's
            // record from +0x110, so these are wheel 0's and 1's centre
            // and radius (+0x120 + 0x30·i, +0x130).
            for (key, i, mirror) in [("WHEELLEFT", 0, true), ("WHEELRIGHT", 1, false)] {
                if attr(key) != 0 {
                    let w = &car_type.wheels[i];
                    fx.markers[1]
                        .push((Vec3::new(w.position.x + w.radius, w.position.y, w.position.z), [false, mirror, false]));
                }
            }
            fx
        });
        purge
    }

    /// The exhausts and the license plate at the walked markers; the
    /// exhausts' mounts.
    fn exhaust_and_plate(&self, flags: &SlotFlags, parts: &mut Vec<CarPart>) -> Vec<Mat4> {
        let (name, db, geometry, solids, by_hash) = (self.name, self.db, self.geometry, &self.solids, &self.by_hash);
        let shown = &flags.shown;
        let fitted = |slot: usize| self.fitted(slot);
        let lod_a = |slot: usize| self.lod_a(slot);
        let part_paint = |paint_slot: usize, gate: usize| self.part_paint(paint_slot, gate);
        // The exhausts (0x620AF0 at 0x6245C5): 0x639843 walks the markers
        // of the group 0 LOD A model of every slot 0–0x3E (with its ride
        // +0x838 flag set) and files those named EXHAUST, LEFT_EXHAUST, …
        // in the list at render info +0x598; the draw puts the exhaust's two
        // model groups (+0x19A8 / +0x19B8) at each, through a quarter turn
        // about y first (0x5BADB0 with 0x4000: rows (0, 0, −1), (0, 1, 0),
        // (1, 0, 0)), so the model's +x runs along the marker's −z. On the
        // 350Z they sit on the rear bumper. Only the car's own models are
        // searched (a shared model's markers are not read).
        let quarter_turn =
            Mat4::from_cols_array(&[0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
        let walked: Vec<(u32, [f32; 16])> = (0..=0x3E)
            .filter(|&s| shown[s])
            .filter_map(|s| fitted(s))
            .filter_map(|p| db.solids(p, name).first().and_then(|g| g.first().copied()))
            .flat_map(|h| {
                let raw = bundle::chunks(&geometry, 0)
                    .filter(|c| c.id == id::SOLID_LIST)
                    .flat_map(move |l| crate::formats::solid::solid_markers(l, h))
                    .collect::<Vec<_>>();
                // Fix Original Model Errors: duplicated markers (off: `raw`).
                crate::model_fixes::markers(by_hash.get(&h).map(|&i| solids[i].name.as_str()), raw)
            })
            .collect();
        let exhaust_mounts: Vec<Mat4> = walked
            .iter()
            .filter(|m| EXHAUST_MARKERS.contains(&m.0))
            .map(|m| Mat4::from_cols_array(&m.1) * quarter_turn)
            .collect();
        let exhausts: Vec<Mat4> = exhaust_mounts.iter().map(|m| *m * quarter_turn.inverse()).collect();
        let exhaust_paint = part_paint(0x47, EXHAUST).map(|m| (EXHAUST_TIP, m));
        for solid in lod_a(EXHAUST).into_iter().take(2).flatten() {
            for &transform in &exhaust_mounts {
                parts.push(CarPart {
                    solid,
                    transform,
                    is_wheel: false,
                    follows_wheel: None,
                    harmonic_scale: 1.0,
                    scene: Scene::Both,
                    audio_paint: AudioPaint::None,
                    spinner: false,
                    slot_paint: exhaust_paint,
                    ..Default::default()
                });
            }
        }
        // The license plate (0x620AF0 at 0x624CBD..0x625A79): the same walk
        // (0x639805) files REAR_LICENSE_PLATE (0xC805F7E0) markers in the
        // list at render info +0x620, and the draw puts the plate part's
        // (slot 0x21, e.g. LICENSE PLATE STYLE01 → CARS\PLATES
        // LICENSE_PLATE_STYLE01_A) two model groups (+0x1BC8 / +0x1BD8) at
        // each through the exhaust's quarter turn; it is skipped past LOD
        // 2 (0x624CAB). Its texture is GLOBALB's LICENSE_PLATE, which has
        // the text "UNDRGND 2" painted in; the light material LICENSEPLATE.
        // FRONT_LICENSE_PLATE / LICENSE_PLATE_FRONT go to the list at
        // +0x618, which the car draw never reads (no front plate).
        for solid in lod_a(LICENSE_PLATE).into_iter().take(2).flatten() {
            for m in walked.iter().filter(|m| m.0 == REAR_LICENSE_PLATE_MARKER) {
                parts.push(CarPart {
                    solid,
                    transform: Mat4::from_cols_array(&m.1) * quarter_turn,
                    harmonic_scale: 1.0,
                    scene: Scene::Both,
                    ..Default::default()
                });
            }
        }
        exhausts
    }

    /// The trunk audio components at the audio part's markers.
    fn trunk_audio(&self, flags: &SlotFlags, parts: &mut Vec<CarPart>) {
        let (name, db, geometry, by_hash) = (self.name, self.db, self.geometry, &self.by_hash);
        let audio_level = flags.audio_level;
        let fitted = |slot: usize| self.fitted(slot);
        let lod_a = |slot: usize| self.lod_a(slot);
        // The trunk audio components (0x620AF0 at 0x630976..0x630D25), after
        // the trunk audio and with its group 0's light context (the
        // harmonic × the trunk's value × 0.9, plus a level-3 part's glow):
        // for each marker i found on the
        // audio part's group 0 LOD A model, at marker × car, model group
        // g = (audio part level > 1) — the `_PAINT` model — at LOD A. In the
        // front end (0x8654A4 = 3) or the in-game showcase (0x836352, set by
        // 0x4CE770, the magazine photo screen) that is component i's own
        // part (slot 0x23 + i, render info +0x2274 + 0xC0·i + 0x60·g);
        // otherwise the placeholder of its size (+0x2B74 + 0x18·(2·size +
        // g)), whatever part is fitted.
        let audio_markers: Vec<(u32, [f32; 16])> = fitted(AUDIO)
            .and_then(|p| db.solids(p, name).first().and_then(|g| g.first().copied()))
            .map(|h| {
                bundle::chunks(&geometry, 0)
                    .filter(|c| c.id == id::SOLID_LIST)
                    .flat_map(|l| crate::formats::solid::solid_markers(l, h))
                    .collect()
            })
            .unwrap_or_default();
        let paint_group = usize::from(audio_level > 1);
        for i in 0..AUDIO_COMP_MARKERS {
            let Some(m) = audio_markers.iter().find(|m| m.0 == bin_hash(&format!("MARKER_AUDIO_COMP_{i}"))) else {
                continue;
            };
            let transform = Mat4::from_cols_array(&m.1);
            let own = lod_a(AUDIO_COMP_FIRST + i).get(paint_group).copied().flatten();
            let empty = by_hash.get(&bin_hash(AUDIO_COMP_EMPTY[AUDIO_COMP_SIZE[i]][paint_group])).copied();
            for (solid, scene) in [(own, Scene::FrontEnd), (empty, Scene::InGame)] {
                if let Some(solid) = solid {
                    // 0x630CEC: the trunk audio's group 0 context
                    // ([esp+0xD8], 0x630705), so lit by the open trunk.
                    parts.push(CarPart {
                        solid,
                        transform,
                        harmonic_scale: 0.0,
                        harmonic_anim: Some((1, 0.9)),
                        harmonic_glow: (audio_level == 3).then_some(1.5),
                        scene,
                        audio_paint: AudioPaint::Component,
                        ..Default::default()
                    });
                }
            }
        }
    }

    /// The part animations' styles; the wheels' wide-body face, brake
    /// markers and drawn positions.
    fn part_styles_and_wheel_setup(&self, car_type: &CarType) -> (crate::part_anim::Styles, WheelSetup) {
        let (name, db, solids) = (self.name, self.db, &self.solids);
        let fitted = |slot: usize| self.fitted(slot);
        let lod_a = |slot: usize| self.lod_a(slot);
        // Wheels and brakes are placed at the car type's wheel positions.
        // The wheel mesh is a right-hand wheel: its rim face is at y = 0 and
        // the tyre extends inward along +y (bounds y -0.008..0.21 on the
        // 240SX), so left-hand wheels (y > 0) are turned round (below). The
        // shop's rims (CARS/WHEELS/GEOMETRY_<brand>.BIN) are built the same
        // way (RACINGHART_STYLE07_20_26_A: y −0.01..0.19, radius 0.34, as
        // the 350Z's own wheel).
        //
        // 0x61FE20 fills the draw's table per wheel: the record wheel's
        // +0x1C / +0x20 (negated) and a width, the record's +0x14 when the
        // rim on that axle (front 0x1D, rear 0x1E) is above level 0, else
        // 0.184 (0x8026C4). The draw (0x6162F0, the loop after 0x6162F0's
        // 0x8026C0 / 0x8026C4 loads) scales the wheel across by
        // width / 0.184 and puts it at −(p ∓ width / 2) after a −width / 2
        // shift: a right wheel's outer face lands at y = +0x1C (−0.88 on
        // the 350Z, whose centre +0x04 is −0.755), the tyre running inward
        // the width. The left wheels (table 0 and 3) are not mirrored:
        // their scale matrix is multiplied by a 180° turn about z (0x5BAE70
        // with 0x8000, i.e. diag(−1, −1, 1); front end 0x617863/0x617A1F
        // and 0x619CBA, in game 0x62766E/0x627993 and 0x62C1E8), before
        // the wheel's frame and the face shift. Wheels 1 and 2 skip it.
        //
        // In game (0x620AF0) the face is read through a slot k (0x62714A:
        // render info +0x400 + 4k + 8i), k = 1 when the WIDE_BODY part
        // (slot 6, ride +0x5A8) is above level 0, else 0 (0x622F73…
        // 0x622FD1): with a wide body the wheels sit at the record's +0x20
        // (render info +0x404, 0x61FE20) instead of +0x1C. The front end
        // (0x6175A3) always reads +0x400.
        let wide_body = fitted(WIDE_BODY).is_some_and(|p| p.level != 0);
        // The part animations' variants (0x61F830): a hood of kind 5 (ride
        // +0x5B4, slot 9) splits — front / back (4) when its VERTSPLIT
        // attribute is non-zero, else left / right (2) — and plays its
        // halves at 0.75 / 1.25 × speed when it is the car's
        // `_STYLE22_HOOD` or `_STYLE22_HOOD_CF`; the doors play the door
        // style's ANIMSTYLE (ride +0x810, slot 0xA0; 0 REG, 2 SUICIDE …),
        // and its SCISSOR (1) the wide body's WIDE<kind> (slot 6 above level
        // 0 with a kind: kind + 2).
        let part_styles = {
            let hood = fitted(9).filter(|p| p.kind == 5);
            let car_hash = bin_hash(name);
            let doors = fitted(0xA0).and_then(|p| db.attribute(p, bin_hash("ANIMSTYLE"))).unwrap_or(0) as usize;
            let doors = match (doors, fitted(WIDE_BODY)) {
                (1, Some(w)) if w.level != 0 && w.kind != 0 => w.kind as usize + 2,
                (d, _) => d,
            };
            crate::part_anim::Styles {
                hood: hood
                    .map_or(0, |p| if db.attribute(p, bin_hash("VERTSPLIT")).is_some_and(|v| v != 0) { 4 } else { 2 }),
                style22: hood.is_some_and(|p| {
                    p.hash == bin_hash_from(car_hash, "_STYLE22_HOOD")
                        || p.hash == bin_hash_from(car_hash, "_STYLE22_HOOD_CF")
                }),
                doors,
            }
        };
        // The brake offset (0x6390A6…0x639112): the y (+0x44) of the
        // MARKER_BRAKE marker on the front / rear rim model (+0x1B48 /
        // +0x1B68: slot 0x1D / 0x1E, group 0, LOD A) → render info +0x430 /
        // +0x434, 0 without the marker (0x7A6694 / 0x7A6698 override them
        // when non-zero; both are 0 and never written).
        let marker_brake = |slot: usize| -> f32 {
            lod_a(slot)
                .first()
                .copied()
                .flatten()
                .and_then(|s| solids[s].markers.iter().find(|m| m.0 == bin_hash("MARKER_BRAKE")).map(|m| m.1[13]))
                .unwrap_or(0.0)
        };
        let brake_marker = [marker_brake(FRONT_WHEELS), marker_brake(REAR_WHEELS)];
        // Fix Original Model Errors: the drawn wheels' x shifts (zeros off).
        let wheel_shift = crate::model_fixes::wheel_shift(name);
        let drawn_wheels: [Wheel; 4] = std::array::from_fn(|i| {
            let mut w = car_type.wheels[i];
            w.position.x += wheel_shift[i];
            w
        });
        (part_styles, WheelSetup { wide_body, brake_marker, wheel_shift, drawn_wheels })
    }

    /// The rims, spinners and brakes at the car type's wheels.
    fn wheels(&self, setup: &WheelSetup, parts: &mut Vec<CarPart>) {
        let db = self.db;
        let WheelSetup { wide_body, brake_marker, ref drawn_wheels, .. } = *setup;
        let fitted = |slot: usize| self.fitted(slot);
        let lod_a = |slot: usize| self.lod_a(slot);
        for (i, w) in drawn_wheels.iter().enumerate() {
            let left = w.position.y > 0.0;
            let front = i < 2;
            let rim = if front { FRONT_WHEELS } else { REAR_WHEELS };
            let aftermarket = fitted(rim).is_some_and(|p| p.level != 0);
            // The table width (render info +0x420 + 4i, 0x61FE20).
            let table_width = if aftermarket { w.width } else { 0.184 };
            let across = table_width / 0.184;
            let turn = if left { -1.0 } else { 1.0 };
            let face_in_game = if wide_body { w.face_wide } else { w.face };
            // The pose turns each model about its own origin after the
            // per-wheel shifts (A·[R]·F·B: F, which carries the steer, comes
            // before the face shift B), so the pivot's y is B's: the
            // tyre's middle, w/2 inside the face (0x6273C8, p = −face).
            let inward = |face: f32, by: f32| if face > 0.0 { face - by } else { face + by };
            let place = |face: f32| {
                Mat4::from_translation(Vec3::new(w.position.x, face, w.position.z))
                    * Mat4::from_scale(Vec3::new(turn, turn * across, 1.0))
            };
            // The showroom's car is drawn by 0x620AF0 too (0x631239, the
            // front end's car view), so it takes the in-game placement:
            // an apitrace of the original's main menu puts Rachel's front
            // rims at |y| 0.9307 (the wide body's record +0x20) and her
            // front brakes at 0.8125 (the MARKER_BRAKE shift). Only traffic
            // goes through 0x6162F0 (always +0x1C), and has no wide body.
            let scenes: &[(Scene, f32)] = &[(Scene::Both, face_in_game)];
            if let Some(&Some(solid)) = lod_a(rim).first() {
                for &(scene, face) in scenes {
                    parts.push(CarPart {
                        solid,
                        transform: place(face),
                        is_wheel: true,
                        follows_wheel: Some((i, true)),
                        pivot_y: Some(inward(face, 0.5 * table_width)),
                        harmonic_scale: 1.0,
                        scene,
                        audio_paint: AudioPaint::None,
                        spinner: false,
                        ..Default::default()
                    });
                }
            }
            // The spinner (0x620AF0 at 0x6274DE…, which draws the front
            // end's car too, 0x631239; only traffic's 0x6162F0 draws group 0
            // alone — Wine, round 3B / scrfix: the Spinners strip's preview
            // wears the highlighted spinner's blades): the rim part's group 1
            // (+0x1B58 front, +0x1B78 rear), with no rim paint (its remap
            // material +0x3AC is 0, 0x62777D), drawn as the rim — the same
            // scale and 180° turn — on the spinner frame (0x5EC800: turned
            // about y by the spinner angle, the front ones steered), face
            // shifted as the rim and SPINNEROFFSET o further out (0x6273E0:
            // T_rim.y + o for a left wheel, − o for a right one; o the
            // front rim part's attribute, 0 without, 0x61FE20).
            if let Some(&Some(solid)) = lod_a(rim).get(1) {
                let o = fitted(FRONT_WHEELS)
                    .and_then(|p| db.attribute(p, bin_hash("SPINNEROFFSET")))
                    .map_or(0.0, f32::from_bits);
                let o = if left { o } else { -o };
                let transform = Mat4::from_translation(Vec3::Y * o) * place(face_in_game);
                let pivot_y = Some(inward(face_in_game, 0.5 * table_width) + o);
                parts.push(CarPart {
                    solid,
                    transform,
                    is_wheel: false,
                    follows_wheel: Some((i, true)),
                    pivot_y,
                    harmonic_scale: 1.0,
                    scene: Scene::Both,
                    audio_paint: AudioPaint::None,
                    spinner: true,
                    ..Default::default()
                });
            }
            // The brakes (0x62DF85 on, models +0x1B08 / +0x1B28). Front end
            // (0x6162F0): the wheel's own matrix (frame +0x1C), not the
            // rim's face-shifted one, so they sit at the wheel centre; the
            // left wheels' through diag(1, −1, 1) (0x7A66D4…DC), the
            // right-hand model mirrored.
            //
            // In game (0x620AF0) each brake is M·F·T (row vectors): M =
            // diag(1, −1, 1) for wheels 0 and 3 (0x62E0CA, 0x62FA9D), none
            // for 1 and 2 (0x62EBDD, 0x62F2D4); F the frame +0x1C + 0x40i,
            // the front ones turned by the steer (0x5BAD00) with their
            // translation kept, its y zeroed (0x627472…0x627491); T the
            // brake shift esp+0x4D0 + 0x40i (0x627416…0x627439): y =
            // −(p + v) for p < 0, −(p − v) else, p = −face (slot k above),
            // v = s·B with B the axle's MARKER_BRAKE y and s = width / 0.184,
            // or width / 2 without the marker. Products at 0x62E43C,
            // 0x62EBDD, 0x62F2D4, 0x62FDBC; then the car's matrix.
            // All four come from slot 0x1B's models (ride +0x5FC; models
            // +0x1B08 + 0x10·group + 4·lod, 0x62E027…0x62E091): the front
            // ones group 0, the rear ones group 1 when the part is level 0
            // (the stock brakes' rear model), else group 0 again (the
            // aftermarket brakes have one group). Slot 0x1C is not drawn.
            let brakes = lod_a(FRONT_BRAKES);
            let aftermarket = fitted(FRONT_BRAKES).is_some_and(|p| p.level > 0);
            let brake = if front || aftermarket { brakes.first() } else { brakes.get(1).or(brakes.first()) };
            if let Some(&Some(solid)) = brake {
                let mirror = Mat4::from_scale(Vec3::new(1.0, turn, 1.0));
                // 0x620AF0's brake (the showroom's too, see the rims above;
                // 0x6162F0's centred brake is traffic's, which has none).
                let b = brake_marker[usize::from(!front)];
                let v = if b != 0.0 { across * b } else { 0.5 * table_width };
                let y = inward(face_in_game, v);
                let shifted = Mat4::from_translation(Vec3::new(w.position.x, y, w.position.z)) * mirror;
                parts.push(CarPart {
                    solid,
                    transform: shifted,
                    is_wheel: false,
                    follows_wheel: Some((i, false)),
                    pivot_y: Some(y),
                    harmonic_scale: 1.0,
                    scene: Scene::Both,
                    audio_paint: AudioPaint::None,
                    spinner: false,
                    ..Default::default()
                });
            }
        }
    }

    /// The LOD B / C / D copies of the parts, and the model fixes' hidden
    /// triangles.
    fn lod_variants(&self, mut parts: Vec<CarPart>) -> Vec<CarPart> {
        let (name, db, solids) = (self.name, self.db, &self.solids);
        let fitted = |slot: usize| self.fitted(slot);
        // The LOD variants (0x620AF0's model column, see [`lod_gate`]):
        // each part built from a slot's LOD A model gets copies with the
        // slot's B / C / D models at the LODs the draw allows.
        {
            let mut table: HashMap<u32, (usize, usize, Vec<u32>)> = HashMap::new();
            for slot in 0..0xAA {
                if let Some(p) = fitted(slot) {
                    for (group, lods) in db.solids(p, name).into_iter().enumerate() {
                        if let Some(&a) = lods.first() {
                            table.entry(a).or_insert((slot, group, lods));
                        }
                    }
                }
            }
            let index: HashMap<u32, usize> = solids.iter().enumerate().map(|(i, s)| (s.hash, i)).collect();
            let mut variants = Vec::new();
            for part in &mut parts {
                let Some((slot, group, lods)) = table.get(&solids[part.solid].hash) else { continue };
                let allowed = lod_gate(*slot, *group);
                part.lods = Some(allowed & 1);
                for (lod, hash) in lods.iter().enumerate().skip(1).take(3) {
                    if allowed >> lod & 1 != 0
                        && let Some(&solid) = index.get(hash)
                    {
                        variants.push(CarPart { solid, lods: Some(1 << lod), ..part.clone() });
                    }
                }
            }
            parts.extend(variants);
        }
        // Fix Original Model Errors: models drawn without some triangles
        // (empty, and nothing changes, when the row is off).
        let scoop_fitted = fitted(ROOF_SCOOP).is_some_and(|p| p.level > 0);
        for part in &mut parts {
            part.hidden_tris = crate::model_fixes::hidden_triangles(&solids[part.solid], scoop_fitted);
        }
        parts
    }

    /// The car: hull, texture remap, materials and the window tint.
    #[allow(clippy::too_many_arguments)]
    fn assemble(
        &mut self,
        game_root: &Path,
        car_type: CarType,
        parts: Vec<CarPart>,
        textures: HashMap<u32, Texture>,
        light_materials: HashMap<u32, LightMaterial>,
        camber: f32,
        looks: CarLooks,
        neons: CarNeon,
        flags: SlotFlags,
        exhausts: Vec<Mat4>,
        wheel_shift: [f32; 4],
        purge: Option<PurgeFx>,
        part_styles: crate::part_anim::Styles,
    ) -> Car {
        let solids = std::mem::take(&mut self.solids);
        let (name, db, geometry) = (self.name, self.db, self.geometry);
        let CarLooks {
            window_tint,
            shadow_box,
            light_flares,
            rim_material,
            audio_material,
            brake_material,
            headlight_pool_color,
        } = looks;
        let CarNeon { neon, neon_tubes, engine_neon, trunk_neon } = neons;
        let SlotFlags { part_paints, doorline, .. } = flags;
        let fitted = |slot: usize| self.fitted(slot);
        let part_paint = |paint_slot: usize, gate: usize| self.part_paint(paint_slot, gate);
        let (hull, hull_links) = collision_hull(&geometry, name);

        // 0x631E30: the headlight and brakelight texture bases are the
        // slot 0xE / 0xF parts' attribute 0x10C98090 (0x4951F0).
        let light_bases = [0xE, 0xF].map(|s| fitted(s).and_then(|p| db.attribute(p, 0x10C9_8090)).filter(|&h| h != 0));
        let texture_remap = stock_texture_remap(name, &textures, light_bases);
        let mut car = Car {
            name: name.to_string(),
            car_type,
            solids,
            parts,
            textures,
            light_materials,
            texture_remap,
            light_material_remap: HashMap::new(),
            rim_material,
            brake_material,
            camber,
            audio_material,
            mirror_material: part_paint(0x4C, MIRRORS),
            hull_reach: HullReach::of(&hull),
            hull,
            hull_links,
            headlight_pool_color,
            neon,
            neon_tubes,
            engine_neon,
            trunk_neon,
            part_paints,
            doorline,
            shadow_box,
            light_flares,
            animations: crate::formats::zam::PartAnimations::load(game_root, name),
            part_styles,
            exhausts,
            wheel_shift,
            purge,
            look_parts: Default::default(),
        };
        if let Some(tint) = window_tint {
            car.light_material_remap.insert(bin_hash("WINDSHIELD"), tint);
        }
        car
    }

    /// The rims' and spinners' paint composited into the car's textures.
    fn wheel_paint(&self, car: &mut Car) {
        let (name, db) = (self.name, self.db);
        let fitted = |slot: usize| self.fitted(slot);
        // The rims' and spinners' paint (0x61D7A0 → 0x6343B0 / 0x61E050 →
        // 0x61DEA0), for a type with a DUMMY_SKIN slot (render info +0x584
        // set, 0x61C2DB): the front rim's (slot 0x1D, +0x604) texture base
        // (attribute 0x10C98090) + `_WHEEL`, under `_WHEEL_INNER_MASK`, is
        // painted with slot 0x49 into DUMMY_WHEEL (+0x588); its SPINNER_TEXTURE
        // under that + `_MASK` with slot 0x4A into DUMMY_SPINNER (+0x58C);
        // the remap table sends the source there (0x631EC6: +0x23C / +0x240
        // → +0x29C / +0x2A0). See [`composite_wheel`].
        if car.car_type.composited_skin
            && let Some(rim) = fitted(FRONT_WHEELS)
        {
            let paint = |slot: usize| {
                fitted(slot).map(|p| {
                    let get = |k: &str| db.attribute(p, bin_hash(k)).unwrap_or(0) as u8;
                    [get("RED"), get("GREEN"), get("BLUE"), get("GLOSS")]
                })
            };
            let base = db.attribute(rim, 0x10C9_8090).filter(|&h| h != 0);
            let spinner = db.attribute(rim, bin_hash("SPINNER_TEXTURE")).filter(|&h| h != 0);
            for (dummy, source, mask, slot, out) in [
                (
                    "DUMMY_WHEEL1",
                    base.map(|b| bin_hash_from(b, "_WHEEL")),
                    base.map(|b| bin_hash_from(b, "_WHEEL_INNER_MASK")),
                    0x49,
                    "_WHEEL1",
                ),
                ("DUMMY_SPINNER1", spinner, spinner.map(|b| bin_hash_from(b, "_MASK")), 0x4A, "_SPINNER1"),
            ] {
                let Some(source) = source else { continue };
                let made =
                    mask.and_then(|mask| composite_wheel(&car.textures, bin_hash(dummy), source, mask, paint(slot)));
                let made =
                    made.map(|t| Texture { name: format!("{name}{out}"), hash: bin_hash(&format!("{name}{out}")), ..t });
                if let Some(to) = wheel_remap_target(
                    made.as_ref().map(|t| t.hash),
                    bin_hash(dummy),
                    car.textures.contains_key(&bin_hash(dummy)),
                ) {
                    car.texture_remap.insert(source, to);
                }
                if let Some(t) = made {
                    car.textures.insert(t.hash, t);
                }
            }
        }
    }

    /// The stock paint and the vinyls.
    fn skin(&self, car: &mut Car, car_dir: &Path, installed: &std::collections::BTreeMap<u8, u32>) {
        let (name, db) = (self.name, self.db);
        // The stock paint: the slot 0x3F part named by car type +0x84C.
        let paint_hash = installed.get(&0x3F).copied().filter(|&h| h != 0).unwrap_or(car.car_type.stock_paint);
        let paint = db.find(name, 0x3F, Some(paint_hash), None).map(|p| Paint::from_part(&db, p));
        // A stored paint this car has no part for (a damaged or edited
        // save): the car keeps its shipped skin, and the log says why.
        if paint.is_none() && installed.get(&0x3F).is_some_and(|&h| h != 0) {
            log::warn!("{name}: paint {paint_hash:#010x} is no paint of this car; the shipped skin stays");
        }
        let vinyls = vinyl_layers(&car_dir, name, &db, installed);
        // Only a type with a DUMMY_SKIN slot (+0x88C) is composited; a
        // traffic car draws the shipped `<CAR>_SKIN1` whatever its skin
        // byte (see `CarType::composited_skin`).
        if let Some(paint) = paint.filter(|_| car.car_type.composited_skin) {
            car.paint_vinyls(paint, &vinyls);
        }
    }
}

#[cfg(test)]
mod read_once_tests {
    use super::*;

    /// The physics-only car (each dyno pull) takes GLOBALB.BUN
    /// from the run's shared copy, as a full car load does: the 8 MB file
    /// is read from disk at most once however many cars load.
    // The read notes are a developer-harness aid (`game_fs::record_reads`).
    #[cfg(feature = "dev-harness")]
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn physics_cars_read_globalb_once_a_run() {
        let root = crate::testkit::game_root();
        let path = crate::game_fs::path(&root, "GLOBAL/GLOBALB.BUN");
        crate::game_fs::record_reads();
        let fresh = Car::load_physics(&root, "PEUGOT", &[], &|_| false, None).unwrap();
        for _ in 0..3 {
            let again = Car::load_physics(&root, "PEUGOT", &[], &|_| false, None).unwrap();
            assert_eq!(format!("{:?}", again.car_type), format!("{:?}", fresh.car_type));
        }
        Car::load(&root, "PEUGOT").unwrap();
        let own = crate::game_fs::recorded_reads().iter().filter(|p| p.ends_with("GLOBALB.BUN")).count();
        assert_eq!(own, 0, "GLOBALB.BUN read past the shared copy");
        assert_eq!(crate::car::assets::shared_file_reads::of(&path), 1, "GLOBALB.BUN read from disk once");
    }
}
