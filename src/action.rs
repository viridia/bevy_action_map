//! Action types carry the value shape and meaning for one gameplay action.
//!
//! Declare an action as a Rust type, choose the value shape it returns, and give it an intent so
//! bindings can find controls that produce a compatible value.
//!
//! Give the action a stable, user-chosen path with `#[action(path = "...")]`. That string is
//! what settings files and other serialized data store.
//!
//! ```rust
//! use bevy_action_map::prelude::*;
//!
//! #[derive(InputAction)]
//! #[action(path = "gameplay.jump", output = bool, intent = Button)]
//! struct Jump;
//!
//! assert_eq!(Jump::INTENT, Intent::Button);
//! ```
//!
//! # Choosing a path
//!
//! The path is required rather than derived from the Rust type. It is a key in your players'
//! saved settings, and stays put when you rename a struct or move it to a different module.
//!
//! Write it as `namespace.name`, all lowercase, with `snake_case` segments separated by dots:
//!
//! ```text
//! gameplay.jump          menu.confirm           vehicle.flight.throttle
//! ```
//!
//! Use at least two segments. The first names the area the action belongs to — `gameplay`, `menu`,
//! `vehicle` for a game; your crate's own name if you are a library contributing actions other
//! crates will use alongside their own. Add intermediate segments to group as you see fit. Contexts
//! follow the same scheme and share the namespace of the actions they bind.
//!
//! Two consequences to plan for:
//!
//! - **The path is free to differ from the type name, and should stay put when the type moves.**
//!   Renaming `Move` to `MoveOnFoot` or relocating it costs you nothing as long as the path is
//!   unchanged.
//! - **Changing a path is a save-data change, not a refactor.** Every binding a player has
//!   customized is stored against the old string, so plan a migration the same way you would for
//!   any other change to a save format.
//!
//! # Grouping actions
//!
//! `#[action(category = "...")]` says what to file an action under in a rebinding screen, so that
//! the four parts of a movement action appear together rather than scattered among everything else.
//!
//! It is a **localization key**, not display text: this crate never decides what a player reads,
//! and a category that said "Movement" would be one string your translators cannot reach. Give the
//! key the same treatment as a path — chosen deliberately, in the same `namespace.name` style, and
//! stable once anything outside the code refers to it:
//!
//! ```text
//! gameplay.movement      gameplay.combat        menu.navigation
//! ```
//!
//! Actions with no category are ungrouped, which is the right answer for a game that shows no
//! rebinding screen.

use alloc::vec::Vec;
use bevy_math::{Vec2, Vec3};
use bevy_platform::sync::atomic::{AtomicU32, Ordering};
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

    /// Finds an action by the path it declared.
    ///
    /// This is how a name read from a settings file becomes something you can look up. It answers
    /// `None` for a path no action in this build declares, which is what happens to a binding
    /// saved against an action that has since been renamed or removed — worth reporting to the
    /// player rather than discarding in silence.
    ///
    /// Only actions that have been used are registered, which in practice means any action bound
    /// in a context.
    pub fn from_path(path: &str) -> Option<Self> {
        with_registry(|registry| {
            registry
                .entries
                .iter()
                .find(|(info, _)| info.path == path)
                .map(|(_, id)| *id)
        })
    }

    /// Returns what the action declared about itself.
    pub fn info(self) -> Option<ActionInfo> {
        with_registry(|registry| {
            registry
                .entries
                .iter()
                .find(|(_, id)| *id == self)
                .map(|(info, _)| *info)
        })
    }
}

