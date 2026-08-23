//! The evaluator: a plan and an input frame in, action state and a transition log out.
//!
//! The evaluator resolves bindings and emits a transition log for later dispatch.

use bevy_ecs::component::Component;
use bevy_ecs::prelude::{Query, Res};
#[cfg(feature = "gamepad")]
use bevy_input::gamepad::{GamepadAxis, RawGamepadEvent};
#[cfg(feature = "keyboard")]
use bevy_input::{ButtonState, keyboard::KeyboardInput};
use bevy_math::{Vec2, Vec3};

use crate::action::{ActionValue, InputContext, Intent, Phase};
#[cfg(any(feature = "keyboard", feature = "gamepad"))]
use crate::binding::ButtonControl;
#[cfg(feature = "gamepad")]
use crate::binding::Stick;
use crate::binding::{BindingSource, ButtonThreshold};
use crate::context::InputContextState;
use crate::frame::{InputFrame, RawEvent};

/// Applies the current input frame to every instance of one context.
pub fn evaluate_context<C: InputContext + Component>(
    frame: Res<'_, InputFrame>,
    threshold: Res<'_, ButtonThreshold>,
    mut states: Query<'_, '_, &mut InputContextState<C>>,
) {
    for mut state in &mut states {
        state.apply_frame(&frame, &threshold);
    }
}

