//! Gamepads as a game asks about them: which are connected, and what kind each one is.
//!
//! Each question is answered by a component on the gamepad's own entity, and the answer reads the
//! same whichever backend supplied the pad. Bevy's own gamepad backend gets these attached for you,
//! from what it reports when a pad connects. A backend of your own inserts them on the entities it
//! spawns, and code that queries them never needs to know which backend it is running on.

use bevy_app::App;
use bevy_ecs::prelude::Component;
use core::ops::Deref;

mod gilrs;

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

/// Fills this module's components from Bevy's own gamepad backend.
pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<GamepadBrands>();
    app.add_observer(resolve_gamepad_brand);
    app.add_observer(mark_gamepad_connected);
    app.add_observer(mark_gamepad_disconnected);
}
