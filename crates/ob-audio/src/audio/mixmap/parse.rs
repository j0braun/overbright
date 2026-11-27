use super::*;

/// One 3D control's variant (0x18 bytes): w0 the view selector (byte 3
/// & 0xF) and the sources (bits 12–15 distance: 0 camera, 1 car, else
/// −1; bits 8–11 the azimuth: 0 camera, 1 car, else kept), w1 the four
/// curve types (bits 16..31), then per direction {lo, hi} metres.
pub(super) type Variant = [u32; 6];

#[derive(Clone, Debug)]
pub(super) struct ThreeD {
    pub(super) id: u32,
    pub(super) variants: Vec<Variant>,
}

#[derive(Clone, Debug)]
pub(super) struct Control {
    pub(super) id: u32,
    /// Its word 1: bit 15 clear → +0x24 = w & 0x7FFF (a boost), +0x28 =
    /// 0x7FFF − lin(−that); set → +0x24 = 0, +0x28 = 0x7FFF − lin(w as
    /// i16) (0x477B60).
    pub(super) boost: i32,
    pub(super) k: i32,
    pub(super) input: u32,
}

#[derive(Clone, Debug)]
pub(super) struct Evt {
    /// {id (byte 3 & 0xF the mode), V (u16 mB, > 10000 negative), the
    /// scale's id, the trigger's id, attack, hold, release} (0x468900;
    /// the time words: bits 0–11 × 16.67 ms, bits 12–15 the curve).
    pub(super) w: [u32; 7],
}

#[derive(Clone, Debug)]
pub(super) struct Chan {
    pub(super) base: i32,
    pub(super) max: i32,
    pub(super) min: i32,
    pub(super) out: u32,
    /// The 3D connections (the 0x8 / 0x9 input words, in order).
    pub(super) threed: Vec<u32>,
    pub(super) inputs: Vec<u32>,
    /// The output map (master channels): {slot bits 16–19, block dword
    /// 24–27, 3D connection 20–23, mB offset i16}.
    pub(super) map: Vec<u32>,
}

/// One section (an object group's mix).
#[derive(Clone, Debug, Default)]
pub struct Section {
    pub(super) controls: Vec<Control>,
    pub(super) threed: Vec<ThreeD>,
    pub(super) evts: Vec<Evt>,
    pub(super) subs: Vec<Chan>,
    pub(super) masters: Vec<Chan>,
}

/// A parsed .mxb.
#[derive(Clone, Debug, Default)]
pub struct MixMap {
    pub sections: Vec<Section>,
}

