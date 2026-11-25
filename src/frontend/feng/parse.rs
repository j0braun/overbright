//! The FEng package reader (tags, scripts, objects, `MsgR` responses).

use anyhow::{Context, Result, bail};

use super::model::*;

// A field past the end of its record reads as 0 (fuzz: these records come
// from the file; the checked readers of `formats` answer with an error,
// but a package keeps loading with what it has).
fn read_u16(b: &[u8], at: usize) -> u16 {
    b.get(at..at + 2).map_or(0, |w| u16::from_le_bytes([w[0], w[1]]))
}

fn read_u32(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map_or(0, |w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}


/// The message nodes of one `MsgR` body (two-character tags, u16 sizes).
pub fn parse_responses(b: &[u8]) -> Vec<MessageNode> {
    let mut nodes: Vec<MessageNode> = Vec::new();
    for (tag, v) in tags2(b) {
        let w = if v.len() >= 4 { read_u32(v, 0) } else { 0 };
        match &tag {
            b"Mi" => nodes.push(MessageNode { message: w, entries: Vec::new() }),
            b"Ri" => {
                if let Some(n) = nodes.last_mut() {
                    n.entries.push([w, 0, 0]);
                }
            }
            b"Ru" | b"Rt" => {
                if let Some(e) = nodes.last_mut().and_then(|n| n.entries.last_mut()) {
                    e[if &tag == b"Ru" { 1 } else { 2 }] = w;
                }
            }
            _ => {}
        }
    }
    nodes
}

fn tags4(b: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut at = 0;
    std::iter::from_fn(move || {
        if at + 8 > b.len() {
            return None;
        }
        let tag = [b[at], b[at + 1], b[at + 2], b[at + 3]];
        let size = read_u32(b, at + 4) as usize;
        let body = &b[at + 8..(at + 8 + size).min(b.len())];
        at += 8 + size;
        Some((tag, body))
    })
}

fn tags2(b: &[u8]) -> impl Iterator<Item = ([u8; 2], &[u8])> {
    let mut at = 0;
    std::iter::from_fn(move || {
        if at + 4 > b.len() {
            return None;
        }
        let tag = [b[at], b[at + 1]];
        let size = read_u16(b, at + 2) as usize;
        let body = &b[at + 4..(at + 4 + size).min(b.len())];
        at += 4 + size;
        Some((tag, body))
    })
}

fn words(b: &[u8]) -> Vec<u32> {
    (0..b.len() / 4).map(|i| read_u32(b, i * 4)).collect()
}

fn parse_script(b: &[u8]) -> Script {
    let mut script = Script { name: 0, length: 0, flags: 0, chain: None, tracks: Vec::new(), events: Vec::new() };
    let mut pending: Option<Track> = None;
    for (tag, body) in tags2(b) {
        match &tag {
            b"Sh" => {
                script.name = read_u32(body, 0);
                script.length = read_u32(body, 4);
                script.flags = read_u32(body, 8);
            }
            b"Sc" => script.chain = Some(read_u32(body, 0)),
            b"FI" => {
                pending = Some(Track {
                    param: body.first().copied().unwrap_or(0),
                    size: body.get(1).copied().unwrap_or(0),
                    interpolation: body.get(2).copied().unwrap_or(0),
                    flags: body.get(3).copied().unwrap_or(0),
                    length: read_u32(body, 4) & 0xFF_FFFF,
                    offset: 0,
                    base: Vec::new(),
                    keys: Vec::new(),
                });
            }
            b"To" => {
                if let Some(t) = pending.as_mut() {
                    t.offset = read_u32(body, 0) as usize;
                }
            }
            b"Kd" => {
                if let Some(mut t) = pending.take() {
                    let n = (t.size as usize / 4).max(1);
                    let stride = 4 + n * 4;
                    let w = words(body);
                    // The first key is the base (time −1); the rest are
                    // deltas from it.
                    for (i, key) in w.chunks_exact(n + 1).enumerate() {
                        let _ = stride;
                        if i == 0 {
                            t.base = key[1..].to_vec();
                        } else {
                            t.keys.push((key[0], key[1..].to_vec()));
                        }
                    }
                    script.tracks.push(t);
                }
            }
            b"EV" => {
                for e in words(body).chunks_exact(3) {
                    script.events.push(Event { message: e[0], target: e[1], time: e[2] });
                }
            }
            _ => {}
        }
    }
    script
}

fn parse_object(b: &[u8]) -> Result<Object> {
    let mut o = Object {
        kind: Kind::Other(0),
        hash: 0,
        guid: 0,
        flags: 0,
        resource: u32::MAX,
        parent: None,
        attributes: Attributes { words: [0; 37], len: 0 },
        image_flags: 0,
        text: String::new(),
        label: None,
        justify: 0,
        leading: 0,
        max_width: 0,
        scripts: Vec::new(),
        responses: Vec::new(),
        messages: Vec::new(),
    };
    for (tag, body) in tags4(b) {
        match &tag {
            b"ObjD" => {
                for (t, v) in tags2(body) {
                    match &t {
                        b"Ot" => o.kind = Kind::from(read_u32(v, 0)),
                        b"Oh" => o.hash = read_u32(v, 0),
                        b"OP" => {
                            o.guid = read_u32(v, 0);
                            o.flags = read_u32(v, 8);
                            o.resource = read_u32(v, 12);
                        }
                        b"PA" => o.parent = Some(read_u32(v, 0)),
                        b"If" => o.image_flags = read_u32(v, 0),
                        b"SA" => {
                            let w = words(v);
                            o.attributes.len = w.len().min(37);
                            o.attributes.words[..o.attributes.len].copy_from_slice(&w[..o.attributes.len]);
                        }
                        b"St" => {
                            let units: Vec<u16> =
                                (0..v.len() / 2).map(|i| read_u16(v, i * 2)).take_while(|&c| c != 0).collect();
                            o.text = String::from_utf16_lossy(&units);
                        }
                        b"SH" => o.label = Some(read_u32(v, 0)),
                        b"Sj" => o.justify = read_u32(v, 0),
                        b"Sl" => o.leading = read_u32(v, 0) as i32,
                        b"Sw" => o.max_width = read_u32(v, 0),
                        _ => {}
                    }
                }
            }
            b"Scrp" => o.scripts.push(parse_script(body)),
            b"MsgR" => {
                o.messages.extend(parse_responses(body));
                o.responses.push(Response { raw: body.to_vec() });
            }
            _ => {}
        }
    }
    Ok(o)
}

pub fn parse(b: &[u8]) -> Result<Package> {
    let (tag, body) = tags4(b).next().context("empty package")?;
    if &tag[..3] != b"FEn" {
        bail!("not an FEng package");
    }
    let mut package = Package {
        name: String::new(),
        path: String::new(),
        resources: Vec::new(),
        objects: Vec::new(),
        buttons: Vec::new(),
    };
    let mut names: &[u8] = &[];
    for (tag, part) in tags4(body) {
        match &tag {
            b"PkHd" => {
                let strings: Vec<&[u8]> =
                    part[0x18.min(part.len())..].split(|&c| c == 0).filter(|s| !s.is_empty()).collect();
                package.name = strings.first().map(|s| String::from_utf8_lossy(s).into_owned()).unwrap_or_default();
                package.path = strings.get(1).map(|s| String::from_utf8_lossy(s).into_owned()).unwrap_or_default();
            }
            [b'R', b'e', b's', _] => {
                for (t, v) in tags4(part) {
                    match &t {
                        b"RsNm" => names = v,
                        b"RsRq" => {
                            let count = read_u32(v, 0) as usize;
                            for i in 0..count {
                                let e = 4 + i * 24;
                                if e + 24 > v.len() {
                                    break;
                                }
                                let at = read_u32(v, e + 4) as usize;
                                let name =
                                    names.get(at..).map(|s| s.split(|&c| c == 0).next().unwrap_or(&[])).unwrap_or(&[]);
                                package.resources.push(Resource {
                                    name: String::from_utf8_lossy(name).into_owned(),
                                    hash: read_u32(v, e),
                                    kind: match read_u32(v, e + 8) {
                                        0 => ResourceKind::Untextured,
                                        1 => ResourceKind::Texture,
                                        2 => ResourceKind::Font,
                                        k => ResourceKind::Other(k),
                                    },
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            [b'O', b'b', b'j', _] => {
                for (t, v) in tags4(part) {
                    match &t {
                        b"Butn" => package.buttons = words(v),
                        [b'F', b'O', b'b', _] => package.objects.push(parse_object(v)?),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(package)
}
