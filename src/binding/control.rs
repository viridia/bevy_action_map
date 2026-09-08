//! The controls a binding reads, and the arrangements of them.

#[cfg(feature = "gamepad")]
use bevy_input::gamepad::{GamepadAxis, GamepadButton};
#[cfg(feature = "keyboard")]
use bevy_input::keyboard::KeyCode;
#[cfg(feature = "mouse")]
use bevy_input::mouse::MouseButton;

use crate::action::ChannelShape;

/// A control that reports on a button channel.
///
/// This is what the parts of a [`DirectionalButtons`] composite are made of. A keyboard key and a
/// D-pad button are the same kind of thing here: both report pressed or not, which is what lets
/// one composite serve either.
///
/// You seldom write this type. Anywhere a part is wanted, the control itself will do:
/// `DirectionalButtons::new(KeyCode::KeyW, ..)` and `DirectionalButtons::new(GamepadButton::DPadUp,
/// ..)` both convert on the way in.
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonControl {
    /// A keyboard key, by physical position rather than by the character the layout prints on
    /// it.
    #[cfg(feature = "keyboard")]
    PhysicalKey(KeyCode),
    /// A mouse button.
    #[cfg(feature = "mouse")]
    MouseButton(MouseButton),
    /// A gamepad button, including the D-pad and the triggers.
    #[cfg(feature = "gamepad")]
    GamepadButton(GamepadButton),
}

#[cfg(feature = "keyboard")]
impl From<KeyCode> for ButtonControl {
    fn from(key: KeyCode) -> Self {
        Self::PhysicalKey(key)
    }
}

#[cfg(feature = "mouse")]
impl From<MouseButton> for ButtonControl {
    fn from(button: MouseButton) -> Self {
        Self::MouseButton(button)
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
/// composite. Both bindings feed the same action, and the player may use either.
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AxisButtons {
    /// The button that drives the axis negative.
    pub negative: ButtonControl,
    /// The button that drives it positive.
    pub positive: ButtonControl,
}

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
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
            negative: ButtonControl::PhysicalKey(KeyCode::KeyA),
            positive: ButtonControl::PhysicalKey(KeyCode::KeyD),
        }
    }

    /// The left and right arrow keys.
    #[cfg(feature = "keyboard")]
    pub const fn left_right() -> Self {
        Self {
            negative: ButtonControl::PhysicalKey(KeyCode::ArrowLeft),
            positive: ButtonControl::PhysicalKey(KeyCode::ArrowRight),
        }
    }
}

/// Four buttons that together make a direction.
///
/// A direction never arrives from the hardware as a direction. WASD is four keys and a D-pad is
/// four buttons, since Bevy reports no D-pad axis at all, so both reach a 2D action through this,
/// and through the same code. Whichever a player uses, an action bound this way behaves
/// identically.
///
/// ```ignore
/// context.bind::<Move>(DirectionalButtons::wasd());
/// context.bind::<Move>(DirectionalButtons::dpad());
/// ```
///
/// The parts are named for the direction each one pushes rather than for its position on a device,
/// which is what a rebinding screen needs in order to say "Move Forward" next to one of them.
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
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

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
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
            up: ButtonControl::PhysicalKey(KeyCode::KeyW),
            down: ButtonControl::PhysicalKey(KeyCode::KeyS),
            left: ButtonControl::PhysicalKey(KeyCode::KeyA),
            right: ButtonControl::PhysicalKey(KeyCode::KeyD),
        }
    }

    /// The four arrow keys.
    #[cfg(feature = "keyboard")]
    pub const fn arrow_keys() -> Self {
        Self {
            up: ButtonControl::PhysicalKey(KeyCode::ArrowUp),
            down: ButtonControl::PhysicalKey(KeyCode::ArrowDown),
            left: ButtonControl::PhysicalKey(KeyCode::ArrowLeft),
            right: ButtonControl::PhysicalKey(KeyCode::ArrowRight),
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

/// Mouse motion as a binding input.
///
/// ```ignore
/// context.bind::<Look>(MouseMove);
/// ```
///
/// This reports a displacement that has already happened, so it can only drive an action whose
/// intent is [`Delta2`](crate::action::ActionIntent::Delta2). It is named for the movement rather
/// than for the device so that it does not collide with Bevy's own `MouseMotion` message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseMove;

/// Which part of a binding's input a control is.
///
/// A single control is the [`Whole`](BindingPart::Whole) of its binding. A composite has parts,
/// named for what each one does rather than for where it sits: the four keys of a directional
/// composite are up, down, left and right whichever keys they happen to be.
///
/// This is what a rebinding screen addresses. A player rebinds "move forward", which is one part of
/// a movement binding — never the movement binding itself, which has no single control to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BindingPart {
    /// The binding reads one control, and this is it.
    Whole,
    /// The half of a two-button axis that drives it negative.
    Negative,
    /// The half that drives it positive.
    Positive,
    /// The part of a directional composite that pushes up.
    Up,
    /// The part that pushes down.
    Down,
    /// The part that pushes left.
    Left,
    /// The part that pushes right.
    Right,
}

impl BindingPart {
    /// The name this part contributes to a mapping key, or `None` for a whole binding.
    ///
    /// Mapping keys are the action's path plus this — `gameplay.move` plus `up` — so a part naming
    /// itself is what keeps the key derivable rather than declared twice.
    pub const fn name(self) -> Option<&'static str> {
        match self {
            Self::Whole => None,
            Self::Negative => Some("negative"),
            Self::Positive => Some("positive"),
            Self::Up => Some("up"),
            Self::Down => Some("down"),
            Self::Left => Some("left"),
            Self::Right => Some("right"),
        }
    }
}

