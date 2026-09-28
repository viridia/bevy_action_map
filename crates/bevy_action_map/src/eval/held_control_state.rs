//! What one context instance knows the player is holding.

#[cfg(feature = "keyboard")]
use alloc::collections::{BTreeMap, BTreeSet};

#[cfg(any(feature = "keyboard", feature = "mouse"))]
use bevy_input::ButtonState;
#[cfg(feature = "gamepad")]
use bevy_input::gamepad::{GamepadAxis, GamepadButton, GamepadConnection, RawGamepadEvent};
#[cfg(feature = "keyboard")]
use bevy_input::keyboard::KeyboardInput;
#[cfg(feature = "mouse")]
use bevy_input::mouse::MouseButtonInput;
#[cfg(feature = "gamepad")]
use bevy_platform::collections::HashMap;
#[cfg(feature = "mouse")]
use bevy_platform::collections::HashSet;

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
use crate::binding::ButtonControl;
use crate::binding::{ButtonThreshold, Control};
use crate::frame::RawEvent;

/// Both views of one button-shaped control.
// A trigger has an analog position and a pressed sense, and the two are not derivable from each
// other on demand: `pressed` is hysteretic, so it depends on what it was last time this control
// was seen. Keeping it beside the value is what lets the button view be settled once per event
// rather than recomputed per binding, and is why two bindings on one trigger cannot disagree.
#[cfg(feature = "gamepad")]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ButtonReading {
    pub(crate) value: f32,
    pub(crate) pressed: bool,
}

/// The state of every control a context instance has heard about, whoever has claimed it.
///
/// Three questions read it, and they differ in how much travel counts:
///
/// - `is_pressed`: a button-shaped control is past the press threshold. For a gamepad button that
///   is this crate's own hysteretic threshold; a key or a mouse button has no travel, so held is
///   pressed. What a button binding reads.
/// - `is_down`: the control has any travel at all, threshold or not, and axes and sticks count. A
///   trigger resting at 0.2 is down and not pressed. Asked only to tell a claimed control the
///   player is still holding from one let go.
/// - `actuated`: asked of an event rather than a control, once `apply_event` has taken it: whether
///   the control it names now reads as on. A gamepad button answers as pressed, an axis as down.
///   What class dispatch uses to fire on a press and not on a release.
#[derive(Default)]
pub(crate) struct HeldControlState {
    #[cfg(feature = "keyboard")]
    keys: BTreeSet<bevy_input::keyboard::KeyCode>,
    // The character each held key reported, for the logical bindings to read. Keyed by position
    // rather than by character so that a release always finds its press: holding a key and then
    // pressing shift changes the character the platform reports, and matching on that would leave
    // the entry stranded. Empty in the overwhelmingly common plan, which binds nothing logically.
    #[cfg(feature = "keyboard")]
    characters: BTreeMap<bevy_input::keyboard::KeyCode, char>,
    // A `HashSet` rather than the `BTreeSet` the keys get, because `MouseButton` is `Hash` but not
    // `Ord` upstream.
    #[cfg(feature = "mouse")]
    mouse_buttons: HashSet<bevy_input::mouse::MouseButton>,
    #[cfg(feature = "gamepad")]
    gamepad_buttons: HashMap<GamepadButton, ButtonReading>,
    #[cfg(feature = "gamepad")]
    gamepad_axes: HashMap<GamepadAxis, f32>,
}

/// The character a logical binding may name this key by, if it has one.
///
/// A key qualifies when it produces a character of its own. `Key::Character` carrying more than one
/// is a composition rather than a key — an IME committing several keystrokes at once, or a Windows
/// dead key that could not combine and reports both what it held and what followed (R12.6) — and
/// belongs to text entry, not to a binding. Dead keys themselves have produced nothing yet, and the
/// named variants are modifiers and the like, which sit in the same place on every layout and are
/// bound by position.
#[cfg(feature = "keyboard")]
fn bound_character(logical_key: &bevy_input::keyboard::Key) -> Option<char> {
    let bevy_input::keyboard::Key::Character(text) = logical_key else {
        return None;
    };
    crate::binding::single_character(text)
}

