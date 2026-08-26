//! Prompts: which control fires an action, and what to call it.
//!
//! "Press ⟨something⟩ to jump" is two questions. Which control — the reverse of everything else in
//! this crate, which turns controls into values and forgets the control on the way — and what to
//! call it on screen. [`Prompts`] answers the first and [`Control::name`] and
//! [`Control::fallback_label`] answer the second.
//!
//! # Naming a control
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
//!
//! # Which control
//!
//! ```ignore
//! for prompt in BindingTable::new(world).prompts(Jump::id(), PromptScope::ANY) {
//!     println!("{}", prompt.origin.fallback_label());
//! }
//! ```
//!
//! This is a **runtime** question rather than a question about what the game declared. A control
//! only answers for an action if something is carrying the context the binding lives in, if that
//! context is active, and if nothing evaluated earlier in the frame takes the control away — so the
//! answer changes as the game runs, and a caller that asks before its contexts exist is told
//! nothing rather than told what they will say. [`mapping::mappings`](crate::mapping::mappings) is
//! the other list, and the one a controls screen wants: everything the game declared, whether or
//! not it is live.
//!
//! [`Prompts`] is a trait because this crate is not always the authority. Where an external backend
//! owns the bindings, it owns the answer too, and its controls are its own enumeration of things we
//! have no name for — which is why an [`ControlOrigin`] is not a [`Control`].

use alloc::string::String;
use alloc::vec::Vec;

use bevy_ecs::world::World;

use crate::action::ActionId;
use crate::binding::{Control, Part};
use crate::capture::ControlClass;
use crate::condition::ConditionDescriptor;
use crate::mapping::Scheme;

#[cfg(feature = "gamepad")]
use bevy_input::gamepad::{GamepadAxis, GamepadButton};
#[cfg(feature = "keyboard")]
use bevy_input::keyboard::KeyCode;
#[cfg(feature = "mouse")]
use bevy_input::mouse::MouseButton;

/// Writes the two directions of a control table from one list of entries.
///
/// Gated with its callers: a build with no device features has no tables to write.
///
/// Encoding is an exhaustive match, so a variant added or renamed upstream is a compile error
/// rather than a control that silently stops having a name.
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
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

// `Back` and `Forward` are the thumb buttons, which every settings screen a player has seen calls
// Mouse 4 and Mouse 5 — the same choice made for `LeftTrigger` above. `Other` is spelled out as
// "Mouse Button {n}" rather than "Mouse {n}" so that a raw index can never be read as one of those.
#[cfg(feature = "mouse")]
control_table!(mouse_name, mouse_from_name, mouse_label, MouseButton, "mouse", ALL_MOUSE_BUTTONS, {
    Left => "Left Mouse",
    Right => "Right Mouse",
    Middle => "Middle Mouse",
    Back => "Mouse 4",
    Forward => "Mouse 5",});

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
            #[cfg(feature = "mouse")]
            Self::MouseButton(button) => mouse_name(button).map_or_else(
                || match button {
                    MouseButton::Other(index) => Cow::Owned(alloc::format!("mouse/other/{index}")),
                    _ => Cow::Borrowed("mouse/unknown"),
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
        #[cfg(feature = "mouse")]
        if let Some(button) = mouse_from_name(name) {
            return Some(Self::MouseButton(button));
        }
        #[cfg(feature = "mouse")]
        if let Some(index) = name.strip_prefix("mouse/other/") {
            return index
                .parse()
                .ok()
                .map(|index| Self::MouseButton(MouseButton::Other(index)));
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
            #[cfg(feature = "mouse")]
            Self::MouseButton(button) => mouse_label(button).map_or_else(
                || match button {
                    MouseButton::Other(index) => Cow::Owned(alloc::format!("Mouse Button {index}")),
                    _ => Cow::Borrowed("Unknown Mouse Button"),
                },
                Cow::Borrowed,
            ),
            Self::MouseMotion => Cow::Borrowed("Mouse"),
        }
    }
}