/// One physical control.
///
/// A binding names an *input*, which may be a control or an arrangement of several — a directional
/// composite is four buttons. This is what those decompose into, and it is the granularity at which
/// one context takes a control from another: a menu claiming the movement keys claims four
/// controls, and a global screenshot key bound to a fifth is unaffected.
///
/// [`GamepadStick`](Self::GamepadStick) is the one exception: a stick is two axes, but nothing
/// binds or rebinds one of them on its own, so it is named here whole, the same way
/// [`MouseMotion`](Self::MouseMotion) already is. It still decomposes to those two axes for
/// consumption — see [`BindingInput::for_each_control`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    /// A keyboard key, by physical position rather than by the character the layout prints on
    /// it.
    #[cfg(feature = "keyboard")]
    PhysicalKey(KeyCode),
    /// A mouse button.
    #[cfg(feature = "mouse")]
    MouseButton(MouseButton),
    /// A gamepad button, including the D-pad and the triggers.
    #[cfg(feature = "gamepad")]
    GamepadButton(GamepadButton),
    /// One axis of a gamepad stick or trigger.
    #[cfg(feature = "gamepad")]
    GamepadAxis(GamepadAxis),
    /// A gamepad stick, read whole rather than as its two axes.
    #[cfg(feature = "gamepad")]
    GamepadStick(Stick),
    /// The mouse being moved.
    MouseMotion,
}

impl Control {
    /// Which set of devices this control belongs to.
    ///
    /// Keyboard and mouse are one family because a player uses them together; a gamepad is another.
    /// Which one a control belongs to is what decides the family a mapping is rebound in.
    pub const fn family(self) -> crate::device::DeviceFamily {
        match self {
            #[cfg(feature = "keyboard")]
            Self::PhysicalKey(_) => crate::device::DeviceFamily::KeyboardMouse,
            #[cfg(feature = "mouse")]
            Self::MouseButton(_) => crate::device::DeviceFamily::KeyboardMouse,
            Self::MouseMotion => crate::device::DeviceFamily::KeyboardMouse,
            #[cfg(feature = "gamepad")]
            Self::GamepadButton(_) | Self::GamepadAxis(_) | Self::GamepadStick(_) => {
                crate::device::DeviceFamily::Gamepad
            }
        }
    }

    /// The kind of channel this one control reports on.
    ///
    /// The counterpart of [`BindingInput::channel_shape`] for a single control rather than an
    /// arrangement of them, and what decides whether a captured control fits the mapping it was
    /// captured for.
    ///
    /// A directional composite is still never one of these: four buttons produce a two-dimensional
    /// reading together, and none of them is it alone. A stick is the exception, read whole rather
    /// than decomposed, on the same terms as [`MouseMotion`](Self::MouseMotion).
    pub const fn shape(self) -> ChannelShape {
        match self {
            #[cfg(feature = "keyboard")]
            Self::PhysicalKey(_) => ChannelShape::Button,
            #[cfg(feature = "mouse")]
            Self::MouseButton(_) => ChannelShape::Button,
            // Including the triggers, which carry a fraction on this channel.
            #[cfg(feature = "gamepad")]
            Self::GamepadButton(_) => ChannelShape::Button,
            #[cfg(feature = "gamepad")]
            Self::GamepadAxis(_) => ChannelShape::Axis1,
            #[cfg(feature = "gamepad")]
            Self::GamepadStick(_) => ChannelShape::Axis2,
            Self::MouseMotion => ChannelShape::Delta2,
        }
    }
}

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
impl From<ButtonControl> for Control {
    fn from(control: ButtonControl) -> Self {
        match control {
            #[cfg(feature = "keyboard")]
            ButtonControl::PhysicalKey(key) => Self::PhysicalKey(key),
            #[cfg(feature = "mouse")]
            ButtonControl::MouseButton(button) => Self::MouseButton(button),
            #[cfg(feature = "gamepad")]
            ButtonControl::GamepadButton(button) => Self::GamepadButton(button),
        }
    }
}