impl HeldControlState {
    /// Moves one control's held state, for the inputs that have a state to hold.
    pub(crate) fn apply_event(&mut self, event: &RawEvent, threshold: &ButtonThreshold) {
        #[cfg(not(feature = "gamepad"))]
        let _ = threshold;

        match event {
            #[cfg(feature = "keyboard")]
            RawEvent::Keyboard(KeyboardInput {
                key_code,
                logical_key,
                state,
                ..
            }) => match state {
                ButtonState::Pressed => {
                    self.keys.insert(*key_code);
                    if let Some(character) = bound_character(logical_key) {
                        self.characters.insert(*key_code, character);
                    }
                }
                ButtonState::Released => {
                    self.keys.remove(key_code);
                    self.characters.remove(key_code);
                }
            },
            #[cfg(feature = "mouse")]
            RawEvent::MouseButton(MouseButtonInput { button, state, .. }) => match state {
                ButtonState::Pressed => {
                    self.mouse_buttons.insert(*button);
                }
                ButtonState::Released => {
                    self.mouse_buttons.remove(button);
                }
            },
            // Accumulated by the caller: a delta is not a state.
            RawEvent::MouseMotion(_) => {}
            #[cfg(feature = "gamepad")]
            RawEvent::Gamepad(event) => match event {
                RawGamepadEvent::Axis(raw_axis) => {
                    self.gamepad_axes.insert(raw_axis.axis, raw_axis.value);
                }
                RawGamepadEvent::Button(raw_button) => {
                    // Our own threshold, deliberately ignoring whatever press or release the
                    // backend synthesized at a threshold of its own (R14.2).
                    let reading = self.gamepad_buttons.entry(raw_button.button).or_default();
                    reading.pressed = threshold.pressed(raw_button.value, reading.pressed);
                    reading.value = raw_button.value;
                }
                // A disconnect leaves no release event to correct a stale reading — the backend has
                // nothing left to send one from (R11.4). Connecting needs nothing: the device's own
                // events repopulate these maps normally.
                RawGamepadEvent::Connection(connection) => {
                    if matches!(connection.connection, GamepadConnection::Disconnected) {
                        self.gamepad_buttons.clear();
                        self.gamepad_axes.clear();
                    }
                }
            },
            #[cfg(any(feature = "keyboard", feature = "mouse"))]
            RawEvent::FocusLost => {
                #[cfg(feature = "keyboard")]
                {
                    self.keys.clear();
                    self.characters.clear();
                }
                #[cfg(feature = "mouse")]
                self.mouse_buttons.clear();
            }
        }
    }

    /// Whether the control this event names reads as on, once `apply_event` has taken the event.
    pub(crate) fn actuated(&self, event: &RawEvent) -> bool {
        match event {
            #[cfg(feature = "keyboard")]
            RawEvent::Keyboard(KeyboardInput { state, .. }) => *state == ButtonState::Pressed,
            #[cfg(feature = "mouse")]
            RawEvent::MouseButton(MouseButtonInput { state, .. }) => *state == ButtonState::Pressed,
            RawEvent::MouseMotion(_) => false,
            #[cfg(feature = "gamepad")]
            RawEvent::Gamepad(RawGamepadEvent::Button(raw_button)) => self
                .gamepad_buttons
                .get(&raw_button.button)
                .is_some_and(|reading| reading.pressed),
            #[cfg(feature = "gamepad")]
            RawEvent::Gamepad(RawGamepadEvent::Axis(raw_axis)) => raw_axis.value != 0.0,
            #[cfg(feature = "gamepad")]
            RawEvent::Gamepad(RawGamepadEvent::Connection(_)) => false,
            // Unreachable in practice: `control()` is `None` for this event, and `class_dispatch`
            // returns before ever asking. Kept for exhaustiveness, same as the arm above.
            #[cfg(any(feature = "keyboard", feature = "mouse"))]
            RawEvent::FocusLost => false,
        }
    }

    /// Whether a button-shaped control is past the press threshold. One predicate for every
    /// button-shaped part, so a composite's part and a plain button binding can never disagree
    /// about what "pressed" means.
    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    pub(crate) fn is_pressed(&self, control: ButtonControl) -> bool {
        match control {
            #[cfg(feature = "keyboard")]
            ButtonControl::PhysicalKey(key) => self.keys.contains(&key),
            #[cfg(feature = "keyboard")]
            ButtonControl::LogicalKey(character) => {
                self.characters.values().any(|&held| held == character)
            }
            #[cfg(feature = "mouse")]
            ButtonControl::MouseButton(button) => self.mouse_buttons.contains(&button),
            #[cfg(feature = "gamepad")]
            ButtonControl::GamepadButton(button) => self
                .gamepad_buttons
                .get(&button)
                .is_some_and(|reading| reading.pressed),
        }
    }

    /// Whether a control has any travel at all. Held and claimed is a control taken away rather
    /// than let go (D94).
    pub(crate) fn is_down(&self, control: Control) -> bool {
        match control {
            #[cfg(feature = "keyboard")]
            Control::PhysicalKey(key) => self.keys.contains(&key),
            #[cfg(feature = "keyboard")]
            Control::LogicalKey(character) => {
                self.characters.values().any(|&held| held == character)
            }
            #[cfg(feature = "mouse")]
            Control::MouseButton(button) => self.mouse_buttons.contains(&button),
            // Any travel, not the threshold: an analog action reads a trigger below it.
            #[cfg(feature = "gamepad")]
            Control::GamepadButton(button) => self
                .gamepad_buttons
                .get(&button)
                .is_some_and(|reading| reading.value != 0.0),
            #[cfg(feature = "gamepad")]
            Control::GamepadAxis(axis) => self.gamepad_axes.get(&axis).is_some_and(|&v| v != 0.0),
            #[cfg(feature = "gamepad")]
            Control::GamepadStick(stick) => {
                let (x, y) = stick.axes();
                [x, y]
                    .iter()
                    .any(|axis| self.gamepad_axes.get(axis).is_some_and(|&v| v != 0.0))
            }
            Control::MouseMotion => false,
        }
    }

    /// How far a gamepad button has travelled, before any threshold.
    #[cfg(feature = "gamepad")]
    pub(crate) fn gamepad_button_value(&self, button: GamepadButton) -> f32 {
        self.gamepad_buttons
            .get(&button)
            .map_or(0.0, |reading| reading.value)
    }

    #[cfg(feature = "gamepad")]
    pub(crate) fn gamepad_axis_value(&self, axis: GamepadAxis) -> f32 {
        self.gamepad_axes.get(&axis).copied().unwrap_or(0.0)
    }
}
