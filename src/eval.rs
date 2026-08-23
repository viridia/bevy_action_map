//! The evaluator: a plan and an input frame in, action state and a transition log out.
//!
//! The evaluator resolves bindings and emits a transition log for later dispatch.

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Query, Res};
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

/// One phase change, in the order it happened.
///
/// The log records transitions rather than final state, which is the whole point: an action that
/// fires and completes inside one tick has two of these, and a reader that only ever sees the
/// current phase cannot express that.
pub(crate) struct Transition {
    pub(crate) slot: usize,
    pub(crate) phase: Phase,
    pub(crate) value: ActionValue,
}

/// Turns each logged transition into its typed event.
///
/// Separate from evaluation because observers run arbitrary code with `&mut World`, and the
/// evaluator has to stay a pure function of its inputs (R10.2).
pub fn dispatch_transitions<C: InputContext + Component>(
    mut commands: Commands<'_, '_>,
    mut states: Query<'_, '_, (Entity, &mut InputContextState<C>)>,
) {
    for (entity, mut state) in &mut states {
        if state.transitions.is_empty() {
            continue;
        }

        // Taken rather than borrowed so the plan stays readable while dispatching, and handed back
        // afterwards so the allocation survives to the next tick.
        let mut log = core::mem::take(&mut state.transitions);
        for transition in log.drain(..) {
            let dispatch = state.plan.dispatch_for_slot(transition.slot);
            dispatch(&mut commands, entity, transition.phase, transition.value);
        }
        state.transitions = log;
    }
}

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

/// Which half of the plan a fold pass is for.
///
/// The two kinds of source have different temporal semantics, and the split is what lets a fast tap
/// be seen without disturbing a mouse delta.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fold {
    /// Controls with a value at every instant — buttons, axes, sticks. Sampled at each change, so a
    /// press and a release inside one window are two separate readings.
    Level,
    /// Controls with no value at an instant, only a total over an interval — mouse motion. Summed
    /// across the whole window and read once, because half of a movement is not a position.
    Delta,
}

impl<C: InputContext> InputContextState<C> {
    pub(crate) fn apply_frame(&mut self, frame: &InputFrame, threshold: &ButtonThreshold) {
        // Only what has arrived since this context last looked. Re-reading the whole queue is what
        // made one mouse delta count three times across three fixed ticks.
        let unread = frame.events_after(self.read_through);
        if let Some(last) = unread.last() {
            self.read_through = Some(last.timestamp);
        }

        let mut mouse_delta = Vec2::ZERO;
        let mut level_changes = 0usize;

        // Replayed one at a time rather than collapsed. Draining the whole window and then folding
        // once is what made a press and release inside a single window vanish: the two cancel in
        // the held state, and the fold sees nothing happen (R9.3).
        for event in unread {
            if let RawEvent::MouseMotion(delta) = &event.event {
                mouse_delta += *delta;
                continue;
            }
            self.apply_level_event(&event.event, threshold);
            self.fold(threshold, Vec2::ZERO, Fold::Level);
            level_changes += 1;
        }

        // Time passes even when nothing arrives: a phase has to reach `Ongoing` from `Fired` on its
        // own, and without an event to prompt it nothing else would.
        if level_changes == 0 {
            self.fold(threshold, Vec2::ZERO, Fold::Level);
        }

        self.fold(threshold, mouse_delta, Fold::Delta);
    }

