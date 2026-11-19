//! The decoders' standard tables, taken from open decoders instead of the
//! original executable (docs/exe-tables.md, "Decoder tables"): the
//! MicroTalk tables from vgmstream ([`vgmstream_utk`], ISC), the MPEG-1
//! layer III tables of ISO/IEC 11172-3 and the VP6 tables from FFmpeg
//! ([`ffmpeg_mpegaudio`], [`ffmpeg_vp6`], LGPL-2.1-or-later). Each vendored
//! file keeps its project's notice; this file puts their values into the
//! layouts the clone's decoders read (the original's), and a data test
//! compares every table with the reference executable.

pub mod ffmpeg_mpegaudio;
pub mod ffmpeg_vp6;
pub mod vgmstream_utk;

use super::Vp6Tables;

/// The layer III Huffman trees by table number 0..31 (`None` for the
/// unused 0, 4 and 14; 16..23 share one tree, as do 24..31). A tree is
/// laid out from the code table depth first with the 0 branch first: a
/// node is −n, where n is the length of its 0 branch (the 1 branch starts
/// n entries after the node), a leaf is its symbol x << 4 | y. Each tree
/// is followed by one 0, as the original stores them.
pub fn mp3_huffman_trees() -> [Option<Box<[i16]>>; 32] {
    use ffmpeg_mpegaudio::*;
    let mut trees: Vec<Vec<i16>> = Vec::new();
    let mut at = 0usize;
    for &size in &HUFF_SIZES_MINUS_ONE {
        let n = size as usize + 1;
        trees.push(tree(&HUFFLENS[at..at + n], &HUFFSYMBOLS[at..at + n]));
        at += n;
    }
    std::array::from_fn(|i| {
        let table = HUFF_DATA[2 * i] as usize;
        (table != 0).then(|| {
            let mut t = trees[table - 1].clone();
            t.push(0);
            t.into_boxed_slice()
        })
    })
}

/// One tree from code lengths given in ascending code order (canonical:
/// each code is the previous one plus one at its own length).
fn tree(lens: &[u8], symbols: &[u8]) -> Vec<i16> {
    let mut code = 0u64;
    let codes: Vec<(u32, u8, i16)> = lens
        .iter()
        .zip(symbols)
        .map(|(&l, &s)| {
            let c = (code >> (32 - l)) as u32;
            code += 1u64 << (32 - l);
            (c, l, s as i16)
        })
        .collect();
    let mut out = Vec::new();
    emit(&codes, 0, &mut out);
    out
}

fn emit(codes: &[(u32, u8, i16)], depth: u8, out: &mut Vec<i16>) {
    if let [(_, len, symbol)] = codes
        && *len == depth
    {
        out.push(*symbol);
        return;
    }
    let bit = |&&(c, l, _): &&(u32, u8, i16)| (c >> (l - depth - 1)) & 1;
    let zero: Vec<_> = codes.iter().filter(|c| bit(c) == 0).copied().collect();
    let one: Vec<_> = codes.iter().filter(|c| bit(c) == 1).copied().collect();
    let node = out.len();
    out.push(0);
    emit(&zero, depth + 1, out);
    out[node] = -((out.len() - node - 1) as i16);
    emit(&one, depth + 1, out);
}

/// The big-value tables' linbits for table numbers 16..31.
pub fn mp3_linbits() -> [u8; 16] {
    std::array::from_fn(|i| ffmpeg_mpegaudio::HUFF_DATA[2 * (16 + i) + 1])
}

/// The synthesis window as the original's filter reads it: 17 rows of
/// 16, row r column c = −W[32c + r] / 2, where W is the window in 1/65536
/// built from its first half D[0..=256] as FFmpeg builds it
/// (W[i] = D[i]; W[512 − i] = D[i], negated unless i is a multiple of 64).
pub fn mp3_window_rows() -> [[f32; 16]; 17] {
    let d = &ffmpeg_mpegaudio::ENWINDOW;
    let w = |i: usize| -> i32 {
        if i <= 256 {
            d[i]
        } else if (512 - i) % 64 == 0 {
            d[512 - i]
        } else {
            -d[512 - i]
        }
    };
    std::array::from_fn(|r| std::array::from_fn(|c| (-w(32 * c + r)) as f32 / 2.0))
}

