//! Filling the backend-neutral components from Bevy's own `Gamepad`, which `bevy_gilrs` supplies.

use bevy_ecs::lifecycle::{Add, Remove};
use bevy_ecs::prelude::{Commands, On, Query, Res, Resource, Without};
#[cfg(feature = "bevy_reflect")]
use bevy_ecs::reflect::ReflectResource;
use bevy_input::gamepad::Gamepad;
use bevy_platform::collections::HashMap;

use super::{Brand, ConnectedGamepad, GamepadBrand};

/// Resolves a connected gamepad's [`GamepadBrand`] from its `vendor_id`.
///
/// Seeded with the three current-generation console makers' USB vendor ids, not
/// SDL_GameControllerDB's full device list. [`insert`](Self::insert) extends the table for
/// hardware this crate does not ship pre-resolved.
///
/// `vendor_id` is often absent, on wasm and some Linux setups, and such a pad resolves as
/// `Generic`.
#[cfg_attr(
    feature = "bevy_reflect",
    derive(bevy_reflect::Reflect),
    reflect(Resource)
)]
#[derive(Resource, Debug)]
pub struct GamepadBrands {
    by_vendor: HashMap<u16, GamepadBrand>,
}

impl Default for GamepadBrands {
    fn default() -> Self {
        let mut by_vendor = HashMap::new();
        by_vendor.insert(0x045E, GamepadBrand::Xbox); // Microsoft
        by_vendor.insert(0x054C, GamepadBrand::PlayStation); // Sony
        by_vendor.insert(0x057E, GamepadBrand::Nintendo); // Nintendo
        Self { by_vendor }
    }
}

impl GamepadBrands {
    /// Adds or replaces which brand a vendor id resolves to.
    pub fn insert(&mut self, vendor_id: u16, brand: GamepadBrand) {
        self.by_vendor.insert(vendor_id, brand);
    }

    /// Resolves a brand from a gamepad's vendor id, `Generic` if it is unknown or absent.
    pub fn resolve(&self, vendor_id: Option<u16>) -> GamepadBrand {
        vendor_id
            .and_then(|id| self.by_vendor.get(&id).copied())
            .unwrap_or(GamepadBrand::Generic)
    }
}

/// Attaches [`Brand`] to a gamepad's entity as soon as it connects, resolved from its `vendor_id`
/// through [`GamepadBrands`].
///
/// Leaves an existing `Brand` alone, so inserting one yourself ahead of time overrides this for a
/// pad you know better than the vendor id table does.
pub fn resolve_gamepad_brand(
    connected: On<Add<Gamepad>>,
    mut commands: Commands,
    gamepads: Query<&Gamepad, Without<Brand>>,
    brands: Res<GamepadBrands>,
) {
    let entity = connected.entity;
    if let Ok(gamepad) = gamepads.get(entity) {
        commands
            .entity(entity)
            .insert(Brand(brands.resolve(gamepad.vendor_id())));
    }
}

/// What Bevy's own gamepad backend can say about which device a pad is: the USB vendor and product
/// ids it reported when it connected.
///
/// **This names a model, not a unit.** Two identical controllers on the same table report the same
/// vendor and product id and cannot be told apart by it, so treat a match as a candidate rather
/// than an answer.
///
/// Not every platform reports these. They are absent on wasm and on some Linux setups, and a pad
/// that reports neither has no identity of this kind at all.
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GamepadModelId {
    /// The USB vendor id.
    pub vendor: u16,
    /// The USB product id.
    pub product: u16,
}

impl GamepadModelId {
    /// The model id a connected pad reports, or `None` if it reports either half as absent.
    pub fn of(gamepad: &Gamepad) -> Option<Self> {
        Some(Self {
            vendor: gamepad.vendor_id()?,
            product: gamepad.product_id()?,
        })
    }
}

/// Attaches [`ConnectedGamepad`] to a pad Bevy's own gamepad backend connected, so it enumerates
/// alongside any other backend's.
pub fn mark_gamepad_connected(connected: On<Add<Gamepad>>, mut commands: Commands) {
    commands.entity(connected.entity).insert(ConnectedGamepad);
}