/// Every action registered so far, in the order they were first used.
///
/// For a screen that lists actions it was not compiled against. Note the order is the order they
/// happened to be reached, not a declaration order anyone chose, so sort it before showing it.
pub fn registered_actions() -> alloc::vec::Vec<ActionInfo> {
    with_registry(|registry| registry.entries.iter().map(|(info, _)| *info).collect())
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

/// The kind of channel a control reports on.
///
/// This is a property of the **control**, and it is independent of both the [`Intent`] of any
/// action you bind it to and the shape of that action's value. The three do not have to agree, and
/// on real hardware they frequently do not:
///
/// - An analog trigger arrives on a [`Button`](ChannelShape::Button) channel carrying a fraction,
///   so it can drive an [`Analog1`](Intent::Analog1) action without a special case.
/// - A D-pad arrives as four separate buttons rather than as an axis pair, so it reaches a
///   [`Directional2`](Intent::Directional2) action through the same composite that turns four
///   keyboard keys into a direction.
///
/// You rarely name this type directly. It is what [`Intent::accepts`] consults to decide whether a
/// binding makes sense, and what a rebinding UI filters candidate controls on.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChannelShape {
    /// A single control with a pressed sense and a magnitude in `0.0..=1.0`.
    ///
    /// Keyboard keys, mouse buttons and gamepad buttons all report here. So do analog triggers,
    /// which is why the magnitude is a fraction rather than a flag.
    Button,
    /// A single bipolar axis resting at zero, in `-1.0..=1.0`.
    Axis1,
    /// A pair of bipolar axes reporting a position, such as a stick.
    ///
    /// A position implies a rate: how far the control is pushed says how fast something should
    /// happen, and it keeps saying so for as long as it is held.
    Axis2,
    /// A relative displacement that has already happened, such as mouse motion.
    ///
    /// Unlike [`Axis2`](ChannelShape::Axis2) this has no resting position and no bound. It is a
    /// measurement of movement rather than a description of it, which is why it must not be
    /// multiplied by a frame time and why it cannot stand in for a position.
    Delta2,
}

impl Intent {
    /// Returns whether this intent is a match for the given output shape.
    pub const fn supports_output<O: ActionOutput>(self) -> bool {
        self.is_one_of(O::INTENTS)
    }

    /// Returns whether this intent appears in a list of them.
    ///
    /// This is a `const fn` because the derive calls it in a compile-time assertion, which is what
    /// turns a mismatched `output` and `intent` into a build error rather than a surprise at run
    /// time.
    pub const fn is_one_of(self, intents: &[Intent]) -> bool {
        let mut index = 0;
        while index < intents.len() {
            if intents[index] as u8 == self as u8 {
                return true;
            }
            index += 1;
        }
        false
    }

    /// Returns whether a control reporting on `shape` can serve this intent.
    ///
    /// Binding an action to a control it cannot serve is refused when the context is declared,
    /// rather than silently producing a value that means something else:
    ///
    /// |                | `Button` | `Axis1` | `Axis2` | `Delta2` |
    /// | -------------- | -------- | ------- | ------- | -------- |
    /// | `Button`       | yes      | yes     | yes     | no       |
    /// | `Analog1`      | yes      | yes     | yes     | no       |
    /// | `Directional2` | no       | no      | yes     | no       |
    /// | `Delta2`       | no       | no      | no      | yes      |
    ///
    /// The two rows that refuse things are the ones worth understanding.
    ///
    /// A single button carries no direction, so it cannot drive a `Directional2` action on its own.
    /// Bind a directional composite instead — four buttons, named for the directions they push —
    /// and the same composite accepts keyboard keys and a D-pad interchangeably.
    ///
    /// A `Delta2` action accepts nothing but a delta, and a delta drives nothing else. A stick
    /// reports how fast you want to turn and a mouse reports how far you have already turned;
    /// adding those together is a units error, and one that shows up as a look speed that changes
    /// with the frame rate. Driving one action from both devices is the normal thing to want, and
    /// it needs the conversion between them written down rather than assumed.
    pub const fn accepts(self, shape: ChannelShape) -> bool {
        match (self, shape) {
            // A press is a press however it arrives; an axis presses by crossing a threshold.
            (Intent::Button, ChannelShape::Button | ChannelShape::Axis1 | ChannelShape::Axis2) => {
                true
            }
            // A button is an analog control with two positions, which is why a key can stand in
            // for a trigger.
            (Intent::Analog1, ChannelShape::Button | ChannelShape::Axis1 | ChannelShape::Axis2) => {
                true
            }
            (Intent::Directional2, ChannelShape::Axis2) => true,
            (Intent::Delta2, ChannelShape::Delta2) => true,
            _ => false,
        }
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

/// The current phase of one action.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Phase {
    /// The action is inactive.
    #[default]
    Idle,
    /// A condition on this action began this tick, but has not been satisfied yet.
    ///
    /// A hold that the player has just pressed is `Started`: something is happening, and it is not
    /// yet a jump. If they let go too soon it becomes [`Canceled`](Phase::Canceled) instead of
    /// [`Fired`](Phase::Fired).
    Started,
    /// The action is continuing to do whatever it was doing last tick.
    ///
    /// This covers both a held action still firing and a condition still building toward firing.
    /// The action's **value** tells them apart: an action that is firing has one, and one still
    /// building is at rest.
    Ongoing,
    /// The action became active this tick.
    Fired,
    /// The action ended this tick after being active.
    Completed,
    /// The action was abandoned before it ever fired.
    ///
    /// A hold released early, or a context deactivating mid-press. The distinction from
    /// [`Completed`](Phase::Completed) is whether the action ever actually happened.
    Canceled,
}

/// Working memory for one condition or one stateful modifier.
///
/// A binding that has to remember something between ticks — how long a button has been down, how
/// many taps have landed, what the last value was — keeps it here. One of these belongs to each
/// condition and each stateful modifier, so two conditions on one binding cannot tread on each
/// other.
///
/// This is one fixed shape rather than a type per condition. A hold's duration and a
/// multi-tap's window do not change from tick to tick, so they live in the compiled plan and
/// never appear here. What is left is uniform and `Copy`, which lets a whole context's state be
/// snapshotted by copying two slices.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scratch {
    /// The previous input value, or an accumulator for a filtering modifier.
    pub prev: ActionValue,
    /// When something started, in the context's own seconds.
    pub time: f32,
    /// A tap count, or how far through a sequence this binding has come.
    pub count: u16,
    /// Condition-defined bits.
    pub flags: u8,
}

