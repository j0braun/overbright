//! On2 VP6 video, decoded as SPEED2.EXE's own codec does (the C paths of
//! the decoder at 0x702C9D: header 0x707CF8, frame 0x7077B2, macroblock
//! 0x70E69C, block 0x70E29A). Every table below is read from the EXE at
//! the address given; the routines are ports of the game's, step for
//! step (see docs/formats.md, "Movies").
//!
//! Supported: what the game's movies use — VP6 versions 0..7 (the
//! decoder refuses 8 and up, 0x707D86), progressive frames, range coded
//! or Huffman coded coefficients (0x707319 / 0x70E9B0), the simple
//! profile's bilinear motion compensation. Refused with an error
// prov: open formats.vp6.module
//! (**open**, not ported): interlaced frames (+0x1DC) and profiles ≠ 0
//! (their bicubic 0x704078 / 0x70423C and deblocking 0x71FDDF filters).
//! All 32 shipped movies are version 7, profile 0, progressive.

use anyhow::{Result, bail};

/// The AC dequantizer per quantizer, luma (0x7C4618, i32; the chroma table 0x7C4718 holds the same values).
static AC_DEQUANT: crate::exe_tables::Table<[i32; 64]> = crate::exe_tables::Table::new(|t| &t.vp6.ac_dequant);

/// The DC dequantizer per quantizer, luma (0x7C4C18, i16; the chroma table 0x7C4C98 holds the same values).
static DC_DEQUANT: crate::exe_tables::Table<[i16; 64]> = crate::exe_tables::Table::new(|t| &t.vp6.dc_dequant);

/// Zigzag index → raster position in the 8×8 block (0x7C5218; 0x71D618 hard-codes the same map).
static ZIGZAG: crate::exe_tables::Table<[u8; 64]> = crate::exe_tables::Table::new(|t| &t.vp6.zigzag);

/// Coefficient group per coefficient index for the AC models (0x7C56F0; index 0 is unused, -1).
static COEFF_GROUP: crate::exe_tables::Table<[i8; 64]> = crate::exe_tables::Table::new(|t| &t.vp6.coeff_group);

/// The default coefficient reorder (band per zigzag index) of a progressive frame (0x7C5A30).
static DEFAULT_REORDER: crate::exe_tables::Table<[u8; 64]> = crate::exe_tables::Table::new(|t| &t.vp6.default_reorder);

/// The reference frame of each macroblock type (0x7C5AB0: 0 intra, 1 previous, 2 golden).
static REF_OF_TYPE: crate::exe_tables::Table<[u8; 10]> = crate::exe_tables::Table::new(|t| &t.vp6.ref_of_type);

/// Whether a macroblock type predicts by motion compensation (0x816660).
const MC_TYPE: [u8; 10] = [
    0, 0, 1, 1, 1, 0, 1, 1, 1, 1,
];

/// The DC model update probabilities per plane (0x7C5FC0, 2 × 11).
static DCCV_UPDATE: crate::exe_tables::Table<[u8; 22]> = crate::exe_tables::Table::new(|t| &t.vp6.dccv_update);

/// The reorder update probability per zigzag index (0x7C5FD8).
static REORDER_UPDATE: crate::exe_tables::Table<[u8; 64]> = crate::exe_tables::Table::new(|t| &t.vp6.reorder_update);

/// The run model update probabilities (0x7C6018, 2 × 14).
static RUNV_UPDATE: crate::exe_tables::Table<[u8; 28]> = crate::exe_tables::Table::new(|t| &t.vp6.runv_update);

/// The key frame run model (0x7C6034, 2 × 14).
static RUNV_DEFAULT: crate::exe_tables::Table<[u8; 28]> = crate::exe_tables::Table::new(|t| &t.vp6.runv_default);

/// The AC model update probabilities, [ct][plane][group][node] (0x7C6050, 3 × 2 × 6 × 11).
static RACT_UPDATE: crate::exe_tables::Table<[u8; 396]> = crate::exe_tables::Table::new(|t| &t.vp6.ract_update);

/// The DC context probabilities' linear combinations, [node][context] (mult, add) (0x7C61E0).
static DCCV_LC: crate::exe_tables::Table<[i32; 30]> = crate::exe_tables::Table::new(|t| &t.vp6.dccv_lc);

/// The motion vector model update probabilities per component (0x7C6258, 2 × 17: dct, sig, 7 short, 8 long).
static VECTOR_UPDATE: crate::exe_tables::Table<[u8; 34]> = crate::exe_tables::Table::new(|t| &t.vp6.vector_update);

/// The macroblock type statistics presets, [context][preset][type] (count1, count0) pairs (0x7C62A8, 3 × 16 × 10 × 2).
static MB_STATS_PRESETS: crate::exe_tables::Table<[u8; 960]> = crate::exe_tables::Table::new(|t| &t.vp6.mb_stats_presets);

/// The key frame macroblock type statistics, per context the 10 tree counts then the 10 same-type counts (0x7C6668).
static MB_STATS_DEFAULT: crate::exe_tables::Table<[u8; 60]> = crate::exe_tables::Table::new(|t| &t.vp6.mb_stats_default);

/// The DCT token categories 5..10 (0x7C5630 + 16 × category): base, last bit, probabilities by bit.
static CAT_TABLE: crate::exe_tables::Table<[u8; 176]> = crate::exe_tables::Table::new(|t| &t.vp6.cat_table);

/// The motion vector candidates, (row, column) offsets (0x7C5528).
static CANDIDATES: crate::exe_tables::Table<[i32; 24]> = crate::exe_tables::Table::new(|t| &t.vp6.candidates);

/// The bilinear filter taps per eighth (0x816708).
static BILINEAR: crate::exe_tables::Table<[i32; 16]> = crate::exe_tables::Table::new(|t| &t.vp6.bilinear);

/// The plane borders the decoder keeps around a frame for motion vectors
/// that point outside (0x704B8B: luma stride = width + 0x60; chroma half).
const BORDER: usize = 48;
/// The macroblock grid's border (0x704B8B: rows / columns + 6; the loop
/// in 0x7077B2 runs from 3).
const MB_BORDER: usize = 3;

/// The range decoder (state at +0x150 / +0x170; init 0x707BCA).
struct Rac<'a> {
    buf: &'a [u8],
    pos: usize,
    value: u32,
    count: i32,
    range: u32,
}

impl<'a> Rac<'a> {
    fn new(buf: &'a [u8]) -> Rac<'a> {
        let b = |i: usize| buf.get(i).copied().unwrap_or(0) as u32;
        Rac { buf, pos: 4, value: b(0) << 24 | b(1) << 16 | b(2) << 8 | b(3), count: 8, range: 0xFF }
    }

    fn byte(&mut self) -> u32 {
        let v = self.buf.get(self.pos).copied().unwrap_or(0) as u32;
        self.pos += 1;
        v
    }

    /// A bit at probability `p` / 256 of 0 (0x70D801, 0x70D9E8).
    fn prob(&mut self, p: u8) -> bool {
        let split = 1 + (((self.range.wrapping_sub(1)).wrapping_mul(p as u32)) >> 8);
        let big = split << 24;
        let bit = self.value >= big;
        if bit {
            self.range -= split;
            self.value = self.value.wrapping_sub(big);
        } else {
            self.range = split;
        }
        while self.range < 0x80 {
            self.range <<= 1;
            self.value <<= 1;
            self.count -= 1;
            if self.count == 0 {
                self.value |= self.byte();
                self.count = 8;
            }
        }
        bit
    }

    /// An even bit with exactly one shift (0x70D87A; the sign reader
    /// 0x706A61 is the same): the range may reach 256.
    fn half(&mut self) -> bool {
        let split = (self.range + 1) >> 1;
        let big = split << 24;
        let bit = self.value >= big;
        if bit {
            self.value = self.value.wrapping_sub(big) << 1;
            self.range = (self.range - split) * 2;
        } else {
            self.value <<= 1;
            self.range = split * 2;
        }
        self.count -= 1;
        if self.count == 0 {
            self.value |= self.byte();
            self.count = 8;
        }
        bit
    }

    /// `n` even bits, most significant first (0x707C18).
    fn bits(&mut self, n: u32) -> u32 {
        (0..n).rev().fold(0, |v, i| v | (self.half() as u32) << i)
    }

    /// A model probability update: 7 bits, doubled, 0 read as 1
    /// (0x706F35, 0x70F1F3).
    fn p7(&mut self) -> u8 {
        let v = (self.bits(7) << 1) as u8;
        if v == 0 { 1 } else { v }
    }
}

/// The plain header bit reader (0x707C7A), most significant bit first.
struct Bits<'a> {
    buf: &'a [u8],
    at: usize,
}

