/// The library tick, ms (0x8B4054, see the module notes).
pub const TICK_MS: f32 = 20.0;

/// One decoded instruction of a module's code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// `push esi; call node`: eax = node(block at esi).
    Call(u8),
    /// `mov [esi+d],eax`.
    Store(i32),
    /// `mov byte [esi+d],imm8`.
    StoreByte(i32, u8),
    /// `mov eax,[esi+d]` / `mov ecx,[esi+d]`.
    Load(i32),
    LoadEcx(i32),
    Add(i32),
    Sub(i32),
    Imul(i32),
    /// `mov eax,ecx`.
    MovEcx,
    /// `cmp eax,ecx`.
    Cmp,
    /// `jl` / `jg` to an op index.
    Jl(usize),
    Jg(usize),
    /// `add esi,imm`.
    AddEsi(i32),
}

/// [esi+d] operand of a ModRM byte at `c[i]`: (reg, disp, length).
pub(super) fn modrm(c: &[u8], i: usize) -> Option<(u8, i32, usize)> {
    let m = *c.get(i)?;
    if m & 7 != 6 {
        return None;
    }
    let reg = (m >> 3) & 7;
    match m >> 6 {
        0 => Some((reg, 0, 1)),
        1 => Some((reg, *c.get(i + 1)? as i8 as i32, 2)),
        2 => Some((reg, i32::from_le_bytes(c.get(i + 1..i + 5)?.try_into().ok()?), 5)),
        _ => None,
    }
}

/// Decode a module's code (from record +0x28). `None` for anything
/// outside the forms listed in the module notes.
pub fn decode(c: &[u8]) -> Option<Vec<Op>> {
    if c.get(..5)? != [0x56, 0x8B, 0x74, 0x24, 0x08] {
        return None;
    }
    let mut ops: Vec<(usize, Op)> = Vec::new();
    let mut jumps: Vec<(usize, usize)> = Vec::new();
    let mut i = 5;
    loop {
        let at = i;
        let op = match *c.get(i)? {
            0x56 => {
                i += 1;
                continue;
            }
            0x5E if c.get(i + 1) == Some(&0xC3) => break,
            0xE8 => {
                let id = i32::from_le_bytes(c.get(i + 1..i + 5)?.try_into().ok()?);
                i += 5;
                Op::Call(u8::try_from(id).ok().filter(|&n| n < 40)?)
            }
            0x89 => {
                let (r, d, n) = modrm(c, i + 1)?;
                i += 1 + n;
                (r == 0).then_some(Op::Store(d))?
            }
            0x8B if c.get(i + 1) == Some(&0xC1) => {
                i += 2;
                Op::MovEcx
            }
            0x8B => {
                let (r, d, n) = modrm(c, i + 1)?;
                i += 1 + n;
                match r {
                    0 => Op::Load(d),
                    1 => Op::LoadEcx(d),
                    _ => return None,
                }
            }
            o @ (0x03 | 0x2B) => {
                let (r, d, n) = modrm(c, i + 1)?;
                i += 1 + n;
                if r != 0 {
                    return None;
                }
                if o == 0x03 { Op::Add(d) } else { Op::Sub(d) }
            }
            0x0F if c.get(i + 1) == Some(&0xAF) => {
                let (r, d, n) = modrm(c, i + 2)?;
                i += 2 + n;
                (r == 0).then_some(Op::Imul(d))?
            }
            0x3B if c.get(i + 1) == Some(&0xC1) => {
                i += 2;
                Op::Cmp
            }
            o @ (0x7C | 0x7F) => {
                let to = (i as isize + 2 + *c.get(i + 1)? as i8 as isize) as usize;
                jumps.push((ops.len(), to));
                i += 2;
                if o == 0x7C { Op::Jl(0) } else { Op::Jg(0) }
            }
            0x83 if c.get(i + 1) == Some(&0xC6) => {
                i += 3;
                Op::AddEsi(*c.get(i - 1)? as i8 as i32)
            }
            0x81 if c.get(i + 1) == Some(&0xC6) => {
                i += 6;
                Op::AddEsi(i32::from_le_bytes(c.get(i - 4..i)?.try_into().ok()?))
            }
            0x83 if c.get(i + 1) == Some(&0xC4) => {
                i += 3;
                continue;
            }
            0xC6 => {
                let (r, d, n) = modrm(c, i + 1)?;
                if r != 0 {
                    return None;
                }
                i += 1 + n + 1;
                Op::StoreByte(d, *c.get(i - 1)?)
            }
            _ => return None,
        };
        ops.push((at, op));
    }
    let end = ops.len();
    for (k, to) in jumps {
        let t = ops.iter().position(|o| o.0 == to).unwrap_or(end);
        ops[k].1 = match ops[k].1 {
            Op::Jl(_) => Op::Jl(t),
            _ => Op::Jg(t),
        };
    }
    Some(ops.into_iter().map(|o| o.1).collect())
}
