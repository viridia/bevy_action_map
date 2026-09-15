//! Conditions: deciding *when* a binding counts as firing.
//!
//! Without one, a binding fires whenever its control is off rest — press the key, the action is
//! active. A condition narrows that to something more specific: only on the press itself, only
//! after the key has been down for half a second, only if it was tapped twice.
//!
//! Attach them where you declare the binding:
//!
//! ```ignore
//! context.bind::<Jump>(KeyCode::Space).press();
//! context.bind::<Charge>(KeyCode::Space).hold(0.4);
//! context.bind::<Hyperspace>(KeyCode::ShiftLeft).multi_tap(2, 0.3);
//! ```
//!
//! Every duration here is in the context's own seconds, which is the fixed timestep for a fixed
//! context. A paused clock pauses them, and a replay reproduces them exactly.
//!
//! # When there is more than one
//!
//! Conditions come in three kinds:
//!
//! - **Explicit** — if a binding has any, at least one must be satisfied.
//! - **Implicit** — every one must be satisfied.
//! - **Blocking** — if any is satisfied, the binding does not fire at all.
//!
//! So `.press()` and `.hold(0.5)` together mean "either a press or a long hold", while a blocking
//! condition vetoes regardless of what the others said.

use bevy_platform::sync::Arc;

use crate::action::{ActionValue, Scratch};

/// What a condition says about its binding this tick.
///
/// Ordered, because several bindings can feed one action and the most definite of them decides: a
/// binding that fired outranks one still building, which outranks one with nothing to say.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConditionState {
    /// Not satisfied, and nothing in progress.
    Idle,
    /// On the way — a hold that has not lasted long enough yet, a tap waiting for its second press.
    ///
    /// Reported so that an action can show a charge meter, and so that giving up part way through
    /// is [`Canceled`](crate::action::ActionPhase::Canceled) rather than silence.
    Building,
    /// Satisfied. The binding fires.
    Satisfied,
}

/// How a condition takes part when a binding has several.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionKind {
    /// At least one explicit condition must be satisfied.
    Explicit,
    /// Every implicit condition must be satisfied.
    Implicit,
    /// If this is satisfied, the binding does not fire.
    Blocking,
}

/// A rule about when a binding fires.
///
/// Implement this for anything the built-in set does not cover. Like a modifier, a condition is a
/// pure function of what it is handed, so a replay or a rollback reruns it to the same answer.
pub trait Condition: Send + Sync + 'static {
    /// Decides what this condition makes of the binding's value this tick.
    ///
    /// `scratch` is this condition's own working memory and persists between ticks; `delta` is how
    /// long the owning context's tick was, in its own seconds.
    fn evaluate(&self, value: ActionValue, scratch: &mut Scratch, delta: f32) -> ConditionState;

    /// How this condition combines with the others on its binding.
    fn kind(&self) -> ConditionKind {
        ConditionKind::Explicit
    }
}

/// The built-in conditions.
// `Clone` for the reason `BindingModifier` is: applying an override copies the authored bindings
// and rewrites their inputs, and the defaults have to survive that intact.
#[derive(Clone)]
pub enum BindingCondition {
    /// Fires on the tick the control leaves rest.
    Press,
    /// Fires on the tick the control returns to rest.
    Release,
    /// Fires for as long as the control is off rest, which is what a binding does anyway.
    ///
    /// Useful as an *implicit* condition alongside an explicit one, to require that the control is
    /// still down when something else fires.
    Down,
    /// Fires once the control has been off rest for `duration`, and keeps firing while it stays.
    Hold {
        /// How long, in the context's seconds.
        duration: f32,
        /// Fire once rather than every tick thereafter.
        one_shot: bool,
    },
    /// Fires on release, but only if the control was held for at least `duration` first.
    HoldAndRelease {
        /// How long, in the context's seconds.
        duration: f32,
    },
    /// Fires on release, but only if the control was held for no longer than `max_duration`.
    Tap {
        /// The longest a press can be and still count as a tap.
        max_duration: f32,
    },
    /// Fires when the control has been tapped `count` times, each within `max_gap` of the last.
    MultiTap {
        /// How many taps. Two is a double-tap.
        count: u16,
        /// The longest gap between taps that still continues the sequence.
        max_gap: f32,
    },
    /// Fires every `interval` for as long as the control is off rest.
    Pulse {
        /// How long between fires, in the context's seconds.
        interval: f32,
        /// Fire immediately on the first tick as well as every interval after.
        immediate: bool,
    },
    /// Fires on the tick the value differs from what it was on the tick before.
    Change,
    /// Calls an application-defined condition.
    ///
    /// Shared rather than owned, so that copying a binding set copies the reference and not the
    /// condition. Use [`when`](crate::binding::BindingBuilder::when) rather than building this by
    /// hand.
    Custom(Arc<dyn Condition>),
}

