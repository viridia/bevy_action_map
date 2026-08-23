//! Bindings, composites, modifiers, and conditions.
//!
//! A binding says what drives an action: a control the hardware reports directly, or a composite
//! that assembles several controls into a value no single one of them carries. Modifiers reshape
//! what a binding produces on its way to the action.

use alloc::{boxed::Box, vec::Vec};
use core::marker::PhantomData;

#[cfg(feature = "gamepad")]
use bevy_input::gamepad::{GamepadAxis, GamepadButton};
#[cfg(feature = "keyboard")]
use bevy_input::keyboard::KeyCode;
use bevy_math::Vec2;

use crate::action::{ActionId, ActionValue, ChannelShape, InputAction, Intent, Scratch};
use crate::condition::{BindingCondition, Condition};
use crate::event::{Dispatch, dispatch_for};

/// A control that reports on a button channel.
///
/// This is what the parts of a [`DirectionalButtons`] composite are made of. A keyboard key and a
/// D-pad button are the same kind of thing here — both report pressed or not — which is what lets
/// one composite serve either.
///
/// You seldom write this type. Anywhere a part is wanted, the control itself will do:
/// `DirectionalButtons::new(KeyCode::KeyW, ..)` and `DirectionalButtons::new(GamepadButton::DPadUp, ..)`
/// both convert on the way in.
#[cfg(any(feature = "keyboard", feature = "gamepad"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonControl {
    /// A keyboard key.
    #[cfg(feature = "keyboard")]
    Key(KeyCode),
    /// A gamepad button, including the D-pad and the triggers.
    #[cfg(feature = "gamepad")]
    GamepadButton(GamepadButton),
}

#[cfg(feature = "keyboard")]
impl From<KeyCode> for ButtonControl {
    fn from(key: KeyCode) -> Self {
        Self::Key(key)
    }
}

#[cfg(feature = "gamepad")]
impl From<GamepadButton> for ButtonControl {
    fn from(button: GamepadButton) -> Self {
        Self::GamepadButton(button)
    }
}

/// Two buttons that together make a signed axis.
///
/// Turning left and right, leaning, strafing, cycling a list — a great many controls are a pair of
/// buttons pushing one number in opposite directions, and there is no single control that reports
/// that way. Holding both is the same as holding neither.
///
/// ```ignore
/// context.bind::<Turn>(AxisButtons::ad());
/// context.bind::<Turn>(GamepadAxis::LeftStickX);
/// ```
///
/// Note what the second line is doing: a stick axis already reports signed, so it needs no
/// composite. Both bindings feed the same action, and the player uses whichever they reach for.
#[cfg(any(feature = "keyboard", feature = "gamepad"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AxisButtons {
    /// The button that drives the axis negative.
    pub negative: ButtonControl,
    /// The button that drives it positive.
    pub positive: ButtonControl,
}

#[cfg(any(feature = "keyboard", feature = "gamepad"))]
impl AxisButtons {
    /// Creates an axis from the two buttons that drive it either way.
    pub fn new(negative: impl Into<ButtonControl>, positive: impl Into<ButtonControl>) -> Self {
        Self {
            negative: negative.into(),
            positive: positive.into(),
        }
    }

    /// The `A` and `D` keys.
    #[cfg(feature = "keyboard")]
    pub const fn ad() -> Self {
        Self {
            negative: ButtonControl::Key(KeyCode::KeyA),
            positive: ButtonControl::Key(KeyCode::KeyD),
        }
    }

    /// The left and right arrow keys.
    #[cfg(feature = "keyboard")]
    pub const fn left_right() -> Self {
        Self {
            negative: ButtonControl::Key(KeyCode::ArrowLeft),
            positive: ButtonControl::Key(KeyCode::ArrowRight),
        }
    }
}

