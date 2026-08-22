//! Bindings, composites, modifiers, and conditions.
//!
//! The first interactive stage needs three source shapes: one keyboard key for a button action,
//! one mouse-motion source for look, and one four-key directional composite for movement.

use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

#[cfg(feature = "gamepad")]
use bevy_input::gamepad::GamepadButton;
#[cfg(feature = "keyboard")]
use bevy_input::keyboard::KeyCode;
use bevy_math::Vec2;

use crate::action::{ActionId, ActionValue, InputAction, Intent};

/// Named parts for a 2D directional composite.
#[cfg(feature = "keyboard")]
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

#[cfg(feature = "keyboard")]
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
    // Carried from the action type at bind time: the plan keys state by `ActionId`, which does not
    // reach back to the type, and folding several bindings into one action needs the intent. The
    // path is here so plan-build diagnostics can name the action a mistake is in.
    pub(crate) intent: Intent,
    pub(crate) path: &'static str,
    pub(crate) source: BindingSource,
    pub(crate) modifiers: Vec<BindingModifier>,
}

/// The binding source used by the first interactive stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingSource {
    /// A keyboard key.
    #[cfg(feature = "keyboard")]
    Button(KeyCode),
    /// A four-key directional composite.
    #[cfg(feature = "keyboard")]
    Directional2(DirectionalKeys),
    /// Mouse motion.
    MouseMotion,
    /// A gamepad button.
    #[cfg(feature = "gamepad")]
    GamepadButton(GamepadButton),
    /// A left or right gamepad stick.
    #[cfg(feature = "gamepad")]
    GamepadStick(Stick),
}

/// The left or right stick on a gamepad.
#[cfg(feature = "gamepad")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stick {
    /// The left stick.
    Left,
    /// The right stick.
    Right,
}

/// A source value that can be turned into a binding source for a particular action output.
pub trait BindingSourceSpec<Output> {
    /// Converts this source value into the internal binding representation.
    fn into_binding_source(self) -> BindingSource;
}

#[cfg(feature = "keyboard")]
impl BindingSourceSpec<bool> for KeyCode {
    fn into_binding_source(self) -> BindingSource {
        BindingSource::Button(self)
    }
}

#[cfg(feature = "keyboard")]
impl BindingSourceSpec<Vec2> for DirectionalKeys {
    fn into_binding_source(self) -> BindingSource {
        BindingSource::Directional2(self)
    }
}

#[cfg(feature = "gamepad")]
impl BindingSourceSpec<bool> for GamepadButton {
    fn into_binding_source(self) -> BindingSource {
        BindingSource::GamepadButton(self)
    }
}

#[cfg(feature = "gamepad")]
impl BindingSourceSpec<Vec2> for Stick {
    fn into_binding_source(self) -> BindingSource {
        BindingSource::GamepadStick(self)
    }
}

/// How a deadzone measures the region it removes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeadZoneShape {
    /// One circular region around centre, measured on the vector as a whole.
    ///
    /// This is what a stick wants. A stick pushed diagonally sits the same distance from centre as
    /// one pushed straight, so measuring the whole vector treats every direction alike.
    Radial,
    /// An independent band on each axis.
    ///
    /// Right where the axes mean unrelated things — a throttle and a rudder on one device — and
    /// wrong for a stick, where it produces the classic square-cornered response: the diagonals
    /// stay live at deflections where the cardinal directions have already gone dead.
    PerAxis,
}

/// The region around centre that reads as no input.
///
/// Every physical control rests slightly off centre, and a deadzone is what stops that from
/// reading as intent. Choose the [shape](DeadZoneShape) that matches the control, and decide
/// whether what remains is stretched back over the full range.
///
/// ```rust
/// use bevy_action_map::binding::DeadZone;
///
/// // The usual case: ignore the first 15% of a stick's travel, and let the rest still reach 1.0.
/// let stick = DeadZone::radial(0.15);
///
/// // A trimming pass that must not disturb what a later deadzone measures.
/// let trim = DeadZone::radial(0.05).without_rescale();
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeadZone {
    /// How the region is measured.
    pub shape: DeadZoneShape,
    /// How far the region extends from centre, as a fraction of full deflection.
    pub lower: f32,
    /// Whether what survives is stretched back over the full range.
    ///
    /// With rescaling on, a control just past the deadzone reads near zero and full deflection
    /// still reads 1.0, which is what makes a deadzone feel like nothing was taken away. It is
    /// almost always what you want, and it is the default.
    ///
    /// Turn it off when something later in the chain measures the same quantity. Stretching the
    /// range means a threshold further along no longer corresponds to any particular physical
    /// position, so at most one deadzone acting on a value may rescale.
    pub rescale: bool,
}