/// One physical control a prompt can name.
///
/// Usually one of ours. It is an enum rather than a [`Control`] because the crate is not always the
/// authority on what is bound: where an external backend owns the bindings, its controls are its
/// own enumeration and cover device families this crate has no variant for and never will. Both
/// variants answer [`name`](ControlOrigin::name) and [`fallback_label`](ControlOrigin::fallback_label), so a
/// screen renders one without first asking which it was handed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlOrigin {
    /// A control this crate knows.
    Ours(Control),
    /// A control only whatever reported it knows.
    Foreign {
        /// What it is stored and looked up under, on the same terms as [`Control::name`]: an
        /// identity that must mean the same thing next year, and the key a catalogue answers to.
        name: String,
        /// Readable text for a game whose catalogue has no entry for `name`.
        label: String,
        /// Which set of devices it belongs to, where the reporter said. `None` where it belongs to
        /// neither — a control on a device family this crate does not model.
        scheme: Option<Scheme>,
        /// What kind of signal it reports, where the reporter said. `None` where it did not, which
        /// is why a caller narrowing by class has to decide whether an unclassified control
        /// belongs in the answer.
        class: Option<ControlClass>,
    },
}

impl ControlOrigin {
    /// What this control is stored and looked up under.
    pub fn name(&self) -> alloc::borrow::Cow<'_, str> {
        match self {
            Self::Ours(control) => control.name(),
            Self::Foreign { name, .. } => alloc::borrow::Cow::Borrowed(name),
        }
    }

    /// Readable text for a game with no translation catalogue.
    pub fn fallback_label(&self) -> alloc::borrow::Cow<'_, str> {
        match self {
            Self::Ours(control) => control.fallback_label(),
            Self::Foreign { label, .. } => alloc::borrow::Cow::Borrowed(label),
        }
    }

    /// Which set of devices this control belongs to, where that is known.
    pub const fn scheme(&self) -> Option<Scheme> {
        match self {
            Self::Ours(control) => Some(control.scheme()),
            Self::Foreign { scheme, .. } => *scheme,
        }
    }

    /// What kind of signal this control reports, where that is known.
    ///
    /// Known for every control of ours, since the class follows from the channel it reports on. A
    /// foreign control answers only if whatever reported it said so, which is why `None` here means
    /// the question went unanswered rather than that the control has no class.
    pub const fn class(&self) -> Option<ControlClass> {
        match self {
            Self::Ours(control) => ControlClass::of(control.shape()),
            Self::Foreign { class, .. } => *class,
        }
    }

    /// The control itself, for a caller that needs more than a name for it.
    ///
    /// `None` for one that came from somewhere else, which is the case a caller reaching for this
    /// has to have an answer for.
    pub const fn control(&self) -> Option<Control> {
        match self {
            Self::Ours(control) => Some(*control),
            Self::Foreign { .. } => None,
        }
    }
}

/// One way to fire an action, as a prompt would show it.
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    /// The control that fires it.
    pub origin: ControlOrigin,
    /// What has to be held for that control to count, in the order it was declared.
    ///
    /// Empty for almost everything. A binding that requires a modifier alongside its own control is
    /// the exception, and dropping this would caption `Ctrl+S` as "S".
    pub with: Vec<ControlOrigin>,
    /// Which part of a composite this control drives.
    ///
    /// [`Part::Whole`] unless the action is bound to an arrangement of several controls, in which
    /// case there is one prompt per part and this is what tells them apart.
    pub part: Part,
    /// What besides pressing the control this binding requires — held a while, tapped twice.
    ///
    /// [`ConditionDescriptor::None`] for almost everything, on the same terms as
    /// [`with`](Self::with) being empty: most bindings fire on a bare press and have nothing here to
    /// say.
    pub condition: ConditionDescriptor,
    /// The path of the context the binding lives in, where it came from a context at all.
    pub context: Option<&'static str>,
}

/// How much of the binding set a lookup is asking about.
///
/// [`ANY`](PromptScope::ANY) is everything. The narrowings are the ones a screen actually wants:
/// one device's worth, because a prompt shows the control the player is holding rather than all of
/// them; one context's, because the same action may be bound differently in two of them; and one
/// kind of signal, for a caller with room to draw a button and none to draw a stick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PromptScope {
    /// Only bindings declared in the context with this path.
    pub context: Option<&'static str>,
    /// Only controls belonging to this set of devices.
    pub scheme: Option<Scheme>,
    /// Only controls reporting this kind of signal.
    ///
    /// A control whose class is unknown is not in the answer. That happens only to one supplied by
    /// a backend that did not say what it was, and passing it through would hand a caller who asked
    /// for a button something it has no idea how to draw.
    pub class: Option<ControlClass>,
}

impl PromptScope {
    /// Everything currently bound, on any device, in any context.
    pub const ANY: Self = Self {
        context: None,
        scheme: None,
        class: None,
    };