/// Four buttons that together make a direction.
///
/// A direction never arrives from the hardware as a direction. WASD is four keys and a D-pad is
/// four buttons — Bevy reports no D-pad axis at all — so both reach a 2D action through this, and
/// through the same code. Whichever a player uses, an action bound this way behaves identically.
///
/// ```ignore
/// context.bind::<Move>(DirectionalButtons::wasd());
/// context.bind::<Move>(DirectionalButtons::dpad());
/// ```
///
/// The parts are named for the direction each one pushes rather than for its position on a device,
/// which is what a rebinding screen needs in order to say "Move Forward" next to one of them.
#[cfg(any(feature = "keyboard", feature = "gamepad"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirectionalButtons {
    /// The button that contributes positive Y.
    pub up: ButtonControl,
    /// The button that contributes negative Y.
    pub down: ButtonControl,
    /// The button that contributes negative X.
    pub left: ButtonControl,
    /// The button that contributes positive X.
    pub right: ButtonControl,
}

#[cfg(any(feature = "keyboard", feature = "gamepad"))]
impl DirectionalButtons {
    /// Creates a directional composite from four buttons.
    ///
    /// Each part accepts anything that reports on a button channel, so the four need not come from
    /// the same device.
    pub fn new(
        up: impl Into<ButtonControl>,
        down: impl Into<ButtonControl>,
        left: impl Into<ButtonControl>,
        right: impl Into<ButtonControl>,
    ) -> Self {
        Self {
            up: up.into(),
            down: down.into(),
            left: left.into(),
            right: right.into(),
        }
    }

    /// The `W`, `A`, `S` and `D` keys.
    #[cfg(feature = "keyboard")]
    pub const fn wasd() -> Self {
        Self {
            up: ButtonControl::Key(KeyCode::KeyW),
            down: ButtonControl::Key(KeyCode::KeyS),
            left: ButtonControl::Key(KeyCode::KeyA),
            right: ButtonControl::Key(KeyCode::KeyD),
        }
    }

    /// The four arrow keys.
    #[cfg(feature = "keyboard")]
    pub const fn arrow_keys() -> Self {
        Self {
            up: ButtonControl::Key(KeyCode::ArrowUp),
            down: ButtonControl::Key(KeyCode::ArrowDown),
            left: ButtonControl::Key(KeyCode::ArrowLeft),
            right: ButtonControl::Key(KeyCode::ArrowRight),
        }
    }

    /// The gamepad D-pad.
    #[cfg(feature = "gamepad")]
    pub const fn dpad() -> Self {
        Self {
            up: ButtonControl::GamepadButton(GamepadButton::DPadUp),
            down: ButtonControl::GamepadButton(GamepadButton::DPadDown),
            left: ButtonControl::GamepadButton(GamepadButton::DPadLeft),
            right: ButtonControl::GamepadButton(GamepadButton::DPadRight),
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
    // The only place the concrete action type survives bind time. Everything downstream works in
    // slots, which cannot name a generic event.
    pub(crate) dispatch: Dispatch,
    pub(crate) source: BindingSource,
    pub(crate) modifiers: Vec<BindingModifier>,
    pub(crate) conditions: Vec<BindingCondition>,
    pub(crate) consume: bool,
    #[cfg(any(feature = "keyboard", feature = "gamepad"))]
    pub(crate) chord: Vec<ButtonControl>,
}

/// Mouse motion as a binding source.
///
/// ```ignore
/// context.bind::<Look>(MouseMove);
/// ```
///
/// This reports a displacement that has already happened, so it can only drive an action whose
/// intent is [`Delta2`](Intent::Delta2). It is named for the movement rather than for the device
/// so that it does not collide with Bevy's own `MouseMotion` message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseMove;

/// One physical control.
///
/// A binding names a *source*, which may be a control or an arrangement of several — a directional
/// composite is four buttons and a stick is two axes. This is what those decompose into, and it is
/// the granularity at which one context takes a control from another: a menu claiming the movement
/// keys claims four controls, and a global screenshot key bound to a fifth is unaffected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    /// A keyboard key.
    #[cfg(feature = "keyboard")]
    Key(KeyCode),
    /// A gamepad button, including the D-pad and the triggers.
    #[cfg(feature = "gamepad")]
    GamepadButton(GamepadButton),
    /// One axis of a gamepad stick or trigger.
    #[cfg(feature = "gamepad")]
    GamepadAxis(GamepadAxis),
    /// The mouse being moved.
    MouseMotion,
}

#[cfg(any(feature = "keyboard", feature = "gamepad"))]
impl From<ButtonControl> for Control {
    fn from(control: ButtonControl) -> Self {
        match control {
            #[cfg(feature = "keyboard")]
            ButtonControl::Key(key) => Self::Key(key),
            #[cfg(feature = "gamepad")]
            ButtonControl::GamepadButton(button) => Self::GamepadButton(button),
        }
    }
}

/// The binding source used by the first interactive stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingSource {
    /// A keyboard key.
    #[cfg(feature = "keyboard")]
    Button(KeyCode),
    /// A two-button signed axis composite.
    #[cfg(any(feature = "keyboard", feature = "gamepad"))]
    Axis1(AxisButtons),
    /// A four-button directional composite.
    #[cfg(any(feature = "keyboard", feature = "gamepad"))]
    Directional2(DirectionalButtons),
    /// Mouse motion.
    MouseMotion,
    /// A gamepad button.
    #[cfg(feature = "gamepad")]
    GamepadButton(GamepadButton),
    /// A single gamepad axis.
    #[cfg(feature = "gamepad")]
    GamepadAxis(GamepadAxis),
    /// A left or right gamepad stick.
    #[cfg(feature = "gamepad")]
    GamepadStick(Stick),
}

