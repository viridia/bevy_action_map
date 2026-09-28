//! The systems that run evaluation over every instance of a context and dispatch what it logged.

use alloc::vec::Vec;

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Query, Res};

use super::{ConsumedControls, ExclusionCeiling};
use crate::action::{ActionPhase, ActionValue, InputContext};
use crate::backend::AuthorityValues;
use crate::binding::ButtonThreshold;
use crate::context::InputContextState;
use crate::device::DeviceHandleSet;
use crate::frame::{InputFrame, RawEvent};

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
        // Sampled once a tick, before the frame, so every run in it reads the same level: the
        // authority is polled rather than replayed.
        if instance.is_active() {
            instance.sample_authority(authority);
        }
        let mut claims = Vec::new();
        instance.apply_frame(&frame, &threshold, delta, &consumed, &mut claims, devices);
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
