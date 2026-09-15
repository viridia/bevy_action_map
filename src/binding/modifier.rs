//! Modifiers: what reshapes a binding's value on its way to the action.

use bevy_math::Vec2;
use bevy_platform::sync::Arc;

use crate::action::{ActionValue, ChannelShape, Scratch};

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
    /// Turn it off when something later in the chain measures the same quantity: stretching the
    /// range moves every threshold downstream of it, so at most one deadzone acting on a value may
    /// rescale.
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

/// How many directions a compass modifier rounds to.
///
/// Four is what a table or a list wants: up, down, left and right, and nothing in between. Eight
/// suits a radial menu or eight-way movement, where a diagonal is a direction in its own right
/// rather than a way of asking for one of its neighbours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompassPoints {
    /// The four cardinal directions.
    Four,
    /// The four cardinal directions and the four diagonals.
    Eight,
}

/// A modifier that transforms one input value before it is written to an action.
///
/// Implement this for anything the built-in set does not cover. A modifier is a pure function of
/// what it is given — no world access — so that it produces the same answer when a replay or a
/// rollback runs it again with the same inputs.
pub trait Modifier: Send + Sync + 'static {
    /// Applies the modifier to a runtime value.
    ///
    /// `scratch` is this modifier's own working memory, untouched by anything else, and persists
    /// between ticks. `delta` is how long the owning context's last tick was, in its own seconds —
    /// which is the fixed timestep for a fixed context and the frame time for a render one, and is
    /// zero on the tick a context first evaluates.
    fn apply(&self, value: ActionValue, scratch: &mut Scratch, delta: f32) -> ActionValue;

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
// `Clone` for the reason `BindingSpec` is: applying an override clones the authored bindings and
// rewrites their inputs.
#[derive(Clone)]
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
    /// Scales a vector down when it exceeds magnitude 1, and leaves it alone otherwise.
    ///
    /// A four-key `DirectionalButtons` reaches its full 1.0 on each axis alone, but 1.414 on a
    /// diagonal — two keys held together outrun what a single key or a stick can produce. This
    /// pulls the diagonal back to the same reach as the rest of the circle, without touching
    /// directions that were never too fast to begin with.
    ClampMagnitude,
    /// Maps `min..max` onto `0..1`, clamping anything outside it.
    ///
    /// Useful when a control's own range does not start at zero — a trigger whose rest position
    /// reads `0.1` rather than `0.0`, say. Like a deadzone, this stretches its input over a new
    /// range, so at most one of it or a rescaling deadzone may appear in the same chain.
    Rescale {
        /// The input value mapped to 0.
        min: f32,
        /// The input value mapped to 1.
        max: f32,
    },
    /// Raises the magnitude to a curve power while preserving sign.
    Curve(f32),
    /// Reads the value as a rate and turns it into the displacement it produced this tick.
    PerSecond(f32),
    /// Rounds a 2D direction to the nearest of four or eight compass points.
    Compass(CompassPoints),
    /// Turns a momentary button into a sustained on/off latch.
    ///
    /// `active: false` is identity — the raw value passes through unchanged, which is an ordinary
    /// held control. `active: true` flips the latch on each press edge and reports the latch state
    /// instead of the raw one. What
    /// [`hold_or_toggle`](crate::binding::InputContextBuilder::hold_or_toggle) declares; `active`
    /// is the field a tunable adjusts.
    Toggle {
        /// Whether the latch is live. Off is a held control; on is a toggle.
        active: bool,
    },
    /// Calls an application-defined modifier.
    ///
    /// Shared rather than owned, so that copying a binding set copies the reference and not the
    /// modifier. Use [`custom`](crate::binding::BindingBuilder::custom) rather than building this
    /// by hand.
    Custom(Arc<dyn Modifier>),
}

impl BindingModifier {
    /// Applies this modifier to a runtime value.
    pub fn apply(&self, value: ActionValue, scratch: &mut Scratch, delta: f32) -> ActionValue {
        match self {
            Self::DeadZone(dead_zone) => apply_dead_zone(value, *dead_zone),
            Self::Scale(scale) => apply_scale(value, *scale),
            Self::Negate => apply_negate(value),
            Self::Swizzle => apply_swizzle(value),
            Self::Clamp { min, max } => apply_clamp(value, *min, *max),
            Self::ClampMagnitude => apply_clamp_magnitude(value),
            Self::Rescale { min, max } => apply_rescale(value, *min, *max),
            Self::Curve(power) => apply_curve(value, *power),
            Self::PerSecond(scale) => apply_scale(value, scale * delta),
            Self::Compass(points) => apply_compass(value, *points),
            Self::Toggle { active } => apply_toggle(value, scratch, *active),
            Self::Custom(modifier) => modifier.apply(value, scratch, delta),
        }
    }