impl BindingSource {
    /// Calls `visit` with every physical control this source reads.
    ///
    /// One for a plain control, four for a directional composite, two for a stick. This is what
    /// consumption and chord clashes are recorded against, so that taking a composite takes its
    /// parts rather than an arrangement nothing else can name.
    ///
    /// Allocation-free, because it runs per binding per tick (R23.2). Use
    /// [`controls`](Self::controls) where a collection is more convenient than a callback.
    pub fn for_each_control(&self, mut visit: impl FnMut(Control)) {
        match self {
            #[cfg(feature = "keyboard")]
            Self::Button(key) => visit(Control::Key(*key)),
            #[cfg(any(feature = "keyboard", feature = "gamepad"))]
            Self::Axis1(parts) => {
                visit(parts.negative.into());
                visit(parts.positive.into());
            }
            #[cfg(any(feature = "keyboard", feature = "gamepad"))]
            Self::Directional2(parts) => {
                visit(parts.up.into());
                visit(parts.down.into());
                visit(parts.left.into());
                visit(parts.right.into());
            }
            Self::MouseMotion => visit(Control::MouseMotion),
            #[cfg(feature = "gamepad")]
            Self::GamepadButton(button) => visit(Control::GamepadButton(*button)),
            #[cfg(feature = "gamepad")]
            Self::GamepadAxis(axis) => visit(Control::GamepadAxis(*axis)),
            #[cfg(feature = "gamepad")]
            Self::GamepadStick(stick) => {
                let (x, y) = stick.axes();
                visit(Control::GamepadAxis(x));
                visit(Control::GamepadAxis(y));
            }
        }
    }

    /// Every physical control this source reads, collected.
    pub fn controls(&self) -> alloc::vec::Vec<Control> {
        let mut controls = alloc::vec::Vec::new();
        self.for_each_control(|control| controls.push(control));
        controls
    }

