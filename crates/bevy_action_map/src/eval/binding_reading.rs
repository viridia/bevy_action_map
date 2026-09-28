//! What one binding's input shows at one moment, before any stateful stage sees it.

use bevy_math::Vec2;
use fixedbitset::FixedBitSet;

use super::{ConsumedControls, HeldControlState};
use crate::action::{ActionIntent, ActionValue};
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
use crate::binding::ButtonControl;
#[cfg(feature = "gamepad")]
use crate::binding::Stick;
use crate::binding::{BindingInput, Control};
use crate::device::DeviceHandleSet;
use crate::plan::{CompiledBinding, Plan};

/// Whether a binding's reading counts, and if it does not, why.
// The chord variants are unbuilt without device features, where no binding has a chord.
#[cfg_attr(
    not(any(feature = "keyboard", feature = "mouse", feature = "gamepad")),
    allow(dead_code)
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadingAvailability {
    /// The control as the player holds it. The reading counts.
    Live,
    /// The chord this binding needs is not held. Reads as rest.
    HeldBackByChord,
    /// A longer chord on a control this binding shares is held (R8.1). Reads as rest.
    OutrankedByLongerChord,
    /// A higher-priority context grabbed the control while it was down (TD5.2).
    ClaimedWhileDown,
    /// The control's source went away while it was down: focus loss, a disconnect, or an authority
    /// no longer supplying the action.
    Withdrawn,
}

/// What one binding's input shows: a pure function of held state, the claims against it and the
/// plan. A binding's pipeline runs once each time this changes within a tick, so it has to be cheap
/// to compute and compare.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BindingReading {
    /// What the input reads, a control another context has claimed counting as untouched; rest
    /// where the chord holds the binding back or a longer chord out-ranks it.
    pub(crate) value: ActionValue,
    /// Whether `value` counts, and if it does not, why.
    pub(crate) availability: ReadingAvailability,
}

// What a binding holds before its first refresh, which replaces it before anything runs.
impl Default for BindingReading {
    fn default() -> Self {
        Self {
            value: ActionValue::Bool(false),
            availability: ReadingAvailability::Live,
        }
    }
}

impl BindingReading {
    /// Whether the control went away with the player still holding it, which ends what it was
    /// firing as `Canceled` rather than `Completed` (D94).
    pub(crate) fn interrupted(self) -> bool {
        matches!(
            self.availability,
            ReadingAvailability::ClaimedWhileDown | ReadingAvailability::Withdrawn
        )
    }
}

/// Reads one binding against the current device state: first whether its reading counts, then its
/// value.
///
/// `disabled` matters only for the binding's rivals, since a disabled action's longer chord
/// out-ranks nothing.
pub(crate) fn read_binding(
    plan: &Plan,
    binding: &CompiledBinding,
    held: &HeldControlState,
    consumed: &ConsumedControls,
    devices: Option<&DeviceHandleSet>,
    disabled: &FixedBitSet,
) -> BindingReading {
    let availability = availability(plan, binding, held, consumed, devices, disabled);
    let value = match availability {
        ReadingAvailability::HeldBackByChord | ReadingAvailability::OutrankedByLongerChord => {
            ActionValue::Bool(false)
        }
        _ => read_value(
            &binding.input,
            plan.intent_for_slot(binding.slot),
            held,
            consumed,
            devices,
        ),
    };
    BindingReading {
        value,
        availability,
    }
}

