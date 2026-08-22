//! Bindings, composites, modifiers, and conditions.
//!
//! The first interactive stage needs three source shapes: one keyboard key for a button action,
//! one mouse-motion source for look, and one four-key directional composite for movement.

use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

use bevy_input::keyboard::KeyCode;
use bevy_math::Vec2;

use crate::action::{ActionId, ActionValue, InputAction};

/// Named parts for a 2D directional composite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirectionalKeys {
    /// The key that contributes positive Y.
    pub up: KeyCode,
    /// The key that contributes negative Y.
    pub down: KeyCode,
    /// The key that contributes negative X.
    pub left: KeyCode,
    /// The key that contributes positive X.
    pub right: KeyCode,
}

impl DirectionalKeys {
    /// Creates a directional composite from the four movement keys.
    pub const fn new(up: KeyCode, down: KeyCode, left: KeyCode, right: KeyCode) -> Self {
        Self {
            up,
            down,
            left,
            right,
        }
    }
}

/// One authored binding in the first end-to-end slice.
pub(crate) struct BindingSpec {
    pub(crate) action: ActionId,
    pub(crate) source: BindingSource,
    pub(crate) modifiers: Vec<BindingModifier>,
}

/// The binding source used by the first interactive stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BindingSource {
    Button(KeyCode),
    Directional2(DirectionalKeys),
    MouseMotion,
}

/// A modifier that transforms one source value before it is written to an action.
pub trait Modifier: Send + Sync + 'static {
    /// Applies the modifier to a runtime value.
    fn apply(&self, value: ActionValue) -> ActionValue;
}

/// Built-in modifiers that can be chained onto a binding.
pub enum BindingModifier {
    /// Suppresses small values near zero and rescales the remainder.
    Deadzone(f32),
    /// Multiplies the value by a scalar.
    Scale(f32),
    /// Flips the sign or boolean sense of the value.
    Negate,
    /// Swaps the X and Y components of a 2D value.
    Swizzle,
    /// Clamps a numeric value to the given range.
    Clamp {
        /// The lower bound.
        min: f32,
        /// The upper bound.
        max: f32,
    },
    /// Raises the magnitude to a curve power while preserving sign.
    Curve(f32),
    /// Calls an application-defined modifier.
    Custom(Box<dyn Modifier>),
}

impl BindingModifier {
    /// Applies this modifier to a runtime value.
    pub fn apply(&self, value: ActionValue) -> ActionValue {
        match self {
            Self::Deadzone(radius) => apply_deadzone(value, *radius),
            Self::Scale(scale) => apply_scale(value, *scale),
            Self::Negate => apply_negate(value),
            Self::Swizzle => apply_swizzle(value),
            Self::Clamp { min, max } => apply_clamp(value, *min, *max),
            Self::Curve(power) => apply_curve(value, *power),
            Self::Custom(modifier) => modifier.apply(value),
        }
    }
}

/// A chainable handle for the binding that was just declared.
pub struct BindingHandle<'a, C> {
    builder: &'a mut ContextBuilder<C>,
    index: usize,
}

impl<'a, C> BindingHandle<'a, C> {
    fn push_modifier(&mut self, modifier: BindingModifier) {
        self.builder.bindings[self.index].modifiers.push(modifier);
    }

    /// Adds a deadzone modifier.
    pub fn deadzone(mut self, radius: f32) -> Self {
        self.push_modifier(BindingModifier::Deadzone(radius));
        self
    }

    /// Adds a scale modifier.
    pub fn scale(mut self, factor: f32) -> Self {
        self.push_modifier(BindingModifier::Scale(factor));
        self
    }

    /// Adds a negate modifier.
    pub fn negate(mut self) -> Self {
        self.push_modifier(BindingModifier::Negate);
        self
    }

    /// Adds an x/y swizzle modifier.
    pub fn swizzle(mut self) -> Self {
        self.push_modifier(BindingModifier::Swizzle);
        self
    }

