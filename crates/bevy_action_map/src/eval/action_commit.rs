//! One action's commit: its bindings' latest outputs combined, its `combined` stage, and its phase
//! machine.

use bevy_math::Vec3;

use super::Transition;
use super::binding_pipeline::apply_modifiers;
use crate::action::{ActionIntent, ActionPhase, ActionValue, InputContext};
use crate::condition::ConditionState;
use crate::context::InputContextState;

impl<C: InputContext> InputContextState<C> {
    /// Commits one action: combines its bindings' latest outputs into one value and one condition,
    /// runs the action's `combined` stage on them, and moves its phase on, marking it dirty if it
    /// changed and logging the transition if the change was an edge.
    ///
    /// Called partway through a tick, with `delta` zero, once an event's superseded readings have
    /// run; and for every action at the closing step, with the tick's `delta`. Either way it takes
    /// the outputs as they stand, so a binding that did not run for this commit contributes what
    /// its last run produced. `bindings` is the slot's positions in the plan's bindings, which the
    /// plan keeps contiguous by sorting on slot.
    pub(super) fn commit_action(
        &mut self,
        slot: usize,
        bindings: core::ops::Range<usize>,
        delta: f32,
    ) {
        let Self {
            plan,
            actions,
            dirty,
            transitions,
            require_reset,
            scratch,
            binding_progress,
            interrupted,
            ..
        } = self;
        let intent = plan.intent_for_slot(slot);

        // The action's bindings, as their last runs left them. Gathered: their values combined into
        // the action's one value, the most definite condition among them, and what the
        // require-reset latch needs to know, which is whether any still reads pressed and whether
        // every one has run this tick.
        let mut combined_values = CombinedBindingValues::default();
        let mut strongest_condition = ConditionState::Idle;
        let mut still_held = false;
        let mut every_binding_ran = true;
        for progress in &binding_progress[bindings] {
            combined_values = combined_values.add(progress.output.value, intent);
            strongest_condition = strongest_condition.max(progress.output.condition);
            still_held |= progress.output.pressed;
            every_binding_ran &= progress.ran;
        }
        // A binding not yet run this tick has an output older than whatever set the require-reset
        // latch, so it cannot show the action at rest.
        if require_reset[slot] && intent == ActionIntent::Button && every_binding_ran && !still_held
        {
            require_reset.set(slot, false);
        }

        // The action's stage, declared through `combined::<A>()`: modifiers and conditions that run
        // on the combined value rather than on any one binding's. Its conditions, where it has any,
        // replace the bindings' strongest one as the action's condition state.
        let value = combined_values.value();
        let (value, condition_state) = match plan.stage(slot) {
            stage if stage.is_empty() => (value, strongest_condition),
            stage => {
                // The stage's working memory sits after every binding's: its modifiers', then its
                // conditions'.
                let stage_scratch = &mut scratch[stage.scratch_base
                    ..stage.scratch_base + stage.modifiers.len() + stage.conditions.len()];
                let (modifier_scratch, condition_scratch) =
                    stage_scratch.split_at_mut(stage.modifiers.len());
                let value = apply_modifiers(value, &stage.modifiers, modifier_scratch, delta);
                if stage.conditions.is_empty() {
                    (value, strongest_condition)
                } else {
                    let stage_condition = crate::condition::combine(
                        &stage.conditions,
                        value,
                        condition_scratch,
                        delta,
                    );
                    // A binding part way through a hold contributes rest, which the stage's
                    // conditions alone would read as `Idle`, and the action would lose its
                    // `Started`. The hold keeps it `Building` instead (TD5.5).
                    let stage_condition = match (stage_condition, strongest_condition) {
                        (ConditionState::Idle, ConditionState::Building) => strongest_condition,
                        _ => stage_condition,
                    };
                    let value = if stage_condition == ConditionState::Satisfied {
                        value
                    } else {
                        ActionValue::Bool(false)
                    };
                    (value, stage_condition)
                }
            }
        };

        // Set by `run_binding_pipeline` when one of this action's bindings, having had something to
        // say, ran on a reading whose control went away while held. Taken here, so it covers only
        // the runs since the action last committed.
        let was_interrupted = interrupted.contains(slot);
        interrupted.set(slot, false);

        // Compared rather than inferred from the phase: a held stick reports `Firing` every tick
        // while its value moves, and an action whose value moved has changed as surely as one that
        // started or stopped.
        let before = actions[slot];
        let phase =
            update_action_state(&mut actions[slot], value, condition_state, was_interrupted);
        if actions[slot] != before {
            dirty.set(slot, true);
        }
        // Only the edges. The level phases say that nothing changed, and an observer firing every
        // tick for a held button would be noise rather than information.
        if matches!(
            phase,
            ActionPhase::Started
                | ActionPhase::Fired
                | ActionPhase::Completed
                | ActionPhase::Canceled
        ) {
            transitions.push(Transition { slot, phase, value });
        }
    }
}