/// Decides whether a binding's reading counts. The first that applies wins, in this order:
/// withdrawn, held back by its chord, out-ranked by a longer chord, claimed while down, live.
// Withdrawal is decided first: focus loss releases a chord's modifier along with its key, and the
// binding has to read as its key going away rather than as its chord coming undone.
fn availability(
    plan: &Plan,
    binding: &CompiledBinding,
    held: &HeldControlState,
    consumed: &ConsumedControls,
    devices: Option<&DeviceHandleSet>,
    disabled: &FixedBitSet,
) -> ReadingAvailability {
    if input_withdrawn(&binding.input, held) {
        return ReadingAvailability::Withdrawn;
    }

    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    {
        if !chord_held(&binding.chord, held, consumed, devices) {
            return ReadingAvailability::HeldBackByChord;
        }
        if plan
            .rivals(binding)
            .any(|rival| !disabled[rival.slot] && chord_held(&rival.chord, held, consumed, devices))
        {
            return ReadingAvailability::OutrankedByLongerChord;
        }
    }
    #[cfg(not(any(feature = "keyboard", feature = "mouse", feature = "gamepad")))]
    let _ = (plan, disabled);

    // Checked after the chord: a binding its chord holds back reads rest whether its control is
    // claimed or not, so a claim costs it nothing and must not mark it `ClaimedWhileDown` (TD5.2).
    if input_claimed_while_down(&binding.input, held, consumed, devices) {
        return ReadingAvailability::ClaimedWhileDown;
    }
    ReadingAvailability::Live
}

// The three questions `availability` asks of the controls, in the order it asks them. Two walk the
// input's controls and ask whether any qualifies; the chord's asks whether every entry is held.

/// Whether any control the input reads was taken away while down this tick, by focus loss or a
/// disconnect. That is one control for a key and two for a stick's axes.
///
/// An authority binding holds no control; it is withdrawn when its source stops being supplied
/// while held.
fn input_withdrawn(input: &BindingInput, held: &HeldControlState) -> bool {
    if let BindingInput::Authority(_, _, source) = *input {
        return held.is_source_withdrawn(source);
    }
    let mut withdrawn = false;
    input.for_each_control(|control| withdrawn |= held.is_withdrawn(control));
    withdrawn
}

/// Whether every entry of a chord is held. A control another context has grabbed reads as
/// untouched.
// A modifier entry, `Ctrl` say, is satisfied by either key of its pair. It is resolved here rather
// than expanded at bind time into one entry per side, because both keys can be down at once
// (TD5.1).
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
pub(crate) fn chord_held(
    chord: &[crate::binding::ChordEntry],
    held: &HeldControlState,
    consumed: &ConsumedControls,
    devices: Option<&DeviceHandleSet>,
) -> bool {
    let is_pressed = |control: ButtonControl| {
        !consumed.contains(control.into(), devices) && held.is_pressed(control)
    };
    chord.iter().all(|&entry| match entry {
        crate::binding::ChordEntry::Control(control) => is_pressed(control),
        #[cfg(feature = "keyboard")]
        crate::binding::ChordEntry::Modifier(modifier) => modifier
            .keys()
            .into_iter()
            .any(|key| is_pressed(ButtonControl::PhysicalKey(key))),
    })
}

/// Whether another context has claimed any control the input reads while the player is still
/// holding it. Skipped outright when nothing is claimed, which is the common case.
fn input_claimed_while_down(
    input: &BindingInput,
    held: &HeldControlState,
    consumed: &ConsumedControls,
    devices: Option<&DeviceHandleSet>,
) -> bool {
    if consumed.is_empty() {
        return false;
    }
    let mut claimed = false;
    input.for_each_control(|control| {
        claimed |= consumed.contains(control, devices) && held.is_down(control);
    });
    claimed
}

