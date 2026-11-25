//! The prop browser's pictures: each placeable solid drawn once by the
//! game's own [`WorldRenderer`], off-screen, on a world holding only the
//! region's placeable solids, and kept in the asset cache
//! (`<cache>/prop-thumbs/<key>.png`, [`DIR`]). Nothing is shipped: the
//! pictures are made on the player's machine from the player's game files,
//! the first time the browser shows a region, and read back from the cache
//! after that.
//!
//! The key ([`key`]) is a hash of what the picture shows: the solid's
//! geometry, its textures' pixels and [`VERSION`]. A solid that is the
//! same in two regions shares its picture; a changed solid (or a new
//! [`VERSION`]) gets a new one.
//!
//! The work is split so the editor never waits: [`Thumbs::request`] (in
//! the egui pass) queues the solids a tile wants, reads the pictures the
//! cache already has, and [`Thumbs::build`] (after the egui pass, with the
//! GPU: `App::render_view`) draws up to [`PER_FRAME`] of the others a frame
//! and stores them. The browser shows its category symbol until a picture
//! is ready.
//!
//! Each picture looks at the solid's front ([`super::props::FRONT`], its
//! local −y side; a solid much longer along y than x from its +x side) from a little to one side and above, as the solid
//! stands, against a clear background.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use glam::{Mat4, Vec3};

use crate::draw::camera::Camera;
use crate::render::{FrameParams, Gpu, RenderOptions, WorldRenderer, create_depth};
use crate::world::{HIDDEN_FLAG, Placement, World};

/// The pictures' folder in the asset cache.
pub const DIR: &str = "prop-thumbs";
/// A picture's size in pixels (square). The tile shows it at 54 points:
/// sharp up to display scale 2.
pub const PX: u32 = 128;
/// Bump when the pictures change (camera, size, background): the old
/// ones are then no longer found.
pub const VERSION: u32 = 1;
/// The most pictures drawn in one frame (one readback each frame).
pub const PER_FRAME: usize = 12;
/// The pictures' vertical field of view (degrees): narrow, so a picture
/// shows the solid's shape with little perspective.
const FOV_DEG: f32 = 28.0;
/// Where the camera stands, seen from the solid's front: turned this far
/// (clockwise seen from above) and raised this far (degrees).
const TURN_DEG: f32 = 32.0;
const RAISE_DEG: f32 = 18.0;
/// The colour format drawn in (as the viewport).
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The cache key of solid `i`'s picture: its name, box, mesh, the pixels
/// of its textures and [`VERSION`].
pub fn key(world: &World, i: usize) -> u64 {
    let Some(s) = world.solids.get(i) else { return 0 };
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    h.update(&VERSION.to_le_bytes());
    h.update(&PX.to_le_bytes());
    h.update(s.name.as_bytes());
    h.update(bytemuck::cast_slice(&s.bbox_min));
    h.update(bytemuck::cast_slice(&s.bbox_max));
    h.update(bytemuck::cast_slice(&s.vertices));
    h.update(bytemuck::cast_slice(&s.indices));
    h.update(&s.mesh_flags.to_le_bytes());
    for m in &s.materials {
        h.update(&[m.texture_slot, m.light_material]);
        h.update(&m.first_index.to_le_bytes());
        h.update(&m.index_count.to_le_bytes());
    }
    for t in &s.textures {
        h.update(&t.to_le_bytes());
        if let Some(tex) = world.textures.get(t) {
            h.update(&tex.width.to_le_bytes());
            h.update(&tex.height.to_le_bytes());
            h.update(&tex.data[..]);
            h.update(&tex.palette);
        }
    }
    h.digest()
}

/// The asset cache's pictures folder (None: the cache is off).
pub fn cache_dir() -> Option<PathBuf> {
    ob_core::cache::dir().map(|d| d.join(DIR))
}

/// Where the picture of `key` is kept in `dir`.
pub fn file(dir: &Path, key: u64) -> PathBuf {
    dir.join(format!("{key:016x}.png"))
}

