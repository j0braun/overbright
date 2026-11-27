//! The Browser's index: the game's assets of the loaded region, searched by name across kinds, with where each is
//! used. Studio's Browser workspace reads it; it holds no window code, so the same answers can be tested here.
//!
//! * A model is a solid of the region. Its uses are the placements that stand it (shown or hidden), the streaming
//!   sections they are in, the placements that use it only as a lower level of detail, and the textures its
//!   materials draw.
//! * A texture is a world texture of the region. Its uses are the models whose materials draw it, with how many
//!   materials each, and (through them) the sections it is seen in.
//!
//! The index is computed once from the loaded world ([`Index::build`]; the window keeps it while the world is the
//! same) and every search and every "used by" answer reads it: nothing is looked up again per row. Only the
//! loaded region is indexed; a region the mod is not edited on is not loaded, and cars live in other files.

use std::collections::{HashMap, HashSet};

use nfsu2::studio_api::world::{HIDDEN_FLAG, Placement, World};
use ob_formats::formats::solid::Solid;
use ob_formats::formats::texture::Texture;

use crate::browse::{Entry, Filter};
use media::{Line, Sound};
use crate::looks::{Domain, TextureId};

/// A model of the region.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    /// Its index in the world's solids.
    pub solid: usize,
    pub name: String,
    pub hash: u32,
    pub knock_over: bool,
    /// The placements that stand it as their model (indices into the world's placements, in order).
    pub placements: Vec<u32>,
    /// How many of them are shown (the others are hidden by a mod).
    pub shown: usize,
    /// Placements that draw it only as a lower level of detail.
    pub as_lod: usize,
    /// The sections its placements are in, with how many each, by section number.
    pub sections: Vec<(u32, usize)>,
    /// The textures its materials draw (texture hash, materials), by hash.
    pub textures: Vec<(u32, usize)>,
    pub triangles: usize,
}

impl Model {
    /// How many placements stand it (shown or hidden).
    pub fn placed(&self) -> usize {
        self.placements.len()
    }

    /// The prop browser's entry for it, for [`Filter::matches`] and the category words.
    pub fn entry(&self) -> Entry {
        Entry { solid: self.solid, name: self.name.clone(), knock_over: self.knock_over, placed: self.shown }
    }
}

/// A texture of the region.
#[derive(Clone, Debug, PartialEq)]
pub struct Tex {
    pub hash: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// It is the base of a flip-book.
    pub animated: bool,
    /// The models whose materials draw it, with how many materials each (by index in [`Index::models`], ascending).
    pub used_by: Vec<(usize, usize)>,
}

impl Tex {
    /// Its Looks id (`world:0x…`).
    pub fn id(&self) -> TextureId {
        TextureId::new(Domain::World, self.hash)
    }

    /// How many materials draw it, in all models.
    pub fn materials(&self) -> usize {
        self.used_by.iter().map(|u| u.1).sum()
    }
}

/// What the search looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    All,
    Models,
    Textures,
    /// The game's sound banks (read when asked for; not in All).
    Sounds,
    /// A language file's lines (read when asked for; not in All).
    Strings,
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::All, Kind::Models, Kind::Textures, Kind::Sounds, Kind::Strings];

    pub fn name(self) -> &'static str {
        match self {
            Kind::All => "All",
            Kind::Models => "Models",
            Kind::Textures => "Textures",
            Kind::Sounds => "Sounds",
            Kind::Strings => "Strings",
        }
    }
}

/// The textures' filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TexFilter {
    #[default]
    Any,
    /// Drawn by a model that is placed.
    InUse,
    /// Drawn by no placed model.
    Unused,
    /// A flip-book's base.
    Animated,
    /// 512 px or more on a side.
    Large,
}

impl TexFilter {
    pub const ALL: [TexFilter; 5] =
        [TexFilter::Any, TexFilter::InUse, TexFilter::Unused, TexFilter::Animated, TexFilter::Large];

    pub fn name(self) -> &'static str {
        match self {
            TexFilter::Any => "Any",
            TexFilter::InUse => "In use",
            TexFilter::Unused => "Unused",
            TexFilter::Animated => "Animated",
            TexFilter::Large => "Large",
        }
    }
}

