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
//! Conditions come in three kinds, and they compose the way Unreal's triggers do because that
//! formulation is the clearest one available:
//!
//! - **Explicit** — if a binding has any, at least one must be satisfied.
//! - **Implicit** — every one must be satisfied.
//! - **Blocking** — if any is satisfied, the binding does not fire at all.
//!
//! So `.press()` and `.hold(0.5)` together mean "either a press or a long hold", while a blocking
//! condition vetoes regardless of what the others said.

use alloc::boxed::Box;

use crate::action::{ActionValue, Scratch};

/// What a condition says about its binding this tick.
///
/// Ordered, because several bindings can feed one action and the most definite of them decides: a
/// binding that fired outranks one still building, which outranks one with nothing to say.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    /// Not satisfied, and nothing in progress.
    Idle,
    /// On the way — a hold that has not lasted long enough yet, a tap waiting for its second press.
    ///
    /// Reported so that an action can show a charge meter, and so that giving up part way through
    /// is [`Canceled`](crate::action::Phase::Canceled) rather than silence.
    Ongoing,
    /// Satisfied. The binding fires.
    Fired,
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
    fn evaluate(&self, value: ActionValue, scratch: &mut Scratch, delta: f32) -> Verdict;

    /// How this condition combines with the others on its binding.
    fn kind(&self) -> ConditionKind {
        ConditionKind::Explicit
    }
}

/// The built-in conditions.
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
    /// Calls an application-defined condition.
    Custom(Box<dyn Condition>),
}

/// Bit positions within [`Scratch::flags`](crate::action::Scratch::flags).
const HELD: u8 = 1 << 0;
const DONE: u8 = 1 << 1;

impl BindingCondition {
    /// Decides what this condition makes of the binding's value this tick.
    pub fn evaluate(&self, value: ActionValue, scratch: &mut Scratch, delta: f32) -> Verdict {
        let actuated = value.to_bool();
        let was = scratch.prev.to_bool();
        scratch.prev = ActionValue::Bool(actuated);

        match self {
            Self::Press => verdict(actuated && !was),
            Self::Release => verdict(!actuated && was),
            Self::Down => verdict(actuated),

            Self::Hold { duration, one_shot } => {
                if !actuated {
                    scratch.time = 0.0;
                    scratch.flags &= !DONE;
                    return Verdict::Idle;
                }
                scratch.time += delta;
                if scratch.time < *duration {
                    return Verdict::Ongoing;
                }
                if *one_shot {
                    if scratch.flags & DONE != 0 {
                        return Verdict::Idle;
                    }
                    scratch.flags |= DONE;
                }
                Verdict::Fired
            }

            Self::HoldAndRelease { duration } => {
                if actuated {
                    scratch.time += delta;
                    return Verdict::Ongoing;
                }
                // The release is the fire, so the timer has to be read before it is cleared.
                let long_enough = was && scratch.time >= *duration;
                scratch.time = 0.0;
                verdict(long_enough)
            }

            Self::Tap { max_duration } => {
                if actuated {
                    scratch.time += delta;
                    return Verdict::Ongoing;
                }
                let quick_enough = was && scratch.time <= *max_duration;
                scratch.time = 0.0;
                verdict(quick_enough)
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
                        return Verdict::Fired;
                    }
                }

                if scratch.count > 0 || actuated {
                    Verdict::Ongoing
                } else {
                    Verdict::Idle
                }
            }

