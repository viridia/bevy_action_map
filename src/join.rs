//! Join gesture: telling an unassigned device's press from everyone else's.
//!
//! Nothing here is a new mechanism. A game declares "join" as an ordinary action on an ordinary
//! context, using [`bind_class`](crate::binding::InputContextBuilder::bind_class) to bind it to
//! [`ControlClass::AnyButton`](crate::capture::ControlClass::AnyButton) (or to whichever class
//! fits) rather than to one named control. Left with no [`Paired`] of its own, that context reads
//! every device, the same way any other unpaired context does.
//!
//! [`ClassFired`](crate::event::ClassFired)'s event is the untouched raw event, not an action's
//! collapsed value, so `event.device()` still says which device pressed it.
//!
//! ```ignore
//! struct Join;
//! impl ClassBinding for Join {
//!     const PATH: &'static str = "menu.join";
//! }
//!
//! controls.bind_class::<Join>(ControlClass::AnyButton);
//!
//! app.add_observer(
//!     |fired: On<ClassFired<Join>>, paired: Query<&Paired>, mut commands: Commands| {
//!         let device = fired.event.device();
//!         if bevy_action_map::join::is_claimed(&paired, device) {
//!             return; // some other player already owns this device
//!         }
//!         // pick a slot and insert `Paired::to(device)` on it — the game's own call
//!     },
//! );
//! ```
//!
//! Which slot a newly claimed device fills, how many slots there are, and whether a game wants
//! "any button" or one particular control per scheme all stay ordinary binding declaration and
//! ordinary application logic — not something this crate decides for you.

use crate::device::DeviceHandle;
use crate::player::Paired;

/// Whether some [`Paired`] already claims this device.
pub fn is_claimed<'a>(paired: impl IntoIterator<Item = &'a Paired>, device: DeviceHandle) -> bool {
    paired.into_iter().any(|paired| paired.contains(device))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_claims_an_empty_world() {
        assert!(!is_claimed(&[], DeviceHandle::KeyboardMouse));
    }

    #[test]
    fn a_pairing_elsewhere_claims_its_device() {
        let pairings = [Paired::to(DeviceHandle::KeyboardMouse)];
        assert!(is_claimed(&pairings, DeviceHandle::KeyboardMouse));
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn a_pairing_does_not_claim_a_different_device() {
        let pad = bevy_ecs::entity::Entity::from_bits(1);
        let pairings = [Paired::to(DeviceHandle::KeyboardMouse)];
        assert!(!is_claimed(&pairings, DeviceHandle::Gamepad(pad)));
    }
}