/// A picture as RGBA8 (unmultiplied), [`PX`] square.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub rgba: Vec<u8>,
}

impl Picture {
    fn image(&self) -> egui::ColorImage {
        egui::ColorImage::from_rgba_unmultiplied([PX as usize, PX as usize], &self.rgba)
    }

    /// How many pixels are not background.
    pub fn coverage(&self) -> usize {
        self.rgba.chunks(4).filter(|p| p[3] != 0).count()
    }
}

/// Read a picture from the cache.
pub fn load(path: &Path) -> Option<Picture> {
    let img = image::open(path).ok()?.into_rgba8();
    (img.width() == PX && img.height() == PX).then(|| Picture { rgba: img.into_raw() })
}

/// Keep a picture in the cache (written whole or not at all).
pub fn store(path: &Path, p: &Picture) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut png = Vec::new();
    image::RgbaImage::from_raw(PX, PX, p.rgba.clone())
        .ok_or_else(|| std::io::Error::other("picture size"))?
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(std::io::Error::other)?;
    crate::qol::saves::write_atomic(path, &png, false)
}

/// A frame's RGBA8 with its background (the colour of the corners, where
/// the solid never reaches) made clear.
pub fn cut_background(mut rgba: Vec<u8>) -> Picture {
    let n = PX as usize;
    let at = |x: usize, y: usize| (y * n + x) * 4;
    let corners = [at(0, 0), at(n - 1, 0), at(0, n - 1), at(n - 1, n - 1)];
    let bg: Vec<[u8; 3]> = corners.iter().map(|&k| [rgba[k], rgba[k + 1], rgba[k + 2]]).collect();
    for p in rgba.chunks_mut(4) {
        let near = bg.iter().any(|b| (0..3).all(|c| (i16::from(p[c]) - i16::from(b[c])).abs() <= 1));
        p[3] = if near { 0 } else { 255 };
    }
    Picture { rgba }
}

/// The camera a picture of a solid with the box `lo`…`hi` (local, standing
/// at the origin) is taken with: on the front's side, turned and raised a
/// little, just far enough for the box's eight corners to fit the picture
/// with a small margin.
pub fn camera_for(lo: Vec3, hi: Vec3) -> Camera {
    let centre = (lo + hi) * 0.5;
    let fov = FOV_DEG.to_radians();
    // Its front, unless it runs along its front (a fence or wall section
    // along local y, seen end-on from there): then its side.
    let size = hi - lo;
    let front = if size.y > size.x * 1.5 { Vec3::X } else { super::props::FRONT };
    // The direction from the solid to the camera: its front, turned
    // clockwise seen from above, and raised.
    let (turn, raise) = (-TURN_DEG.to_radians(), RAISE_DEG.to_radians());
    let flat = glam::Mat3::from_rotation_z(turn) * front;
    let to_eye = (flat * raise.cos() + Vec3::Z * raise.sin()).normalize();
    let forward = -to_eye;
    let right = forward.cross(Vec3::Z).normalize();
    let up = right.cross(forward);
    // Each corner at (x, y) across the view and z along it from the
    // centre needs |x|, |y| <= (d - z) tan(fov / 2) · MARGIN.
    let t = (fov * 0.5).tan() * 0.9;
    let mut d: f32 = 0.1;
    let mut reach: f32 = 0.0;
    for k in 0..8 {
        let c = Vec3::new(
            if k & 1 == 0 { lo.x } else { hi.x },
            if k & 2 == 0 { lo.y } else { hi.y },
            if k & 4 == 0 { lo.z } else { hi.z },
        ) - centre;
        let (x, y, z) = (c.dot(right), c.dot(up), c.dot(forward));
        d = d.max(x.abs().max(y.abs()) / t - z);
        reach = reach.max(c.length());
    }
    let mut c = Camera::new(centre + to_eye * d, forward.x.atan2(forward.y), forward.z.asin());
    c.fov_y = fov;
    c.near = (d - reach * 1.2).max(0.02);
    c.far = d + reach * 2.0 + 1.0;
    c
}