/// The part of a binding's timing that a prompt or a rebinding row has to say something about.
///
/// Every condition changes *when* a binding fires, but only holding and multi-tapping change what a
/// player needs to be told beyond the control's own name: `Thrust` and `Afterburner` on one key
/// produce the same caption unless the caption knows one of them wants the key held. The rest —
/// press, release, down, a tap's own ceiling, a pulse, a change — read the same as a bare press to a
/// player, so they carry [`None`](Self::None) here.
///
/// This is handed to a localization layer as structure rather than as rendered text, so a
/// translator chooses its own word order; [`fallback_format`](Self::fallback_format) is the
/// built-in renderer for a game that ships no catalogue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConditionDescriptor {
    /// Nothing to add: the control's own name is the whole answer.
    None,
    /// Fires only once the control has been held for `duration` seconds.
    Hold {
        /// How long, in the context's seconds.
        duration: f32,
    },
    /// Fires only after `count` taps.
    MultiTap {
        /// How many taps. Two is a double-tap.
        count: u16,
    },
}

impl ConditionDescriptor {
    /// English fallback text combining this with a control's own label — "Hold W", "W ×2" — for a
    /// game with no catalogue to ask instead.
    ///
    /// Returns the whole formula rather than a diff against the control, because a bare "Hold"
    /// means nothing to a player who has not already read the control it qualifies. Word order is
    /// English; a catalogue exists so a translator can choose its own.
    pub fn fallback_format(self, control: &str) -> alloc::string::String {
        match self {
            Self::None => alloc::string::String::from(control),
            Self::Hold { .. } => alloc::format!("Hold {control}"),
            Self::MultiTap { count } => alloc::format!("{control} \u{d7}{count}"),
        }
    }
}

impl BindingCondition {
    /// What this one condition contributes to a `ConditionDescriptor`, where it contributes
    /// anything. `HoldAndRelease` reads as a hold for the same reason `Hold` does: the player still
    /// has to hold the control, even though what fires is the release at the end of it.
    fn descriptor(&self) -> Option<ConditionDescriptor> {
        match self {
            Self::Hold { duration, .. } | Self::HoldAndRelease { duration } => {
                Some(ConditionDescriptor::Hold {
                    duration: *duration,
                })
            }
            Self::MultiTap { count, .. } => Some(ConditionDescriptor::MultiTap { count: *count }),
            _ => None,
        }
    }
}

/// What a binding's whole set of conditions contributes to a prompt or a mapping row.
///
/// The first condition in declaration order that has anything to say wins. Nothing built in ever
/// gives one binding both a hold and a multi-tap, so this is a simplification only a `Custom`
/// combination could ever notice.
pub(crate) fn describe(conditions: &[BindingCondition]) -> ConditionDescriptor {
    conditions
        .iter()
        .find_map(BindingCondition::descriptor)
        .unwrap_or(ConditionDescriptor::None)
}

/// Bit positions within [`Scratch::flags`](crate::action::Scratch::flags).
const HELD: u8 = 1 << 0;
const DONE: u8 = 1 << 1;

