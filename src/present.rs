//! Naming a control: what to store it as, and what to show for it.
//!
//! A control needs two strings and they are not the same string. One goes in a settings file and
//! has to mean the same thing next year; the other goes on screen and has to be readable in the
//! player's language. This module is where both come from.
//!
//! ```ignore
//! let control = Control::Key(KeyCode::KeyW);
//! control.name()             // "key/KeyW"  — stored, and the key a catalogue looks up
//! control.fallback_label()   // "W"         — shown when there is no catalogue
//! Control::from_name("key/KeyW")  // Some(control) — and None for anything else
//! ```
//!
//! **The stored name is ours, deliberately.** It would be less code to write out Bevy's own enum
//! variants and read them back, and it would put every saved binding at the mercy of a rename in a
//! crate we do not control. The table below is written once and pinned by a round-trip test; if an
//! upstream name changes, the compiler says so and the stored string stays what it was.
//!
//! **The same string is the localization key.** A rebinding row has two halves, and the control
//! half is as translatable as the other: an app looks up `key/KeyW` in its catalogue and renders
//! whatever its translators wrote. [`fallback_label`](Control::fallback_label) is for when there is
//! no catalogue, so that shipping translations is never the price of a legible screen.
//!
//! # What the fallback cannot do
//!
//! It answers for a US keyboard. A binding to a physical key shows the letter that key carries on
//! that layout, so an AZERTY player is told "W" for a key their keyboard calls Z — which is a bug
//! rather than a rounding error, and one this crate cannot fix alone: nothing in Bevy reports what
//! a physical key produces on the current layout outside of an event that has already happened.
//!
//! What an app can do today is supply the control half of its catalogue per layout. What the crate
//! will be able to do once capture lands is remember the logical key seen at the moment a player
//! bound something, which is right for every binding they chose themselves.

use crate::binding::Control;

#[cfg(feature = "gamepad")]
use bevy_input::gamepad::{GamepadAxis, GamepadButton};
#[cfg(feature = "keyboard")]
use bevy_input::keyboard::KeyCode;

/// Writes the two directions of a control table from one list of entries.
///
/// Gated with its callers: a build with no device features has no tables to write.
///
/// Encoding is an exhaustive match, so a variant added or renamed upstream is a compile error
/// rather than a control that silently stops having a name.
#[cfg(any(feature = "keyboard", feature = "gamepad"))]
macro_rules! control_table {
    ($encode:ident, $decode:ident, $label:ident, $kind:ty, $prefix:literal, $all:ident, {
        $($variant:ident => $text:literal,)*
    }) => {
        fn $encode(value: $kind) -> Option<&'static str> {
            Some(match value {
                $(<$kind>::$variant => concat!($prefix, "/", stringify!($variant)),)*
                _ => return None,
            })
        }

        fn $decode(name: &str) -> Option<$kind> {
            Some(match name {
                $(concat!($prefix, "/", stringify!($variant)) => <$kind>::$variant,)*
                _ => return None,
            })
        }

        fn $label(value: $kind) -> Option<&'static str> {
            Some(match value {
                $(<$kind>::$variant => $text,)*
                _ => return None,
            })
        }

        #[cfg(test)]
        const $all: &[$kind] = &[$(<$kind>::$variant,)*];
    };
}