/// The world a picture is drawn from: the `solids` of `world` (with their
/// textures), each placed once at the origin, upright, all hidden.
pub fn picture_world(world: &World, solids: &[usize]) -> World {
    let mut w = World::default();
    for (k, &i) in solids.iter().enumerate() {
        let Some(s) = world.solids.get(i) else { continue };
        for t in &s.textures {
            if let Some(tex) = world.textures.get(t) {
                w.textures.entry(*t).or_insert_with(|| tex.clone());
            }
        }
        w.solids.push(s.clone());
        w.placements.push(Placement {
            solid: k,
            low_solid: None,
            mid_solid: Some(k),
            transform: Mat4::IDENTITY,
            section: 1,
            flags: HIDDEN_FLAG,
            cull_radius: (glam::Vec3::from_array(s.bbox_max) - glam::Vec3::from_array(s.bbox_min)).length(),
        });
    }
    w
}

/// The pictures' drawing side: the renderer of a [`picture_world`], its
/// target and the readback buffer.
struct Painter {
    /// The region's solid of each placement of the picture world.
    solids: Vec<usize>,
    world: World,
    renderer: WorldRenderer,
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    readback: wgpu::Buffer,
}

impl Painter {
    fn new(gpu: &Gpu, world: &World, solids: Vec<usize>) -> Painter {
        let pw = picture_world(world, &solids);
        let renderer = WorldRenderer::new(gpu, &pw, FORMAT, &RenderOptions::in_force());
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("prop picture"),
            size: wgpu::Extent3d { width: PX, height: PX, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("prop pictures readback"),
            size: u64::from(PX * PX * 4) * PER_FRAME as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Painter {
            solids,
            world: pw,
            renderer,
            color: texture.create_view(&Default::default()),
            depth: create_depth(gpu, PX, PX),
            readback,
        }
    }

    /// Draw the solids at `slots` (indices into `self.solids`), one frame
    /// each, and read them back.
    fn draw(&mut self, gpu: &Gpu, slots: &[usize]) -> Vec<Picture> {
        let opts = RenderOptions::in_force();
        let time_of_day = crate::sky::TimeOfDay::default();
        let bytes = u64::from(PX * PX * 4);
        for (n, &k) in slots.iter().enumerate() {
            let Some(p) = self.world.placements.get(k) else { continue };
            let s = &self.world.solids[p.solid];
            let camera = camera_for(Vec3::from_array(s.bbox_min), Vec3::from_array(s.bbox_max));
            // Only this one shown (each frame its own submit: the
            // renderer's uniforms go through the queue).
            self.renderer.update_placement(gpu, k, Mat4::IDENTITY, 0);
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            let params = FrameParams {
                camera: &camera,
                aspect: 1.0,
                visible_sections: None,
                far_sections: None,
                fog: None,
                time: 0.0,
                road_reflections: false,
                road_reflection_plane: None,
                sky: time_of_day.sky(),
                reversed: false,
                headlight_pools: &[],
                reflection_glows: &[],
                display: Default::default(),
                opts: &opts,
                rear_view: None,
                occluders: &[],
            };
            self.renderer.render(gpu, &mut encoder, &self.color, &self.depth, (PX, PX), &params);
            encoder.copy_texture_to_buffer(
                self.color.texture().as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &self.readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: bytes * n as u64,
                        bytes_per_row: Some(PX * 4),
                        rows_per_image: None,
                    },
                },
                wgpu::Extent3d { width: PX, height: PX, depth_or_array_layers: 1 },
            );
            gpu.queue.submit([encoder.finish()]);
            self.renderer.update_placement(gpu, k, Mat4::IDENTITY, HIDDEN_FLAG);
        }
        let slice = self.readback.slice(..bytes * slots.len() as u64);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        if gpu.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).is_err() {
            return Vec::new();
        }
        let out = {
            let Ok(data) = slice.get_mapped_range() else { return Vec::new() };
            data.chunks(bytes as usize).map(|c| cut_background(c.to_vec())).collect()
        };
        self.readback.unmap();
        out
    }
}

