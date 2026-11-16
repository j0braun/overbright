//! Keys the screens understand.

/// Keys the screens understand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Accept,
    Back,
    /// The shoulder triggers ($JOY_EVENT_FENG_LTRIGGER / _RTRIGGER).
    LTrigger,
    RTrigger,
    /// $JOY_EVENT_FENG_BUTTON1$ (message 0xC519BFC0): the graphics
    /// screens' Cancel Changes / Change Color / Remove All.
    Button1,
    /// $JOY_EVENT_FENG_START$ (comma; PAD_START 0xB5AF2461). The replay's
    /// slow motion takes it (game.rs); the menus' reactions are not ported.
    Start,
    /// $JOY_EVENT_FENG_BUTTON0$ (P; PAD_BUTTON0 0xC519BFBF). The replay
    /// hides the bar with it (0x4D800D → HIDE 0x4A5200).
    Button0,
    /// $JOY_EVENT_FENG_BUTTON3$ (period; PAD_BUTTON3 0xC519BFC2). No code
    /// compares that message.
    Button3,
    /// $JOY_EVENT_FENG_QUIT$ (Q). No pad-table bit is named QUIT (its message hash is not identified).
    Quit,
}
