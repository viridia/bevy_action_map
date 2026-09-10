//! Bindings, composites, modifiers, and conditions.
//!
//! A binding says what drives an action: a control the hardware reports directly, or a composite
//! that assembles several controls into a value no single one of them carries. Modifiers reshape
//! what a binding produces on its way to the action.

mod builder;
mod control;
mod modifier;

pub use builder::{BindingBuilder, ClassBindingBuilder, InputContextBuilder};
pub use control::{
    BindingInput, BindingPart, ButtonThreshold, Control, IntoBindingInput, MouseMove,
};
pub use modifier::{BindingModifier, CompassPoints, DeadZone, DeadZoneShape, Modifier};

#[cfg(feature = "keyboard")]
pub use control::LogicalKey;
#[cfg(feature = "gamepad")]
pub use control::Stick;
#[cfg(feature = "keyboard")]
pub(crate) use control::normalize_character;
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
pub use control::{AxisButtons, ButtonControl, DirectionalButtons};

pub(crate) use builder::{BindingSpec, ClassBindingSpec, DelegatedSpec};
pub(crate) use modifier::{toggle_active, toggle_latch};

// Both read a control, so neither exists in a build with no device to read one from.
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
pub(crate) use control::as_button_control;
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
pub(crate) use modifier::resolve_shared_toggle;
