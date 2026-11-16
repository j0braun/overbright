use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unlock {
    pub part: u8,
    pub level: u8,
    pub param: u8,
    pub kind: u8,
    pub range: (f32, f32),
    pub display: (f32, f32),
}

/// Parse chunk 0x34A1A.
pub fn parse(body: &[u8]) -> Vec<Unlock> {
    body.chunks_exact(24)
        .map(|r| Unlock {
            part: read_u32(r, 0) as u8,
            level: r[4],
            param: r[5],
            kind: r[6],
            range: (read_f32(r, 8), read_f32(r, 12)),
            display: (read_f32(r, 16), read_f32(r, 20)),
        })
        .collect()
}
