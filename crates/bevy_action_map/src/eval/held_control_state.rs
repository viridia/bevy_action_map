//! What one context instance knows the player is holding.

#[cfg(feature = "keyboard")]
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

#[cfg(any(feature = "keyboard", feature = "mouse"))]
use bevy_input::ButtonState;
#[cfg(feature = "gamepad")]
use bevy_input::gamepad::{GamepadAxis, GamepadButton, GamepadConnection, RawGamepadEvent};
#[cfg(feature = "keyboard")]
use bevy_input::keyboard::KeyboardInput;
#[cfg(feature = "mouse")]
use bevy_input::mouse::MouseButtonInput;
use bevy_math::Vec2;
#[cfg(feature = "gamepad")]
use bevy_platform::collections::HashMap;
#[cfg(feature = "mouse")]
use bevy_platform::collections::HashSet;

use crate::action::ActionId;
use crate::backend::AuthorityValues;
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
use crate::binding::ButtonControl;
use crate::binding::{ButtonThreshold, Control};
use crate::frame::RawEvent;

/// Both views of one held button-shaped control.
// A trigger has an analog position and a pressed sense, and the two are not derivable from each
// other on demand: `pressed` is hysteretic, so it depends on what it was last time this control
// was seen. Keeping it beside the value is what lets the button view be settled once per event
// rather than recomputed per binding, and is why two bindings on one trigger cannot disagree.
#[cfg(feature = "gamepad")]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct HeldButton {
    pub(crate) value: f32,
    pub(crate) pressed: bool,
}