impl Bits<'_> {
    fn read(&mut self, n: usize) -> u32 {
        let mut v = 0;
        for _ in 0..n {
            let b = self.buf.get(self.at / 8).copied().unwrap_or(0);
            v = v << 1 | ((b >> (7 - self.at % 8)) & 1) as u32;
            self.at += 1;
        }
        v
    }
}

/// One plane with its border.
#[derive(Clone)]
struct Plane {
    data: Vec<u8>,
    stride: usize,
    rows: usize,
    border: usize,
}

impl Plane {
    fn new(width: usize, height: usize, border: usize) -> Plane {
        let stride = width + 2 * border;
        let rows = height + 2 * border;
        Plane { data: vec![0; stride * rows], stride, rows, border }
    }

    /// A pixel at (x, y) in buffer coordinates; reads past the border
    /// clamp to it (the game reads whatever memory is there).
    fn at(&self, x: isize, y: isize) -> u8 {
        let x = x.clamp(0, self.stride as isize - 1) as usize;
        let y = y.clamp(0, self.rows as isize - 1) as usize;
        self.data[y * self.stride + x]
    }

    /// 0x704D87: copy the edge pixels into the border, the rows first,
    /// then the first and last rows (with their borders) up and down.
    fn extend(&mut self) {
        let (s, b) = (self.stride, self.border);
        let w = s - 2 * b;
        let h = self.rows - 2 * b;
        for y in b..b + h {
            let row = &mut self.data[y * s..(y + 1) * s];
            let (l, r) = (row[b], row[b + w - 1]);
            row[..b].fill(l);
            row[b + w..].fill(r);
        }
        for y in 0..b {
            self.data.copy_within(b * s..(b + 1) * s, y * s);
            self.data.copy_within((b + h - 1) * s..(b + h) * s, (b + h + y) * s);
        }
    }
}

/// A decoded frame: Y, U, V with borders.
#[derive(Clone)]
pub struct Frame {
    planes: [Plane; 3],
}

impl Frame {
    fn new(width: usize, height: usize) -> Frame {
        Frame { planes: [Plane::new(width, height, BORDER), Plane::new(width / 2, height / 2, BORDER / 2), Plane::new(width / 2, height / 2, BORDER / 2)] }
    }

    /// Plane `p`'s visible pixels, row by row.
    pub fn plane(&self, p: usize) -> impl Iterator<Item = &[u8]> {
        let pl = &self.planes[p];
        let w = pl.stride - 2 * pl.border;
        let h = pl.rows - 2 * pl.border;
        (0..h).map(move |y| &pl.data[(y + pl.border) * pl.stride + pl.border..][..w])
    }
}

/// A block context (the 16-byte entries above, +0x10C..+0x114, and to the
/// left, +0xCC..+0xFC): whether the DC was coded, the reference frame
/// (4 = none) and the DC.
#[derive(Clone, Copy)]
struct Context {
    nz: u8,
    reference: u16,
    dc: i16,
}

const NO_CONTEXT: Context = Context { nz: 0, reference: 4, dc: 0 };

/// The decoder (the codec instance at 0x702C9D's EBX).
pub struct Decoder {
    pub width: usize,
    pub height: usize,
    mb_cols: usize,
    mb_rows: usize,
    version: u8,
    profile: u8,
    current: Frame,
    previous: Frame,
    golden: Frame,
    // Models (+0x3A0 on).
    dccv: [[u8; 11]; 2],
    ract: [[[[u8; 11]; 6]; 3]; 2],
    dcct: [[[u8; 5]; 3]; 2],
    runv: [[u8; 14]; 2],
    reorder: [u8; 64],
    idx_to_pos: [u8; 64],
    /// +0x5FC and what follows it in memory (the reorder table, then the
    /// 22 bytes set to 0x80 on key frames): the end-of-block class read
    /// at (index − 1) may go past 64 after a long zero run.
    max_pos: [u8; 64],
    mb_stats: [[u8; 20]; 3],
    same_prob: [[u8; 10]; 3],
    type_tree: [[[u8; 9]; 10]; 3],
    vec_sig: [u8; 2],
    vec_dct: [u8; 2],
    pdv: [[u8; 7]; 2],
    fdv: [[u8; 8]; 2],
    /// Per macroblock in the grid with its border: type and vector.
    mb_types: Vec<u8>,
    mb_vectors: Vec<(i16, i16)>,
    prev_type: u8,
    dequant: [[i16; 64]; 2],
    /// The Huffman tables built from the models (0x706D62): DC per plane,
    /// zero runs by position, AC per [ct][plane][group].
    huff_dc: Vec<Huffman>,
    huff_run: Vec<Huffman>,
    huff_ac: Vec<Huffman>,
    /// Blocks left whose DC (row 0) / AC (row 1) is skipped, per plane
    /// (+0x4524..+0x4530).
    skip: [[i32; 2]; 2],
    /// Whether a key frame has been decoded.
    started: bool,
}

/// What one macroblock carries between its parse and its blocks.
struct Macroblock {
    kind: u8,
    vectors: [(i16, i16); 6],
}

/// The vector candidates of 0x70DE83 for one reference.
#[derive(Default, Clone, Copy)]
struct Candidates {
    first: usize,
    nearest: (i16, i16),
    near: (i16, i16),
}

fn unpack(v: u32) -> (i16, i16) {
    (v as u16 as i16, (v >> 16) as u16 as i16)
}

fn pack(v: (i16, i16)) -> u32 {
    v.0 as u16 as u32 | (v.1 as u16 as u32) << 16
}

impl Default for Decoder {
    fn default() -> Self {
        Decoder::new()
    }
}

impl Decoder {
    pub fn new() -> Decoder {
        Decoder {
            width: 0,
            height: 0,
            mb_cols: 0,
            mb_rows: 0,
            version: 0,
            profile: 0,
            current: Frame::new(0, 0),
            previous: Frame::new(0, 0),
            golden: Frame::new(0, 0),
            dccv: [[0x80; 11]; 2],
            ract: [[[[0x80; 11]; 6]; 3]; 2],
            dcct: [[[0x80; 5]; 3]; 2],
            runv: [[0x80; 14]; 2],
            reorder: *DEFAULT_REORDER,
            idx_to_pos: [0; 64],
            max_pos: [0; 64],
            mb_stats: [[0; 20]; 3],
            same_prob: [[0; 10]; 3],
            type_tree: [[[0; 9]; 10]; 3],
            vec_sig: [0x80; 2],
            vec_dct: [0xA2, 0xA4],
            pdv: [[0; 7]; 2],
            fdv: [[0; 8]; 2],
            mb_types: Vec::new(),
            mb_vectors: Vec::new(),
            prev_type: 0,
            dequant: [[0; 64]; 2],
            huff_dc: Vec::new(),
            huff_run: Vec::new(),
            huff_ac: Vec::new(),
            skip: [[0; 2]; 2],
            started: false,
        }
    }

    /// The frame last decoded (+0x254 after 0x702C9D's swap).
    pub fn frame(&self) -> &Frame {
        &self.previous
    }