    /// The kind of channel this source reports on.
    pub const fn channel_shape(&self) -> ChannelShape {
        match self {
            #[cfg(feature = "keyboard")]
            Self::Button(_) => ChannelShape::Button,
            // Buttons, but an axis and a direction by the time anything binds to them.
            #[cfg(any(feature = "keyboard", feature = "gamepad"))]
            Self::Axis1(_) => ChannelShape::Axis1,
            #[cfg(any(feature = "keyboard", feature = "gamepad"))]
            Self::Directional2(_) => ChannelShape::Axis2,
            Self::MouseMotion => ChannelShape::Delta2,
            // Including the triggers, which carry a fraction on this channel.
            #[cfg(feature = "gamepad")]
            Self::GamepadButton(_) => ChannelShape::Button,
            #[cfg(feature = "gamepad")]
            Self::GamepadAxis(_) => ChannelShape::Axis1,
            #[cfg(feature = "gamepad")]
            Self::GamepadStick(_) => ChannelShape::Axis2,
        }
    }
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

#[cfg(feature = "gamepad")]
impl Stick {
    /// The horizontal and vertical axes this stick reports on.
    pub const fn axes(self) -> (GamepadAxis, GamepadAxis) {
        match self {
            Self::Left => (GamepadAxis::LeftStickX, GamepadAxis::LeftStickY),
            Self::Right => (GamepadAxis::RightStickX, GamepadAxis::RightStickY),
        }
    }
}

/// A value that names a control you can bind an action to.
///
/// Implemented for the control types you would reach for directly — a [`KeyCode`], a
/// [`GamepadButton`], a [`Stick`], [`MouseMove`], a [`DirectionalButtons`] composite — so that
/// [`bind`](InputContextBuilder::bind) accepts any of them.
///
/// Note what this trait does *not* say: which actions the control is good for. A control reports on
/// a channel of a given [`ChannelShape`] and that is all it knows about itself; whether that suits
/// a particular action is decided against the action's [`Intent`] when the context is declared.
/// This is what lets one trigger drive a button action in one game and an analog action in another.
pub trait BindingSourceSpec {
    /// Converts this source value into the internal binding representation.
    fn into_binding_source(self) -> BindingSource;
}

#[cfg(feature = "keyboard")]
impl BindingSourceSpec for KeyCode {
    fn into_binding_source(self) -> BindingSource {
        BindingSource::Button(self)
    }
}

#[cfg(any(feature = "keyboard", feature = "gamepad"))]
impl BindingSourceSpec for AxisButtons {
    fn into_binding_source(self) -> BindingSource {
        BindingSource::Axis1(self)
    }
}

#[cfg(any(feature = "keyboard", feature = "gamepad"))]
impl BindingSourceSpec for DirectionalButtons {
    fn into_binding_source(self) -> BindingSource {
        BindingSource::Directional2(self)
    }
}

impl BindingSourceSpec for MouseMove {
    fn into_binding_source(self) -> BindingSource {
        BindingSource::MouseMotion
    }
}

#[cfg(feature = "gamepad")]
impl BindingSourceSpec for GamepadButton {
    fn into_binding_source(self) -> BindingSource {
        BindingSource::GamepadButton(self)
    }
}

#[cfg(feature = "gamepad")]
impl BindingSourceSpec for GamepadAxis {
    fn into_binding_source(self) -> BindingSource {
        BindingSource::GamepadAxis(self)
    }
}

#[cfg(feature = "gamepad")]
impl BindingSourceSpec for Stick {
    fn into_binding_source(self) -> BindingSource {
        BindingSource::GamepadStick(self)
    }
}

/// When a control that reports a fraction counts as pressed.
///
/// An analog trigger does not press — it travels. Something has to decide where along that travel
/// a button action fires, and a single point is the wrong answer: a finger resting near it makes
/// the value wobble across the line and the action chatters on and off. So there are two points.
/// The control becomes pressed at [`press`](Self::press) and does not release until it falls back
/// to [`release`](Self::release), and anything in between leaves it as it was.
///
/// ```rust
/// use bevy_action_map::binding::ButtonThreshold;
///
/// // A hair trigger that still resists chatter.
/// let quick = ButtonThreshold { press: 0.25, release: 0.15 };
/// ```
///
/// This is one setting for the whole app rather than one per binding, so that a trigger bound to
/// two actions can never be pressed for one and released for the other.
#[derive(bevy_ecs::resource::Resource, Clone, Copy, Debug, PartialEq)]
pub struct ButtonThreshold {
    /// The value at or above which a control becomes pressed.
    pub press: f32,
    /// The value at or below which it releases again.
    pub release: f32,
}

impl Default for ButtonThreshold {
    fn default() -> Self {
        // Astride the half-way point, which is where a backend that synthesizes its own press
        // usually puts it, with enough of a gap that a resting finger cannot rattle across both.
        Self {
            press: 0.6,
            release: 0.4,
        }
    }
}

impl ButtonThreshold {
    /// Decides whether a control reading `value` is pressed, given whether it was a moment ago.
    pub fn pressed(&self, value: f32, was_pressed: bool) -> bool {
        if value >= self.press {
            true
        } else if value <= self.release {
            false
        } else {
            was_pressed
        }
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
    /// Reads the value as a rate and turns it into the displacement it produced this tick.
    PerSecond(f32),
    /// Calls an application-defined modifier.
    Custom(Box<dyn Modifier>),
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
            Self::Curve(power) => apply_curve(value, *power),
            Self::PerSecond(scale) => apply_scale(value, scale * delta),
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

    fn push_condition(&mut self, condition: BindingCondition) {
        self.builder.bindings[self.index].conditions.push(condition);
    }

    /// Fires on the press rather than for as long as the control is held.
    pub fn press(mut self) -> Self {
        self.push_condition(BindingCondition::Press);
        self
    }

    /// Fires when the control is let go.
    pub fn release(mut self) -> Self {
        self.push_condition(BindingCondition::Release);
        self
    }

    /// Requires the control to still be held, alongside whatever else this binding asks for.
    pub fn down(mut self) -> Self {
        self.push_condition(BindingCondition::Down);
        self
    }

    /// Fires once the control has been held for `duration` seconds, and keeps firing after.
    ///
    /// Letting go early cancels rather than firing, so an action can show how far along it is and
    /// then take it back.
    pub fn hold(mut self, duration: f32) -> Self {
        self.push_condition(BindingCondition::Hold {
            duration,
            one_shot: false,
        });
        self
    }

    /// Fires once, when the control has been held for `duration` seconds.
    pub fn hold_once(mut self, duration: f32) -> Self {
        self.push_condition(BindingCondition::Hold {
            duration,
            one_shot: true,
        });
        self
    }

    /// Fires on release, if the control was held for at least `duration` seconds first.
    pub fn hold_and_release(mut self, duration: f32) -> Self {
        self.push_condition(BindingCondition::HoldAndRelease { duration });
        self
    }

    /// Fires on release, if the control was held no longer than `max_duration` seconds.
    pub fn tap(mut self, max_duration: f32) -> Self {
        self.push_condition(BindingCondition::Tap { max_duration });
        self
    }

    /// Fires after `count` taps, each within `max_gap` seconds of the one before.
    pub fn multi_tap(mut self, count: u16, max_gap: f32) -> Self {
        self.push_condition(BindingCondition::MultiTap { count, max_gap });
        self
    }

    /// Fires every `interval` seconds while the control is held, starting immediately.
    pub fn pulse(mut self, interval: f32) -> Self {
        self.push_condition(BindingCondition::Pulse {
            interval,
            immediate: true,
        });
        self
    }

    /// Requires another control to be held as well.
    ///
    /// This is how `Ctrl+S` is spelled: bind the action to `S`, and add `Ctrl` with this. Call it
    /// more than once for a longer chord.
    ///
    /// ```ignore
    /// context.bind::<Save>(KeyCode::KeyS).with(KeyCode::ControlLeft);
    /// context.bind::<SaveAs>(KeyCode::KeyS).with(KeyCode::ControlLeft).with(KeyCode::ShiftLeft);
    /// ```
    ///
    /// **A longer chord wins.** When several bindings read the same control, the one requiring the
    /// most held alongside it takes the control and the shorter ones do not fire — so `Ctrl+S` does
    /// not also trigger a plain `S` binding, and `Ctrl+Shift+S` does not trigger either of the
    /// other two. Nothing has to be declared for that; it follows from the lengths.
    #[cfg(any(feature = "keyboard", feature = "gamepad"))]
    pub fn with(self, control: impl Into<ButtonControl>) -> Self {
        self.builder.bindings[self.index].chord.push(control.into());
        self
    }

    /// Takes this binding's controls, so that lower-priority contexts do not see them.
    ///
    /// Opt-in per binding rather than per context, because a context usually wants to claim only
    /// some of what it reads: a menu should take `Escape` from the game behind it, while a global
    /// screenshot key on `F12` goes on working whatever is on screen.
    ///
    /// The claim lasts for the rest of the frame, and reaches only contexts that evaluate after
    /// this one — which means later in priority order, and never backwards across a tick domain.
    /// A control is claimed only on the ticks the binding actually fires.
    pub fn consume(self) -> Self {
        self.builder.bindings[self.index].consume = true;
        self
    }

    /// Adds an application-defined condition.
    pub fn when<K: Condition>(mut self, condition: K) -> Self {
        self.push_condition(BindingCondition::Custom(Box::new(condition)));
        self
    }

    /// Adds a deadzone.
    ///
    /// ```ignore
    /// context.bind::<Move>(Stick::Left).dead_zone(DeadZone::radial(0.15));
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

    /// Reads this control as a rate, and converts it to the movement it caused this tick.
    ///
    /// A stick says how fast; a mouse says how far. They are different quantities, and adding them
    /// is the reason a look control can feel different at different frame rates. This is the
    /// conversion between them: `scale` is how far a fully deflected control should move the action
    /// in one second, and what comes out is the distance covered since the last tick.
    ///
    /// ```ignore
    /// // Both drive the same look action, in the same units.
    /// context.bind::<Look>(MouseMove);
    /// context.bind::<Look>(Stick::Right).dead_zone(DeadZone::radial(0.12)).per_second(180.0);
    /// ```
    ///
    /// A control that already reports a displacement cannot be read as a rate, so this is refused
    /// on one.
    pub fn per_second(mut self, scale: f32) -> Self {
        self.push_modifier(BindingModifier::PerSecond(scale));
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
    // Installed against the `App` once the context has been declared. `None` leaves the context
    // live from the moment an entity carries it; see `active_if`, which lives in `context` because
    // everything it touches does.
    pub(crate) activation: Option<crate::context::Activation>,
    _marker: PhantomData<C>,
}

impl<C> Default for InputContextBuilder<C> {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
            activation: None,
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
            dispatch: dispatch_for::<A>,
            source,
            modifiers: Vec::new(),
            conditions: Vec::new(),
            consume: false,
            #[cfg(any(feature = "keyboard", feature = "gamepad"))]
            chord: Vec::new(),
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
    ///
    /// The control has to be one the action can actually use. A control reports on a channel of a
    /// particular [`ChannelShape`], the action declares an [`Intent`], and a binding between two
    /// that do not fit — a single button asked to give a direction, a mouse asked to hold a
    /// position — is refused when the context is declared. [`Intent::accepts`] has the table.
    ///
    /// ```ignore
    /// context.bind::<Jump>(KeyCode::Space);
    /// context.bind::<Jump>(GamepadButton::South);
    /// context.bind::<Move>(DirectionalButtons::wasd());
    /// context.bind::<Move>(Stick::Left).dead_zone(DeadZone::radial(0.15));
    /// context.bind::<Look>(MouseMove);
    /// ```
    pub fn bind<A: InputAction>(&mut self, source: impl BindingSourceSpec) -> BindingHandle<'_, C> {
        self.push_binding::<A>(source.into_binding_source())
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
    fn apply(&self, value: ActionValue, _scratch: &mut Scratch, _delta: f32) -> ActionValue {
        Self::apply(self, value, &mut Scratch::default(), 0.0)
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

    struct DummyDelta2;

    impl InputAction for DummyDelta2 {
        type Output = Vec2;

        const INTENT: crate::action::Intent = crate::action::Intent::Delta2;
        const PATH: &'static str = "tests::DummyDelta2";
    }

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
        ];

        for (modifier, input, expected) in cases {
            assert_eq!(
                modifier.apply(input, &mut Scratch::default(), 0.0),
                expected
            );
        }
    }

    #[test]
    fn custom_modifiers_fit_into_the_chain() {
        let modifier = BindingModifier::Custom(Box::new(DoubleAxis));

        assert_eq!(
            modifier.apply(
                ActionValue::Axis2(Vec2::new(1.0, -2.0)),
                &mut Scratch::default(),
                0.0
            ),
            ActionValue::Axis2(Vec2::new(2.0, -4.0))
        );
    }

    #[cfg(feature = "keyboard")]
    #[test]
    fn binding_builders_collect_modifiers_in_order() {
        let mut builder = InputContextBuilder::<()>::default();
        builder
            .bind::<DummyButton>(KeyCode::Space)
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

    /// The two cases R2.10 names: a trigger that is button-shaped despite carrying a fraction, and
    /// a directional composite that is direction-shaped despite being made of buttons.
    #[test]
    fn a_source_reports_the_channel_it_arrives_on() {
        #[cfg(feature = "keyboard")]
        {
            assert_eq!(
                BindingSource::Button(KeyCode::Space).channel_shape(),
                ChannelShape::Button
            );
            assert_eq!(
                BindingSource::Directional2(DirectionalButtons::wasd()).channel_shape(),
                ChannelShape::Axis2
            );
        }

        assert_eq!(
            BindingSource::MouseMotion.channel_shape(),
            ChannelShape::Delta2
        );

        #[cfg(feature = "gamepad")]
        {
            assert_eq!(
                BindingSource::GamepadButton(GamepadButton::LeftTrigger2).channel_shape(),
                ChannelShape::Button
            );
            assert_eq!(
                BindingSource::GamepadAxis(GamepadAxis::RightStickX).channel_shape(),
                ChannelShape::Axis1
            );
            assert_eq!(
                BindingSource::GamepadStick(Stick::Left).channel_shape(),
                ChannelShape::Axis2
            );
        }
    }

    #[test]
    fn a_reading_between_the_thresholds_keeps_what_it_had() {
        let threshold = ButtonThreshold::default();

        // Outside the band the previous state does not matter.
        assert!(threshold.pressed(0.9, false));
        assert!(!threshold.pressed(0.1, true));

        // Inside it, nothing else does.
        assert!(threshold.pressed(0.5, true));
        assert!(!threshold.pressed(0.5, false));

        // The two edges belong to the states they name, so a reading exactly on one settles it.
        assert!(threshold.pressed(threshold.press, false));
        assert!(!threshold.pressed(threshold.release, true));
    }

    /// A composite's parts are controls, not keys, so nothing stops them coming from two devices.
    #[cfg(all(feature = "keyboard", feature = "gamepad"))]
    #[test]
    fn composite_parts_are_not_tied_to_one_device() {
        let mixed = DirectionalButtons::new(
            KeyCode::KeyW,
            GamepadButton::DPadDown,
            KeyCode::KeyA,
            GamepadButton::DPadRight,
        );

        assert_eq!(mixed.up, ButtonControl::Key(KeyCode::KeyW));
        assert_eq!(
            mixed.down,
            ButtonControl::GamepadButton(GamepadButton::DPadDown)
        );
        assert_eq!(
            DirectionalButtons::dpad().up,
            ButtonControl::GamepadButton(GamepadButton::DPadUp)
        );
        assert_eq!(
            DirectionalButtons::wasd().up,
            ButtonControl::Key(KeyCode::KeyW)
        );
    }

    /// The case chunk 15 exists for: an analog action driven by a control that arrives on a button
    /// channel. Nothing about the binding is special, which is the point.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_trigger_can_drive_an_analog_action() {
        struct Thrust;

        impl InputAction for Thrust {
            type Output = f32;

            const INTENT: crate::action::Intent = crate::action::Intent::Analog1;
            const PATH: &'static str = "tests::Thrust";
        }

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<Thrust>(GamepadButton::LeftTrigger2);
        crate::plan::Plan::<()>::from_bindings(builder.finish());
    }

    #[cfg(feature = "keyboard")]
    #[test]
    #[should_panic(expected = "carries no direction")]
    fn a_lone_button_cannot_drive_a_directional_action() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyVec2>(KeyCode::Space);
        crate::plan::Plan::<()>::from_bindings(builder.finish());
    }

    #[cfg(feature = "gamepad")]
    #[test]
    #[should_panic(expected = "displacement that has already happened")]
    fn a_stick_cannot_stand_in_for_a_delta() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyDelta2>(Stick::Right);
        crate::plan::Plan::<()>::from_bindings(builder.finish());
    }

    #[test]
    #[should_panic(expected = "displacement that has already happened")]
    fn mouse_motion_cannot_stand_in_for_a_direction() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyVec2>(MouseMove);
        crate::plan::Plan::<()>::from_bindings(builder.finish());
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn gamepad_source_values_bind_through_the_same_pipeline() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyButton>(GamepadButton::South);
        builder.bind::<DummyVec2>(Stick::Left);

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
        assert!(!BindingModifier::Custom(Box::new(DoubleAxis)).rescales());
    }
}