impl Default for Scratch {
    fn default() -> Self {
        Self {
            prev: ActionValue::Bool(false),
            time: 0.0,
            count: 0,
            flags: 0,
        }
    }
}

/// The state we keep for one action inside a context.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionState {
    /// The current value for the action.
    pub value: ActionValue,
    /// The current phase for the action.
    pub phase: Phase,
}

impl Default for ActionState {
    fn default() -> Self {
        Self {
            value: ActionValue::Bool(false),
            phase: Phase::Idle,
        }
    }
}

impl ActionState {
    /// Creates a state from a value and phase.
    pub const fn new(value: ActionValue, phase: Phase) -> Self {
        Self { value, phase }
    }
}

impl ActionValue {
    /// Converts this value into a typed output.
    ///
    /// Use this when you already have a runtime action value and want a typed result. Every shape
    /// converts to every other, per the rules on [`ActionOutput`].
    pub fn into_output<O: ActionOutput>(self) -> O {
        O::from_action_value(self)
    }

    /// Wraps a typed output as a runtime action value.
    pub fn from_output<O: ActionOutput>(output: O) -> Self {
        output.into_action_value()
    }

    /// Reads this value as a press.
    pub fn to_bool(self) -> bool {
        match self {
            Self::Bool(value) => value,
            Self::Axis1(value) => value != 0.0,
            Self::Axis2(value) => value != Vec2::ZERO,
            Self::Axis3(value) => value != Vec3::ZERO,
        }
    }

    /// Reads this value as a single number.
    pub fn to_axis1(self) -> f32 {
        match self {
            Self::Bool(value) => f32::from(u8::from(value)),
            Self::Axis1(value) => value,
            // Magnitude rather than one component: dropping an axis of a stick would silently
            // discard half of what the player did, whereas its length is a fair answer to "how
            // far". The cost is the sign, which is why a signed 1D reading wants a single axis as
            // its source rather than a whole stick.
            Self::Axis2(value) => value.length(),
            Self::Axis3(value) => value.length(),
        }
    }

    /// Reads this value as a 2D vector.
    pub fn to_axis2(self) -> Vec2 {
        match self {
            Self::Bool(value) => Vec2::new(f32::from(u8::from(value)), 0.0),
            Self::Axis1(value) => Vec2::new(value, 0.0),
            Self::Axis2(value) => value,
            Self::Axis3(value) => value.truncate(),
        }
    }