/// An action's value part way through combining its bindings, split by sign on each axis (D76).
///
/// Kept as two halves so that each direction holds its own strongest push until `value` adds them;
/// that method's doc says why the result is the right one.
#[derive(Clone, Copy, Default)]
struct CombinedBindingValues {
    positive: Vec3,
    negative: Vec3,
    /// The most components any contribution carried.
    rank: u8,
}

impl CombinedBindingValues {
    /// Combines one more binding's contribution.
    ///
    /// A delta is a displacement, so two of them add. Everything else is a position or a press,
    /// where adding would be a units error: each sign keeps its strongest contribution, so opposite
    /// directions cancel and like ones do not add.
    fn add(self, contribution: ActionValue, intent: ActionIntent) -> Self {
        let value = contribution.to_axis3();
        let (positive, negative) = match intent {
            ActionIntent::Delta2 => (
                self.positive + value.max(Vec3::ZERO),
                self.negative + value.min(Vec3::ZERO),
            ),
            ActionIntent::Button | ActionIntent::Analog1 | ActionIntent::Directional2 => {
                (self.positive.max(value), self.negative.min(value))
            }
        };
        Self {
            positive,
            negative,
            rank: self.rank.max(rank(contribution)),
        }
    }

    /// The combined value, in the shape of the widest contribution.
    ///
    /// Why each result is the one a player expects:
    ///
    /// - **A press** is on when any of its bindings is. Space and a pad's South both bound to jump
    ///   are two ways to ask for the same thing, and either one asks.
    /// - **An axis or a direction** takes, on each axis, the strongest push each way and adds the
    ///   two. Two keys pushing right are not twice as fast as one, and a stick at `0.6` beside a
    ///   key at `1.0` reads `1.0`, the more definite of the two. Pushing both ways at once, left
    ///   and right held together, reads as neither. Each axis is judged on its own, so `W` and `D`
    ///   make the diagonal `(1, 1)`; nothing here clamps it, since only the action knows whether a
    ///   diagonal should be shortened, and a `combined` stage says so.
    /// - **A delta** is summed. Two movements in one tick are two distances travelled, and the
    ///   total is how far the pointer went.
    ///
    /// The shape is the widest any contribution had, so a component appears only where some binding
    /// supplied one.
    fn value(self) -> ActionValue {
        let total = self.positive + self.negative;
        match self.rank {
            0 => ActionValue::Bool(total != Vec3::ZERO),
            1 => ActionValue::Axis1(total.x),
            2 => ActionValue::Axis2(total.truncate()),
            _ => ActionValue::Axis3(total),
        }
    }
}

/// How many components a value carries: none for a press, up to three for `Axis3`.
fn rank(value: ActionValue) -> u8 {
    match value {
        ActionValue::Bool(_) => 0,
        ActionValue::Axis1(_) => 1,
        ActionValue::Axis2(_) => 2,
        ActionValue::Axis3(_) => 3,
    }
}

/// Moves one action's state on, and reports the edge if there was one.
///
/// The edges are `Started`, `Fired`, `Completed` and `Canceled`, each lasting one commit; `Idle`,
/// `Building` and `Firing` are the levels between them.
///
/// `condition_state` says what the bindings decided; this decides what that means given where the
/// action already was. That is what makes giving up on a hold emit a `Canceled` rather than a
/// `Completed` — the action never actually happened.
///
/// `interrupted` says a binding reached rest because its control went away while held rather than
/// being let go, which makes a firing action that goes idle `Canceled` too (D94).
fn update_action_state(
    action_state: &mut crate::action::ActionState,
    value: ActionValue,
    condition_state: ConditionState,
    interrupted: bool,
) -> ActionPhase {
    let was_firing = matches!(action_state.phase, ActionPhase::Fired | ActionPhase::Firing);
    let was_building = matches!(
        action_state.phase,
        ActionPhase::Started | ActionPhase::Building
    );

    let phase = match condition_state {
        ConditionState::Satisfied => {
            if was_firing {
                ActionPhase::Firing
            } else {
                ActionPhase::Fired
            }
        }
        ConditionState::Building => {
            if was_firing {
                // It was firing and has fallen back to merely building, which from the outside is
                // the action ending.
                ActionPhase::Completed
            } else if was_building {
                ActionPhase::Building
            } else {
                ActionPhase::Started
            }
        }
        ConditionState::Idle => {
            if was_firing {
                if interrupted {
                    ActionPhase::Canceled
                } else {
                    ActionPhase::Completed
                }
            } else if was_building {
                ActionPhase::Canceled
            } else {
                ActionPhase::Idle
            }
        }
    };

    action_state.value = value;
    action_state.phase = phase;
    phase
}
