//! The texture editor's hooks into the game (docs/modding/texture-editor.md): off
//! unless the game was started with `--editor`; then F7 opens the panel.
//! While it is open it takes the keyboard (key releases still reach the
//! game) and the clicks and wheel over the panel; the game's own cursor
//! is shown.

use crate::*;
use glam::Vec2;
use nfsu2::frontend::mouse as fm;
use nfsu2::tools::overlay::{Built, Input, NavKey, PLACE};
use nfsu2::tools::texed::{self, Dirs, Editor};
use std::sync::Mutex;

/// The hotkey.
pub(crate) const HOTKEY: KeyCode = KeyCode::F7;

struct State {
    editor: Editor,
    /// Input gathered since the last frame.
    input: Input,
    /// The last frame's panel (clicks on it are the editor's).
    last: Option<Built>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    if !texed::enabled() {
        return None;
    }
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = s.get_or_insert_with(|| State { editor: Editor::new(Dirs::player()), input: Input::default(), last: None });
    Some(f(s))
}

/// The panel is open.
pub(crate) fn open() -> bool {
    texed::enabled() && with(|s| s.editor.open).unwrap_or(false)
}

fn nav(code: KeyCode) -> Option<NavKey> {
    Some(match code {
        KeyCode::ArrowUp => NavKey::Up,
        KeyCode::ArrowDown => NavKey::Down,
        KeyCode::PageUp => NavKey::PageUp,
        KeyCode::PageDown => NavKey::PageDown,
        KeyCode::Home => NavKey::Home,
        KeyCode::End => NavKey::End,
        KeyCode::Enter | KeyCode::NumpadEnter => NavKey::Enter,
        KeyCode::Escape => NavKey::Escape,
        KeyCode::Backspace => NavKey::Backspace,
        KeyCode::Tab => NavKey::Tab,
        _ => return None,
    })
}

/// A key pressed: true when the editor took it (the game does nothing
/// with it). F7 opens and closes the panel; while it is open every key but
/// the F keys is the editor's.
pub(crate) fn key(code: KeyCode, text: Option<&str>, repeat: bool) -> bool {
    with(|s| {
        if code == HOTKEY {
            if !repeat {
                s.editor.toggle();
                s.input = Input::default();
                s.last = None;
            }
            return true;
        }
        if !s.editor.open {
            return false;
        }
        let function_key = matches!(
            code,
            KeyCode::F1
                | KeyCode::F2
                | KeyCode::F3
                | KeyCode::F4
                | KeyCode::F5
                | KeyCode::F6
                | KeyCode::F8
                | KeyCode::F9
                | KeyCode::F10
                | KeyCode::F11
                | KeyCode::F12
        );
        if function_key {
            return false;
        }
        if let Some(n) = nav(code) {
            s.input.keys.push(n);
        } else if let Some(t) = text {
            s.input.text.extend(t.chars().filter(|c| !c.is_control()));
        }
        true
    })
    .unwrap_or(false)
}

/// The cursor moved (window pixels; the panel's canvas point kept).
pub(crate) fn cursor(game: &Game, px: Vec2) {
    let Some(frame) = game.ui_frame() else { return };
    let p = fm::canvas_point(&frame, px, PLACE);
    with(|s| s.input.cursor = Some(p));
}

/// The clicking button pressed or the wheel turned: true when the editor
/// took it (over the panel).
pub(crate) fn mouse(game: &Game, event: &InputEvent) -> bool {
    if !open() {
        return false;
    }
    if let InputEvent::CursorMoved(px) = event {
        cursor(game, *px);
        return false;
    }
    with(|s| {
        let over = match (&s.last, s.input.cursor) {
            (Some(b), Some(c)) => b.contains(c),
            _ => false,
        };
        match event {
            InputEvent::Button(true) => {
                // A click anywhere goes to the panel (it takes a text
                // field's focus away); the game gets it only off the panel.
                s.input.clicked = true;
                over
            }
            InputEvent::Wheel(lines) if over => {
                s.input.wheel += lines;
                true
            }
            _ => false,
        }
    })
    .unwrap_or(false)
}

impl Game {
    /// The panel's quads for this frame (none while closed): the textures
    /// drawn since the last frame taken into the list, the editor's
    /// finished jobs, the panel built from the input gathered.
    pub(crate) fn texture_editor_quads(&mut self, out: &mut Vec<Quad>) {
        if !open() {
            return;
        }
        let Some(font) = self.game_data.assets.fonts.values().find(|f| f.name.contains("tt21i")) else { return };
        let renderer = self.render.world.as_ref();
        let textures = self.sim.drive.as_ref().map(|d| &d.world.textures);
        let world = |slot: usize| -> Option<nfsu2::formats::texture::Texture> {
            let hash = renderer?.slot_hash(slot)?;
            textures?.get(&hash).cloned()
        };
        let assets = &self.game_data.assets;
        with(|s| {
            let now = std::time::Instant::now();
            s.editor.take_frame(&world, now);
            s.editor.poll(now);
            let input = std::mem::take(&mut s.input);
            s.input.cursor = input.cursor;
            if let Some(b) = s.editor.panel(font, &input) {
                b.quads(assets, font, out);
                s.last = Some(b);
            }
        });
    }
}
