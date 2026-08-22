//! Action types carry the value shape and meaning for one gameplay action.
//!
//! Declare an action as a Rust type, choose the value shape it returns, and give it an intent so
//! bindings can find controls that produce a compatible value.
//!
//! Give the action a stable, user-chosen path with `#[action(path = "...")]`. That string is
//! what settings files and other serialized data should store.
//!
//! ```rust
//! use bevy_action_map::prelude::*;
//!
//! #[derive(bevy_action_map::InputAction)]
//! #[action(path = "gameplay.jump", output = bool, intent = Button)]
//! struct Jump;
//!
//! assert_eq!(Jump::INTENT, Intent::Button);
//! ```

use alloc::vec::Vec;
use bevy_math::{Vec2, Vec3};
use core::fmt;

#[cfg(feature = "bevy_reflect")]
use bevy_reflect::Reflect;

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

/// Stable handle for an action type.
///
/// Use this when you need to store or compare actions without carrying the full type.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActionId(u32);

impl ActionId {
    /// Returns the dense numeric identifier.
    pub const fn index(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ActionId").field(&self.0).finish()
    }
}

/// What kind of control an action can consume.
///
/// Use this to describe the controls that are a good fit for the action, so binding UIs can filter
/// to sources that produce the right kind of input.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intent {
    /// A digital press or release.
    Button,
    /// A 1D analog control, such as a trigger.
    Analog1,
    /// A 2D directional control, such as a stick.
    Directional2,
    /// A 2D delta control, such as mouse motion.
    Delta2,
}

impl Intent {
    /// Returns whether this intent is a match for the given output shape.
    pub fn supports_output<O: ActionOutput>(self) -> bool {
        O::INTENTS.contains(&self)
    }
}

/// Runtime value returned by an action.
///
/// This is the shape you read from an action at runtime: button, 1D, 2D, or 3D.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActionValue {
    /// A button-like value.
    Bool(bool),
    /// A 1D analog value.
    Axis1(f32),
    /// A 2D value.
    Axis2(Vec2),
    /// A 3D value.
    Axis3(Vec3),
}

/// The tick domain a context runs in.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TickDomain {
    /// Run once per rendered frame.
    Render,
    /// Run once per fixed simulation tick.
    Fixed,
}

impl ActionValue {
    /// Converts this value into a typed output when the shape matches.
    ///
    /// Use this when you already have a runtime action value and want a typed result.
    pub fn into_output<O: ActionOutput>(self) -> Option<O> {
        O::from_action_value(self)
    }

    /// Wraps a typed output as a runtime action value.
    pub fn from_output<O: ActionOutput>(output: O) -> Self {
        output.into_action_value()
    }
}

impl From<bool> for ActionValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<f32> for ActionValue {
    fn from(value: f32) -> Self {
        Self::Axis1(value)
    }
}

impl From<Vec2> for ActionValue {
    fn from(value: Vec2) -> Self {
        Self::Axis2(value)
    }
}

impl From<Vec3> for ActionValue {
    fn from(value: Vec3) -> Self {
        Self::Axis3(value)
    }
}

/// A value shape an action can produce.
///
/// This is the production side of the match: it says what the action yields, and which intents are
/// sensible consumers for that shape.
pub trait ActionOutput: Copy + Send + Sync + 'static {
    /// Intents that can consume this output shape.
    const INTENTS: &'static [Intent];

    /// Converts the typed value into the runtime representation.
    fn into_action_value(self) -> ActionValue;

    /// Converts the runtime representation back into the typed value.
    fn from_action_value(value: ActionValue) -> Option<Self>;
}

/// A declared gameplay context.
///
/// Derive this on a unit struct and choose the tick domain where it should be evaluated.
///
/// Give the context a stable, user-chosen path with `#[context(path = "...")]`.
///
/// ```rust
/// use bevy_action_map::prelude::*;
///
/// #[derive(bevy_action_map::InputContext)]
/// #[context(path = "gameplay.on_foot", tick = Fixed)]
/// struct OnFoot;
///
/// assert_eq!(OnFoot::TICK, TickDomain::Fixed);
/// ```
pub trait InputContext: Send + Sync + 'static {
    /// The tick domain this context runs in.
    const TICK: TickDomain;

    /// The evaluation priority of this context.
    const PRIORITY: i32;

    /// Stable path used to identify the context across runs.
    const PATH: &'static str;
}

impl ActionOutput for bool {
    const INTENTS: &'static [Intent] = &[Intent::Button];

    fn into_action_value(self) -> ActionValue {
        ActionValue::Bool(self)
    }

    fn from_action_value(value: ActionValue) -> Option<Self> {
        Some(match value {
            ActionValue::Bool(value) => value,
            ActionValue::Axis1(value) => value != 0.0,
            ActionValue::Axis2(value) => value.length_squared() != 0.0,
            ActionValue::Axis3(value) => value.length_squared() != 0.0,
        })
    }
}

impl ActionOutput for f32 {
    const INTENTS: &'static [Intent] = &[Intent::Button, Intent::Analog1];