impl DeadZone {
    /// Removes a circular region around centre, rescaling what remains.
    pub const fn radial(lower: f32) -> Self {
        Self {
            shape: DeadZoneShape::Radial,
            lower,
            rescale: true,
        }
    }

    /// Removes an independent band on each axis, rescaling what remains.
    pub const fn per_axis(lower: f32) -> Self {
        Self {
            shape: DeadZoneShape::PerAxis,
            lower,
            rescale: true,
        }
    }

    /// Removes the region without stretching what remains back over the full range.
    pub const fn without_rescale(mut self) -> Self {
        self.rescale = false;
        self
    }
}

/// A modifier that transforms one source value before it is written to an action.
pub trait Modifier: Send + Sync + 'static {
    /// Applies the modifier to a runtime value.
    fn apply(&self, value: ActionValue) -> ActionValue;

    /// Whether this modifier stretches its input onto a different range.
    ///
    /// At most one modifier acting on a value may do this, because a later stage's threshold stops
    /// corresponding to a physical position once an earlier one has rescaled. Say `true` here if
    /// yours does, and a binding that stacks two will be rejected when its context is declared.
    fn rescales(&self) -> bool {
        false
    }
}

/// Built-in modifiers that can be chained onto a binding.
pub enum BindingModifier {
    /// Suppresses values near centre, per [`DeadZone`].
    DeadZone(DeadZone),
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
            Self::DeadZone(dead_zone) => apply_dead_zone(value, *dead_zone),
            Self::Scale(scale) => apply_scale(value, *scale),
            Self::Negate => apply_negate(value),
            Self::Swizzle => apply_swizzle(value),
            Self::Clamp { min, max } => apply_clamp(value, *min, *max),
            Self::Curve(power) => apply_curve(value, *power),
            Self::Custom(modifier) => modifier.apply(value),
        }
    }

    /// Whether this modifier stretches its input onto a different range.
    pub fn rescales(&self) -> bool {
        match self {
            Self::DeadZone(dead_zone) => dead_zone.rescale,
            Self::Custom(modifier) => modifier.rescales(),
            _ => false,
        }
    }
}

/// A chainable handle for the binding that was just declared.
pub struct BindingHandle<'a, C> {
    builder: &'a mut InputContextBuilder<C>,
    index: usize,
}

impl<'a, C> BindingHandle<'a, C> {
    fn push_modifier(&mut self, modifier: BindingModifier) {
        self.builder.bindings[self.index].modifiers.push(modifier);
    }

    /// Adds a deadzone.
    ///
    /// ```ignore
    /// context.bind::<Move, _>(Stick::Left).dead_zone(DeadZone::radial(0.15));
    /// ```
    pub fn dead_zone(mut self, dead_zone: DeadZone) -> Self {
        self.push_modifier(BindingModifier::DeadZone(dead_zone));
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
pub struct InputContextBuilder<C> {
    bindings: Vec<BindingSpec>,
    _marker: PhantomData<C>,
}

impl<C> Default for InputContextBuilder<C> {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
            _marker: PhantomData,
        }
    }
}

impl<C> InputContextBuilder<C> {
    fn push_binding<A: InputAction>(&mut self, source: BindingSource) -> BindingHandle<'_, C> {
        self.bindings.push(BindingSpec {
            action: A::id(),
            intent: A::INTENT,
            path: A::PATH,
            source,
            modifiers: Vec::new(),
        });
        let index = self.bindings.len() - 1;
        BindingHandle {
            builder: self,
            index,
        }
    }

    /// Binds an action to a source value.
    ///
    /// An action may be bound more than once — a keyboard key and a gamepad button, a stick and
    /// the movement keys. Every binding for an action contributes to the same value, combined
    /// according to the action's [`Intent`]:
    ///
    /// - `Button`, `Analog1` and `Directional2` take the **strongest** contribution, so pushing the
    ///   stick further wins over tapping a key, and either of two buttons fires the action. Equal
    ///   contributions resolve in the order the bindings were declared.
    /// - `Delta2` **sums** its contributions, because a delta is a displacement and two devices
    ///   moving at once should move the action by both.
    pub fn bind<A, S>(&mut self, source: S) -> BindingHandle<'_, C>
    where
        A: InputAction,
        S: BindingSourceSpec<A::Output>,
    {
        self.push_binding::<A>(source.into_binding_source())
    }

    /// Binds a 2D action to mouse motion.
    pub fn bind_mouse_motion<A>(&mut self) -> BindingHandle<'_, C>
    where
        A: InputAction<Output = Vec2>,
    {
        self.push_binding::<A>(BindingSource::MouseMotion)
    }

    /// Binds a 2D action to four named directional keys.
    #[cfg(feature = "keyboard")]
    pub fn bind_directional<A>(&mut self, keys: DirectionalKeys) -> BindingHandle<'_, C>
    where
        A: InputAction<Output = Vec2>,
    {
        self.push_binding::<A>(BindingSource::Directional2(keys))
    }

    pub(crate) fn finish(self) -> Vec<BindingSpec> {
        self.bindings
    }
}

