/// The speech library's random pick among a candidate's records (0x738860,
/// from the play step's candidate loop 0x737B20: each candidate slot of
/// 12 bytes holds its entry id (+4), first record (+8) and record count
/// (+9); the record played is first + pick(count, entry)).
///
/// * The generator 0x738750: five 32-bit words 0x819AB4…0x819AC4 and a
///   counter 0x819AC8, an add-with-carry chain. Seeded by 0x738940
///   from the library's start-up 0x736180 at each speech-library init
///   0x486220 ([`super::Library::init`]): w = seed × 0x10001, then the words w − 0xDD2F1AA, − 0x69FBE76D,
///   + 0x3DF3B646, + 0x40DDE76D, − 0x68CD851F, the counter + 0xD1A9FBE7,
///   and 32 − (seed & 31) draws thrown away.
/// * 0x738860(n, key): i = ((draw >> 16) × n) >> 16. With a key (the
///   entry), the history 0x8B5688 (32 × {u16 index, u16 key}, ring index
///   0x8B5708) is searched from the newest back (0x738800): among its
///   entries of the same key, the first min(n / 2, 10) are looked at; i
///   not among them is taken at once, else the one last played longest
///   ago is kept and another drawn, at most 32 draws. The kept index goes
///   into the history; the last drawn one is returned (they differ only
///   when all 32 draws were recent — the native quirk is kept).
/// * Without a key (−1) 0x738860 makes one draw and returns it: no
///   history is read or written ([`Picker::rand`]). The library draws so
///   for the request gate (0x73698A), the alternatives' weighted order
///   (0x7374E3) and their chance (0x738489), all on this one generator,
///   for the speech and the NIS bank alike (`speech::Library`).
#[derive(Clone, Debug)]
pub struct Picker {
    w: [u32; 5],
    count: u32,
    history: [(u16, u16); 32],
    at: usize,
}

impl Picker {
    /// 0x738940(seed).
    pub fn new(seed: u32) -> Picker {
        let s = seed.wrapping_mul(0x10001);
        let a = s.wrapping_sub(0xDD2_F1AA);
        let b = a.wrapping_sub(0x69FB_E76D);
        let c = b.wrapping_add(0x3DF3_B646);
        let d = c.wrapping_add(0x40DD_E76D);
        let e = d.wrapping_sub(0x68CD_851F);
        let mut p =
            Picker { w: [a, b, c, d, e], count: e.wrapping_add(0xD1A9_FBE7), history: [(0xFFFF, 0xFFFF); 32], at: 0 };
        for _ in 0..32 - (s & 31) {
            p.draw();
        }
        p
    }

    /// 0x738750: the words are summed from the last up with each carry
    /// passed on, then the counter steps; when it wraps to 0 the words
    /// count up as one number.
    pub fn draw(&mut self) -> u32 {
        let [w0, w1, w2, w3, w4] = self.w;
        let s4 = w4.wrapping_add(self.count);
        let c = (s4 < self.count || s4 < w4) as u32;
        let s3 = w3.wrapping_add(c).wrapping_add(s4);
        let c = (s3 < w3) as u32;
        let s2 = s3.wrapping_add(c).wrapping_add(w2);
        let c = (s2 < w2) as u32;
        let s1 = s2.wrapping_add(c).wrapping_add(w1);
        let c = (s1 < w1) as u32;
        let s0 = w0.wrapping_add(s1.wrapping_add(c));
        self.w = [s0, s1, s2, s3, s4];
        self.count = self.count.wrapping_add(1);
        if self.count == 0 {
            for i in (0..5).rev() {
                self.w[i] = self.w[i].wrapping_add(1);
                if self.w[i] != 0 {
                    break;
                }
            }
        }
        self.w[0]
    }

    fn roll(&mut self, n: u32) -> u16 {
        (((self.draw() >> 16).wrapping_mul(n)) >> 16) as u16
    }

    /// 0x738800: how many same-key picks back `i` was played (0 the last),
    /// looking at `depth` of them and at most 33 entries.
    fn age(&self, i: u16, key: u16, depth: usize) -> Option<usize> {
        let mut seen = 0;
        let mut at = self.at;
        for _ in 0..=32 {
            if seen >= depth {
                return None;
            }
            let (idx, k) = self.history[at];
            if k == key {
                seen += 1;
                if idx == i {
                    return Some(seen - 1);
                }
            }
            at = (at + 31) % 32;
        }
        None
    }

    /// 0x738860(n, −1): one draw, ((draw >> 16) · n) >> 16 (32-bit
    /// product), no history.
    pub fn rand(&mut self, n: u32) -> u16 {
        (((self.draw() >> 16).wrapping_mul(n)) >> 16) as u16
    }

    /// 0x738860(n, key).
    pub fn pick(&mut self, n: u32, key: u16) -> u16 {
        let depth = ((n / 2) as usize).min(10);
        let mut i = self.roll(n);
        let mut kept = i;
        let mut best: i64 = -1;
        for tries in 1..=32 {
            match self.age(i, key, depth) {
                None => {
                    kept = i;
                    break;
                }
                Some(a) if a as i64 > best => {
                    best = a as i64;
                    kept = i;
                }
                Some(_) => {}
            }
            if tries == 32 {
                break;
            }
            i = self.roll(n);
        }
        self.at = (self.at + 1) % 32;
        self.history[self.at] = (kept, key);
        i
    }
}