    /// Adds a clamp modifier.
    pub fn clamp(mut self, min: f32, max: f32) -> Self {
        self.push_modifier(BindingModifier::Clamp { min, max });
        self
    }

    /// Adds a response-curve modifier.
    pub fn curve(mut self, power: f32) -> Self {
        self.push_modifier(BindingModifier::Curve(power));
        self
    }

    /// Adds a custom modifier.
    pub fn custom<M: Modifier>(mut self, modifier: M) -> Self {
        self.push_modifier(BindingModifier::Custom(Box::new(modifier)));
        self
    }
}

/// Builder used by [`crate::player::ActionMapAppExt::add_context`].
pub struct ContextBuilder<C> {
    bindings: Vec<BindingSpec>,
    _marker: PhantomData<C>,
}

impl<C> Default for ContextBuilder<C> {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
            _marker: PhantomData,
        }
    }
}

impl<C> ContextBuilder<C> {
    fn push_binding(&mut self, action: ActionId, source: BindingSource) -> BindingHandle<'_, C> {
        self.bindings.push(BindingSpec {
            action,
            source,
            modifiers: Vec::new(),
        });
        let index = self.bindings.len() - 1;
        BindingHandle {
            builder: self,
            index,
        }
    }

    /// Binds a boolean action to one keyboard key.
    pub fn bind<A>(&mut self, key_code: KeyCode) -> BindingHandle<'_, C>
    where
        A: InputAction<Output = bool>,
    {
        self.push_binding(A::id(), BindingSource::Button(key_code))
    }

    /// Binds a 2D action to mouse motion.
    pub fn bind_mouse_motion<A>(&mut self) -> BindingHandle<'_, C>
    where
        A: InputAction<Output = Vec2>,
    {
        self.push_binding(A::id(), BindingSource::MouseMotion)
    }

    /// Binds a 2D action to four named directional keys.
    pub fn bind_directional<A>(&mut self, keys: DirectionalKeys) -> BindingHandle<'_, C>
    where
        A: InputAction<Output = Vec2>,
    {
        self.push_binding(A::id(), BindingSource::Directional2(keys))
    }

    pub(crate) fn finish(self) -> Vec<BindingSpec> {
        self.bindings
    }
}

fn apply_deadzone(value: ActionValue, radius: f32) -> ActionValue {
    match value {
        ActionValue::Bool(value) => ActionValue::Bool(value),
        ActionValue::Axis1(value) => {
            let magnitude = value.abs();
            if magnitude <= radius {
                ActionValue::Axis1(0.0)
            } else {
                let scaled = (magnitude - radius) / (1.0 - radius);
                ActionValue::Axis1(value.signum() * scaled)
            }
        }
        ActionValue::Axis2(value) => {
            let magnitude = value.length();
            if magnitude <= radius {
                ActionValue::Axis2(Vec2::ZERO)
            } else {
                let scaled = (magnitude - radius) / (1.0 - radius);
                ActionValue::Axis2(value.normalize() * scaled)
            }
        }
        ActionValue::Axis3(value) => ActionValue::Axis3(value),
    }
}

fn apply_scale(value: ActionValue, factor: f32) -> ActionValue {
    match value {
        ActionValue::Bool(value) => ActionValue::Bool(value),
        ActionValue::Axis1(value) => ActionValue::Axis1(value * factor),
        ActionValue::Axis2(value) => ActionValue::Axis2(value * factor),
        ActionValue::Axis3(value) => ActionValue::Axis3(value * factor),
    }
}

fn apply_negate(value: ActionValue) -> ActionValue {
    match value {
        ActionValue::Bool(value) => ActionValue::Bool(!value),
        ActionValue::Axis1(value) => ActionValue::Axis1(-value),
        ActionValue::Axis2(value) => ActionValue::Axis2(-value),
        ActionValue::Axis3(value) => ActionValue::Axis3(-value),
    }
}

fn apply_swizzle(value: ActionValue) -> ActionValue {
    match value {
        ActionValue::Axis2(value) => ActionValue::Axis2(Vec2::new(value.y, value.x)),
        other => other,
    }
}