    /// Narrows to one context, named by the path it declared.
    pub const fn in_context(mut self, path: &'static str) -> Self {
        self.context = Some(path);
        self
    }

    /// Narrows to one set of devices.
    pub const fn on(mut self, scheme: Scheme) -> Self {
        self.scheme = Some(scheme);
        self
    }

    /// Narrows to one kind of signal.
    pub const fn of(mut self, class: ControlClass) -> Self {
        self.class = Some(class);
        self
    }
}

/// Who to ask what a control prompt should say.
///
/// A trait rather than a function over this crate's own tables, because those tables are not always
/// the authority: a game whose bindings live in an external backend asks the backend, gets that
/// backend's own controls back, and renders them through the same [`ControlOrigin`]. The trait is about
/// *who is asked*; [`BindingTable`] is the answer when the asking stops here.
pub trait Prompts {
    /// The controls that would currently fire `action`, strongest first.
    ///
    /// Empty is a real answer, and the common way to get one is an action whose context nothing is
    /// carrying or nothing has activated.
    fn prompts(&self, action: ActionId, scope: PromptScope) -> Vec<Prompt>;
}

/// This crate's own binding tables, as a source of prompts.
///
/// # What "strongest first" means, and what it does not
///
/// Contexts come back in the order they get to claim a control: render-tick contexts before
/// fixed-tick ones, then by priority, then in the order they were declared. Within one context the
/// order is the order the bindings were written, which is what makes the first one the primary.
///
/// **Nothing here ranks one device above another.** A player on a pad should be shown the pad
/// control first, and this cannot know which device they are holding — that wants a per-player
/// record of the most recently used one, which does not exist yet. So a caller that knows passes a
/// [`PromptScope`], and a caller that does not is given every device's answer in a stable order rather
/// than a guess presented as a ranking.
pub struct BindingTable<'w>(&'w World);

impl<'w> BindingTable<'w> {
    /// Reads prompts from the contexts declared in this world.
    pub const fn new(world: &'w World) -> Self {
        Self(world)
    }
}

impl Prompts for BindingTable<'_> {
    fn prompts(&self, action: ActionId, scope: PromptScope) -> Vec<Prompt> {
        let Some(declared) = self.0.get_resource::<crate::inspect::DeclaredContexts>() else {
            return Vec::new();
        };

        // Every live context, not only the ones in scope: what removes a control from the answer is
        // some *other* context claiming it, and narrowing the scope must not hide that.
        let mut live: Vec<_> = declared
            .0
            .iter()
            .map(|context| (context, (context.bindings)(self.0)))
            .filter(|(_, bound)| bound.active)
            .collect();
        // Consumption flows forward in schedule order and priority orders within a schedule, so
        // this is exactly the order in which contexts get to take a control from one another. The
        // sort is stable, which leaves declaration order as the last tiebreak.
        live.sort_by_key(|(context, _)| {
            (
                match context.tick {
                    crate::action::TickDomain::Render => 0,
                    crate::action::TickDomain::Fixed => 1,
                },
                core::cmp::Reverse(context.priority),
            )
        });

        let mut prompts: Vec<Prompt> = Vec::new();
        for (index, (context, bound)) in live.iter().enumerate() {
            if scope.context.is_some_and(|path| path != context.path) {
                continue;
            }
            for entry in &bound.prompts {
                if entry.action != action {
                    continue;
                }
                if scope
                    .scheme
                    .is_some_and(|scheme| scheme != entry.control.scheme())
                {
                    continue;
                }
                if scope
                    .class
                    .is_some_and(|class| !class.contains(entry.control))
                {
                    continue;
                }
                // Taken by something stronger, for something else: pressing it does that instead,
                // and a prompt saying otherwise is telling the player a lie they can check.
                if live[..index].iter().any(|(_, earlier)| {
                    earlier
                        .claims
                        .iter()
                        .any(|&(control, by)| control == entry.control && by != action)
                }) {
                    continue;
                }
                let prompt = Prompt {
                    origin: ControlOrigin::Ours(entry.control),
                    with: entry
                        .chord
                        .iter()
                        .copied()
                        .map(ControlOrigin::Ours)
                        .collect(),
                    part: entry.part,
                    condition: entry.condition,
                    context: Some(context.path),
                };
                // The same control reached twice — one action bound in two contexts, most often —
                // is one prompt. A caption naming a key twice is noise rather than information,
                // and which context it came from is not what the player is being told.
                if prompts.iter().any(|seen| {
                    seen.origin == prompt.origin
                        && seen.part == prompt.part
                        && seen.with == prompt.with
                        && seen.condition == prompt.condition
                }) {
                    continue;
                }
                prompts.push(prompt);
            }
        }
        prompts
    }
}

