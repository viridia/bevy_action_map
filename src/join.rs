//! Join gesture: telling which device just asked to play.
//!
//! Nothing here is a new mechanism. A game declares "join" as an ordinary action on a context it
//! spawns **once per available device**, each [`Paired`] to its own. A press then arrives on an
//! entity that already names who pressed it — an ordinary action's value is device-agnostic, so
//! pairing the listener is what answers the question, rather than asking the action.
//!
//! ```ignore
//! #[derive(InputAction)]
//! #[action(path = "menu.join", output = bool, intent = Button)]
//! struct Join;
//!
//! #[derive(InputContext)]
//! #[context(path = "menu.join_listener", tick = Render)]
//! struct JoinListener;
//!
//! // Both bindings on every instance: pairing does the sorting, so the keyboard's listener never
//! // sees a pad event and each pad's listener never sees a key.
//! app.add_context::<JoinListener>(|controls| {
//!     controls.bind::<Join>(GamepadButton::South);
//!     controls.bind::<Join>(KeyCode::Enter);
//! });
//!
//! // The keyboard is always here; a pad comes and goes with `ConnectedGamepad`.
//! app.add_systems(Startup, |mut commands: Commands| {
//!     commands.spawn((JoinListener, Paired::to(DeviceHandle::KeyboardMouse)));
//! });
//! app.add_observer(|pad: On<Add<ConnectedGamepad>>, mut commands: Commands| {
//!     commands.spawn((JoinListener, Paired::to(DeviceHandle::Gamepad(pad.entity))));
//! });
//!
//! app.add_observer(
//!     |fired: On<Fired<Join>>,
//!      listeners: Query<&Paired, With<JoinListener>>,
//!      mut slots: ResMut<MySlotTable>| {
//!         let Ok(pairing) = listeners.get(fired.entity) else { return };
//!         let Some(device) = pairing.iter().next() else { return };
//!         // claim a slot for `device` — the game's own call, and see the warning below
//!     },
//! );
//! ```
//!
//! # Claiming a slot
//!
//! Decide whether a device is already taken from state your own code updates **synchronously**, not
//! from a `Query<&Paired>`. Two devices pressing join on the same tick both fire before either
//! `Paired` insert — a deferred command — has been applied, so a query sees both as unclaimed and
//! hands them the same slot. A table you write inside the observer itself is already correct by the
//! time the second press arrives. [`is_claimed`] answers the question it is named for and does not
//! rescue you from this one, because the data it reads has not caught up yet.
//!
//! # A simpler recipe, while input comes from hardware
//!
//! One listener per device is the recipe that always answers. A game can instead bind join once
//! with [`bind_class`](crate::binding::InputContextBuilder::bind_class) to a class such as
//! [`ControlClass::AnyButton`](crate::capture::ControlClass::AnyButton) and read the device off
//! [`ClassFired`](crate::event::ClassFired)'s raw event: one context, one binding, and no spawning
//! per device. The race above applies to it unchanged.
//!
//! What it rests on is input arriving as hardware events. A backend that supplies action values
//! directly, rather than reporting the controls it read, leaves no raw event to take a device from,
//! so this recipe has no answer under one. Pair the listener instead if you may ever run that way.
//!
//! # The rest is yours
//!
//! Which slot a newly claimed device fills, how many slots there are, whether a listener survives
//! its device being claimed, and which control means "join" on each family are all ordinary
//! declaration and ordinary application logic.
//!
//! Requiring a particular kind of device for a slot works the same way: check
//! [`DeviceHandle::family`](crate::device::DeviceHandle::family) before claiming, and return
//! without claiming if it is not the one this slot wants. A co-op mode where every player is on a
//! gamepad is the usual reason. The crate never treats one family as a stand-in for another — a
//! mouse and a stick aim nothing alike — so which substitutions your game accepts is yours to
//! state.

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