/// The other direction: recovers a [`ButtonControl`] from a [`Control`] that turns out to name a
/// button.
///
/// The error carries nothing: the caller already has the [`Control`] that failed, and there is only
/// one way this can fail — the control names a stick axis or mouse motion, neither of which has a
/// press to report.
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
impl TryFrom<Control> for ButtonControl {
    type Error = ();

    fn try_from(control: Control) -> Result<Self, Self::Error> {
        match control {
            #[cfg(feature = "keyboard")]
            Control::PhysicalKey(key) => Ok(Self::PhysicalKey(key)),
            #[cfg(feature = "mouse")]
            Control::MouseButton(button) => Ok(Self::MouseButton(button)),
            #[cfg(feature = "gamepad")]
            Control::GamepadButton(button) => Ok(Self::GamepadButton(button)),
            _ => Err(()),
        }
    }
}

/// Puts a control in one part of a composite, refusing a control that is not a button — a stick
/// axis or mouse motion has no press to put there.
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
fn set_button(part: &mut ButtonControl, control: Control) -> bool {
    match ButtonControl::try_from(control) {
        Ok(button) => {
            *part = button;
            true
        }
        Err(()) => false,
    }
}

/// The input that reads exactly this one control.
///
/// Every control is an input on its own; the composites are the inputs that are *not* reachable
/// this way, since no single control carries a direction or a signed axis.
impl From<Control> for BindingInput {
    fn from(control: Control) -> Self {
        match control {
            #[cfg(feature = "keyboard")]
            Control::PhysicalKey(key) => Self::Button(key),
            #[cfg(feature = "mouse")]
            Control::MouseButton(button) => Self::MouseButton(button),
            #[cfg(feature = "gamepad")]
            Control::GamepadButton(button) => Self::GamepadButton(button),
            #[cfg(feature = "gamepad")]
            Control::GamepadAxis(axis) => Self::GamepadAxis(axis),
            #[cfg(feature = "gamepad")]
            Control::GamepadStick(stick) => Self::GamepadStick(stick),
            Control::MouseMotion => Self::MouseMotion,
        }
    }
}

/// The binding input used by the first interactive stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingInput {
    /// A keyboard key.
    #[cfg(feature = "keyboard")]
    Button(KeyCode),
    /// A mouse button.
    #[cfg(feature = "mouse")]
    MouseButton(MouseButton),
    /// A two-button signed axis composite.
    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    Axis1(AxisButtons),
    /// A four-button directional composite.
    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
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

