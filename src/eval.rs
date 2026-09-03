//! The evaluator: a plan and an input frame in, action state and a transition log out.
//!
//! The evaluator resolves bindings and emits a transition log for later dispatch.

use alloc::vec::Vec;

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Query, Res};
#[cfg(any(feature = "keyboard", feature = "mouse"))]
use bevy_input::ButtonState;
#[cfg(feature = "gamepad")]
use bevy_input::gamepad::{GamepadAxis, GamepadConnection, RawGamepadEvent};
#[cfg(feature = "keyboard")]
use bevy_input::keyboard::KeyboardInput;
#[cfg(feature = "mouse")]
use bevy_input::mouse::MouseButtonInput;
use bevy_math::{Vec2, Vec3};

use crate::action::{ActionValue, InputContext, Intent, Phase};
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
use crate::binding::ButtonControl;
#[cfg(feature = "gamepad")]
use crate::binding::Stick;
use crate::binding::{BindingSource, ButtonThreshold, Control};
use crate::condition::Verdict;
use crate::context::InputContextState;
use crate::frame::{InputFrame, RawEvent, TimedRawEvent};

/// Which controls have already been claimed this frame, and by which schedule.
///
/// Keyed by schedule rather than held flat, so that three cases come out right: what `PreUpdate`
/// claimed stays claimed for every fixed tick in the frame, what one fixed tick claimed does not
/// bind the next, and nothing survives into a frame where no fixed tick runs.
#[derive(bevy_ecs::resource::Resource, Default)]
pub struct ConsumedControls {
    // Which context took it, not merely that something did: "consumed" is one of five reasons an
    // action can silently not fire (R22.1), and the only useful form of the answer names the taker.
    by_schedule: bevy_platform::collections::HashMap<
        core::any::TypeId,
        bevy_platform::collections::HashMap<Control, &'static str>,
    >,
}

impl ConsumedControls {
    /// Whether any schedule has claimed this control.
    pub fn contains(&self, control: Control) -> bool {
        self.claimant(control).is_some()
    }

    /// The path of the context that claimed this control, if one did.
    pub fn claimant(&self, control: Control) -> Option<&'static str> {
        self.by_schedule
            .values()
            .find_map(|claims| claims.get(&control).copied())
    }

    fn claim<S: bevy_ecs::schedule::ScheduleLabel>(&mut self, control: Control, by: &'static str) {
        self.by_schedule
            .entry(core::any::TypeId::of::<S>())
            .or_default()
            .insert(control, by);
    }

    /// Takes a control on behalf of a live capture, so that what a player presses at a rebinding
    /// screen does not also play the game.
    ///
    /// Claimed under `PreUpdate`, where capture runs, which is what carries it through to the fixed
    /// schedules: a fixed tick releases only its own claims, so this one still stands when a
    /// fixed-tick context evaluates later in the frame.
    pub(crate) fn claim_for_capture(&mut self, control: Control) {
        self.claim::<bevy_app::PreUpdate>(control, "capture");
    }

    /// Forgets what one schedule claimed, which it does on entry so that each run decides afresh.
    fn release<S: bevy_ecs::schedule::ScheduleLabel>(&mut self) {
        if let Some(set) = self.by_schedule.get_mut(&core::any::TypeId::of::<S>()) {
            set.clear();
        }
    }

    fn release_all(&mut self) {
        for set in self.by_schedule.values_mut() {
            set.clear();
        }
    }
}

/// Starts a frame with nothing claimed.
pub fn release_consumed_controls(mut consumed: bevy_ecs::prelude::ResMut<'_, ConsumedControls>) {
    consumed.release_all();
}

/// The priority of the highest-priority active exclusive context seen so far this frame.
///
/// Unlike `ConsumedControls`, this needs no per-schedule bookkeeping: a context's activity does not
/// reset between fixed ticks the way a control's actuation does, so an exclusive context simply
/// re-raises the ceiling to the same value every time it runs. One number, reset once at the top of
/// the frame, is the whole mechanism — set by whichever exclusive context runs first in priority
/// order (render-tick contexts run before fixed-tick ones, so the same forward-only direction
/// applies here too), and read by everything lower that runs after it, in either domain, for the
/// rest of the frame.
#[derive(bevy_ecs::resource::Resource, Default)]
pub(crate) struct ExclusionCeiling(Option<i32>);

impl ExclusionCeiling {
    fn reset(&mut self) {
        self.0 = None;
    }

    /// Records that an exclusive context at this priority is active, raising the ceiling if it is
    /// not already at least this high. Monotonic within a frame — nothing lowers it before `reset`.
    fn raise(&mut self, priority: i32) {
        self.0 = Some(self.0.map_or(priority, |ceiling| ceiling.max(priority)));
    }

