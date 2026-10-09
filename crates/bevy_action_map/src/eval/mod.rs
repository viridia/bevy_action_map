//! The evaluator: a plan and an input frame in, action state and a transition log out.

mod action_commit;
mod binding_pipeline;
mod binding_reading;
mod consumed_controls;
mod context_systems;
mod exclusion_ceiling;
mod held_control_state;
mod tick_evaluation;

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
pub(crate) use binding_reading::chord_held;
pub use consumed_controls::ConsumedControls;
pub(crate) use consumed_controls::{release_consumed_controls, release_consumed_in};
pub(crate) use context_systems::{
    ClassFire, Transition, dispatch_class_fires, dispatch_transitions, evaluate_context,
};
pub(crate) use exclusion_ceiling::{ExclusionCeiling, reset_exclusion_ceiling};
pub(crate) use held_control_state::HeldControlState;
pub(crate) use tick_evaluation::BindingProgress;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{
        ActionIntent, ActionPhase, ActionValue, InputAction, InputContext, TickDomain,
    };
    use crate::backend::{Authority, AuthorityValues};
    #[cfg(feature = "gamepad")]
    use crate::binding::Stick;
    use crate::binding::{ButtonThreshold, Control, InputContextBuilder};
    use crate::context::InputContextState;
    use crate::device::DeviceFamily;
    use crate::frame::{InputFrame, RawEvent};
    use crate::plan::Plan;
    use alloc::vec::Vec;
    #[cfg(any(feature = "keyboard", feature = "mouse"))]
    use bevy_input::ButtonState;
    #[cfg(feature = "gamepad")]
    use bevy_input::gamepad::{GamepadAxis, RawGamepadEvent};
    use bevy_math::Vec2;
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

        const INTENT: ActionIntent = ActionIntent::Button;
        const PATH: &'static str = "eval_tests.jump";
    }

    struct Serve;

    impl InputAction for Serve {
        type Output = bool;

        const INTENT: ActionIntent = ActionIntent::Button;
        const PATH: &'static str = "eval_tests.serve";
    }

    /// A plausible fixed timestep, for the tests that do not care what it is.
    const TICK: f32 = 1.0 / 64.0;

    /// A context with `Serve` bound to an authority standing in for the pad, and nothing else.
    fn authority_context() -> InputContextState<Flying> {
        context_declaring(|controls| {
            controls.bind::<Serve>(Authority(DeviceFamily::Gamepad));
        })
    }

    /// One tick with no device input, in the order `evaluate_context` runs it: the authority is
    /// sampled first, then the frame applied.
    fn tick(state: &mut InputContextState<Flying>, values: Option<&AuthorityValues>) {
        if state.is_active() {
            state.sample_authority(values);
        }
        state.apply_frame(
            &InputFrame::default(),
            &ButtonThreshold::default(),
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
    }

    /// An authority hands over a level and never an edge — Steam's `GetDigitalActionData` is
    /// sampled when asked, and reports no press or release of its own. The state machine is what
    /// turns that level moving into `Fired` and `Completed`, so gameplay code reads the action
    /// exactly as it reads one bound to a control.
    #[test]
    fn an_authority_supplies_a_level_and_the_state_machine_makes_the_edges() {
        let mut state = authority_context();
        let mut values = AuthorityValues::new();
        values.set::<Serve>(false);

        tick(&mut state, Some(&values));
        assert!(state.transitions.is_empty(), "rest is not an edge");

        values.set::<Serve>(true);
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Fired);
        assert!(state.value::<Serve>());

        // Dispatch would have drained it by now.
        state.transitions.clear();

        tick(&mut state, Some(&values));
        assert!(
            state.transitions.is_empty(),
            "a level that has not moved is not news"
        );

        values.set::<Serve>(false);
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Completed);
    }

    /// Reading rest rather than refusing is what lets a context be spawned before whatever drives
    /// it exists, and what an authority that has lost its device says by clearing the action.
    #[test]
    fn an_authority_binding_with_no_values_reads_at_rest() {
        let mut state = authority_context();
        let mut values = AuthorityValues::new();

        tick(&mut state, None);
        assert!(!state.value::<Serve>());
        assert!(state.transitions.is_empty());

        values.set::<Serve>(false);
        tick(&mut state, Some(&values));
        values.set::<Serve>(true);
        tick(&mut state, Some(&values));
        values.set::<Serve>(false);
        tick(&mut state, Some(&values));

        values.clear::<Serve>();
        tick(&mut state, Some(&values));
        assert!(!state.value::<Serve>());
        assert_eq!(state.phase::<Serve>(), ActionPhase::Idle);
    }

    /// Clearing an action the player is holding is the backend losing the device, not the player
    /// letting go, and ends it as a pad disconnecting does: `Canceled` rather than `Completed`. So
    /// does the component going away.
    #[test]
    fn an_authority_that_stops_supplying_a_held_action_cancels_it() {
        let mut state = authority_context();
        let mut values = AuthorityValues::new();
        values.set::<Serve>(false);
        tick(&mut state, Some(&values));
        values.set::<Serve>(true);
        tick(&mut state, Some(&values));
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Firing);
        state.transitions.clear();

        values.clear::<Serve>();
        tick(&mut state, Some(&values));
        let phases: Vec<_> = state.transitions.iter().map(|t| t.phase).collect();
        assert_eq!(phases, [ActionPhase::Canceled]);
        assert_eq!(state.phase::<Serve>(), ActionPhase::Canceled);

        values.set::<Serve>(false);
        tick(&mut state, Some(&values));
        values.set::<Serve>(true);
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Fired);

        state.transitions.clear();

        tick(&mut state, None);
        let phases: Vec<_> = state.transitions.iter().map(|t| t.phase).collect();
        assert_eq!(phases, [ActionPhase::Canceled]);
        assert_eq!(state.phase::<Serve>(), ActionPhase::Canceled);
    }

    /// The same guard a bound action gets (R7.5), on the authority's values: a menu closing while
    /// the backend still reports its button down must not read as a fresh press underneath.
    #[test]
    fn an_authority_held_across_activation_waits_for_rest() {
        let mut state = authority_context();
        let mut values = AuthorityValues::new();
        values.set::<Serve>(true);

        state.deactivate();
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Idle);

        state.activate();
        tick(&mut state, Some(&values));
        assert_eq!(
            state.phase::<Serve>(),
            ActionPhase::Idle,
            "a value held across activation is not a press"
        );

        values.set::<Serve>(false);
        tick(&mut state, Some(&values));
        values.set::<Serve>(true);
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Fired);
    }

    /// A disabled action does not listen to its authority either, and comes back under the same
    /// guard as one bound to a control.
    #[test]
    fn a_disabled_action_ignores_its_authority() {
        let mut state = authority_context();
        let mut values = AuthorityValues::new();
        values.set::<Serve>(true);

        state.disable::<Serve>();
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Idle);

        state.enable::<Serve>();
        tick(&mut state, Some(&values));
        assert_eq!(
            state.phase::<Serve>(),
            ActionPhase::Idle,
            "a value held across the enable is not a press"
        );
    }

    /// The keyboard beside a pad the authority drives: either one holds the action, and it
    /// completes only when both have let go.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_key_and_an_authority_on_one_action_combine() {
        use bevy_input::keyboard::KeyCode;

        let mut state = context_declaring(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
            controls.bind::<Jump>(Authority(DeviceFamily::Gamepad));
        });
        let mut values = AuthorityValues::new();
        let mut frame = InputFrame::default();

        values.set::<Jump>(true);
        state.held.authority.hold(Some(&values));
        press(&mut state, &mut frame, key(ButtonState::Pressed));
        assert_eq!(state.phase::<Jump>(), ActionPhase::Fired);

        values.set::<Jump>(false);
        tick(&mut state, Some(&values));
        assert!(state.value::<Jump>(), "the key still holds it");

        press(&mut state, &mut frame, key(ButtonState::Released));
        assert_eq!(state.phase::<Jump>(), ActionPhase::Completed);
    }

    /// The authority losing its device leaves the keyboard alone, as a pad disconnecting does: the
    /// key still holds the action, and releasing it later is an ordinary `Completed`.
    #[cfg(feature = "keyboard")]
    #[test]
    fn an_authority_going_unsupplied_leaves_a_held_key_holding() {
        use bevy_input::keyboard::KeyCode;

        let mut state = context_declaring(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
            controls.bind::<Jump>(Authority(DeviceFamily::Gamepad));
        });
        let mut values = AuthorityValues::new();
        let mut frame = InputFrame::default();

        values.set::<Jump>(true);
        state.held.authority.hold(Some(&values));
        press(&mut state, &mut frame, key(ButtonState::Pressed));
        state.transitions.clear();

        values.clear::<Jump>();
        tick(&mut state, Some(&values));
        assert!(state.value::<Jump>(), "the key still holds it");
        assert_eq!(state.phase::<Jump>(), ActionPhase::Firing);
        assert!(state.transitions.is_empty());

        press(&mut state, &mut frame, key(ButtonState::Released));
        assert_eq!(state.phase::<Jump>(), ActionPhase::Completed);
    }

    /// A menu opened by a button spawns its context while the button is still down. A control
    /// pressed before the instance existed never reaches it as a press, and an authority's level
    /// is held over the same way: the new instance waits for a release before it fires, rather than
    /// closing the menu the press just opened.
    #[test]
    fn a_new_instance_holds_over_an_authority_already_held() {
        let mut state = authority_context();
        let mut values = AuthorityValues::new();

        values.set::<Serve>(true);
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Idle);

        values.set::<Serve>(false);
        tick(&mut state, Some(&values));
        values.set::<Serve>(true);
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Fired);
    }

    /// Steam switching action sets is the everyday case: a button that opened a menu means
    /// something else in the menu's set, and it arrives there already held. An action the authority
    /// starts supplying while held is held over, exactly as at an instance's first sample.
    #[test]
    fn an_authority_resuming_a_held_action_holds_it_over() {
        let mut state = authority_context();
        let mut values = AuthorityValues::new();
        values.set::<Serve>(false);
        tick(&mut state, Some(&values));

        values.set::<Serve>(true);
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Fired);
        values.set::<Serve>(false);
        tick(&mut state, Some(&values));

        // Not supplied for a tick, then supplied held.
        values.clear::<Serve>();
        tick(&mut state, Some(&values));
        values.set::<Serve>(true);
        tick(&mut state, Some(&values));
        assert_ne!(state.phase::<Serve>(), ActionPhase::Fired);

        values.set::<Serve>(false);
        tick(&mut state, Some(&values));
        values.set::<Serve>(true);
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Fired);
    }

    #[derive(crate::InputAction)]
    #[action(path = "eval_tests.throttle", output = f32, intent = Analog1)]
    struct Throttle;

    /// A follower rides its leader's authority as it rides a leader's control: the backend writes
    /// `Throttle` once, and `Serve` presses from that same value rather than waiting for one of
    /// its own.
    #[test]
    fn a_follower_reads_its_leaders_authority_value() {
        let mut state = context_declaring(|controls| {
            controls.bind::<Throttle>(Authority(DeviceFamily::Gamepad));
            controls.follow::<Serve, Throttle>(|binding| binding);
            assert!(
                controls.diagnostics().is_empty(),
                "{:?}",
                controls.diagnostics()
            );
        });
        let mut values = AuthorityValues::new();
        values.set::<Throttle>(0.0);
        tick(&mut state, Some(&values));

        values.set::<Throttle>(1.0);
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Fired);

        values.set::<Throttle>(0.0);
        tick(&mut state, Some(&values));
        assert_eq!(state.phase::<Serve>(), ActionPhase::Completed);
    }

    /// A condition the game declares runs on the authority's value like any other: a rate of fire
    /// is the game's rule, not the player's way of pressing, and holds under an authority too.
    #[test]
    fn a_pulse_on_an_authority_repeats_while_its_level_is_held() {
        let mut state = context_declaring(|controls| {
            controls
                .bind::<Serve>(Authority(DeviceFamily::Gamepad))
                .pulse(TICK * 4.0);
        });
        let mut values = AuthorityValues::new();
        values.set::<Serve>(false);
        tick(&mut state, Some(&values));
        values.set::<Serve>(true);

        let mut fired = 0;
        for _ in 0..12 {
            tick(&mut state, Some(&values));
            fired += state
                .transitions
                .drain(..)
                .filter(|transition| transition.phase == ActionPhase::Fired)
                .count();
        }
        assert!(fired >= 3, "held for three intervals, fired {fired} times");
    }

    #[cfg(feature = "keyboard")]
    fn key(state: ButtonState) -> RawEvent {
        use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};

        RawEvent::Keyboard(KeyboardInput {
            key_code: KeyCode::Space,
            logical_key: Key::Space,
            state,
            text: None,
            repeat: false,
            window: bevy_ecs::entity::Entity::PLACEHOLDER,
        })
    }

    /// A context with `Jump` on the space bar, and nothing else.
    #[cfg(feature = "keyboard")]
    fn jump_context() -> InputContextState<Flying> {
        use bevy_input::keyboard::KeyCode;

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(KeyCode::Space);
        InputContextState::<Flying>::new(
            Arc::new({
                let (bindings, class_bindings) = builder.finish();
                Plan::from_bindings(bindings, class_bindings)
            }),
            None,
        )
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
        let mut state = jump_context();
        let threshold = ButtonThreshold::default();

        let mut frame = InputFrame::default();
        frame.record(key(ButtonState::Pressed));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.transitions.len(), 1);
        assert_eq!(state.transitions[0].phase, ActionPhase::Fired);

        // Dispatch would have drained it by now.
        state.transitions.clear();

        // Nothing new arrives; the key is still down.
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
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
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.transitions.len(), 1);
        assert_eq!(state.transitions[0].phase, ActionPhase::Completed);
    }

    /// A player who taps faster than the tick rate still tapped. Polling cannot express that — one
    /// `ActionPhase` per read — which is why the log exists.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_tap_inside_one_window_is_two_transitions() {
        let mut state = jump_context();
        let threshold = ButtonThreshold::default();

        let mut frame = InputFrame::default();
        frame.record(key(ButtonState::Pressed));
        frame.record(key(ButtonState::Released));

        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );

        let phases: Vec<_> = state.transitions.iter().map(|t| t.phase).collect();
        assert_eq!(phases, [ActionPhase::Fired, ActionPhase::Completed]);

        // And the poll agrees with where the tick ended, which is the key back up.
        assert_eq!(state.phase::<Jump>(), ActionPhase::Completed);
        assert!(!state.value::<Jump>());
    }

    /// The other side of the split. A delta has no value at an instant, so several motions inside
    /// one window are one movement and not several: they sum, and the action transitions once.
    #[test]
    fn several_motions_inside_one_window_are_one_transition() {
        struct Look;

        impl InputAction for Look {
            type Output = Vec2;

            const INTENT: ActionIntent = ActionIntent::Delta2;
            const PATH: &'static str = "eval_tests.look";
        }

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Look>(crate::binding::MouseMove);
        let plan = Arc::new({
            let (bindings, class_bindings) = builder.finish();
            Plan::from_bindings(bindings, class_bindings)
        });
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();

        let mut frame = InputFrame::default();
        frame.record(RawEvent::MouseMotion(Vec2::new(3.0, 0.0)));
        frame.record(RawEvent::MouseMotion(Vec2::new(1.0, -2.0)));

        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );

        let phases: Vec<_> = state.transitions.iter().map(|t| t.phase).collect();
        assert_eq!(phases, [ActionPhase::Fired], "one movement, one transition");
        assert_eq!(state.value::<Look>(), Vec2::new(4.0, -2.0), "summed");
    }

    /// Closing a menu with the same key that interacts with the world must not interact the instant
    /// the menu disappears. The key is still down, and
    /// a context that started reading it now would see a press that the player made for the menu.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_context_activating_ignores_a_control_already_held() {
        let mut state = jump_context();
        let threshold = ButtonThreshold::default();
        let mut frame = InputFrame::default();

        state.deactivate();

        // The player presses the key while the context is not listening.
        frame.record(key(ButtonState::Pressed));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(
            state.phase::<Jump>(),
            ActionPhase::Idle,
            "inactive contexts do not fire"
        );

        state.activate();
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(
            state.phase::<Jump>(),
            ActionPhase::Idle,
            "a key held across activation is not a press"
        );
        assert!(state.transitions.is_empty());

        // Letting go arms it again without firing anything.
        frame.record(key(ButtonState::Released));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.phase::<Jump>(), ActionPhase::Idle);
        assert!(state.transitions.is_empty());

        // And now a real press is a real press.
        frame.record(key(ButtonState::Pressed));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.phase::<Jump>(), ActionPhase::Fired);
    }

    /// The opt-out, for a context taking over from one that was already driving the same control.
    #[cfg(feature = "keyboard")]
    #[test]
    fn activating_can_accept_a_control_already_held() {
        let mut state = jump_context();
        let threshold = ButtonThreshold::default();
        let mut frame = InputFrame::default();

        state.deactivate();
        frame.record(key(ButtonState::Pressed));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );

        state.activate_including_held();
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.phase::<Jump>(), ActionPhase::Fired);
    }

    /// An action interrupted by a context going away has to resolve: left as it was, a hold would
    /// still read as held for as long as the menu is up, and would never complete.
    #[cfg(feature = "keyboard")]
    #[test]
    fn deactivating_cancels_what_was_in_flight() {
        let mut state = jump_context();
        let threshold = ButtonThreshold::default();
        let mut frame = InputFrame::default();

        frame.record(key(ButtonState::Pressed));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.phase::<Jump>(), ActionPhase::Fired);
        state.transitions.clear();

        state.deactivate();

        let phases: Vec<_> = state.transitions.iter().map(|t| t.phase).collect();
        assert_eq!(phases, [ActionPhase::Canceled]);
        assert_eq!(state.phase::<Jump>(), ActionPhase::Canceled);
        assert!(!state.value::<Jump>(), "and it is no longer held");
    }

    /// Nothing in flight, nothing to cancel — deactivating an idle context is silent.
    #[cfg(feature = "keyboard")]
    #[test]
    fn deactivating_an_idle_context_says_nothing() {
        let mut state = jump_context();
        state.deactivate();
        assert!(state.transitions.is_empty());
    }

    /// Losing focus while a button is down must not read as the player finishing it — that would
    /// let alt-tab complete a hold-to-fire action for free. It resolves as an interruption instead,
    /// the same `Canceled` transition `deactivate` already uses — not the `Completed` an ordinary
    /// release produces (`the_log_records_edges_and_not_held_state`, above).
    #[cfg(feature = "keyboard")]
    #[test]
    fn focus_loss_cancels_what_a_release_would_have_completed() {
        let mut state = jump_context();
        let threshold = ButtonThreshold::default();
        let mut frame = InputFrame::default();

        frame.record(key(ButtonState::Pressed));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.phase::<Jump>(), ActionPhase::Fired);
        state.transitions.clear();

        frame.record(RawEvent::FocusLost);
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );

        let phases: Vec<_> = state.transitions.iter().map(|t| t.phase).collect();
        assert_eq!(
            phases,
            [ActionPhase::Canceled],
            "not Completed: nothing was let go"
        );
        assert!(!state.value::<Jump>());
    }

    /// A higher context claiming a held key takes it away without the player letting go, so what
    /// the key was firing here is canceled, as focus loss cancels it (D94).
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_claim_arriving_cancels_what_it_took() {
        use bevy_input::keyboard::KeyCode;

        let mut state = jump_context();
        let threshold = ButtonThreshold::default();
        let mut frame = InputFrame::default();

        frame.record(key(ButtonState::Pressed));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.phase::<Jump>(), ActionPhase::Fired);
        state.transitions.clear();

        let mut consumed = ConsumedControls::default();
        consumed.claim::<bevy_app::PreUpdate>(
            Control::PhysicalKey(KeyCode::Space),
            None,
            "eval_tests.vehicle",
        );
        state.apply_frame(&frame, &threshold, TICK, &consumed, &mut Vec::new(), None);

        let phases: Vec<_> = state.transitions.iter().map(|t| t.phase).collect();
        assert_eq!(
            phases,
            [ActionPhase::Canceled],
            "not Completed: nothing was let go"
        );
        state.transitions.clear();

        // Released under the claim: already canceled, so there is nothing left to end.
        frame.record(key(ButtonState::Released));
        state.apply_frame(&frame, &threshold, TICK, &consumed, &mut Vec::new(), None);
        assert!(state.transitions.is_empty());
    }

    /// A control still physically held when focus returns must not re-fire on its own. Bevy never
    /// resends the press that never released, so nothing here needs to re-arm anything — the fix is
    /// that no press event arrives at all, proven by the absence of a further transition even though
    /// the key is, by construction, still down.
    #[cfg(feature = "keyboard")]
    #[test]
    fn focus_loss_requires_a_fresh_press_before_refiring() {
        let mut state = jump_context();
        let threshold = ButtonThreshold::default();
        let mut frame = InputFrame::default();

        frame.record(key(ButtonState::Pressed));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        frame.record(RawEvent::FocusLost);
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        state.transitions.clear();

        // Focus returns; the key was never physically released, so no event says anything changed.
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert!(
            state.transitions.is_empty(),
            "a control focus already released must not refire on its own"
        );

        // Only an actual release-and-press cycle brings it back.
        frame.record(key(ButtonState::Released));
        frame.record(key(ButtonState::Pressed));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.phase::<Jump>(), ActionPhase::Fired);
    }

    /// The gamepad half of the same policy: a disconnect leaves no release event to clear a button
    /// still recorded down, so the crate has to notice the connection event itself and cancel what
    /// it was holding rather than leave it stuck.
    #[cfg(feature = "gamepad")]
    #[test]
    fn gamepad_disconnect_cancels_what_it_was_holding() {
        use bevy_input::gamepad::{GamepadButton, GamepadConnection, GamepadConnectionEvent};

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(GamepadButton::South);
        let plan = Arc::new({
            let (bindings, class_bindings) = builder.finish();
            Plan::from_bindings(bindings, class_bindings)
        });
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();
        let mut frame = InputFrame::default();

        frame.record(RawEvent::Gamepad(RawGamepadEvent::Button(
            bevy_input::gamepad::RawGamepadButtonChangedEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                GamepadButton::South,
                1.0,
            ),
        )));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.phase::<Jump>(), ActionPhase::Fired);
        state.transitions.clear();

        frame.record(RawEvent::Gamepad(RawGamepadEvent::Connection(
            GamepadConnectionEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                GamepadConnection::Disconnected,
            ),
        )));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );

        let phases: Vec<_> = state.transitions.iter().map(|t| t.phase).collect();
        assert_eq!(phases, [ActionPhase::Canceled]);
        assert!(!state.value::<Jump>());
    }

    /// Neither trigger reaches past the device it names. An action held through a surviving binding
    /// must survive — over-cancelling a keyboard-driven `Jump` because an unrelated gamepad
    /// disconnected would be as much a bug as leaving a stuck key would be.
    #[cfg(all(feature = "keyboard", feature = "gamepad"))]
    #[test]
    fn a_surviving_binding_is_untouched_by_the_others_device_going_away() {
        use bevy_input::gamepad::{GamepadButton, GamepadConnection, GamepadConnectionEvent};

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(bevy_input::keyboard::KeyCode::Space);
        builder.bind::<Jump>(GamepadButton::South);
        let plan = Arc::new({
            let (bindings, class_bindings) = builder.finish();
            Plan::from_bindings(bindings, class_bindings)
        });
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();
        let mut frame = InputFrame::default();

        // Held on the keyboard side only.
        frame.record(key(ButtonState::Pressed));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.phase::<Jump>(), ActionPhase::Fired);
        state.transitions.clear();

        // The pad going away must not touch it: nothing was ever held there.
        frame.record(RawEvent::Gamepad(RawGamepadEvent::Connection(
            GamepadConnectionEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                GamepadConnection::Disconnected,
            ),
        )));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert!(
            state.transitions.is_empty(),
            "an unrelated device disconnecting canceled a still-held action"
        );
        assert!(state.value::<Jump>(), "the key is still down");
    }

    /// A stick reports how fast, a mouse reports how far, and the two are only addable once the
    /// first has been multiplied by how long the tick was.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_rate_becomes_the_distance_it_covered_this_tick() {
        use crate::binding::{MouseMove, Stick};

        struct Look;

        impl InputAction for Look {
            type Output = Vec2;

            const INTENT: ActionIntent = ActionIntent::Delta2;
            const PATH: &'static str = "eval_tests.rate_look";
        }

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Look>(MouseMove);
        builder.bind::<Look>(Stick::Right).per_second(180.0);
        let plan = Arc::new({
            let (bindings, class_bindings) = builder.finish();
            Plan::from_bindings(bindings, class_bindings)
        });
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();

        let mut frame = InputFrame::default();
        frame.record(RawEvent::Gamepad(
            bevy_input::gamepad::RawGamepadEvent::Axis(
                bevy_input::gamepad::RawGamepadAxisChangedEvent::new(
                    bevy_ecs::entity::Entity::PLACEHOLDER,
                    GamepadAxis::RightStickX,
                    0.5,
                ),
            ),
        ));

        // Half deflection for a quarter second, at 180 a second, is 22.5 — and the same stick over
        // a shorter tick moves the action less, which is the entire point.
        state.apply_frame(
            &frame,
            &threshold,
            0.25,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.value::<Look>().x, 22.5);

        state.transitions.clear();
        state.apply_frame(
            &frame,
            &threshold,
            0.125,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.value::<Look>().x, 11.25);
    }

    /// The refusal that still stands: a displacement is not a rate, so there is nothing to
    /// integrate and asking for it is a mistake rather than a no-op.
    #[test]
    fn a_delta_control_cannot_be_read_as_a_rate() {
        use crate::plan::{DiagnosticKind, Severity};

        struct Look;

        impl InputAction for Look {
            type Output = Vec2;

            const INTENT: ActionIntent = ActionIntent::Delta2;
            const PATH: &'static str = "eval_tests.double_integrated";
        }

        let mut builder = InputContextBuilder::<Flying>::default();
        builder
            .bind::<Look>(crate::binding::MouseMove)
            .per_second(180.0);

        let found = builder.diagnostics();
        assert_eq!(
            found.first().map(|d| d.kind.clone()),
            Some(DiagnosticKind::RateFromDelta {
                shape: crate::action::ChannelShape::Delta2
            })
        );
        assert_eq!(found[0].severity(), Severity::Error);
    }

    /// Each modifier gets its own memory. Two of a kind on one binding must not share, or the
    /// second would read what the first wrote and the chain would depend on its own length.
    #[cfg(feature = "keyboard")]
    #[test]
    fn every_modifier_in_a_chain_has_its_own_registers() {
        struct Remembering;

        impl crate::binding::Modifier for Remembering {
            fn apply(
                &self,
                _value: ActionValue,
                registers: &mut crate::action::Registers,
                _delta: f32,
            ) -> ActionValue {
                registers.count += 1;
                ActionValue::Axis1(f32::from(registers.count))
            }
        }

        struct Counted;

        impl InputAction for Counted {
            type Output = f32;

            const INTENT: ActionIntent = ActionIntent::Analog1;
            const PATH: &'static str = "eval_tests.counted";
        }

        let mut builder = InputContextBuilder::<Flying>::default();
        builder
            .bind::<Counted>(bevy_input::keyboard::KeyCode::Space)
            .custom(Remembering)
            .custom(Remembering);
        let plan = Arc::new({
            let (bindings, class_bindings) = builder.finish();
            Plan::from_bindings(bindings, class_bindings)
        });
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();
        let frame = InputFrame::default();

        // Both start at zero and both count to one, so the pair reads 1 rather than 2.
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.value::<Counted>(), 1.0);
        // ...and to two on the next tick, having each kept their own count.
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.value::<Counted>(), 2.0);
    }

    /// Turns every eligible binding's `hold_or_toggle` modifier on, standing in for the override an
    /// app would normally apply from a settings screen — the low-level `InputContextBuilder` this
    /// module tests through has no path from a `TunableValue` to a compiled plan.
    #[cfg(feature = "keyboard")]
    fn force_toggle_on(bindings: &mut [crate::binding::BindingSpec]) {
        for binding in bindings {
            for modifier in &mut binding.modifiers {
                if let crate::binding::BindingModifier::Toggle { active } = modifier {
                    *active = true;
                }
            }
        }
    }

    /// Two bindings sharing a `hold_or_toggle` key — a primary and a secondary, the way
    /// Disasteroids' `Thrust` is — read one latch rather than two once a player turns toggle mode
    /// on. Pressing either one flips it; which one pressed last time is not remembered anywhere.
    #[cfg(feature = "keyboard")]
    #[test]
    fn two_bindings_sharing_a_toggle_share_one_latch() {
        use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};

        fn key_at(code: KeyCode, state: ButtonState) -> RawEvent {
            RawEvent::Keyboard(KeyboardInput {
                key_code: code,
                logical_key: Key::Space,
                state,
                text: None,
                repeat: false,
                window: bevy_ecs::entity::Entity::PLACEHOLDER,
            })
        }

        fn apply(
            state: &mut InputContextState<Flying>,
            frame: &mut InputFrame,
            threshold: &ButtonThreshold,
            event: RawEvent,
        ) {
            frame.record(event);
            state.apply_frame(
                frame,
                threshold,
                TICK,
                &ConsumedControls::default(),
                &mut Vec::new(),
                None,
            );
        }

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(KeyCode::KeyW);
        builder.bind::<Jump>(KeyCode::ArrowUp);
        builder.hold_or_toggle::<Jump>("eval_tests.jump.hold_or_toggle");
        let mut state = InputContextState::<Flying>::new(
            Arc::new({
                let (mut bindings, class_bindings) = builder.finish();
                force_toggle_on(&mut bindings);
                Plan::from_bindings(bindings, class_bindings)
            }),
            None,
        );
        let threshold = ButtonThreshold::default();
        // One frame for the whole test, its events accumulating: `apply_frame` reads only what is
        // new to it.
        let mut frame = InputFrame::default();

        apply(
            &mut state,
            &mut frame,
            &threshold,
            key_at(KeyCode::KeyW, ButtonState::Pressed),
        );
        assert!(state.value::<Jump>(), "pressing W turns the latch on");

        apply(
            &mut state,
            &mut frame,
            &threshold,
            key_at(KeyCode::KeyW, ButtonState::Released),
        );
        assert!(
            state.value::<Jump>(),
            "letting go of W does not turn a toggle back off"
        );

        apply(
            &mut state,
            &mut frame,
            &threshold,
            key_at(KeyCode::ArrowUp, ButtonState::Pressed),
        );
        assert!(
            !state.value::<Jump>(),
            "the OTHER key flips the same shared latch off — two independent latches would still \
             read true here"
        );

        apply(
            &mut state,
            &mut frame,
            &threshold,
            key_at(KeyCode::ArrowUp, ButtonState::Released),
        );
        apply(
            &mut state,
            &mut frame,
            &threshold,
            key_at(KeyCode::KeyW, ButtonState::Pressed),
        );
        assert!(
            state.value::<Jump>(),
            "and back on again, from whichever key is pressed next"
        );
    }

    /// Held is the default, and two bindings sharing a `hold_or_toggle` key must not change that:
    /// before a player ever turns toggle mode on, each key is an ordinary momentary control, exactly
    /// as if `hold_or_toggle` had never been declared.
    ///
    /// A shared latch that resolves without first checking the tunable's own value reads as
    /// permanently toggled instead, with no way to turn it off.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_shared_toggle_left_at_its_default_behaves_as_an_ordinary_hold() {
        use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};

        fn key_at(code: KeyCode, state: ButtonState) -> RawEvent {
            RawEvent::Keyboard(KeyboardInput {
                key_code: code,
                logical_key: Key::Space,
                state,
                text: None,
                repeat: false,
                window: bevy_ecs::entity::Entity::PLACEHOLDER,
            })
        }

        fn apply(
            state: &mut InputContextState<Flying>,
            frame: &mut InputFrame,
            threshold: &ButtonThreshold,
            event: RawEvent,
        ) {
            frame.record(event);
            state.apply_frame(
                frame,
                threshold,
                TICK,
                &ConsumedControls::default(),
                &mut Vec::new(),
                None,
            );
        }

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(KeyCode::KeyW);
        builder.bind::<Jump>(KeyCode::ArrowUp);
        builder.hold_or_toggle::<Jump>("eval_tests.jump.hold_or_toggle_default");
        let mut state = InputContextState::<Flying>::new(
            Arc::new({
                let (bindings, class_bindings) = builder.finish();
                Plan::from_bindings(bindings, class_bindings)
            }),
            None,
        );
        let threshold = ButtonThreshold::default();
        let mut frame = InputFrame::default();

        apply(
            &mut state,
            &mut frame,
            &threshold,
            key_at(KeyCode::KeyW, ButtonState::Pressed),
        );
        assert!(
            state.value::<Jump>(),
            "pressing W fires it, same as any hold"
        );

        apply(
            &mut state,
            &mut frame,
            &threshold,
            key_at(KeyCode::KeyW, ButtonState::Released),
        );
        assert!(
            !state.value::<Jump>(),
            "and letting go clears it — no latch to hold it on"
        );

        apply(
            &mut state,
            &mut frame,
            &threshold,
            key_at(KeyCode::ArrowUp, ButtonState::Pressed),
        );
        assert!(state.value::<Jump>(), "the secondary key fires it too");

        apply(
            &mut state,
            &mut frame,
            &threshold,
            key_at(KeyCode::ArrowUp, ButtonState::Released),
        );
        assert!(!state.value::<Jump>(), "and clears the same way on release");
    }

    /// A hold, all the way through and then abandoned. The distinction the phases exist for is that
    /// giving up part way is visibly different from seeing it through, and neither is silence.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_hold_starts_fires_completes_and_can_be_abandoned() {
        use bevy_input::keyboard::KeyCode;

        let mut script = Script::new(|controls| {
            controls.bind::<Jump>(KeyCode::Space).hold(0.25);
        });

        // Press, and wait it out.
        assert_eq!(
            script.tick(0.1, [key(ButtonState::Pressed)]),
            [ActionPhase::Started]
        );
        assert!(script.tick(0.1, []).is_empty(), "still charging");
        assert_eq!(script.state.phase::<Jump>(), ActionPhase::Building);
        assert!(!script.state.value::<Jump>(), "and not yet jumping");
        assert_eq!(
            script.tick(0.1, []),
            [ActionPhase::Fired],
            "0.3s is past 0.25s"
        );
        assert!(script.state.value::<Jump>());
        assert!(script.tick(0.1, []).is_empty(), "still held");
        assert_eq!(script.state.phase::<Jump>(), ActionPhase::Firing);

        assert_eq!(
            script.tick(0.1, [key(ButtonState::Released)]),
            [ActionPhase::Completed]
        );

        // Now the same press, given up on early.
        assert_eq!(
            script.tick(0.1, [key(ButtonState::Pressed)]),
            [ActionPhase::Started]
        );
        assert_eq!(
            script.tick(0.1, [key(ButtonState::Released)]),
            [ActionPhase::Canceled],
            "abandoned before it ever fired"
        );
        assert!(!script.state.value::<Jump>());
    }

    /// A tick is one tick of time however many events it carries. Three keys that have nothing to
    /// do with the hold arrive together, and the hold is charged for one tick, not three.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_hold_charges_once_per_tick() {
        use bevy_input::keyboard::KeyCode;

        let mut script = Script::new(|controls| {
            controls.bind::<Jump>(KeyCode::Space).hold(0.25);
        });

        assert_eq!(
            script.tick(0.1, [key(ButtonState::Pressed)]),
            [ActionPhase::Started]
        );
        assert!(
            script
                .tick(
                    0.1,
                    [
                        layout_key(KeyCode::KeyA, "a", ButtonState::Pressed),
                        layout_key(KeyCode::KeyS, "s", ButtonState::Pressed),
                        layout_key(KeyCode::KeyA, "a", ButtonState::Released),
                    ]
                )
                .is_empty(),
            "0.2s is short of 0.25s"
        );
        assert_eq!(script.tick(0.1, []), [ActionPhase::Fired]);
    }

    /// A claim lifting off a key still held hands it back already down, which is held over until
    /// the player lets go, as on activation (D94). A hold is the binding the require-reset latch has to reach:
    /// held long enough, the returning key would otherwise charge it through to firing.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_claim_lifting_waits_for_a_release() {
        use bevy_input::keyboard::KeyCode;

        let mut script = Script::new(|controls| {
            controls.bind::<Jump>(KeyCode::Space).hold(0.25);
        });
        script.consumed.claim::<bevy_app::PreUpdate>(
            Control::PhysicalKey(KeyCode::Space),
            None,
            "eval_tests.vehicle",
        );

        assert!(script.tick(0.1, [key(ButtonState::Pressed)]).is_empty());
        script.consumed = ConsumedControls::default();
        for _ in 0..4 {
            assert!(script.tick(0.1, []).is_empty(), "the key came back down");
        }
        assert!(script.tick(0.1, [key(ButtonState::Released)]).is_empty());

        assert_eq!(
            script.tick(0.1, [key(ButtonState::Pressed)]),
            [ActionPhase::Started]
        );
        assert!(script.tick(0.1, []).is_empty());
        assert_eq!(script.tick(0.1, []), [ActionPhase::Fired]);
    }

    /// A claim reaches only the binding on the control it took. With Enter held and claimed, Space
    /// on the same action still fires, and letting Space go is an ordinary release.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_claim_leaves_the_actions_other_bindings_alone() {
        use bevy_input::keyboard::KeyCode;

        let mut script = Script::new(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
            controls.bind::<Jump>(KeyCode::Enter);
        });
        script.consumed.claim::<bevy_app::PreUpdate>(
            Control::PhysicalKey(KeyCode::Enter),
            None,
            "eval_tests.vehicle",
        );
        let enter = layout_key(KeyCode::Enter, "", ButtonState::Pressed);

        assert!(script.tick(TICK, [enter]).is_empty(), "Enter is taken");
        assert_eq!(
            script.tick(TICK, [key(ButtonState::Pressed)]),
            [ActionPhase::Fired]
        );
        assert_eq!(
            script.tick(TICK, [key(ButtonState::Released)]),
            [ActionPhase::Completed],
            "not Canceled: Space was let go"
        );
    }

    /// A key bound nowhere in the context changes nothing it reports. Space and F go down in one
    /// tick, once alone and once with A beside them, and both ticks must log the same phases and
    /// end in the same ones.
    #[cfg(feature = "keyboard")]
    #[test]
    fn an_unbound_key_changes_nothing_reported() {
        use bevy_input::keyboard::KeyCode;

        let tick = |unbound: bool| {
            let mut script = Script::new(|controls| {
                controls.bind::<Jump>(KeyCode::Space).press();
                controls.bind::<Serve>(KeyCode::KeyF).pulse(10.0);
            });
            let mut events = alloc::vec![
                key(ButtonState::Pressed),
                layout_key(KeyCode::KeyF, "f", ButtonState::Pressed),
            ];
            if unbound {
                events.push(layout_key(KeyCode::KeyA, "a", ButtonState::Pressed));
            }
            let logged = script.tick(0.1, events);
            (
                logged,
                script.state.phase::<Jump>(),
                script.state.phase::<Serve>(),
            )
        };

        assert_eq!(tick(false), tick(true));
    }

    /// Events inside one tick carry no time between them, so a press and a release in the same tick
    /// are as quick as a press can be, even when the tick itself was long.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_press_and_release_in_one_long_tick_is_a_tap() {
        use bevy_input::keyboard::KeyCode;

        let press_and_release = [key(ButtonState::Pressed), key(ButtonState::Released)];

        let mut tap = Script::new(|controls| {
            controls.bind::<Jump>(KeyCode::Space).tap(0.2);
        });
        assert_eq!(
            tap.tick(1.0, press_and_release.clone()),
            [ActionPhase::Started, ActionPhase::Fired]
        );

        let mut hold = Script::new(|controls| {
            controls.bind::<Jump>(KeyCode::Space).hold(0.25);
        });
        assert_eq!(
            hold.tick(1.0, press_and_release),
            [ActionPhase::Started, ActionPhase::Canceled],
            "not held for the tick"
        );
    }

    /// A key held across activation is not a press, and a time condition on its binding must not
    /// make one of it: a hold does not charge while the key stays down, and a tap or a
    /// hold-and-release does not fire when it comes up.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_time_condition_ignores_a_control_held_across_activation() {
        use bevy_input::keyboard::KeyCode;

        type Declare = fn(&mut InputContextBuilder<Flying>);
        let conditions: [(&str, Declare); 3] = [
            ("hold", |controls| {
                controls.bind::<Jump>(KeyCode::Space).hold(0.25);
            }),
            ("tap", |controls| {
                controls.bind::<Jump>(KeyCode::Space).tap(0.2);
            }),
            ("hold_and_release", |controls| {
                controls.bind::<Jump>(KeyCode::Space).hold_and_release(0.25);
            }),
        ];
        for (name, declare) in conditions {
            let mut script = Script::new(declare);
            script.state.deactivate();
            assert!(script.tick(0.1, [key(ButtonState::Pressed)]).is_empty());

            script.state.activate();
            for _ in 0..4 {
                assert!(script.tick(0.1, []).is_empty(), "{name}: still held over");
            }
            assert_eq!(script.state.phase::<Jump>(), ActionPhase::Idle, "{name}");
            assert!(
                script.tick(0.1, [key(ButtonState::Released)]).is_empty(),
                "{name}: letting go is not a release"
            );

            assert_eq!(
                script.tick(0.1, [key(ButtonState::Pressed)]),
                [ActionPhase::Started],
                "{name}: the next press counts"
            );
        }
    }

    /// Two bindings on one action, one of which has a condition. The action reports the most
    /// definite thing any of them said, so a plain press is not drowned out by a hold in progress.
    #[cfg(all(feature = "keyboard", feature = "gamepad"))]
    #[test]
    fn the_most_definite_binding_decides_the_action() {
        use bevy_input::gamepad::GamepadButton;
        use bevy_input::keyboard::KeyCode;

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(KeyCode::Space).hold(10.0);
        builder.bind::<Jump>(GamepadButton::South);
        let plan = Arc::new({
            let (bindings, class_bindings) = builder.finish();
            Plan::from_bindings(bindings, class_bindings)
        });
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();
        let mut frame = InputFrame::default();

        // The keyboard hold will never finish, so on its own the action is merely charging.
        frame.record(key(ButtonState::Pressed));
        state.apply_frame(
            &frame,
            &threshold,
            0.1,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.phase::<Jump>(), ActionPhase::Started);

        // The pad has no condition, so it fires outright and the action goes with it.
        frame.record(RawEvent::Gamepad(
            bevy_input::gamepad::RawGamepadEvent::Button(
                bevy_input::gamepad::RawGamepadButtonChangedEvent::new(
                    bevy_ecs::entity::Entity::PLACEHOLDER,
                    GamepadButton::South,
                    1.0,
                ),
            ),
        ));
        state.apply_frame(
            &frame,
            &threshold,
            0.1,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
        assert_eq!(state.phase::<Jump>(), ActionPhase::Fired);
        assert!(state.value::<Jump>());
    }

    /// A press derived from an axis is thresholded with the same hysteresis the button channel
    /// uses, so a stick wobbling across the line does not chatter.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_press_derived_from_an_axis_does_not_chatter() {
        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(GamepadAxis::LeftStickY);
        let plan = Arc::new({
            let (bindings, class_bindings) = builder.finish();
            Plan::from_bindings(bindings, class_bindings)
        });
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();
        let midband = (threshold.press + threshold.release) / 2.0;

        let mut frame = InputFrame::default();
        let push_to = |state: &mut InputContextState<Flying>, frame: &mut InputFrame, to: f32| {
            frame.record(RawEvent::Gamepad(
                bevy_input::gamepad::RawGamepadEvent::Axis(
                    bevy_input::gamepad::RawGamepadAxisChangedEvent::new(
                        bevy_ecs::entity::Entity::PLACEHOLDER,
                        GamepadAxis::LeftStickY,
                        to,
                    ),
                ),
            ));
            state.apply_frame(
                frame,
                &threshold,
                TICK,
                &ConsumedControls::default(),
                &mut Vec::new(),
                None,
            );
            state.value::<Jump>()
        };

        assert!(push_to(&mut state, &mut frame, 0.9));
        // Falling back into the band holds the press rather than dropping it.
        assert!(push_to(&mut state, &mut frame, midband));
        assert!(!push_to(&mut state, &mut frame, 0.1));
        // ...and re-entering it keeps it let go.
        assert!(!push_to(&mut state, &mut frame, midband));
    }

    /// A menu that takes the pad's confirm button, trigger or stick takes it from the game behind
    /// it. Each shape reads as untouched in its own terms, and a stick loses only the axis that was
    /// taken.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_consumed_gamepad_control_reads_as_untouched() {
        use bevy_input::gamepad::{
            GamepadButton, RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent,
        };

        struct Throttle;

        impl InputAction for Throttle {
            type Output = f32;

            const INTENT: ActionIntent = ActionIntent::Analog1;
            const PATH: &'static str = "eval_tests.throttle";
        }

        struct Steer;

        impl InputAction for Steer {
            type Output = Vec2;

            const INTENT: ActionIntent = ActionIntent::Directional2;
            const PATH: &'static str = "eval_tests.steer";
        }

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(GamepadButton::South);
        builder.bind::<Throttle>(GamepadButton::RightTrigger2);
        builder.bind::<Steer>(Stick::Left);
        let plan = Arc::new({
            let (bindings, class_bindings) = builder.finish();
            Plan::from_bindings(bindings, class_bindings)
        });
        let threshold = ButtonThreshold::default();

        let mut frame = InputFrame::default();
        let button = |button, value| {
            RawEvent::Gamepad(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                button,
                value,
            )))
        };
        let axis = |axis, value| {
            RawEvent::Gamepad(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                bevy_ecs::entity::Entity::PLACEHOLDER,
                axis,
                value,
            )))
        };
        frame.record(button(GamepadButton::South, 1.0));
        frame.record(button(GamepadButton::RightTrigger2, 0.8));
        frame.record(axis(GamepadAxis::LeftStickX, 0.9));
        frame.record(axis(GamepadAxis::LeftStickY, 0.9));

        let mut consumed = ConsumedControls::default();
        consumed.claim::<bevy_app::PreUpdate>(
            Control::GamepadButton(GamepadButton::South),
            None,
            "tests.menu",
        );
        consumed.claim::<bevy_app::PreUpdate>(
            Control::GamepadButton(GamepadButton::RightTrigger2),
            None,
            "tests.menu",
        );
        consumed.claim::<bevy_app::PreUpdate>(
            Control::GamepadAxis(GamepadAxis::LeftStickX),
            None,
            "tests.menu",
        );

        let mut state = InputContextState::<Flying>::new(plan.clone(), None);
        state.apply_frame(&frame, &threshold, TICK, &consumed, &mut Vec::new(), None);
        assert!(!state.value::<Jump>(), "the button, read as a press");
        assert_eq!(
            state.value::<Throttle>(),
            0.0,
            "the trigger, read as travel"
        );
        let steer = state.value::<Steer>();
        assert_eq!(steer.x, 0.0, "the axis that was taken");
        assert!(steer.y > 0.0, "and the one that was not");

        // A capture listening for a stick claims it whole, and that takes both axes.
        let mut consumed = ConsumedControls::default();
        consumed.claim_for_capture(Control::GamepadStick(Stick::Left), None);
        let mut state = InputContextState::<Flying>::new(plan, None);
        state.apply_frame(&frame, &threshold, TICK, &consumed, &mut Vec::new(), None);
        assert_eq!(state.value::<Steer>(), Vec2::ZERO);
        assert_eq!(
            consumed.claimant(Control::GamepadAxis(GamepadAxis::LeftStickY), None),
            Some("capture")
        );
    }

    struct CharacterInput;

    #[cfg(feature = "keyboard")]
    impl crate::event::ClassBinding for CharacterInput {
        const PATH: &'static str = "eval_tests.character_input";
    }

    #[cfg(feature = "keyboard")]
    fn char_key(state: ButtonState, text: Option<&str>) -> RawEvent {
        use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};

        RawEvent::Keyboard(KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: Key::Character(text.unwrap_or_default().into()),
            state,
            text: text.map(Into::into),
            repeat: false,
            window: bevy_ecs::entity::Entity::PLACEHOLDER,
        })
    }

    /// An unindexed, class-matching key fires the class binding and, once `consume` is set, is
    /// claimed the same way a plain consuming binding claims its control.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_class_binding_fires_and_consumes_an_unclaimed_key() {
        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind_characters::<CharacterInput>().consume();
        let plan = Arc::new({
            let (bindings, class_bindings) = builder.finish();
            Plan::from_bindings(bindings, class_bindings)
        });
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();

        let mut frame = InputFrame::default();
        frame.record(char_key(ButtonState::Pressed, Some("a")));
        let mut claims = Vec::new();
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut claims,
            None,
        );

        assert_eq!(state.class_fires.len(), 1);
        assert!(matches!(
            &state.class_fires[0].event,
            RawEvent::Keyboard(bevy_input::keyboard::KeyboardInput { text: Some(text), .. })
                if text.as_str() == "a"
        ));
        assert_eq!(
            claims,
            alloc::vec![Control::PhysicalKey(bevy_input::keyboard::KeyCode::KeyA)]
        );
    }

    /// A control already read by a plain binding never reaches the class list, even when it would
    /// also match — the per-control index wins unconditionally.
    #[cfg(feature = "keyboard")]
    #[test]
    fn an_indexed_control_never_reaches_the_class_list() {
        use crate::capture::ControlClass;
        use bevy_input::keyboard::KeyCode;

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(KeyCode::KeyA);
        builder
            .bind_class::<CharacterInput>(ControlClass::AnyButton)
            .consume();
        let plan = Arc::new({
            let (bindings, class_bindings) = builder.finish();
            Plan::from_bindings(bindings, class_bindings)
        });
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();

        let mut frame = InputFrame::default();
        frame.record(char_key(ButtonState::Pressed, Some("a")));
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );

        assert!(state.class_fires.is_empty());
        // The plain binding still saw it.
        assert_eq!(state.transitions.len(), 1);
    }

    /// A class binding that does not ask to consume leaves the control for a lower-priority context
    /// to see, the same as any other binding's default.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_non_consuming_class_binding_claims_nothing() {
        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind_characters::<CharacterInput>();
        let plan = Arc::new({
            let (bindings, class_bindings) = builder.finish();
            Plan::from_bindings(bindings, class_bindings)
        });
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();

        let mut frame = InputFrame::default();
        frame.record(char_key(ButtonState::Pressed, Some("a")));
        let mut claims = Vec::new();
        state.apply_frame(
            &frame,
            &threshold,
            TICK,
            &ConsumedControls::default(),
            &mut claims,
            None,
        );

        assert_eq!(state.class_fires.len(), 1, "it still fires");
        assert!(claims.is_empty(), "but claims nothing");
    }

    /// One keypress, spelled as the two things it is: where the key sits, and what the layout makes
    /// it say. Every logical-binding test below turns on the two disagreeing.
    #[cfg(feature = "keyboard")]
    fn layout_key(
        key_code: bevy_input::keyboard::KeyCode,
        character: &str,
        state: ButtonState,
    ) -> RawEvent {
        use bevy_input::keyboard::{Key, KeyboardInput};

        RawEvent::Keyboard(KeyboardInput {
            key_code,
            logical_key: Key::Character(character.into()),
            state,
            text: Some(character.into()),
            repeat: false,
            window: bevy_ecs::entity::Entity::PLACEHOLDER,
        })
    }

    #[cfg(feature = "keyboard")]
    fn context_bound_to(input: impl crate::binding::IntoBindingInput) -> InputContextState<Flying> {
        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(input);
        InputContextState::<Flying>::new(
            Arc::new({
                let (bindings, class_bindings) = builder.finish();
                Plan::from_bindings(bindings, class_bindings)
            }),
            None,
        )
    }

    /// One tick: the event arrives on the running frame and the context reads as far as it goes.
    /// The frame is carried across calls because a context reads only what is new to it.
    #[cfg(feature = "keyboard")]
    fn press(state: &mut InputContextState<Flying>, frame: &mut InputFrame, event: RawEvent) {
        frame.record(event);
        state.apply_frame(
            frame,
            &ButtonThreshold::default(),
            TICK,
            &ConsumedControls::default(),
            &mut Vec::new(),
            None,
        );
    }

    /// On AZERTY the key that says `z` is the one QWERTY calls `W`, so a logical binding has to
    /// follow the character across the board.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_logical_binding_follows_the_character_not_the_position() {
        use bevy_input::keyboard::KeyCode;

        let mut frame = InputFrame::default();
        let mut state = context_bound_to(crate::binding::LogicalKey('z'));
        press(
            &mut state,
            &mut frame,
            layout_key(KeyCode::KeyW, "z", ButtonState::Pressed),
        );

        assert!(state.value::<Jump>(), "the AZERTY z key fired it");
    }

    /// The other half of R12.1: the choice is explicit because the two answer differently. The same
    /// press that satisfies `LogicalKey('z')` above leaves a binding on the Z *position* alone.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_physical_binding_ignores_what_the_layout_prints() {
        use bevy_input::keyboard::KeyCode;

        let mut frame = InputFrame::default();
        let mut state = context_bound_to(KeyCode::KeyZ);
        press(
            &mut state,
            &mut frame,
            layout_key(KeyCode::KeyW, "z", ButtonState::Pressed),
        );

        assert!(!state.value::<Jump>(), "a different position entirely");
    }

    /// A capital `Z` is the Z key and shift, not a key of its own — and with control held, which of
    /// the two a platform reports is not something a binding should have to know.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_logical_binding_ignores_the_case_the_platform_reports() {
        use bevy_input::keyboard::KeyCode;

        let mut frame = InputFrame::default();
        let mut state = context_bound_to(crate::binding::LogicalKey('z'));
        press(
            &mut state,
            &mut frame,
            layout_key(KeyCode::KeyZ, "Z", ButtonState::Pressed),
        );

        assert!(state.value::<Jump>());
    }

    /// Only a key that produces a character of its own is bindable. A dead key has produced nothing
    /// yet, and the several characters an IME commits at once are a composition — text entry's
    /// problem (R12.6), not a binding's.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_composition_is_not_a_key_a_binding_can_name() {
        use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};

        let mut frame = InputFrame::default();
        let mut state = context_bound_to(crate::binding::LogicalKey('a'));

        press(
            &mut state,
            &mut frame,
            layout_key(KeyCode::KeyA, "ae", ButtonState::Pressed),
        );
        assert!(!state.value::<Jump>(), "two characters are not a key");

        press(
            &mut state,
            &mut frame,
            RawEvent::Keyboard(KeyboardInput {
                key_code: KeyCode::Quote,
                logical_key: Key::Dead(Some('\u{b4}')),
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: bevy_ecs::entity::Entity::PLACEHOLDER,
            }),
        );
        assert!(!state.value::<Jump>(), "a dead key has produced nothing");
    }

    /// Held state is keyed by position, so a release always finds its press. Pressing shift partway
    /// through a hold changes the character the platform reports, and a release matched on that
    /// would strand the key down forever.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_release_clears_a_hold_the_shift_key_renamed() {
        use bevy_input::keyboard::KeyCode;

        let mut frame = InputFrame::default();
        let mut state = context_bound_to(crate::binding::LogicalKey('z'));

        press(
            &mut state,
            &mut frame,
            layout_key(KeyCode::KeyZ, "z", ButtonState::Pressed),
        );
        assert!(state.value::<Jump>());

        // Shift goes down mid-hold, and the same physical key now reports itself capitalized.
        press(
            &mut state,
            &mut frame,
            layout_key(KeyCode::KeyZ, "Z", ButtonState::Released),
        );
        assert!(!state.value::<Jump>(), "let go, not stranded");
    }

    #[derive(crate::InputAction)]
    #[action(path = "eval_tests.move", output = Vec2, intent = Directional2)]
    struct Move;

    /// A context compiled the way `add_context` compiles one, `combined` included.
    fn context_declaring(
        declare: impl FnOnce(&mut InputContextBuilder<Flying>),
    ) -> InputContextState<Flying> {
        let mut builder = InputContextBuilder::<Flying>::default();
        declare(&mut builder);
        let combined = builder.take_combined();
        let (bindings, class_bindings) = builder.finish();
        let mut plan = Plan::from_bindings(bindings, class_bindings);
        plan.combine(combined);
        InputContextState::<Flying>::new(Arc::new(plan), None)
    }

    /// One context driven a tick at a time, each tick with its own events and `delta`, for tests
    /// about what a tick logged rather than where it left the action.
    #[cfg(feature = "keyboard")]
    struct Script {
        state: InputContextState<Flying>,
        frame: InputFrame,
        /// What contexts above have claimed, standing until a test changes it.
        consumed: ConsumedControls,
    }

    #[cfg(feature = "keyboard")]
    impl Script {
        fn new(declare: impl FnOnce(&mut InputContextBuilder<Flying>)) -> Self {
            Self {
                state: context_declaring(declare),
                frame: InputFrame::default(),
                consumed: ConsumedControls::default(),
            }
        }

        /// The phases this tick logged, in order. The log is cleared first, as dispatch would.
        fn tick(
            &mut self,
            delta: f32,
            events: impl IntoIterator<Item = RawEvent>,
        ) -> Vec<ActionPhase> {
            for event in events {
                self.frame.record(event);
            }
            self.state.transitions.clear();
            self.state.apply_frame(
                &self.frame,
                &ButtonThreshold::default(),
                delta,
                &self.consumed,
                &mut Vec::new(),
                None,
            );
            self.state.transitions.iter().map(|t| t.phase).collect()
        }
    }

    /// Two keys held for a diagonal read `(1, 1)`, and a clamp declared once for the action pulls
    /// that back to unit length without either binding naming it.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_diagonal_is_clamped_once_for_the_action() {
        use crate::binding::DirectionalButtons;
        use bevy_input::keyboard::KeyCode;

        let mut state = context_declaring(|controls| {
            controls.bind::<Move>(DirectionalButtons::wasd());
            controls.combined::<Move>().clamp_magnitude();
        });
        let mut frame = InputFrame::default();

        press(
            &mut state,
            &mut frame,
            layout_key(KeyCode::KeyW, "w", ButtonState::Pressed),
        );
        assert_eq!(
            state.value::<Move>(),
            Vec2::Y,
            "a straight line is already unit length"
        );

        press(
            &mut state,
            &mut frame,
            layout_key(KeyCode::KeyD, "d", ButtonState::Pressed),
        );
        let diagonal = state.value::<Move>();
        assert!((diagonal.length() - 1.0).abs() < 1e-5, "{diagonal}");
        assert!(
            (diagonal.x - diagonal.y).abs() < 1e-5,
            "still a diagonal: {diagonal}"
        );
    }

    /// A condition on the combined value judges the direction the player is asking for, not the
    /// control asking. A second control agreeing with the first changes nothing, where the same
    /// `on_change` on each binding would fire again for the second.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_second_control_agreeing_with_the_first_is_not_a_change() {
        use crate::binding::DirectionalButtons;
        use bevy_input::keyboard::KeyCode;

        let mut state = context_declaring(|controls| {
            controls.bind::<Move>(DirectionalButtons::arrow_keys());
            controls.bind::<Move>(DirectionalButtons::wasd());
            controls.combined::<Move>().on_change();
        });
        let mut frame = InputFrame::default();
        let fired = |state: &mut InputContextState<Flying>| {
            let fired = state
                .transitions
                .iter()
                .any(|transition| transition.phase == ActionPhase::Fired);
            state.transitions.clear();
            fired
        };

        let up = |state| layout_key(KeyCode::ArrowUp, "", state);
        let w = |state| layout_key(KeyCode::KeyW, "w", state);

        press(&mut state, &mut frame, up(ButtonState::Pressed));
        assert!(fired(&mut state), "up");

        press(&mut state, &mut frame, w(ButtonState::Pressed));
        assert!(!fired(&mut state), "W is up as well, and up is not news");

        press(&mut state, &mut frame, up(ButtonState::Released));
        assert!(!fired(&mut state), "W is still asking for up");

        press(&mut state, &mut frame, w(ButtonState::Released));
        assert!(fired(&mut state), "letting go of both is a change");
    }

    /// A binding part way through a hold contributes rest to its action, and a condition on the
    /// combined value must not read that as the player doing nothing.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_hold_in_progress_survives_a_condition_on_the_combined_value() {
        use bevy_input::keyboard::KeyCode;

        let mut state = context_declaring(|controls| {
            controls.bind::<Jump>(KeyCode::Space).hold(10.0);
            controls.combined::<Jump>().press();
        });
        let mut frame = InputFrame::default();

        press(&mut state, &mut frame, key(ButtonState::Pressed));
        assert_eq!(state.phase::<Jump>(), ActionPhase::Started);
    }
}