    fn grid_stride(&self) -> usize {
        self.mb_cols + 2 * MB_BORDER
    }

    fn allocate(&mut self, cols: usize, rows: usize) {
        self.mb_cols = cols;
        self.mb_rows = rows;
        self.width = cols * 16;
        self.height = rows * 16;
        self.current = Frame::new(self.width, self.height);
        self.previous = self.current.clone();
        self.golden = self.current.clone();
        let n = (cols + 2 * MB_BORDER) * (rows + 2 * MB_BORDER);
        self.mb_types = vec![1; n];
        self.mb_vectors = vec![(0, 0); n];
    }

    /// 0x706AC2: the coefficient order from the reorder bands, and the
    /// running maximum position (+1 from version 7, which picks the
    /// inverse transform).
    fn order(&mut self) {
        self.idx_to_pos[0] = 0;
        let mut n = 1;
        for band in 0..16 {
            for pos in 1..64 {
                if self.reorder[pos] == band {
                    self.idx_to_pos[n] = pos as u8;
                    n += 1;
                }
            }
        }
        let mut m = 0;
        for i in 0..64 {
            m = m.max(self.idx_to_pos[i]);
            self.max_pos[i] = if self.version > 6 { m + 1 } else { m };
        }
    }

    /// The end-of-block class at an index, reading on into the bytes
    /// that follow +0x5FC as the game does.
    fn eob_class(&self, i: usize) -> u8 {
        match i {
            0..64 => self.max_pos[i],
            64..128 => self.reorder[i - 64],
            _ => 0x80,
        }
    }

    /// 0x70390C / 0x70394D: the dequantizers, DC and AC × 4.
    fn set_quantizer(&mut self, q: usize) {
        for t in &mut self.dequant {
            t[0] = DC_DEQUANT[q] << 2;
            for v in &mut t[1..] {
                *v = (AC_DEQUANT[q] << 2) as i16;
            }
        }
    }

    /// Decode one frame (a chunk's payload, 0x702E47 → 0x702C9D).
    pub fn decode(&mut self, buf: &[u8]) -> Result<()> {
        // The bytes from `n` on (none when the buffer is shorter).
        let tail = |n: usize| buf.get(n..).unwrap_or(&[]);
        let mut bits = Bits { buf, at: 0 };
        let inter = bits.read(1) != 0;
        let q = bits.read(6) as usize;
        let separated = bits.read(1) != 0;
        let mut offset = 0usize;
        let mut rac;
        let mut golden_update = false;
        if inter {
            if !self.started {
                bail!("an inter frame before any key frame");
            }
            if !separated && self.profile != 0 {
                rac = Rac::new(tail(1));
            } else {
                offset = bits.read(16) as usize;
                rac = Rac::new(tail(3));
            }
            golden_update = rac.prob(0x80);
            if self.profile != 0 && rac.prob(0x80) {
                rac.prob(0x80);
            }
        } else {
            // Kept only once the frame is accepted: a refused key frame
            // leaves the decoder as it was.
            let version = bits.read(5) as u8;
            let profile = bits.read(2) as u8;
            if version > 7 {
                bail!("VP6 version {version} (the game decodes 0..7)");
            }
            if bits.read(1) != 0 {
                bail!("interlaced VP6 frames are not ported");
            }
            if profile != 0 {
                bail!("VP6 profile {profile} (its filters are not ported)");
            }
            (self.version, self.profile) = (version, profile);
            if !separated && self.profile != 0 {
                rac = Rac::new(tail(2));
            } else {
                offset = bits.read(16) as usize;
                rac = Rac::new(tail(4));
            }
            let rows = rac.bits(8) as usize;
            let cols = rac.bits(8) as usize;
            rac.bits(8);
            rac.bits(8);
            rac.bits(2);
            if rows == 0 || cols == 0 {
                bail!("a VP6 frame of {cols} × {rows} macroblocks");
            }
            if cols != self.mb_cols || rows != self.mb_rows || !self.started {
                self.allocate(cols, rows);
            }
            if self.profile != 0 {
                if rac.prob(0x80) {
                    rac.bits(5);
                    rac.bits(3);
                } else {
                    rac.prob(0x80);
                }
            }
            self.started = true;
        }
        let huffman = rac.prob(0x80);
        self.set_quantizer(q);
        // 0x702C9D: the coefficients' own coder when separated, or always
        // in the simple profile: a range coder, or with Huffman codes a
        // bit reader (+0x190..+0x198).
        let data = tail(offset);
        let mut coeffs = match (separated || self.profile == 0, huffman) {
            (true, false) => Coefficients::Range(Rac::new(data)),
            (true, true) => Coefficients::Huffman(HuffBits { buf: data, pos: 0, left: 0, word: 0 }),
            (false, false) => Coefficients::Shared,
            (false, true) => bail!("Huffman coefficients without their own partition"),
        };
        self.frame_body(&mut rac, &mut coeffs, inter)?;
        // The new frame becomes the previous one; its border is extended
        // (0x704D87); a key frame or a golden update copies it.
        std::mem::swap(&mut self.current, &mut self.previous);
        for p in &mut self.previous.planes {
            p.extend();
        }
        if !inter || golden_update {
            self.golden = self.previous.clone();
        }
        Ok(())
    }

