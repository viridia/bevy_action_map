//! Gamepads as a game asks about them: which are connected, and what kind each one is, and as a
//! game drives them: how hard each one rumbles.
//!
//! Each question is answered by a component on the gamepad's own entity, and the answer reads the
//! same whichever backend supplied the pad. Bevy's own gamepad backend gets these attached for you,
//! from what it reports when a pad connects. A backend of your own inserts them on the entities it
//! spawns, and code that queries them never needs to know which backend it is running on.
//!
//! [`Rumble`] runs the other way: the game sets it on a pad's entity, and the backend reads it.

use bevy_app::{App, PostUpdate};
use bevy_ecs::prelude::Component;
use bevy_input::gamepad::{GamepadRumbleIntensity, GamepadRumbleRequest};
use core::ops::Deref;

mod gilrs;

use gilrs::drive_gamepad_rumble;
pub use gilrs::{
    GamepadBrands, GamepadModelId, mark_gamepad_connected, mark_gamepad_disconnected,
    resolve_gamepad_brand,
};

/// A gamepad that is connected right now.
///
/// Query it to enumerate the pads available, and observe `Add` and `Remove` on it to learn when
/// one arrives or goes away. It carries nothing, because the entity it sits on is already the
/// answer: that entity is the pad, and its other components describe it.
///
/// ```ignore
/// fn pads(pads: Query<(Entity, &Brand), With<ConnectedGamepad>>) {
///     for (pad, brand) in &pads {
///         info!("{pad} is a {brand} pad");
///     }
/// }
/// ```
///
/// Bevy's own `Gamepad` component is a different thing, and not a substitute for this one. It holds
/// a pad's live button and axis readings, which only a backend feeding raw device messages can fill
/// in; a platform input service that reports finished actions has no such readings to offer, so a
/// pad it supplies would carry an empty one and read as though nobody were touching it. Enumerating
/// through this component instead is what lets one game work on either kind of backend.
///
/// Bevy's gamepad backend gets this attached for you. A backend of your own inserts it on the
/// entities it spawns, and removes it when a pad goes away.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConnectedGamepad;

/// Which manufacturer's conventions a connected gamepad follows, for prompts and glyphs that want
/// to say "A" on an Xbox pad and "Cross" on a PlayStation one rather than "South Button" on both.
///
/// `Generic` is the ordinary answer for a pad the backend could not identify, not an error.
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GamepadBrand {
    /// An Xbox controller.
    Xbox,
    /// A PlayStation controller.
    PlayStation,
    /// A Nintendo controller — a Switch Pro Controller or Joy-Con.
    Nintendo,
    /// Every other pad, and one the backend could not identify.
    Generic,
}

impl core::fmt::Display for GamepadBrand {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Xbox => "Xbox",
            Self::PlayStation => "PlayStation",
            Self::Nintendo => "Nintendo",
            Self::Generic => "Generic",
        })
    }
}

/// A connected gamepad's [`GamepadBrand`], resolved once and attached to its entity.
///
/// Query this instead of working a brand out from what the backend reported, so that a pad from
/// any backend answers the same way. Bevy's gamepad backend resolves it from the pad's vendor id,
/// through [`GamepadBrands`].
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Brand(pub GamepadBrand);

impl Deref for Brand {
    type Target = GamepadBrand;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// How hard a gamepad's motors rumble, for as long as this is set.
///
/// Insert it on a pad's entity to start the pad rumbling, change it to change the rumble, and
/// remove it or set both motors to zero to stop. It is a level rather than an effect: the pad holds
/// the value until you change it, with no duration to choose and no timer to keep.
///
/// ```ignore
/// fn engine_rumble(ships: Query<(&Ship, &Pilot)>, mut commands: Commands) {
///     for (ship, pilot) in &ships {
///         let intensity = GamepadRumbleIntensity::strong_motor(ship.throttle * 0.4);
///         commands.entity(pilot.pad).insert(Rumble(intensity));
///     }
/// }
/// ```
///
/// There is one value per pad, and the last one written wins. A game that rumbles for more than one
/// reason, such as gameplay and a menu at once, keeps each reason in a component of its own and
/// combines them in one system that writes this.
///
/// Bevy's gamepad backend is driven from it for you, except on macOS, where that backend cannot
/// rumble a pad at all. A backend of your own reads it from the entities it spawns and drives its
/// pads the same way. For a short one-off buzz, sending Bevy's `GamepadRumbleRequest` yourself
/// remains the simpler tool, but a change to this component stops any such buzz running on the same
/// pad.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Rumble(pub GamepadRumbleIntensity);

impl Rumble {
    /// Whether both motors are at rest.
    pub fn is_still(&self) -> bool {
        self.0.strong_motor <= 0.0 && self.0.weak_motor <= 0.0
    }
}

impl Deref for Rumble {
    type Target = GamepadRumbleIntensity;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Fills this module's components from Bevy's own gamepad backend, and drives its rumble from
/// [`Rumble`].
pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<GamepadBrands>();
    app.add_observer(resolve_gamepad_brand);
    app.add_observer(mark_gamepad_connected);
    app.add_observer(mark_gamepad_disconnected);
    app.add_message::<GamepadRumbleRequest>();
    app.add_systems(PostUpdate, drive_gamepad_rumble);
}
