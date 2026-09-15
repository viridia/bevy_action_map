//! Players, device pairing, and control families.
//!
//! This module maps devices to the players that own them, so one player's input never reaches
//! another.

use bevy_ecs::prelude::Component;
use core::ops::Deref;

use crate::device::{DeviceHandle, DeviceHandleSet};

#[cfg(feature = "gamepad")]
use bevy_ecs::entity::Entity;
#[cfg(feature = "gamepad")]
use bevy_ecs::prelude::{Commands, EntityEvent, Event, Query, Res};

/// The devices one occupant's contexts should read, and no others.
///
/// Attach it beside [`InputContextState`](crate::context::InputContextState) to restrict which
/// devices that context reads. Being a plain component rather than part of the context's own state
/// means something outside any declared context type — a device-selection screen, say — can still
/// ask "is this device claimed by anything" without knowing every context a game has declared.
///
/// A context entity with no `Paired` reads every device, so a single-player game needs no opt-in.
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

/// A paired device stopped answering — a pad disconnecting mid-game, not a player leaving on
/// purpose.
///
/// Triggered on the entity whose `Paired` named the device. `Paired` itself is untouched: keeping
/// the slot open for a reconnect, or tearing the pairing down, is the app's call.
#[cfg(feature = "gamepad")]
#[derive(EntityEvent, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceDisconnected {
    /// The entity carrying the `Paired` this device belonged to.
    pub entity: Entity,
    /// The device that went away.
    pub device: DeviceHandle,
}

/// A gamepad became available.
///
/// Unscoped: the crate cannot tell a fresh join from an existing pairing's own device coming back,
/// so it does not guess. Matching this to a waiting occupant, if any, is the app's call.
#[cfg(feature = "gamepad")]
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceConnected {
    /// The device that connected.
    pub device: DeviceHandle,
}

