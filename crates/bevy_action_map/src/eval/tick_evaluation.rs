//! One context instance's tick: the read cursor, the event loop, pending readings, the closing
//! step, and class dispatch.

use alloc::vec::Vec;

use bevy_platform::sync::Arc;
use fixedbitset::FixedBitSet;

use super::binding_pipeline::{BindingOutput, PipelineRun, record_reading};
use super::binding_reading::{BindingReading, ReadingAvailability, read_binding};
use super::{ClassFire, ConsumedControls};
use crate::action::{ActionIntent, InputContext};
use crate::binding::{BindingInput, ButtonThreshold, Control};
use crate::condition::ConditionState;
use crate::context::InputContextState;
use crate::device::DeviceHandleSet;
use crate::frame::{InputFrame, RawEvent, TimedRawEvent};
use crate::plan::Plan;

/// Where one binding stands in the tick being evaluated.
///
/// A binding's *reading* is what its input shows at one moment, taken from held state. Its pipeline
/// *records* each reading: modifiers, threshold, require-reset and conditions each update their
/// memory from it, and produce what the binding contributes to its action. One pass of the pipeline
/// recording one reading is a *run*, and no reading is recorded twice.
///
/// A tick takes its events in order, and a binding's pipeline runs each time an event changes its
/// reading and once more at the closing step. Between runs the binding waits here, one per binding
/// in the plan: its latest reading, whether that reading has been recorded, and the output of its
/// last run, which is what its action combines when it commits.
#[derive(Clone, Copy, Default)]
pub(crate) struct BindingProgress {
    /// The latest reading taken, recorded or not.
    pub(crate) reading: BindingReading,
    /// `reading` has not been recorded yet. A tick begins with none pending, except where a reading
    /// moved between ticks, with no event to report it.
    pub(crate) pending: bool,
    /// The pipeline ran at least once this tick. Cleared after the closing step's commit.
    pub(crate) ran: bool,
    /// What the last run produced: the latest reading's, or while `pending`, an earlier one's.
    pub(crate) output: BindingOutput,
}

impl<C: InputContext> InputContextState<C> {
    /// Runs one tick of this context instance over the events that arrived since its last.
    ///
    /// Three steps. First, readings that can have moved with no event, through a claim or an
    /// authority, are refreshed. Then the events are taken one at a time: each updates held state,
    /// and a binding whose reading it changes has its previous reading recorded, if that one had
    /// not been yet, and its action committed. Last, the closing step runs every binding once more
    /// and commits every action. So a binding's pipeline runs once each time its reading changes
    /// and once at the end, and only the run at the end is handed the tick's `delta` (D97).
    ///
    /// `consumed` is what other contexts have claimed; `claims` collects what this instance grabs,
    /// which reaches `ConsumedControls` only once the instance has finished, so it never reads back
    /// its own claim partway through a tick.
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
        // nobody paired hears every device.
        let owns =
            |event: &TimedRawEvent| devices.is_none_or(|set| set.contains(event.event.device()));

        // An inactive context still tracks its devices, shadowed or not. Skipping that would leave
        // the held state stale, so reactivating would need a rebuild — and R7.6 wants activation to
        // be free.
        if !self.is_active() {
            for event in unread.iter().filter(|e| owns(e)) {
                self.held.apply_event(&event.event, threshold, |_| {});
            }
            self.held.end_tick();
            self.readings_stale = true;
            return;
        }

        // Held apart from `self`, so the plan can be borrowed while bindings run.
        let plan = Arc::clone(&self.plan);