#[cfg(feature = "keyboard")]
control_table!(key_name, key_from_name, key_label, KeyCode, "key", ALL_KEYS, {
    Backquote => "`",
    Backslash => "\\",
    BracketLeft => "[",
    BracketRight => "]",
    Comma => ",",
    Digit0 => "0",
    Digit1 => "1",
    Digit2 => "2",
    Digit3 => "3",
    Digit4 => "4",
    Digit5 => "5",
    Digit6 => "6",
    Digit7 => "7",
    Digit8 => "8",
    Digit9 => "9",
    Equal => "=",
    IntlBackslash => "Intl Backslash",
    IntlRo => "Intl Ro",
    IntlYen => "Intl Yen",
    KeyA => "A",
    KeyB => "B",
    KeyC => "C",
    KeyD => "D",
    KeyE => "E",
    KeyF => "F",
    KeyG => "G",
    KeyH => "H",
    KeyI => "I",
    KeyJ => "J",
    KeyK => "K",
    KeyL => "L",
    KeyM => "M",
    KeyN => "N",
    KeyO => "O",
    KeyP => "P",
    KeyQ => "Q",
    KeyR => "R",
    KeyS => "S",
    KeyT => "T",
    KeyU => "U",
    KeyV => "V",
    KeyW => "W",
    KeyX => "X",
    KeyY => "Y",
    KeyZ => "Z",
    Minus => "-",
    Period => ".",
    Quote => "'",
    Semicolon => ";",
    Slash => "/",
    AltLeft => "Left Alt",
    AltRight => "Right Alt",
    Backspace => "Backspace",
    CapsLock => "Caps Lock",
    ContextMenu => "Menu",
    ControlLeft => "Left Ctrl",
    ControlRight => "Right Ctrl",
    Enter => "Enter",
    SuperLeft => "Left Super",
    SuperRight => "Right Super",
    ShiftLeft => "Left Shift",
    ShiftRight => "Right Shift",
    Space => "Space",
    Tab => "Tab",
    Convert => "Convert",
    KanaMode => "Kana Mode",
    Lang1 => "Lang1",
    Lang2 => "Lang2",
    Lang3 => "Lang3",
    Lang4 => "Lang4",
    Lang5 => "Lang5",
    NonConvert => "Non Convert",
    Delete => "Delete",
    End => "End",
    Help => "Help",
    Home => "Home",
    Insert => "Insert",
    PageDown => "Page Down",
    PageUp => "Page Up",
    ArrowDown => "Down Arrow",
    ArrowLeft => "Left Arrow",
    ArrowRight => "Right Arrow",
    ArrowUp => "Up Arrow",
    NumLock => "Num Lock",
    Numpad0 => "Numpad 0",
    Numpad1 => "Numpad 1",
    Numpad2 => "Numpad 2",
    Numpad3 => "Numpad 3",
    Numpad4 => "Numpad 4",
    Numpad5 => "Numpad 5",
    Numpad6 => "Numpad 6",
    Numpad7 => "Numpad 7",
    Numpad8 => "Numpad 8",
    Numpad9 => "Numpad 9",
    NumpadAdd => "Numpad +",
    NumpadBackspace => "Numpad Backspace",
    NumpadClear => "Numpad Clear",
    NumpadClearEntry => "Numpad Clear Entry",
    NumpadComma => "Numpad ,",
    NumpadDecimal => "Numpad .",
    NumpadDivide => "Numpad /",
    NumpadEnter => "Numpad Enter",
    NumpadEqual => "Numpad =",
    NumpadHash => "Numpad #",
    NumpadMemoryAdd => "Numpad Memory Add",
    NumpadMemoryClear => "Numpad Memory Clear",
    NumpadMemoryRecall => "Numpad Memory Recall",
    NumpadMemoryStore => "Numpad Memory Store",
    NumpadMemorySubtract => "Numpad Memory Subtract",
    NumpadMultiply => "Numpad *",
    NumpadParenLeft => "Numpad (",
    NumpadParenRight => "Numpad )",
    NumpadStar => "Numpad *",
    NumpadSubtract => "Numpad -",
    Escape => "Esc",
    Fn => "Fn",
    FnLock => "Fn Lock",
    PrintScreen => "Print Screen",
    ScrollLock => "Scroll Lock",
    Pause => "Pause",
    BrowserBack => "Browser Back",
    BrowserFavorites => "Browser Favorites",
    BrowserForward => "Browser Forward",
    BrowserHome => "Browser Home",
    BrowserRefresh => "Browser Refresh",
    BrowserSearch => "Browser Search",
    BrowserStop => "Browser Stop",
    Eject => "Eject",
    LaunchApp1 => "Launch App1",
    LaunchApp2 => "Launch App2",
    LaunchMail => "Launch Mail",
    MediaPlayPause => "Media Play Pause",
    MediaSelect => "Media Select",
    MediaStop => "Media Stop",
    MediaTrackNext => "Media Track Next",
    MediaTrackPrevious => "Media Track Previous",
    Power => "Power",
    Sleep => "Sleep",
    AudioVolumeDown => "Audio Volume Down",
    AudioVolumeMute => "Audio Volume Mute",
    AudioVolumeUp => "Audio Volume Up",
    WakeUp => "Wake Up",
    Meta => "Meta",
    Hyper => "Hyper",
    Turbo => "Turbo",
    Abort => "Abort",
    Resume => "Resume",
    Suspend => "Suspend",
    Again => "Again",
    Copy => "Copy",
    Cut => "Cut",
    Find => "Find",
    Open => "Open",
    Paste => "Paste",
    Props => "Props",
    Select => "Select",
    Undo => "Undo",
    Hiragana => "Hiragana",
    Katakana => "Katakana",
    F1 => "F1",
    F2 => "F2",
    F3 => "F3",
    F4 => "F4",
    F5 => "F5",
    F6 => "F6",
    F7 => "F7",
    F8 => "F8",
    F9 => "F9",
    F10 => "F10",
    F11 => "F11",
    F12 => "F12",
    F13 => "F13",
    F14 => "F14",
    F15 => "F15",
    F16 => "F16",
    F17 => "F17",
    F18 => "F18",
    F19 => "F19",
    F20 => "F20",
    F21 => "F21",
    F22 => "F22",
    F23 => "F23",
    F24 => "F24",
    F25 => "F25",
    F26 => "F26",
    F27 => "F27",
    F28 => "F28",
    F29 => "F29",
    F30 => "F30",
    F31 => "F31",
    F32 => "F32",
    F33 => "F33",
    F34 => "F34",
    F35 => "F35",});

