fn u16le(d: &[u8], o: usize) -> usize {
    d.get(o..o + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize)
}

fn u32le(d: &[u8], o: usize) -> usize {
    d.get(o..o + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

#[derive(Clone, Debug, Default)]
pub struct Mpf {
    /// Per node: the table entry of the stream it plays (id − 1, type 4
    /// nodes), the next node.
    pub nodes: Vec<(Option<usize>, usize)>,
    /// Per event: the node it starts at (record +0x1C).
    pub events: Vec<usize>,
    /// Per stream: (offset in the `.mus`, samples).
    pub samples: Vec<(u64, u32)>,
}

impl Mpf {
    pub fn parse(d: &[u8]) -> Option<Mpf> {
        if d.get(..4)? != b"xDFP" {
            return None;
        }
        let count = u16le(d, 0x12);
        let node_at = |i: usize| u16le(d, 0x20 + 2 * i) * 4;
        let nodes = (0..count)
            .map(|i| {
                let o = node_at(i);
                let id = u16le(d, o);
                let kind = u32le(d, o + 4) >> 24;
                ((kind == 4 && id != 0 && id != 0xFFFF).then(|| id - 1), d.get(o + 0x12).copied().unwrap_or(0) as usize)
            })
            .collect();
        // 0x73A8C6..: the event table follows the last node, the bank
        // table follows the last event.
        let last = node_at(count.checked_sub(1)?);
        let events_at = (last + 0x13 + ((u32le(d, last + 4) >> 12) & 0x1F) * 4) & !3;
        let n_events = *d.get(0xF)? as usize;
        let event_at = |i: usize| u16le(d, events_at + 2 * i) * 4;
        let events = (0..n_events).map(|i| u16le(d, event_at(i) + 0x1C)).collect();
        let last_ev = event_at(n_events.checked_sub(1)?);
        let banks_ptr = (last_ev + 0x13 + (*d.get(last_ev + 0xE)? as usize & 0x3F) * 16) & !3;
        let bank0 = u32le(d, banks_ptr) * 4 + 4 * *d.get(0x10)? as usize;
        let (start, end) = (u32le(d, bank0) * 4, u32le(d, bank0 + 4) * 4);
        let mut samples = Vec::new();
        let mut p = start;
        while p + 8 <= end.min(d.len()) {
            samples.push((u32le(d, p) as u64 * 0x80, (u32le(d, p + 4) * 32) as u32));
            p += 8;
        }
        Some(Mpf { nodes, events, samples })
    }

    /// The stream of song `i`: event i's start node, then on along the
    /// nodes' next links to the first that plays a stream.
    pub fn song_sample(&self, i: usize) -> Option<usize> {
        self.event_sample(i)
    }

    /// The stream event `e` plays (its start node, then the next links).
    /// The score's events (27, 29, … 77) start at a type-1 node whose next
    /// is a type-4 node: event 27 → node 108 → node 110, stream id 1
    /// (entry 0). Their streams (entries 0, 11–16, 23–28, 40–42) are none
    /// of the songs'.
    pub fn event_sample(&self, e: usize) -> Option<usize> {
        let mut n = *self.events.get(e)?;
        for _ in 0..4 {
            let (sample, next) = *self.nodes.get(n)?;
            if sample.is_some() {
                return sample;
            }
            n = next;
        }
        None
    }
}