impl BindingCondition {
    /// Decides what this condition makes of the binding's value this tick.
    pub fn evaluate(
        &self,
        value: ActionValue,
        scratch: &mut Scratch,
        delta: f32,
    ) -> ConditionState {
        let actuated = value.to_bool();
        // The whole value rather than whether it was off rest, so that a condition comparing one
        // tick against the last has something to compare. Everything below reads `was`, which is
        // the same answer either way, so this costs nothing to the conditions that do not care.
        let previous = scratch.prev;
        let was = previous.to_bool();
        scratch.prev = value;

        match self {
            Self::Press => condition_state(actuated && !was),
            Self::Release => condition_state(!actuated && was),
            Self::Down => condition_state(actuated),

            Self::Hold { duration, one_shot } => {
                if !actuated {
                    scratch.time = 0.0;
                    scratch.flags &= !DONE;
                    return ConditionState::Idle;
                }
                scratch.time += delta;
                if scratch.time < *duration {
                    return ConditionState::Building;
                }
                if *one_shot {
                    if scratch.flags & DONE != 0 {
                        return ConditionState::Idle;
                    }
                    scratch.flags |= DONE;
                }
                ConditionState::Satisfied
            }

            Self::HoldAndRelease { duration } => {
                if actuated {
                    scratch.time += delta;
                    return ConditionState::Building;
                }
                // The release is the fire, so the timer has to be read before it is cleared.
                let long_enough = was && scratch.time >= *duration;
                scratch.time = 0.0;
                condition_state(long_enough)
            }

            Self::Tap { max_duration } => {
                if actuated {
                    scratch.time += delta;
                    return ConditionState::Building;
                }
                let quick_enough = was && scratch.time <= *max_duration;
                scratch.time = 0.0;
                condition_state(quick_enough)
            }

            Self::MultiTap { count, max_gap } => {
                // The gap runs between taps, so it is measured whether or not the control is down.
                scratch.time += delta;

                if scratch.count > 0 && scratch.time > *max_gap {
                    // Too slow: the sequence lapses rather than counting toward the next one.
                    scratch.count = 0;
                    scratch.flags &= !HELD;
                }

                if actuated && !was {
                    scratch.flags |= HELD;
                    scratch.time = 0.0;
                } else if !actuated && was && scratch.flags & HELD != 0 {
                    scratch.flags &= !HELD;
                    scratch.count += 1;
                    scratch.time = 0.0;
                    if scratch.count >= *count {
                        scratch.count = 0;
                        return ConditionState::Satisfied;
                    }
                }

                if scratch.count > 0 || actuated {
                    ConditionState::Building
                } else {
                    ConditionState::Idle
                }
            }

            Self::Pulse {
                interval,
                immediate,
            } => {
                if !actuated {
                    scratch.time = 0.0;
                    scratch.flags &= !DONE;
                    return ConditionState::Idle;
                }
                if scratch.flags & DONE == 0 {
                    scratch.flags |= DONE;
                    if *immediate {
                        return ConditionState::Satisfied;
                    }
                }
                scratch.time += delta;
                if scratch.time >= *interval {
                    scratch.time -= *interval;
                    return ConditionState::Satisfied;
                }
                ConditionState::Building
            }

            Self::Change => {
                // Two values that are both at rest are the same input however they are spelled —
                // a fresh scratch holds `Bool(false)` and the first tick of a stick reports
                // `Axis2(ZERO)`, and that is not the player doing anything.
                if value != previous && (actuated || was) {
                    ConditionState::Satisfied
                } else if actuated {
                    // Unchanged, but the control is still off rest. `Building` rather than `Idle`
                    // because a consuming binding claims its controls for as long as it has
                    // something to say, and letting go of the claim between two crossings would
                    // hand the control back to whatever is underneath in the meantime.
                    ConditionState::Building
                } else {
                    ConditionState::Idle
                }
            }

            Self::Custom(condition) => condition.evaluate(value, scratch, delta),
        }
    }

    /// How this condition combines with the others on its binding.
    pub fn kind(&self) -> ConditionKind {
        match self {
            // `Down` is a requirement rather than an event: it says the control is still held,
            // which is what you want alongside something else rather than on its own.
            Self::Down => ConditionKind::Implicit,
            Self::Custom(condition) => condition.kind(),
            _ => ConditionKind::Explicit,
        }
    }
}

fn condition_state(satisfied: bool) -> ConditionState {
    if satisfied {
        ConditionState::Satisfied
    } else {
        ConditionState::Idle
    }
}