fn apply_dead_zone(value: ActionValue, dead_zone: DeadZone) -> ActionValue {
    match (value, dead_zone.shape) {
        // A deadzone measures distance from centre, which a boolean does not have.
        (ActionValue::Bool(value), _) => ActionValue::Bool(value),

        // One axis has only one distance to measure, so both shapes agree on it.
        (ActionValue::Axis1(value), _) => ActionValue::Axis1(dead_zone_scalar(value, dead_zone)),

        (ActionValue::Axis2(value), DeadZoneShape::Radial) => {
            ActionValue::Axis2(dead_zone_radial(value, value.length(), dead_zone))
        }
        (ActionValue::Axis3(value), DeadZoneShape::Radial) => {
            ActionValue::Axis3(dead_zone_radial(value, value.length(), dead_zone))
        }

        (ActionValue::Axis2(value), DeadZoneShape::PerAxis) => ActionValue::Axis2(Vec2::new(
            dead_zone_scalar(value.x, dead_zone),
            dead_zone_scalar(value.y, dead_zone),
        )),
        (ActionValue::Axis3(value), DeadZoneShape::PerAxis) => {
            ActionValue::Axis3(bevy_math::Vec3::new(
                dead_zone_scalar(value.x, dead_zone),
                dead_zone_scalar(value.y, dead_zone),
                dead_zone_scalar(value.z, dead_zone),
            ))
        }
    }
}

/// Removes `lower` from a distance, optionally stretching what remains back over the full range.
fn dead_zone_remainder(magnitude: f32, dead_zone: DeadZone) -> f32 {
    let remainder = magnitude - dead_zone.lower;
    if dead_zone.rescale {
        // A deadzone at or above full deflection leaves nothing to stretch.
        remainder / (1.0 - dead_zone.lower).max(f32::EPSILON)
    } else {
        remainder
    }
}

fn dead_zone_scalar(value: f32, dead_zone: DeadZone) -> f32 {
    let magnitude = value.abs();
    if magnitude <= dead_zone.lower {
        0.0
    } else {
        value.signum() * dead_zone_remainder(magnitude, dead_zone)
    }
}

