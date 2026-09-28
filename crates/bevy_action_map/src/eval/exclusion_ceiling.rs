//! Which exclusive contexts are suppressing the ones below them this frame.

use alloc::vec::Vec;

use super::consumed_controls::claim_reaches;
use crate::device::DeviceHandleSet;

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
    /// As a claim's devices: `None` is a context nobody paired, which shadows everything below it.
    devices: Option<DeviceHandleSet>,
}

impl ExclusionCeiling {
    fn reset(&mut self) {
        self.0.clear();
    }

    /// Records that an exclusive context at this priority is active over these devices. Every run
    /// of that context re-asserts the same entry, so an identical one already standing is left
    /// alone and the list stays a function of the world rather than of the frame's tick count.
    pub(super) fn raise(&mut self, priority: i32, devices: Option<&DeviceHandleSet>) {
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
    pub(super) fn shadows(&self, priority: i32, reader: Option<&DeviceHandleSet>) -> bool {
        self.0
            .iter()
            .any(|entry| priority < entry.priority && claim_reaches(entry.devices.as_ref(), reader))
    }
}

/// Starts a frame with no exclusion in effect — the same clear point as
/// [`release_consumed_controls`](super::release_consumed_controls), for the same reason.
pub(crate) fn reset_exclusion_ceiling(
    mut ceiling: bevy_ecs::prelude::ResMut<'_, ExclusionCeiling>,
) {
    ceiling.reset();
}