fn apply_clamp(value: ActionValue, min: f32, max: f32) -> ActionValue {
    match value {
        ActionValue::Bool(value) => ActionValue::Bool(value),
        ActionValue::Axis1(value) => ActionValue::Axis1(value.clamp(min, max)),
        ActionValue::Axis2(value) => {
            ActionValue::Axis2(value.clamp(Vec2::splat(min), Vec2::splat(max)))
        }
        ActionValue::Axis3(value) => ActionValue::Axis3(
            value.clamp(bevy_math::Vec3::splat(min), bevy_math::Vec3::splat(max)),
        ),
    }
}

fn apply_curve(value: ActionValue, power: f32) -> ActionValue {
    match value {
        ActionValue::Bool(value) => ActionValue::Bool(value),
        ActionValue::Axis1(value) => ActionValue::Axis1(value.signum() * value.abs().powf(power)),
        ActionValue::Axis2(value) => ActionValue::Axis2(value.signum() * value.abs().powf(power)),
        ActionValue::Axis3(value) => ActionValue::Axis3(value.signum() * value.abs().powf(power)),
    }
}

impl Modifier for BindingModifier {
    fn apply(&self, value: ActionValue) -> ActionValue {
        Self::apply(self, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionValue;

    #[derive(Clone, Copy)]
    struct DummyButton;

    impl InputAction for DummyButton {
        type Output = bool;

        const INTENT: crate::action::Intent = crate::action::Intent::Button;
        const PATH: &'static str = "tests::DummyButton";
    }

    struct DoubleAxis;

    impl Modifier for DoubleAxis {
        fn apply(&self, value: ActionValue) -> ActionValue {
            match value {
                ActionValue::Axis2(value) => ActionValue::Axis2(value * 2.0),
                other => other,
            }
        }
    }

    #[test]
    fn built_in_modifiers_are_pure_functions() {
        let cases = [
            (
                BindingModifier::Deadzone(0.25),
                ActionValue::Axis1(0.1),
                ActionValue::Axis1(0.0),
            ),
            (
                BindingModifier::Scale(2.0),
                ActionValue::Axis1(0.5),
                ActionValue::Axis1(1.0),
            ),
            (
                BindingModifier::Negate,
                ActionValue::Bool(true),
                ActionValue::Bool(false),
            ),
            (
                BindingModifier::Swizzle,
                ActionValue::Axis2(Vec2::new(1.0, 2.0)),
                ActionValue::Axis2(Vec2::new(2.0, 1.0)),
            ),
            (
                BindingModifier::Clamp {
                    min: -1.0,
                    max: 1.0,
                },
                ActionValue::Axis1(2.5),
                ActionValue::Axis1(1.0),
            ),
            (
                BindingModifier::Curve(2.0),
                ActionValue::Axis1(-0.5),
                ActionValue::Axis1(-0.25),
            ),
        ];

        for (modifier, input, expected) in cases {
            assert_eq!(modifier.apply(input), expected);
        }
    }

    #[test]
    fn custom_modifiers_fit_into_the_chain() {
        let modifier = BindingModifier::Custom(Box::new(DoubleAxis));

        assert_eq!(
            modifier.apply(ActionValue::Axis2(Vec2::new(1.0, -2.0))),
            ActionValue::Axis2(Vec2::new(2.0, -4.0))
        );
    }

    #[test]
    fn binding_builders_collect_modifiers_in_order() {
        let mut builder = ContextBuilder::<()>::default();
        builder
            .bind::<DummyButton>(KeyCode::Space)
            .scale(2.0)
            .negate()
            .deadzone(0.1);

        let bindings = builder.finish();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].modifiers.len(), 3);
        assert!(matches!(
            bindings[0].modifiers[0],
            BindingModifier::Scale(2.0)
        ));
        assert!(matches!(bindings[0].modifiers[1], BindingModifier::Negate));
        assert!(matches!(
            bindings[0].modifiers[2],
            BindingModifier::Deadzone(0.1)
        ));
    }
}