    /// 0x7077B2.
    fn frame_body<'a>(&mut self, rac: &mut Rac<'a>, coeffs: &mut Coefficients<'a>, inter: bool) -> Result<()> {
        if !inter {
            for (c, s) in self.mb_stats.iter_mut().enumerate() {
                s.copy_from_slice(&MB_STATS_DEFAULT[c * 20..c * 20 + 20]);
            }
            self.vec_dct = [0xA2, 0xA4];
            self.vec_sig = [0x80, 0x80];
            self.pdv = [[0xE1, 0x92, 0xAC, 0x93, 0xD6, 0x27, 0x9C], [0xCC, 0xAA, 0x77, 0xEB, 0x8C, 0xE6, 0xE4]];
            self.fdv = [[0xF7, 0xD2, 0x87, 0x44, 0x8A, 0xDC, 0xEF, 0xF6], [0xF4, 0xB8, 0xC9, 0x2C, 0xAD, 0xDD, 0xEF, 0xFD]];
            self.mb_types.fill(1);
            self.reorder = *DEFAULT_REORDER;
            self.order();
        } else {
            self.mb_type_models(rac);
            self.vector_models(rac);
            self.prev_type = 0;
        }
        self.coefficient_models(rac, !inter);
        if matches!(coeffs, Coefficients::Huffman(_)) {
            self.huffman_tables();
        }
        self.skip = [[0; 2]; 2];
        // 0x70717E: the contexts above and the DC predictors.
        let cols = self.mb_cols;
        let mut above_y = vec![NO_CONTEXT; 2 * (cols + 2 * MB_BORDER) + 8];
        let mut above_u = vec![NO_CONTEXT; cols + 2 * MB_BORDER + 8];
        let mut above_v = above_u.clone();
        if self.version < 6 {
            above_u[1] = Context { nz: 0, reference: 0, dc: 0 };
            above_v[1] = Context { nz: 0, reference: 0, dc: 0 };
        }
        let mut prev_dc = [[0i16; 4]; 3];
        prev_dc[1][0] = 0x80;
        prev_dc[2][0] = 0x80;
        let mut block = [[0i16; 64]; 6];
        for row in 0..self.mb_rows {
            // 0x70710A: the contexts to the left.
            let mut left = [NO_CONTEXT; 4];
            for col in 0..cols {
                let mb = if inter { self.parse_mb(rac, row, col) } else { Macroblock { kind: 1, vectors: [(0, 0); 6] } };
                let gc = col + MB_BORDER;
                for b in 0..6 {
                    let (plane, pt) = match b {
                        0..4 => (0, 0),
                        4 => (1, 1),
                        _ => (2, 1),
                    };
                    let (above, left_ctx) = match b {
                        0 => (&mut above_y[gc * 2], 0),
                        1 => (&mut above_y[gc * 2 + 1], 0),
                        2 => (&mut above_y[gc * 2], 1),
                        3 => (&mut above_y[gc * 2 + 1], 1),
                        4 => (&mut above_u[gc], 2),
                        _ => (&mut above_v[gc], 3),
                    };
                    let left = &mut left[left_ctx];
                    let class = match coeffs {
                        Coefficients::Range(c) => self.tokens(c, pt, &mut block[b], &mut above.nz, &mut left.nz),
                        Coefficients::Shared => self.tokens(rac, pt, &mut block[b], &mut above.nz, &mut left.nz),
                        Coefficients::Huffman(h) => self.huffman_tokens(h, pt, &mut block[b]),
                    };
                    // 0x70E92E: the DC prediction.
                    let r = REF_OF_TYPE[mb.kind as usize] as u16;
                    let mut n = 0;
                    let mut sum: u32 = 0;
                    if left.reference == r {
                        sum = left.dc as i32 as u32;
                        n = 1;
                    }
                    if above.reference == r {
                        sum = sum.wrapping_add(above.dc as i32 as u32);
                        n += 1;
                    }
                    if n == 0 {
                        sum = prev_dc[plane][r as usize] as i32 as u32;
                    }
                    if n == 2 {
                        sum = (sum.wrapping_add((sum & 0xFFFF) >> 15) as i32 >> 1) as u32;
                    }
                    block[b][0] = block[b][0].wrapping_add(sum as i16);
                    prev_dc[plane][r as usize] = block[b][0];
                    left.dc = block[b][0];
                    above.dc = block[b][0];
                    left.reference = r;
                    above.reference = r;
                    let residual = idct(&mut block[b], &self.dequant[pt], class);
                    self.reconstruct(&mb, b, row, col, &residual);
                }
            }
        }
        Ok(())
    }

    /// 0x707319: one block's tokens; returns the end-of-block class.
    fn tokens(&self, rac: &mut Rac, pt: usize, out: &mut [i16; 64], above: &mut u8, left: &mut u8) -> u8 {
        let dccv = &self.dccv[pt];
        let dcct = &self.dcct[pt][(*above + *left) as usize];
        let mut ct;
        if !rac.prob(dcct[0]) {
            *left = 0;
            *above = 0;
            ct = 0;
        } else {
            *left = 1;
            *above = 1;
            let v = if !rac.prob(dcct[2]) {
                ct = 1;
                1
            } else {
                ct = 2;
                if !rac.prob(dcct[3]) {
                    if !rac.prob(dcct[4]) { 2 } else { 3 + rac.prob(dccv[5]) as i32 }
                } else {
                    category(rac, [dccv[6], dccv[7], dccv[8], dccv[9], dccv[10]])
                }
            };
            out[0] = if rac.half() { -v } else { v } as i16;
        }
        let mut idx: u8 = 1;
        loop {
            let i = idx as usize;
            let model = &self.ract[pt][ct][COEFF_GROUP[i] as usize];
            if (idx < 2 || ct != 0) && !rac.prob(model[0]) {
                if !rac.prob(model[1]) {
                    idx += 1;
                    break;
                }
                ct = 0;
                let m = &self.runv[(idx > 5) as usize];
                let run = if !rac.prob(m[0]) {
                    if !rac.prob(m[1]) { 1 + rac.prob(m[2]) as u8 } else { 3 + rac.prob(m[3]) as u8 }
                } else if !rac.prob(m[4]) {
                    if !rac.prob(m[5]) { 5 + rac.prob(m[6]) as u8 } else { 7 + rac.prob(m[7]) as u8 }
                } else {
                    let mut r = 9;
                    for k in 0..6 {
                        r += (rac.prob(m[8 + k]) as u8) << k;
                    }
                    r
                };
                idx = idx.wrapping_add(run);
            } else {
                let v = if !rac.prob(model[2]) {
                    ct = 1;
                    1
                } else {
                    ct = 2;
                    if !rac.prob(model[3]) {
                        if !rac.prob(model[4]) { 2 } else { 3 + rac.prob(model[5]) as i32 }
                    } else {
                        category(rac, [model[6], model[7], model[8], model[9], model[10]])
                    }
                };
                let v = if rac.half() { -v } else { v };
                out[self.idx_to_pos[i] as usize] = v as i16;
                idx += 1;
            }
            if idx >= 64 {
                break;
            }
        }
        self.eob_class(idx.wrapping_sub(1) as usize)
    }

    /// 0x70E1EE: predict the block and add the residual.
    fn reconstruct(&mut self, mb: &Macroblock, b: usize, row: usize, col: usize, residual: &[i16; 64]) {
        let (p, x, y) = match b {
            0..4 => (0, BORDER + col * 16 + (b & 1) * 8, BORDER + row * 16 + (b >> 1) * 8),
            4 => (1, BORDER / 2 + col * 8, BORDER / 2 + row * 8),
            _ => (2, BORDER / 2 + col * 8, BORDER / 2 + row * 8),
        };
        let kind = mb.kind as usize;
        let mut pred = [0i16; 64];
        if kind == 1 {
            // 0x71D39E: intra, the residual + 128.
            for v in pred.iter_mut() {
                *v = 128;
            }
        } else {
            let reference = if REF_OF_TYPE[kind] == 2 { &self.golden } else { &self.previous };
            let plane = &reference.planes[p];
            if MC_TYPE[kind] != 0 {
                motion(plane, x, y, mb.vectors[b], b < 4, &mut pred);
            } else {
                // 0x71D42A: the same place in the reference.
                for r in 0..8 {
                    for c in 0..8 {
                        pred[r * 8 + c] = plane.data[(y + r) * plane.stride + x + c] as i16;
                    }
                }
            }
        }
        // 0x71D296: the sum clamped to 0..255.
        let plane = &mut self.current.planes[p];
        for r in 0..8 {
            for c in 0..8 {
                let v = pred[r * 8 + c].wrapping_add(residual[r * 8 + c]);
                plane.data[(y + r) * plane.stride + x + c] = v.clamp(0, 255) as u8;
            }
        }
    }

    /// 0x70E42E: an inter macroblock's type and vectors.
    fn parse_mb(&mut self, rac: &mut Rac, row: usize, col: usize) -> Macroblock {
        let g = (row + MB_BORDER) * self.grid_stride() + col + MB_BORDER;
        let (ctx, prev) = self.candidates(g, 1);
        let kind = self.mb_type(rac, ctx);
        self.prev_type = kind;
        self.mb_types[g] = kind;
        let mut vectors = [(0i16, 0i16); 6];
        if kind == 7 {
            // Four vectors: each luma block's own type (0x70DB16).
            let mut sub = [0u8; 4];
            for s in &mut sub {
                let v = (rac.half() as u8) * 2 + rac.half() as u8;
                *s = [0, 2, 3, 4][v as usize];
            }
            let (mut sx, mut sy) = (0i32, 0i32);
            for (b, s) in sub.iter().enumerate() {
                let v = match s {
                    0 => (0, 0),
                    3 => prev.nearest,
                    4 => prev.near,
                    _ => self.delta(rac, if prev.first < 2 { prev.nearest } else { (0, 0) }),
                };
                if *s != 0 {
                    sx += v.0 as i32;
                    sy += v.1 as i32;
                }
                vectors[b] = v;
            }
            self.mb_vectors[g] = vectors[3];
            let avg = |s: i32| ((s + if s >= 0 { 2 } else { 1 }) >> 2) as i16;
            vectors[4] = (avg(sx), avg(sy));
            vectors[5] = vectors[4];
        } else {
            let v = match kind {
                2 => self.delta(rac, if prev.first < 2 { prev.nearest } else { (0, 0) }),
                3 => prev.nearest,
                4 => prev.near,
                6 => {
                    let (_, gold) = self.candidates(g, 2);
                    self.delta(rac, if gold.first < 2 { gold.nearest } else { (0, 0) })
                }
                8 => self.candidates(g, 2).1.nearest,
                9 => self.candidates(g, 2).1.near,
                _ => (0, 0),
            };
            self.mb_vectors[g] = v;
            vectors = [v; 6];
        }
        Macroblock { kind, vectors }
    }

    /// 0x70DE83: the first and second distinct non-zero vectors among the
    /// 12 neighbours using `reference`, and the type context (0 two, 2
    /// one, 1 none).
    fn candidates(&self, g: usize, reference: u8) -> (usize, Candidates) {
        let gs = self.grid_stride() as isize;
        let at = |k: usize| -> Option<u32> {
            let i = g as isize + CANDIDATES[2 * k] as isize * gs + CANDIDATES[2 * k + 1] as isize;
            let i = usize::try_from(i).ok()?;
            let t = *self.mb_types.get(i)?;
            let v = pack(*self.mb_vectors.get(i)?);
            (REF_OF_TYPE[t as usize] == reference && v != 0).then_some(v)
        };
        let mut c = Candidates { first: 12, ..Default::default() };
        let mut ctx = 1;
        let mut nearest = 0;
        for k in 0..12 {
            if let Some(v) = at(k) {
                nearest = v;
                c.first = k;
                ctx = 2;
                break;
            }
        }
        let mut near = 0;
        for k in c.first + 1..12 {
            if let Some(v) = at(k).filter(|&v| v != nearest) {
                near = v;
                ctx = 0;
                break;
            }
        }
        c.nearest = unpack(nearest);
        c.near = unpack(near);
        (ctx, c)
    }

    /// 0x70DB59: the macroblock type.
    fn mb_type(&self, rac: &mut Rac, ctx: usize) -> u8 {
        let prev = self.prev_type as usize;
        if rac.prob(self.same_prob[ctx][prev]) {
            return prev as u8;
        }
        let p = &self.type_tree[ctx][prev];
        if !rac.prob(p[0]) {
            if !rac.prob(p[1]) { (rac.prob(p[3]) as u8) << 1 } else { 3 + rac.prob(p[4]) as u8 }
        } else if !rac.prob(p[2]) {
            if rac.prob(p[5]) { 7 } else { 1 }
        } else if !rac.prob(p[6]) {
            5 + rac.prob(p[7]) as u8
        } else {
            8 + rac.prob(p[8]) as u8
        }
    }

    /// 0x70DC4C: a coded vector, added to `base`.
    fn delta(&self, rac: &mut Rac, base: (i16, i16)) -> (i16, i16) {
        let mut out = [base.0, base.1];
        for (comp, o) in out.iter_mut().enumerate() {
            let v = if !rac.prob(self.vec_dct[comp]) {
                let p = &self.pdv[comp];
                if !rac.prob(p[0]) {
                    if !rac.prob(p[1]) { rac.prob(p[2]) as i32 } else { 2 + rac.prob(p[3]) as i32 }
                } else if !rac.prob(p[4]) {
                    4 + rac.prob(p[5]) as i32
                } else {
                    6 + rac.prob(p[6]) as i32
                }
            } else {
                let f = &self.fdv[comp];
                let mut v = 0;
                for (bit, k) in [(0, 0), (1, 1), (2, 2), (7, 7), (6, 6), (5, 5), (4, 4)] {
                    v |= (rac.prob(f[k]) as i32) << bit;
                }
                if v & 0xF0 == 0 { v + 8 } else { v + ((rac.prob(f[3]) as i32) << 3) }
            };
            let v = if v != 0 && rac.prob(self.vec_sig[comp]) { -v } else { v };
            *o = o.wrapping_add(v as i16);
        }
        (out[0], out[1])
    }

    /// 0x70F5AC: the macroblock type statistics, then 0x70F32B.
    fn mb_type_models(&mut self, rac: &mut Rac) {
        for ctx in 0..3 {
            if rac.prob(174) {
                let preset = rac.bits(4) as usize;
                let at = (ctx * 16 + preset) * 20;
                for i in 0..10 {
                    self.mb_stats[ctx][10 + i] = MB_STATS_PRESETS[at + 2 * i];
                    self.mb_stats[ctx][i] = MB_STATS_PRESETS[at + 2 * i + 1];
                }
            }
            if rac.prob(254) {
                for i in 0..10 {
                    let d = stats_delta(rac);
                    self.mb_stats[ctx][10 + i] = (self.mb_stats[ctx][10 + i] as i32 + d).clamp(0, 255) as u8;
                    let d = stats_delta(rac);
                    self.mb_stats[ctx][i] = (self.mb_stats[ctx][i] as i32 + d).clamp(0, 255) as u8;
                }
            }
        }
        for prev in 0..10 {
            for ctx in 0..3 {
                let st = &self.mb_stats[ctx];
                let s: [u32; 10] = std::array::from_fn(|i| if i == prev { 0 } else { st[i] as u32 * 100 });
                let total: u32 = s.iter().sum();
                let (a, b) = (st[prev] as u64, st[10 + prev] as u64);
                self.same_prob[ctx][prev] = 255 - ((b * 255) / (a + 1 + b)) as u8;
                let q = |n: u32, d: u32| ((n * 255) / (d + 1)) as u8 + 1;
                let t = &mut self.type_tree[ctx][prev];
                let s0234 = s[2] + s[4] + s[3] + s[0];
                t[0] = q(s0234, total);
                t[1] = q(s[2] + s[0], s0234);
                let s5689 = s[6] + s[9] + s[8] + s[5];
                t[2] = q(s[1] + s[7], s5689 + s[7] + s[1]);
                t[3] = q(s[0], s[2] + s[0]);
                t[4] = q(s[3], s[4] + s[3]);
                t[5] = q(s[1], s[1] + s[7]);
                t[6] = q(s[6] + s[5], s5689);
                t[7] = q(s[5], s[6] + s[5]);
                t[8] = q(s[8], s[9] + s[8]);
            }
        }
    }

    /// 0x70F1F3: the vector models.
    fn vector_models(&mut self, rac: &mut Rac) {
        for comp in 0..2 {
            if rac.prob(VECTOR_UPDATE[comp * 17]) {
                self.vec_dct[comp] = rac.p7();
            }
            if rac.prob(VECTOR_UPDATE[comp * 17 + 1]) {
                self.vec_sig[comp] = rac.p7();
            }
        }
        for comp in 0..2 {
            for i in 0..7 {
                if rac.prob(VECTOR_UPDATE[comp * 17 + 2 + i]) {
                    self.pdv[comp][i] = rac.p7();
                }
            }
        }
        for comp in 0..2 {
            for i in 0..8 {
                if rac.prob(VECTOR_UPDATE[comp * 17 + 9 + i]) {
                    self.fdv[comp][i] = rac.p7();
                }
            }
        }
    }

    /// 0x706F35: the coefficient models, then 0x70F16E.
    fn coefficient_models(&mut self, rac: &mut Rac, key: bool) {
        let mut def = [0x80u8; 11];
        for pt in 0..2 {
            for node in 0..11 {
                if rac.prob(DCCV_UPDATE[pt * 11 + node]) {
                    def[node] = rac.p7();
                    self.dccv[pt][node] = def[node];
                } else if key {
                    self.dccv[pt][node] = def[node];
                }
            }
        }
        if key {
            self.runv[0].copy_from_slice(&RUNV_DEFAULT[..14]);
            self.runv[1].copy_from_slice(&RUNV_DEFAULT[14..]);
        }
        if rac.prob(0x80) {
            for pos in 1..64 {
                if rac.prob(REORDER_UPDATE[pos]) {
                    self.reorder[pos] = rac.bits(4) as u8;
                }
            }
            self.order();
        }
        for i in 0..2 {
            for node in 0..14 {
                if rac.prob(RUNV_UPDATE[i * 14 + node]) {
                    self.runv[i][node] = rac.p7();
                }
            }
        }
        let mut k = 0;
        for ct in 0..3 {
            for pt in 0..2 {
                for cg in 0..6 {
                    for node in 0..11 {
                        if rac.prob(RACT_UPDATE[k]) {
                            def[node] = rac.p7();
                            self.ract[pt][ct][cg][node] = def[node];
                        } else if key {
                            self.ract[pt][ct][cg][node] = def[node];
                        }
                        k += 1;
                    }
                }
            }
        }
        for pt in 0..2 {
            for ctx in 0..3 {
                for node in 0..5 {
                    let (mul, add) = (DCCV_LC[node * 6 + ctx * 2], DCCV_LC[node * 6 + ctx * 2 + 1]);
                    let v = (((self.dccv[pt][node] as u32).wrapping_mul(mul as u32).wrapping_add(0x80) as i32) >> 8) + add;
                    self.dcct[pt][ctx][node] = v.clamp(1, 255) as u8;
                }
            }
        }
    }
    /// 0x706D62: the Huffman trees and their 6-bit lookups from the
    /// current models.
    fn huffman_tables(&mut self) {
        self.huff_dc = (0..2).map(|pt| Huffman::new(&mut token_counts(&self.dccv[pt]))).collect();
        self.huff_run = (0..2).map(|i| Huffman::new(&mut run_counts(&self.runv[i]))).collect();
        self.huff_ac.clear();
        for ct in 0..3 {
            for pt in 0..2 {
                for cg in 0..6 {
                    let c = token_counts(&self.ract[pt][ct][cg]);
                    self.huff_ac.push(Huffman::new(&mut c.clone()));
                }
            }
        }
    }

    /// 0x70E9B0: one block's tokens from the Huffman codes.
    fn huffman_tokens(&mut self, h: &mut HuffBits, pt: usize, out: &mut [i16; 64]) -> u8 {
        let mut idx = 1usize;
        let mut ct;
        if self.skip[0][pt] < 1 {
            let token = h.read(&self.huff_dc[pt]);
            if token == 11 {
                return self.eob_class(idx - 1);
            }
            if token == 0 {
                self.skip[0][pt] = h.zero_run() - 1;
                ct = 0;
            } else {
                let v = h.token_value(token);
                let sign = h.bit();
                out[0] = if sign != 0 { -v } else { v } as i16;
                ct = if v > 1 { 2 } else { 1 };
            }
        } else {
            self.skip[0][pt] -= 1;
            ct = 0;
        }
        if self.skip[1][pt] < 1 {
            loop {
                let t = (COEFF_GROUP_HUFFMAN[idx] as usize) + (pt + ct * 2) * 6;
                let token = h.read(&self.huff_ac[t]);
                if token == 0 {
                    let r = h.read(&self.huff_run[(idx > 5) as usize]) as usize;
                    idx += if r < 8 { r } else { 8 + h.bits(6) as usize };
                    ct = 0;
                } else if token == 11 {
                    if idx == 1 {
                        self.skip[1][pt] = h.zero_run() - 1;
                    }
                    break;
                } else {
                    let v = h.token_value(token);
                    let sign = h.bit();
                    if let Some(&p) = self.idx_to_pos.get(idx) {
                        out[p as usize] = if sign != 0 { -v } else { v } as i16;
                    }
                    ct = if v > 1 { 2 } else { 1 };
                }
                idx += 1;
                if idx >= 64 {
                    break;
                }
            }
        } else {
            self.skip[1][pt] -= 1;
        }
        self.eob_class(idx - 1)
    }
}