impl BindingInput {
    /// Calls `visit` with every physical control this input reads.
    ///
    /// One for a plain control, four for a directional composite, two for a stick — its two axes,
    /// never [`Control::GamepadStick`] itself, so that a plain binding on one axis and a binding on
    /// the whole stick still contest the same control. This is what consumption and chord clashes
    /// are recorded against, so that taking a composite takes its parts rather than an arrangement
    /// nothing else can name.
    ///
    /// Allocation-free, because it runs per binding per tick. Use [`controls`](Self::controls)
    /// where a collection is more convenient than a callback.
    pub fn for_each_control(&self, mut visit: impl FnMut(Control)) {
        match self {
            #[cfg(feature = "keyboard")]
            Self::Button(key) => visit(Control::PhysicalKey(*key)),
            #[cfg(feature = "mouse")]
            Self::MouseButton(button) => visit(Control::MouseButton(*button)),
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
            Self::Axis1(parts) => {
                visit(parts.negative.into());
                visit(parts.positive.into());
            }
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
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

    /// Calls `visit` with every control this input reads, and the part of the input it is.
    ///
    /// A composite's parts are named for the direction each one pushes rather than for their
    /// position, which is what lets a rebinding screen address one of them — "the key that moves
    /// you forward" — without the four being an ordered list somebody has to keep in step.
    pub fn for_each_part(&self, mut visit: impl FnMut(BindingPart, Control)) {
        match self {
            #[cfg(feature = "keyboard")]
            Self::Button(key) => visit(BindingPart::Whole, Control::PhysicalKey(*key)),
            #[cfg(feature = "mouse")]
            Self::MouseButton(button) => visit(BindingPart::Whole, Control::MouseButton(*button)),
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
            Self::Axis1(parts) => {
                visit(BindingPart::Negative, parts.negative.into());
                visit(BindingPart::Positive, parts.positive.into());
            }
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
            Self::Directional2(parts) => {
                visit(BindingPart::Up, parts.up.into());
                visit(BindingPart::Down, parts.down.into());
                visit(BindingPart::Left, parts.left.into());
                visit(BindingPart::Right, parts.right.into());
            }
            // A stick and a mouse have no parts a player would rebind one of. They are one thing
            // as far as the presentation model is concerned, and what they get instead of
            // per-part rebinding is a tunable.
            Self::MouseMotion => visit(BindingPart::Whole, Control::MouseMotion),
            #[cfg(feature = "gamepad")]
            Self::GamepadButton(button) => {
                visit(BindingPart::Whole, Control::GamepadButton(*button))
            }
            #[cfg(feature = "gamepad")]
            Self::GamepadAxis(axis) => visit(BindingPart::Whole, Control::GamepadAxis(*axis)),
            #[cfg(feature = "gamepad")]
            Self::GamepadStick(stick) => visit(BindingPart::Whole, Control::GamepadStick(*stick)),
        }
    }

    /// Puts a different control in one part of this input, which is what a rebind does.
    ///
    /// The exact inverse of [`for_each_part`](Self::for_each_part): a part this input does not have
    /// is refused, and so is a control that cannot serve the part it was offered for. Both refusals
    /// are `false` rather than a panic, because the caller is applying a saved override and a saved
    /// override can say anything.
    ///
    /// **The input's channel shape is invariant.** A whole binding on a key takes another button
    /// and not a stick axis, so applying an override can never turn a plan that compiled into one
    /// that would not — the shape mismatch is caught here even if nothing caught it earlier.
    pub(crate) fn set_part(&mut self, part: BindingPart, control: Control) -> bool {
        match (&mut *self, part) {
            // A whole binding is replaced outright, since the new input is entirely the new
            // control.
            (Self::MouseMotion, BindingPart::Whole) => self.replace_whole(control),
            #[cfg(feature = "keyboard")]
            (Self::Button(_), BindingPart::Whole) => self.replace_whole(control),
            #[cfg(feature = "mouse")]
            (Self::MouseButton(_), BindingPart::Whole) => self.replace_whole(control),
            #[cfg(feature = "gamepad")]
            (
                Self::GamepadButton(_) | Self::GamepadAxis(_) | Self::GamepadStick(_),
                BindingPart::Whole,
            ) => self.replace_whole(control),
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
            (Self::Axis1(parts), BindingPart::Negative) => set_button(&mut parts.negative, control),
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
            (Self::Axis1(parts), BindingPart::Positive) => set_button(&mut parts.positive, control),
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
            (Self::Directional2(parts), BindingPart::Up) => set_button(&mut parts.up, control),
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
            (Self::Directional2(parts), BindingPart::Down) => set_button(&mut parts.down, control),
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
            (Self::Directional2(parts), BindingPart::Left) => set_button(&mut parts.left, control),
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
            (Self::Directional2(parts), BindingPart::Right) => {
                set_button(&mut parts.right, control)
            }
            _ => false,
        }
    }

    /// Swaps a whole-binding input for the one that reads `control`, if the shape survives it.
    fn replace_whole(&mut self, control: Control) -> bool {
        let replacement = Self::from(control);
        if replacement.channel_shape() != self.channel_shape() {
            return false;
        }
        *self = replacement;
        true
    }

    /// Every physical control this input reads, collected.
    pub fn controls(&self) -> alloc::vec::Vec<Control> {
        let mut controls = alloc::vec::Vec::new();
        self.for_each_control(|control| controls.push(control));
        controls
    }

    /// The kind of channel this input reports on.
    pub const fn channel_shape(&self) -> ChannelShape {
        match self {
            #[cfg(feature = "keyboard")]
            Self::Button(_) => ChannelShape::Button,
            #[cfg(feature = "mouse")]
            Self::MouseButton(_) => ChannelShape::Button,
            // Buttons, but an axis and a direction by the time anything binds to them.
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
            Self::Axis1(_) => ChannelShape::Axis1,
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

    /// Which stick this axis is half of, if it is half of one — `None` for a trigger.
    pub const fn containing(axis: GamepadAxis) -> Option<Self> {
        match axis {
            GamepadAxis::LeftStickX | GamepadAxis::LeftStickY => Some(Self::Left),
            GamepadAxis::RightStickX | GamepadAxis::RightStickY => Some(Self::Right),
            _ => None,
        }
    }
}

/// A value that names a control you can bind an action to.
///
/// Implemented for the control types you would use directly — a [`KeyCode`], a
/// [`GamepadButton`], a [`Stick`], [`MouseMove`], a [`DirectionalButtons`] composite — so that
/// [`bind`](crate::binding::InputContextBuilder::bind) accepts any of them.
///
/// Note what this trait does *not* say: which actions the control is good for. A control reports on
/// a channel of a given [`ChannelShape`] and that is all it knows about itself; whether that suits
/// a particular action is decided against the action's
/// [`ActionIntent`](crate::action::ActionIntent) when the context is
/// declared. This is what lets one trigger drive a button action in one game and an analog action
/// in another.
pub trait IntoBindingInput {
    /// Converts this value into the internal binding representation.
    fn into_binding_input(self) -> BindingInput;
}

#[cfg(feature = "keyboard")]
impl IntoBindingInput for KeyCode {
    fn into_binding_input(self) -> BindingInput {
        BindingInput::Button(self)
    }
}

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
impl IntoBindingInput for AxisButtons {
    fn into_binding_input(self) -> BindingInput {
        BindingInput::Axis1(self)
    }
}

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
impl IntoBindingInput for DirectionalButtons {
    fn into_binding_input(self) -> BindingInput {
        BindingInput::Directional2(self)
    }
}

impl IntoBindingInput for MouseMove {
    fn into_binding_input(self) -> BindingInput {
        BindingInput::MouseMotion
    }
}

#[cfg(feature = "mouse")]
impl IntoBindingInput for MouseButton {
    fn into_binding_input(self) -> BindingInput {
        BindingInput::MouseButton(self)
    }
}

#[cfg(feature = "gamepad")]
impl IntoBindingInput for GamepadButton {
    fn into_binding_input(self) -> BindingInput {
        BindingInput::GamepadButton(self)
    }
}

#[cfg(feature = "gamepad")]
impl IntoBindingInput for GamepadAxis {
    fn into_binding_input(self) -> BindingInput {
        BindingInput::GamepadAxis(self)
    }
}

#[cfg(feature = "gamepad")]
impl IntoBindingInput for Stick {
    fn into_binding_input(self) -> BindingInput {
        BindingInput::GamepadStick(self)
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

/// A binding's input, converted to the control [`is_pressed`](crate::eval)-style raw actuation
/// checks read — `None` for anything [`always_reports_bool`] would already have refused, which is
/// every input a shared toggle's pre-pass ever needs to ask about.
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
pub(crate) fn as_button_control(input: &BindingInput) -> Option<ButtonControl> {
    match input {
        #[cfg(feature = "keyboard")]
        BindingInput::Button(key) => Some(ButtonControl::PhysicalKey(*key)),
        #[cfg(feature = "mouse")]
        BindingInput::MouseButton(button) => Some(ButtonControl::MouseButton(*button)),
        #[cfg(feature = "gamepad")]
        BindingInput::GamepadButton(button) => Some(ButtonControl::GamepadButton(*button)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::action::ChannelShape;
    #[cfg(feature = "gamepad")]
    use bevy_input::gamepad::{GamepadAxis, GamepadButton};
    #[cfg(feature = "keyboard")]
    use bevy_input::keyboard::KeyCode;

    #[test]
    fn a_source_reports_the_channel_it_arrives_on() {
        #[cfg(feature = "keyboard")]
        {
            assert_eq!(
                BindingInput::Button(KeyCode::Space).channel_shape(),
                ChannelShape::Button
            );
            assert_eq!(
                BindingInput::Directional2(DirectionalButtons::wasd()).channel_shape(),
                ChannelShape::Axis2
            );
        }

        assert_eq!(
            BindingInput::MouseMotion.channel_shape(),
            ChannelShape::Delta2
        );

        #[cfg(feature = "gamepad")]
        {
            assert_eq!(
                BindingInput::GamepadButton(GamepadButton::LeftTrigger2).channel_shape(),
                ChannelShape::Button
            );
            assert_eq!(
                BindingInput::GamepadAxis(GamepadAxis::RightStickX).channel_shape(),
                ChannelShape::Axis1
            );
            assert_eq!(
                BindingInput::GamepadStick(Stick::Left).channel_shape(),
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
}