/// Which device family a prompt speaks for when nothing says otherwise.
///
/// Almost every game has one. A console title's prompts name pad buttons even when a keyboard is
/// plugged in; a desktop title's name keys even when a pad is. So this is a fact about the game
/// rather than about the moment, usually set once at startup and never touched again.
///
/// **Absence means the game has not said**, which is not the same as saying there is no primary:
/// a game that genuinely treats every device alike inserts this holding `None` and gets an answer
/// ranked by nothing, deliberately. Nothing in this crate inserts it for you, because a default
/// here would be a guess about which device your players hold, and being wrong about that is
/// silent — every prompt in the game names the wrong control and nothing reports it.
///
/// A game that *does* change it while running — prompts that follow the device just used — has to
/// say so with [`PromptGeneration::invalidate`], since a resource being written is not something
/// this crate watches for.
#[derive(bevy_ecs::resource::Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptDevice(pub Option<Scheme>);

/// Counts the times the answer to a prompt lookup may have changed.
///
/// A prompt on screen goes stale when a binding changes, when a context activates or stops being
/// carried, or when the game changes which device it speaks for. Recomputing every prompt every
/// frame in case one of those happened is what this exists to avoid: whatever draws prompts runs
/// when this changes and is skipped when it does not.
///
/// # Reading it
///
/// Either way round, and the crate is deliberately neutral between them. A run condition —
/// `resource_changed::<PromptGeneration>` — coalesces a frame's worth of changes into one pass and
/// rewrites at a point in the schedule you choose, which is what a text layer wants. An observer
/// works too, because a resource is a component on an entity of its own: the touch below is an
/// insert rather than a mutable deref precisely so that it fires hooks, so an observer of
/// `Insert`/`Replace` on this type is a live signal for a consumer that would rather not own a
/// system. It runs once per change rather than once per frame, which is the trade.
///
/// # Writing it
///
/// The crate raises it for everything it can see. Two things it cannot see are yours to raise:
/// calling [`activate`](crate::context::InputContextState::activate) or
/// [`deactivate`](crate::context::InputContextState::deactivate) by hand, and changing
/// [`PromptDevice`]. A backend that owns the bindings elsewhere raises it when the player edits
/// them there.
#[derive(bevy_ecs::resource::Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PromptGeneration(pub u64);

impl PromptGeneration {
    /// Says that something a prompt reads has changed.
    pub fn invalidate(commands: &mut bevy_ecs::system::Commands<'_, '_>) {
        commands.queue(|world: &mut World| {
            // Inserted rather than incremented in place: a hook fires on insert and not on a
            // mutable deref, and hooks are half of how this is read.
            let next = world
                .get_resource::<Self>()
                .map_or(0, |generation| generation.0)
                .wrapping_add(1);
            world.insert_resource(Self(next));
        });
    }
}

/// One context's bindings, flattened once its type is no longer known.
///
/// Read whole rather than filtered by action, because deciding whether a control still answers for
/// one action means knowing what every *other* action in the frame does with it.
#[derive(Default)]
pub(crate) struct ContextBindings {
    /// Whether anything is carrying this context and has it switched on.
    pub(crate) active: bool,
    /// One entry per control per binding, in declaration order.
    pub(crate) prompts: Vec<BoundControl>,
    /// The controls this context takes for itself when they fire, and what it takes them for.
    pub(crate) claims: Vec<(Control, ActionId)>,
}