/// The 8×8 zig-zag scan: the position of each coefficient in scan order.
fn zigzag() -> [u8; 64] {
    let mut out = [0u8; 64];
    let mut k = 0;
    for s in 0..15usize {
        let range: Vec<usize> = (s.saturating_sub(7)..=s.min(7)).collect();
        // Odd diagonals run down the rows, even ones up.
        let rows: Vec<usize> = if s % 2 == 1 { range } else { range.into_iter().rev().collect() };
        for row in rows {
            out[k] = (8 * row + (s - row)) as u8;
            k += 1;
        }
    }
    out
}

/// The VP6 decoder's tables in the original's layout.
pub fn vp6() -> Vp6Tables {
    use ffmpeg_vp6::*;
    let groups: [i8; 64] = std::array::from_fn(|i| if i == 0 { -1 } else { COEFF_GROUPS[i] as i8 });
    // The category table: 11 rows of 16 bytes {u16 base, u16 extra bits,
    // 12 probabilities}; the zero token's row has bits 0xFFFF, the single
    // values none, the six categories their parse rows.
    let mut cat_table = [0u8; 176];
    for (r, row) in cat_table.chunks_exact_mut(16).enumerate() {
        row[0] = COEFF_BIAS[r];
        if r == 0 {
            row[2..4].copy_from_slice(&[0xFF, 0xFF]);
        } else if r >= 5 {
            row[2] = COEFF_BIT_LENGTH[r - 5];
            row[4..15].copy_from_slice(&COEFF_PARSE_TABLE[11 * (r - 5)..11 * (r - 4)]);
        }
    }
    // Macroblock type statistics: per context the second counts of the ten
    // types, then the first; the presets grouped by context, then preset.
    let mut mb_stats_default = [0u8; 60];
    for c in 0..3 {
        for j in 0..10 {
            mb_stats_default[20 * c + j] = DEF_MB_TYPES_STATS[20 * c + 2 * j + 1];
            mb_stats_default[20 * c + 10 + j] = DEF_MB_TYPES_STATS[20 * c + 2 * j];
        }
    }
    let mut mb_stats_presets = [0u8; 960];
    for c in 0..3 {
        for p in 0..16 {
            let from = 20 * (3 * p + c);
            mb_stats_presets[20 * (16 * c + p)..][..20].copy_from_slice(&PRE_DEF_MB_TYPE_STATS[from..from + 20]);
        }
    }
    let mut vector_update = [0u8; 34];
    for c in 0..2 {
        let v = &mut vector_update[17 * c..17 * (c + 1)];
        v[..2].copy_from_slice(&SIG_DCT_PCT[2 * c..2 * c + 2]);
        v[2..9].copy_from_slice(&PDV_PCT[7 * c..7 * c + 7]);
        v[9..].copy_from_slice(&FDV_PCT[8 * c..8 * c + 8]);
    }
    // [3][5][2] → by model, then plane.
    let dccv_lc: [i32; 30] = std::array::from_fn(|i| {
        let (j, rest) = (i / 6, i % 6);
        DCCV_LC[10 * (rest / 2) + 2 * j + rest % 2]
    });
    let mut token_base = [0i32; 12];
    for (t, &b) in token_base.iter_mut().zip(&COEFF_BIAS) {
        *t = b as i32;
    }
    Vp6Tables {
        ac_dequant: AC_DEQUANT.map(|v| v as i32),
        dc_dequant: DC_DEQUANT.map(|v| v as i16),
        zigzag: zigzag(),
        coeff_group: groups,
        default_reorder: DEF_COEFF_REORDER,
        ref_of_type: REFERENCE_FRAME,
        dccv_update: DCCV_PCT,
        reorder_update: COEFF_REORDER_PCT,
        runv_update: RUNV_PCT,
        runv_default: DEF_RUNV_COEFF_MODEL,
        ract_update: RACT_PCT,
        dccv_lc,
        vector_update,
        mb_stats_presets,
        mb_stats_default,
        cat_table,
        // (y, x) of each neighbour.
        candidates: std::array::from_fn(|i| CANDIDATE_PREDICTOR_POS[i ^ 1] as i32),
        // The bilinear taps per eighth: (128 − 16k, 16k).
        bilinear: std::array::from_fn(|i| if i % 2 == 0 { 128 - 16 * (i as i32 / 2) } else { 16 * (i as i32 / 2) }),
        token_base,
        // The Huffman profile's bands stop at 3.
        coeff_group_huffman: groups.map(|g| g.min(3)),
    }
}