/// Turns `RawGamepadEvent::Connection` into [`DeviceDisconnected`] and [`DeviceConnected`].
///
/// Runs once, independent of any context type: `evaluate_context` runs once per context *type*, so
/// raising a signal from inside it would fire once per context an occupant happens to carry rather
/// than once per device.
#[cfg(feature = "gamepad")]
pub(crate) fn watch_gamepad_connections(
    frame: Res<'_, crate::frame::InputFrame>,
    paired: Query<'_, '_, (Entity, &Paired)>,
    mut commands: Commands<'_, '_>,
) {
    use bevy_input::gamepad::{GamepadConnection, RawGamepadEvent};

    for timed in frame.events_after(None) {
        let crate::frame::RawEvent::Gamepad(RawGamepadEvent::Connection(connection)) = &timed.event
        else {
            continue;
        };
        let device = timed.event.device();
        match connection.connection {
            GamepadConnection::Disconnected => {
                for (entity, pairing) in &paired {
                    if pairing.contains(device) {
                        commands.trigger(DeviceDisconnected { entity, device });
                    }
                }
            }
            GamepadConnection::Connected { .. } => {
                commands.trigger(DeviceConnected { device });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::DeviceFamily;

    #[test]
    fn paired_reads_through_to_the_inner_set() {
        let paired = Paired::to(DeviceHandle::KeyboardMouse);
        assert!(paired.contains(DeviceHandle::KeyboardMouse));
        assert_eq!(
            paired.owner_for(DeviceFamily::KeyboardMouse),
            Some(DeviceHandle::KeyboardMouse)
        );
    }

    #[cfg(feature = "gamepad")]
    mod connections {
        use alloc::vec::Vec;

        use bevy_app::{App, Update};
        use bevy_ecs::prelude::{On, Resource};
        use bevy_input::gamepad::{GamepadConnection, GamepadConnectionEvent};

        use super::*;
        use crate::frame::RawEvent;

        #[derive(Resource, Default)]
        struct Disconnects(Vec<(Entity, DeviceHandle)>);

        #[derive(Resource, Default)]
        struct Connects(Vec<DeviceHandle>);

        fn app() -> App {
            let mut app = App::new();
            app.init_resource::<crate::frame::InputFrame>();
            app.init_resource::<Disconnects>();
            app.init_resource::<Connects>();
            app.add_systems(Update, watch_gamepad_connections);
            app.add_observer(
                |trigger: On<DeviceDisconnected>,
                 mut log: bevy_ecs::prelude::ResMut<Disconnects>| {
                    log.0.push((trigger.entity, trigger.device));
                },
            );
            app.add_observer(
                |trigger: On<DeviceConnected>, mut log: bevy_ecs::prelude::ResMut<Connects>| {
                    log.0.push(trigger.device);
                },
            );
            app
        }

        /// The owning pairing is told, and a pairing whose own device is unrelated is not — the
        /// first half of R15.5, with a signal attached to it.
        #[test]
        fn disconnect_signals_only_the_paired_entity() {
            let mut app = app();
            let lost = Entity::from_bits(1);
            let survives = Entity::from_bits(2);
            let waiting = app
                .world_mut()
                .spawn(Paired::to(DeviceHandle::Gamepad(lost)))
                .id();
            app.world_mut()
                .spawn(Paired::to(DeviceHandle::Gamepad(survives)));

            app.world_mut()
                .resource_mut::<crate::frame::InputFrame>()
                .record(RawEvent::Gamepad(
                    bevy_input::gamepad::RawGamepadEvent::Connection(GamepadConnectionEvent::new(
                        lost,
                        GamepadConnection::Disconnected,
                    )),
                ));
            app.update();

            assert_eq!(
                app.world().resource::<Disconnects>().0,
                [(waiting, DeviceHandle::Gamepad(lost))]
            );
        }

        /// An app matching a returning pad to the occupant that lost it reads [`Identity`] off the
        /// device the signal names, so the identity has to be attached by the time the signal
        /// arrives. It is — `sample_input` runs `.after(InputSystems)`, and the sync point that
        /// order implies is where Bevy's own deferred `Gamepad` insert lands and where the observer
        /// watching for it gets to run. Ordering nobody declared, so it is asserted rather than
        /// assumed.
        #[cfg(feature = "bevy_reflect")]
        #[test]
        fn a_connect_signal_arrives_after_the_identity_it_will_be_matched_on() {
            use bevy_ecs::prelude::ResMut;

            #[derive(Resource, Default)]
            struct SawIdentity(Option<bool>);

            let mut app = App::new();
            app.add_plugins(bevy_input::InputPlugin);
            app.add_plugins(crate::ActionMapPlugin);
            app.init_resource::<SawIdentity>();
            app.add_observer(
                |connected: On<DeviceConnected>,
                 identities: Query<&crate::device::Identity>,
                 mut saw: ResMut<SawIdentity>| {
                    let DeviceHandle::Gamepad(device) = connected.device else {
                        return;
                    };
                    saw.0 = Some(identities.get(device).is_ok());
                },
            );

            let pad = app.world_mut().spawn_empty().id();
            let connected = GamepadConnectionEvent::new(
                pad,
                GamepadConnection::Connected {
                    name: "test pad".into(),
                    vendor_id: Some(0x054C),
                    product_id: Some(0x05C4),
                },
            );
            // Both messages, which is what a real backend writes: the raw one reaches the input
            // frame, the connection one drives Bevy's own entity setup.
            app.world_mut()
                .write_message(bevy_input::gamepad::RawGamepadEvent::from(
                    connected.clone(),
                ));
            app.world_mut().write_message(connected);
            app.update();

            assert_eq!(
                app.world().resource::<SawIdentity>().0,
                Some(true),
                "an app cannot match a returning pad if the signal outruns its identity"
            );
        }

        /// Unscoped: fires whether or not anything is paired at all, since matching a newly
        /// connected pad to a waiting occupant is the app's call (D53), not this system's.
        #[test]
        fn connect_signals_regardless_of_pairing() {
            let mut app = app();
            let arrived = Entity::from_bits(3);

            app.world_mut()
                .resource_mut::<crate::frame::InputFrame>()
                .record(RawEvent::Gamepad(
                    bevy_input::gamepad::RawGamepadEvent::Connection(GamepadConnectionEvent::new(
                        arrived,
                        GamepadConnection::Connected {
                            name: "test pad".into(),
                            vendor_id: None,
                            product_id: None,
                        },
                    )),
                ));
            app.update();

            assert_eq!(
                app.world().resource::<Connects>().0,
                [DeviceHandle::Gamepad(arrived)]
            );
        }
    }
}
