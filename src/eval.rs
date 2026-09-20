//! The evaluator: a plan and an input frame in, action state and a transition log out.

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
use fixedbitset::FixedBitSet;

use crate::action::{ActionIntent, ActionPhase, ActionValue, InputContext};
use crate::backend::AuthorityValues;
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
use crate::binding::ButtonControl;
#[cfg(feature = "gamepad")]
use crate::binding::Stick;
use crate::binding::{BindingInput, ButtonThreshold, Control};
use crate::condition::ConditionState;
use crate::context::InputContextState;
use crate::device::DeviceHandleSet;
use crate::frame::{InputFrame, RawEvent, TimedRawEvent};

/// Which controls have already been claimed this frame, by which schedule, and for whose devices.
///
/// What `PreUpdate` claimed stays claimed for every fixed tick in the frame; what one fixed tick
/// claimed does not bind the next; and a frame where no fixed tick runs starts clear regardless.
///
/// # A claim is scoped to the devices it was made for
///
/// A claim records the devices the claiming context reads. Two players pressing the same button on
/// two different pads are pressing two different things, so one player's menu consuming `South`
/// leaves the other player's gameplay context free to read it. A context with no
/// [`Paired`](crate::player::Paired) reads every device, so its claims reach every reader and every
/// claim reaches it — which is the single-player case, and it needs no opt-in.
#[derive(bevy_ecs::resource::Resource, Default)]
pub struct ConsumedControls {
    // A flat list rather than a map: a read matches a control *and* an overlapping device set,
    // which no single key expresses. A frame holds a handful of claims.
    claims: Vec<Claim>,
}

struct Claim {
    schedule: core::any::TypeId,
    control: Control,
    /// The devices the claiming context reads; `None` for a context nobody paired, which reads
    /// every device. Not a `Paired`, which is how a context entity carries this and nothing else.
    devices: Option<DeviceHandleSet>,
    /// Which context took it, not merely that something did: "consumed" is one of five reasons an
    /// action can silently not fire (R22.1), and the only useful form of the answer names the
    /// taker.
    by: &'static str,
}

/// Whether a claim made for `claimed` devices reaches a reader that reads `reader` devices.
///
/// One question: do the two device sets overlap, where `None` stands for all of them. A reader that
/// owns nothing is the case worth spelling out — it cannot lose what it never heard, so no claim
/// reaches it, not even one made by a context nobody paired.
fn reaches(claimed: Option<&DeviceHandleSet>, reader: Option<&DeviceHandleSet>) -> bool {
    match (claimed, reader) {
        (_, None) => true,
        (None, Some(reader)) => !reader.is_empty(),
        (Some(claimed), Some(reader)) => claimed.intersects(reader),
    }
}

impl ConsumedControls {
    /// Whether this control has been claimed away from a reader that reads these devices.
    ///
    /// Pass the reader's [`Paired`](crate::player::Paired) devices, or `None` for a context that
    /// reads every device.
    pub fn contains(&self, control: Control, reader: Option<&DeviceHandleSet>) -> bool {
        self.claimant(control, reader).is_some()
    }

    /// The path of the context that claimed this control away from such a reader, if one did.
    ///
    /// A stick counts as claimed when either of its axes is.
    pub fn claimant(
        &self,
        control: Control,
        reader: Option<&DeviceHandleSet>,
    ) -> Option<&'static str> {
        #[cfg(feature = "gamepad")]
        if let Control::GamepadStick(stick) = control {
            let (x, y) = stick.axes();
            return self
                .claimant(Control::GamepadAxis(x), reader)
                .or_else(|| self.claimant(Control::GamepadAxis(y), reader));
        }
        self.claims
            .iter()
            .find(|claim| claim.control == control && reaches(claim.devices.as_ref(), reader))
            .map(|claim| claim.by)
    }

    fn claim<S: bevy_ecs::schedule::ScheduleLabel>(
        &mut self,
        control: Control,
        devices: Option<&DeviceHandleSet>,
        by: &'static str,
    ) {
        // Stored as its two axes, which is how a context's claim on a stick already arrives and
        // what every reader checks, so a capture's whole-stick claim is not the one they miss.
        #[cfg(feature = "gamepad")]
        if let Control::GamepadStick(stick) = control {
            let (x, y) = stick.axes();
            self.claim::<S>(Control::GamepadAxis(x), devices, by);
            self.claim::<S>(Control::GamepadAxis(y), devices, by);
            return;
        }
        self.claims.push(Claim {
            schedule: core::any::TypeId::of::<S>(),
            control,
            devices: devices.cloned(),
            by,
        });
    }

    /// Takes a control on behalf of a live capture, so that what a player presses at a rebinding
    /// screen does not also play the game.
    ///
    /// Scoped to the session's own devices for the same reason a context's claim is: two players
    /// rebinding at once are pressing two different things.
    ///
    /// Claimed under `PreUpdate`, where capture runs, which is what carries it through to the fixed
    /// schedules: a fixed tick releases only its own claims, so this one still stands when a
    /// fixed-tick context evaluates later in the frame.
    pub(crate) fn claim_for_capture(
        &mut self,
        control: Control,
        devices: Option<&DeviceHandleSet>,
    ) {
        self.claim::<bevy_app::PreUpdate>(control, devices, "capture");
    }

    /// Forgets what one schedule claimed, which it does on entry so that each run decides afresh.
    fn release<S: bevy_ecs::schedule::ScheduleLabel>(&mut self) {
        let schedule = core::any::TypeId::of::<S>();
        self.claims.retain(|claim| claim.schedule != schedule);
    }

    fn release_all(&mut self) {
        self.claims.clear();
    }
}