/// Where a frame's coefficients come from.
enum Coefficients<'a> {
    Range(Rac<'a>),
    /// The header's coder (+0x150).
    Shared,
    Huffman(HuffBits<'a>),
}

/// The Huffman token values (0x7C58F8): token 0 a zero run (or a run of
/// zero DCs), 11 the end of block.
static TOKEN_BASE: crate::exe_tables::Table<[i32; 12]> = crate::exe_tables::Table::new(|t| &t.vp6.token_base);

/// The coefficient groups of the Huffman AC tables (0x7C5928).
static COEFF_GROUP_HUFFMAN: crate::exe_tables::Table<[i8; 64]> = crate::exe_tables::Table::new(|t| &t.vp6.coeff_group_huffman);

/// 0x706B27: the twelve token frequencies from a model's eleven
/// probabilities (each product >> 8 as the game truncates).
fn token_counts(p: &[u8; 11]) -> [u32; 12] {
    let p: [u32; 11] = p.map(|v| v as u32);
    let mut o = [0u32; 12];
    o[11] = p[0] * p[1] >> 8;
    o[0] = (0xFF - p[1]) * p[0] >> 8;
    o[1] = p[2] * (0xFF - p[0]) >> 8;
    let mut t = (0xFF - p[2]) * (0xFF - p[0]) >> 8;
    let mut u = p[3] * t >> 8;
    o[2] = p[4] * u >> 8;
    u = (0xFF - p[4]) * u >> 8;
    o[3] = p[5] * u >> 8;
    o[4] = (0xFF - p[5]) * u >> 8;
    t = (0xFF - p[3]) * t >> 8;
    u = p[6] * t >> 8;
    o[5] = p[7] * u >> 8;
    o[6] = (0xFF - p[7]) * u >> 8;
    t = (0xFF - p[6]) * t >> 8;
    u = p[8] * t >> 8;
    o[7] = p[9] * u >> 8;
    o[8] = (0xFF - p[9]) * u >> 8;
    u = (0xFF - p[8]) * t >> 8;
    o[9] = p[10] * u >> 8;
    o[10] = (0xFF - p[10]) * u >> 8;
    o
}

/// 0x706C67: the nine zero-run frequencies from a run model.
fn run_counts(p: &[u8; 14]) -> [u32; 9] {
    let p: [u32; 14] = p.map(|v| v as u32);
    let mut o = [0u32; 9];
    let mut u = p[0] * p[1] >> 8;
    o[0] = p[2] * u >> 8;
    o[1] = (0xFF - p[2]) * u >> 8;
    u = (0xFF - p[1]) * p[0] >> 8;
    o[2] = p[3] * u >> 8;
    o[3] = (0xFF - p[3]) * u >> 8;
    u = ((0xFF - p[0]) * p[4] >> 8) * p[5] >> 8;
    o[4] = p[6] * u >> 8;
    o[5] = (0xFF - p[6]) * u >> 8;
    u = ((0xFF - p[0]) * p[4] >> 8) * (0xFF - p[5]) >> 8;
    o[6] = p[7] * u >> 8;
    o[7] = (0xFF - p[7]) * u >> 8;
    o[8] = (0xFF - p[0]) * (0xFF - p[4]) >> 8;
    o
}

/// A Huffman tree (0x70EE7C) and its 6-bit lookup (0x70EFBE). A child is
/// a token × 2 + 1 or a node × 2; the root is node 0.
struct Huffman {
    nodes: Vec<[u32; 2]>,
    lookup: [u16; 64],
}

impl Huffman {
    /// 0x70EE7C: the symbols in a list sorted by frequency (a zero
    /// frequency counts 1; a new one goes before the first that is not
    /// smaller, 0x70EE33); the two least are merged into node n − 2,
    /// n − 3, ... down to the root 0, and the merged entry re-inserted.
    fn new(counts: &mut [u32]) -> Huffman {
        let n = counts.len();
        // List items: (next, count, value).
        let mut items: Vec<(i64, u32, u32)> = Vec::with_capacity(2 * n);
        for (i, c) in counts.iter_mut().enumerate() {
            if *c == 0 {
                *c = 1;
            }
            items.push((-1, *c, (i as u32) << 1 | 1));
        }
        let insert = |items: &mut Vec<(i64, u32, u32)>, head: &mut i64, new: usize| {
            let mut prev: i64 = -1;
            let mut cur = *head;
            while cur != -1 && items[new].1 > items[cur as usize].1 {
                prev = cur;
                cur = items[cur as usize].0;
            }
            if prev == -1 {
                *head = new as i64;
            } else {
                items[prev as usize].0 = new as i64;
            }
            items[new].0 = cur;
        };
        let mut head: i64 = 0;
        for i in 1..n {
            insert(&mut items, &mut head, i);
        }
        let mut nodes = vec![[0u32; 2]; n.saturating_sub(1)];
        let mut node = n as i64 - 1;
        loop {
            let a = head as usize;
            let b = items[a].0;
            if b == -1 {
                break;
            }
            let b = b as usize;
            node -= 1;
            nodes[node as usize] = [items[a].2, items[b].2];
            items.push((-1, items[a].1 + items[b].1, (node as u32 & 0x7F) << 1));
            head = items[b].0;
            let new = items.len() - 1;
            if head == -1 {
                head = new as i64;
            } else {
                insert(&mut items, &mut head, new);
            }
        }
        let mut lookup = [0u16; 64];
        for (v, e) in lookup.iter_mut().enumerate() {
            let mut child = 0u32;
            let mut depth = 0u16;
            let mut bit = 6;
            loop {
                depth += 1;
                bit -= 1;
                let at = ((child >> 1) & 0x7F) as usize;
                child = nodes.get(at).map_or(1, |nd| nd[(v >> bit) & 1]);
                if child & 1 != 0 || bit == 0 {
                    break;
                }
            }
            *e = ((((child >> 1) & 0x7F) as u16 | depth << 11) << 1) | (child & 1) as u16;
        }
        Huffman { nodes, lookup }
    }
}

/// The Huffman bit reader (+0x190 bits left, +0x194 word, +0x198 data):
/// big-endian words, most significant bit first.
struct HuffBits<'a> {
    buf: &'a [u8],
    pos: usize,
    left: i32,
    word: u32,
}

