//! A dialog's DIM / UNDIM over the screen under it (the `DIM` thread-local and its scopes).

use crate::frontend::feng;
use crate::frontend::feng::Package;
use crate::frontend::feng::package_run::apply;

/// "DIM" (fe_hash, 0x00009E99): the script a screen's objects fade with
/// while a dialog is up over it.
pub const DIM_SCRIPT: u32 = 0x0000_9E99;
/// "UNDIM" (fe_hash "UnDim", 0x03D8EABC): the script that brings a
/// dimmed screen back when the dialog over it closes.
pub const UNDIM_SCRIPT: u32 = 0x03D8_EABC;
/// "MENUFADEOUT" (0x79BE40, fe_hash 0x9B90037C): UI_GenericParts_Browser's
/// Event Handler script its exit question plays (0x567272), sending
/// 0x821F58AE to every object (target 0), whose own MENUFADEOUT fades them
/// out, and 0xE1FDE1D1 to the code at 140 ms.
pub const MENUFADEOUT_SCRIPT: u32 = 0x9B90_037C;
/// "MENUFADE" (0x79D798, fe_hash 0x16DEFAA4): the browser's No (0x1FAB5998
/// at 0x5674D8, out of a category 0x567557) plays it on the handler; it
/// sends 0xDDFC69A0 to every object (their MENUFADE: back in).
pub const MENUFADE_SCRIPT: u32 = 0x16DE_FAA4;

/// The script a dialog's dim is playing on `package` now (None: none, or
/// its undim has started).
pub fn dim_script(package: &str) -> Option<u32> {
    DIM.with(|d| {
        d.borrow()
            .as_ref()
            .filter(|d| d.package.eq_ignore_ascii_case(package) && d.undim_ms.is_none())
            .map(|d| d.script)
    })
}
/// The screens' "Event Handler" object (0x47FF4E7C).
pub const EVENT_HANDLER: u32 = 0x47FF_4E7C;

#[derive(Clone)]
pub(crate) struct Dim {
    pub(crate) package: &'static str,
    pub(crate) objects: Vec<u32>,
    pub(crate) age_ms: f32,
    /// How long ago the dialog closed and the screen's UNDIM started.
    pub(crate) undim_ms: Option<f32>,
    /// The objects UNDIM is played on.
    pub(crate) undim_objects: Vec<u32>,
    /// The scripts played: DIM / UNDIM, or another pair (the parts
    /// browser's exit question: MENUFADEOUT / MENUFADE).
    pub(crate) script: u32,
    pub(crate) unscript: u32,
}

thread_local! {
    pub(crate) static DIM: std::cell::RefCell<Option<Dim>> = const { std::cell::RefCell::new(None) };
}

/// Lay out the screen under a dialog with its DIM started `age_ms` ago on
/// `objects` of `package` (by default the Event Handler): IconScrollerMenu
/// StartDim (0x538C10) plays the package's Event Handler script 0x9E99
/// (0x537C10(package, 0x47FF4E7C, "DIM") does the same), whose event, when
/// the package has one, sends 0xA301C015 to every object; those that
/// answer it run their own DIM (an alpha track over 100 ms: groups to 0,
/// some images to 60). Packages whose handler's DIM has no event (e.g.
/// UI_ChoosePaintCategory, UI_ChooseVinylLayer) stay as they were, and
/// those with no handler DIM (UI_Sponsorship_new) too.
pub fn with_dim<R>(package: &'static str, objects: &[u32], age_ms: f32, f: impl FnOnce() -> R) -> R {
    with_dim_undim(package, objects, objects, age_ms, None, f)
}

