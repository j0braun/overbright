//! The inbox: how another program changes a mod that Studio has open. It writes a request into the mod's
//! `.editor/inbox/` (atomically: Studio never reads half of one); Studio takes it at its next tick, makes it one
//! step in the history under the program's name (after the journal has it on the disk) and only then writes the
//! reply. So an `ok` is final: no conflict can be reported after it. A request answered before a crash is
//! answered again, from the journal, never made twice. There is no background process and no network endpoint:
//! the requests are files, and nothing is read while no Studio has the mod open.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The inbox folder, in the mod's editor folder.
pub const INBOX: &str = "inbox";
const REQUEST: &str = ".request.json";
const REPLY: &str = ".reply.json";

/// One change asked for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Unique: [`new_id`].
    pub id: String,
    /// Who asks ("overbright-studio (command line)", "agent: …"); the step is named after it.
    pub author: String,
    /// The step's name.
    pub label: String,
    /// The history step the program read the mod at ([`super::History::seq`]); None: the current one, whatever
    /// it is (the documents are taken as they are, each item checked against nothing).
    pub base: Option<u64>,
    /// The documents as the program wants them (None: remove the document).
    pub docs: BTreeMap<String, Option<String>>,
}

/// What became of a request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    /// Made as history step `seq` (or there was nothing to change: `seq` is then the current step).
    Ok { seq: u64 },
    /// Not made: these items (`doc#key`) changed since `base` in another way.
    Conflict { items: Vec<String> },
    /// Not made, for this reason (a document that does not read, a base no longer kept).
    Refused { reason: String },
}

/// A request id no other program and no earlier call uses.
pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    format!("{:013}-{}-{}", super::now_ms(), std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Put `r` into the inbox of the mod whose editor folder is `dir`.
pub fn submit(dir: &Path, r: &Request) -> io::Result<()> {
    if !valid_id(&r.id) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad request id"));
    }
    let bytes = serde_json::to_vec_pretty(r).map_err(io::Error::other)?;
    ob_core::platform::fs::write_atomic(&dir.join(INBOX).join(format!("{}{REQUEST}", r.id)), &bytes, true)
}

/// The reply to request `id`, once there is one (the caller then may [`forget`] it).
pub fn reply(dir: &Path, id: &str) -> Option<Reply> {
    let bytes = std::fs::read(reply_path(dir, id)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Remove a read reply.
pub fn forget(dir: &Path, id: &str) {
    let _ = std::fs::remove_file(reply_path(dir, id));
}

pub(super) fn reply_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(INBOX).join(format!("{id}{REPLY}"))
}

pub(super) fn request_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(INBOX).join(format!("{id}{REQUEST}"))
}

/// The waiting requests, oldest first (a request file that does not read is answered `Refused` by its id).
pub(super) fn pending(dir: &Path) -> Vec<Result<Request, String>> {
    let Ok(rd) = std::fs::read_dir(dir.join(INBOX)) else { return Vec::new() };
    let mut names: Vec<String> =
        rd.filter_map(|e| e.ok()?.file_name().into_string().ok()).filter(|n| n.ends_with(REQUEST)).collect();
    names.sort();
    names
        .into_iter()
        .map(|n| {
            let id = n.trim_end_matches(REQUEST).to_string();
            std::fs::read(dir.join(INBOX).join(&n))
                .ok()
                .and_then(|b| serde_json::from_slice::<Request>(&b).ok())
                .filter(|r| r.id == id)
                .ok_or(id)
        })
        .collect()
}