impl<C: InputContext> InputContextState<C> {
    pub(crate) fn apply_frame(&mut self, frame: &InputFrame, threshold: &ButtonThreshold) {
        let mut mouse_delta = Vec2::ZERO;

        // Only what has arrived since this context last looked. Re-reading the whole queue is what
        // made one mouse delta count three times across three fixed ticks.
        let unread = frame.events_after(self.read_through);
        if let Some(last) = unread.last() {
            self.read_through = Some(last.timestamp);
        }

        for event in unread {
            match &event.event {
                #[cfg(feature = "keyboard")]
                RawEvent::Keyboard(KeyboardInput {
                    key_code, state, ..
                }) => match state {
                    ButtonState::Pressed => {
                        self.held_buttons.insert(*key_code);
                    }
                    ButtonState::Released => {
                        self.held_buttons.remove(key_code);
                    }
                },
                RawEvent::MouseMotion(delta) => {
                    mouse_delta += *delta;
                }
                #[cfg(feature = "gamepad")]
                RawEvent::Gamepad(event) => match event {
                    RawGamepadEvent::Axis(raw_axis) => {
                        self.held_gamepad_axes.insert(raw_axis.axis, raw_axis.value);
                    }
                    RawGamepadEvent::Button(raw_button) => {
                        // Our own threshold, deliberately ignoring whatever press or release the
                        // backend synthesized at a threshold of its own (R14.2).
                        let reading = self
                            .held_gamepad_buttons
                            .entry(raw_button.button)
                            .or_default();
                        reading.pressed = threshold.pressed(raw_button.value, reading.pressed);
                        reading.value = raw_button.value;
                    }
                    RawGamepadEvent::Connection(_) => {}
                },
            }
        }

        // Field-level borrows: the fold reads the device state and the plan while writing actions.
        let Self {
            plan,
            actions,
            #[cfg(feature = "keyboard")]
            held_buttons,
            #[cfg(feature = "gamepad")]
            held_gamepad_buttons,
            #[cfg(feature = "gamepad")]
            held_gamepad_axes,
            ..
        } = self;

        // One predicate for every button-shaped part, so a composite and a plain button binding
        // can never disagree about what "pressed" means.
        #[cfg(any(feature = "keyboard", feature = "gamepad"))]
        let is_pressed = |control: ButtonControl| match control {
            #[cfg(feature = "keyboard")]
            ButtonControl::Key(key) => held_buttons.contains(&key),
            #[cfg(feature = "gamepad")]
            ButtonControl::GamepadButton(button) => held_gamepad_buttons
                .get(&button)
                .is_some_and(|reading| reading.pressed),
        };

        let bindings = plan.bindings();
        let mut index = 0;
        while index < bindings.len() {
            let slot = bindings[index].slot;
            let intent = plan.intent_for_slot(slot);
            let mut combined = None;

            // Bindings are grouped by slot, so this inner walk is one action's contributions.
            while index < bindings.len() && bindings[index].slot == slot {
                let binding = &bindings[index];
                let value = match binding.source {
                    #[cfg(feature = "keyboard")]
                    BindingSource::Button(key_code) => {
                        ActionValue::Bool(held_buttons.contains(&key_code))
                    }
                    #[cfg(any(feature = "keyboard", feature = "gamepad"))]
                    BindingSource::Directional2(parts) => {
                        // Four keys and a D-pad reach an action through this same arm, which is
                        // the whole point of the composite.
                        let x = axis_from_buttons(is_pressed(parts.left), is_pressed(parts.right));
                        let y = axis_from_buttons(is_pressed(parts.down), is_pressed(parts.up));
                        ActionValue::Axis2(Vec2::new(x, y))
                    }
                    BindingSource::MouseMotion => ActionValue::Axis2(mouse_delta),
                    // Both views of a button channel, chosen by what the action asked for. A
                    // trigger carries a fraction, so an analog action gets the travel and a button
                    // action gets the thresholded press — R2.10's case, and the reason a binding
                    // cannot be resolved from the source alone.
                    #[cfg(feature = "gamepad")]
                    BindingSource::GamepadButton(button) => {
                        let reading = held_gamepad_buttons
                            .get(&button)
                            .copied()
                            .unwrap_or_default();
                        match intent {
                            Intent::Button => ActionValue::Bool(reading.pressed),
                            _ => ActionValue::Axis1(reading.value),
                        }
                    }
                    #[cfg(feature = "gamepad")]
                    BindingSource::GamepadAxis(axis) => {
                        ActionValue::Axis1(held_gamepad_axes.get(&axis).copied().unwrap_or(0.0))
                    }
                    #[cfg(feature = "gamepad")]
                    BindingSource::GamepadStick(stick) => {
                        ActionValue::Axis2(gamepad_stick_value(held_gamepad_axes, stick))
                    }
                };

                let value = apply_modifiers(value, &binding.modifiers);
                // Where a press comes from something that was not already a press, the threshold
                // has to settle it here. Reading it later cannot: by then the only question a
                // stored value can answer is whether it is off centre, and a resting stick always
                // is. Modifiers run first so that a deadzone gets to define centre.
                let value = match (intent, value) {
                    (Intent::Button, ActionValue::Bool(_)) => value,
                    (Intent::Button, _) => {
                        ActionValue::Bool(threshold.pressed(magnitude(value), false))
                    }
                    _ => value,
                };
                combined = Some(match combined {
                    Some(previous) => combine(previous, value, intent),
                    None => value,
                });
                index += 1;
            }

            if let Some(value) = combined {
                update_action_state(&mut actions[slot], value);
            }
        }
    }
}

/// Folds one more binding's contribution into an action's value.
///
/// A delta is a displacement, so two of them add. Everything else is a position or a press, where
/// adding would be a units error: the strongest contribution wins instead, and ties keep the
/// earlier one so that declaration order decides.
fn combine(accumulated: ActionValue, contribution: ActionValue, intent: Intent) -> ActionValue {
    match intent {
        Intent::Delta2 => sum(accumulated, contribution),
        Intent::Button | Intent::Analog1 | Intent::Directional2 => {
            if magnitude(contribution) > magnitude(accumulated) {
                contribution
            } else {
                accumulated
            }
        }
    }
}

/// How strong a contribution is, for deciding which of two wins.
// Not `to_axis1`, which keeps the sign: pushing a stick left is as strong as pushing it right, and
// a comparison that thought otherwise would let the weaker of two bindings win.
fn magnitude(value: ActionValue) -> f32 {
    value.to_axis1().abs()
}