    /// The channel shape this modifier leaves its value on, when it changes it.
    ///
    /// Only a conversion between a rate and a displacement does: everything else reshapes the
    /// number without changing what kind of quantity it is.
    pub(crate) fn reshapes(&self) -> Option<ChannelShape> {
        match self {
            Self::PerSecond(_) => Some(ChannelShape::Delta2),
            _ => None,
        }
    }

    /// Whether this modifier stretches its input onto a different range.
    pub fn rescales(&self) -> bool {
        match self {
            Self::DeadZone(dead_zone) => dead_zone.rescale,
            Self::Rescale { .. } => true,
            Self::Custom(modifier) => modifier.rescales(),
            // Not `Compass`, which discards magnitude rather than stretching it, or
            // `ClampMagnitude`, which only pulls in what already overshot. A deadzone deciding when
            // the stick counts as deflected and a compass reading which way is the pairing this is
            // built for, not the stacking the check refuses.
            _ => false,
        }
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
    // A deadzone at or above full deflection leaves nothing to stretch the remainder onto, so it
    // passes through unstretched rather than dividing by (near) zero. Reachable at runtime even
    // where `dead_zone` was declared well inside range: `tunable_dead_zone` lets a player drag
    // `lower` there from a slider. The result is not continuous with `lower` just under 1.0, where
    // rescaling still stretches hard.
    if dead_zone.rescale && dead_zone.lower < 1.0 {
        remainder / (1.0 - dead_zone.lower)
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

fn apply_compass(value: ActionValue, points: CompassPoints) -> ActionValue {
    match value {
        ActionValue::Axis2(value) => ActionValue::Axis2(compass_direction(value, points)),
        ActionValue::Axis1(value) => {
            ActionValue::Axis1(if value == 0.0 { 0.0 } else { value.signum() })
        }
        // Neither a boolean nor a 3D value is a direction this knows how to round.
        other => other,
    }
}

/// Rounds a vector to the nearest compass point, as a unit vector.
///
/// Bevy's own compass types do the rounding, so that a menu navigated through this and one
/// navigated by `bevy_input_focus` directly agree about where the boundary between two directions
/// falls, down to the degree.
fn compass_direction(value: Vec2, points: CompassPoints) -> Vec2 {
    // At rest there is no direction to round to, which is also what a reader wants to see: no
    // input, rather than an arbitrary one of the points.
    let Ok(direction) = bevy_math::Dir2::new(value) else {
        return Vec2::ZERO;
    };
    match points {
        CompassPoints::Four => bevy_math::Dir2::from(bevy_math::CompassQuadrant::from(direction)),
        CompassPoints::Eight => bevy_math::Dir2::from(bevy_math::CompassOctant::from(direction)),
    }
    .as_vec2()
}

/// Bit position within `Scratch::flags` this modifier's latch lives at. Its own `Scratch` slot —
/// see `apply_modifiers`' per-modifier split in `eval.rs` — so nothing else on the binding can
/// collide with it.
const TOGGLE_LATCH: u8 = 1 << 0;

/// Converts a momentary button into a sustained latch, active only while `active` says so.
///
/// `scratch.prev` is tracked whether or not the latch is live, so switching modes mid-press cannot
/// manufacture a spurious edge the tick after the switch.
///
/// Used only for a binding whose tunable is *not* shared with another. A shared one is resolved
/// once per tick for the whole group instead — see `eval.rs`'s `fold`, which reads `toggle_latch`
/// rather than calling this at all, and the doc on `TunableShared` for why: running this
/// independently per binding, against a scratch cell other bindings in the group also write,
/// spuriously re-flips the latch on every tick a *different* member of the group is held.
fn apply_toggle(value: ActionValue, scratch: &mut Scratch, active: bool) -> ActionValue {
    let actuated = value.to_bool();
    let was = scratch.prev.to_bool();
    scratch.prev = value;

    if !active {
        return value;
    }
    if actuated && !was {
        scratch.flags ^= TOGGLE_LATCH;
    }
    ActionValue::Bool(scratch.flags & TOGGLE_LATCH != 0)
}

/// Whether a shared toggle's latch currently reads on — the bit [`apply_toggle`] uses, read back
/// out of the plan's shared cell for a binding's group rather than its own private one.
pub(crate) fn toggle_latch(scratch: &Scratch) -> bool {
    scratch.flags & TOGGLE_LATCH != 0
}

/// Resolves one tick of a shared toggle's latch, from every sharing binding's raw actuation
/// combined — never per binding, which is what [`apply_toggle`]'s own doc explains is unsafe here.
/// `actuated` is the combined reading; `scratch` is the group's one shared cell, carrying the
/// combined reading from last tick in `prev` the same way a private toggle carries its own.
///
/// `active` mirrors [`apply_toggle`]'s own parameter: the bit only moves while the group's tunable
/// says toggle mode is on, and `prev` is tracked regardless of it, for the same reason.
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
pub(crate) fn resolve_shared_toggle(actuated: bool, active: bool, scratch: &mut Scratch) {
    let was = scratch.prev.to_bool();
    if active && actuated && !was {
        scratch.flags ^= TOGGLE_LATCH;
    }
    scratch.prev = ActionValue::Bool(actuated);
}

/// A shared group's current toggle setting, read off any one member — `hold_or_toggle` and override
/// application ([`apply_tunable_value`]) keep every sharing binding's own copy in lockstep, so
/// which one answers does not matter.
///
/// Not feature-gated like its neighbours: the fold's per-binding read reaches this unconditionally,
/// same as [`toggle_latch`], since which device features are enabled cannot change what a slice of
/// already-compiled modifiers holds.
pub(crate) fn toggle_active(modifiers: &[BindingModifier]) -> bool {
    modifiers
        .iter()
        .find_map(|modifier| match modifier {
            BindingModifier::Toggle { active } => Some(*active),
            _ => None,
        })
        .unwrap_or(false)
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

fn apply_clamp_magnitude(value: ActionValue) -> ActionValue {
    match value {
        ActionValue::Bool(value) => ActionValue::Bool(value),
        ActionValue::Axis1(value) => ActionValue::Axis1(value.clamp(-1.0, 1.0)),
        ActionValue::Axis2(value) => {
            ActionValue::Axis2(clamp_magnitude_radial(value, value.length()))
        }
        ActionValue::Axis3(value) => {
            ActionValue::Axis3(clamp_magnitude_radial(value, value.length()))
        }
    }
}

fn clamp_magnitude_radial<V>(value: V, magnitude: f32) -> V
where
    V: core::ops::Mul<f32, Output = V>,
{
    if magnitude > 1.0 {
        value * (1.0 / magnitude)
    } else {
        value
    }
}

fn apply_rescale(value: ActionValue, min: f32, max: f32) -> ActionValue {
    match value {
        ActionValue::Bool(value) => ActionValue::Bool(value),
        ActionValue::Axis1(value) => ActionValue::Axis1(rescale_scalar(value, min, max)),
        ActionValue::Axis2(value) => ActionValue::Axis2(Vec2::new(
            rescale_scalar(value.x, min, max),
            rescale_scalar(value.y, min, max),
        )),
        ActionValue::Axis3(value) => ActionValue::Axis3(bevy_math::Vec3::new(
            rescale_scalar(value.x, min, max),
            rescale_scalar(value.y, min, max),
            rescale_scalar(value.z, min, max),
        )),
    }
}

fn rescale_scalar(value: f32, min: f32, max: f32) -> f32 {
    ((value - min) / (max - min)).clamp(0.0, 1.0)
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

#[cfg(test)]
mod tests {
    use super::*;

    use bevy_math::Vec2;

    struct DoubleAxis;

    impl Modifier for DoubleAxis {
        fn apply(&self, value: ActionValue, _scratch: &mut Scratch, _delta: f32) -> ActionValue {
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
            (
                BindingModifier::ClampMagnitude,
                ActionValue::Axis1(2.5),
                ActionValue::Axis1(1.0),
            ),
            (
                BindingModifier::Rescale { min: 0.1, max: 1.0 },
                ActionValue::Axis1(0.1),
                ActionValue::Axis1(0.0),
            ),
        ];

        for (modifier, input, expected) in cases {
            assert_eq!(
                modifier.apply(input, &mut Scratch::default(), 0.0),
                expected
            );
        }
    }

    /// A toggle stays engaged across a release — that is the whole point of it — and flips off
    /// again only on the second press, not the second release.
    #[test]
    fn toggle_latches_on_a_press_and_survives_a_release() {
        let modifier = BindingModifier::Toggle { active: true };
        let mut scratch = Scratch::default();

        assert_eq!(
            modifier.apply(ActionValue::Bool(false), &mut scratch, 0.0),
            ActionValue::Bool(false),
            "nothing pressed yet"
        );
        assert_eq!(
            modifier.apply(ActionValue::Bool(true), &mut scratch, 0.0),
            ActionValue::Bool(true),
            "a press flips the latch on"
        );
        assert_eq!(
            modifier.apply(ActionValue::Bool(false), &mut scratch, 0.0),
            ActionValue::Bool(true),
            "letting go does not turn a toggle back off"
        );
        assert_eq!(
            modifier.apply(ActionValue::Bool(true), &mut scratch, 0.0),
            ActionValue::Bool(false),
            "the second press flips it back off"
        );
    }

    /// `active: false` is what `hold_or_toggle` declares before a player turns toggle mode on, and
    /// it must cost nothing: the raw value passes straight through.
    #[test]
    fn an_inactive_toggle_is_identity() {
        let modifier = BindingModifier::Toggle { active: false };
        let mut scratch = Scratch::default();

        assert_eq!(
            modifier.apply(ActionValue::Bool(true), &mut scratch, 0.0),
            ActionValue::Bool(true)
        );
        assert_eq!(
            modifier.apply(ActionValue::Bool(false), &mut scratch, 0.0),
            ActionValue::Bool(false)
        );
    }

    #[test]
    fn switching_to_toggle_mode_mid_press_does_not_manufacture_an_edge() {
        let mut scratch = Scratch::default();
        BindingModifier::Toggle { active: false }.apply(ActionValue::Bool(true), &mut scratch, 0.0);

        assert_eq!(
            BindingModifier::Toggle { active: true }.apply(
                ActionValue::Bool(true),
                &mut scratch,
                0.0
            ),
            ActionValue::Bool(false),
            "still held rather than a new press, so the latch has not moved"
        );
    }

    #[test]
    fn custom_modifiers_fit_into_the_chain() {
        let modifier = BindingModifier::Custom(Arc::new(DoubleAxis));

        assert_eq!(
            modifier.apply(
                ActionValue::Axis2(Vec2::new(1.0, -2.0)),
                &mut Scratch::default(),
                0.0
            ),
            ActionValue::Axis2(Vec2::new(2.0, -4.0))
        );
    }

    fn dead_zoned(dead_zone: DeadZone, value: Vec2) -> Vec2 {
        match BindingModifier::DeadZone(dead_zone).apply(
            ActionValue::Axis2(value),
            &mut Scratch::default(),
            0.0,
        ) {
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

    fn compassed(points: CompassPoints, value: Vec2) -> Vec2 {
        match BindingModifier::Compass(points).apply(
            ActionValue::Axis2(value),
            &mut Scratch::default(),
            0.0,
        ) {
            ActionValue::Axis2(value) => value,
            other => panic!("expected Axis2, got {other:?}"),
        }
    }

    #[test]
    fn four_points_round_a_diagonal_to_a_cardinal_direction() {
        // Slightly north of north-east, so there is a right answer rather than a tie.
        let out = compassed(CompassPoints::Four, Vec2::new(0.4, 0.5));
        assert_eq!(out, Vec2::Y);
        assert_eq!(
            compassed(CompassPoints::Four, Vec2::new(-0.9, -0.1)),
            -Vec2::X
        );
    }

    #[test]
    fn eight_points_keep_a_diagonal_as_a_direction_of_its_own() {
        let out = compassed(CompassPoints::Eight, Vec2::new(0.4, 0.5));
        let expected = Vec2::splat(core::f32::consts::FRAC_1_SQRT_2);
        assert!(out.abs_diff_eq(expected, 1e-6), "{out:?}");
    }

    /// Magnitude is what the modifier throws away: how far the stick was pushed says nothing about
    /// which way it was pushed, and a menu only asked the second question.
    #[test]
    fn a_compass_reports_a_direction_at_full_length_however_far_the_control_travelled() {
        assert_eq!(
            compassed(CompassPoints::Eight, Vec2::new(0.0, 0.05)),
            Vec2::Y
        );
        assert_eq!(
            compassed(CompassPoints::Eight, Vec2::new(0.0, 1.0)),
            Vec2::Y
        );
        // Rest has no direction, and rounding it to an arbitrary one would be a phantom input.
        assert_eq!(compassed(CompassPoints::Eight, Vec2::ZERO), Vec2::ZERO);
    }

    #[test]
    fn one_dimension_has_two_compass_points() {
        let signed = |value: f32| match BindingModifier::Compass(CompassPoints::Four).apply(
            ActionValue::Axis1(value),
            &mut Scratch::default(),
            0.0,
        ) {
            ActionValue::Axis1(value) => value,
            other => panic!("expected Axis1, got {other:?}"),
        };
        assert_eq!(signed(0.3), 1.0);
        assert_eq!(signed(-0.9), -1.0);
        assert_eq!(signed(0.0), 0.0);
    }

    /// A deadzone rescales and a compass does not, so the two stack — which is the pairing the
    /// modifier is built for, and would be refused if it declared otherwise.
    #[test]
    fn a_dead_zone_and_a_compass_are_not_two_rescalings() {
        assert!(!BindingModifier::Compass(CompassPoints::Eight).rescales());
    }

    /// `ClampMagnitude` only pulls in what already overshot, so it stacks with a rescaling
    /// deadzone the same way a compass does.
    #[test]
    fn clamp_magnitude_does_not_rescale_but_rescale_does() {
        assert!(!BindingModifier::ClampMagnitude.rescales());
        assert!(BindingModifier::Rescale { min: 0.0, max: 1.0 }.rescales());
    }

    #[test]
    fn clamp_magnitude_reins_in_a_diagonal_but_leaves_a_cardinal_alone() {
        let diagonal = BindingModifier::ClampMagnitude.apply(
            ActionValue::Axis2(Vec2::new(1.0, 1.0)),
            &mut Scratch::default(),
            0.0,
        );
        let ActionValue::Axis2(diagonal) = diagonal else {
            unreachable!()
        };
        assert!((diagonal.length() - 1.0).abs() < 1e-6, "{diagonal:?}");

        assert_eq!(
            BindingModifier::ClampMagnitude.apply(
                ActionValue::Axis2(Vec2::new(1.0, 0.0)),
                &mut Scratch::default(),
                0.0,
            ),
            ActionValue::Axis2(Vec2::new(1.0, 0.0))
        );
    }

    /// `rescale` maps its declared range onto 0..1 and clamps whatever falls outside it, the same
    /// way a trigger whose rest position never quite reaches zero gets corrected.
    #[test]
    fn rescale_maps_its_range_onto_zero_to_one_and_clamps_outside_it() {
        let modifier = BindingModifier::Rescale { min: 0.1, max: 0.9 };
        let mut scratch = Scratch::default();
        let mut rescaled = |input| {
            modifier
                .apply(ActionValue::Axis1(input), &mut scratch, 0.0)
                .to_axis1()
        };

        assert!((rescaled(0.5) - 0.5).abs() < 1e-6);
        assert_eq!(rescaled(0.0), 0.0, "below the range clamps to 0");
        assert_eq!(rescaled(1.0), 1.0, "above the range clamps to 1");
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
    fn a_dead_zone_at_full_deflection_does_not_blow_up() {
        // Reachable from `radial(1.0)` directly, and from any lower value a player drove there
        // with `tunable_dead_zone`. A magnitude past 1.0 is not exotic: a diagonal
        // `DirectionalButtons` reaches 1.414, and `MouseMove` carries an unbounded pixel delta.
        let out = dead_zoned(DeadZone::radial(1.0), Vec2::new(3.0, 0.0));
        assert!((out.x - 2.0).abs() < 1e-6, "{out:?}");
        assert_eq!(out.y, 0.0);

        // At or under full deflection, nothing survives the zone.
        assert_eq!(
            dead_zoned(DeadZone::radial(1.0), Vec2::new(1.0, 0.0)),
            Vec2::ZERO
        );
    }

    #[test]
    fn a_dead_zone_applies_in_three_dimensions() {
        let value = ActionValue::Axis3(bevy_math::Vec3::new(0.1, 0.1, 0.1));
        assert_eq!(
            BindingModifier::DeadZone(DeadZone::radial(0.5)).apply(
                value,
                &mut Scratch::default(),
                0.0
            ),
            ActionValue::Axis3(bevy_math::Vec3::ZERO)
        );
    }

    #[test]
    fn a_curve_shapes_distance_without_bending_direction() {
        let diagonal = Vec2::splat(core::f32::consts::FRAC_1_SQRT_2 * 0.5);
        let curved = match BindingModifier::Curve(2.0).apply(
            ActionValue::Axis2(diagonal),
            &mut Scratch::default(),
            0.0,
        ) {
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
        assert!(!BindingModifier::Custom(Arc::new(DoubleAxis)).rescales());
    }
}