impl HuffBits<'_> {
    fn byte(&self, i: usize) -> u32 {
        self.buf.get(i).copied().unwrap_or(0) as u32
    }

    fn load(&mut self) {
        self.word = self.byte(self.pos) << 24 | self.byte(self.pos + 1) << 16 | self.byte(self.pos + 2) << 8 | self.byte(self.pos + 3);
        self.pos += 4;
    }

    /// 0x70D99E.
    fn bit(&mut self) -> u32 {
        if self.left != 0 {
            self.left -= 1;
            return (self.word >> self.left) & 1;
        }
        self.load();
        self.left = 31;
        self.word >> 31
    }

    /// 0x70D8DB: `n` bits.
    fn bits(&mut self, n: i32) -> u32 {
        let mask = if self.left >= 32 { u32::MAX } else { (1u32 << self.left) - 1 };
        self.word &= mask;
        let mut d = n - self.left;
        let mut hi = 0;
        if d > 0 {
            hi = self.word << d;
            self.load();
            d -= 32;
        }
        self.left = -d;
        let shifted = if self.left >= 32 { 0 } else { self.word >> self.left };
        shifted | hi
    }

    /// A token: six bits through the lookup (peeking into the next byte
    /// when fewer are left), then the tree bit by bit if no leaf yet.
    fn read(&mut self, t: &Huffman) -> u8 {
        let mask = if self.left >= 32 { u32::MAX } else { (1u32 << self.left) - 1 };
        let mut v = mask & self.word;
        v = if self.left < 6 { (self.byte(self.pos) | v << 8) >> (self.left + 2) } else { v >> (self.left - 6) };
        let e = t.lookup[(v & 63) as usize];
        self.left -= (e >> 12) as i32;
        if self.left < 0 {
            self.load();
            self.left += 32;
        }
        if e & 1 != 0 {
            return ((e >> 1) & 0x1F) as u8;
        }
        let mut child = (e & 0x3E) as u32;
        loop {
            let at = ((child >> 1) & 0x7F) as usize;
            let b = self.bit() as usize;
            child = t.nodes.get(at).map_or(1, |nd| nd[b]);
            if child & 1 != 0 {
                return ((child >> 1) & 0x7F) as u8;
            }
        }
    }

    /// A token's value: its base plus 1..5 extra bits (tokens 5..9) or 11
    /// (token 10).
    fn token_value(&mut self, token: u8) -> i32 {
        let base = TOKEN_BASE[token as usize];
        match token {
            5..=9 => base + self.bits(token as i32 - 4) as i32,
            10 => base + self.bits(11) as i32,
            _ => base,
        }
    }

    /// The run of zero-DC blocks or first-AC end-of-blocks: 2 bits + 1;
    /// 3 → 3 + 2 bits; 4 → a bit, then 7 + 2 bits or 11 + 6 bits.
    fn zero_run(&mut self) -> i32 {
        let r = self.bits(2) as i32 + 1;
        match r {
            3 => self.bits(2) as i32 + 3,
            4 => {
                if self.bit() == 0 { self.bits(2) as i32 + 7 } else { self.bits(6) as i32 + 11 }
            }
            _ => r,
        }
    }
}

