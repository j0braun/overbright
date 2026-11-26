//! FEng package model: objects, scripts, tracks, message nodes, resources.

use glam::{Quat, Vec3};

/// What an object draws (`Ot`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Image,
    String,
    Model,
    List,
    Group,
    CodeList,
    Movie,
    Effect,
    ColoredImage,
    AnimImage,
    SimpleImage,
    MultiImage,
    Other(u32),
}

impl Kind {
    pub fn from(v: u32) -> Kind {
        match v {
            1 => Kind::Image,
            2 => Kind::String,
            3 => Kind::Model,
            4 => Kind::List,
            5 => Kind::Group,
            6 => Kind::CodeList,
            7 => Kind::Movie,
            8 => Kind::Effect,
            9 => Kind::ColoredImage,
            10 => Kind::AnimImage,
            11 => Kind::SimpleImage,
            12 => Kind::MultiImage,
            v => Kind::Other(v),
        }
    }
}

/// An object's animatable state (`SA`, 17 dwords; scripts address it by
/// dword offset): colour (B, G, R, A as integers 0..255), pivot,
/// position, rotation, size, then for images the texture rectangle and
/// for coloured images four corner colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attributes {
    pub words: [u32; 37],
    pub len: usize,
}

impl Attributes {
    pub fn color(&self) -> [f32; 4] {
        // Stored B, G, R, A.
        let c = |i: usize| (self.words[i] as i32).clamp(0, 255) as f32 / 255.0;
        [c(2), c(1), c(0), c(3)]
    }
    fn f(&self, i: usize) -> f32 {
        f32::from_bits(self.words[i])
    }
    pub fn pivot(&self) -> Vec3 {
        Vec3::new(self.f(4), self.f(5), self.f(6))
    }
    pub fn position(&self) -> Vec3 {
        Vec3::new(self.f(7), self.f(8), self.f(9))
    }
    pub fn rotation(&self) -> Quat {
        Quat::from_xyzw(self.f(10), self.f(11), self.f(12), self.f(13))
    }
    pub fn size(&self) -> Vec3 {
        Vec3::new(self.f(14), self.f(15), self.f(16))
    }
    /// The texture rectangle (upper left, lower right), when present.
    pub fn uv(&self) -> [f32; 4] {
        if self.len >= 21 { [self.f(17), self.f(18), self.f(19), self.f(20)] } else { [0.0, 0.0, 1.0, 1.0] }
    }
}

/// One keyframe track: which attribute dwords it drives and its keys.
#[derive(Clone, Debug)]
pub struct Track {
    /// `FI`: parameter type (4 vector, 5 quaternion, 6 colour, …).
    pub param: u8,
    /// Value size in bytes.
    pub size: u8,
    pub interpolation: u8,
    /// `FI` byte 3: the loop mode (track +3 & 0x7F; 0 clamp, 1 loop,
    /// 2 ping-pong, 0x52BDD0).
    pub flags: u8,
    pub length: u32,
    /// `To`: first attribute dword.
    pub offset: usize,
    /// The base value (key at time −1)...
    pub base: Vec<u32>,
    /// ...and timed deltas from it (ms).
    pub keys: Vec<(u32, Vec<u32>)>,
}

/// A timed message a script sends (`EV`).
#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub message: u32,
    pub target: u32,
    pub time: u32,
}

#[derive(Clone, Debug)]
pub struct Script {
    /// `Sh`: name hash (`bin_hash` of e.g. "INIT").
    pub name: u32,
    pub length: u32,
    pub flags: u32,
    /// `Sc`: the script to run when this one ends.
    pub chain: Option<u32>,
    pub tracks: Vec<Track>,
    pub events: Vec<Event>,
}

/// A response to a message (`MsgR`), kept raw.
#[derive(Clone, Debug)]
pub struct Response {
    pub raw: Vec<u8>,
}

/// One message node of an object's `MsgR` (0x54D1E0): `Mi` the message
/// hash (node +0xC), then per entry `Ri` type, `Ru` arg 1, `Rt` arg 2
/// (12 bytes each, node +0x14); 0x52C7D0 runs them in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageNode {
    pub message: u32,
    pub entries: Vec<[u32; 3]>,
}

#[derive(Clone, Debug)]
pub struct Object {
    pub kind: Kind,
    pub hash: u32,
    /// `OP`: id, name hash, flags, resource index.
    pub guid: u32,
    pub flags: u32,
    pub resource: u32,
    pub parent: Option<u32>,
    pub attributes: Attributes,
    /// `If`: image flags.
    pub image_flags: u32,
    /// Strings: the text as authored, the label hash looked up in the
    /// language table, justification, leading and wrap width.
    pub text: String,
    pub label: Option<u32>,
    pub justify: u32,
    pub leading: i32,
    pub max_width: u32,
    pub scripts: Vec<Script>,
    pub responses: Vec<Response>,
    /// The `MsgR` nodes, in file order (0x5367F0 runs the first one whose
    /// message matches).
    pub messages: Vec<MessageNode>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Untextured,
    Texture,
    Font,
    Other(u32),
}

#[derive(Clone, Debug)]
pub struct Resource {
    pub name: String,
    pub hash: u32,
    pub kind: ResourceKind,
}

#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    pub path: String,
    pub resources: Vec<Resource>,
    pub objects: Vec<Object>,
    /// `Butn`: raw button list.
    pub buttons: Vec<u32>,
}

impl Package {
    pub fn object(&self, hash: u32) -> Option<&Object> {
        self.objects.iter().find(|o| o.hash == hash)
    }
}