fn dead_zone_radial<V>(value: V, magnitude: f32, dead_zone: DeadZone) -> V
where
    V: core::ops::Mul<f32, Output = V> + Default,
{
    if magnitude <= dead_zone.lower {
        V::default()
    } else {
        // Scale the vector rather than normalizing it: direction is preserved exactly, and a
        // magnitude that survived the test above cannot be zero.
        value * (dead_zone_remainder(magnitude, dead_zone) / magnitude)
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

// The curve shapes distance from centre, not each axis on its own. Shaping the axes separately
// bends the direction a stick is pointing: a 45° push has both components raised to the power,
// which moves the result off the diagonal.
fn apply_curve(value: ActionValue, power: f32) -> ActionValue {
    match value {
        ActionValue::Bool(value) => ActionValue::Bool(value),
        ActionValue::Axis1(value) => {
            ActionValue::Axis1(value.signum() * bevy_math::ops::powf(value.abs(), power))
        }
        ActionValue::Axis2(value) => ActionValue::Axis2(curve_radial(value, value.length(), power)),
        ActionValue::Axis3(value) => ActionValue::Axis3(curve_radial(value, value.length(), power)),
    }
}

fn curve_radial<V>(value: V, magnitude: f32, power: f32) -> V
where
    V: core::ops::Mul<f32, Output = V> + Default,
{
    if magnitude == 0.0 {
        V::default()
    } else {
        value * (bevy_math::ops::powf(magnitude, power) / magnitude)
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

    struct DummyVec2;

    impl InputAction for DummyVec2 {
        type Output = Vec2;

        const INTENT: crate::action::Intent = crate::action::Intent::Directional2;
        const PATH: &'static str = "tests::DummyVec2";
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
                BindingModifier::DeadZone(DeadZone::radial(0.25)),
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

    #[cfg(feature = "keyboard")]
    #[test]
    fn binding_builders_collect_modifiers_in_order() {
        let mut builder = InputContextBuilder::<()>::default();
        builder
            .bind::<DummyButton, _>(KeyCode::Space)
            .scale(2.0)
            .negate()
            .dead_zone(DeadZone::radial(0.1));

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
            BindingModifier::DeadZone(_)
        ));
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn gamepad_source_values_bind_through_the_same_pipeline() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyButton, _>(GamepadButton::South);
        builder.bind::<DummyVec2, _>(Stick::Left);

        let bindings = builder.finish();
        assert_eq!(bindings.len(), 2);
        assert!(matches!(
            bindings[0].source,
            BindingSource::GamepadButton(GamepadButton::South)
        ));
        assert!(matches!(
            bindings[1].source,
            BindingSource::GamepadStick(Stick::Left)
        ));
    }

    fn dead_zoned(dead_zone: DeadZone, value: Vec2) -> Vec2 {
        match BindingModifier::DeadZone(dead_zone).apply(ActionValue::Axis2(value)) {
            ActionValue::Axis2(value) => value,
            other => panic!("expected Axis2, got {other:?}"),
        }
    }

    #[test]
    fn a_radial_dead_zone_treats_every_direction_alike() {
        let dead_zone = DeadZone::radial(0.5);

        // A diagonal push of the same length as a cardinal one is inside the zone too. A per-axis
        // zone of the same size would let this through.
        let diagonal = Vec2::splat(core::f32::consts::FRAC_1_SQRT_2 * 0.4);
        assert_eq!(dead_zoned(dead_zone, diagonal), Vec2::ZERO);
        assert_eq!(dead_zoned(dead_zone, Vec2::new(0.4, 0.0)), Vec2::ZERO);

        // Direction survives the zone unchanged; only the distance is remapped.
        let out = dead_zoned(dead_zone, Vec2::new(0.75, 0.0));
        assert!((out.x - 0.5).abs() < 1e-6, "{out:?}");
        assert_eq!(out.y, 0.0);
    }

    #[test]
    fn a_per_axis_dead_zone_measures_each_axis_on_its_own() {
        let dead_zone = DeadZone::per_axis(0.5);

        // The axis past the threshold survives while the one inside it does not.
        let out = dead_zoned(dead_zone, Vec2::new(0.75, 0.25));
        assert!((out.x - 0.5).abs() < 1e-6, "{out:?}");
        assert_eq!(out.y, 0.0);
    }

    #[test]
    fn rescaling_restores_full_range_and_declining_it_does_not() {
        let rescaled = dead_zoned(DeadZone::radial(0.2), Vec2::new(1.0, 0.0));
        assert!((rescaled.x - 1.0).abs() < 1e-6, "{rescaled:?}");

        // Without rescaling the zone is subtracted and nothing is stretched, so full deflection
        // reads short by exactly the zone.
        let kept = dead_zoned(DeadZone::radial(0.2).without_rescale(), Vec2::new(1.0, 0.0));
        assert!((kept.x - 0.8).abs() < 1e-6, "{kept:?}");
    }

    #[test]
    fn a_dead_zone_applies_in_three_dimensions() {
        let value = ActionValue::Axis3(bevy_math::Vec3::new(0.1, 0.1, 0.1));
        assert_eq!(
            BindingModifier::DeadZone(DeadZone::radial(0.5)).apply(value),
            ActionValue::Axis3(bevy_math::Vec3::ZERO)
        );
    }

    #[test]
    fn a_curve_shapes_distance_without_bending_direction() {
        let diagonal = Vec2::splat(core::f32::consts::FRAC_1_SQRT_2 * 0.5);
        let curved = match BindingModifier::Curve(2.0).apply(ActionValue::Axis2(diagonal)) {
            ActionValue::Axis2(value) => value,
            other => panic!("expected Axis2, got {other:?}"),
        };

        assert!((curved.length() - 0.25).abs() < 1e-6, "{curved:?}");
        assert!(
            (curved.x - curved.y).abs() < 1e-6,
            "still on the diagonal: {curved:?}"
        );
    }

    #[test]
    fn only_a_deliberately_rescaling_modifier_reports_that_it_does() {
        assert!(BindingModifier::DeadZone(DeadZone::radial(0.1)).rescales());
        assert!(!BindingModifier::DeadZone(DeadZone::radial(0.1).without_rescale()).rescales());
        assert!(!BindingModifier::Scale(2.0).rescales());
        assert!(!BindingModifier::Custom(Box::new(DoubleAxis)).rescales());
    }
}