/// Adds two contributions, widening to whichever shape carries more components.
fn sum(accumulated: ActionValue, contribution: ActionValue) -> ActionValue {
    let total = widen(accumulated) + widen(contribution);
    match rank(accumulated).max(rank(contribution)) {
        0 => ActionValue::Bool(total != Vec3::ZERO),
        1 => ActionValue::Axis1(total.x),
        2 => ActionValue::Axis2(total.truncate()),
        _ => ActionValue::Axis3(total),
    }
}

fn rank(value: ActionValue) -> u8 {
    match value {
        ActionValue::Bool(_) => 0,
        ActionValue::Axis1(_) => 1,
        ActionValue::Axis2(_) => 2,
        ActionValue::Axis3(_) => 3,
    }
}

fn widen(value: ActionValue) -> Vec3 {
    value.to_axis3()
}

#[cfg(any(feature = "keyboard", feature = "gamepad"))]
fn axis_from_buttons(negative: bool, positive: bool) -> f32 {
    match (negative, positive) {
        (true, false) => -1.0,
        (false, true) => 1.0,
        _ => 0.0,
    }
}

fn apply_modifiers(
    mut value: ActionValue,
    modifiers: &[crate::binding::BindingModifier],
) -> ActionValue {
    for modifier in modifiers {
        value = modifier.apply(value);
    }
    value
}

fn update_action_state(action_state: &mut crate::action::ActionState, value: ActionValue) {
    let previous = action_state.value;
    action_state.value = value;
    action_state.phase = match (previous, value) {
        (ActionValue::Bool(false), ActionValue::Bool(false)) => Phase::Idle,
        (ActionValue::Bool(false), ActionValue::Bool(true)) => Phase::Fired,
        (ActionValue::Bool(true), ActionValue::Bool(true)) => Phase::Ongoing,
        (ActionValue::Bool(true), ActionValue::Bool(false)) => Phase::Completed,
        (ActionValue::Axis1(previous), ActionValue::Axis1(value)) => {
            match (previous == 0.0, value == 0.0) {
                (true, true) => Phase::Idle,
                (true, false) => Phase::Fired,
                (false, false) => Phase::Ongoing,
                (false, true) => Phase::Completed,
            }
        }
        (ActionValue::Axis2(previous), ActionValue::Axis2(value)) => {
            match (previous == Vec2::ZERO, value == Vec2::ZERO) {
                (true, true) => Phase::Idle,
                (true, false) => Phase::Fired,
                (false, false) => Phase::Ongoing,
                (false, true) => Phase::Completed,
            }
        }
        (ActionValue::Axis3(previous), ActionValue::Axis3(value)) => match (
            previous == bevy_math::Vec3::ZERO,
            value == bevy_math::Vec3::ZERO,
        ) {
            (true, true) => Phase::Idle,
            (true, false) => Phase::Fired,
            (false, false) => Phase::Ongoing,
            (false, true) => Phase::Completed,
        },
        (_, ActionValue::Bool(true)) => Phase::Fired,
        (_, ActionValue::Bool(false)) => Phase::Idle,
        (_, ActionValue::Axis1(value)) if value != 0.0 => Phase::Fired,
        (_, ActionValue::Axis1(_)) => Phase::Idle,
        (_, ActionValue::Axis2(value)) if value != Vec2::ZERO => Phase::Fired,
        (_, ActionValue::Axis2(_)) => Phase::Idle,
        (_, ActionValue::Axis3(value)) if value != bevy_math::Vec3::ZERO => Phase::Fired,
        (_, ActionValue::Axis3(_)) => Phase::Idle,
    };
}

#[cfg(feature = "gamepad")]
fn gamepad_stick_value(
    axes: &bevy_platform::collections::HashMap<GamepadAxis, f32>,
    stick: Stick,
) -> Vec2 {
    let (x_axis, y_axis) = match stick {
        Stick::Left => (GamepadAxis::LeftStickX, GamepadAxis::LeftStickY),
        Stick::Right => (GamepadAxis::RightStickX, GamepadAxis::RightStickY),
    };
    Vec2::new(
        axes.get(&x_axis).copied().unwrap_or(0.0),
        axes.get(&y_axis).copied().unwrap_or(0.0),
    )
}