/// Starts a frame with nothing claimed.
pub(crate) fn release_consumed_controls(
    mut consumed: bevy_ecs::prelude::ResMut<'_, ConsumedControls>,
) {
    consumed.release_all();
}

/// The active exclusive contexts seen so far this frame, each with the devices it holds.
///
/// One ceiling for the whole world would let one player's pause menu deactivate another player's
/// gameplay, so an entry carries the same device scope a claim does and shadows only a context that
/// shares a device with it.
///
/// Unlike `ConsumedControls`, this needs no per-schedule bookkeeping: a context's activity does not
/// reset between fixed ticks the way a control's actuation does, so an exclusive context re-raises
/// the same entry every time it runs. Reset once at the top of the frame, set by whichever
/// exclusive context runs first in priority order, and read by everything lower that runs after it
/// for the rest of the frame. Render-tick contexts run before fixed-tick ones, so exclusion flows
/// forward through the frame the same way consumption does.
#[derive(bevy_ecs::resource::Resource, Default)]
pub(crate) struct ExclusionCeiling(Vec<Exclusion>);

struct Exclusion {
    priority: i32,
    /// As [`Claim::devices`]: `None` is a context nobody paired, which shadows everything below it.
    devices: Option<DeviceHandleSet>,
}

impl ExclusionCeiling {
    fn reset(&mut self) {
        self.0.clear();
    }

    /// Records that an exclusive context at this priority is active over these devices. Every run
    /// of that context re-asserts the same entry, so an identical one already standing is left
    /// alone and the list stays a function of the world rather than of the frame's tick count.
    fn raise(&mut self, priority: i32, devices: Option<&DeviceHandleSet>) {
        if self
            .0
            .iter()
            .any(|entry| entry.priority == priority && entry.devices.as_ref() == devices)
        {
            return;
        }
        self.0.push(Exclusion {
            priority,
            devices: devices.cloned(),
        });
    }

    /// Whether a context at this priority reading these devices is shadowed by an exclusive one
    /// that has already run and shares a device with it.
    fn shadows(&self, priority: i32, reader: Option<&DeviceHandleSet>) -> bool {
        self.0
            .iter()
            .any(|entry| priority < entry.priority && reaches(entry.devices.as_ref(), reader))
    }
}

/// Starts a frame with no exclusion in effect — the same clear point as
/// [`release_consumed_controls`], for the same reason.
pub(crate) fn reset_exclusion_ceiling(
    mut ceiling: bevy_ecs::prelude::ResMut<'_, ExclusionCeiling>,
) {
    ceiling.reset();
}

/// Starts one run of a schedule with nothing claimed *by that schedule*.
pub(crate) fn release_consumed_in<S: bevy_ecs::schedule::ScheduleLabel>(
    mut consumed: bevy_ecs::prelude::ResMut<'_, ConsumedControls>,
) {
    consumed.release::<S>();
}

/// One phase change, in the order it happened.
///
/// The log records transitions rather than final state: an action that fires and completes inside
/// one tick has two of these, and a reader that only ever sees the current phase cannot express
/// that.
pub(crate) struct Transition {
    pub(crate) slot: usize,
    pub(crate) phase: ActionPhase,
    pub(crate) value: ActionValue,
}