    /// Whether a context at this priority is shadowed by an exclusive one that has already run.
    fn shadows(&self, priority: i32) -> bool {
        self.0.is_some_and(|ceiling| priority < ceiling)
    }
}

/// Starts a frame with no exclusion in effect — the same clear point as
/// [`release_consumed_controls`], for the same reason: both describe "nothing has claimed anything
/// yet".
pub(crate) fn reset_exclusion_ceiling(
    mut ceiling: bevy_ecs::prelude::ResMut<'_, ExclusionCeiling>,
) {
    ceiling.reset();
}

/// Starts one run of a schedule with nothing claimed *by that schedule*.
pub fn release_consumed_in<S: bevy_ecs::schedule::ScheduleLabel>(
    mut consumed: bevy_ecs::prelude::ResMut<'_, ConsumedControls>,
) {
    consumed.release::<S>();
}

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
/// evaluator has to stay a pure function of its inputs.
pub fn dispatch_transitions<C: InputContext + Component>(
    mut commands: Commands<'_, '_>,
    mut states: Query<'_, '_, (Entity, &mut InputContextState<C>)>,
) {
    for (entity, mut state) in &mut states {
        if state.transitions.is_empty() {
            continue;
        }
        // Draining the log is not the action changing — the evaluation that filled it already said
        // so, and saying it twice would move the change tick a system later than the fact.
        let state = state.bypass_change_detection();

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

/// A control matching a bound class arrived and nothing indexed claimed it, logged in the order it
/// happened.
pub(crate) struct ClassFire {
    pub(crate) binding_index: usize,
    pub(crate) event: RawEvent,
}

/// `ClassFire`'s counterpart to [`dispatch_transitions`], and separate for the same reason.
pub fn dispatch_class_fires<C: InputContext + Component>(
    mut commands: Commands<'_, '_>,
    mut states: Query<'_, '_, (Entity, &mut InputContextState<C>)>,
) {
    for (entity, mut state) in &mut states {
        if state.class_fires.is_empty() {
            continue;
        }
        // A class binding writes no action state, so a fire is not something a subscriber to this
        // component asked to hear about.
        let state = state.bypass_change_detection();

        let mut log = core::mem::take(&mut state.class_fires);
        for fire in log.drain(..) {
            let dispatch = state.plan.class_bindings()[fire.binding_index].dispatch;
            dispatch(&mut commands, entity, fire.event);
        }
        state.class_fires = log;
    }
}

/// Applies the current input frame to every instance of one context.
pub(crate) fn evaluate_context<
    C: InputContext + Component,
    S: bevy_ecs::schedule::ScheduleLabel,
>(
    frame: Res<'_, InputFrame>,
    threshold: Res<'_, ButtonThreshold>,
    mut consumed: bevy_ecs::prelude::ResMut<'_, ConsumedControls>,
    mut ceiling: bevy_ecs::prelude::ResMut<'_, ExclusionCeiling>,
    // The generic clock, which Bevy points at the fixed timestep inside the fixed schedules — so a
    // context is told how long its own tick was rather than how long the frame was (R9.6).
    time: Res<'_, bevy_time::Time>,
    mut states: Query<'_, '_, (&mut InputContextState<C>, Option<&crate::player::Paired>)>,
) {
    let delta = time.delta_secs();
    // Read once, before this context's own instances can raise it further — evaluation order is
    // priority order (docs/design.md §5.1, §5.3), so whatever a higher-priority exclusive context
    // already did this frame is visible here, and nothing this context does can affect its own
    // shadowing.
    let shadowed = ceiling.shadows(C::PRIORITY);
    let mut any_active = false;
    for (mut state, pairing) in &mut states {
        // Bypassed for the whole pass and re-marked at the end only if an action moved. Every tick
        // writes *something* here — the read cursor at least — so taking the deref at face value
        // would mark every instance changed every tick, which is the all-or-nothing wake-up R23.4
        // asks us not to hand a subscriber.
        let instance = state.bypass_change_detection();
        instance.dirty.clear();

        if shadowed {
            instance.shadow();
        } else {
            instance.unshadow();
        }
        if instance.is_active() {
            any_active = true;
        }

        // Every instance of one context sees the same claims and adds to them together, so two
        // players sharing a context cannot take controls from each other.
        let mut claims = Vec::new();
        instance.apply_frame(&frame, &threshold, delta, &consumed, &mut claims, pairing);
        let moved = !instance.dirty.is_clear();
        for control in claims {
            consumed.claim::<S>(control, C::PATH);
        }
        if moved {
            state.set_changed();
        }
    }

    // Only a context that is itself active-and-unshadowed gets to shadow anything below it — which
    // is what makes two stacked exclusive contexts compose correctly with nothing extra: a second
    // exclusive context shadowed by a third does not also shadow whatever the second would have.
    if C::EXCLUSIVE && any_active {
        ceiling.raise(C::PRIORITY);
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
    /// A level pass triggered by a source disappearing — focus loss or a device disconnect — rather
    /// than a player releasing a control. Reads exactly like `Level`; the only difference is that a
    /// binding which was firing and reads at rest this pass reports `Canceled` rather than
    /// `Completed`, since nothing was let go. A binding on an unaffected device is untouched.
    Interrupted,
}

/// Whether this event means the source is gone — a window losing focus or a device disconnecting —
/// rather than an ordinary press, release, or motion.
fn interruption_kind(event: &RawEvent) -> Fold {
    match event {
        #[cfg(feature = "keyboard")]
        RawEvent::FocusLost => Fold::Interrupted,
        #[cfg(feature = "gamepad")]
        RawEvent::Gamepad(RawGamepadEvent::Connection(connection))
            if matches!(connection.connection, GamepadConnection::Disconnected) =>
        {
            Fold::Interrupted
        }
        _ => Fold::Level,
    }
}

impl<C: InputContext> InputContextState<C> {
    pub(crate) fn apply_frame(
        &mut self,
        frame: &InputFrame,
        threshold: &ButtonThreshold,
        delta: f32,
        consumed: &ConsumedControls,
        claims: &mut Vec<Control>,
        pairing: Option<&crate::player::Paired>,
    ) {
        // Only what has arrived since this context last looked. Re-reading the whole queue is what
        // made one mouse delta count three times across three fixed ticks. The cursor advances past
        // the full unfiltered slice — including another device's events an unpaired viewer never
        // touches below — so nothing here is ever offered twice.
        let unread = frame.events_after(self.read_through);
        if let Some(last) = unread.last() {
            self.read_through = Some(last.timestamp);
        }
        // Absent pairing reads every device, which is today's exact behavior (R15.3): a device's
        // input must not reach a context paired to someone else, but a context nobody paired stays
        // deaf to nothing.
        let owns = |event: &TimedRawEvent| pairing.is_none_or(|p| p.contains(event.event.device()));

        // An inactive context still tracks its devices, shadowed or not. Skipping that would leave
        // the held state stale, so reactivating would need a rebuild — and R7.6 wants activation to
        // be free.
        if !self.is_active() {
            for event in unread.iter().filter(|e| owns(e)) {
                self.apply_level_event(&event.event, threshold);
            }
            return;
        }

        let mut mouse_delta = Vec2::ZERO;
        let mut level_changes = 0usize;

        // Replayed one at a time rather than collapsed. Draining the whole window and then folding
        // once is what made a press and release inside a single window vanish: the two cancel in
        // the held state, and the fold sees nothing happen (R9.3).
        for event in unread.iter().filter(|e| owns(e)) {
            // `MouseMotion` is the one variant `RawEvent` keeps with every device feature off, so
            // there this is the only arm there is.
            #[cfg_attr(
                not(any(feature = "keyboard", feature = "mouse", feature = "gamepad")),
                allow(irrefutable_let_patterns)
            )]
            if let RawEvent::MouseMotion(delta) = &event.event {
                mouse_delta += *delta;
                continue;
            }
            self.apply_level_event(&event.event, threshold);
            self.fold(
                threshold,
                Vec2::ZERO,
                delta,
                interruption_kind(&event.event),
                consumed,
                claims,
            );
            // After the fold, not before: docs/design.md §5.4's ordering. A class binding never
            // competes on specificity, so it only ever gets a look at a control the fold's own
            // bindings did not already index — checked once here rather than woven into the fold.
            self.class_dispatch(&event.event, consumed, claims);
            level_changes += 1;
        }

        // Time passes even when nothing arrives: a phase has to reach `Ongoing` from `Fired` on its
        // own, and without an event to prompt it nothing else would.
        if level_changes == 0 {
            self.fold(threshold, Vec2::ZERO, delta, Fold::Level, consumed, claims);
        }

        self.fold(threshold, mouse_delta, delta, Fold::Delta, consumed, claims);
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
            #[cfg(feature = "mouse")]
            RawEvent::MouseButton(MouseButtonInput { button, state, .. }) => match state {
                ButtonState::Pressed => {
                    self.held_mouse_buttons.insert(*button);
                }
                ButtonState::Released => {
                    self.held_mouse_buttons.remove(button);
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
                // A disconnect leaves no release event to correct a stale reading — the backend has
                // nothing left to send one from (R11.4). Connecting needs nothing: the device's own
                // events repopulate these maps normally.
                RawGamepadEvent::Connection(connection) => {
                    if matches!(connection.connection, GamepadConnection::Disconnected) {
                        self.held_gamepad_buttons.clear();
                        self.held_gamepad_axes.clear();
                    }
                }
            },
            #[cfg(feature = "keyboard")]
            RawEvent::FocusLost => {
                self.held_buttons.clear();
                #[cfg(feature = "mouse")]
                self.held_mouse_buttons.clear();
            }
        }
    }

    /// Whether the control this event names is actuated right now, using the held state
    /// `apply_level_event` just updated — so a gamepad button reads through this crate's own
    /// threshold hysteresis rather than the raw fraction the backend reported.
    fn actuated(&self, event: &RawEvent) -> bool {
        match event {
            #[cfg(feature = "keyboard")]
            RawEvent::Keyboard(KeyboardInput { state, .. }) => *state == ButtonState::Pressed,
            #[cfg(feature = "mouse")]
            RawEvent::MouseButton(MouseButtonInput { state, .. }) => *state == ButtonState::Pressed,
            RawEvent::MouseMotion(_) => false,
            #[cfg(feature = "gamepad")]
            RawEvent::Gamepad(RawGamepadEvent::Button(raw_button)) => self
                .held_gamepad_buttons
                .get(&raw_button.button)
                .is_some_and(|reading| reading.pressed),
            #[cfg(feature = "gamepad")]
            RawEvent::Gamepad(RawGamepadEvent::Axis(raw_axis)) => raw_axis.value != 0.0,
            #[cfg(feature = "gamepad")]
            RawEvent::Gamepad(RawGamepadEvent::Connection(_)) => false,
            // Unreachable in practice: `control()` is `None` for this event, and `class_dispatch`
            // returns before ever asking. Kept for exhaustiveness, same as the arm above.
            #[cfg(feature = "keyboard")]
            RawEvent::FocusLost => false,
        }
    }

    /// Tests one raw event against the plan's class list.
    ///
    /// Called once per level event, after the fold: a class binding never competes on specificity,
    /// so it only ever sees a control no plain binding in this context already indexes, and only
    /// while that control reads as actuated and nothing else has already consumed it this schedule.
    fn class_dispatch(
        &mut self,
        event: &RawEvent,
        consumed: &ConsumedControls,
        claims: &mut Vec<Control>,
    ) {
        let Some(control) = event.control() else {
            return;
        };
        if !self.actuated(event) || consumed.contains(control) || self.plan.is_indexed(control) {
            return;
        }
        let Some(binding_index) = self
            .plan
            .class_bindings()
            .iter()
            .position(|binding| binding.class.contains_event(event))
        else {
            return;
        };
        self.class_fires.push(ClassFire {
            binding_index,
            event: event.clone(),
        });
        if self.plan.class_bindings()[binding_index].consume {
            claims.push(control);
        }
    }

    /// Resolves one half of the plan against the current device state.
    fn fold(
        &mut self,
        threshold: &ButtonThreshold,
        mouse_delta: Vec2,
        delta: f32,
        kind: Fold,
        consumed: &ConsumedControls,
        claims: &mut Vec<Control>,
    ) {
        // Field-level borrows: the fold reads the device state and the plan while writing actions.
        let Self {
            plan,
            actions,
            dirty,
            transitions,
            require_reset,
            scratch,
            tunable_scratch,
            chord_claims,
            #[cfg(feature = "keyboard")]
            held_buttons,
            #[cfg(feature = "mouse")]
            held_mouse_buttons,
            #[cfg(feature = "gamepad")]
            held_gamepad_buttons,
            #[cfg(feature = "gamepad")]
            held_gamepad_axes,
            ..
        } = self;

        // One predicate for every button-shaped part, so a composite and a plain button binding
        // can never disagree about what "pressed" means.
        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        let is_pressed = |control: ButtonControl| {
            // A control another context has taken reads as untouched, rather than being skipped:
            // one part of a composite going away should leave the other three working.
            if consumed.contains(control.into()) {
                return false;
            }
            match control {
                #[cfg(feature = "keyboard")]
                ButtonControl::Key(key) => held_buttons.contains(&key),
                #[cfg(feature = "mouse")]
                ButtonControl::MouseButton(button) => held_mouse_buttons.contains(&button),
                #[cfg(feature = "gamepad")]
                ButtonControl::GamepadButton(button) => held_gamepad_buttons
                    .get(&button)
                    .is_some_and(|reading| reading.pressed),
            }
        };

        // Which chord has the strongest claim on each control. Computed before anything is read,
        // because a binding cannot know it is out-ranked without looking at the others — and it is
        // a pure function of what is held, so it costs nothing stateful and can be redone per fold.
        chord_claims.clear();
        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        if plan.has_chords() {
            for binding in plan.bindings() {
                if !binding.chord.iter().copied().all(&is_pressed) {
                    continue;
                }
                binding.source.for_each_control(|control| {
                    match chord_claims.iter_mut().find(|(seen, _)| *seen == control) {
                        Some((_, best)) => *best = (*best).max(binding.chord_len),
                        None => chord_claims.push((control, binding.chord_len)),
                    }
                });
            }
        }
        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        let out_ranked = |binding: &crate::plan::CompiledBinding| {
            let mut lost = false;
            binding.source.for_each_control(|control| {
                lost |= chord_claims
                    .iter()
                    .any(|&(seen, best)| seen == control && best > binding.chord_len);
            });
            lost
        };

        let bindings = plan.bindings();

        // Every group of bindings sharing a `hold_or_toggle` key resolves its latch once per tick,
        // from the combined actuation of every member — computed here, before any binding's own
        // evaluation, for the same reason `chord_claims` is: a binding cannot resolve a fact about
        // the whole group from partway through visiting it. See `resolve_shared_toggle`'s own doc
        // for what goes wrong resolving this per binding instead. Most plans share none, and this
        // loop then runs zero times.
        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        for (scratch_index, cell) in tunable_scratch.iter_mut().enumerate() {
            let actuated = bindings.iter().any(|binding| {
                binding.tunable_shared == Some(scratch_index)
                    && crate::binding::as_button_control(&binding.source).is_some_and(&is_pressed)
            });
            crate::binding::resolve_shared_toggle(actuated, cell);
        }

        let mut index = 0;
        while index < bindings.len() {
            let slot = bindings[index].slot;
            let intent = plan.intent_for_slot(slot);

            // A slot belongs to exactly one half, and `Intent::accepts` is what guarantees it: a
            // `Delta2` action admits only delta-shaped sources and every other intent admits none,
            // so no slot can want both passes.
            let wanted = match kind {
                Fold::Delta => intent == Intent::Delta2,
                Fold::Level | Fold::Interrupted => intent != Intent::Delta2,
            };
            if !wanted {
                while index < bindings.len() && bindings[index].slot == slot {
                    index += 1;
                }
                continue;
            }

            let mut combined = None;
            let mut best = Verdict::Idle;

            // Bindings are grouped by slot, so this inner walk is one action's contributions.
            while index < bindings.len() && bindings[index].slot == slot {
                let binding = &bindings[index];

                // Two ways to be out of the running before the control is even read: the chord this
                // binding needs is not held, or a longer one on the same control is (R8.1).
                #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
                let held_back = !binding.chord.iter().copied().all(&is_pressed)
                    || (plan.has_chords() && out_ranked(binding));
                #[cfg(not(any(feature = "keyboard", feature = "mouse", feature = "gamepad")))]
                let held_back = false;

                let value = match binding.source {
                    #[cfg(feature = "keyboard")]
                    BindingSource::Button(key_code) => ActionValue::Bool(
                        !consumed.contains(Control::Key(key_code))
                            && held_buttons.contains(&key_code),
                    ),
                    #[cfg(feature = "mouse")]
                    BindingSource::MouseButton(button) => ActionValue::Bool(
                        !consumed.contains(Control::MouseButton(button))
                            && held_mouse_buttons.contains(&button),
                    ),
                    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
                    BindingSource::Axis1(parts) => ActionValue::Axis1(axis_from_buttons(
                        is_pressed(parts.negative),
                        is_pressed(parts.positive),
                    )),
                    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
                    BindingSource::Directional2(parts) => {
                        // Four keys and a D-pad reach an action through this same arm, which is
                        // the whole point of the composite.
                        let x = axis_from_buttons(is_pressed(parts.left), is_pressed(parts.right));
                        let y = axis_from_buttons(is_pressed(parts.down), is_pressed(parts.up));
                        ActionValue::Axis2(Vec2::new(x, y))
                    }
                    BindingSource::MouseMotion => {
                        ActionValue::Axis2(if consumed.contains(Control::MouseMotion) {
                            Vec2::ZERO
                        } else {
                            mouse_delta
                        })
                    }
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
                        ActionValue::Axis1(if consumed.contains(Control::GamepadAxis(axis)) {
                            0.0
                        } else {
                            held_gamepad_axes.get(&axis).copied().unwrap_or(0.0)
                        })
                    }
                    #[cfg(feature = "gamepad")]
                    BindingSource::GamepadStick(stick) => {
                        ActionValue::Axis2(gamepad_stick_value(held_gamepad_axes, stick))
                    }
                };

                // Three disjoint pieces of this binding's working memory, in the order they are
                // used: reshape the value, decide whether it is a press, then judge it.
                let owned = &mut scratch
                    [binding.scratch_base..binding.scratch_base + binding.scratch_len()];
                let (modifier_scratch, rest) = owned.split_at_mut(binding.modifiers.len());
                let (condition_scratch, press_scratch) =
                    rest.split_at_mut(binding.conditions.len());

                // Conditions still run: a hold that loses its control has to be told, or it would
                // resume from where it left off when the control came back.
                let value = if held_back {
                    ActionValue::Bool(false)
                } else {
                    value
                };
                // A binding whose tunable is shared (`hold_or_toggle` reaching a primary and a
                // secondary key, most often) does not run its own modifier chain at all — the
                // group-wide pre-pass above has already resolved this tick's latch once, from every
                // sharing binding's raw actuation combined, and every member simply reads that back.
                // Running each binding's chain independently here, against its own private scratch,
                // is exactly what let one binding's evaluation order clobber another's edge
                // detection before the pre-pass existed.
                let value = match binding.tunable_shared {
                    Some(scratch_index) => ActionValue::Bool(crate::binding::toggle_latch(
                        &tunable_scratch[scratch_index],
                    )),
                    None => apply_modifiers(value, &binding.modifiers, modifier_scratch, delta),
                };
                // Where a press comes from something that was not already a press, the threshold
                // has to settle it here. Reading it later cannot: by then the only question a
                // stored value can answer is whether it is off centre, and a resting stick always
                // is. Modifiers run first so that a deadzone gets to define centre.
                //
                // Hysteretic like the button channel's own, but remembered per *binding* rather
                // than per control: the value here was assembled from a deadzone, a composite, or
                // whatever else the chain did, and no single control owns the answer.
                let value = match (intent, value) {
                    (Intent::Button, ActionValue::Bool(_)) => value,
                    (Intent::Button, _) => {
                        let memory = &mut press_scratch[0];
                        let pressed = threshold.pressed(magnitude(value), memory.prev.to_bool());
                        memory.prev = ActionValue::Bool(pressed);
                        ActionValue::Bool(pressed)
                    }
                    _ => value,
                };
                // Conditions decide *whether* this binding is firing; the value it contributes is
                // rest until it is. A hold half-finished must not move the ship.
                let verdict =
                    crate::condition::combine(&binding.conditions, value, condition_scratch, delta);
                if verdict > best {
                    best = verdict;
                }
                // Claimed while the binding has something to say, so a binding that is merely bound
                // to a control does not hold it against everyone else all the time — but one whose
                // condition is part way through does. Firing alone is too narrow: a menu binding
                // that fires once per direction entered would hand the stick back to the game
                // between two crossings, and a charging hold would leak its key to whatever is
                // underneath until it completed.
                if binding.consume && verdict >= Verdict::Ongoing {
                    claims.extend(binding.source.controls());
                }
                let value = if verdict == Verdict::Fired {
                    value
                } else {
                    ActionValue::Bool(false)
                };

                combined = Some(match combined {
                    Some(previous) => combine(previous, value, intent),
                    None => value,
                });
                index += 1;
            }

            if let Some(value) = combined {
                // Held over from before this context activated: report rest until the player lets
                // go once, then let the action behave normally (R7.5).
                //
                // Button intents only. What R7.5 guards is a *press* synthesized from a control the
                // player was already holding, and an analog action has no press to synthesize — its
                // value simply resumes. Holding one back until it reads exactly rest can wedge it
                // forever, because an axis is not obliged to ever read rest: a drifting stick whose
                // deadzone the player has taken to zero never does, and the action never recovers.
                if require_reset[slot] && intent == Intent::Button {
                    if value.to_bool() {
                        continue;
                    }
                    require_reset[slot] = false;
                }

                // Compared rather than inferred from the phase: a held stick reports `Ongoing`
                // every tick while its value moves, and an action whose value moved has changed
                // as surely as one that started or stopped.
                let before = actions[slot];
                let phase = update_action_state(&mut actions[slot], value, best, kind);
                if actions[slot] != before {
                    dirty.set(slot, true);
                }
                // Only the edges. `Idle` and `Ongoing` say that nothing changed, and an observer
                // firing every tick for a held button would be noise rather than information.
                if matches!(phase, Phase::Fired | Phase::Completed | Phase::Canceled) {
                    transitions.push(Transition { slot, phase, value });
                }
            }
        }
    }
}

