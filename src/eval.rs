//! The evaluator: a plan and an input frame in, action state and a transition log out.
//!
//! The evaluator resolves bindings and emits a transition log for later dispatch.

use alloc::vec;

use bevy_ecs::prelude::{Res, ResMut};
use bevy_input::{ButtonState, keyboard::KeyboardInput};
use bevy_math::Vec2;

use crate::action::{ActionValue, InputContext, Phase};
use crate::binding::BindingSource;
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
            }
        }

        for (slot, binding) in self.plan.bindings().iter().enumerate() {
            matched[slot] = true;
            let action_state = &mut self.actions[slot];

            match binding.source {
                BindingSource::Button(key_code) => {
                    update_button_state(action_state, self.held_buttons.contains(&key_code));
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
                    update_vec2_state(action_state, Vec2::new(x, y));
                }
                BindingSource::MouseMotion => {
                    update_vec2_state(action_state, mouse_delta);
                }
            }
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

fn update_button_state(action_state: &mut crate::action::ActionState, value: bool) {
    let previous = matches!(action_state.value, ActionValue::Bool(true));
    action_state.value = ActionValue::Bool(value);
    action_state.phase = match (previous, value) {
        (false, false) => Phase::Idle,
        (false, true) => Phase::Fired,
        (true, true) => Phase::Ongoing,
        (true, false) => Phase::Completed,
    };
}

fn update_vec2_state(action_state: &mut crate::action::ActionState, value: Vec2) {
    let previous = match action_state.value {
        ActionValue::Axis2(previous) => previous,
        _ => Vec2::ZERO,
    };
    action_state.value = ActionValue::Axis2(value);
    action_state.phase = match (previous == Vec2::ZERO, value == Vec2::ZERO) {
        (true, true) => Phase::Idle,
        (true, false) => Phase::Fired,
        (false, false) => Phase::Ongoing,
        (false, true) => Phase::Completed,
    };
}