/// Turns each logged transition into its typed event.
///
/// Separate from evaluation because observers run arbitrary code with `&mut World`, and the
/// evaluator has to stay a pure function of its inputs.
pub(crate) fn dispatch_transitions<C: InputContext + Component>(
    mut commands: Commands<'_, '_>,
    mut states: Query<'_, '_, (Entity, &mut InputContextState<C>)>,
) {
    for (entity, mut state) in &mut states {
        if state.transitions.is_empty() {
            continue;
        }
        // Bypass change detection: draining the log is bookkeeping, not a meaningful state change.
        // The evaluation that populated the log already triggered change detection at the right
        // time; re-triggering it here would advance the change tick one system past the actual event.
        let state = state.bypass_change_detection();

        // Handed back afterwards so the allocation survives to the next tick.
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
pub(crate) fn dispatch_class_fires<C: InputContext + Component>(
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
    mut states: Query<
        '_,
        '_,
        (
            &mut InputContextState<C>,
            Option<&crate::player::Paired>,
            Option<&AuthorityValues>,
        ),
    >,
) {
    let delta = time.delta_secs();
    // The ceiling is read inside the loop and raised only after it, so nothing this context does
    // can affect its own shadowing — neither an instance shadowing its siblings, nor a context
    // shadowing itself. Evaluation order is priority order (TD5.1, TD5.3), so what is standing
    // here is what higher-priority exclusive contexts already did this frame.
    let mut active_scopes: Vec<Option<DeviceHandleSet>> = Vec::new();
    for (mut state, pairing, authority) in &mut states {
        let devices = pairing.map(|paired| &**paired);
        // Bypassed for the whole pass and re-marked at the end only if an action moved. Every tick
        // writes *something* here — the read cursor at least — so taking the deref at face value
        // would mark every instance changed every tick, which is the all-or-nothing wake-up R23.4
        // asks us not to hand a subscriber.
        let instance = state.bypass_change_detection();
        instance.dirty.clear();

        if ceiling.shadows(C::PRIORITY, devices) {
            instance.shadow();
        } else {
            instance.unshadow();
        }
        if C::EXCLUSIVE && instance.is_active() {
            active_scopes.push(devices.cloned());
        }

        // An instance's claims land scoped to its own devices, so the instance evaluated next
        // reads them only where the two players overlap.
        let mut claims = Vec::new();
        instance.apply_frame(&frame, &threshold, delta, &consumed, &mut claims, devices);
        instance.apply_authority(authority, delta);
        let moved = !instance.dirty.is_clear();
        for control in claims {
            consumed.claim::<S>(control, devices, C::PATH);
        }
        if moved {
            state.set_changed();
        }
    }

    // Only a context that is itself active-and-unshadowed gets to shadow anything below it — which
    // is what makes two stacked exclusive contexts compose correctly with nothing extra: a second
    // exclusive context shadowed by a third does not also shadow whatever the second would have.
    for devices in active_scopes {
        ceiling.raise(C::PRIORITY, devices.as_ref());
    }
}

/// Which half of the plan a fold pass is for.
///
/// The two kinds of input have different temporal semantics, and the split is what lets a fast tap
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
    /// than a player releasing a control. Reads like `Level`, except that a binding which was firing
    /// and reads at rest this pass reports `Canceled` rather than `Completed`, since nothing was let
    /// go. A binding on an unaffected device is untouched.
    Interrupted,
}

/// Whether this event means the source is gone — a window losing focus or a device disconnecting —
/// rather than an ordinary press, release, or motion.
fn interruption_kind(event: &RawEvent) -> Fold {
    match event {
        #[cfg(any(feature = "keyboard", feature = "mouse"))]
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

/// The character a logical binding may name this key by, if it has one.
///
/// A key qualifies when it produces a character of its own. `Key::Character` carrying more than one
/// is a composition rather than a key — an IME committing several keystrokes at once, or a Windows
/// dead key that could not combine and reports both what it held and what followed (R12.6) — and
/// belongs to text entry, not to a binding. Dead keys themselves have produced nothing yet, and the
/// named variants are modifiers and the like, which sit in the same place on every layout and are
/// bound by position.
#[cfg(feature = "keyboard")]
fn bound_character(logical_key: &bevy_input::keyboard::Key) -> Option<char> {
    let bevy_input::keyboard::Key::Character(text) = logical_key else {
        return None;
    };
    let mut characters = text.chars();
    match (characters.next(), characters.next()) {
        (Some(single), None) => Some(crate::binding::normalize_character(single)),
        _ => None,
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
        devices: Option<&DeviceHandleSet>,
    ) {
        // Only what has arrived since this context last looked: re-reading the whole queue counts
        // one mouse delta once per fixed tick in the frame. The cursor advances past the full
        // unfiltered slice — including another device's events an unpaired viewer never touches
        // below — so every event is offered exactly once.
        let unread = frame.events_after(self.read_through);
        if let Some(last) = unread.last() {
            self.read_through = Some(last.timestamp);
        }
        // A device's input must not reach a context paired to someone else (R15.3); a context
        // nobody paired hears every device, which is today's exact behaviour.
        let owns =
            |event: &TimedRawEvent| devices.is_none_or(|set| set.contains(event.event.device()));

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

        // Replayed one at a time rather than collapsed: a press and a release inside one window
        // cancel in the held state, and a single fold afterwards sees neither (R9.3).
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
                devices,
            );
            // After the fold, not before: TD5.4's ordering. Checked once here rather
            // than woven into the fold.
            self.class_dispatch(&event.event, consumed, claims, devices);
            level_changes += 1;
        }

        // Time passes even when nothing arrives: a phase has to reach `Firing` from `Fired` on its
        // own, and without an event to prompt it nothing else would.
        if level_changes == 0 {
            self.fold(
                threshold,
                Vec2::ZERO,
                delta,
                Fold::Level,
                consumed,
                claims,
                devices,
            );
        }

        self.fold(
            threshold,
            mouse_delta,
            delta,
            Fold::Delta,
            consumed,
            claims,
            devices,
        );
    }

    /// Moves one control's held state, for the inputs that have a state to hold.
    fn apply_level_event(&mut self, event: &RawEvent, threshold: &ButtonThreshold) {
        #[cfg(not(feature = "gamepad"))]
        let _ = threshold;

        match event {
            #[cfg(feature = "keyboard")]
            RawEvent::Keyboard(KeyboardInput {
                key_code,
                logical_key,
                state,
                ..
            }) => match state {
                ButtonState::Pressed => {
                    self.held_buttons.insert(*key_code);
                    if let Some(character) = bound_character(logical_key) {
                        self.held_characters.insert(*key_code, character);
                    }
                }
                ButtonState::Released => {
                    self.held_buttons.remove(key_code);
                    self.held_characters.remove(key_code);
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
            #[cfg(any(feature = "keyboard", feature = "mouse"))]
            RawEvent::FocusLost => {
                #[cfg(feature = "keyboard")]
                {
                    self.held_buttons.clear();
                    self.held_characters.clear();
                }
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
            #[cfg(any(feature = "keyboard", feature = "mouse"))]
            RawEvent::FocusLost => false,
        }
    }

    /// Tests one raw event against the plan's class list.
    ///
    /// Called once per level event, after the fold: only a control no plain binding in this context
    /// indexes reaches here, and only while it reads as actuated and no one else has already
    /// consumed it this schedule.
    fn class_dispatch(
        &mut self,
        event: &RawEvent,
        consumed: &ConsumedControls,
        claims: &mut Vec<Control>,
        devices: Option<&DeviceHandleSet>,
    ) {
        let Some(control) = event.control() else {
            return;
        };
        if !self.actuated(event)
            || consumed.contains(control, devices)
            || self.plan.is_indexed(control)
        {
            return;
        }
        let Some(binding_index) = self
            .plan
            .class_bindings()
            .iter()
            .position(|binding| binding.filter.matches(event))
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
    // The last three are one question in three parts — what others took, whose devices this
    // instance reads, what it takes in turn — and a claim only means anything against the devices
    // that made it. Bundling them to satisfy a count would name something the code does not have.
    #[allow(clippy::too_many_arguments)]
    fn fold(
        &mut self,
        threshold: &ButtonThreshold,
        mouse_delta: Vec2,
        delta: f32,
        kind: Fold,
        consumed: &ConsumedControls,
        claims: &mut Vec<Control>,
        devices: Option<&DeviceHandleSet>,
    ) {
        let Self {
            plan,
            actions,
            dirty,
            transitions,
            require_reset,
            disabled,
            scratch,
            tunable_scratch,
            chord_claims,
            #[cfg(feature = "keyboard")]
            held_buttons,
            #[cfg(feature = "keyboard")]
            held_characters,
            #[cfg(feature = "mouse")]
            held_mouse_buttons,
            #[cfg(feature = "gamepad")]
            held_gamepad_buttons,
            #[cfg(feature = "gamepad")]
            held_gamepad_axes,
            ..
        } = self;

        // One predicate for every button-shaped part, so a composite's part and a plain button
        // binding can never disagree about what "pressed" means.
        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        let is_pressed = |control: ButtonControl| {
            // A control another context has taken reads as untouched, rather than being skipped.
            if consumed.contains(control.into(), devices) {
                return false;
            }
            match control {
                #[cfg(feature = "keyboard")]
                ButtonControl::PhysicalKey(key) => held_buttons.contains(&key),
                #[cfg(feature = "keyboard")]
                ButtonControl::LogicalKey(character) => {
                    held_characters.values().any(|&held| held == character)
                }
                #[cfg(feature = "mouse")]
                ButtonControl::MouseButton(button) => held_mouse_buttons.contains(&button),
                #[cfg(feature = "gamepad")]
                ButtonControl::GamepadButton(button) => held_gamepad_buttons
                    .get(&button)
                    .is_some_and(|reading| reading.pressed),
            }
        };

        // A modifier entry is a disjunction: either key of the pair satisfies it, and both being
        // live at once is why this cannot be expanded at bind time into one entry per side.
        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        let entry_held = |entry: crate::binding::ChordEntry| match entry {
            crate::binding::ChordEntry::Control(control) => is_pressed(control),
            #[cfg(feature = "keyboard")]
            crate::binding::ChordEntry::Modifier(modifier) => modifier
                .keys()
                .into_iter()
                .any(|key| is_pressed(ButtonControl::PhysicalKey(key))),
        };

        // Which chord has the strongest claim on each control. Computed before anything is read,
        // because a binding cannot know it is out-ranked without looking at the others — and it is
        // a pure function of what is held, so it keeps no state and can be redone per fold.
        chord_claims.clear();
        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        if plan.has_chords() {
            for binding in plan.bindings() {
                if disabled[binding.slot] || !binding.chord.iter().copied().all(&entry_held) {
                    continue;
                }
                binding.input.for_each_control(|control| {
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
            binding.input.for_each_control(|control| {
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
            let mut actuated = false;
            let mut active = false;
            for binding in bindings.iter().filter(|binding| {
                binding.tunable_shared == Some(scratch_index) && !disabled[binding.slot]
            }) {
                active = crate::binding::toggle_active(&binding.modifiers);
                if crate::binding::as_button_control(&binding.input).is_some_and(&is_pressed) {
                    actuated = true;
                }
            }
            crate::binding::resolve_shared_toggle(actuated, active, cell);
        }

        let mut index = 0;
        while index < bindings.len() {
            let slot = bindings[index].slot;
            let intent = plan.intent_for_slot(slot);

            // A slot belongs to exactly one half, and `ActionIntent::accepts` is what guarantees
            // it: a `Delta2` action admits only delta-shaped inputs and every other intent admits
            // none, so no slot can want both passes.
            let wanted = match kind {
                Fold::Delta => intent == ActionIntent::Delta2,
                Fold::Level | Fold::Interrupted => intent != ActionIntent::Delta2,
            };
            if !wanted || disabled[slot] {
                while index < bindings.len() && bindings[index].slot == slot {
                    index += 1;
                }
                continue;
            }

            let mut folded = Folded::default();
            let mut best = ConditionState::Idle;

            while index < bindings.len() && bindings[index].slot == slot {
                let binding = &bindings[index];

                // Two ways to be out of the running before the control is even read: the chord this
                // binding needs is not held, or a longer one on the same control is (R8.1).
                #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
                let held_back = !binding.chord.iter().copied().all(&entry_held)
                    || (plan.has_chords() && out_ranked(binding));
                #[cfg(not(any(feature = "keyboard", feature = "mouse", feature = "gamepad")))]
                let held_back = false;

                let value =
                    match binding.input {
                        #[cfg(feature = "keyboard")]
                        BindingInput::Button(key_code) => {
                            ActionValue::Bool(is_pressed(ButtonControl::PhysicalKey(key_code)))
                        }
                        #[cfg(feature = "keyboard")]
                        BindingInput::LogicalKey(character) => {
                            ActionValue::Bool(is_pressed(ButtonControl::LogicalKey(character)))
                        }
                        #[cfg(feature = "mouse")]
                        BindingInput::MouseButton(button) => {
                            ActionValue::Bool(is_pressed(ButtonControl::MouseButton(button)))
                        }
                        // Four keys and a D-pad reach an action through this same arm, and the fold
                        // is what turns their parts back into one direction.
                        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
                        BindingInput::Part(button, part) => part_value(part, is_pressed(button)),
                        BindingInput::MouseMotion => ActionValue::Axis2(
                            if consumed.contains(Control::MouseMotion, devices) {
                                Vec2::ZERO
                            } else {
                                mouse_delta
                            },
                        ),
                        // Both views of a button channel, chosen by what the action asked for. A
                        // trigger carries a fraction, so an analog action gets the travel and a
                        // button action gets the thresholded press — R2.10's case, and the reason a
                        // binding cannot be resolved from the input alone.
                        #[cfg(feature = "gamepad")]
                        BindingInput::GamepadButton(button) => match intent {
                            ActionIntent::Button => {
                                ActionValue::Bool(is_pressed(ButtonControl::GamepadButton(button)))
                            }
                            _ => ActionValue::Axis1(
                                if consumed.contains(Control::GamepadButton(button), devices) {
                                    0.0
                                } else {
                                    held_gamepad_buttons
                                        .get(&button)
                                        .map_or(0.0, |reading| reading.value)
                                },
                            ),
                        },
                        #[cfg(feature = "gamepad")]
                        BindingInput::GamepadAxis(axis) => ActionValue::Axis1(
                            if consumed.contains(Control::GamepadAxis(axis), devices) {
                                0.0
                            } else {
                                held_gamepad_axes.get(&axis).copied().unwrap_or(0.0)
                            },
                        ),
                        #[cfg(feature = "gamepad")]
                        BindingInput::GamepadStick(stick) => ActionValue::Axis2(
                            gamepad_stick_value(held_gamepad_axes, stick, consumed, devices),
                        ),
                    };

                // This binding's working memory, split into the three disjoint pieces
                // `CompiledBinding::scratch_base` allocates.
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
                // With the group's toggle on, a binding whose tunable is shared reads back the
                // latch the pre-pass above resolved rather than running its own modifier chain:
                // running each chain independently against its own private scratch lets one
                // binding's evaluation order clobber another's edge detection.
                //
                // In hold mode there is no latch to share: each binding is an ordinary momentary
                // control again, so it falls through to the same modifier chain an unshared binding
                // runs, whose own `Toggle { active: false }` is identity.
                let value = match binding.tunable_shared {
                    Some(scratch_index) if crate::binding::toggle_active(&binding.modifiers) => {
                        ActionValue::Bool(crate::binding::toggle_latch(
                            &tunable_scratch[scratch_index],
                        ))
                    }
                    _ => apply_modifiers(value, &binding.modifiers, modifier_scratch, delta),
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
                    (ActionIntent::Button, ActionValue::Bool(_)) => value,
                    (ActionIntent::Button, _) => {
                        let memory = &mut press_scratch[0];
                        let pressed =
                            threshold.pressed(value.to_axis1().abs(), memory.prev.to_bool());
                        memory.prev = ActionValue::Bool(pressed);
                        ActionValue::Bool(pressed)
                    }
                    _ => value,
                };
                // Conditions decide *whether* this binding is firing; the value it contributes is
                // rest until it is. A hold half-finished must not move the ship.
                let condition_state =
                    crate::condition::combine(&binding.conditions, value, condition_scratch, delta);
                if condition_state > best {
                    best = condition_state;
                }
                // Claimed while the binding has something to say, so a binding that is merely bound
                // to a control does not hold it against everyone else all the time — but one whose
                // condition is part way through does. Firing alone is too narrow: a menu binding
                // that fires once per direction entered would hand the stick back to the game
                // between two crossings, and a charging hold would leak its key to whatever is
                // underneath until it completed.
                if binding.consume && condition_state >= ConditionState::Building {
                    claims.extend(binding.input.controls());
                }
                let value = if condition_state == ConditionState::Satisfied {
                    value
                } else {
                    ActionValue::Bool(false)
                };

                folded = folded.add(value, intent);
                index += 1;
            }

            let value = folded.value();
            let (value, condition_state) = match plan.stage(slot) {
                stage if stage.is_empty() => (value, best),
                stage => {
                    let owned = &mut scratch[stage.scratch_base
                        ..stage.scratch_base + stage.modifiers.len() + stage.conditions.len()];
                    let (modifier_scratch, condition_scratch) =
                        owned.split_at_mut(stage.modifiers.len());
                    let value = apply_modifiers(value, &stage.modifiers, modifier_scratch, delta);
                    if stage.conditions.is_empty() {
                        (value, best)
                    } else {
                        let judged = crate::condition::combine(
                            &stage.conditions,
                            value,
                            condition_scratch,
                            delta,
                        );
                        // A binding part way through a hold contributes rest, which the stage alone
                        // would read as nothing happening, and the action would lose its `Started`.
                        let judged = match (judged, best) {
                            (ConditionState::Idle, ConditionState::Building) => best,
                            _ => judged,
                        };
                        let value = if judged == ConditionState::Satisfied {
                            value
                        } else {
                            ActionValue::Bool(false)
                        };
                        (value, judged)
                    }
                }
            };
            commit_slot(
                Commit {
                    slot,
                    intent,
                    value,
                    condition_state,
                    kind,
                },
                actions,
                dirty,
                require_reset,
                transitions,
            );
        }
    }

    /// Writes the values an outside authority supplies, for the slots this context delegates to it.
    ///
    /// The authority hands over the value the fold would otherwise have produced, and `commit_slot`
    /// synthesizes the edges from it — a backend that reports only a level never has to carry one.
    ///
    /// Once a tick, unlike the fold: the authority is sampled, not replayed. `Fold::Level` for the
    /// same reason — an interruption is this crate's device going away, and the authority's has
    /// not.
    pub(crate) fn apply_authority(&mut self, authority: Option<&AuthorityValues>, delta: f32) {
        // Unlike `apply_frame`, this can stop dead while inactive: the authority is sampled afresh
        // every tick, so there is no held state here to go stale.
        if !self.is_active() {
            return;
        }
        let Self {
            plan,
            actions,
            dirty,
            transitions,
            require_reset,
            disabled,
            ..
        } = self;

        for &slot in plan
            .delegated_slots()
            .iter()
            .filter(|&&slot| !disabled[slot])
        {
            let intent = plan.intent_for_slot(slot);
            let value = authority
                .and_then(|values| values.value_of(plan.slot_actions()[slot]))
                .unwrap_or_else(|| at_rest(intent));
            // The no-conditions path a plain binding takes, rather than a second reading of what a
            // value means: delegating leaves no way to declare a condition in the first place.
            let condition_state = crate::condition::combine(&[], value, &mut [], delta);
            commit_slot(
                Commit {
                    slot,
                    intent,
                    value,
                    condition_state,
                    kind: Fold::Level,
                },
                actions,
                dirty,
                require_reset,
                transitions,
            );
        }
    }
}

/// One slot's resolved value, however it was resolved.
struct Commit {
    slot: usize,
    intent: ActionIntent,
    value: ActionValue,
    condition_state: ConditionState,
    kind: Fold,
}

/// Moves one action's state on, and records the change and the edge.
///
/// The single write path into action state, whether the value came from the fold or from an
/// authority backend. Two implementations of the lifecycle would drift, and the promise that a
/// consumer need not know which produced a value is what that would break.
fn commit_slot(
    commit: Commit,
    actions: &mut [crate::action::ActionState],
    dirty: &mut FixedBitSet,
    require_reset: &mut FixedBitSet,
    transitions: &mut Vec<Transition>,
) {
    let Commit {
        slot,
        intent,
        value,
        condition_state,
        kind,
    } = commit;

    // Held over from before this context activated: report rest until the player lets go once, then
    // let the action behave normally (R7.5).
    //
    // Button intents only. What R7.5 guards is a *press* synthesized from a control the player was
    // already holding, and an analog action has no press to synthesize, only a value that resumes.
    // Holding one back until it reads exactly rest can wedge it forever, because an axis is not
    // obliged to ever read rest: a drifting stick whose deadzone the player has taken to zero never
    // does, and the action never recovers.
    if require_reset[slot] && intent == ActionIntent::Button {
        if value.to_bool() {
            return;
        }
        require_reset.set(slot, false);
    }

    // Compared rather than inferred from the phase: a held stick reports `Firing` every tick while
    // its value moves, and an action whose value moved has changed as surely as one that started or
    // stopped.
    let before = actions[slot];
    let phase = update_action_state(&mut actions[slot], value, condition_state, kind);
    if actions[slot] != before {
        dirty.set(slot, true);
    }
    // Only the edges. The level phases say that nothing changed, and an observer firing every tick
    // for a held button would be noise rather than information.
    if matches!(
        phase,
        ActionPhase::Fired | ActionPhase::Completed | ActionPhase::Canceled
    ) {
        transitions.push(Transition { slot, phase, value });
    }
}

/// What an action of this intent reads when nothing is driving it.
fn at_rest(intent: ActionIntent) -> ActionValue {
    match intent {
        ActionIntent::Button => ActionValue::Bool(false),
        ActionIntent::Analog1 => ActionValue::Axis1(0.0),
        ActionIntent::Directional2 | ActionIntent::Delta2 => ActionValue::Axis2(Vec2::ZERO),
    }
}

/// An action's value part way through the fold, split by sign on each axis (D76).
#[derive(Clone, Copy, Default)]
struct Folded {
    positive: Vec3,
    negative: Vec3,
    /// The most components any contribution carried, so the result does not take its shape from
    /// whichever binding was declared first.
    rank: u8,
}

impl Folded {
    /// Combines one more binding's contribution.
    ///
    /// A delta is a displacement, so two of them add. Everything else is a position or a press,
    /// where adding would be a units error: each sign keeps its strongest contribution, so opposite
    /// directions cancel and like ones do not add. A `Button` contribution is always a `Bool` by
    /// now, which makes that strongest-wins.
    fn add(self, contribution: ActionValue, intent: ActionIntent) -> Self {
        let value = widen(contribution);
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
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
fn part_value(part: crate::binding::BindingPart, pressed: bool) -> ActionValue {
    use crate::binding::BindingPart;
    match (part, pressed) {
        (BindingPart::Negative | BindingPart::Positive, false) => ActionValue::Axis1(0.0),
        (BindingPart::Negative, true) => ActionValue::Axis1(-1.0),
        (BindingPart::Positive, true) => ActionValue::Axis1(1.0),
        (_, false) => ActionValue::Axis2(Vec2::ZERO),
        (BindingPart::Up, true) => ActionValue::Axis2(Vec2::Y),
        (BindingPart::Down, true) => ActionValue::Axis2(Vec2::NEG_Y),
        (BindingPart::Left, true) => ActionValue::Axis2(Vec2::NEG_X),
        (BindingPart::Right, true) => ActionValue::Axis2(Vec2::X),
        // Expansion never builds one.
        (BindingPart::Whole, true) => ActionValue::Bool(false),
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
/// `condition_state` says what the bindings decided; this decides what that means given where the
/// action already was. That is what makes giving up on a hold a `Canceled` rather than a
/// `Completed` — the action never actually happened.
///
/// `kind` decides which of `Completed` and `Canceled` a firing-then-idle transition is — see
/// `Fold::Interrupted`.
fn update_action_state(
    action_state: &mut crate::action::ActionState,
    value: ActionValue,
    condition_state: ConditionState,
    kind: Fold,
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
                if kind == Fold::Interrupted {
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

#[cfg(feature = "gamepad")]
fn gamepad_stick_value(
    axes: &bevy_platform::collections::HashMap<GamepadAxis, f32>,
    stick: Stick,
    consumed: &ConsumedControls,
    devices: Option<&DeviceHandleSet>,
) -> Vec2 {
    let read = |axis| {
        if consumed.contains(Control::GamepadAxis(axis), devices) {
            0.0
        } else {
            axes.get(&axis).copied().unwrap_or(0.0)
        }
    };
    let (x_axis, y_axis) = stick.axes();
    Vec2::new(read(x_axis), read(y_axis))
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

    /// A context that binds nothing and leaves `Serve` to an outside authority.
    fn delegated_context() -> InputContextState<Flying> {
        let mut builder = InputContextBuilder::<Flying>::default();
        builder.delegate::<Serve>();
        let (bindings, class_bindings, delegated) = builder.finish();
        let mut plan = Plan::from_bindings(bindings, class_bindings);
        plan.delegate(delegated);
        InputContextState::<Flying>::new(Arc::new(plan), None)
    }

    /// An authority hands over a level and never an edge — Steam's `GetDigitalActionData` is
    /// sampled when asked, and reports no press or release of its own. The state machine is what
    /// turns that level moving into `Fired` and `Completed`, so gameplay code reads a delegated
    /// action exactly as it reads a bound one.
    #[test]
    fn an_authority_supplies_a_level_and_the_state_machine_makes_the_edges() {
        let mut state = delegated_context();
        let mut values = AuthorityValues::new();

        state.apply_authority(Some(&values), TICK);
        assert!(state.transitions.is_empty(), "rest is not an edge");

        values.set::<Serve>(true);
        state.apply_authority(Some(&values), TICK);
        assert_eq!(state.phase::<Serve>(), ActionPhase::Fired);
        assert!(state.value::<Serve>());

        // Dispatch would have drained it by now.
        state.transitions.clear();

        state.apply_authority(Some(&values), TICK);
        assert!(
            state.transitions.is_empty(),
            "a level that has not moved is not news"
        );

        values.set::<Serve>(false);
        state.apply_authority(Some(&values), TICK);
        assert_eq!(state.phase::<Serve>(), ActionPhase::Completed);
    }

    /// Reading rest rather than refusing is what lets a context be spawned before whatever drives
    /// it exists, and what an authority that has lost its device says by clearing the action.
    #[test]
    fn a_delegated_action_with_no_authority_reads_at_rest() {
        let mut state = delegated_context();
        let mut values = AuthorityValues::new();

        state.apply_authority(None, TICK);
        assert!(!state.value::<Serve>());
        assert!(state.transitions.is_empty());

        values.set::<Serve>(true);
        state.apply_authority(Some(&values), TICK);
        state.transitions.clear();

        values.clear::<Serve>();
        state.apply_authority(Some(&values), TICK);
        assert_eq!(state.phase::<Serve>(), ActionPhase::Completed);
    }

    /// The same guard a bound action gets (R7.5), on the authority's values: a menu closing while
    /// the backend still reports its button down must not read as a fresh press underneath.
    #[test]
    fn a_delegated_action_activating_on_a_held_value_waits_for_rest() {
        let mut state = delegated_context();
        let mut values = AuthorityValues::new();
        values.set::<Serve>(true);

        state.deactivate();
        state.apply_authority(Some(&values), TICK);
        assert_eq!(state.phase::<Serve>(), ActionPhase::Idle);

        state.activate();
        state.apply_authority(Some(&values), TICK);
        assert_eq!(
            state.phase::<Serve>(),
            ActionPhase::Idle,
            "a value held across activation is not a press"
        );

        values.set::<Serve>(false);
        state.apply_authority(Some(&values), TICK);
        values.set::<Serve>(true);
        state.apply_authority(Some(&values), TICK);
        assert_eq!(state.phase::<Serve>(), ActionPhase::Fired);
    }

    /// A disabled delegated action does not listen to its authority either, and comes back under
    /// the same guard as a bound one.
    #[test]
    fn a_disabled_delegated_action_ignores_its_authority() {
        let mut state = delegated_context();
        let mut values = AuthorityValues::new();
        values.set::<Serve>(true);

        state.disable::<Serve>();
        state.apply_authority(Some(&values), TICK);
        assert_eq!(state.phase::<Serve>(), ActionPhase::Idle);

        state.enable::<Serve>();
        state.apply_authority(Some(&values), TICK);
        assert_eq!(
            state.phase::<Serve>(),
            ActionPhase::Idle,
            "a value held across the enable is not a press"
        );
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
                let (bindings, class_bindings, _) = builder.finish();
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
            let (bindings, class_bindings, _) = builder.finish();
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
            let (bindings, class_bindings, _) = builder.finish();
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
            let (bindings, class_bindings, _) = builder.finish();
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
            let (bindings, class_bindings, _) = builder.finish();
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

            const INTENT: ActionIntent = ActionIntent::Analog1;
            const PATH: &'static str = "eval_tests.counted";
        }

        let mut builder = InputContextBuilder::<Flying>::default();
        builder
            .bind::<Counted>(bevy_input::keyboard::KeyCode::Space)
            .custom(Remembering)
            .custom(Remembering);
        let plan = Arc::new({
            let (bindings, class_bindings, _) = builder.finish();
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
                let (mut bindings, class_bindings, _) = builder.finish();
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
                let (bindings, class_bindings, _) = builder.finish();
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

        let mut builder = InputContextBuilder::<Flying>::default();
        builder.bind::<Jump>(KeyCode::Space).hold(0.25);
        let plan = Arc::new({
            let (bindings, class_bindings, _) = builder.finish();
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
        assert_eq!(step(&mut state, &frame), ActionPhase::Started);
        assert_eq!(
            step(&mut state, &frame),
            ActionPhase::Building,
            "still charging"
        );
        assert!(!state.value::<Jump>(), "and not yet jumping");
        assert_eq!(
            step(&mut state, &frame),
            ActionPhase::Fired,
            "0.3s is past 0.25s"
        );
        assert!(state.value::<Jump>());
        assert_eq!(step(&mut state, &frame), ActionPhase::Firing, "still held");

        frame.record(key(ButtonState::Released));
        assert_eq!(step(&mut state, &frame), ActionPhase::Completed);

        // Now the same press, given up on early.
        frame.record(key(ButtonState::Pressed));
        assert_eq!(step(&mut state, &frame), ActionPhase::Started);
        frame.record(key(ButtonState::Released));
        assert_eq!(
            step(&mut state, &frame),
            ActionPhase::Canceled,
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
            let (bindings, class_bindings, _) = builder.finish();
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
            let (bindings, class_bindings, _) = builder.finish();
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
            let (bindings, class_bindings, _) = builder.finish();
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
            let (bindings, class_bindings, _) = builder.finish();
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
            let (bindings, class_bindings, _) = builder.finish();
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
            let (bindings, class_bindings, _) = builder.finish();
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
                let (bindings, class_bindings, _) = builder.finish();
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
        let (bindings, class_bindings, _) = builder.finish();
        let mut plan = Plan::from_bindings(bindings, class_bindings);
        plan.combine(combined);
        InputContextState::<Flying>::new(Arc::new(plan), None)
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

    /// A binding part way through a hold contributes rest to the fold, and a condition on the
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