    /// Reads this value as a 3D vector.
    pub fn to_axis3(self) -> Vec3 {
        match self {
            Self::Bool(value) => Vec3::new(f32::from(u8::from(value)), 0.0, 0.0),
            Self::Axis1(value) => Vec3::new(value, 0.0, 0.0),
            Self::Axis2(value) => value.extend(0.0),
            Self::Axis3(value) => value,
        }
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
///
/// # Converting between shapes
///
/// Every shape converts to every other, so reading an action never fails on a shape mismatch. Two
/// rules cover the whole table, and both exist to avoid inventing information that was not there:
///
/// - **Widening** puts the value in the first component and leaves the rest at zero. A trigger at
///   40% read as a 2D value is `(0.4, 0.0)` and not `(0.4, 0.4)` — a control pulled part way is
///   not a diagonal.
/// - **Narrowing to a single number or a press measures the whole value**, so a 2D value becomes
///   its length, and a press is that length being non-zero. Narrowing between vector shapes drops
///   the trailing components instead, since those are named and the one being dropped is known.
///
/// You will not normally hit these, because an action stores the shape its intent calls for. They
/// matter when you read a value in a different shape than the one it was written in.
pub trait ActionOutput: Copy + Send + Sync + 'static {
    /// Intents that can consume this output shape.
    const INTENTS: &'static [Intent];

    /// The value this shape has when nobody is touching the control.
    const REST: Self;

    /// Converts the typed value into the runtime representation.
    fn into_action_value(self) -> ActionValue;

    /// Converts the runtime representation into this shape, per the rules above.
    fn from_action_value(value: ActionValue) -> Self;
}

/// A declared gameplay context.
///
/// Derive this on a unit struct and choose the tick domain where it should be evaluated.
///
/// Give the context a stable, user-chosen path with `#[context(path = "...")]`.
///
/// The derive also makes the type a `Component`, because a context is something an entity carries
/// — put it on the player, on one entity per local player, or on an entity of its own — and it can
/// be spawned from a scene like any other component. Write the `InputContext` impl by hand if you
/// need to configure the component differently; it is three associated constants.
///
/// ```rust
/// use bevy_action_map::prelude::*;
///
/// #[derive(InputContext)]
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

    /// Whether this context, while active, treats every lower-priority context as inactive.
    ///
    /// A settings screen wants this: it should not have to name every action `Flying` and `Shell`
    /// bind just to keep them from answering while it is up. A context above it in priority is
    /// untouched either way, which is how a global hotkey survives without an opt-out list.
    const EXCLUSIVE: bool = false;

    /// Stable path used to identify the context across runs.
    const PATH: &'static str;
}

impl ActionOutput for bool {
    const INTENTS: &'static [Intent] = &[Intent::Button];
    const REST: Self = false;

    fn into_action_value(self) -> ActionValue {
        ActionValue::Bool(self)
    }

    fn from_action_value(value: ActionValue) -> Self {
        value.to_bool()
    }
}

impl ActionOutput for f32 {
    const INTENTS: &'static [Intent] = &[Intent::Button, Intent::Analog1];
    const REST: Self = 0.0;

    fn into_action_value(self) -> ActionValue {
        ActionValue::Axis1(self)
    }

    fn from_action_value(value: ActionValue) -> Self {
        value.to_axis1()
    }
}

impl ActionOutput for Vec2 {
    const INTENTS: &'static [Intent] = &[Intent::Directional2, Intent::Delta2];
    const REST: Self = Vec2::ZERO;

    fn into_action_value(self) -> ActionValue {
        ActionValue::Axis2(self)
    }

    fn from_action_value(value: ActionValue) -> Self {
        value.to_axis2()
    }
}

impl ActionOutput for Vec3 {
    const INTENTS: &'static [Intent] = &[Intent::Directional2, Intent::Delta2];
    const REST: Self = Vec3::ZERO;

    fn into_action_value(self) -> ActionValue {
        ActionValue::Axis3(self)
    }