/// The value one input reads. A control another context has grabbed counts as untouched rather than
/// being skipped: the binding still runs, so a hold on it is told its control let go.
fn read_value(
    input: &BindingInput,
    intent: ActionIntent,
    held: &HeldControlState,
    consumed: &ConsumedControls,
    devices: Option<&DeviceHandleSet>,
) -> ActionValue {
    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    let is_pressed = |control: ButtonControl| {
        !consumed.contains(control.into(), devices) && held.is_pressed(control)
    };

    match *input {
        #[cfg(feature = "keyboard")]
        BindingInput::Button(key_code) => {
            ActionValue::Bool(is_pressed(ButtonControl::PhysicalKey(key_code)))
        }
        #[cfg(feature = "keyboard")]
        BindingInput::LogicalKey(character) => {
            ActionValue::Bool(is_pressed(ButtonControl::LogicalKey(character)))
        }
        #[cfg(feature = "mouse")]
        BindingInput::MouseButton(button) => {
            ActionValue::Bool(is_pressed(ButtonControl::MouseButton(button)))
        }
        // Four keys and a D-pad reach an action through this same arm, and combining is what turns
        // their parts back into one direction.
        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        BindingInput::Part(button, part) => part_value(part, is_pressed(button)),
        BindingInput::MouseMotion => {
            ActionValue::Axis2(if consumed.contains(Control::MouseMotion, devices) {
                Vec2::ZERO
            } else {
                held.mouse_motion
            })
        }
        // Both views of a button channel, chosen by what the action asked for. A trigger carries a
        // fraction, so an analog action gets the travel and a button action gets the thresholded
        // press — R2.10's case, and the reason a binding cannot be resolved from the input alone.
        #[cfg(feature = "gamepad")]
        BindingInput::GamepadButton(button) => match intent {
            ActionIntent::Button => {
                ActionValue::Bool(is_pressed(ButtonControl::GamepadButton(button)))
            }
            _ => ActionValue::Axis1(
                if consumed.contains(Control::GamepadButton(button), devices) {
                    0.0
                } else {
                    held.gamepad_button_value(button)
                },
            ),
        },
        #[cfg(feature = "gamepad")]
        BindingInput::GamepadAxis(axis) => {
            ActionValue::Axis1(if consumed.contains(Control::GamepadAxis(axis), devices) {
                0.0
            } else {
                held.gamepad_axis_value(axis)
            })
        }
        #[cfg(feature = "gamepad")]
        BindingInput::GamepadStick(stick) => {
            ActionValue::Axis2(gamepad_stick_value(held, stick, consumed, devices))
        }
        // `source` is the action whose value the authority supplies: the binding's own, or, for a
        // follower, which rides another action's mapping and reads what it reads, that leader's.
        BindingInput::Authority(_, _, source) => held
            .authority
            .value_of(source)
            .unwrap_or_else(|| at_rest(intent)),
    }
}

/// What an action of this intent reads when nothing is driving it.
pub(super) fn at_rest(intent: ActionIntent) -> ActionValue {
    match intent {
        ActionIntent::Button => ActionValue::Bool(false),
        ActionIntent::Analog1 => ActionValue::Axis1(0.0),
        ActionIntent::Directional2 | ActionIntent::Delta2 => ActionValue::Axis2(Vec2::ZERO),
    }
}

/// What one part of a composite contributes: a unit push its own way while its button is pressed,
/// and rest otherwise. `WASD` is four parts, and combining their pushes gives the direction.
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
fn part_value(part: crate::binding::BindingPart, pressed: bool) -> ActionValue {
    use crate::binding::BindingPart;
    match (part, pressed) {
        (BindingPart::Negative | BindingPart::Positive, false) => ActionValue::Axis1(0.0),
        (BindingPart::Negative, true) => ActionValue::Axis1(-1.0),
        (BindingPart::Positive, true) => ActionValue::Axis1(1.0),
        (_, false) => ActionValue::Axis2(Vec2::ZERO),
        (BindingPart::Up, true) => ActionValue::Axis2(Vec2::Y),
        (BindingPart::Down, true) => ActionValue::Axis2(Vec2::NEG_Y),
        (BindingPart::Left, true) => ActionValue::Axis2(Vec2::NEG_X),
        (BindingPart::Right, true) => ActionValue::Axis2(Vec2::X),
        // Expansion never builds one.
        (BindingPart::Whole, true) => ActionValue::Bool(false),
    }
}

/// A stick's two axes, each reading zero where it is claimed, so a claim on one axis leaves the
/// other.
#[cfg(feature = "gamepad")]
fn gamepad_stick_value(
    held: &HeldControlState,
    stick: Stick,
    consumed: &ConsumedControls,
    devices: Option<&DeviceHandleSet>,
) -> Vec2 {
    let read = |axis| {
        if consumed.contains(Control::GamepadAxis(axis), devices) {
            0.0
        } else {
            held.gamepad_axis_value(axis)
        }
    };
    let (x_axis, y_axis) = stick.axes();
    Vec2::new(read(x_axis), read(y_axis))
}
