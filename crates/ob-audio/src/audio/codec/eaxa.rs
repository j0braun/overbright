//! EA-XA (the audio library's ADPCM, 0x734AFF / 0x734A81): the coefficient
//! tables and scaled nibbles, and the sample decoder the banks and the
//! streams use. The granular engine recordings ([`super::super::gin`]) share
//! the tables.

/// The EA-XA tables (0x819668): coefficients for the high nibble, the
/// second coefficients, then per shift the scaled nibbles.
pub(crate) fn coef(i: usize) -> (f32, f32) {
    const C1: [f32; 4] = [0.0, 0.9375, 1.796875, 1.53125];
    const C2: [f32; 4] = [0.0, 0.0, -0.8125, -0.859375];
    // The tables are read past their four entries for indices ≥ 4 (the
    // next table follows in memory); valid data only uses 0–3.
    (C1.get(i).copied().unwrap_or(0.0), C2.get(i).copied().unwrap_or(0.0))
}

pub(crate) fn scaled(shift: usize, nibble: u8) -> f32 {
    let n = ((nibble as i8) << 4 >> 4) as f32;
    if shift <= 12 { n * (1u32 << (12 - shift)) as f32 } else { 0.0 }
}

/// EA-XA decoding (0x734AFF / 0x734A81): 15-byte blocks of 28 samples,
/// the first byte's high nibble picking the coefficients and the low the
/// shift, each nibble (high first) s = c1·h1 + c2·h2 + table; a block
/// starting 0xEE holds the history (two big-endian i16) and 28 raw
/// big-endian samples. The history starts at 0.
pub fn decode_eaxa(d: &[u8], samples: usize) -> Vec<f32> {
    // (a block of 15 bytes gives 28 samples: never reserve more than the data can fill)
    let mut out = Vec::with_capacity(samples.min(d.len() / 15 * 28 + 61) + 28);
    let (mut h1, mut h2) = (0.0f32, 0.0f32);
    let mut o = 0;
    let be16 = |o: usize| i16::from_be_bytes([d[o], d[o + 1]]) as f32;
    while out.len() < samples && o < d.len() {
        if d[o] == 0xEE {
            if o + 61 > d.len() {
                break;
            }
            h1 = be16(o + 1);
            h2 = be16(o + 3);
            for k in 0..28 {
                out.push(be16(o + 5 + k * 2));
            }
            o += 61;
        } else {
            if o + 15 > d.len() {
                break;
            }
            let (c1, c2) = coef((d[o] >> 4) as usize);
            let shift = (d[o] & 0xF) as usize;
            for k in 0..14 {
                let b = d[o + 1 + k];
                for nib in [b >> 4, b & 0xF] {
                    let s = c1 * h1 + c2 * h2 + scaled(shift, nib);
                    h2 = h1;
                    h1 = s;
                    out.push(s);
                }
            }
            o += 15;
        }
    }
    out.truncate(samples);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_and_adpcm_blocks_decode() {
        let mut d = vec![0xEE, 0, 5, 0, 3];
        for k in 0..28i16 {
            d.extend_from_slice(&(k * 10).to_be_bytes());
        }
        // A block with coefficients 1 (0.9375, 0), shift 12, nibbles 1.
        d.push(0x1C);
        d.extend_from_slice(&[0x11; 14]);
        let s = decode_eaxa(&d, 56);
        assert_eq!(s[27], 270.0);
        // h1 = 5 from the raw block's header: 0.9375·5 + 1.
        assert_eq!(s[28], 0.9375 * 5.0 + 1.0);
        assert_eq!(s.len(), 56);
    }
}