    fn from_action_value(value: ActionValue) -> Self {
        value.to_axis3()
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

    /// What to group this action under in a rebinding screen.
    ///
    /// A localization key rather than display text, on the same terms as the action's path: it is
    /// a name that appears outside your code, so it should survive a refactor. Actions that share a
    /// category are shown together — the four parts of a movement action, say, under "Movement".
    ///
    /// Actions in the same group should give the same key. It lives on the action rather than on
    /// each binding, so there is one place to change it and no way for two bindings to disagree.
    const CATEGORY: Option<&'static str> = None;

    /// Whether bindings of this action take their controls away from lower-priority contexts.
    ///
    /// A menu's `Back` action wants this: while the menu is up, the control it uses should not also
    /// reach the game behind it. A global screenshot key does not, since nothing should stop it
    /// working. Bindings inherit this and can say otherwise one at a time.
    const CONSUMES: bool = false;

    /// Returns the registered id for this action type.
    ///
    /// The derive overrides this with a cached version, so reading an action costs an atomic load
    /// rather than a registry lookup. A hand-written impl gets the uncached path unless it does the
    /// same — see [`ActionIdCache`].
    fn id() -> ActionId {
        intern_action(ActionInfo::of::<Self>())
    }
}

/// What is known about one action without naming its type.
///
/// A rebinding screen groups rows by category and labels them from the path; a settings file names
/// an action by path and has to find it again. Both need this, and neither can name the Rust type
/// that declared it.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActionInfo {
    /// The declared path, which is the action's identity everywhere outside the code.
    pub path: &'static str,
    /// What to group it under in a rebinding screen, if the game says.
    pub category: Option<&'static str>,
    /// The kind of control that should drive it.
    pub intent: Intent,
}

impl ActionInfo {
    /// Reads the metadata an action type declares.
    pub const fn of<A: InputAction + ?Sized>() -> Self {
        Self {
            path: A::PATH,
            category: A::CATEGORY,
            intent: A::INTENT,
        }
    }
}

#[derive(Default)]
struct ActionRegistry {
    next_id: u32,
    entries: Vec<(ActionInfo, ActionId)>,
}

static ACTION_REGISTRY: bevy_platform::sync::OnceLock<bevy_platform::sync::Mutex<ActionRegistry>> =
    bevy_platform::sync::OnceLock::new();

/// The index [`ActionIdCache`] uses to mean "not resolved yet", and therefore never a real id.
const UNRESOLVED: u32 = u32::MAX;

