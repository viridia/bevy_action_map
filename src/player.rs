//! Players, device pairing, and control schemes.
//!
//! This module maps devices to the players that own them, so one player's input never reaches
//! another, and describes the named device requirements a game can assign players against.

use bevy_ecs::prelude::Component;
use core::ops::Deref;

use crate::device::{DeviceHandle, DeviceHandleSet};

/// The devices one occupant's contexts should read, and no others.
///
/// Attach it beside [`InputContextState`](crate::context::InputContextState) to restrict which
/// devices that context reads. Being a plain component rather than part of the context's own state
/// means something outside any declared context type — a device-selection screen, say — can still
/// ask "is this device claimed by anything" without knowing every context a game has declared.
///
/// A context entity with no `Paired` reads every device, which is exactly today's single-player
/// behavior: nothing has to opt in for a game that never mentions this component to keep working
/// unchanged.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct Paired(DeviceHandleSet);

impl Paired {
    /// A pairing claiming exactly one device.
    pub fn to(device: DeviceHandle) -> Self {
        Self(DeviceHandleSet::of(device))
    }

    /// Adds another device to this pairing.
    ///
    /// For an occupant that owns more than one device at once — a keyboard-and-mouse player who
    /// also gets a pad, say — rather than the common one-device case `to` alone covers.
    pub fn with(mut self, device: DeviceHandle) -> Self {
        self.0.insert(device);
        self
    }
}

impl Deref for Paired {
    type Target = DeviceHandleSet;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::Scheme;

    #[test]
    fn paired_reads_through_to_the_inner_set() {
        let paired = Paired::to(DeviceHandle::KeyboardMouse);
        assert!(paired.contains(DeviceHandle::KeyboardMouse));
        assert_eq!(
            paired.owner_for(Scheme::KeyboardMouse),
            Some(DeviceHandle::KeyboardMouse)
        );
    }
}
