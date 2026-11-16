use crate::*;

/// The button a Yes / No question focuses. 0x5587A0 and 0x558760 both
/// call 0x558690 with a constant 1 as its eighth parameter (0x5587F7 /
/// 0x55877B: `push 0x1`; the dialog's byte local_c), and their own eighth
/// argument goes to the ninth (local_1c), so every two-button question
/// opens on No (**verified**, menus15: "Reset to default settings?",
/// 0x49B9C7 pushing 0 as its eighth argument, lights No under Wine, as the
/// sites pushing 1 do; menus13 read that argument as the focus). The
/// meaning of local_1c is not traced.
pub(crate) fn ask_focus(_question: u32) -> usize {
    1
}

/// The dialog in `mode` with its second button (No) focused: 0x5587A0's
/// eighth argument 1 (its sites push 1 or 0).
pub(crate) fn focus_no(mut mode: Mode) -> Mode {
    if let Mode::Menu(Menu::Dialog(d, _)) = &mut mode {
        d.selected = 1;
    }
    mode
}

/// A screen dimmed under a dialog (`Dialog::dim_under`) that the dialog
/// left again: IconScrollerMenu StopDim (0x538CB0) plays its Event
/// Handler's UNDIM (0x03D8EABC), which sends 0x9F218E63 to its objects
/// (their UNDIM: alpha back to full over 100 ms). The asking code sends
/// the screen 0x251BF83E when the dialog is answered (IconScrollerMenu's
/// handler 0x543D97 → 0x538CB0: the pauses' Cancel 0x4C230D, Post Race,
/// the dyno 0x564AE4, the decal overlay 0x56B9DA) or calls 0x538CB0 from
/// its own answer handlers (the crib's No 0x4AF4C9, the shops 0x55A7A8 …
/// 0x56B8AB). Kept while that screen is up; `undim_at` None while the
/// dialog still is.
#[derive(Clone, Copy)]
pub(crate) struct Undim {
    pub(crate) package: &'static str,
    pub(crate) dim: &'static [u32],
    pub(crate) undim: &'static [u32],
    pub(crate) dim_at: f32,
    pub(crate) undim_at: Option<f32>,
    pub(crate) scripts: (u32, u32),
}

/// UI_CareerCrib's UNDIM objects (0x4AF4BB): StopDim's Event Handler and
/// the two images 0x444969FD / 0x444969FE.
pub(crate) const CRIB_UNDIM: &[u32] = &[nfsu2::frontend::EVENT_HANDLER, 0x4449_69FD, 0x4449_69FE];

/// The package a menu screen draws (the screens a dialog can dim).
pub(crate) fn menu_package(m: &Menu) -> Option<&'static str> {
    match m {
        Menu::Icons(_, k) => Some(k.package()),
        Menu::CarLot(_) => Some("UI_CareerCarLot"),
        Menu::Parts(b) => Some(b.package),
        Menu::Paint(_) => Some("UI_Paint"),
        // UI_Options' pages under their questions ("Reset to default
        // settings?", Wine menus15).
        Menu::Options(o) => Some(o.category.package(o.pause.is_some())),
        // UI_CareerWorldMap under "Would you like to race this event?"
        // (Wine, the finished-career save: the map stays, dimmed).
        Menu::WorldMap(_) => Some("UI_CareerWorldMap"),
        _ => None,
    }
}

pub(crate) fn mode_package(m: &Mode) -> Option<&'static str> {
    match m {
        Mode::Menu(m) => menu_package(m),
        _ => None,
    }
}

/// A screen opened as a dialog's outcome at `at` is the screen the dialog
/// was pushed over, up for a while already: its opening is moved back so
/// its FADEIN is over (the original never closed it).
pub(crate) fn settle_screen(m: &mut Mode, at: f32) {
    const UP: f32 = 2000.0;
    let opened = match m {
        Mode::Menu(Menu::Icons(i, _)) => &mut i.opened,
        Mode::Menu(Menu::CarLot(l)) => &mut l.opened,
        Mode::Menu(Menu::Parts(b)) => &mut b.opened,
        Mode::Menu(Menu::Paint(p)) => &mut p.opened,
        Mode::Menu(Menu::Options(o)) => &mut o.opened,
        _ => return,
    };
    if *opened > at - UP {
        *opened = at - UP;
    }
}

/// The dialog in `mode` pushed over the screen it asks from (its last
/// menu outcome, drawn under it) and, with `dim`, that screen's StartDim
/// (0x538C10: its Event Handler's DIM) right after the ask; `undim`: the
/// objects StopDim's side plays UNDIM on when it is answered (None: the
/// same).
pub(crate) fn dialog_over(mut mode: Mode, dim: bool, undim: Option<&'static [u32]>) -> Mode {
    if let Mode::Menu(Menu::Dialog(d, outcomes)) = &mut mode {
        let under = d.under.or_else(|| outcomes.iter().rposition(|m| mode_package(m).is_some()));
        if let Some(i) = under {
            d.under = Some(i);
            let package = mode_package(&outcomes[i]);
            for o in outcomes.iter_mut() {
                if package.is_some() && mode_package(o) == package {
                    settle_screen(o, d.opened);
                }
            }
            if dim && let Some(p) = package {
                d.dim_under = Some((p, screens::DIM_HANDLER));
                // UI_CareerCrib's No (0x43DA9FD0 / 0x1FAB5998 → 0x4AF4BB)
                // also plays "UnDim" on 0x444969FD and 0x444969FE.
                d.undim_under = undim.or((p == "UI_CareerCrib").then_some(CRIB_UNDIM));
            }
        }
    }
    mode
}