        // Between ticks a reading moves only through a claim arriving or lifting, or what
        // `readings_moved` and `readings_stale` record. With none of these the stored readings
        // stand, which spares an idle tick a pass over every binding.
        let claimed = !consumed.is_empty();
        if self.readings_stale
            || core::mem::take(&mut self.readings_moved)
            || claimed
            || self.claimed_last_tick
        {
            self.refresh_readings(&plan, consumed, devices);
        }
        self.claimed_last_tick = claimed;
        // One at a time rather than collapsed: a press and a release inside one tick cancel in the
        // held state, and a single reading afterwards would see neither (R9.3).
        for event in unread.iter().filter(|e| owns(e)) {
            self.evaluate_event(&plan, &event.event, threshold, consumed, claims, devices);
            self.class_dispatch(&event.event, consumed, claims, devices);
        }
        self.close_tick(&plan, threshold, delta, consumed, claims, devices);
        self.readings_moved |= self.held.end_tick();
    }

    /// Reads every binding afresh. One that moved since the last tick closed becomes pending.
    fn refresh_readings(
        &mut self,
        plan: &Plan,
        consumed: &ConsumedControls,
        devices: Option<&DeviceHandleSet>,
    ) {
        // Readings stored before a spell inactive, before a plan was adopted, or while an action
        // was disabled predate the held state, and a difference from them is no change this context
        // saw. Running one would show an action at rest that the player may never have let go, and
        // clear its require-reset.
        let stale = core::mem::take(&mut self.readings_stale);
        for (index, binding) in plan.bindings().iter().enumerate() {
            let reading =
                read_binding(plan, binding, &self.held, consumed, devices, &self.disabled);
            if stale || self.disabled[binding.slot] {
                self.binding_progress[index].reading = reading;
                continue;
            }
            supersede(
                &mut self.binding_progress[index],
                reading,
                &mut self.require_reset,
                binding.slot,
                plan.intent_for_slot(binding.slot),
            );
        }
    }

    /// Applies one event to held state, and gathers into `affected_bindings` the bindings whose
    /// reading it may have moved: those reading a control it changed, chorded on one, or out-ranked
    /// by a chord on one. Sorted and without duplicates, as positions in the plan's bindings.
    fn update_held_state(&mut self, plan: &Plan, event: &RawEvent, threshold: &ButtonThreshold) {
        let Self {
            held,
            affected_bindings,
            ..
        } = self;
        affected_bindings.clear();
        held.apply_event(event, threshold, |control| {
            affected_bindings.extend_from_slice(plan.bindings_affected_by(control));
        });
        // A key reports its logical character as well, and a binding may be reached through both.
        affected_bindings.sort_unstable();
        affected_bindings.dedup();
    }

    /// Evaluates one event: applies it to held state, then records each affected binding's reading
    /// the event superseded and commits the actions touched.
    ///
    /// A binding's reading is superseded when the event changes it before it has been recorded:
    /// pressed earlier in this tick and released now, say. The earlier reading is recorded here,
    /// with `delta` zero, so the press is seen even though the tick ends released. A binding whose
    /// previous reading has already been recorded just takes the new one, to be recorded at the
    /// closing step or when an event supersedes it in turn.
    fn evaluate_event(
        &mut self,
        plan: &Plan,
        event: &RawEvent,
        threshold: &ButtonThreshold,
        consumed: &ConsumedControls,
        claims: &mut Vec<Control>,
        devices: Option<&DeviceHandleSet>,
    ) {
        self.update_held_state(plan, event, threshold);

        // Taken out of `self` for the loop, and put back after, so its allocation is reused.
        let affected_bindings = core::mem::take(&mut self.affected_bindings);

        // Each enabled binding the event reached has a fresh reading taken. Where that supersedes a
        // reading not yet recorded, the old reading is recorded now and its action is marked to
        // commit. Bindings are grouped by slot, so the slots arrive in order and each action
        // touched commits once, after every one of its superseded readings has been recorded.
        let mut slot_awaiting_commit = None;
        for &index in &affected_bindings {
            let index = index as usize;
            let binding = &plan.bindings()[index];
            if self.disabled[binding.slot] {
                continue;
            }
            let reading =
                read_binding(plan, binding, &self.held, consumed, devices, &self.disabled);
            let Some(superseded_reading) = supersede(
                &mut self.binding_progress[index],
                reading,
                &mut self.require_reset,
                binding.slot,
                plan.intent_for_slot(binding.slot),
            ) else {
                continue;
            };
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
            if let Some(cell) = binding.tunable_shared {
                self.resolve_shared_toggle(plan, cell, Some((index, superseded_reading)));
            }
            self.run_binding_pipeline(plan, index, superseded_reading, 0.0, threshold, claims);
            if let Some(slot) = slot_awaiting_commit
                && slot != binding.slot
            {
                self.commit_action(slot, plan.bindings_of(slot), 0.0);
            }
            slot_awaiting_commit = Some(binding.slot);
        }
        if let Some(slot) = slot_awaiting_commit {
            self.commit_action(slot, plan.bindings_of(slot), 0.0);
        }
        self.affected_bindings = affected_bindings;
    }

    /// The closing step: every enabled binding records its pending or unchanged reading with the
    /// tick's `delta`, and every enabled action commits once.
    fn close_tick(
        &mut self,
        plan: &Plan,
        threshold: &ButtonThreshold,
        delta: f32,
        consumed: &ConsumedControls,
        claims: &mut Vec<Control>,
        devices: Option<&DeviceHandleSet>,
    ) {
        // Each shared `hold_or_toggle` latch resolves before its members run, so they all read the
        // same one. Most plans have none.
        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        for cell in 0..self.tunable_scratch.len() {
            self.resolve_shared_toggle(plan, cell, None);
        }

        // One action at a time: run each of its bindings, then commit it once.
        let bindings = plan.bindings();
        let mut start = 0;
        while start < bindings.len() {
            let slot = bindings[start].slot;
            // Where this action's bindings end: the first binding past `start` on another slot.
            let end = start
                + bindings[start..]
                    .iter()
                    .take_while(|binding| binding.slot == slot)
                    .count();
            if !self.disabled[slot] {
                // Each of the action's bindings records its latest reading, handed the tick's
                // `delta`; `index` is its position in the plan's bindings.
                for (index, binding) in (start..end).zip(&bindings[start..end]) {
                    // Mouse motion is summed across the tick rather than reported per event, so a
                    // motion binding's reading is taken here, once, from the tick's total.
                    let reading = if binding.input == BindingInput::MouseMotion {
                        let reading = read_binding(
                            plan,
                            binding,
                            &self.held,
                            consumed,
                            devices,
                            &self.disabled,
                        );
                        self.binding_progress[index].reading = reading;
                        reading
                    } else {
                        self.binding_progress[index].reading
                    };
                    self.run_binding_pipeline(plan, index, reading, delta, threshold, claims);
                    self.binding_progress[index].pending = false;
                }
                self.commit_action(slot, start..end, delta);
                // Cleared here rather than at the next tick's start, which may not visit them.
                for progress in &mut self.binding_progress[start..end] {
                    progress.ran = false;
                }
            }
            start = end;
        }
    }

    /// Runs one binding's pipeline on `reading`, and notes an action one of whose bindings its
    /// control going away brought to rest.
    fn run_binding_pipeline(
        &mut self,
        plan: &Plan,
        index: usize,
        reading: BindingReading,
        delta: f32,
        threshold: &ButtonThreshold,
        claims: &mut Vec<Control>,
    ) {
        let binding = &plan.bindings()[index];
        let intent = plan.intent_for_slot(binding.slot);
        // The toggle latch of this binding's shared `hold_or_toggle` group, if it has one and the
        // group is in toggle mode. The binding reads it in place of its own modifier chain.
        let shared_latch = binding
            .tunable_shared
            .filter(|_| crate::binding::toggle_active(&binding.modifiers))
            .map(|cell| crate::binding::toggle_latch(&self.tunable_scratch[cell]));
        let run = PipelineRun {
            reading,
            delta,
            // Held over from before this context activated: every binding reads rest until the
            // player lets go once, then the action behaves normally (R7.5). Decided before the
            // conditions rather than on the action's value after them, which a hold still charging
            // reports as rest, and a tap or a hold-and-release would fire on the release.
            //
            // Button intents only. What R7.5 guards is a *press* synthesized from a control the
            // player was already holding, and an analog action has no press to synthesize, only a
            // value that resumes. Holding one back until it reads exactly rest can wedge it
            // forever, because an axis is not obliged to ever read rest: a drifting stick whose
            // deadzone the player has taken to zero never does, and the action never recovers.
            awaiting_release: self.require_reset[binding.slot] && intent == ActionIntent::Button,
            shared_latch,
        };
        let scratch =
            &mut self.scratch[binding.scratch_base..binding.scratch_base + binding.scratch_len()];
        let output = record_reading(binding, intent, run, scratch, threshold, claims);

        let progress = &mut self.binding_progress[index];
        // A binding already at rest when its control went away has nothing to cancel, and marking
        // it would turn another binding's ordinary release into `Canceled`.
        if reading.interrupted() && progress.output.condition != ConditionState::Idle {
            self.interrupted.set(binding.slot, true);
        }
        progress.output = output;
        progress.ran = true;
    }

    /// Resolves one shared `hold_or_toggle` latch from the combined actuation of every enabled
    /// member of the group, each by its latest reading.
    ///
    /// `about_to_record` names the one member, if any, whose pipeline is about to record a
    /// superseded reading: its position in the plan's bindings, and that reading, which stands in
    /// for its latest.
    ///
    /// Resolved for the group rather than by each member's own modifier chain: see `apply_toggle`'s
    /// doc in `binding/modifier.rs` for what goes wrong otherwise.
    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    fn resolve_shared_toggle(
        &mut self,
        plan: &Plan,
        cell: usize,
        about_to_record: Option<(usize, BindingReading)>,
    ) {
        let mut actuated = false;
        let mut active = false;
        for (index, binding) in plan.bindings().iter().enumerate() {
            if binding.tunable_shared != Some(cell) || self.disabled[binding.slot] {
                continue;
            }
            active = crate::binding::toggle_active(&binding.modifiers);
            let reading = match about_to_record {
                Some((recording_index, superseded_reading)) if recording_index == index => {
                    superseded_reading
                }
                _ => self.binding_progress[index].reading,
            };
            // A toggle flips on a press, so only a button-shaped member can actuate it.
            if crate::binding::as_button_control(&binding.input).is_some()
                && reading.value.to_bool()
            {
                actuated = true;
            }
        }
        crate::binding::resolve_shared_toggle(actuated, active, &mut self.tunable_scratch[cell]);
    }

    /// Tests one raw event against the plan's class list.
    ///
    /// A class binding fires on any control of a kind, such as any button for a "press any key"
    /// screen, rather than on one it names. It hands the event itself to its observer, and holds no
    /// state between ticks.
    ///
    /// Called once per event, after its bindings: only a control no plain binding in this context
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
        if !self.held.actuated(event)
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
}

/// Makes `reading` the binding's latest if it differs, and returns the reading it superseded when
/// that one had not been recorded yet, for the caller to record.
///
/// Every fresh reading passes through here, at the tick's refresh and after each event, which is
/// what makes it the place a reading leaving `ClaimedWhileDown` is noticed.
fn supersede(
    progress: &mut BindingProgress,
    reading: BindingReading,
    require_reset: &mut FixedBitSet,
    slot: usize,
    intent: ActionIntent,
) -> Option<BindingReading> {
    if reading == progress.reading {
        return None;
    }
    // A claim lifting off a key the player is still holding hands it back already down, which would
    // read as a press they never made, so the require-reset latch holds it until released (D94,
    // TD5.2). Released while claimed instead, the latch clears at the next commit.
    if progress.reading.availability == ReadingAvailability::ClaimedWhileDown
        && reading.availability != ReadingAvailability::ClaimedWhileDown
        && intent == ActionIntent::Button
    {
        require_reset.set(slot, true);
    }
    let superseded_reading = progress.pending.then_some(progress.reading);
    progress.reading = reading;
    progress.pending = true;
    superseded_reading
}
