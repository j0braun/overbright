use crate::frontend::fe_ids;
use super::*;

/// UI_MagazineSelect's cover icons (0x523F70): every magazine shows the
/// "View Magazines" icon, every DVD the "View DVD Covers" one.
pub const MAGAZINE_ICON: u32 = fe_ids::MAGAZINE_ICON;

pub const DVD_ICON: u32 = fe_ids::DVD_ICON;

/// UI_MagazineView (screen 0x554ED0 → 0x553BF0, messages 0x565B60): the
/// cover alone in a black frame. Its constructor (0x565A8E → 0x523D10)
/// finds the cover's magazine setup (0x510790, by the cover 0x837FD8, else
/// the first), picks its parts (0x5105C0), poses the FE car at the picked
/// photo-shoot camera (0x4C2070; MAGAZINES_FRONTEND.BIN chunk 0x30230,
/// `formats::magazines`), loads the setup's environment map and streams
/// the four picked layers into 0x863780 (0x523BD0: +0x10 background,
/// +0x14 masthead, +0x18 model, +0x1C extra; +0x0C set once all are in).
/// The photo shoot's screens show them (**verified**): UI_MagazineBack's
/// tick (0x54B0F0) puts the background on its image "MagazineBack", and
/// UI_MagazineReward's tick (0x55B270) the masthead on "MagazineMast",
/// the model on "MagazineChick" and the extra on "MagazineExtra" (0x495400
/// then shown, 0x495500; else hidden, 0x4954C0), all three 512² at
/// (84, 35) in UI_MagazineReward. The shot (0x4889F0(320, 432) → 0x5C3130) is
/// the back buffer's rectangle from W/2 − w/2 − 10 to W/2 + w/2 + 3 across
/// and H/2 − h/2 − 4 to H/2 + h/2 − 6 down, w = 320 · W/640, h = 432 ·
/// H/480 — at 640 × 480 the 333 × 430 pixels from (150, 20) (`CROP`) —
/// copied into the 512² "ScreenCap" texture; it goes into MagImage
/// (0xD1CC9C37) under "SCREENSHOTHASH". The clone composes that screen at
/// 640 × 480 — the background layer, the career car at the camera, the
// prov: inferred frontend.screens.CoverView
/// masthead and extra — and binds the rectangle under `SHOT` (**inferred**:
/// UI_MagazineBack draws behind the car, the lens across the screen's
// prov: open frontend.screens.CoverView-2
/// width, black behind everything; **open**: the environment map).
///
/// UI_MagazineView (Reward Options → View Magazines / DVD covers) shows
/// the picture the shoot saved: the memory-card library loads `<NAME>
/// Magazine <n>` (or `DVD <n>`) and tells "Load complete." before the view,
/// or "Unable to load <NAME> Magazine <n>." and stays on the list
/// (**observed** under Wine, the finished-career save: both, the loaded cover the
/// community file's own picture). The clone composed a new shot there.
pub struct CoverView {
    /// The milestone whose cover it is, and whether it is a DVD.
    pub milestone: usize,
    pub dvd: bool,
    pub opened: f32,
    /// The photo-shoot camera picked, when the setups loaded.
    pub camera: Option<crate::formats::magazines::PhotoCamera>,
    /// The picked layers (0x5105C0): background, masthead, model, extra.
    pub layers: [u32; 4],
    /// UI_MagazineReward after the photo shoot instead of UI_MagazineView.
    pub reward: bool,
    /// UI_MagazineView: the cover's saved picture is loaded and bound under
    /// `shot()` (the view shows the file the shoot saved, not a new shot).
    pub picture: bool,
}

impl CoverView {
    pub const IMAGE: u32 = fe_ids::IMAGE;
    /// The composed screen the shot is taken from (FEng's canvas).
    pub const SCREEN: (u32, u32) = (640, 480);
    /// The shot's rectangle of it, origin and size (0x5C3130 at 640 × 480).
    pub const CROP: ((u32, u32), (u32, u32)) = ((150, 20), (333, 430));

    /// The picked layers' quads on the photo shoot's screen: behind the
    /// car (UI_MagazineBack's "MagazineBack") and over it
    /// (UI_MagazineReward's "MagazineMast", "MagazineChick",
    /// "MagazineExtra"); a zero pick hides its object. `layers` is
    /// [background, masthead, model, extra].
    pub fn layer_quads(assets: &Assets, layers: [u32; 4]) -> (Vec<Quad>, Vec<Quad>) {
        let fe = crate::formats::hash::fe_hash;
        let one = |package: &str, objects: &[(&str, u32)]| {
            let Some(package) = assets.package(package) else { return Vec::new() };
            let mut state = ScreenState::default();
            for o in &package.objects {
                state.hidden.insert(o.hash);
            }
            for &(name, texture) in objects {
                if texture != 0 {
                    state.hidden.remove(&fe(name));
                    state.image.insert(fe(name), texture);
                }
            }
            layout(assets, package, &state, 0.0)
        };
        (
            one("UI_MagazineBack", &[("MagazineBack", layers[0])]),
            one(
                "UI_MagazineReward",
                &[("MagazineMast", layers[1]), ("MagazineChick", layers[2]), ("MagazineExtra", layers[3])],
            ),
        )
    }