#[cfg(feature = "gamepad")]
control_table!(button_name, button_from_name, button_label, GamepadButton, "pad", ALL_BUTTONS, {
    South => "South Button",
    East => "East Button",
    North => "North Button",
    West => "West Button",
    C => "C Button",
    Z => "Z Button",
    LeftTrigger => "Left Bumper",
    LeftTrigger2 => "Left Trigger",
    RightTrigger => "Right Bumper",
    RightTrigger2 => "Right Trigger",
    Select => "Select",
    Start => "Start",
    Mode => "Guide",
    LeftThumb => "Left Stick Press",
    RightThumb => "Right Stick Press",
    DPadUp => "D-Pad Up",
    DPadDown => "D-Pad Down",
    DPadLeft => "D-Pad Left",
    DPadRight => "D-Pad Right",});

#[cfg(feature = "gamepad")]
control_table!(axis_name, axis_from_name, axis_label, GamepadAxis, "axis", ALL_AXES, {
    LeftStickX => "Left Stick X",
    LeftStickY => "Left Stick Y",
    LeftZ => "Left Z",
    RightStickX => "Right Stick X",
    RightStickY => "Right Stick Y",
    RightZ => "Right Z",});

impl Control {
    /// The name this control is stored and looked up under.
    ///
    /// Stable across versions of this crate and of Bevy, so it is safe to write into a settings
    /// file — and it doubles as the key an app's translation catalogue answers to. Round-trips
    /// through [`from_name`](Control::from_name).
    ///
    /// ```ignore
    /// Control::Key(KeyCode::Space).name()             // "key/Space"
    /// Control::GamepadButton(GamepadButton::South).name()  // "pad/South"
    /// ```
    pub fn name(self) -> alloc::borrow::Cow<'static, str> {
        use alloc::borrow::Cow;