fn intern_action(info: ActionInfo) -> ActionId {
    let mut registry = ACTION_REGISTRY
        .get_or_init(|| bevy_platform::sync::Mutex::new(ActionRegistry::default()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());

    if let Some((_, id)) = registry
        .entries
        .iter()
        .find(|(registered, _)| registered.path == info.path)
    {
        return *id;
    }

    let index = registry.next_id;
    assert!(index < UNRESOLVED, "action registry exhausted u32 ids");
    registry.next_id = index + 1;
    let id = ActionId(index);
    registry.entries.push((info, id));
    id
}

/// Reads the registry, which is global and behind a lock.
fn with_registry<T>(read: impl FnOnce(&ActionRegistry) -> T) -> T {
    let registry = ACTION_REGISTRY
        .get_or_init(|| bevy_platform::sync::Mutex::new(ActionRegistry::default()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    read(&registry)
}

/// Remembers the [`ActionId`] for one action so it is resolved once rather than on every read.
///
/// The derive generates one of these per action and you will not normally name it. Use it only
/// if you are writing an `InputAction` impl by hand and want reads to cost the same as a derived
/// one:
///
/// ```rust
/// use bevy_action_map::action::{ActionId, ActionIdCache, InputAction, Intent};
///
/// struct Jump;
///
/// impl InputAction for Jump {
///     type Output = bool;
///     const INTENT: Intent = Intent::Button;
///     const PATH: &'static str = "gameplay.jump";
///
///     fn id() -> ActionId {
///         static ID: ActionIdCache = ActionIdCache::new();
///         ID.get_or_intern::<Self>()
///     }
/// }
/// ```
pub struct ActionIdCache(AtomicU32);

impl ActionIdCache {
    /// Creates an empty cache, to be stored in a `static` alongside one action's impl.
    pub const fn new() -> Self {
        Self(AtomicU32::new(UNRESOLVED))
    }

    /// Returns the id for an action, registering it the first time and reusing it after.
    pub fn get_or_intern<A: InputAction + ?Sized>(&self) -> ActionId {
        // Relaxed is enough: nothing is published through this value. It is either the sentinel or
        // the right id, and two threads racing both resolve the same path to the same number, so
        // the worst case is interning once more than necessary.
        match self.0.load(Ordering::Relaxed) {
            UNRESOLVED => {
                let id = intern_action(ActionInfo::of::<A>());
                self.0.store(id.index(), Ordering::Relaxed);
                id
            }
            index => ActionId(index),
        }
    }
}

impl Default for ActionIdCache {
    fn default() -> Self {
        Self::new()
    }
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

    /// The whole table, spelled out. It is the crate's answer to "may I bind this to that", so a
    /// change to any cell should have to be made here as well as in the code.
    #[test]
    fn every_intent_and_channel_pair_is_decided() {
        use ChannelShape::{Axis1, Axis2, Button as ButtonChannel, Delta2 as Delta2Channel};
        use Intent::{Analog1, Button, Delta2, Directional2};

        let expected = [
            //                     Button  Axis1  Axis2  Delta2
            (Button, [true, true, true, false]),
            (Analog1, [true, true, true, false]),
            (Directional2, [false, false, true, false]),
            (Delta2, [false, false, false, true]),
        ];

        for (intent, row) in expected {
            for (shape, accepted) in [ButtonChannel, Axis1, Axis2, Delta2Channel]
                .into_iter()
                .zip(row)
            {
                assert_eq!(
                    intent.accepts(shape),
                    accepted,
                    "{intent:?} against a {shape:?} channel"
                );
            }
        }
    }

    #[test]
    fn action_ids_are_interned_by_path() {
        assert_eq!(Jump::id(), Jump::id());
        assert_ne!(Jump::id(), Look::id());
        assert_eq!(Jump::id().index(), Jump::id().index());
    }

    #[test]
    fn a_cached_id_agrees_with_the_registry_and_stays_put() {
        static ID: ActionIdCache = ActionIdCache::new();

        // The first call resolves through the registry; later ones must not disagree with it.
        let first = ID.get_or_intern::<Jump>();
        assert_eq!(first, intern_action(ActionInfo::of::<Jump>()));
        assert_eq!(first, ID.get_or_intern::<Jump>());
        assert_eq!(first, Jump::id());
    }

    #[test]
    fn separate_caches_for_separate_paths_do_not_collide() {
        static FIRST: ActionIdCache = ActionIdCache::new();
        static SECOND: ActionIdCache = ActionIdCache::new();

        assert_ne!(
            FIRST.get_or_intern::<Jump>(),
            SECOND.get_or_intern::<Look>()
        );
    }

    #[test]
    fn a_derived_action_caches_its_id() {
        #[derive(crate::InputAction)]
        #[action(path = "tests.derived_cache", output = bool, intent = Button)]
        struct Derived;

        // Whatever the derive generated has to agree with the registry, or a plan compiled from
        // one and read through the other would silently address the wrong slot.
        assert_eq!(Derived::id(), intern_action(ActionInfo::of::<Derived>()));
        assert_eq!(Derived::id(), Derived::id());
    }

    /// What a rebinding screen needs and cannot get from the type: the group to file an action
    /// under, and the way back from a name in a settings file to the action it meant.
    #[test]
    fn an_action_registers_what_it_declared_about_itself() {
        #[derive(crate::InputAction)]
        #[action(
            path = "tests.registered_move",
            output = Vec2,
            intent = Directional2,
            category = "tests.movement"
        )]
        struct RegisteredMove;

        assert_eq!(RegisteredMove::CATEGORY, Some("tests.movement"));
        const { assert!(!RegisteredMove::CONSUMES, "not asked for, so not on") };

        // Nothing is registered until the action is used, which binding it does.
        let id = RegisteredMove::id();
        assert_eq!(ActionId::from_path("tests.registered_move"), Some(id));

        let info = id.info().expect("a registered action knows itself");
        assert_eq!(info.path, "tests.registered_move");
        assert_eq!(info.category, Some("tests.movement"));
        assert_eq!(info.intent, Intent::Directional2);

        assert!(registered_actions().contains(&info));
    }

    /// A path nobody declared is the shape of a binding saved against an action that has since been
    /// renamed. Answering `None` is what lets that be reported rather than mistaken for something.
    #[test]
    fn an_unknown_path_resolves_to_nothing() {
        assert_eq!(ActionId::from_path("tests.no_such_action_anywhere"), None);
    }

    /// Declared once on the action rather than repeated on each of its bindings, because the
    /// bindings of one action should not be able to disagree about it by accident.
    #[test]
    fn an_action_can_ask_that_its_bindings_consume() {
        #[derive(crate::InputAction)]
        #[action(path = "tests.consuming", output = bool, intent = Button, consume)]
        struct Consuming;

        const { assert!(Consuming::CONSUMES) };
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

    /// Every cell of the shape conversion table, written out, so a change to any cell has to be
    /// made here as well as in the code.
    #[test]
    fn every_shape_converts_to_every_other() {
        // Chosen so that no two answers coincide: 3-4-5 and 1-2-2 have integral lengths, and 0.4
        // is distinguishable from both 0 and 1.
        let cases = [
            (
                ActionValue::Bool(true),
                true,
                1.0,
                Vec2::new(1.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
            ),
            (
                ActionValue::Axis1(0.4),
                true,
                0.4,
                Vec2::new(0.4, 0.0),
                Vec3::new(0.4, 0.0, 0.0),
            ),
            (
                ActionValue::Axis2(Vec2::new(3.0, 4.0)),
                true,
                5.0,
                Vec2::new(3.0, 4.0),
                Vec3::new(3.0, 4.0, 0.0),
            ),
            (
                ActionValue::Axis3(Vec3::new(1.0, 2.0, 2.0)),
                true,
                3.0,
                Vec2::new(1.0, 2.0),
                Vec3::new(1.0, 2.0, 2.0),
            ),
        ];

        for (value, as_bool, as_axis1, as_axis2, as_axis3) in cases {
            assert_eq!(value.to_bool(), as_bool, "{value:?} as bool");
            assert_eq!(value.to_axis1(), as_axis1, "{value:?} as f32");
            assert_eq!(value.to_axis2(), as_axis2, "{value:?} as Vec2");
            assert_eq!(value.to_axis3(), as_axis3, "{value:?} as Vec3");
        }

        // Rest reads as rest in every shape.
        for zero in [
            ActionValue::Bool(false),
            ActionValue::Axis1(0.0),
            ActionValue::Axis2(Vec2::ZERO),
            ActionValue::Axis3(Vec3::ZERO),
        ] {
            assert!(!zero.to_bool(), "{zero:?} as bool");
            assert_eq!(zero.to_axis1(), 0.0, "{zero:?} as f32");
            assert_eq!(zero.to_axis2(), Vec2::ZERO, "{zero:?} as Vec2");
            assert_eq!(zero.to_axis3(), Vec3::ZERO, "{zero:?} as Vec3");
        }
    }

    /// A control pulled part way is not pushed diagonally, and every widening conversion has to
    /// agree about that.
    #[test]
    fn widening_does_not_invent_a_direction() {
        assert_eq!(
            ActionValue::Axis1(0.4).into_output::<Vec2>(),
            Vec2::new(0.4, 0.0)
        );
        assert_eq!(
            ActionValue::Axis1(0.4).into_output::<Vec3>(),
            Vec3::new(0.4, 0.0, 0.0)
        );
        assert_eq!(
            ActionValue::Bool(true).into_output::<Vec2>(),
            Vec2::new(1.0, 0.0)
        );
    }

    #[test]
    fn an_output_shape_admits_only_the_intents_it_can_serve() {
        // A 3D value is a direction or a displacement. It was previously claimed by every intent
        // including `Button`, which would have let a jump action declare itself as a `Vec3`.
        assert!(!Intent::Button.supports_output::<Vec3>());
        assert!(!Intent::Analog1.supports_output::<Vec3>());
        assert!(Intent::Directional2.supports_output::<Vec3>());
        assert!(Intent::Delta2.supports_output::<Vec3>());

        // A press is a press; a number can be either a press or a reading.
        assert!(Intent::Button.supports_output::<bool>());
        assert!(!Intent::Analog1.supports_output::<bool>());
        assert!(Intent::Button.supports_output::<f32>());
        assert!(Intent::Analog1.supports_output::<f32>());
    }
}