/// A token category's value (the DCT_CAT tables at 0x7C5630): the
/// category from the last five probabilities of a model, then its bits.
fn category(rac: &mut Rac, p: [u8; 5]) -> i32 {
    let cat = if !rac.prob(p[0]) {
        5 + rac.prob(p[1]) as usize
    } else if !rac.prob(p[2]) {
        7 + rac.prob(p[3]) as usize
    } else {
        9 + rac.prob(p[4]) as usize
    };
    let e = &CAT_TABLE[cat * 16..cat * 16 + 16];
    let base = u16::from_le_bytes([e[0], e[1]]) as i32;
    let last = i16::from_le_bytes([e[2], e[3]]) as i32;
    let mut v = base;
    let mut bit = last;
    while bit >= 0 {
        v += (rac.prob(e[4 + bit as usize]) as i32) << bit;
        bit -= 1;
    }
    v
}

/// 0x70F4EA: a statistics change.
fn stats_delta(rac: &mut Rac) -> i32 {
    if !rac.prob(205) {
        return 0;
    }
    let s = 1 - 2 * rac.half() as i32;
    if !rac.prob(171) {
        let b = rac.prob(83) as u32;
        return s << (3 - b);
    }
    let v = if !rac.prob(199) {
        if !rac.prob(140) {
            if rac.prob(125) {
                return s * 16;
            }
            if !rac.prob(104) {
                return s * 24;
            }
            s * 5
        } else {
            s * 3
        }
    } else {
        rac.bits(7) as i32 * s
    };
    v << 2
}

