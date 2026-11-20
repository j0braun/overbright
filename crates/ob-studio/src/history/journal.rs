//! The journal: every step of a mod's history, appended to one file and flushed to the disk before the step
//! counts. A record is `OBJ1`, its length (u32 LE), the XXH3-64 of its body (u64 LE) and the body (JSON). A
//! record cut short by a crash, or damaged, ends the journal there: what comes before it is whole, and the next
//! writer cuts the file back to that point before it appends.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::xxh3_64;

use super::doc::State;

/// A record's first four bytes.
pub const MAGIC: &[u8; 4] = b"OBJ1";
/// Magic, length and hash.
pub const HEAD: usize = 16;
/// The largest body read back (a mod is at most a few MiB; a damaged length is not followed further).
pub const MAX_BODY: usize = 64 << 20;

/// One journal record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rec {
    /// Where the history starts: the documents as they were (the first record; a compacted journal's first
    /// record holds the oldest kept state). `done` lists the inbox requests already answered.
    Base {
        seq: u64,
        at: u64,
        docs: State,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        done: Vec<String>,
    },
    /// One step: the documents it changed, as they are after it (None: the document was removed).
    Step {
        seq: u64,
        at: u64,
        author: String,
        label: String,
        docs: BTreeMap<String, Option<String>>,
        /// The inbox request it answers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request: Option<String>,
    },
    /// The last step taken back.
    Undo { at: u64, author: String },
    /// The last step taken back made again.
    Redo { at: u64, author: String },
    /// The mod's files were written as of the current state (their hashes).
    Saved { at: u64, docs: BTreeMap<String, u64> },
    /// The current state kept under a name.
    Version { at: u64, name: String, docs: State },
}

/// A record as bytes.
pub fn encode(r: &Rec) -> Vec<u8> {
    let body = serde_json::to_vec(r).unwrap_or_default();
    let mut out = Vec::with_capacity(HEAD + body.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&xxh3_64(&body).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

/// The whole records at the start of `bytes`, and how many bytes they take (the rest is a cut or damaged
/// record, or nothing).
pub fn decode(bytes: &[u8]) -> (Vec<Rec>, usize) {
    let mut recs = Vec::new();
    let mut at = 0;
    while bytes.len() - at >= HEAD {
        let h = &bytes[at..at + HEAD];
        if &h[..4] != MAGIC {
            break;
        }
        let len = u32::from_le_bytes([h[4], h[5], h[6], h[7]]) as usize;
        let hash = u64::from_le_bytes(h[8..16].try_into().unwrap_or_default());
        if len > MAX_BODY || bytes.len() - at - HEAD < len {
            break;
        }
        let body = &bytes[at + HEAD..at + HEAD + len];
        if xxh3_64(body) != hash {
            break;
        }
        let Ok(r) = serde_json::from_slice::<Rec>(body) else { break };
        recs.push(r);
        at += HEAD + len;
    }
    (recs, at)
}

/// A document's hash as `Saved` records it.
pub fn hash(text: &str) -> u64 {
    xxh3_64(text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(n: u64) -> Rec {
        Rec::Step {
            seq: n,
            at: 7,
            author: "studio".into(),
            label: format!("Step {n}"),
            docs: [("map.obmap".to_string(), Some(format!("text {n}")))].into(),
            request: None,
        }
    }

    #[test]
    fn records_read_back_and_a_cut_tail_ends_the_journal() {
        let mut bytes = Vec::new();
        for n in 0..3 {
            bytes.extend(encode(&step(n)));
        }
        let whole = bytes.len();
        let (recs, used) = decode(&bytes);
        assert_eq!((recs.len(), used), (3, whole));
        // Every cut of a fourth record leaves the three.
        let more = encode(&step(3));
        for cut in 0..more.len() {
            let mut b = bytes.clone();
            b.extend_from_slice(&more[..cut]);
            assert_eq!(decode(&b), (recs.clone(), whole), "cut at {cut}");
        }
        // A flipped byte in a body ends the journal before that record.
        let mut b = bytes.clone();
        b[whole - 3] ^= 0x20;
        assert_eq!(decode(&b).0.len(), 2);
    }
}