/// A search: words of the name (all must be in it, case ignored), `section:N` (what is placed or seen in that
/// section) and a hash (`0x1A2B3C4D`), with the filters of each kind.
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub text: String,
    pub kind: Kind,
    pub models: Filter,
    pub textures: TexFilter,
}

impl Default for Query {
    /// Everything: the prop browser's default category (knock-over) would hide most models.
    fn default() -> Query {
        Query { text: String::new(), kind: Kind::All, models: Filter::All, textures: TexFilter::Any }
    }
}

/// One result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// An index into [`Index::models`].
    Model(usize),
    /// An index into [`Index::textures`].
    Texture(usize),
    /// An index into [`Index::sounds`].
    Sound(usize),
    /// An index into [`Index::lines`].
    Line(usize),
}

/// The words of a search text, split into name words, a section and a hash.
struct Parsed {
    words: Vec<String>,
    section: Option<u32>,
    hash: Option<u32>,
}

fn parse(text: &str) -> Parsed {
    let mut p = Parsed { words: vec![], section: None, hash: None };
    for w in text.split_whitespace() {
        let lower = w.to_ascii_lowercase();
        if let Some(n) = lower.strip_prefix("section:").or_else(|| lower.strip_prefix("sec:")) {
            match n.parse() {
                Ok(n) => p.section = Some(n),
                // "section:" alone, or half typed: no restriction yet.
                Err(_) => {}
            }
        } else if let Some(h) = lower.strip_prefix("0x").filter(|h| !h.is_empty() && h.len() <= 8) {
            match u32::from_str_radix(h, 16) {
                Ok(h) => p.hash = Some(h),
                Err(_) => p.words.push(lower),
            }
        } else {
            p.words.push(lower);
        }
    }
    p
}

/// The index of one region's assets.
#[derive(Clone, Debug, Default)]
pub struct Index {
    pub region: String,
    pub models: Vec<Model>,
    pub textures: Vec<Tex>,
    /// The game's sounds, once read ([`Index::set_sounds`]).
    pub sounds: Option<Vec<Sound>>,
    /// The language shown and the lines of its file, once read ([`Index::set_lines`]), and the languages there are.
    pub language: String,
    pub languages: Vec<String>,
    pub lines: Option<Vec<Line>>,
    tex_at: HashMap<u32, usize>,
    model_at: HashMap<usize, usize>,
}

/// What a built index was made from: the cache's key.
pub type Signature = (String, usize, usize, usize, usize);

impl Index {
    /// What `world` and the mod's prop count make of the index; an index with the same signature is current.
    pub fn signature(region: &str, world: &World, mod_props: usize) -> Signature {
        (region.to_string(), world.solids.len(), world.placements.len(), world.textures.len(), mod_props)
    }

    /// An index of a region with nothing in it (the sounds and lines are read into it without a world).
    pub fn empty(region: &str) -> Index {
        Index { region: region.to_string(), ..Index::default() }
    }

    /// The index of a loaded region.
    pub fn build(region: &str, world: &World) -> Index {
        let knock: HashSet<usize> =
            world.smackables.iter().filter_map(|p| world.placements.get(p.placement)).map(|p| p.solid).collect();
        let animated: HashSet<u32> = world.texture_animations.keys().copied().collect();
        Index::from_parts(region, &world.solids, &world.placements, &world.textures, &knock, &animated)
    }