/// A solid's picture as the browser has it.
enum Slot {
    /// Waiting for [`Thumbs::build`].
    Queued,
    /// Ready, not yet an egui texture.
    Picture(Picture),
    Texture(egui::TextureHandle),
    /// Nothing to show (the solid draws nothing in the picture).
    Empty,
}

/// The pictures of one region's solids ([`super::browser::Browser`] asks,
/// `App::render_view` builds).
pub struct Thumbs {
    /// The cache folder (None: pictures are made each run, not kept).
    dir: Option<PathBuf>,
    /// The region and solid count the slots are for.
    region: (String, usize),
    slots: HashMap<usize, Slot>,
    queue: VecDeque<usize>,
    painter: Option<Painter>,
    /// Pictures drawn and pictures read from the cache (tests, the log).
    pub drawn: usize,
    pub read: usize,
}

impl Default for Thumbs {
    /// The asset cache's pictures (in tests none: they never read the
    /// player's cache).
    fn default() -> Thumbs {
        Thumbs::new(if cfg!(test) { None } else { cache_dir() })
    }
}

impl Thumbs {
    pub fn new(dir: Option<PathBuf>) -> Thumbs {
        Thumbs {
            dir,
            region: (String::new(), 0),
            slots: HashMap::new(),
            queue: VecDeque::new(),
            painter: None,
            drawn: 0,
            read: 0,
        }
    }

    /// Start over for another region (or a reloaded world).
    fn for_region(&mut self, region: &str, world: &World) {
        let r = (region.to_string(), world.solids.len());
        if self.region != r {
            self.region = r;
            self.slots.clear();
            self.queue.clear();
            self.painter = None;
        }
    }

    /// Ask for the pictures of `solids` of `world`: those the cache holds
    /// are read now, the others queued for [`Thumbs::build`].
    pub fn request(&mut self, region: &str, world: &World, solids: impl IntoIterator<Item = usize>) {
        self.for_region(region, world);
        for i in solids {
            if self.slots.contains_key(&i) {
                continue;
            }
            let cached = self.dir.as_ref().and_then(|d| load(&file(d, key(world, i))));
            match cached {
                Some(p) => {
                    self.read += 1;
                    self.slots.insert(i, if p.coverage() == 0 { Slot::Empty } else { Slot::Picture(p) });
                }
                None => {
                    self.slots.insert(i, Slot::Queued);
                    self.queue.push_back(i);
                }
            }
        }
    }

    /// Pictures still to draw.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// The picture of solid `i` when it is ready (made an egui texture on
    /// first use).
    pub fn texture(&mut self, ctx: &egui::Context, i: usize) -> Option<egui::TextureId> {
        let slot = self.slots.get_mut(&i)?;
        if let Slot::Picture(p) = slot {
            let t = ctx.load_texture(format!("prop-thumb-{i}"), p.image(), egui::TextureOptions::LINEAR);
            *slot = Slot::Texture(t);
        }
        match slot {
            Slot::Texture(t) => Some(t.id()),
            _ => None,
        }
    }

    /// The picture of solid `i` if it is ready and not yet a texture
    /// (tests).
    pub fn picture(&self, i: usize) -> Option<&Picture> {
        match self.slots.get(&i)? {
            Slot::Picture(p) => Some(p),
            _ => None,
        }
    }

