//! The evaluator: a plan and an input frame in, action state and a transition log out.
//!
//! The evaluator resolves bindings and emits a transition log for later dispatch.

use alloc::vec;

use bevy_ecs::prelude::{Res, ResMut};
use bevy_input::{ButtonState, keyboard::KeyboardInput};

use crate::action::{ActionValue, InputContext, Phase};
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

        for event in frame.events() {
            let RawEvent::Keyboard(KeyboardInput {
                key_code,
                state,
                repeat,
                ..
            }) = event.event;

            for (slot, binding) in self.plan.bindings().iter().enumerate() {
                if binding.key_code != key_code {
                    continue;
                }

                matched[slot] = true;
                let action_state = &mut self.actions[slot];

                match state {
                    ButtonState::Pressed => {
                        let was_pressed = matches!(action_state.value, ActionValue::Bool(true));
                        action_state.value = ActionValue::Bool(true);
                        action_state.phase = if repeat && was_pressed {
                            Phase::Ongoing
                        } else {
                            Phase::Fired
                        };
                    }
                    ButtonState::Released => {
                        action_state.value = ActionValue::Bool(false);
                        action_state.phase = Phase::Completed;
                    }
                }
            }
        }

        for (slot, action_state) in self.actions.iter_mut().enumerate() {
            if matched[slot] {
                continue;
            }

            action_state.phase = if matches!(action_state.value, ActionValue::Bool(true)) {
                Phase::Ongoing
            } else {
                Phase::Idle
            };
        }
    }
}