    /// The texture key the shot is bound under: "SCREENSHOTHASH".
    pub fn shot() -> u32 {
        crate::formats::hash::bin_hash("SCREENSHOTHASH")
    }

    /// UI_MagazineReward's layer images (MagazineMast, MagazineChick,
    /// MagazineExtra), which the shot already holds.
    const REWARD_LAYERS: [u32; 3] = [fe_ids::MAGAZINEMAST, fe_ids::MAGAZINECHICK, fe_ids::MAGAZINEEXTRA];

    /// UI_MagazineReward (descriptor 0x7F7FC0, factory 0x56B150 → 0x565760,
    /// vtable 0x79D838, messages 0x55B270) after a photo shoot: the
    /// constructor hides the three layers, shows "Continue" (0xDE02CF84) on
    /// 0xB8A7C6CF and the button 0x69BA0838, loads UI_MagazineBack and
    /// poses the car (0x523D10(0)); its tick shows each loaded layer on its
    /// image (0x495400 / 0x495500). The clone draws the composed shot at
    /// its rectangle (`CROP`) under the package (**inferred**: the game
    /// draws the whole 640 × 480 composition live).
    fn reward_quads(&self, assets: &Assets, now: f32, hover: Option<(u32, f32)>) -> Vec<Quad> {
        let Some(package) = assets.package("UI_MagazineReward") else { return Vec::new() };
        let mut state = ScreenState::default();
        for h in Self::REWARD_LAYERS {
            state.hidden.insert(h);
        }
        state.shown.insert(fe_ids::LABELS_3);
        state.shown.insert(fe_ids::REWARD_BUTTON);
        if let Some(text) = assets.strings.get(fe_ids::CONTINUE) {
            state.text.insert(fe_ids::LABELS_3, text.to_string());
        }
        // 0x55B270's mouse tick: the cursor over the button (+0x7C) runs
        // "Init_Pulse" on it once (+0x78 set), leaving runs "Init".
        if let Some((_, since)) = hover.filter(|&(o, _)| o == Self::REWARD_BUTTON) {
            state.scripts.insert(Self::REWARD_BUTTON, (INIT_PULSE, since - self.opened));
        }
        let mut quads = Vec::new();
        if self.camera.is_some() {
            let ((x, y), (w, h)) = Self::CROP;
            let lo = Vec2::new(x as f32, y as f32);
            quads.push(Quad {
                texture: Some(Self::shot()),
                rect: [lo, lo + Vec2::new(w as f32, h as f32)],
                uv: [Vec2::ZERO, Vec2::ONE],
                color: [1.0; 4],
                z: 255.0,
                angle: 0.0,
                place: Default::default(),
            });
        }
        quads.extend(layout(assets, package, &state, now - self.opened));
        quads
    }

    /// UI_MagazineReward's button (0x69BA0838) and UI_MagazineView's
    /// BACK_BUTTON (0x1C7FCF8B).
    pub const REWARD_BUTTON: u32 = fe_ids::REWARD_BUTTON;
    pub const BACK_BUTTON: u32 = fe_ids::BACK_BUTTON;

    pub fn quads(&self, assets: &Assets, now: f32) -> Vec<Quad> {
        self.quads_hover(assets, now, None)
    }

    /// With the button under the cursor and since when (the screens'
    /// mouse ticks, see `reward_quads`; UI_MagazineView 0x565B60 runs
    /// "Init_Pulse" on BACK_BUTTON while it is hovered, else 0x5BAB0FC0).
    pub fn quads_hover(&self, assets: &Assets, now: f32, hover: Option<(u32, f32)>) -> Vec<Quad> {
        if self.reward {
            return self.reward_quads(assets, now, hover);
        }
        let Some(package) = assets.package("UI_MagazineView") else { return Vec::new() };
        let mut state = ScreenState::default();
        footer(&mut state, package, "Back");
        match hover.filter(|&(o, _)| o == Self::BACK_BUTTON) {
            Some((_, since)) => state.scripts.insert(Self::BACK_BUTTON, (INIT_PULSE, since - self.opened)),
            None => state.scripts.insert(Self::BACK_BUTTON, (0x5BAB_0FC0, now - self.opened)),
        };
        if self.picture || self.camera.is_some() {
            state.image.insert(Self::IMAGE, Self::shot());
        } else {
            state.hidden.insert(Self::IMAGE);
        }
        layout(assets, package, &state, now - self.opened)
    }
}
