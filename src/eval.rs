//! The evaluator: a plan and an input frame in, action state and a transition log out.
//!
//! The evaluator resolves bindings and emits a transition log for later dispatch.

use alloc::vec;

use bevy_ecs::prelude::{Res, ResMut};
#[cfg(feature = "gamepad")]
use bevy_input::gamepad::{GamepadAxis, RawGamepadEvent};
use bevy_input::{ButtonState, keyboard::KeyboardInput};
use bevy_math::Vec2;

use crate::action::{ActionValue, InputContext, Phase};
use crate::binding::BindingSource;
#[cfg(feature = "gamepad")]
use crate::binding::Stick;
use crate::frame::{InputFrame, RawEvent};
use crate::player::ContextInstance;

/// Applies the current input frame to one context's state.
pub fn evaluate_context<C: InputContext>(
    frame: Res<'_, InputFrame>,
    mut state: ResMut<'_, ContextInstance<C>>,
) {
    state.apply_frame(&frame);
}

impl<C: InputContext> ContextInstance<C> {
    pub(crate) fn apply_frame(&mut self, frame: &InputFrame) {
        let mut matched = vec![false; self.actions.len()];
        let mut mouse_delta = Vec2::ZERO;

        for event in frame.events() {
            match &event.event {
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
                        self.held_gamepad_buttons
                            .insert(raw_button.button, raw_button.value);
                    }
                    RawGamepadEvent::Connection(_) => {}
                },
            }
        }

        for (slot, binding) in self.plan.bindings().iter().enumerate() {
            matched[slot] = true;
            let action_state = &mut self.actions[slot];
            let value = match binding.source {
                BindingSource::Button(key_code) => {
                    ActionValue::Bool(self.held_buttons.contains(&key_code))
                }
                BindingSource::Directional2(keys) => {
                    let x = axis_from_buttons(
                        self.held_buttons.contains(&keys.left),
                        self.held_buttons.contains(&keys.right),
                    );
                    let y = axis_from_buttons(
                        self.held_buttons.contains(&keys.down),
                        self.held_buttons.contains(&keys.up),
                    );
                    ActionValue::Axis2(Vec2::new(x, y))
                }
                BindingSource::MouseMotion => ActionValue::Axis2(mouse_delta),
                #[cfg(feature = "gamepad")]
                BindingSource::GamepadButton(button) => {
                    let pressed = self
                        .held_gamepad_buttons
                        .get(&button)
                        .copied()
                        .unwrap_or(0.0)
                        >= 0.5;
                    ActionValue::Bool(pressed)
                }
                #[cfg(feature = "gamepad")]
                BindingSource::GamepadStick(stick) => {
                    ActionValue::Axis2(gamepad_stick_value(&self.held_gamepad_axes, stick))
                }
            };

            let value = apply_modifiers(value, &binding.modifiers);
            update_action_state(action_state, value);
        }

        for (slot, action_state) in self.actions.iter_mut().enumerate() {
            if matched[slot] {
                continue;
            }

            action_state.phase = match action_state.value {
                ActionValue::Bool(true) => Phase::Ongoing,
                ActionValue::Bool(false) => Phase::Idle,
                ActionValue::Axis1(value) if value != 0.0 => Phase::Ongoing,
                ActionValue::Axis1(_) => Phase::Idle,
                ActionValue::Axis2(value) if value != Vec2::ZERO => Phase::Ongoing,
                ActionValue::Axis2(_) => Phase::Idle,
                ActionValue::Axis3(_) => Phase::Idle,
            };
        }
    }
}

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