pub(super) fn rd(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

impl MixMap {
    /// The file (0x467610 / 0x477F80 / 0x477EB0 / 0x477DF0 / 0x480270 /
    /// 0x480320 / 0x479030): +4 the section count, +8 the offset of their
    /// offsets; a section: +4 controls, +8 3D controls, +0xC sub
    /// channels, +0x10 master channels, +0x14 the master output maps,
    /// +0x18 event controls (offsets from the section, −1 none); each
    /// block a count and 12 bytes before its entries.
    pub fn parse(d: &[u8]) -> Option<MixMap> {
        let n = rd(d, 4)? as usize;
        let tab = rd(d, 8)? as usize;
        let mut sections = Vec::new();
        for s in 0..n.min(32) {
            let b = rd(d, tab + 4 * s)? as usize;
            let off = |k: usize| rd(d, b + 4 * k).map(|v| v as i32).filter(|&v| v >= 0).map(|v| b + v as usize);
            let mut sec = Section::default();
            if let Some(o) = off(1) {
                let mut p = o + 16;
                for _ in 0..rd(d, o)? {
                    let (id, w, input) = (rd(d, p)?, rd(d, p + 4)?, rd(d, p + 8)?);
                    let (boost, k) = if w & 0x8000 == 0 {
                        ((w & 0x7FFF) as i32, 0x7FFF - db_to_lin(-((w & 0x7FFF) as i32)))
                    } else {
                        (0, 0x7FFF - db_to_lin((w | 0xFFFF_0000) as i32))
                    };
                    sec.controls.push(Control { id, boost, k, input });
                    p += 12;
                }
            }
            if let Some(o) = off(2) {
                let mut p = o + 16;
                for _ in 0..rd(d, o)? {
                    let nv = (*d.get(p + 3)? & 0xF) as usize;
                    let mut variants = Vec::new();
                    for v in 0..nv {
                        let q = p + 4 + 24 * v;
                        let mut w = [0u32; 6];
                        for (j, x) in w.iter_mut().enumerate() {
                            *x = rd(d, q + 4 * j)?;
                        }
                        variants.push(w);
                    }
                    sec.threed.push(ThreeD { id: rd(d, p)?, variants });
                    p += 4 + 24 * nv;
                }
            }
            if let Some(o) = off(6) {
                let mut p = o + 16;
                for _ in 0..rd(d, o)? {
                    let mut w = [0u32; 7];
                    for (j, x) in w.iter_mut().enumerate() {
                        *x = rd(d, p + 4 * j)?;
                    }
                    sec.evts.push(Evt { w });
                    p += 28;
                }
            }
            for (k, list) in [(3usize, 0usize), (4, 1)] {
                let Some(o) = off(k) else { continue };
                let mut p = o + 16;
                for _ in 0..rd(d, o)? {
                    let (id, w1) = (rd(d, p)?, rd(d, p + 4)?);
                    let n = ((id >> 16) & 0xFF) as usize;
                    let (base, max, min, out, start) = if id & 0x1000_0000 == 0 {
                        let up = (w1 >> 16) as i16 as i32;
                        (-up, (w1 >> 16) as i32, -up - 10000, rd(d, p + 8)?, p + 12)
                    } else {
                        (0, ((w1 >> 16) & 0x7FFF) as i32, (w1 | 0xFFFF_0000) as i32, 0, p + 8)
                    };
                    let words: Vec<u32> = (0..n).map(|i| rd(d, start + 4 * i)).collect::<Option<_>>()?;
                    let (threed, inputs) = words.iter().partition(|&&w| w & 0xE000_0000 == 0x8000_0000);
                    let c = Chan { base, max, min, out, threed, inputs, map: Vec::new() };
                    if list == 0 {
                        sec.subs.push(c)
                    } else {
                        sec.masters.push(c)
                    }
                    p = start + 4 * n;
                }
            }
            if let Some(mut p) = off(5) {
                for m in sec.masters.iter_mut() {
                    let h = rd(d, p)?;
                    m.map = (0..(h & 0xF) as usize).map(|i| rd(d, p + 4 + 4 * i)).collect::<Option<_>>()?;
                    p += 4 + 4 * (h & 0xF) as usize;
                }
            }
            sections.push(sec);
        }
        Some(MixMap { sections })
    }

    /// The map 0x4869A0 loads: 0 MAPOUTPUT, 1 DFT (drift), 2 DRG (drag),
    /// 3 URL.
    /// Every object field a section reads (ids of kind 4, a sound
    /// object's field, and 6, a controller's: the controls' inputs and
    /// own ids, the event controls' triggers and scales, the 3D
    /// controls' position objects), sorted.
    pub fn field_ids(&self, s: usize) -> Vec<u32> {
        let Some(sec) = self.sections.get(s) else {
            return Vec::new();
        };
        let mut v: Vec<u32> = sec
            .controls
            .iter()
            .flat_map(|c| [c.id, c.input])
            .chain(sec.evts.iter().flat_map(|e| [e.w[2], e.w[3]]))
            .chain(sec.threed.iter().map(|t| 0x6000_0000 | (t.id & 0x00FF_FFF0)))
            .filter(|id| matches!(id >> 28, 4 | 6))
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    pub fn file_name(k: usize) -> &'static str {
        ["MAPOUTPUT.mxb", "MAPOUTPUTDFT.mxb", "MAPOUTPUTDRG.mxb", "MAPOUTPUTURL.mxb"][k.min(3)]
    }

    pub fn load(root: &std::path::Path, k: usize) -> Option<MixMap> {
        let path = crate::game_fs::find_in(root, "SOUND/MixMaps", Self::file_name(k))?;
        MixMap::parse(&crate::game_fs::read_file(&path).ok()?)
    }
}