/// Combines every condition on one binding into a single answer, following the rules described in
/// this module.
///
/// With no conditions at all, a binding fires whenever its control is off rest — which is what a
/// binding does before anyone asks for anything more specific.
pub(crate) fn combine(
    conditions: &[BindingCondition],
    value: ActionValue,
    scratch: &mut [Scratch],
    delta: f32,
) -> ConditionState {
    if conditions.is_empty() {
        return condition_state(value.to_bool());
    }

    let mut explicit = 0usize;
    let mut explicit_satisfied = false;
    let mut implicit_all = true;
    let mut has_value_condition = false;
    let mut building = false;
    let mut blocked = false;

    for (condition, scratch) in conditions.iter().zip(scratch) {
        let outcome = condition.evaluate(value, scratch, delta);
        match condition.kind() {
            ConditionKind::Explicit => {
                has_value_condition = true;
                explicit += 1;
                match outcome {
                    ConditionState::Satisfied => explicit_satisfied = true,
                    ConditionState::Building => building = true,
                    ConditionState::Idle => {}
                }
            }
            ConditionKind::Implicit => {
                has_value_condition = true;
                if outcome != ConditionState::Satisfied {
                    implicit_all = false;
                }
                if outcome == ConditionState::Building {
                    building = true;
                }
            }
            // A blocker's own progress is nobody's business; only whether it vetoes.
            ConditionKind::Blocking => blocked |= outcome == ConditionState::Satisfied,
        }
    }

    if blocked {
        return ConditionState::Idle;
    }
    // Blocking conditions alone read nothing off the control's own value, so a binding with only
    // those falls back to the same at-rest check the no-conditions case uses, past the veto.
    if !has_value_condition {
        return condition_state(value.to_bool());
    }
    if implicit_all && (explicit == 0 || explicit_satisfied) {
        return ConditionState::Satisfied;
    }
    if building {
        return ConditionState::Building;
    }
    ConditionState::Idle
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::vec::Vec;

    const TICK: f32 = 0.1;

    /// Drives one condition through a script of "is the control down this tick", and reports what
    /// it said each time. Every duration below is a multiple of `TICK`, so the arithmetic is exact.
    fn run(condition: &BindingCondition, script: &[bool]) -> Vec<ConditionState> {
        let mut scratch = Scratch::default();
        script
            .iter()
            .map(|down| condition.evaluate(ActionValue::Bool(*down), &mut scratch, TICK))
            .collect()
    }

    #[test]
    fn press_and_release_are_edges() {
        use ConditionState::{Idle, Satisfied};

        assert_eq!(
            run(&BindingCondition::Press, &[false, true, true, false, true]),
            [Idle, Satisfied, Idle, Idle, Satisfied]
        );
        assert_eq!(
            run(
                &BindingCondition::Release,
                &[false, true, true, false, false]
            ),
            [Idle, Idle, Idle, Satisfied, Idle]
        );
    }

    #[test]
    fn a_hold_reports_progress_then_fires_and_keeps_firing() {
        use ConditionState::{Building, Idle, Satisfied};

        let hold = BindingCondition::Hold {
            duration: 0.25,
            one_shot: false,
        };
        // Down for three ticks reaches 0.3, so the third crosses the line.
        assert_eq!(
            run(&hold, &[true, true, true, true, false]),
            [Building, Building, Satisfied, Satisfied, Idle]
        );
    }

    #[test]
    fn a_one_shot_hold_fires_exactly_once_per_press() {
        use ConditionState::{Building, Idle, Satisfied};

        let hold = BindingCondition::Hold {
            duration: 0.25,
            one_shot: true,
        };
        assert_eq!(
            run(&hold, &[true, true, true, true, false, true, true, true]),
            [
                Building, Building, Satisfied, Idle, Idle, Building, Building, Satisfied
            ]
        );
    }

    // The distinction that makes a hold worth having: letting go early has to be visibly different
    // from letting go late, and neither may look like nothing happened.
    #[test]
    fn hold_and_release_fires_only_when_the_hold_was_long_enough() {
        use ConditionState::{Building, Idle, Satisfied};

        let condition = BindingCondition::HoldAndRelease { duration: 0.25 };
        assert_eq!(
            run(&condition, &[true, true, true, false]),
            [Building, Building, Building, Satisfied],
            "held long enough, then released"
        );
        assert_eq!(
            run(&condition, &[true, false]),
            [Building, Idle],
            "let go too early"
        );
    }

    #[test]
    fn a_tap_is_a_press_that_did_not_last() {
        use ConditionState::{Building, Idle, Satisfied};

        let tap = BindingCondition::Tap { max_duration: 0.25 };
        assert_eq!(
            run(&tap, &[true, false]),
            [Building, Satisfied],
            "quick enough"
        );
        assert_eq!(
            run(&tap, &[true, true, true, true, false]),
            [Building, Building, Building, Building, Idle],
            "held far too long to be a tap"
        );
    }

    #[test]
    fn a_double_tap_needs_both_taps_inside_the_window() {
        use ConditionState::{Building, Idle, Satisfied};

        let double = BindingCondition::MultiTap {
            count: 2,
            max_gap: 0.25,
        };
        assert_eq!(
            run(&double, &[true, false, true, false]),
            [Building, Building, Building, Satisfied]
        );

        // The same two taps with a long enough wait between them never make a double-tap. The
        // second one is not wasted — it begins a fresh sequence, which is why the tail is `Building`
        // rather than `Idle`.
        let dawdled = run(&double, &[true, false, false, false, false, true, false]);
        assert!(
            !dawdled.contains(&Satisfied),
            "two taps a window apart fired anyway: {dawdled:?}"
        );
        assert_eq!(dawdled[4], Idle, "the first sequence lapsed");
    }

    #[test]
    fn a_pulse_repeats_while_the_control_is_held() {
        use ConditionState::{Building, Idle, Satisfied};

        let pulse = BindingCondition::Pulse {
            interval: 0.2,
            immediate: true,
        };
        assert_eq!(
            run(&pulse, &[true, true, true, true, true, false]),
            [Satisfied, Building, Satisfied, Building, Satisfied, Idle]
        );
    }

    // The three-way split. Each kind is checked for the thing only it can do.
    #[test]
    fn the_three_kinds_compose_as_documented() {
        struct Always(ConditionState, ConditionKind);

        impl Condition for Always {
            fn evaluate(&self, _: ActionValue, _: &mut Scratch, _: f32) -> ConditionState {
                self.0
            }
            fn kind(&self) -> ConditionKind {
                self.1
            }
        }

        fn state_of(conditions: Vec<BindingCondition>) -> ConditionState {
            let mut scratch = alloc::vec![Scratch::default(); conditions.len()];
            combine(&conditions, ActionValue::Bool(true), &mut scratch, TICK)
        }

        let explicit = |v| BindingCondition::Custom(Arc::new(Always(v, ConditionKind::Explicit)));
        let implicit = |v| BindingCondition::Custom(Arc::new(Always(v, ConditionKind::Implicit)));
        let blocking = |v| BindingCondition::Custom(Arc::new(Always(v, ConditionKind::Blocking)));

        // No conditions at all: the control being off rest is the whole test.
        assert_eq!(state_of(Vec::new()), ConditionState::Satisfied);

        // Explicit: any one is enough.
        assert_eq!(
            state_of(alloc::vec![
                explicit(ConditionState::Idle),
                explicit(ConditionState::Satisfied)
            ]),
            ConditionState::Satisfied
        );
        // Implicit: all of them, or none of it.
        assert_eq!(
            state_of(alloc::vec![
                implicit(ConditionState::Satisfied),
                implicit(ConditionState::Idle)
            ]),
            ConditionState::Idle
        );
        // Blocking: a veto beats everything the others agreed on.
        assert_eq!(
            state_of(alloc::vec![
                explicit(ConditionState::Satisfied),
                blocking(ConditionState::Satisfied)
            ]),
            ConditionState::Idle
        );
        // And progress survives to be reported when nothing has fired yet.
        assert_eq!(
            state_of(alloc::vec![explicit(ConditionState::Building)]),
            ConditionState::Building
        );
    }

    // A binding with nothing but a non-vetoing blocking condition reads no value at all, so it
    // must fall back to the same at-rest check the no-conditions case uses, not fire unasked.
    #[test]
    fn a_lone_blocking_condition_falls_back_to_the_control_at_rest() {
        struct NeverVetoes;

        impl Condition for NeverVetoes {
            fn evaluate(&self, _: ActionValue, _: &mut Scratch, _: f32) -> ConditionState {
                ConditionState::Idle
            }
            fn kind(&self) -> ConditionKind {
                ConditionKind::Blocking
            }
        }

        fn state_of(value: ActionValue) -> ConditionState {
            let conditions = alloc::vec![BindingCondition::Custom(Arc::new(NeverVetoes))];
            let mut scratch = alloc::vec![Scratch::default(); conditions.len()];
            combine(&conditions, value, &mut scratch, TICK)
        }

        assert_eq!(state_of(ActionValue::Bool(false)), ConditionState::Idle);
        assert_eq!(state_of(ActionValue::Bool(true)), ConditionState::Satisfied);
    }

    // Drives one condition through a script of values, which is what `run` cannot do: a condition
    // that compares one tick against the last needs the value and not only whether it was down.
    fn run_values(condition: &BindingCondition, script: &[ActionValue]) -> Vec<ConditionState> {
        let mut scratch = Scratch::default();
        script
            .iter()
            .map(|value| condition.evaluate(*value, &mut scratch, TICK))
            .collect()
    }

    #[test]
    fn a_change_is_a_new_value_rather_than_a_new_press() {
        use ConditionState::{Building, Idle, Satisfied};
        use bevy_math::Vec2;

        let north = ActionValue::Axis2(Vec2::Y);
        let east = ActionValue::Axis2(Vec2::X);
        let rest = ActionValue::Axis2(Vec2::ZERO);

        assert_eq!(
            run_values(&BindingCondition::Change, &[rest, north, north, east, rest]),
            [Idle, Satisfied, Building, Satisfied, Satisfied],
            "one fire per direction entered, and one more on the way back to rest"
        );
    }

    // The trap a `Bool(false)` default sets: a fresh scratch and a stick sitting at centre are the
    // same input spelled two ways, and reading them as a change would fire on the first tick of
    // every context with nobody touching anything.
    #[test]
    fn rest_spelled_differently_is_not_a_change() {
        use bevy_math::Vec2;

        assert_eq!(
            run_values(
                &BindingCondition::Change,
                &[ActionValue::Axis2(Vec2::ZERO), ActionValue::Axis1(0.0)]
            ),
            [ConditionState::Idle, ConditionState::Idle]
        );
    }

    // A held direction has to keep saying something, because consumption follows this state: a
    // menu that dropped to `Idle` between two crossings would hand the stick back to the game
    // underneath it for those ticks.
    #[test]
    fn a_held_direction_stays_ongoing_between_changes() {
        let held = run_values(&BindingCondition::Change, &[ActionValue::Axis1(1.0); 4]);
        assert_eq!(
            held,
            [
                ConditionState::Satisfied,
                ConditionState::Building,
                ConditionState::Building,
                ConditionState::Building
            ]
        );
    }

    // Auto-repeat, built from two conditions that exist for other reasons: the change fires on the
    // crossing, and the pulse keeps firing while the direction is held.
    #[test]
    fn a_change_and_a_pulse_together_are_auto_repeat() {
        use ConditionState::{Building, Satisfied};

        let conditions = alloc::vec![
            BindingCondition::Change,
            BindingCondition::Pulse {
                interval: TICK * 3.0,
                immediate: false,
            },
        ];
        let mut scratch = alloc::vec![Scratch::default(); conditions.len()];
        let held = ActionValue::Axis1(1.0);

        let states: Vec<_> = (0..6)
            .map(|_| combine(&conditions, held, &mut scratch, TICK))
            .collect();
        // The change fires on the crossing; the pulse's clock starts on that same tick, so the
        // first repeat lands one interval after it and every one thereafter is evenly spaced.
        assert_eq!(
            states,
            [
                Satisfied, Building, Satisfied, Building, Building, Satisfied
            ]
        );
    }

    // Hold and multi-tap, the two conditions that need a caption of their own, and a bare press,
    // which reads the same as no condition and so has nothing to add.
    #[test]
    fn describing_finds_the_hold_or_the_multi_tap() {
        assert_eq!(
            describe(&[BindingCondition::Hold {
                duration: 0.75,
                one_shot: false,
            }]),
            ConditionDescriptor::Hold { duration: 0.75 }
        );
        assert_eq!(
            describe(&[BindingCondition::MultiTap {
                count: 2,
                max_gap: 0.3,
            }]),
            ConditionDescriptor::MultiTap { count: 2 }
        );
        assert_eq!(
            describe(&[BindingCondition::Press]),
            ConditionDescriptor::None
        );
        assert_eq!(describe(&[]), ConditionDescriptor::None);
    }

    // `HoldAndRelease` still asks the player to hold the control, even though what fires is the
    // release — the caption is the same one `Hold` gets.
    #[test]
    fn hold_and_release_reads_as_a_hold() {
        assert_eq!(
            describe(&[BindingCondition::HoldAndRelease { duration: 0.5 }]),
            ConditionDescriptor::Hold { duration: 0.5 }
        );
    }

    // The whole formula rather than a qualifier on its own: a bare "Hold" means nothing to a
    // player who has not already read the control it modifies.
    #[test]
    fn the_fallback_renderer_names_the_control_every_time() {
        assert_eq!(ConditionDescriptor::None.fallback_format("W"), "W");
        assert_eq!(
            ConditionDescriptor::Hold { duration: 0.75 }.fallback_format("W"),
            "Hold W"
        );
        assert_eq!(
            ConditionDescriptor::MultiTap { count: 2 }.fallback_format("Space"),
            "Space \u{d7}2"
        );
    }
}