        match self {
            #[cfg(feature = "keyboard")]
            // `KeyCode` has no unnamed variants, so the table covers it exhaustively.
            Self::Key(key) => Cow::Borrowed(key_name(key).unwrap_or("key/unknown")),
            #[cfg(feature = "gamepad")]
            Self::GamepadButton(button) => button_name(button).map_or_else(
                || match button {
                    GamepadButton::Other(index) => Cow::Owned(alloc::format!("pad/other/{index}")),
                    _ => Cow::Borrowed("pad/unknown"),
                },
                Cow::Borrowed,
            ),
            #[cfg(feature = "gamepad")]
            Self::GamepadAxis(axis) => axis_name(axis).map_or_else(
                || match axis {
                    GamepadAxis::Other(index) => Cow::Owned(alloc::format!("axis/other/{index}")),
                    _ => Cow::Borrowed("axis/unknown"),
                },
                Cow::Borrowed,
            ),
            Self::MouseMotion => Cow::Borrowed("mouse/motion"),
        }
    }

    /// Reads back a control written by [`name`](Control::name).
    ///
    /// `None` for a name this build does not know, which is what a binding saved against a control
    /// that has since gone away looks like — worth reporting to the player rather than discarding
    /// in silence.
    pub fn from_name(name: &str) -> Option<Self> {
        #[cfg(feature = "keyboard")]
        if let Some(key) = key_from_name(name) {
            return Some(Self::Key(key));
        }
        #[cfg(feature = "gamepad")]
        if let Some(button) = button_from_name(name) {
            return Some(Self::GamepadButton(button));
        }
        #[cfg(feature = "gamepad")]
        if let Some(axis) = axis_from_name(name) {
            return Some(Self::GamepadAxis(axis));
        }
        #[cfg(feature = "gamepad")]
        if let Some(index) = name.strip_prefix("pad/other/") {
            return index
                .parse()
                .ok()
                .map(|index| Self::GamepadButton(GamepadButton::Other(index)));
        }
        #[cfg(feature = "gamepad")]
        if let Some(index) = name.strip_prefix("axis/other/") {
            return index
                .parse()
                .ok()
                .map(|index| Self::GamepadAxis(GamepadAxis::Other(index)));
        }
        match name {
            "mouse/motion" => Some(Self::MouseMotion),
            _ => None,
        }
    }

    /// Readable text for a game with no translation catalogue.
    ///
    /// Use it as the fallback when a catalogue lookup on [`name`](Control::name) misses, not in
    /// place of one: it answers for a US keyboard, and names gamepad controls by position rather
    /// than by any one manufacturer's letters.
    pub fn fallback_label(self) -> alloc::borrow::Cow<'static, str> {
        use alloc::borrow::Cow;

        match self {
            #[cfg(feature = "keyboard")]
            Self::Key(key) => Cow::Borrowed(key_label(key).unwrap_or("Unknown Key")),
            #[cfg(feature = "gamepad")]
            Self::GamepadButton(button) => button_label(button).map_or_else(
                || match button {
                    GamepadButton::Other(index) => Cow::Owned(alloc::format!("Button {index}")),
                    _ => Cow::Borrowed("Unknown Button"),
                },
                Cow::Borrowed,
            ),
            #[cfg(feature = "gamepad")]
            Self::GamepadAxis(axis) => axis_label(axis).map_or_else(
                || match axis {
                    GamepadAxis::Other(index) => Cow::Owned(alloc::format!("Axis {index}")),
                    _ => Cow::Borrowed("Unknown Axis"),
                },
                Cow::Borrowed,
            ),
            Self::MouseMotion => Cow::Borrowed("Mouse"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    /// The promise the format makes: what was written comes back meaning the same thing. Run over
    /// every control there is, because a table is exactly the kind of thing that is right in the
    /// cases anyone thinks to check by hand.
    #[test]
    fn every_control_survives_being_written_and_read() {
        let round_trip = |control: Control| {
            let name = control.name();
            assert_eq!(
                Control::from_name(&name),
                Some(control),
                "{name} did not read back"
            );
        };

        #[cfg(feature = "keyboard")]
        for &key in ALL_KEYS {
            round_trip(Control::Key(key));
        }
        #[cfg(feature = "gamepad")]
        for &button in ALL_BUTTONS {
            round_trip(Control::GamepadButton(button));
        }
        #[cfg(feature = "gamepad")]
        for &axis in ALL_AXES {
            round_trip(Control::GamepadAxis(axis));
        }
        round_trip(Control::MouseMotion);
        #[cfg(feature = "gamepad")]
        {
            round_trip(Control::GamepadButton(GamepadButton::Other(7)));
            round_trip(Control::GamepadAxis(GamepadAxis::Other(3)));
        }
    }

    /// Two controls sharing a name would mean one binding reading back as another.
    #[test]
    fn no_two_controls_share_a_name() {
        let mut names = alloc::vec::Vec::new();
        #[cfg(feature = "keyboard")]
        names.extend(ALL_KEYS.iter().map(|&key| Control::Key(key).name()));
        #[cfg(feature = "gamepad")]
        names.extend(
            ALL_BUTTONS
                .iter()
                .map(|&button| Control::GamepadButton(button).name()),
        );
        #[cfg(feature = "gamepad")]
        names.extend(
            ALL_AXES
                .iter()
                .map(|&axis| Control::GamepadAxis(axis).name()),
        );
        names.push(Control::MouseMotion.name());

        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "a name is used twice");
    }

    /// A name this build does not know is the shape of a binding saved against a control that has
    /// gone away. It has to be distinguishable from a control, not guessed at.
    #[test]
    fn an_unknown_name_reads_as_nothing() {
        assert_eq!(Control::from_name("key/NoSuchKey"), None);
        assert_eq!(Control::from_name("KeyW"), None, "the prefix is required");
        assert_eq!(Control::from_name(""), None);
        assert_eq!(Control::from_name("pad/other/nine"), None);
    }

    /// The stored name and the shown text are different strings, which is the whole reason there
    /// are two of them.
    #[cfg(all(feature = "keyboard", feature = "gamepad"))]
    #[test]
    fn what_is_stored_is_not_what_is_shown() {
        let space = Control::Key(KeyCode::Space);
        assert_eq!(space.name(), "key/Space");
        assert_eq!(space.fallback_label(), "Space");

        assert_eq!(Control::Key(KeyCode::KeyW).fallback_label(), "W");
        assert_eq!(
            Control::Key(KeyCode::ShiftLeft).fallback_label(),
            "Left Shift"
        );
        assert_eq!(Control::Key(KeyCode::Digit1).fallback_label(), "1");
        assert_eq!(Control::Key(KeyCode::ArrowUp).fallback_label(), "Up Arrow");
        assert_eq!(
            Control::Key(KeyCode::NumpadAdd).fallback_label(),
            "Numpad +"
        );
        assert_eq!(Control::Key(KeyCode::Backquote).fallback_label(), "`");

        // Named by position rather than by any one manufacturer's letters, and the two triggers
        // are told apart by what they are rather than by Bevy's numbering.
        assert_eq!(
            Control::GamepadButton(GamepadButton::South).fallback_label(),
            "South Button"
        );
        assert_eq!(
            Control::GamepadButton(GamepadButton::LeftTrigger).fallback_label(),
            "Left Bumper"
        );
        assert_eq!(
            Control::GamepadButton(GamepadButton::LeftTrigger2).fallback_label(),
            "Left Trigger"
        );
        assert_eq!(
            Control::GamepadAxis(GamepadAxis::LeftStickX).fallback_label(),
            "Left Stick X"
        );
        assert_eq!(Control::MouseMotion.fallback_label().to_string(), "Mouse");
    }

    /// Every control has something to show. A label nobody wrote would surface as a blank row.
    #[test]
    fn nothing_is_left_without_a_label() {
        #[cfg(feature = "keyboard")]
        for &key in ALL_KEYS {
            let label = Control::Key(key).fallback_label();
            assert!(!label.is_empty(), "{key:?} has no label");
            assert_ne!(label, "Unknown Key", "{key:?} fell through the table");
        }
        #[cfg(feature = "gamepad")]
        for &button in ALL_BUTTONS {
            assert!(!Control::GamepadButton(button).fallback_label().is_empty());
        }
    }
}