/// Combines one more binding's contribution into an action's value.
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

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
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
    scratch: &mut [crate::action::Scratch],
    delta: f32,
) -> ActionValue {
    for (modifier, scratch) in modifiers.iter().zip(scratch) {
        value = modifier.apply(value, scratch, delta);
    }
    value
}

/// Moves one action's state on by a tick, and reports the edge if there was one.
///
/// The verdict says what the bindings decided; this decides what that means given where the action
/// already was. Two states are distinguished by the *value* rather than by the phase: an action
/// that is `Ongoing` with a value is firing, and one that is `Ongoing` at rest is a condition still
/// building toward firing. That is what makes giving up on a hold a `Canceled` rather than a
/// `Completed` — the action never actually happened.
///
/// `kind` is `Fold::Interrupted` for a pass forced by a source disappearing rather than an ordinary
/// release; only there does a firing-then-idle transition become `Canceled` instead of `Completed`.
fn update_action_state(
    action_state: &mut crate::action::ActionState,
    value: ActionValue,
    verdict: Verdict,
    kind: Fold,
) -> Phase {
    let was_firing = matches!(
        action_state.phase,
        Phase::Fired | Phase::Ongoing if action_state.value.to_bool()
    );
    let was_building = matches!(action_state.phase, Phase::Started)
        || matches!(action_state.phase, Phase::Ongoing if !action_state.value.to_bool());

    let phase = match verdict {
        Verdict::Fired => {
            if was_firing {
                Phase::Ongoing
            } else {
                Phase::Fired
            }
        }
        Verdict::Ongoing => {
            if was_firing {
                // It was firing and has fallen back to merely building, which from the outside is
                // the action ending.
                Phase::Completed
            } else if was_building {
                Phase::Ongoing
            } else {
                Phase::Started
            }
        }
        Verdict::Idle => {
            if was_firing {
                if kind == Fold::Interrupted {
                    Phase::Canceled
                } else {
                    Phase::Completed
                }
            } else if was_building {
                Phase::Canceled
            } else {
                Phase::Idle
            }
        }
    };

    action_state.value = value;
    action_state.phase = phase;
    phase
}