/// One control of one binding, with what the binding requires alongside it.
pub(crate) struct BoundControl {
    pub(crate) action: ActionId,
    pub(crate) part: Part,
    pub(crate) control: Control,
    pub(crate) chord: Vec<Control>,
    pub(crate) condition: ConditionDescriptor,
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
        #[cfg(feature = "mouse")]
        for &button in ALL_MOUSE_BUTTONS {
            round_trip(Control::MouseButton(button));
        }
        round_trip(Control::MouseMotion);
        #[cfg(feature = "gamepad")]
        {
            round_trip(Control::GamepadButton(GamepadButton::Other(7)));
            round_trip(Control::GamepadAxis(GamepadAxis::Other(3)));
        }
        #[cfg(feature = "mouse")]
        round_trip(Control::MouseButton(MouseButton::Other(9)));
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
        #[cfg(feature = "mouse")]
        names.extend(
            ALL_MOUSE_BUTTONS
                .iter()
                .map(|&button| Control::MouseButton(button).name()),
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
        // `mouse/motion` and `mouse/Left` share a prefix and must not be confusable for each other.
        assert_eq!(Control::from_name("mouse/Motion"), None);
        assert_eq!(Control::from_name("mouse/left"), None);
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
        #[cfg(feature = "mouse")]
        for &button in ALL_MOUSE_BUTTONS {
            let label = Control::MouseButton(button).fallback_label();
            assert!(!label.is_empty(), "{button:?} has no label");
            assert_ne!(
                label, "Unknown Mouse Button",
                "{button:?} fell through the table"
            );
        }
    }

    /// The thumb buttons are shown the way a player's other games show them, and the stored name is
    /// Bevy's word for the same button — the two halves being different strings is the point.
    #[cfg(feature = "mouse")]
    #[test]
    fn a_mouse_button_is_stored_by_name_and_shown_by_convention() {
        let left = Control::MouseButton(MouseButton::Left);
        assert_eq!(left.name(), "mouse/Left");
        assert_eq!(left.fallback_label(), "Left Mouse");

        let back = Control::MouseButton(MouseButton::Back);
        assert_eq!(back.name(), "mouse/Back", "stored as what Bevy calls it");
        assert_eq!(back.fallback_label(), "Mouse 4", "shown as players say it");
        assert_eq!(
            Control::MouseButton(MouseButton::Forward).fallback_label(),
            "Mouse 5"
        );

        // Spelled out, so an unnamed button can never be mistaken for one of the two above.
        assert_eq!(
            Control::MouseButton(MouseButton::Other(4)).fallback_label(),
            "Mouse Button 4"
        );

        // A mouse button is keyboard-and-mouse and reports on a button channel, which is what lets
        // it fill a mapping a key could fill.
        assert_eq!(left.scheme(), crate::mapping::Scheme::KeyboardMouse);
        assert_eq!(left.shape(), crate::action::ChannelShape::Button);
    }
}

#[cfg(all(test, feature = "keyboard", feature = "gamepad"))]
mod prompt_tests {
    use super::*;

    use alloc::string::ToString;
    use alloc::vec;
    use bevy_app::App;
    use bevy_input::gamepad::GamepadButton;
    use bevy_input::keyboard::KeyCode;

    use crate::action::InputAction as _;
    use crate::binding::AxisButtons;
    use crate::context::{ActionMapAppExt, InputContextState};
    use crate::{ActionMapPlugin, InputAction, InputContext};

    #[derive(InputAction)]
    #[action(path = "prompt_tests.jump", output = bool, intent = Button)]
    struct Jump;

    #[derive(InputAction)]
    #[action(path = "prompt_tests.turn", output = f32, intent = Analog1)]
    struct Turn;

    #[derive(InputAction)]
    #[action(path = "prompt_tests.save", output = bool, intent = Button)]
    struct Save;

    #[derive(InputContext)]
    #[context(path = "prompt_tests.shell", tick = Render)]
    struct Shell;