/// 0x70DF7C (the simple profile path) → 0x7043D2 / 0x703F08: an 8 × 8
/// prediction at a quarter (luma) or eighth (chroma) pel vector.
fn motion(plane: &Plane, x: usize, y: usize, v: (i16, i16), luma: bool, out: &mut [i16; 64]) {
    let (mask, shift) = if luma { (3, 2) } else { (7, 3) };
    let (vx, vy) = (v.0 as i32, v.1 as i32);
    let sx = x as isize + ((vx + if vx < 0 { mask } else { 0 }) >> shift) as isize;
    let sy = y as isize + ((vy + if vy < 0 { mask } else { 0 }) >> shift) as isize;
    let mut fx = vx & mask;
    let mut fy = vy & mask;
    let dx: isize = if fx != 0 { if vx > 0 { 1 } else { -1 } } else { 0 };
    let dy: isize = if fy != 0 { if vy > 0 { 1 } else { -1 } } else { 0 };
    if dx == 0 && dy == 0 {
        // 0x703F08: a copy.
        for r in 0..8 {
            for c in 0..8 {
                out[r * 8 + c] = plane.at(sx + c as isize, sy + r as isize) as i16;
            }
        }
        return;
    }
    if luma {
        fx <<= 1;
        fy <<= 1;
    }
    // 0x7043D2: start from the upper / left of the two points.
    let ox = if dx < 0 { sx - 1 } else { sx };
    let oy = if dy < 0 { sy - 1 } else { sy };
    let tx = [BILINEAR[2 * fx as usize], BILINEAR[2 * fx as usize + 1]];
    let ty = [BILINEAR[2 * fy as usize], BILINEAR[2 * fy as usize + 1]];
    let px = |c: isize, r: isize| plane.at(ox + c, oy + r) as i32;
    match (dx != 0, dy != 0) {
        // 0x704279 across, then down.
        (true, false) => {
            for r in 0..8 {
                for c in 0..8 {
                    let (a, b) = (px(c, r), px(c + 1, r));
                    out[r as usize * 8 + c as usize] = ((b * tx[1] + 0x40 + a * tx[0]) >> 7) as i16;
                }
            }
        }
        (false, true) => {
            for r in 0..8 {
                for c in 0..8 {
                    let (a, b) = (px(c, r), px(c, r + 1));
                    out[r as usize * 8 + c as usize] = ((b * ty[1] + 0x40 + a * ty[0]) >> 7) as i16;
                }
            }
        }
        // 0x70439A: 9 rows across (0x7042D4), then down (0x704330).
        _ => {
            let mut tmp = [0i32; 72];
            for r in 0..9 {
                for c in 0..8 {
                    let (a, b) = (px(c, r), px(c + 1, r));
                    tmp[r as usize * 8 + c as usize] = (b * tx[1] + 0x40 + a * tx[0]) >> 7;
                }
            }
            for r in 0..8 {
                for c in 0..8 {
                    let (a, b) = (tmp[r * 8 + c], tmp[(r + 1) * 8 + c]);
                    out[r * 8 + c] = ((b * ty[1] + 0x40 + a * ty[0]) >> 7) as i16;
                }
            }
        }
    }
}

/// The inverse transform for an end-of-block class (0x70E29A's choice):
/// below 2 the DC alone (0x71E136), else the full transform (0x71DA07;
/// the ≤ 10 variant 0x71DE7C computes the same on the coefficients it is
/// chosen for). Clears what the game clears.
fn idct(block: &mut [i16; 64], dq: &[i16; 64], class: u8) -> [i16; 64] {
    let mut out = [0i16; 64];
    if class < 2 {
        let v = ((block[0] as i32 * dq[0] as i32 + 15) >> 5) as i16;
        out.fill(v);
        block[0] = 0;
        return out;
    }
    let mut d = [0i32; 64];
    for i in 0..64 {
        d[ZIGZAG[i] as usize] = block[i] as i32 * dq[i] as i32;
    }
    if class <= 10 {
        for i in (0..8).chain(8..12).chain(16..20).chain(24..28).chain(32..33) {
            block[i] = 0;
        }
    } else {
        block.fill(0);
    }
    const C1: i32 = 0xFB15;
    const C2: i32 = 0xEC83;
    const C3: i32 = 0xD4DB;
    const C4: i32 = 0xB505;
    const C5: i32 = 0x8E3A;
    const C6: i32 = 0x61F8;
    const C7: i32 = 0x31F1;
    let m = |a: i32, c: i32| a.wrapping_mul(c) >> 16;
    // One 1-D pass on x[0..8]: (Gd + Cd, Add + Hd, Add − Hd, Ed + Dd,
    // Ed − Dd, Fd + Bdd, Fd − Bdd, Gd − Cd) and the rounding added.
    let pass = |x: [i32; 8], round: i32| -> [i32; 8] {
        let a = m(x[1], C1) + m(x[7], C7);
        let b = m(x[1], C7) - m(x[7], C1);
        let c = m(x[3], C3) + m(x[5], C5);
        let dd = m(x[5], C3) - m(x[3], C5);
        let ad = m(a - c, C4);
        let bd = m(b - dd, C4);
        let cd = a + c;
        let ddd = b + dd;
        let e = m(x[0] + x[4], C4);
        let f = m(x[0] - x[4], C4);
        let g = m(x[2], C2) + m(x[6], C6);
        let h = m(x[2], C6) - m(x[6], C2);
        let ed = e - g + round;
        let gd = e + g + round;
        let add = f + ad + round;
        let bdd = bd - h;
        let fd = f - ad + round;
        let hd = bd + h;
        [gd + cd, add + hd, add - hd, ed + ddd, ed - ddd, fd + bdd, fd - bdd, gd - cd]
    };
    for r in 0..8 {
        let row: [i32; 8] = std::array::from_fn(|c| d[r * 8 + c]);
        if row.iter().all(|&v| v == 0) {
            continue;
        }
        let o = pass(row, 0);
        for c in 0..8 {
            d[r * 8 + c] = o[c] as i16 as i32;
        }
    }
    for c in 0..8 {
        let col: [i32; 8] = std::array::from_fn(|r| d[r * 8 + c]);
        if col.iter().all(|&v| v == 0) {
            continue;
        }
        let o = pass(col, 8);
        for r in 0..8 {
            out[r * 8 + c] = (o[r] >> 4) as i16;
        }
    }
    out
}

/// 0x7004F7's tables (rounded half away from zero by 0x7004B9) and the
/// row converter 0x6FFD4A: Y + U and V terms per channel, each sum
/// saturated to 0..255 (paddw, packuswb), alpha 255. One chroma sample
/// serves 2 × 2 pixels.
pub fn to_rgba(frame: &Frame, out: &mut Vec<u8>) {
    fn round(v: f32) -> i32 {
        if v > 0.0 {
            (v as f64 + 0.5) as i32
        } else if v < 0.0 {
            (v as f64 - 0.5) as i32
        } else {
            0
        }
    }
    let tables: Vec<[i32; 4]> = (0..256)
        .map(|i| {
            let c = (i - 128) as f32;
            [round(c * 1.772f32), round(c * -0.3441f32), round(c * -0.7141f32), round(c * 1.402f32)]
        })
        .collect();
    let y: Vec<&[u8]> = frame.plane(0).collect();
    let u: Vec<&[u8]> = frame.plane(1).collect();
    let v: Vec<&[u8]> = frame.plane(2).collect();
    let w = y.first().map_or(0, |r| r.len());
    out.clear();
    out.reserve(w * y.len() * 4);
    for (j, row) in y.iter().enumerate() {
        for (i, &l) in row.iter().enumerate() {
            let tu = tables[u[j / 2][i / 2] as usize];
            let tv = tables[v[j / 2][i / 2] as usize];
            let l = l as i32;
            let b = (l + tu[0]).clamp(0, 255) as u8;
            let g = (l + tu[1] + tv[2]).clamp(0, 255) as u8;
            let r = (l + tv[3]).clamp(0, 255) as u8;
            out.extend_from_slice(&[r, g, b, 255]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn dc_only_and_full_transform_agree_on_flat_blocks() {
        // A DC of 64 at dequantizer 16: the DC path gives (1024 + 15) >> 5.
        let dq = [16i16; 64];
        let mut b = [0i16; 64];
        b[0] = 64;
        let o = idct(&mut b, &dq, 1);
        assert!(o.iter().all(|&v| v == 32));
        assert_eq!(b[0], 0);
        // The full transform of a lone DC is flat too.
        let mut b = [0i16; 64];
        b[0] = 64;
        let o = idct(&mut b, &dq, 20);
        assert!(o.iter().all(|&v| v == o[0]));
    }

    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn coefficient_order_covers_every_position() {
        let mut d = Decoder::new();
        d.version = 7;
        d.reorder = *DEFAULT_REORDER;
        d.order();
        let mut seen = [false; 64];
        for &p in &d.idx_to_pos {
            seen[p as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
        assert_eq!(d.max_pos[0], 1);
    }
}