#[cfg(feature = "gamepad")]
fn gamepad_stick_value(
    axes: &bevy_platform::collections::HashMap<GamepadAxis, f32>,
    stick: Stick,
) -> Vec2 {
    let (x_axis, y_axis) = stick.axes();
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

    /// A plausible fixed timestep, for the tests that do not care what it is.
    const TICK: f32 = 1.0 / 64.0;

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
        assert_eq!(state.transitions[0].phase, Phase::Fired);

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
        assert_eq!(state.transitions[0].phase, Phase::Completed);
    }

    /// A player who taps faster than the tick rate still tapped, and collapsing the window to its
    /// final state loses the whole event: press and release cancel in the held state, and a single
    /// fold afterwards sees nothing happen at all.
    ///
    /// Polling cannot express this — one `Phase` per read — which is why the log exists.
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
        assert_eq!(phases, [Phase::Fired], "one movement, one transition");
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
            Phase::Idle,
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
            Phase::Idle,
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
        assert_eq!(state.phase::<Jump>(), Phase::Idle);
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
        assert_eq!(state.phase::<Jump>(), Phase::Fired);
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
        assert_eq!(state.phase::<Jump>(), Phase::Fired);
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
        assert_eq!(state.phase::<Jump>(), Phase::Fired);
        state.transitions.clear();

        state.deactivate();

        let phases: Vec<_> = state.transitions.iter().map(|t| t.phase).collect();
        assert_eq!(phases, [Phase::Canceled]);
        assert_eq!(state.phase::<Jump>(), Phase::Canceled);
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
        assert_eq!(state.phase::<Jump>(), Phase::Fired);
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
            [Phase::Canceled],
            "not Completed: nothing was let go"
        );
        assert!(!state.value::<Jump>());
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
        assert_eq!(state.phase::<Jump>(), Phase::Fired);
    }

    /// The gamepad half of the same policy: a disconnect leaves no release event to correct a stale
    /// reading, so the crate has to notice the connection event itself and cancel what it was
    /// holding rather than leave it stuck.
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
        assert_eq!(state.phase::<Jump>(), Phase::Fired);
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
        assert_eq!(phases, [Phase::Canceled]);
        assert!(!state.value::<Jump>());
    }

    /// Neither trigger reaches past the device it names. An action held through a surviving
    /// binding must survive — over-cancelling a keyboard-driven `Jump` because an unrelated gamepad
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
        assert_eq!(state.phase::<Jump>(), Phase::Fired);
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

            const INTENT: Intent = Intent::Delta2;
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

            const INTENT: Intent = Intent::Delta2;
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
    fn every_modifier_in_a_chain_has_its_own_scratch() {
        struct Remembering;

        impl crate::binding::Modifier for Remembering {
            fn apply(
                &self,
                _value: ActionValue,
                scratch: &mut crate::action::Scratch,
                _delta: f32,
            ) -> ActionValue {
                scratch.count += 1;
                ActionValue::Axis1(f32::from(scratch.count))
            }
        }

        struct Counted;

        impl InputAction for Counted {
            type Output = f32;

            const INTENT: Intent = Intent::Analog1;
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

    /// Two bindings sharing a `hold_or_toggle` key — a primary and a secondary, the way
    /// Disasteroids' `Thrust` is — read one latch rather than two. Pressing either one flips it;
    /// which one pressed last time is not remembered anywhere.
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
                let (bindings, class_bindings) = builder.finish();
                Plan::from_bindings(bindings, class_bindings)
            }),
            None,
        );
        let threshold = ButtonThreshold::default();
        // One frame for the whole test, its events accumulating over time — the same reason every
        // other test here does, since `apply_frame` tracks how much of it has been read rather than
        // each call bringing its own.
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

    /// A hold, all the way through and then abandoned. The distinction the phases exist for is that
    /// giving up part way is visibly different from seeing it through, and neither is silence.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_hold_starts_fires_completes_and_can_be_abandoned() {
        use bevy_input::keyboard::KeyCode;

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(KeyCode::Space).hold(0.25);
        let plan = Arc::new({
            let (bindings, class_bindings) = builder.finish();
            Plan::from_bindings(bindings, class_bindings)
        });
        let mut state = InputContextState::<Flying>::new(plan, None);
        let threshold = ButtonThreshold::default();
        let mut frame = InputFrame::default();

        let step = |state: &mut InputContextState<Flying>, frame: &InputFrame| {
            state.transitions.clear();
            state.apply_frame(
                frame,
                &threshold,
                0.1,
                &ConsumedControls::default(),
                &mut Vec::new(),
                None,
            );
            state.phase::<Jump>()
        };

        // Press, and wait it out.
        frame.record(key(ButtonState::Pressed));
        assert_eq!(step(&mut state, &frame), Phase::Started);
        assert_eq!(step(&mut state, &frame), Phase::Ongoing, "still charging");
        assert!(!state.value::<Jump>(), "and not yet jumping");
        assert_eq!(step(&mut state, &frame), Phase::Fired, "0.3s is past 0.25s");
        assert!(state.value::<Jump>());
        assert_eq!(step(&mut state, &frame), Phase::Ongoing, "still held");

        frame.record(key(ButtonState::Released));
        assert_eq!(step(&mut state, &frame), Phase::Completed);

        // Now the same press, given up on early.
        frame.record(key(ButtonState::Pressed));
        assert_eq!(step(&mut state, &frame), Phase::Started);
        frame.record(key(ButtonState::Released));
        assert_eq!(
            step(&mut state, &frame),
            Phase::Canceled,
            "abandoned before it ever fired"
        );
        assert!(!state.value::<Jump>());
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
        assert_eq!(state.phase::<Jump>(), Phase::Started);

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
        assert_eq!(state.phase::<Jump>(), Phase::Fired);
        assert!(state.value::<Jump>());
    }

    /// A press derived from an axis was thresholded with no memory of what it decided last tick, so
    /// a stick wobbling across the line chattered — the same defect the button channel had fixed,
    /// in the neighbouring case.
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

    /// The mechanism's whole point: an unindexed, class-matching key fires the class binding and,
    /// once `consume` is set, is claimed the same way a plain consuming binding claims its control.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_class_binding_fires_and_consumes_an_unclaimed_key() {
        use crate::capture::ControlClass;

        let mut builder = InputContextBuilder::<Flying>::default();
        builder
            .bind_class::<CharacterInput>(ControlClass::CharacterProducing)
            .consume();
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
            alloc::vec![Control::Key(bevy_input::keyboard::KeyCode::KeyA)]
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
        use crate::capture::ControlClass;

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind_class::<CharacterInput>(ControlClass::CharacterProducing);
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
}