    /// Draw up to [`PER_FRAME`] queued pictures with the game's renderer
    /// and keep them in the cache. True when it drew any.
    pub fn build(&mut self, gpu: &Gpu, region: &str, world: &World) -> bool {
        self.for_region(region, world);
        if self.queue.is_empty() {
            return false;
        }
        // The picture world holds every solid queued so far; one asked for
        // later than it was made makes it again.
        if self.painter.as_ref().is_some_and(|p| self.queue.iter().any(|i| !p.solids.contains(i))) {
            self.painter = None;
        }
        if self.painter.is_none() {
            let mut solids: Vec<usize> = self.queue.iter().copied().collect();
            solids.sort_unstable();
            self.painter = Some(Painter::new(gpu, world, solids));
        }
        let Some(painter) = self.painter.as_mut() else { return false };
        let batch: Vec<usize> = (0..PER_FRAME).map_while(|_| self.queue.pop_front()).collect();
        let slots: Vec<usize> =
            batch.iter().map(|i| painter.solids.iter().position(|s| s == i).unwrap_or(usize::MAX)).collect();
        let pictures = painter.draw(gpu, &slots);
        for (i, p) in batch.into_iter().zip(pictures) {
            self.drawn += 1;
            if let Some(d) = &self.dir
                && let Err(e) = store(&file(d, key(world, i)), &p)
            {
                log::warn!("prop pictures: cannot keep {}: {e}", world.solids[i].name);
            }
            self.slots.insert(i, if p.coverage() == 0 { Slot::Empty } else { Slot::Picture(p) });
        }
        if self.queue.is_empty() {
            log::info!("prop pictures: {} drawn, {} from the cache", self.drawn, self.read);
            // The picture world is not needed until another region.
            self.painter = None;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::Scratch;

    /// A world with one 1 m box solid (no mesh) placed once.
    fn world(name: &str) -> World {
        let mut w = World::default();
        w.solids.push(crate::formats::solid::Solid {
            name: name.into(),
            hash: 1,
            bbox_min: [-0.5; 3],
            bbox_max: [0.5; 3],
            textures: vec![],
            vertices: vec![],
            normals: vec![],
            light_materials: vec![],
            indices: vec![],
            materials: vec![],
            mesh_flags: 0,
            markers: vec![],
            light_markers: vec![],
        });
        w
    }

    fn picture() -> Picture {
        let mut rgba = vec![0u8; (PX * PX * 4) as usize];
        for p in rgba.chunks_mut(4).skip(3000).take(500) {
            p.copy_from_slice(&[200, 120, 40, 255]);
        }
        Picture { rgba }
    }

    #[test]
    fn the_key_follows_what_the_picture_shows() {
        let a = world("XC_CONE_01");
        assert_eq!(key(&a, 0), key(&world("XC_CONE_01"), 0), "the same solid in another world");
        assert_ne!(key(&a, 0), key(&world("XC_CONE_02"), 0));
        let mut b = world("XC_CONE_01");
        b.solids[0].bbox_max[2] = 2.0;
        assert_ne!(key(&a, 0), key(&b, 0), "a changed solid");
    }

    /// A picture in the cache is read, not drawn again; a missing one is
    /// queued; nothing is read or queued twice; another region starts over.
    #[test]
    fn the_cache_is_read_before_anything_is_drawn() {
        let sc = Scratch::new("prop-thumbs");
        let dir = sc.path().join(DIR);
        let mut w = world("XC_CONE_01");
        w.solids.push(world("XS_SIGN_STOP").solids.remove(0));
        store(&file(&dir, key(&w, 0)), &picture()).unwrap();
        let mut t = Thumbs::new(Some(dir.clone()));
        t.request("L4RA", &w, [0, 1]);
        assert_eq!((t.read, t.pending()), (1, 1));
        assert_eq!(t.picture(0), Some(&picture()), "the PNG round trip keeps every pixel");
        t.request("L4RA", &w, [0, 1]);
        assert_eq!((t.read, t.pending()), (1, 1), "asked again: nothing new");
        t.request("L4RB", &w, [0]);
        assert_eq!((t.read, t.pending()), (2, 0), "another region starts over");
        // No cache folder: everything is drawn.
        let mut t = Thumbs::new(None);
        t.request("L4RA", &w, [0, 1]);
        assert_eq!((t.read, t.pending()), (0, 2));
    }

    #[test]
    fn the_background_is_cut_by_the_corners_colour() {
        let mut rgba = vec![30u8; (PX * PX * 4) as usize];
        let mid = ((PX / 2 * PX + PX / 2) * 4) as usize;
        rgba[mid..mid + 4].copy_from_slice(&[200, 100, 50, 255]);
        let p = cut_background(rgba);
        assert_eq!(p.coverage(), 1);
        assert_eq!(p.rgba[mid + 3], 255);
        assert_eq!(p.rgba[3], 0);
    }

    #[test]
    fn the_camera_frames_the_box_from_the_front() {
        let (lo, hi) = (Vec3::new(-0.3, -0.05, 0.0), Vec3::new(0.3, 0.05, 2.0));
        let c = camera_for(lo, hi);
        let centre = (lo + hi) * 0.5;
        assert!(c.position.y < centre.y - 1.0, "in front (local −y): {}", c.position);
        assert!(c.position.z > centre.z && c.position.x < centre.x - 0.5, "raised, turned a little: {}", c.position);
        assert!((c.forward() - (centre - c.position).normalize()).length() < 1e-3, "looks at the centre");
        assert!(c.near > 0.0 && c.near < c.position.distance(centre) - 1.0);
    }

    /// With the game's files and a GPU: pictures of a few of the city's
    /// solids are drawn by the game's renderer, kept, and read back the
    /// next time instead of being drawn again.
    #[test]
    #[ignore = "requires a GPU adapter and NFSU2_ROOT; run serially with --ignored"]
    fn pictures_are_drawn_once_and_reused() {
        let root = crate::testkit::game_root();
        let world = World::load(&root, "L4RA").unwrap();
        let gpu = pollster::block_on(Gpu::new(
            None,
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()),
        ))
        .unwrap();
        // A sign known by its face, and the first solid of each filter.
        use super::super::browser::Filter;
        let all = super::super::browser::entries(&world);
        let mut ids: Vec<usize> =
            all.iter().filter(|e| e.name.contains("SIGNALAHEAD")).map(|e| e.solid).take(1).collect();
        for f in [Filter::KnockOver, Filter::Barriers, Filter::Signs, Filter::StreetFurniture, Filter::Trees] {
            let more: Vec<usize> = all
                .iter()
                .filter(|e| f.matches(e) && !e.name.starts_with("LIGHTMASK") && !ids.contains(&e.solid))
                .map(|e| e.solid)
                .take(2)
                .collect();
            ids.extend(more);
        }
        let names: Vec<&str> = ids.iter().map(|&i| world.solids[i].name.as_str()).collect();
        assert_eq!(ids.len(), 11, "{names:?}");
        let sc = Scratch::new("prop-thumbs-gpu");
        let dir = sc.path().join(DIR);
        let mut t = Thumbs::new(Some(dir.clone()));
        t.request("L4RA", &world, ids.iter().copied());
        assert_eq!(t.pending(), ids.len());
        while t.build(&gpu, "L4RA", &world) {}
        assert_eq!((t.drawn, t.read, t.pending()), (ids.len(), 0, 0));
        let none: Vec<&str> =
            ids.iter().filter(|i| t.picture(**i).is_none()).map(|&i| world.solids[i].name.as_str()).collect();
        assert!(none.is_empty(), "no picture: {none:?} of {names:?}");
        let first: Vec<Picture> = ids.iter().map(|i| t.picture(*i).expect("a picture").clone()).collect();
        if let Ok(out) = std::env::var("THUMB_DUMP") {
            for (p, i) in first.iter().zip(&ids) {
                let path = PathBuf::from(&out).join(format!("{}.png", world.solids[*i].name));
                store(&path, p).unwrap();
            }
        }
        for (p, n) in first.iter().zip(names) {
            let c = p.coverage();
            assert!(c > 20 && c < (PX * PX) as usize * 9 / 10, "{n}: {c} pixels");
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), ids.len(), "one file each");
        let mut again = Thumbs::new(Some(dir));
        again.request("L4RA", &world, ids.iter().copied());
        assert_eq!((again.read, again.pending()), (ids.len(), 0), "all from the cache");
        assert!(!again.build(&gpu, "L4RA", &world), "nothing to draw");
        for (p, i) in first.iter().zip(&ids) {
            assert_eq!(again.picture(*i), Some(p));
        }
    }
}