/// Removes [`ConnectedGamepad`] when Bevy's own gamepad backend loses a pad.
///
/// Watches the component rather than the entity because that backend keeps the entity alive across
/// a disconnect — it removes `Gamepad` and re-adds it on reconnect, so the entity outlives any
/// single connection and despawning is never the signal.
pub fn mark_gamepad_disconnected(disconnected: On<Remove<Gamepad>>, mut commands: Commands) {
    // `try_` because a game is free to despawn a pad's entity outright, which removes `Gamepad` on
    // the way out and would leave this addressing something already gone.
    commands
        .entity(disconnected.entity)
        .try_remove::<ConnectedGamepad>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_input::InputPlugin;
    use bevy_input::gamepad::{GamepadConnection, GamepadConnectionEvent};

    #[test]
    fn brand_resolves_from_the_seeded_vendor_ids() {
        let brands = GamepadBrands::default();
        assert_eq!(brands.resolve(Some(0x045E)), GamepadBrand::Xbox);
        assert_eq!(brands.resolve(Some(0x054C)), GamepadBrand::PlayStation);
        assert_eq!(brands.resolve(Some(0x057E)), GamepadBrand::Nintendo);
    }

    #[test]
    fn brand_is_generic_when_the_vendor_id_is_unknown_or_absent() {
        let brands = GamepadBrands::default();
        assert_eq!(brands.resolve(Some(0x1234)), GamepadBrand::Generic);
        assert_eq!(brands.resolve(None), GamepadBrand::Generic);
    }

    #[test]
    fn an_app_can_override_or_extend_the_seeded_table() {
        let mut brands = GamepadBrands::default();
        // A pad this crate does not ship pre-resolved.
        brands.insert(0x2DC8, GamepadBrand::PlayStation); // 8BitDo, playing PlayStation-style
        assert_eq!(brands.resolve(Some(0x2DC8)), GamepadBrand::PlayStation);

        // The seeded table is a default, not a fixture — an app can also correct it.
        brands.insert(0x045E, GamepadBrand::Generic);
        assert_eq!(brands.resolve(Some(0x045E)), GamepadBrand::Generic);
    }

    /// The pool follows a pad nobody has claimed, which is the case `DeviceDisconnected` cannot
    /// report: that is an entity event raised once per `Paired` holding the device, so an unclaimed
    /// pad going away signals nothing at all — and an unclaimed pad is exactly what a join screen
    /// is prompting for.
    ///
    /// Through the module's plugin rather than by adding the observers here, so the wiring is under
    /// test alongside the behaviour.
    #[test]
    fn the_marker_follows_a_pad_with_no_pairing_behind_it() {
        let mut app = App::new();
        app.add_plugins(InputPlugin);
        app.add_plugins(crate::gamepad::plugin);

        let pad = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(GamepadConnectionEvent::new(
            pad,
            GamepadConnection::Connected {
                name: "test pad".into(),
                vendor_id: None,
                product_id: None,
            },
        ));
        app.update();

        assert!(
            app.world().get::<ConnectedGamepad>(pad).is_some(),
            "a connected pad never entered the pool"
        );

        app.world_mut().write_message(GamepadConnectionEvent::new(
            pad,
            GamepadConnection::Disconnected,
        ));
        app.update();

        assert!(
            app.world().get::<ConnectedGamepad>(pad).is_none(),
            "a pad left without leaving the pool"
        );
        // The entity outlives the connection, which is why a pool reads the marker rather than the
        // entity.
        assert!(app.world().get_entity(pad).is_ok());
    }

    #[test]
    fn resolve_gamepad_brand_attaches_it_once_per_connection() {
        let mut app = App::new();
        app.add_plugins(InputPlugin);
        app.insert_resource(GamepadBrands::default());
        app.add_observer(resolve_gamepad_brand);

        let known = app.world_mut().spawn_empty().id();
        let unreported = app.world_mut().spawn_empty().id();
        // A pad this crate would otherwise call `Xbox` — a game that knows better inserts its own
        // `Brand` ahead of the connection event, and the observer must leave it standing.
        let overridden = app.world_mut().spawn(Brand(GamepadBrand::Nintendo)).id();

        for (gamepad, vendor_id) in [
            (known, Some(0x045E)),
            (unreported, None),
            (overridden, Some(0x045E)),
        ] {
            app.world_mut().write_message(GamepadConnectionEvent::new(
                gamepad,
                GamepadConnection::Connected {
                    name: "test pad".into(),
                    vendor_id,
                    product_id: None,
                },
            ));
        }
        app.update();

        assert_eq!(
            app.world().get::<Brand>(known),
            Some(&Brand(GamepadBrand::Xbox))
        );
        assert_eq!(
            app.world().get::<Brand>(unreported),
            Some(&Brand(GamepadBrand::Generic))
        );
        assert_eq!(
            app.world().get::<Brand>(overridden),
            Some(&Brand(GamepadBrand::Nintendo))
        );
    }
}