    #[derive(InputContext)]
    #[context(path = "prompt_tests.flying", tick = Fixed)]
    struct Flying;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app
    }

    /// What the labels come out as, which is what a caption is made of.
    fn labels(prompts: &[Prompt]) -> alloc::vec::Vec<alloc::string::String> {
        prompts
            .iter()
            .map(|prompt| prompt.origin.fallback_label().to_string())
            .collect()
    }

    /// The lookup itself: name an action, get back the controls that fire it. Nothing in the caller
    /// says what those controls are, which is the whole point — a caption written this way stays
    /// true when somebody changes the binding.
    #[test]
    fn a_lookup_names_the_controls_that_fire_an_action() {
        let mut app = app();
        app.add_context::<Shell>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
            controls.bind::<Jump>(GamepadButton::South);
        });
        app.world_mut().spawn(Shell);

        let prompts = BindingTable::new(app.world()).prompts(Jump::id(), PromptScope::ANY);
        assert_eq!(labels(&prompts), ["Space", "South Button"]);
        assert_eq!(prompts[0].part, crate::binding::Part::Whole);
        assert_eq!(prompts[0].context, Some("prompt_tests.shell"));
        assert!(prompts[0].with.is_empty());
        // An origin carries the stored name as well as the readable one, so a game with a
        // catalogue looks up the first and falls back to the second.
        assert_eq!(prompts[0].origin.name(), "key/Space");
        assert_eq!(
            prompts[0].origin.control(),
            Some(Control::Key(KeyCode::Space))
        );
    }

    /// A prompt is a runtime question. A context nobody is carrying fires nothing, so naming its
    /// controls would tell the player to press a key that does nothing — which is the failure this
    /// answers, not a hole in it.
    #[test]
    fn a_context_nobody_carries_answers_nothing() {
        let mut app = app();
        app.add_context::<Shell>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
        });

        assert!(
            BindingTable::new(app.world())
                .prompts(Jump::id(), PromptScope::ANY)
                .is_empty()
        );
    }

    /// And the same for one that is carried and switched off, which is the ordinary state of a
    /// context gated on a game state the player is not in.
    #[test]
    fn a_context_that_is_switched_off_answers_nothing() {
        let mut app = app();
        app.add_context::<Shell>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
        });
        let entity = app.world_mut().spawn(Shell).id();

        assert_eq!(
            BindingTable::new(app.world())
                .prompts(Jump::id(), PromptScope::ANY)
                .len(),
            1
        );

        app.world_mut()
            .get_mut::<InputContextState<Shell>>(entity)
            .unwrap()
            .deactivate();
        assert!(
            BindingTable::new(app.world())
                .prompts(Jump::id(), PromptScope::ANY)
                .is_empty()
        );
    }

    /// The two narrowings a screen wants: one device's worth, and one context's.
    #[test]
    fn a_scope_narrows_to_one_device_and_one_context() {
        let mut app = app();
        app.add_context::<Shell>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
            controls.bind::<Jump>(GamepadButton::South);
        });
        app.add_context::<Flying>(|controls| {
            controls.bind::<Jump>(KeyCode::KeyJ);
        });
        app.world_mut().spawn((Shell, Flying));

        let table = BindingTable::new(app.world());
        assert_eq!(
            labels(&table.prompts(
                Jump::id(),
                PromptScope::ANY.on(crate::mapping::Scheme::Gamepad)
            )),
            ["South Button"]
        );
        assert_eq!(
            labels(&table.prompts(
                Jump::id(),
                PromptScope::ANY.in_context("prompt_tests.flying")
            )),
            ["J"]
        );
    }

    /// Ranking, such as it is: a render-tick context answers before a fixed-tick one because that
    /// is the order they get to claim a control in, and declaration order decides the rest.
    #[test]
    fn a_render_tick_context_answers_before_a_fixed_tick_one() {
        let mut app = app();
        // Declared the other way round, so the order below is the schedule's rather than this
        // file's.
        app.add_context::<Flying>(|controls| {
            controls.bind::<Jump>(KeyCode::KeyJ);
        });
        app.add_context::<Shell>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
        });
        app.world_mut().spawn((Shell, Flying));

        let prompts = BindingTable::new(app.world()).prompts(Jump::id(), PromptScope::ANY);
        assert_eq!(labels(&prompts), ["Space", "J"]);
    }

    /// A composite has no single control to name, so it answers once per direction — the same view
    /// the player-facing model takes, and what lets a caption say which key turns which way.
    #[test]
    fn a_composite_answers_once_per_direction() {
        use crate::binding::Part;

        let mut app = app();
        app.add_context::<Flying>(|controls| {
            controls.bind::<Turn>(AxisButtons::ad());
        });
        app.world_mut().spawn(Flying);

        let prompts = BindingTable::new(app.world()).prompts(Turn::id(), PromptScope::ANY);
        assert_eq!(labels(&prompts), ["A", "D"]);
        assert_eq!(prompts[0].part, Part::Negative);
        assert_eq!(prompts[1].part, Part::Positive);
    }

    /// What has to be held alongside travels with the control, because a prompt that dropped it
    /// would caption `Ctrl+S` as "S" — which is not an unpolished answer but a wrong one.
    #[test]
    fn a_chord_carries_what_must_be_held() {
        let mut app = app();
        app.add_context::<Shell>(|controls| {
            controls
                .bind::<Save>(KeyCode::KeyS)
                .with(KeyCode::ControlLeft);
        });
        app.world_mut().spawn(Shell);

        let prompts = BindingTable::new(app.world()).prompts(Save::id(), PromptScope::ANY);
        assert_eq!(labels(&prompts), ["S"]);
        assert_eq!(
            prompts[0].with,
            vec![ControlOrigin::Ours(Control::Key(KeyCode::ControlLeft))]
        );
    }

    /// R18.3's condition half: `Thrust` and `Afterburner` share a control and a prompt is the only
    /// place that would otherwise show them as identical.
    #[test]
    fn a_hold_travels_with_the_control_it_qualifies() {
        use crate::condition::ConditionDescriptor;

        let mut app = app();
        app.add_context::<Shell>(|controls| {
            controls.bind::<Turn>(KeyCode::KeyW);
            controls.bind::<Save>(KeyCode::KeyW).hold(0.75);
        });
        app.world_mut().spawn(Shell);

        let table = BindingTable::new(app.world());
        assert_eq!(
            table.prompts(Turn::id(), PromptScope::ANY)[0].condition,
            ConditionDescriptor::None
        );
        let held = table.prompts(Save::id(), PromptScope::ANY);
        assert_eq!(
            held[0].condition,
            ConditionDescriptor::Hold { duration: 0.75 }
        );
        assert_eq!(
            ConditionDescriptor::Hold { duration: 0.75 }
                .fallback_format(&held[0].origin.fallback_label()),
            "Hold W"
        );
    }

    /// R18.2's other half. A control a stronger context takes for something else does not fire this
    /// action, whatever the binding says, so a prompt naming it is a lie the player can check.
    #[test]
    fn a_stronger_context_taking_a_control_takes_it_out_of_the_prompt() {
        let mut app = app();
        // Render tick, so it claims first whatever the priorities say.
        app.add_context::<Shell>(|controls| {
            controls.bind::<Save>(KeyCode::Space).consume();
        });
        app.add_context::<Flying>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
            controls.bind::<Jump>(KeyCode::KeyJ);
        });
        let entity = app.world_mut().spawn((Shell, Flying)).id();

        let prompts = BindingTable::new(app.world()).prompts(Jump::id(), PromptScope::ANY);
        assert_eq!(labels(&prompts), ["J"], "space belongs to the shell");

        // Stand the claimant down and the control comes back, which is what makes this a live
        // answer rather than a fact about the binding tables.
        app.world_mut()
            .get_mut::<InputContextState<Shell>>(entity)
            .unwrap()
            .deactivate();
        let prompts = BindingTable::new(app.world()).prompts(Jump::id(), PromptScope::ANY);
        assert_eq!(labels(&prompts), ["Space", "J"]);
    }

    /// A prompt is not a row of the controls screen. `private` keeps a binding off that screen,
    /// because it reads a control already listed under another name — and it says nothing about
    /// whether the control fires the action, which is all a prompt is asking.
    #[test]
    fn a_private_binding_still_answers_a_prompt() {
        let mut app = app();
        app.add_context::<Shell>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).private();
        });
        app.world_mut().spawn(Shell);

        assert!(crate::mapping::mappings(app.world()).is_empty());
        assert_eq!(
            labels(&BindingTable::new(app.world()).prompts(Jump::id(), PromptScope::ANY)),
            ["Space"]
        );
    }

    /// One control reached twice is one prompt: what the player is told is which key to press, not
    /// how many places in the game are listening for it.
    #[test]
    fn one_control_bound_in_two_contexts_is_one_prompt() {
        let mut app = app();
        app.add_context::<Shell>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
        });
        app.add_context::<Flying>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
        });
        app.world_mut().spawn((Shell, Flying));

        assert_eq!(
            labels(&BindingTable::new(app.world()).prompts(Jump::id(), PromptScope::ANY)),
            ["Space"]
        );
    }

    /// The point of an origin being an enum. A backend authoritative for the bindings answers with
    /// its own enumeration of controls, covering devices this crate has no variant for, and a
    /// caption renders one without knowing where it came from.
    #[test]
    fn an_origin_from_somewhere_else_renders_like_one_of_ours() {
        let foreign = ControlOrigin::Foreign {
            name: "steam/dualsense_touchpad".into(),
            label: "Touchpad".into(),
            scheme: Some(crate::mapping::Scheme::Gamepad),
            class: Some(ControlClass::AnyDelta),
        };

        assert_eq!(foreign.name(), "steam/dualsense_touchpad");
        assert_eq!(foreign.fallback_label(), "Touchpad");
        assert_eq!(foreign.scheme(), Some(crate::mapping::Scheme::Gamepad));
        assert_eq!(foreign.class(), Some(ControlClass::AnyDelta));
        // And the one thing it cannot answer, so that a caller reaching past the name has to say
        // what it does when the control is not ours.
        assert_eq!(foreign.control(), None);
    }

    /// A backend that said nothing about the class is not the same as a control with no class, and
    /// the difference is load-bearing: a caller narrowing to buttons must not be handed something
    /// nobody has claimed is one.
    #[test]
    fn an_origin_that_never_said_what_it_was_has_no_class() {
        let unsaid = ControlOrigin::Foreign {
            name: "steam/mystery".into(),
            label: "Mystery".into(),
            scheme: None,
            class: None,
        };

        assert_eq!(unsaid.class(), None);
    }

    /// Nothing declared at all is not an error, and asking is not a panic.
    #[test]
    fn a_world_with_no_contexts_answers_nothing() {
        let app = app();
        assert!(
            BindingTable::new(app.world())
                .prompts(Jump::id(), PromptScope::ANY)
                .is_empty()
        );
    }

    /// The third narrowing, for a caller with room to draw a button and none to draw a stick. Both
    /// answers come out of the same binding list, and neither renders what the other does.
    #[test]
    fn a_scope_narrows_to_one_kind_of_signal() {
        use bevy_input::gamepad::GamepadAxis;

        let mut app = app();
        app.add_context::<Flying>(|controls| {
            controls.bind::<Turn>(AxisButtons::ad());
            controls.bind::<Turn>(GamepadAxis::LeftStickX);
        });
        app.world_mut().spawn(Flying);

        let table = BindingTable::new(app.world());
        assert_eq!(
            labels(&table.prompts(Turn::id(), PromptScope::ANY.of(ControlClass::AnyButton))),
            ["A", "D"]
        );

        let axes = table.prompts(Turn::id(), PromptScope::ANY.of(ControlClass::AnyAxis));
        assert_eq!(axes.len(), 1);
        assert_eq!(axes[0].origin.class(), Some(ControlClass::AnyAxis));
    }

    fn generation(app: &App) -> u64 {
        app.world()
            .get_resource::<PromptGeneration>()
            .map_or(0, |generation| generation.0)
    }

    /// The case no `activate` call covers: the answer is folded over every entity carrying the
    /// context, so it changes when the first one arrives and when the last one leaves.
    #[test]
    fn an_instance_arriving_or_leaving_says_prompts_may_have_changed() {
        let mut app = app();
        app.add_context::<Shell>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
        });

        let before = generation(&app);
        let entity = app.world_mut().spawn(Shell).id();
        app.update();
        let after_spawn = generation(&app);
        assert!(after_spawn > before, "spawning an instance said nothing");

        app.world_mut().entity_mut(entity).despawn();
        app.update();
        assert!(
            generation(&app) > after_spawn,
            "despawning the last instance said nothing"
        );
    }

    /// A context switching off empties every prompt that named its controls, so the edge has to be
    /// raised — once for the edge rather than once per instance.
    #[test]
    fn a_context_going_quiet_says_prompts_may_have_changed() {
        use bevy_ecs::prelude::{Component, resource_exists};

        #[derive(Component)]
        struct Flies;

        let mut app = app();
        app.add_context::<Flying>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
            controls.active_if(resource_exists::<Landed>);
        });
        app.world_mut().spawn((Flying, Flies));

        #[derive(bevy_ecs::resource::Resource)]
        struct Landed;

        app.update();
        app.insert_resource(Landed);
        app.update();
        let active = generation(&app);

        app.world_mut().remove_resource::<Landed>();
        app.update();
        assert!(generation(&app) > active, "deactivation said nothing");
    }

    /// The claim the touch is written as an insert to keep true: a resource is a component on an
    /// entity of its own, so a consumer can observe the signal instead of polling for it.
    #[test]
    fn the_signal_can_be_observed_rather_than_polled() {
        use bevy_ecs::lifecycle::Insert;
        use bevy_ecs::prelude::{On, ResMut};

        #[derive(bevy_ecs::resource::Resource, Default)]
        struct Heard(u32);

        let mut app = app();
        app.init_resource::<Heard>();
        app.add_observer(
            |_: On<Insert<PromptGeneration>>, mut heard: ResMut<'_, Heard>| {
                heard.0 += 1;
            },
        );
        app.add_context::<Shell>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
        });

        app.world_mut().spawn(Shell);
        app.update();

        assert!(app.world().resource::<Heard>().0 > 0);
    }
}
