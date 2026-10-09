//! Public items that no example or other test names, named here so that narrowing one breaks the
//! build rather than quietly changing the API.
//!
//! These check that each item can be reached and used from outside the crate, not what it does:
//! the tests pass by compiling.

use bevy_action_map::action::{ActionInfo, Registers, registered_actions};
use bevy_action_map::prelude::*;

/// A custom condition, the one place a game handles `Registers` itself.
struct Charging;

impl Condition for Charging {
    fn evaluate(
        &self,
        value: ActionValue,
        registers: &mut Registers,
        delta: f32,
    ) -> ConditionState {
        if value.to_bool() {
            registers.time += delta;
            ConditionState::Building
        } else {
            registers.time = 0.0;
            ConditionState::Idle
        }
    }
}

#[test]
fn a_custom_condition_is_handed_its_registers() {
    let _: &dyn Condition = &Charging;
}

#[test]
fn registered_actions_lists_what_has_been_reached() {
    let _: fn() -> Vec<ActionInfo> = registered_actions;
}