            Self::Pulse {
                interval,
                immediate,
            } => {
                if !actuated {
                    scratch.time = 0.0;
                    scratch.flags &= !DONE;
                    return Verdict::Idle;
                }
                if scratch.flags & DONE == 0 {
                    scratch.flags |= DONE;
                    if *immediate {
                        return Verdict::Fired;
                    }
                }
                scratch.time += delta;
                if scratch.time >= *interval {
                    scratch.time -= *interval;
                    return Verdict::Fired;
                }
                Verdict::Ongoing
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

fn verdict(fired: bool) -> Verdict {
    if fired { Verdict::Fired } else { Verdict::Idle }
}

/// Folds every condition on one binding into a single answer, per the rules on this module.
///
/// With no conditions at all, a binding fires whenever its control is off rest — which is what a
/// binding does before anyone asks for anything more specific.
pub(crate) fn combine(
    conditions: &[BindingCondition],
    value: ActionValue,
    scratch: &mut [Scratch],
    delta: f32,
) -> Verdict {
    if conditions.is_empty() {
        return verdict(value.to_bool());
    }

    let mut explicit = 0usize;
    let mut explicit_fired = false;
    let mut implicit_all = true;
    let mut ongoing = false;
    let mut blocked = false;

    for (condition, scratch) in conditions.iter().zip(scratch) {
        let outcome = condition.evaluate(value, scratch, delta);
        match condition.kind() {
            ConditionKind::Explicit => {
                explicit += 1;
                match outcome {
                    Verdict::Fired => explicit_fired = true,
                    Verdict::Ongoing => ongoing = true,
                    Verdict::Idle => {}
                }
            }
            ConditionKind::Implicit => {
                if outcome != Verdict::Fired {
                    implicit_all = false;
                }
                if outcome == Verdict::Ongoing {
                    ongoing = true;
                }
            }
            // A blocker's own progress is nobody's business; only whether it vetoes.
            ConditionKind::Blocking => blocked |= outcome == Verdict::Fired,
        }
    }

    if blocked {
        return Verdict::Idle;
    }
    if implicit_all && (explicit == 0 || explicit_fired) {
        return Verdict::Fired;
    }
    if ongoing {
        return Verdict::Ongoing;
    }
    Verdict::Idle
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::vec::Vec;

    const TICK: f32 = 0.1;

    /// Drives one condition through a script of "is the control down this tick", and reports what
    /// it said each time. Every duration below is a multiple of `TICK`, so the arithmetic is exact.
    fn run(condition: &BindingCondition, script: &[bool]) -> Vec<Verdict> {
        let mut scratch = Scratch::default();
        script
            .iter()
            .map(|down| condition.evaluate(ActionValue::Bool(*down), &mut scratch, TICK))
            .collect()
    }

    #[test]
    fn press_and_release_are_edges() {
        use Verdict::{Fired, Idle};

        assert_eq!(
            run(&BindingCondition::Press, &[false, true, true, false, true]),
            [Idle, Fired, Idle, Idle, Fired]
        );
        assert_eq!(
            run(
                &BindingCondition::Release,
                &[false, true, true, false, false]
            ),
            [Idle, Idle, Idle, Fired, Idle]
        );
    }

    #[test]
    fn a_hold_reports_progress_then_fires_and_keeps_firing() {
        use Verdict::{Fired, Idle, Ongoing};

        let hold = BindingCondition::Hold {
            duration: 0.25,
            one_shot: false,
        };
        // Down for three ticks reaches 0.3, so the third crosses the line.
        assert_eq!(
            run(&hold, &[true, true, true, true, false]),
            [Ongoing, Ongoing, Fired, Fired, Idle]
        );
    }

    #[test]
    fn a_one_shot_hold_fires_exactly_once_per_press() {
        use Verdict::{Fired, Idle, Ongoing};

        let hold = BindingCondition::Hold {
            duration: 0.25,
            one_shot: true,
        };
        assert_eq!(
            run(&hold, &[true, true, true, true, false, true, true, true]),
            [Ongoing, Ongoing, Fired, Idle, Idle, Ongoing, Ongoing, Fired]
        );
    }

    /// The distinction that makes a hold worth having: letting go early has to be visibly different
    /// from letting go late, and neither may look like nothing happened.
    #[test]
    fn hold_and_release_fires_only_when_the_hold_was_long_enough() {
        use Verdict::{Fired, Idle, Ongoing};

        let condition = BindingCondition::HoldAndRelease { duration: 0.25 };
        assert_eq!(
            run(&condition, &[true, true, true, false]),
            [Ongoing, Ongoing, Ongoing, Fired],
            "held long enough, then released"
        );
        assert_eq!(
            run(&condition, &[true, false]),
            [Ongoing, Idle],
            "let go too early"
        );
    }

    #[test]
    fn a_tap_is_a_press_that_did_not_last() {
        use Verdict::{Fired, Idle, Ongoing};

        let tap = BindingCondition::Tap { max_duration: 0.25 };
        assert_eq!(run(&tap, &[true, false]), [Ongoing, Fired], "quick enough");
        assert_eq!(
            run(&tap, &[true, true, true, true, false]),
            [Ongoing, Ongoing, Ongoing, Ongoing, Idle],
            "held far too long to be a tap"
        );
    }

    #[test]
    fn a_double_tap_needs_both_taps_inside_the_window() {
        use Verdict::{Fired, Idle, Ongoing};

        let double = BindingCondition::MultiTap {
            count: 2,
            max_gap: 0.25,
        };
        assert_eq!(
            run(&double, &[true, false, true, false]),
            [Ongoing, Ongoing, Ongoing, Fired]
        );

        // The same two taps with a long enough wait between them never make a double-tap. The
        // second one is not wasted — it begins a fresh sequence, which is why the tail is `Ongoing`
        // rather than `Idle`.
        let dawdled = run(&double, &[true, false, false, false, false, true, false]);
        assert!(
            !dawdled.contains(&Fired),
            "two taps a window apart fired anyway: {dawdled:?}"
        );
        assert_eq!(dawdled[4], Idle, "the first sequence lapsed");
    }

    #[test]
    fn a_pulse_repeats_while_the_control_is_held() {
        use Verdict::{Fired, Idle, Ongoing};

        let pulse = BindingCondition::Pulse {
            interval: 0.2,
            immediate: true,
        };
        assert_eq!(
            run(&pulse, &[true, true, true, true, true, false]),
            [Fired, Ongoing, Fired, Ongoing, Fired, Idle]
        );
    }

    /// R6.2's three-way split. Each kind is checked for the thing only it can do.
    #[test]
    fn the_three_kinds_compose_as_documented() {
        struct Always(Verdict, ConditionKind);

        impl Condition for Always {
            fn evaluate(&self, _: ActionValue, _: &mut Scratch, _: f32) -> Verdict {
                self.0
            }
            fn kind(&self) -> ConditionKind {
                self.1
            }
        }

        fn verdict_of(conditions: Vec<BindingCondition>) -> Verdict {
            let mut scratch = alloc::vec![Scratch::default(); conditions.len()];
            combine(&conditions, ActionValue::Bool(true), &mut scratch, TICK)
        }

        let explicit = |v| BindingCondition::Custom(Box::new(Always(v, ConditionKind::Explicit)));
        let implicit = |v| BindingCondition::Custom(Box::new(Always(v, ConditionKind::Implicit)));
        let blocking = |v| BindingCondition::Custom(Box::new(Always(v, ConditionKind::Blocking)));

        // No conditions at all: the control being off rest is the whole test.
        assert_eq!(verdict_of(Vec::new()), Verdict::Fired);

        // Explicit: any one is enough.
        assert_eq!(
            verdict_of(alloc::vec![
                explicit(Verdict::Idle),
                explicit(Verdict::Fired)
            ]),
            Verdict::Fired
        );
        // Implicit: all of them, or none of it.
        assert_eq!(
            verdict_of(alloc::vec![
                implicit(Verdict::Fired),
                implicit(Verdict::Idle)
            ]),
            Verdict::Idle
        );
        // Blocking: a veto beats everything the others agreed on.
        assert_eq!(
            verdict_of(alloc::vec![
                explicit(Verdict::Fired),
                blocking(Verdict::Fired)
            ]),
            Verdict::Idle
        );
        // And progress survives to be reported when nothing has fired yet.
        assert_eq!(
            verdict_of(alloc::vec![explicit(Verdict::Ongoing)]),
            Verdict::Ongoing
        );
    }
}