    fn into_action_value(self) -> ActionValue {
        ActionValue::Axis1(self)
    }

    fn from_action_value(value: ActionValue) -> Option<Self> {
        Some(match value {
            ActionValue::Bool(value) => {
                if value {
                    1.0
                } else {
                    0.0
                }
            }
            ActionValue::Axis1(value) => value,
            ActionValue::Axis2(value) => value.length(),
            ActionValue::Axis3(value) => value.length(),
        })
    }
}

impl ActionOutput for Vec2 {
    const INTENTS: &'static [Intent] = &[Intent::Directional2, Intent::Delta2];

    fn into_action_value(self) -> ActionValue {
        ActionValue::Axis2(self)
    }

    fn from_action_value(value: ActionValue) -> Option<Self> {
        Some(match value {
            ActionValue::Bool(value) => Vec2::splat(if value { 1.0 } else { 0.0 }),
            ActionValue::Axis1(value) => Vec2::splat(value),
            ActionValue::Axis2(value) => value,
            ActionValue::Axis3(value) => Vec2::new(value.x, value.y),
        })
    }
}

impl ActionOutput for Vec3 {
    const INTENTS: &'static [Intent] = &[
        Intent::Button,
        Intent::Analog1,
        Intent::Directional2,
        Intent::Delta2,
    ];

    fn into_action_value(self) -> ActionValue {
        ActionValue::Axis3(self)
    }

    fn from_action_value(value: ActionValue) -> Option<Self> {
        Some(match value {
            ActionValue::Bool(value) => Vec3::splat(if value { 1.0 } else { 0.0 }),
            ActionValue::Axis1(value) => Vec3::splat(value),
            ActionValue::Axis2(value) => Vec3::new(value.x, value.y, 0.0),
            ActionValue::Axis3(value) => value,
        })
    }
}

/// A declared gameplay action.
///
/// Derive this on a unit struct, give it an output shape and intent, and use the generated type
/// as the thing you bind and read from your systems.
pub trait InputAction: Send + Sync + 'static {
    /// The value shape returned when this action is read.
    type Output: ActionOutput;

    /// The kind of control that should drive this action.
    const INTENT: Intent;

    /// Stable path used to identify the action across runs.
    const PATH: &'static str;

    /// Returns the registered id for this action type.
    fn id() -> ActionId {
        intern_action(Self::PATH)
    }
}

#[derive(Default)]
struct ActionRegistry {
    next_id: u32,
    entries: Vec<(&'static str, ActionId)>,
}

static ACTION_REGISTRY: bevy_platform::sync::OnceLock<bevy_platform::sync::Mutex<ActionRegistry>> =
    bevy_platform::sync::OnceLock::new();

fn intern_action(path: &'static str) -> ActionId {
    let mut registry = ACTION_REGISTRY
        .get_or_init(|| bevy_platform::sync::Mutex::new(ActionRegistry::default()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());

    if let Some((_, id)) = registry
        .entries
        .iter()
        .find(|(registered_path, _)| *registered_path == path)
    {
        return *id;
    }

    let index = registry.next_id;
    registry.next_id = registry
        .next_id
        .checked_add(1)
        .expect("action registry exhausted u32 ids");
    let id = ActionId(index);
    registry.entries.push((path, id));
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct Jump;

    impl InputAction for Jump {
        type Output = bool;

        const INTENT: Intent = Intent::Button;
        const PATH: &'static str = "tests::Jump";
    }

    #[derive(Clone, Copy)]
    struct Look;

    impl InputAction for Look {
        type Output = Vec2;

        const INTENT: Intent = Intent::Delta2;
        const PATH: &'static str = "tests::Look";
    }

    #[test]
    fn action_ids_are_interned_by_path() {
        assert_eq!(Jump::id(), Jump::id());
        assert_ne!(Jump::id(), Look::id());
        assert_eq!(Jump::id().index(), Jump::id().index());
    }

    #[test]
    fn intent_matrix_matches_output_shapes() {
        assert!(Intent::Button.supports_output::<bool>());
        assert!(Intent::Button.supports_output::<f32>());
        assert!(!Intent::Button.supports_output::<Vec2>());
        assert!(Intent::Analog1.supports_output::<f32>());
        assert!(Intent::Directional2.supports_output::<Vec2>());
        assert!(Intent::Delta2.supports_output::<Vec2>());
    }

    #[test]
    fn action_value_conversions_are_explicit() {
        assert_eq!(ActionValue::Bool(true).into_output::<f32>(), Some(1.0));
        assert_eq!(ActionValue::Axis1(0.5).into_output::<bool>(), Some(true));
        assert_eq!(
            ActionValue::Axis2(Vec2::new(3.0, 4.0)).into_output::<f32>(),
            Some(5.0)
        );
        assert_eq!(
            ActionValue::Axis3(Vec3::new(1.0, 2.0, 3.0)).into_output::<Vec2>(),
            Some(Vec2::new(1.0, 2.0))
        );
        assert_eq!(
            ActionValue::Axis1(0.25).into_output::<Vec3>(),
            Some(Vec3::splat(0.25))
        );
    }
}
