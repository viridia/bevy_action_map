//! Which controls have been claimed this frame, and the systems that clear the claims.

use alloc::vec::Vec;

use crate::binding::Control;
use crate::device::DeviceHandleSet;

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
pub(super) fn claim_reaches(
    claimed: Option<&DeviceHandleSet>,
    reader: Option<&DeviceHandleSet>,
) -> bool {
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
            .find(|claim| claim.control == control && claim_reaches(claim.devices.as_ref(), reader))
            .map(|claim| claim.by)
    }

    /// Whether anything at all is claimed, from anyone.
    pub(super) fn is_empty(&self) -> bool {
        self.claims.is_empty()
    }

    pub(super) fn claim<S: bevy_ecs::schedule::ScheduleLabel>(
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

/// Starts one run of a schedule with nothing claimed *by that schedule*.
pub(crate) fn release_consumed_in<S: bevy_ecs::schedule::ScheduleLabel>(
    mut consumed: bevy_ecs::prelude::ResMut<'_, ConsumedControls>,
) {
    consumed.release::<S>();
}