    /// Moves one control's held state, for the sources that have a state to hold.
    fn apply_level_event(&mut self, event: &RawEvent, threshold: &ButtonThreshold) {
        #[cfg(not(feature = "gamepad"))]
        let _ = threshold;

        match event {
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
            // Accumulated by the caller: a delta is not a state.
            RawEvent::MouseMotion(_) => {}
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

    /// Resolves one half of the plan against the current device state.
    fn fold(&mut self, threshold: &ButtonThreshold, mouse_delta: Vec2, kind: Fold) {
        // Field-level borrows: the fold reads the device state and the plan while writing actions.
        let Self {
            plan,
            actions,
            transitions,
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

            // A slot belongs to exactly one half, and `Intent::accepts` is what guarantees it: a
            // `Delta2` action admits only delta-shaped sources and every other intent admits none,
            // so no slot can want both passes.
            let wanted = match kind {
                Fold::Delta => intent == Intent::Delta2,
                Fold::Level => intent != Intent::Delta2,
            };
            if !wanted {
                while index < bindings.len() && bindings[index].slot == slot {
                    index += 1;
                }
                continue;
            }

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
                    BindingSource::Axis1(parts) => ActionValue::Axis1(axis_from_buttons(
                        is_pressed(parts.negative),
                        is_pressed(parts.positive),
                    )),
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
                let phase = update_action_state(&mut actions[slot], value);
                // Only the edges. `Idle` and `Ongoing` say that nothing changed, and an observer
                // firing every tick for a held button would be noise rather than information.
                if matches!(phase, Phase::Fired | Phase::Completed | Phase::Canceled) {
                    transitions.push(Transition { slot, phase, value });
                }
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

fn update_action_state(action_state: &mut crate::action::ActionState, value: ActionValue) -> Phase {
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
    action_state.phase
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{InputAction, TickDomain};
    use crate::binding::InputContextBuilder;
    use crate::plan::Plan;
    use alloc::vec::Vec;
    use bevy_platform::sync::Arc;

    struct Flying;

    impl InputContext for Flying {
        const TICK: TickDomain = TickDomain::Fixed;
        const PRIORITY: i32 = 0;
        const PATH: &'static str = "eval_tests.flying";
    }

    struct Jump;

    impl InputAction for Jump {
        type Output = bool;

        const INTENT: Intent = Intent::Button;
        const PATH: &'static str = "eval_tests.jump";
    }

    /// The log holds transitions, not state. A key that is still down is not news, and if held
    /// actions logged an entry per tick the log would grow with the number of things a player is
    /// holding rather than with the number of things they did.
    ///
    /// Asserted against the log itself rather than against observers, because dispatch drops
    /// non-edges on its way out and would hide a log that recorded them.
    #[cfg(feature = "keyboard")]
    #[test]
    fn the_log_records_edges_and_not_held_state() {
        use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};

        fn key(state: ButtonState) -> RawEvent {
            RawEvent::Keyboard(KeyboardInput {
                key_code: KeyCode::Space,
                logical_key: Key::Space,
                state,
                text: None,
                repeat: false,
                window: bevy_ecs::entity::Entity::PLACEHOLDER,
            })
        }

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(KeyCode::Space);
        let plan = Arc::new(Plan::from_bindings(builder.finish()));
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();

        let mut frame = InputFrame::default();
        frame.record(key(ButtonState::Pressed));
        state.apply_frame(&frame, &threshold);
        assert_eq!(state.transitions.len(), 1);
        assert_eq!(state.transitions[0].phase, Phase::Fired);

        // Dispatch would have drained it by now.
        state.transitions.clear();

        // Nothing new arrives; the key is still down.
        state.apply_frame(&frame, &threshold);
        assert!(
            state.transitions.is_empty(),
            "a held key logged {:?}",
            state
                .transitions
                .iter()
                .map(|t| t.phase)
                .collect::<Vec<_>>()
        );

        frame.record(key(ButtonState::Released));
        state.apply_frame(&frame, &threshold);
        assert_eq!(state.transitions.len(), 1);
        assert_eq!(state.transitions[0].phase, Phase::Completed);
    }

    /// R9.3's other half. A player who taps faster than the tick rate still tapped, and collapsing
    /// the window to its final state loses the whole event: press and release cancel in the held
    /// state, and a single fold afterwards sees nothing happen at all.
    ///
    /// Polling cannot express this — one `Phase` per read — which is why the log exists.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_tap_inside_one_window_is_two_transitions() {
        use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};

        fn key(state: ButtonState) -> RawEvent {
            RawEvent::Keyboard(KeyboardInput {
                key_code: KeyCode::Space,
                logical_key: Key::Space,
                state,
                text: None,
                repeat: false,
                window: bevy_ecs::entity::Entity::PLACEHOLDER,
            })
        }

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(KeyCode::Space);
        let plan = Arc::new(Plan::from_bindings(builder.finish()));
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();

        let mut frame = InputFrame::default();
        frame.record(key(ButtonState::Pressed));
        frame.record(key(ButtonState::Released));

        state.apply_frame(&frame, &threshold);

        let phases: Vec<_> = state.transitions.iter().map(|t| t.phase).collect();
        assert_eq!(phases, [Phase::Fired, Phase::Completed]);

        // And the poll agrees with where the tick ended, which is the key back up.
        assert_eq!(state.phase::<Jump>(), Phase::Completed);
        assert!(!state.value::<Jump>());
    }

    /// The other side of the split. A delta has no value at an instant, so several motions inside
    /// one window are one movement and not several: they sum, and the action transitions once.
    #[test]
    fn several_motions_inside_one_window_are_one_transition() {
        struct Look;

        impl InputAction for Look {
            type Output = Vec2;

            const INTENT: Intent = Intent::Delta2;
            const PATH: &'static str = "eval_tests.look";
        }

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Look>(crate::binding::MouseMove);
        let plan = Arc::new(Plan::from_bindings(builder.finish()));
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();

        let mut frame = InputFrame::default();
        frame.record(RawEvent::MouseMotion(Vec2::new(3.0, 0.0)));
        frame.record(RawEvent::MouseMotion(Vec2::new(1.0, -2.0)));

        state.apply_frame(&frame, &threshold);

        let phases: Vec<_> = state.transitions.iter().map(|t| t.phase).collect();
        assert_eq!(phases, [Phase::Fired], "one movement, one transition");
        assert_eq!(state.value::<Look>(), Vec2::new(4.0, -2.0), "summed");
    }
}