/// The state of every control a context instance has heard about, whoever has claimed it, and of
/// the two sources that are not controls: the mouse motion summed this tick, and what the authority
/// supplied at its start.
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
    gamepad_buttons: HashMap<GamepadButton, HeldButton>,
    #[cfg(feature = "gamepad")]
    gamepad_axes: HashMap<GamepadAxis, f32>,
    // Summed rather than held, because half of a movement is not a position; read once, at the
    // tick's closing step.
    pub(crate) mouse_motion: Vec2,
    // What the entity's `AuthorityValues` said at the start of this tick, held like a control's
    // state so a binding reads either the same way. Refreshed only while active, since nothing
    // reads it otherwise.
    pub(crate) authority: AuthorityValues,
    // Controls that were down when focus loss or a disconnect took them, and authority sources that
    // stopped being supplied while held, kept for the rest of that tick. Held state alone cannot
    // tell these from a release, and D94 needs the difference. A control at rest is never here.
    withdrawn: Vec<Control>,
    withdrawn_sources: Vec<ActionId>,
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
    /// Moves one control's held state, for the inputs that have a state to hold, and calls
    /// `changed` with every control whose state the event may have moved.
    // Mouse motion, the one event left with every device feature off, moves no held state.
    #[cfg_attr(
        not(any(feature = "keyboard", feature = "mouse", feature = "gamepad")),
        allow(unused_mut, unused_variables)
    )]
    pub(crate) fn apply_event(
        &mut self,
        event: &RawEvent,
        threshold: &ButtonThreshold,
        mut changed: impl FnMut(Control),
    ) {
        #[cfg(not(feature = "gamepad"))]
        let _ = threshold;

        match event {
            // A key reports its logical character too, taken from the press it releases, since
            // shift may have changed what the platform reports in between.
            #[cfg(feature = "keyboard")]
            RawEvent::Keyboard(KeyboardInput {
                key_code,
                logical_key,
                state,
                ..
            }) => {
                let character = match state {
                    ButtonState::Pressed => {
                        self.keys.insert(*key_code);
                        let character = bound_character(logical_key);
                        if let Some(character) = character {
                            self.characters.insert(*key_code, character);
                        }
                        character
                    }
                    ButtonState::Released => {
                        self.keys.remove(key_code);
                        self.characters.remove(key_code)
                    }
                };
                self.touch(Control::PhysicalKey(*key_code), &mut changed);
                if let Some(character) = character {
                    self.touch(Control::LogicalKey(character), &mut changed);
                }
            }
            #[cfg(feature = "mouse")]
            RawEvent::MouseButton(MouseButtonInput { button, state, .. }) => {
                match state {
                    ButtonState::Pressed => {
                        self.mouse_buttons.insert(*button);
                    }
                    ButtonState::Released => {
                        self.mouse_buttons.remove(button);
                    }
                }
                self.touch(Control::MouseButton(*button), &mut changed);
            }
            // Summed and not reported: a motion binding reads the tick's total once, at the closing
            // step, rather than a reading per event.
            RawEvent::MouseMotion(delta) => self.mouse_motion += *delta,
            #[cfg(feature = "gamepad")]
            RawEvent::Gamepad(event) => match event {
                RawGamepadEvent::Axis(raw_axis) => {
                    self.gamepad_axes.insert(raw_axis.axis, raw_axis.value);
                    self.touch(Control::GamepadAxis(raw_axis.axis), &mut changed);
                }
                RawGamepadEvent::Button(raw_button) => {
                    // Our own threshold, deliberately ignoring whatever press or release the
                    // backend synthesized at a threshold of its own (R14.2).
                    let held_button = self.gamepad_buttons.entry(raw_button.button).or_default();
                    held_button.pressed = threshold.pressed(raw_button.value, held_button.pressed);
                    held_button.value = raw_button.value;
                    self.touch(Control::GamepadButton(raw_button.button), &mut changed);
                }
                // A disconnect leaves no release event to clear a button still recorded down — the
                // backend has nothing left to send one from (R11.4). Connecting needs nothing: the
                // device's own events repopulate these maps normally.
                RawGamepadEvent::Connection(connection) => {
                    if matches!(connection.connection, GamepadConnection::Disconnected) {
                        for (&button, held_button) in &self.gamepad_buttons {
                            if held_button.value != 0.0 {
                                self.withdrawn.push(Control::GamepadButton(button));
                                changed(Control::GamepadButton(button));
                            }
                        }
                        for (&axis, &value) in &self.gamepad_axes {
                            if value != 0.0 {
                                self.withdrawn.push(Control::GamepadAxis(axis));
                                changed(Control::GamepadAxis(axis));
                            }
                        }
                        self.gamepad_buttons.clear();
                        self.gamepad_axes.clear();
                    }
                }
            },
            #[cfg(any(feature = "keyboard", feature = "mouse"))]
            RawEvent::FocusLost => {
                #[cfg(feature = "keyboard")]
                {
                    for &key in &self.keys {
                        self.withdrawn.push(Control::PhysicalKey(key));
                        changed(Control::PhysicalKey(key));
                    }
                    for &character in self.characters.values() {
                        self.withdrawn.push(Control::LogicalKey(character));
                        changed(Control::LogicalKey(character));
                    }
                    self.keys.clear();
                    self.characters.clear();
                }
                #[cfg(feature = "mouse")]
                {
                    for &button in &self.mouse_buttons {
                        self.withdrawn.push(Control::MouseButton(button));
                        changed(Control::MouseButton(button));
                    }
                    self.mouse_buttons.clear();
                }
            }
        }
    }

    /// Reports a control an event named, which also ends its withdrawal: pressed again after focus
    /// came back, it is the player's own press.
    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    fn touch(&mut self, control: Control, changed: &mut impl FnMut(Control)) {
        self.withdrawn.retain(|&withdrawn| withdrawn != control);
        changed(control);
    }

    /// Whether a control was taken away while down this tick, rather than let go.
    pub(crate) fn is_withdrawn(&self, control: Control) -> bool {
        self.withdrawn.contains(&control)
    }

    /// Records an authority source that stopped being supplied while held.
    pub(crate) fn withdraw_source(&mut self, action: ActionId) {
        self.withdrawn_sources.push(action);
    }

    /// Whether an authority source stopped being supplied while held this tick, the authority's
    /// counterpart to `is_withdrawn`.
    pub(crate) fn is_source_withdrawn(&self, action: ActionId) -> bool {
        self.withdrawn_sources.contains(&action)
    }

    /// Forgets what lasts only a tick: the motion summed across it and what was withdrawn in it.
    /// Returns whether anything was withdrawn, whose bindings' readings this moves.
    pub(crate) fn end_tick(&mut self) -> bool {
        self.mouse_motion = Vec2::ZERO;
        let withdrawn = !self.withdrawn.is_empty() || !self.withdrawn_sources.is_empty();
        self.withdrawn.clear();
        self.withdrawn_sources.clear();
        withdrawn
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
                .is_some_and(|held| held.pressed),
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
                .is_some_and(|held| held.pressed),
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
                .is_some_and(|held| held.value != 0.0),
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
            .map_or(0.0, |held| held.value)
    }

    #[cfg(feature = "gamepad")]
    pub(crate) fn gamepad_axis_value(&self, axis: GamepadAxis) -> f32 {
        self.gamepad_axes.get(&axis).copied().unwrap_or(0.0)
    }
}