    /// The index of `solids` placed by `placements`, with the world's `textures`.
    pub fn from_parts(
        region: &str,
        solids: &[Solid],
        placements: &[Placement],
        textures: &HashMap<u32, Texture>,
        knock_over: &HashSet<usize>,
        animated: &HashSet<u32>,
    ) -> Index {
        let mut by_solid: Vec<Vec<u32>> = vec![vec![]; solids.len()];
        let mut lod = vec![0usize; solids.len()];
        for (i, p) in placements.iter().enumerate() {
            if let Some(v) = by_solid.get_mut(p.solid) {
                v.push(i as u32);
            }
            for l in [p.low_solid, p.mid_solid].into_iter().flatten() {
                if l != p.solid
                    && let Some(n) = lod.get_mut(l)
                {
                    *n += 1;
                }
            }
        }
        let mut tex: HashMap<u32, Tex> = textures
            .values()
            .map(|t| {
                let t = Tex {
                    hash: t.hash,
                    name: t.name.clone(),
                    width: t.width,
                    height: t.height,
                    animated: animated.contains(&t.hash),
                    used_by: vec![],
                };
                (t.hash, t)
            })
            .collect();
        let mut models: Vec<Model> = solids
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let list = std::mem::take(&mut by_solid[i]);
                let mut sections: HashMap<u32, usize> = HashMap::new();
                let mut shown = 0;
                for &pi in &list {
                    let p = &placements[pi as usize];
                    *sections.entry(p.section).or_default() += 1;
                    if p.flags & HIDDEN_FLAG == 0 {
                        shown += 1;
                    }
                }
                let mut sections: Vec<(u32, usize)> = sections.into_iter().collect();
                sections.sort_unstable();
                let mut drawn: HashMap<u32, usize> = HashMap::new();
                for m in &s.materials {
                    if let Some(&h) = s.textures.get(m.texture_slot as usize) {
                        *drawn.entry(h).or_default() += 1;
                    }
                }
                let mut drawn: Vec<(u32, usize)> = drawn.into_iter().collect();
                drawn.sort_unstable();
                Model {
                    solid: i,
                    name: s.name.clone(),
                    hash: s.hash,
                    knock_over: knock_over.contains(&i),
                    placements: list,
                    shown,
                    as_lod: lod[i],
                    sections,
                    textures: drawn,
                    triangles: s.indices.len() / 3,
                }
            })
            .collect();
        models.sort_by(|a, b| a.name.cmp(&b.name).then(a.solid.cmp(&b.solid)));
        for (mi, m) in models.iter().enumerate() {
            for &(h, n) in &m.textures {
                if let Some(t) = tex.get_mut(&h) {
                    t.used_by.push((mi, n));
                }
            }
        }
        let mut textures: Vec<Tex> = tex.into_values().collect();
        textures.sort_by(|a, b| a.name.cmp(&b.name).then(a.hash.cmp(&b.hash)));
        let tex_at = textures.iter().enumerate().map(|(i, t)| (t.hash, i)).collect();
        let model_at = models.iter().enumerate().map(|(i, m)| (m.solid, i)).collect();
        Index { region: region.to_string(), models, textures, tex_at, model_at, ..Default::default() }
    }

    /// The model of world solid `solid`.
    pub fn model_of_solid(&self, solid: usize) -> Option<usize> {
        self.model_at.get(&solid).copied()
    }

    /// The texture with `hash`.
    pub fn texture_of_hash(&self, hash: u32) -> Option<usize> {
        self.tex_at.get(&hash).copied()
    }

    /// Whether a placed (shown) model draws `t`.
    fn in_use(&self, t: &Tex) -> bool {
        t.used_by.iter().any(|&(m, _)| self.models.get(m).is_some_and(|m| m.shown > 0))
    }

    fn model_in_section(m: &Model, sec: u32) -> bool {
        m.sections.binary_search_by_key(&sec, |s| s.0).is_ok()
    }

    fn texture_in_section(&self, t: &Tex, sec: u32) -> bool {
        t.used_by.iter().any(|&(m, _)| self.models.get(m).is_some_and(|m| Self::model_in_section(m, sec)))
    }

    /// The results of `q`: models first, then textures, each by name.
    pub fn search(&self, q: &Query) -> Vec<Hit> {
        let p = parse(&q.text);
        let named = |name: &str, hash: u32| {
            let n = name.to_ascii_lowercase();
            if let Some(h) = p.hash
                && h == hash
            {
                return true;
            }
            // A hash alone finds only that hash; words must all be in the name.
            (!p.words.is_empty() || p.hash.is_none()) && p.words.iter().all(|w| n.contains(w.as_str()))
        };
        let mut out = vec![];
        if matches!(q.kind, Kind::All | Kind::Models) {
            for (i, m) in self.models.iter().enumerate() {
                if named(&m.name, m.hash)
                    && q.models.matches(&m.entry())
                    && p.section.is_none_or(|s| Self::model_in_section(m, s))
                {
                    out.push(Hit::Model(i));
                }
            }
        }
        if matches!(q.kind, Kind::All | Kind::Textures) {
            for (i, t) in self.textures.iter().enumerate() {
                if !named(&t.name, t.hash) || !p.section.is_none_or(|s| self.texture_in_section(t, s)) {
                    continue;
                }
                let ok = match q.textures {
                    TexFilter::Any => true,
                    TexFilter::InUse => self.in_use(t),
                    TexFilter::Unused => !self.in_use(t),
                    TexFilter::Animated => t.animated,
                    TexFilter::Large => t.width.max(t.height) >= 512,
                };
                if ok {
                    out.push(Hit::Texture(i));
                }
            }
        }
        if q.kind == Kind::Sounds {
            let all = self.sounds.as_deref().unwrap_or_default();
            out.extend(media::find_sounds(all, &q.text).into_iter().map(Hit::Sound));
        }
        if q.kind == Kind::Strings {
            let all = self.lines.as_deref().unwrap_or_default();
            out.extend(media::find_lines(all, &q.text).into_iter().map(Hit::Line));
        }
        out
    }

    /// Hand the index the game's sounds (read once, on the first look at them).
    pub fn set_sounds(&mut self, sounds: Vec<Sound>) {
        self.sounds = Some(sounds);
    }

    /// Hand the index a language file's lines, and which languages there are.
    pub fn set_lines(&mut self, language: &str, languages: Vec<String>, lines: Vec<Line>) {
        self.language = language.to_string();
        self.languages = languages;
        self.lines = Some(lines);
    }

    /// How many sounds and lines the words of `text` find (0 until they are read), for the kind chips.
    pub fn media_counts(&self, text: &str) -> (usize, usize) {
        (
            self.sounds.as_deref().map_or(0, |s| media::find_sounds(s, text).len()),
            self.lines.as_deref().map_or(0, |l| media::find_lines(l, text).len()),
        )
    }

    /// How many models and textures the words of `text` find (every filter at its widest), for the kind chips.
    pub fn counts(&self, text: &str) -> (usize, usize) {
        let all = Query { text: text.to_string(), ..Default::default() };
        let hits = self.search(&all);
        let m = hits.iter().filter(|h| matches!(h, Hit::Model(_))).count();
        (m, hits.len() - m)
    }

    /// The name of a hit.
    pub fn name(&self, h: Hit) -> &str {
        match h {
            Hit::Model(i) => &self.models[i].name,
            Hit::Texture(i) => &self.textures[i].name,
            Hit::Sound(i) => self.sounds.as_deref().and_then(|s| s.get(i)).map_or("", |s| s.name.as_str()),
            Hit::Line(i) => self.lines.as_deref().and_then(|l| l.get(i)).map_or("", |l| l.text.as_str()),
        }
    }

    /// The words of the "used by" line of a model.
    pub fn model_used(&self, i: usize) -> String {
        let m = &self.models[i];
        if m.placements.is_empty() {
            return if m.as_lod > 0 {
                format!("Only as a lower level of detail, by {} placements", m.as_lod)
            } else {
                "Not placed in this region".into()
            };
        }
        let hidden = m.placed() - m.shown;
        let mut s = format!(
            "{} placed in {} {}",
            m.placed(),
            m.sections.len(),
            if m.sections.len() == 1 { "section" } else { "sections" }
        );
        if hidden > 0 {
            s.push_str(&format!(" ({hidden} hidden)"));
        }
        s
    }

    /// The words of the "used by" line of a texture.
    pub fn texture_used(&self, i: usize) -> String {
        let t = &self.textures[i];
        if t.used_by.is_empty() {
            return "No model of this region draws it".into();
        }
        format!(
            "{} {} in {} {}",
            t.materials(),
            if t.materials() == 1 { "material" } else { "materials" },
            t.used_by.len(),
            if t.used_by.len() == 1 { "model" } else { "models" }
        )
    }

    /// The sections a texture is seen in: the sections of the models that draw it, with their placements.
    pub fn texture_sections(&self, i: usize) -> Vec<(u32, usize)> {
        let mut by: HashMap<u32, usize> = HashMap::new();
        for &(m, _) in &self.textures[i].used_by {
            for &(s, n) in &self.models[m].sections {
                *by.entry(s).or_default() += n;
            }
        }
        let mut v: Vec<_> = by.into_iter().collect();
        v.sort_unstable();
        v
    }
}

pub mod gltf;
pub mod import;
pub mod media;
pub mod mesh;
pub mod worker;

#[cfg(test)]
mod tests;
