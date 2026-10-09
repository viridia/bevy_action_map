//! One binding's stateful stages, which record one reading at a time.

use alloc::vec::Vec;

use super::binding_reading::BindingReading;
use crate::action::{ActionIntent, ActionValue, Registers};
use crate::binding::{BindingModifier, ButtonThreshold, Control};
use crate::condition::ConditionState;
use crate::plan::CompiledBinding;

/// What one binding contributed the last time its pipeline ran, kept so that an action committing
/// partway through a tick can take it without running the binding again.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BindingOutput {
    /// Rest unless the conditions are satisfied.
    pub(crate) value: ActionValue,
    /// What the conditions made of the value: idle, building (a hold charging, say), or satisfied.
    pub(crate) condition: ConditionState,
    /// Whether the value read as pressed before the require-reset latch held it back, which is what
    /// decides whether that latch may clear.
    pub(crate) pressed: bool,
}

impl Default for BindingOutput {
    fn default() -> Self {
        Self {
            value: ActionValue::Bool(false),
            condition: ConditionState::Idle,
            pressed: false,
        }
    }
}

/// What one run of a binding's pipeline is handed.
pub(crate) struct PipelineRun {
    pub(crate) reading: BindingReading,
    /// Zero for a reading superseded partway through a tick, and the tick's `delta` for the run at
    /// its close (D97).
    pub(crate) delta: f32,
    /// A `Button` action waiting to be seen at rest once before it may fire (R7.5).
    pub(crate) awaiting_release: bool,
    /// The latch of the binding's shared `hold_or_toggle` group, when the group is in toggle mode.
    pub(crate) shared_latch: Option<bool>,
}

/// Runs a binding's modifiers, press threshold, require-reset and conditions on one reading, in
/// that order, and records its claim.
///
/// These are the binding's stateful stages, each remembering something in `scratch` between runs,
/// which is why `run_binding_pipeline` calls this only when the reading changes and at the closing
/// step. `scratch` is the binding's own, as `CompiledBinding::scratch_base` places it; `claims`
/// collects the controls the binding grabs from contexts below while it has something to say.
pub(crate) fn record_reading(
    binding: &CompiledBinding,
    intent: ActionIntent,
    run: PipelineRun,
    scratch: &mut [Registers],
    threshold: &ButtonThreshold,
    claims: &mut Vec<Control>,
) -> BindingOutput {
    let PipelineRun {
        reading,
        delta,
        awaiting_release,
        shared_latch,
    } = run;

    // The three disjoint pieces `CompiledBinding::scratch_base` allocates.
    let (modifier_scratch, after_modifiers) = scratch.split_at_mut(binding.modifiers.len());
    let (condition_scratch, press_scratch) = after_modifiers.split_at_mut(binding.conditions.len());

    // With the group's toggle on, a binding whose tunable is shared reads back the latch the group
    // resolved rather than running its own modifier chain. The members share one latch, and each
    // running its own toggle against it would re-flip it on every tick a different member is held;
    // `apply_toggle`'s doc has the detail.
    //
    // In hold mode there is no latch to share: each binding is an ordinary momentary control again,
    // so it falls through to the same modifier chain an unshared binding runs, whose own
    // `Toggle { active: false }` is identity.
    //
    // A reading that does not count is already rest, and is still recorded by the chain and the
    // conditions: a hold that loses its control has to be told, or it would resume from where it
    // left off when the control came back.
    let value = match shared_latch {
        Some(latch) => ActionValue::Bool(latch),
        None => apply_modifiers(reading.value, &binding.modifiers, modifier_scratch, delta),
    };
    // Where a press comes from something that was not already a press, the threshold has to settle
    // it here. Reading it later cannot: by then the only question a stored value can answer is
    // whether it is off centre, and a resting stick always is. Modifiers run first so that a
    // deadzone gets to define centre.
    //
    // Hysteretic like the button channel's own, but remembered per *binding* rather than per
    // control: the value here was assembled from a deadzone, a composite, or whatever else the
    // chain did, and no single control owns the answer.
    let value = match (intent, value) {
        (ActionIntent::Button, ActionValue::Bool(_)) => value,
        (ActionIntent::Button, _) => {
            let registers = &mut press_scratch[0];
            let pressed = threshold.pressed(value.to_axis1().abs(), registers.prev.to_bool());
            registers.prev = ActionValue::Bool(pressed);
            ActionValue::Bool(pressed)
        }
        _ => value,
    };
    let pressed = value.to_bool();
    // Rest reaches the conditions too, so a hold does not charge and a binding held back by the
    // require-reset latch never claims its control from a context below.
    let value = if awaiting_release {
        ActionValue::Bool(false)
    } else {
        value
    };
    // Conditions decide *whether* this binding is firing; the value it contributes is rest until it
    // is. A hold half-finished must not move the ship.
    let condition = crate::condition::combine(&binding.conditions, value, condition_scratch, delta);
    // Claimed while the binding has something to say, so a binding that is merely bound to a
    // control does not hold it against everyone else all the time — but one whose condition is part
    // way through does. Firing alone is too narrow: a menu binding that fires once per direction
    // entered would hand the stick back to the game between two crossings, and a charging hold
    // would leak its key to whatever is underneath until it completed.
    if binding.consume && condition >= ConditionState::Building {
        claims.extend(binding.input.controls());
    }
    let value = if condition == ConditionState::Satisfied {
        value
    } else {
        ActionValue::Bool(false)
    };
    BindingOutput {
        value,
        condition,
        pressed,
    }
}

/// Runs a modifier chain in order, each modifier with its own registers. Shared by a binding's
/// chain and an action's `combined` stage.
pub(super) fn apply_modifiers(
    mut value: ActionValue,
    modifiers: &[BindingModifier],
    scratch: &mut [Registers],
    delta: f32,
) -> ActionValue {
    for (modifier, registers) in modifiers.iter().zip(scratch) {
        value = modifier.apply(value, registers, delta);
    }
    value
}