/// [`with_dim`], and with `undim_ms` the screen's UNDIM started that long
/// ago on the same objects: IconScrollerMenu StopDim (0x538CB0, reached by
/// 0x251BF83E at 0x543D97, which the asking code sends when the dialog is
/// answered, or called straight from the shops' handlers) plays the
/// package's Event Handler script UNDIM (0x03D8EABC), whose event sends
/// 0x9F218E63 to every object (target 0xFFFFFFFC); those that answer it
/// run their own UNDIM (alpha tracks back to full over 100 ms). UI_Paint
/// in a slot also plays "UnDim" (0x790CEC) on its group 0xDB285767
/// (0x5634C5, 0x5639BE); UI_CareerCrib's No "UnDim"s 0x444969FD / FE
/// besides (0x4AF4BB): `undim` names the UNDIM's objects.
pub fn with_dim_undim<R>(
    package: &'static str,
    objects: &[u32],
    undim: &[u32],
    age_ms: f32,
    undim_ms: Option<f32>,
    f: impl FnOnce() -> R,
) -> R {
    with_dim_scripts(package, objects, undim, (DIM_SCRIPT, UNDIM_SCRIPT), age_ms, undim_ms, f)
}

/// [`with_dim_undim`] with another pair of scripts in place of DIM / UNDIM:
/// UI_GenericParts_Browser's exit question plays MENUFADEOUT on its Event
/// Handler (0x567272: 0x537C10(package, 0x47FF4E7C, "MENUFADEOUT")) and No
/// MENUFADE (0x56755C) — see [`MENUFADEOUT_SCRIPT`].
pub fn with_dim_scripts<R>(
    package: &'static str,
    objects: &[u32],
    undim: &[u32],
    scripts: (u32, u32),
    age_ms: f32,
    undim_ms: Option<f32>,
    f: impl FnOnce() -> R,
) -> R {
    let old = DIM.with(|d| {
        d.borrow_mut().replace(Dim {
            package,
            objects: objects.to_vec(),
            age_ms,
            undim_ms,
            undim_objects: undim.to_vec(),
            script: scripts.0,
            unscript: scripts.1,
        })
    });
    let r = f();
    DIM.with(|d| *d.borrow_mut() = old);
    r
}

/// The alpha (0–1) an icon cloned from `template` has under the screen's
/// DIM (see [`with_dim`]): IconScrollerMenu's options copy their
/// template's scripts and answer 0xA301C015 as it does, so they fade with
/// its DIM; 1 when no DIM is running on `package` or the template has none.
pub fn dim_alpha(package: &Package, template: &feng::Object) -> f32 {
    dim_alpha_of(package, template).unwrap_or(1.0)
}

/// [`dim_alpha`], None when no DIM reaches the template (no dialog's DIM
/// on `package`, its handler's DIM sends nothing, the template has no DIM,
/// or the screen's UNDIM has started). IconScrollerMenu StartDim's
/// 0x51FD20 clears the strip's fade (+0xE4 = 0) and sets every option's
/// colour from its item (+0x1C, 0x51D800), so while Some the options show
/// the DIM's alpha alone, not their alpha by place (0x52F8D0 runs
/// 0x50D8D0 → 0x51FD70 only with +0xE4 set).
pub fn dim_alpha_of(package: &Package, template: &feng::Object) -> Option<f32> {
    let Some(dim) = DIM.with(|d| d.borrow().clone()) else { return None };
    if !package.name.trim_end_matches(".fng").eq_ignore_ascii_case(dim.package) || !dim.objects.contains(&EVENT_HANDLER)
    {
        return None;
    }
    // The handler's DIM reaches the objects only when it sends the message.
    let sends = package
        .objects
        .iter()
        .filter(|o| o.hash == EVENT_HANDLER)
        .flat_map(|o| o.scripts.iter().filter(|s| s.name == dim.script))
        .any(|s| !s.events.is_empty());
    // StopDim (0x538CB0 → 0x538B40) turns the strip's own fade back on
    // (+0xE4 = 1, which StartDim's 0x51FD20 cleared) and sets every icon's
    // alpha by its place (0x52F8D0 → 0x50D8D0, 0x51FD70) at once: the
    // icons are back at the answer while the rest runs UNDIM (Wine
    // menus12 pause_undim u09).
    if dim.undim_ms.is_some() {
        return None;
    }
    let s = template.scripts.iter().find(|s| s.name == dim.script).filter(|_| sends)?;
    let mut a = template.attributes;
    apply(&mut a, s, dim.age_ms.clamp(0.0, s.length as f32) as u32);
    Some(a.color()[3])
}
