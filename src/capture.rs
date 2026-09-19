//! "Press a control now": reading a control's identity rather than its value.
//!
//! Every other path through this crate turns a control into a *value* — a bool, an axis, a
//! direction — and throws the control away in the process, because a game wants to know that the
//! player jumped rather than which button they jumped with. Rebinding wants exactly the half that
//! gets discarded, so capture reads the input frame directly instead of going through a binding.
//!
//! This lets rebinding work in a game that is not running. A main-menu settings screen has no
//! gameplay contexts spawned and no evaluator stepping, and capture does not care: the frame is
//! filled by the sampler either way.
//!
//! ```ignore
//! // The player activated a table cell on the settings screen. A mapping holds an ordered list of
//! // slots, so `for_slot` says which one this capture is going to fill — `for_mapping` takes the
//! // first, which is the only one a single-column table has.
//! commands.entity(cell).insert(CaptureSession::for_slot(&mapping, column));
//!
//! // …and the crate answers on that same entity, once.
//! commands.entity(cell).observe(|captured: On<ControlCaptured>, world: &World| {
//!     let name = captured.control.fallback_label();
//!     let clashes = conflicts(world, &captured.control.into(), captured.mapping);
//!     // `captured.slot` comes back too, which is where the new control belongs in the row.
//! });
//! ```
//!
//! The session is a component so that "which row is listening" is answered by where the component
//! is, rather than by a screen holding that state beside a global session and keeping the two in
//! step. Put it on whatever entity the answer is useful on — usually the widget that will show it.
//! Removing the component cancels the capture; the crate removes it itself once something is taken.
//!
//! # What capture will not take
//!
//! Three separate refusals, which look alike and are not:
//!
//! - **Shape and family.** A mapping holding a key accepts another key, not a stick axis and not a
//!   gamepad button — the first because the action cannot use it, the second because a rebind is
//!   scoped to one control family, and moving a binding across families would mean moving it to a
//!   different mapping.
//! - **Excluded** ([`excluding`](CaptureSession::excluding)): the screen's own controls, so it
//!   stays operable while listening. Silent: an excluded control is not being refused, it is busy
//!   doing its normal job, which is how the key that cancels a capture gets through to cancel it.
//! - **Reserved** ([`reserved`](crate::binding::BindingBuilder::reserved)): declared on a binding,
//!   global across its family. Loud, because a player who just pressed it meant to bind it and is
//!   owed the reason.

use alloc::vec::Vec;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Component, EntityEvent, Query, Res, ResMut, Resource};
use bevy_ecs::world::World;
#[cfg(feature = "keyboard")]
use bevy_input::ButtonState;
#[cfg(feature = "keyboard")]
use bevy_input::keyboard::KeyboardInput;

use crate::action::ChannelShape;
use crate::binding::{ButtonThreshold, Control};
use crate::device::DeviceFamily;
use crate::frame::{FrameTimestamp, InputFrame, RawEvent};
use crate::mapping::{ActionMapping, BoundSlot, MappingKey};
use crate::overrides::{Override, Overrides};

/// How far a stick or trigger must be pushed before capture treats it as a choice.
///
/// A stick at rest is not quite at rest, and a capture that took the first non-zero reading would
/// bind whichever axis the hardware happened to be drifting on. Half deflection is well past any
/// resting jitter and well short of what a player has to strain for.
pub const DEFLECTION: f32 = 0.5;

/// How far the mouse must move in one event before capture treats it as a choice.
///
/// Same reason as [`DEFLECTION`], for a device with no resting position: a hand on the desk moves
/// the mouse a pixel at a time without anybody choosing anything.
pub const MOUSE_MOTION: f32 = 8.0;

/// A set of controls named by what its members are, rather than by listing them.
///
/// This is the language capture filters in. A class is defined by the channel a control reports
/// on, never by an enumeration of `KeyCode` and `GamepadButton` variants, which lets a device kind
/// that does not exist yet join a class the day its backend ships, rather than needing to be added
/// to a list here.
///
/// The set of classes is closed: a class earns its place only where writing the members out is not
/// reasonable. "Any button-shaped control" qualifies because the device set is open. "The arrow
/// keys" does not — there are four of them, and naming them is clearer.
///
/// A directional composite is still not a member of any class here: it is four buttons, and a
/// player rebinds one of them at a time. A stick is the one two-dimensional reading a single
/// control produces, on the same terms as the mouse's [`AnyDelta`](Self::AnyDelta).
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ControlClass {
    /// Anything with a pressed sense: keyboard keys, gamepad buttons, and analog triggers, which
    /// report a fraction on the same channel.
    AnyButton,
    /// Any single bipolar axis, such as one half of a stick.
    AnyAxis,
    /// A gamepad stick, read as the whole two-axis position it reports rather than as one axis.
    AnyStick,
    /// Anything reporting a displacement that has already happened, such as the mouse.
    AnyDelta,
}

impl ControlClass {
    /// Whether this control is a member.
    pub const fn contains(self, control: Control) -> bool {
        matches!(
            (self, control.shape()),
            (Self::AnyButton, ChannelShape::Button)
                | (Self::AnyAxis, ChannelShape::Axis1)
                | (Self::AnyStick, ChannelShape::Axis2)
                | (Self::AnyDelta, ChannelShape::Delta2)
        )
    }

    /// The class of controls that can fill a mapping expecting this channel.
    pub const fn of(shape: ChannelShape) -> Self {
        match shape {
            ChannelShape::Button => Self::AnyButton,
            ChannelShape::Axis1 => Self::AnyAxis,
            ChannelShape::Axis2 => Self::AnyStick,
            ChannelShape::Delta2 => Self::AnyDelta,
        }
    }
}

/// What a class binding actually watches: one of the three shape classes above, or the
/// character-producing door [`InputContextBuilder::bind_characters`] opens instead.
///
/// Not a fourth [`ControlClass`] variant: membership in that class is a property of the *event* a
/// control produced, not of the control's identity — the same key is a dead key on one press and a
/// plain letter on the next — so it could never honestly answer [`contains`](ControlClass::contains)
/// the way the other three do. Kept private, since a class binding is the only thing that needs to
/// name it.
///
/// [`InputContextBuilder::bind_characters`]: crate::binding::InputContextBuilder::bind_characters
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClassFilter {
    /// One of the three shape classes, matched against the event's own control.
    Shape(ControlClass),
    /// Any keyboard key whose event carries text, once IME composition and dead keys are
    /// accounted for.
    Characters,
}

impl ClassFilter {
    /// Whether the control that produced `event` matches this filter, given what actually
    /// happened.
    pub(crate) fn matches(self, event: &crate::frame::RawEvent) -> bool {
        match self {
            Self::Shape(class) => event
                .control()
                .is_some_and(|control| class.contains(control)),
            Self::Characters => character_producing(event),
        }
    }
}

// Measured with `examples/ime_diagnostic.rs` on macOS, not reasoned from documentation — changing
// this predicate means measuring again. A kana source delivers each keystroke as its own `Pressed`
// with `text: Some(...)`; there is no `Pressed` carrying `text: None` mid-composition, and releases
// always carry `text: None`. Composition upstream arrives already composed in one event, single- or
// multi-character alike.
//
// `Pressed` only, so a release never re-fires a class binding — the same rule every other binding
// follows, stated here because there is no per-control state to fall back on.
//
// Unmeasured: committing a multi-candidate kana-to-kanji conversion from an IME popup. Expected to
// hold, since it commits through ordinary keystrokes this already judges independently.
#[cfg(feature = "keyboard")]
fn character_producing(event: &crate::frame::RawEvent) -> bool {
    matches!(
        event,
        crate::frame::RawEvent::Keyboard(KeyboardInput {
            text: Some(_),
            state: ButtonState::Pressed,
            ..
        })
    )
}

#[cfg(not(feature = "keyboard"))]
fn character_producing(_event: &crate::frame::RawEvent) -> bool {
    false
}

/// A control withheld from capture, and what withheld it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReservedControl {
    /// The control nothing may be bound over.
    pub control: Control,
    /// The declared path of the action that reserved it.
    pub action_path: &'static str,
    /// The declared path of the context the reserving binding lives in.
    pub context: &'static str,
}

/// Every control any context has reserved.
///
/// Flat and global rather than per-context, because that is the scope reserving has: the control
/// that opens the settings screen must be refused while capturing for a mapping declared anywhere.
#[derive(Resource, Default, Debug)]
pub struct ReservedControls(pub(crate) Vec<ReservedControl>);

impl ReservedControls {
    /// What reserved this control, if anything did.
    pub fn claimant(&self, control: Control) -> Option<&ReservedControl> {
        self.0.iter().find(|reserved| reserved.control == control)
    }

    /// Whether anything reserved this control.
    pub fn contains(&self, control: Control) -> bool {
        self.claimant(control).is_some()
    }

    /// Every reservation, in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = &ReservedControl> {
        self.0.iter()
    }
}

/// A request to report the next control the player chooses.
///
/// Insert it on an entity; the crate fills the answer in as [`ControlCaptured`] on that same
/// entity and removes the component. Remove it yourself to cancel.
#[derive(Component, Clone, Debug)]
pub struct CaptureSession {
    mapping: Option<MappingKey>,
    slot: usize,
    accepts: ControlClass,
    family: Option<DeviceFamily>,
    excluded: Vec<Control>,
    // `false` until the session has seen one run of the capture system. Arming costs a frame and
    // buys the thing this would otherwise get wrong every time: the press that opened the capture
    // is still in the queue when the session arrives, so a session that read the queue immediately
    // would bind whichever key the player activated the row with.
    armed: bool,
    cursor: Option<FrameTimestamp>,
}

impl CaptureSession {
    /// Listens for a control for this mapping's first slot.
    ///
    /// Takes the shape and the family from the mapping, which is what makes a keyboard row accept a
    /// key and not a gamepad button, and a stick row accept a stick pushed whole rather than one of
    /// its axes.
    ///
    /// A mapping holds a list of slots, and this addresses the front of it — the "primary" column
    /// of a table with more than one. Use [`for_slot`](Self::for_slot) for the others.
    pub fn for_mapping(mapping: &ActionMapping) -> Option<Self> {
        Self::for_slot(mapping, 0)
    }

    /// Listens for a control for one numbered slot of this mapping.
    ///
    /// A mapping holds an ordered list of slots, and a "primary and secondary" table is that list
    /// drawn as columns — so which slot the player activated is what a capture has to carry, or the
    /// answer has nowhere to go but the front of the row.
    ///
    /// Any slot number is addressable, whether the row reaches that far yet or not: the row grows
    /// to fit, and the slots skipped on the way are left empty. Writing to the third cell of a row
    /// holding one control gives a row of three, the middle one blank — so a screen can offer
    /// whatever cells it draws without first checking how long the row happens to be.
    ///
    /// Returns `None` only for a mapping the player may not change at all — see
    /// [`RebindPolicy`](crate::mapping::RebindPolicy).
    pub fn for_slot(mapping: &ActionMapping, slot: usize) -> Option<Self> {
        // A mapping the player cannot change has nothing to capture *for*. It is on the screen so
        // they can read it, and a screen that asked anyway would be offering a rebind it could not
        // then apply.
        if !mapping.rebind_policy.is_rebindable() {
            return None;
        }
        Some(Self {
            mapping: Some(mapping.key),
            slot,
            ..Self::accepting(ControlClass::of(mapping.accepts)).within(mapping.family)
        })
    }

    /// Listens for any control of a class, without a mapping in mind.
    pub fn accepting(class: ControlClass) -> Self {
        Self {
            mapping: None,
            slot: 0,
            accepts: class,
            family: None,
            excluded: Vec::new(),
            armed: false,
            cursor: None,
        }
    }

    /// Restricts capture to one control family.
    pub fn within(mut self, family: DeviceFamily) -> Self {
        self.family = Some(family);
        self
    }

    /// Ignores these controls entirely, so they keep doing whatever they normally do.
    ///
    /// This is what keeps the screen usable while it is listening: the control that cancels a
    /// capture has to reach the thing that cancels it, so capture must neither take it nor swallow
    /// it.
    pub fn excluding(mut self, controls: impl IntoIterator<Item = Control>) -> Self {
        self.excluded.extend(controls);
        self
    }

    /// The mapping this capture is for, if it was made for one.
    pub fn mapping(&self) -> Option<MappingKey> {
        self.mapping
    }

    /// Which slot of that mapping it is for. Zero for a session made without a mapping.
    pub fn slot(&self) -> usize {
        self.slot
    }

    /// The class of control it will take.
    pub fn accepts(&self) -> ControlClass {
        self.accepts
    }

    /// The family it is restricted to, if any.
    pub fn family(&self) -> Option<DeviceFamily> {
        self.family
    }

    /// The controls it ignores.
    pub fn excluded(&self) -> &[Control] {
        &self.excluded
    }

    /// Whether it has started listening.
    ///
    /// False for the first frame after insertion, while the session is skipping past whatever was
    /// already in the queue. A screen can show "press a control" regardless; this exists for tests
    /// and for anything that wants to be exact about it.
    pub fn is_listening(&self) -> bool {
        self.armed
    }
}

/// Whether a control may fill a slot that takes `accepts`, and if not, why not.
///
/// The same three questions arrive from two directions — a press at a rebinding screen, and a row
/// in a save file — and one control must get one answer either way. `family` is `None` for a
/// capture not restricted to one.
///
/// The order is the order the reasons come in, and reserved is asked before shape so that pressing
/// the settings key is answered with the reason it cannot be bound rather than with a complaint
/// about its channel.
pub(crate) fn admissible(
    control: Control,
    family: Option<DeviceFamily>,
    accepts: ControlClass,
    reserved: bool,
) -> Result<(), RefusedReason> {
    if family.is_some_and(|family| family != control.family()) {
        return Err(RefusedReason::Family);
    }
    if reserved {
        return Err(RefusedReason::Reserved);
    }
    if !accepts.contains(control) {
        return Err(RefusedReason::Shape);
    }
    Ok(())
}

/// The player chose a control.
///
/// Fired on the entity that carried the [`CaptureSession`], which is then removed. Nothing has been
/// rebound: this reports what was chosen, and what to do about it — including what it clashes with,
/// via [`conflicts`] — is the caller's.
#[derive(EntityEvent, Clone, Debug)]
pub struct ControlCaptured {
    /// The entity whose capture this was.
    pub entity: Entity,
    /// The mapping it was for, if it was made for one.
    pub mapping: Option<MappingKey>,
    /// Which slot of that mapping the control belongs in.
    ///
    /// Zero unless the session named another, which is what a "primary and secondary" table does.
    /// See [`CaptureSession::for_slot`].
    pub slot: usize,
    /// The control the player chose.
    pub control: Control,
}

/// The player pressed something capture will not take, and deserves to be told why.
///
/// Fired for a deliberate press only. A stick drifting or a mouse twitching past its threshold is
/// dropped silently, because a screen that complained about every one of those would do nothing
/// else. The session stays: the player can try again.
#[derive(EntityEvent, Clone, Debug)]
pub struct CaptureRefused {
    /// The entity whose capture this is.
    pub entity: Entity,
    /// The mapping it is for, if it was made for one.
    pub mapping: Option<MappingKey>,
    /// The control that was refused.
    pub control: Control,
    /// Why it was refused.
    pub reason: RefusedReason,
}

/// Why capture would not take a control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefusedReason {
    /// It reports on a channel the mapping's action cannot use.
    Shape,
    /// It belongs to a different device family than the one being rebound.
    Family,
    /// A binding reserved it, so nothing may be bound over it.
    Reserved,
}

/// A mapping that already answers the press in question.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MappingConflict {
    /// The mapping that holds it.
    pub mapping: MappingKey,
    /// The declared path of the action that mapping drives.
    pub action_path: &'static str,
    /// The declared path of the context it lives in.
    pub context: &'static str,
    /// Whether the two are certainly in each other's way, or only possibly.
    pub overlap: ConflictOverlap,
}

/// How much of a problem a [`MappingConflict`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictOverlap {
    /// Both mappings are in one context, so both are always live together and the clash is real.
    SameContext,
    /// The mappings are in different contexts, which may never be active at the same time — a menu
    /// key and a gameplay key can share a control quite deliberately. Whether this matters is a
    /// question about the game's own activation rules, which this crate does not know.
    OtherContext,
}

/// Which mappings already answer a press.
///
/// This can be answered before anything is committed to: a screen calls this with the slot it is
/// about to write, usually the control capture just reported, and decides what to say. Deciding
/// what to *do* — reject, swap, unbind the other — needs somewhere to write the answer, which is a
/// separate matter.
///
/// `target` is the mapping being rebound, and is excluded from the result: a mapping does not
/// conflict with itself, and rebinding a control to where it already is should report nothing. The
/// whole mapping is excluded rather than the one slot, so putting a control in a row's second slot
/// while its first already holds it is not reported here — a repeat *within* one row is a question
/// for the conflict policy that applies a rebind, not for the detection that precedes it.
///
/// Conflicts are per family, so a keyboard binding never clashes with a gamepad one. A slot clashes
/// with another holding the same control and the same chord, as [`BoundSlot::clashes_with`]
/// decides, so `S` and `Ctrl+S` can sit on two rows without either being reported. Two cases get
/// through: `Ctrl+S` beside a chord naming one Control key by its
/// [`KeyCode`](bevy_input::keyboard::KeyCode), and `Ctrl+S` beside `Shift+S` for a player holding
/// both modifiers. Both are real clashes that go unreported.
pub fn conflicts(
    world: &World,
    candidate: &BoundSlot,
    target: Option<MappingKey>,
) -> Vec<MappingConflict> {
    conflicts_in(&crate::mapping::mappings(world), None, candidate, target)
}

/// Which mappings already answer a press, as a screen's own unconfirmed choices would leave things.
///
/// Same question as [`conflicts`], against a working copy rather than what is currently applied — a
/// settings screen holds its player's choices in a [`Overrides`] of its own until they confirm, and
/// a choice that has not been confirmed yet still has to be able to clash with another one that
/// hasn't either. `mappings` is the applied baseline (as `crate::mapping::mappings` returns), and
/// `pending` is laid over it: a row `pending` names reads as that row says, and everything else
/// reads as `mappings` already has it.
///
/// A backend-owned row (`Override::NotOurs` in `pending`) reads as `mappings` already has it
/// (unaffected, not cleared), matching how [`crate::overrides::apply_overrides`] treats it.
///
/// Resolving a conflict this finds is the caller's decision, made with [`Overrides::bind`] and
/// [`Overrides::get`] directly rather than through another crate API: refuse the conflict by not
/// writing the candidate row, allow the duplicate by writing it regardless, or rewrite the
/// conflicting row without the clashing slot — putting the candidate's own previous slot there
/// instead trades the two. A row that would hold one slot twice never reaches this function, since
/// a mapping never conflicts with itself, so a caller checks its own candidate list for that before
/// writing it.
pub fn conflicts_pending(
    mappings: &[ActionMapping],
    pending: &Overrides,
    candidate: &BoundSlot,
    target: Option<MappingKey>,
) -> Vec<MappingConflict> {
    conflicts_in(mappings, Some(pending), candidate, target)
}

/// The shared walk behind `conflicts` and `conflicts_pending`.
///
/// `pending` is `None` for the world-only form; `Some` layers a working copy over `mappings` before
/// asking the same question, which is why both forms produce identical results for identical
/// inputs.
fn conflicts_in(
    mappings: &[ActionMapping],
    pending: Option<&Overrides>,
    candidate: &BoundSlot,
    target: Option<MappingKey>,
) -> Vec<MappingConflict> {
    let target_context = target.and_then(|key| {
        mappings
            .iter()
            .find(|mapping| mapping.key == key)
            .map(|mapping| mapping.context)
    });

    mappings
        .iter()
        .filter(|mapping| Some(mapping.key) != target && holds(mapping, pending, candidate))
        .map(|mapping| MappingConflict {
            mapping: mapping.key,
            action_path: mapping.action_path,
            context: mapping.context,
            overlap: if Some(mapping.context) == target_context {
                ConflictOverlap::SameContext
            } else {
                ConflictOverlap::OtherContext
            },
        })
        .collect()
}

/// Whether a mapping currently holds a slot clashing with `candidate`: in `pending`'s row for it if
/// there is one, else in its own slots.
///
/// A row absent from `pending` means untouched, and a `NotOurs` row means the same, since something
/// else owns it and this crate neither fills it in nor reads it as cleared.
///
/// An empty slot holds nothing rather than holding "no control", so two rows with a gap apiece are
/// not a clash.
fn holds(mapping: &ActionMapping, pending: Option<&Overrides>, candidate: &BoundSlot) -> bool {
    let slots = match pending.and_then(|pending| pending.get(mapping.family, mapping.key)) {
        Some(Override::Slots(slots)) => slots,
        Some(Override::Cleared) => return false,
        Some(Override::NotOurs) | None => &mapping.slots,
    };
    slots
        .iter()
        .flatten()
        .any(|slot| slot.clashes_with(candidate))
}

/// One control arriving, and whether the player meant it.
struct Arrival {
    control: Control,
    /// True for a press, false for a continuous reading that crossed its threshold. Only a
    /// deliberate arrival is worth refusing out loud.
    deliberate: bool,
}

/// Turns one raw event into the control a player would say they just used, if any.
///
/// `accepts` is what disambiguates a stick's axis: the same deflection is the whole
/// [`Control::GamepadStick`] to a session listening for one, and the bare
/// [`Control::GamepadAxis`] to one listening for a trigger or a single axis bound directly. Every
/// other event has only one control it could ever mean, `accepts` or not.
fn arrival(
    event: &RawEvent,
    threshold: &ButtonThreshold,
    accepts: ControlClass,
) -> Option<Arrival> {
    #[cfg(not(feature = "gamepad"))]
    let _ = (threshold, accepts);

    match event {
        // Presses only. Capturing on release would let go of a key the player is still holding, and
        // `repeat` would bind the same key several times over while they waited.
        #[cfg(feature = "keyboard")]
        RawEvent::Keyboard(key) => (key.state == bevy_input::ButtonState::Pressed && !key.repeat)
            .then_some(Arrival {
                control: Control::PhysicalKey(key.key_code),
                deliberate: true,
            }),
        // A press, like a key: the player meant it, so refusing one is worth saying out loud.
        #[cfg(feature = "mouse")]
        RawEvent::MouseButton(button) => (button.state == bevy_input::ButtonState::Pressed)
            .then_some(Arrival {
                control: Control::MouseButton(button.button),
                deliberate: true,
            }),
        RawEvent::MouseMotion(delta) => (delta.length() >= MOUSE_MOTION).then_some(Arrival {
            control: Control::MouseMotion,
            deliberate: false,
        }),
        #[cfg(feature = "gamepad")]
        RawEvent::Gamepad(event) => match event {
            // Our own threshold rather than whatever the backend synthesized, for the same reason
            // the evaluator uses its own (R14.2).
            bevy_input::gamepad::RawGamepadEvent::Button(button) => {
                threshold.pressed(button.value, false).then_some(Arrival {
                    control: Control::GamepadButton(button.button),
                    deliberate: true,
                })
            }
            bevy_input::gamepad::RawGamepadEvent::Axis(axis) => (axis.value.abs() >= DEFLECTION)
                .then(|| {
                    let control = match (accepts, crate::binding::Stick::containing(axis.axis)) {
                        (ControlClass::AnyStick, Some(stick)) => Control::GamepadStick(stick),
                        _ => Control::GamepadAxis(axis.axis),
                    };
                    Arrival {
                        control,
                        deliberate: false,
                    }
                }),
            bevy_input::gamepad::RawGamepadEvent::Connection(_) => None,
        },
        // Losing focus never arrives as a control a player meant to bind.
        #[cfg(any(feature = "keyboard", feature = "mouse"))]
        RawEvent::FocusLost => None,
    }
}

/// Says so when a screen opens a capture for a slot its own [`MaxSlots`] would then refuse.
///
/// Filling slot `n` makes the row at least `n + 1` long, so the slot number alone settles it and
/// nothing has to look the row up. The capture still runs and the control is still captured; it is
/// [`apply_overrides`](crate::overrides::apply_overrides) that turns the row down, and this is the
/// line that says why before the player finds out by pressing something.
///
/// A game that set no ceiling has no cell this could be wrong about, so the observer costs it one
/// absent-resource check per session.
pub(crate) fn warn_if_past_the_ceiling(
    session: bevy_ecs::prelude::On<'_, '_, bevy_ecs::lifecycle::Insert<CaptureSession>>,
    sessions: Query<'_, '_, &CaptureSession>,
    max_slots: Option<Res<'_, crate::overrides::MaxSlots>>,
) {
    let Some(max) = max_slots else {
        return;
    };
    let Ok(started) = sessions.get(session.entity) else {
        return;
    };
    if started.slot >= max.0 {
        bevy_utils::once!(log::warn!(
            "a capture was opened for slot {} of a row, but `MaxSlots` is {} — the control will be \
             captured and then refused when the override set is applied. The screen is offering a \
             cell past the ceiling the game set",
            started.slot,
            max.0
        ));
    }
}

/// Reads the frame on behalf of every live capture session.
///
/// Runs between sampling and evaluation, which is what lets it claim what it saw before any context
/// gets to act on it.
pub fn run_captures(
    mut commands: Commands<'_, '_>,
    frame: Res<'_, InputFrame>,
    threshold: Res<'_, ButtonThreshold>,
    reserved: Res<'_, ReservedControls>,
    mut consumed: ResMut<'_, crate::eval::ConsumedControls>,
    mut sessions: Query<'_, '_, (Entity, &mut CaptureSession)>,
) {
    for (entity, mut session) in &mut sessions {
        if !session.armed {
            session.armed = true;
            session.cursor = frame.latest();
            continue;
        }

        for event in frame.events_after(session.cursor) {
            session.cursor = Some(event.timestamp);

            let Some(arrival) = arrival(&event.event, &threshold, session.accepts) else {
                continue;
            };

            // Asked before admissibility, and unconditionally: an excluded control is not capture's
            // business at all.
            if session.excluded.contains(&arrival.control) {
                continue;
            }

            if let Err(reason) = admissible(
                arrival.control,
                session.family,
                session.accepts,
                reserved.contains(arrival.control),
            ) {
                // Claimed even though it was refused: the player pressed it at a rebinding screen,
                // and whatever it would otherwise have done is not what they meant.
                consumed.claim_for_capture(arrival.control);
                if arrival.deliberate {
                    commands.trigger(CaptureRefused {
                        entity,
                        mapping: session.mapping,
                        control: arrival.control,
                        reason,
                    });
                }
                continue;
            }

            consumed.claim_for_capture(arrival.control);
            // Removed *before* the event, and both halves of that matter. An observer is entitled
            // to do anything to this entity, despawning it included — a settings row that closes on
            // being answered is an ordinary thing to write — so the crate must have finished with
            // the entity before it hands it over. It also means the observer sees the component
            // already gone, so "is this row still listening" reads the same from inside the
            // observer as from anywhere else.
            //
            // Fallible because one run can answer several sessions, and the first observer to run
            // may despawn a later one's entity.
            commands.entity(entity).try_remove::<CaptureSession>();
            commands.trigger(ControlCaptured {
                entity,
                mapping: session.mapping,
                slot: session.slot,
                control: arrival.control,
            });
            break;
        }
    }
}

// No test here spawns an instance of the context being rebound, except where one is the point:
// capture reads the frame rather than a binding (R19.1, D40).
#[cfg(all(test, feature = "keyboard"))]
mod tests {
    use super::*;

    use bevy_app::App;
    use bevy_ecs::prelude::On;
    use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
    use bevy_input::{ButtonState, InputPlugin};

    use crate::context::ActionMapAppExt;
    use crate::{ActionMapPlugin, InputAction, InputContext};

    #[derive(InputAction)]
    #[action(path = "capture_tests.move", output = bevy_math::Vec2, intent = Directional2)]
    struct Move;

    #[derive(InputAction)]
    #[action(path = "capture_tests.jump", output = bool, intent = Button)]
    struct Jump;

    #[derive(InputAction)]
    #[action(path = "capture_tests.crouch", output = bool, intent = Button)]
    struct Crouch;

    #[derive(InputAction)]
    #[action(path = "capture_tests.settings", output = bool, intent = Button)]
    struct OpenSettings;

    #[derive(InputAction)]
    #[action(path = "capture_tests.confirm", output = bool, intent = Button)]
    struct Confirm;

    #[derive(InputContext)]
    #[context(path = "capture_tests.on_foot", tick = Render)]
    struct OnFoot;

    #[derive(InputContext)]
    #[context(path = "capture_tests.menu", tick = Render)]
    struct Menu;

    /// Everything a captured or refused control was reported as, in order.
    #[derive(Resource, Default)]
    struct Heard {
        captured: Vec<Control>,
        slots: Vec<usize>,
        rows: Vec<Option<MappingKey>>,
        refused: Vec<(Control, RefusedReason)>,
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.init_resource::<Heard>();
        app.add_observer(|event: On<ControlCaptured>, mut heard: ResMut<'_, Heard>| {
            heard.captured.push(event.control);
            heard.slots.push(event.slot);
            heard.rows.push(event.mapping);
        });
        app.add_observer(|event: On<CaptureRefused>, mut heard: ResMut<'_, Heard>| {
            heard.refused.push((event.control, event.reason));
        });
        app.add_context::<OnFoot>(|controls| {
            controls
                .bind::<Move>(crate::binding::DirectionalButtons::wasd())
                .mappable();
            // One default: enough to address a filled slot, the next empty one, and one that would
            // leave a hole behind it.
            controls.bind::<Jump>(KeyCode::Space).mappable();
            // Two defaults, so a row has a secondary for conflict detection to find.
            controls.bind::<Crouch>(KeyCode::KeyC).mappable();
            controls.bind::<Crouch>(KeyCode::KeyV).mappable();
            controls.bind::<OpenSettings>(KeyCode::F1).reserved();
        });
        // A second context sharing a control with the first, so a conflict can be reported across
        // contexts as well as within one.
        app.add_context::<Menu>(|controls| {
            controls.bind::<Confirm>(KeyCode::KeyC).mappable();
        });
        app
    }

    fn key_event(code: KeyCode, state: ButtonState) -> KeyboardInput {
        KeyboardInput {
            key_code: code,
            logical_key: Key::Space,
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

    fn press(app: &mut App, code: KeyCode) {
        app.world_mut()
            .write_message(key_event(code, ButtonState::Pressed));
    }

    fn mapping(app: &App, key: &str) -> ActionMapping {
        crate::mapping::mappings(app.world())
            .into_iter()
            .find(|mapping| alloc::string::ToString::to_string(&mapping.key) == key)
            .expect("no such mapping")
    }

    /// The whole protocol: what comes back is the control's *identity*, which a binding would have
    /// turned into a value and discarded.
    #[test]
    fn a_session_arms_answers_once_and_then_is_gone() {
        let mut app = app();
        let target = mapping(&app, "capture_tests.move.up");
        let row = app
            .world_mut()
            .spawn(CaptureSession::for_mapping(&target).expect("a button mapping"))
            .id();

        // The arming frame takes no control, which is what stops it binding the key the player
        // opened the row with.
        press(&mut app, KeyCode::Enter);
        app.update();
        assert!(app.world().resource::<Heard>().captured.is_empty());
        assert!(
            app.world().get::<CaptureSession>(row).is_some(),
            "still listening"
        );

        press(&mut app, KeyCode::KeyT);
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::PhysicalKey(KeyCode::KeyT)]
        );
        // Answered once, and the component is gone — which is how a screen knows it has stopped
        // listening without being told separately.
        assert!(app.world().get::<CaptureSession>(row).is_none());

        // Cancelling is removing the component, and a cancelled session hears nothing more.
        let cancelled = app
            .world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton))
            .id();
        app.update();
        app.world_mut()
            .entity_mut(cancelled)
            .remove::<CaptureSession>();
        press(&mut app, KeyCode::KeyM);
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().captured.len(),
            1,
            "the cancelled session heard nothing"
        );
    }

    /// An observer may do anything to the entity it is handed, despawning it included. A guard
    /// rather than a reproduction: whether an observer's deferred commands run before or after
    /// those already queued depends on the executor, so the original failure showed up only under
    /// `DefaultPlugins`.
    #[test]
    fn an_observer_owns_the_entity_by_the_time_it_runs() {
        #[derive(Resource, Default)]
        struct StillListening(Option<bool>);

        let mut app = app();
        app.init_resource::<StillListening>();
        // Anything the crate does wrong to a despawned entity arrives through the error handler,
        // which warns by default and would let this pass unnoticed.
        app.set_error_handler(bevy_ecs::error::panic);
        app.add_observer(
            |captured: On<ControlCaptured>,
             sessions: Query<'_, '_, &CaptureSession>,
             mut seen: ResMut<'_, StillListening>,
             mut commands: Commands<'_, '_>| {
                seen.0 = Some(sessions.get(captured.entity).is_ok());
                // Deferred rather than inline, which is what an observer wanting the whole world
                // has to do — reading `conflicts` needs `&World` — and is the shape the failure
                // arrived in.
                let entity = captured.entity;
                commands.queue(move |world: &mut World| {
                    world.despawn(entity);
                });
            },
        );

        let row = app
            .world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton))
            .id();
        app.update();
        press(&mut app, KeyCode::KeyP);
        app.update();

        assert_eq!(
            app.world().resource::<StillListening>().0,
            Some(false),
            "the component is already removed when the observer runs"
        );
        assert!(app.world().get_entity(row).is_err(), "the observer had it");
        // And a second frame, in case anything was left queued against it.
        app.update();
    }

    /// Two rows can listen at once, which is what a split screen needs and what a single global
    /// session could not have offered.
    #[test]
    fn two_sessions_capture_independently() {
        let mut app = app();
        let first = app
            .world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton))
            .id();
        let second = app
            .world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton))
            .id();
        app.update();

        press(&mut app, KeyCode::KeyN);
        app.update();

        assert_eq!(
            app.world().resource::<Heard>().captured,
            alloc::vec![Control::PhysicalKey(KeyCode::KeyN); 2]
        );
        assert!(app.world().get::<CaptureSession>(first).is_none());
        assert!(app.world().get::<CaptureSession>(second).is_none());
    }

    /// Each of these reaches a listening session and must produce neither a capture nor a refusal:
    /// an excluded control because it is still doing its normal job, the rest because nobody chose
    /// anything. Capturing on a release would take a key the player is still holding, and a repeat
    /// would bind the same key over and over while they waited.
    #[test]
    fn what_capture_passes_over_in_silence() {
        let mut app = app();
        app.world_mut().spawn(
            CaptureSession::accepting(ControlClass::AnyButton)
                .excluding([Control::PhysicalKey(KeyCode::Escape)]),
        );
        app.update();

        /// One thing arriving at a session, as the test drives it.
        type Arrival = fn(&mut App);

        let cases: [(&str, Arrival); 3] = [
            ("an excluded control", |app| press(app, KeyCode::Escape)),
            ("a key release", |app| {
                app.world_mut()
                    .write_message(key_event(KeyCode::KeyJ, ButtonState::Released));
            }),
            ("a key repeat", |app| {
                let mut held = key_event(KeyCode::KeyJ, ButtonState::Pressed);
                held.repeat = true;
                app.world_mut().write_message(held);
            }),
        ];

        for (what, arrive) in cases {
            arrive(&mut app);
            app.update();
            let heard = app.world().resource::<Heard>();
            assert!(heard.captured.is_empty(), "{what} was captured");
            assert!(heard.refused.is_empty(), "{what} was refused");
        }

        #[cfg(feature = "mouse")]
        {
            app.world_mut()
                .write_message(bevy_input::mouse::MouseButtonInput {
                    button: bevy_input::mouse::MouseButton::Middle,
                    state: ButtonState::Released,
                    window: Entity::PLACEHOLDER,
                });
            app.update();
            let heard = app.world().resource::<Heard>();
            assert!(heard.captured.is_empty(), "a mouse button release");
            assert!(heard.refused.is_empty(), "a mouse button release");
        }

        // And the session is still listening through all of it.
        press(&mut app, KeyCode::KeyY);
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::PhysicalKey(KeyCode::KeyY)]
        );
    }

    /// The keyboard-and-mouse family is one family, so a mouse button fills a mapping a key holds —
    /// what a player expects of "fire on left click", and what a check comparing devices rather
    /// than families would get wrong. Motion has no resting position, so only a deliberate sweep
    /// counts.
    #[cfg(feature = "mouse")]
    #[test]
    fn what_the_mouse_offers_capture() {
        use bevy_input::mouse::{MouseButton, MouseButtonInput, MouseMotion};

        let mut clicking = app();
        let target = mapping(&clicking, "capture_tests.jump");
        clicking
            .world_mut()
            .spawn(CaptureSession::for_mapping(&target).expect("a button mapping"));
        clicking.update();
        clicking.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        });
        clicking.update();
        assert_eq!(
            clicking.world().resource::<Heard>().captured,
            [Control::MouseButton(MouseButton::Left)]
        );

        let mut moving = app();
        moving
            .world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyDelta));
        moving.update();

        moving.world_mut().write_message(MouseMotion {
            delta: bevy_math::Vec2::new(MOUSE_MOTION - 1.0, 0.0),
        });
        moving.update();
        assert!(
            moving.world().resource::<Heard>().captured.is_empty(),
            "short of the threshold"
        );

        moving.world_mut().write_message(MouseMotion {
            delta: bevy_math::Vec2::new(MOUSE_MOTION, 0.0),
        });
        moving.update();
        assert_eq!(
            moving.world().resource::<Heard>().captured,
            [Control::MouseMotion]
        );
    }

    /// A control refusable twice over gets the reason it is owed. The order is checked on the
    /// predicate as well, because a saved file is answered from the same rule and the two must not
    /// disagree.
    #[test]
    fn a_refusal_names_its_reason_and_leaves_the_session_listening() {
        assert_eq!(
            admissible(
                Control::PhysicalKey(KeyCode::F1),
                None,
                ControlClass::AnyAxis,
                true
            ),
            Err(RefusedReason::Reserved),
            "reserved is answered before shape"
        );

        let mut app = app();
        let target = mapping(&app, "capture_tests.jump");
        app.world_mut()
            .spawn(CaptureSession::for_mapping(&target).expect("a button mapping"));
        app.update();

        // Reserving would be worth little if the screen key merely had no mapping of its own —
        // anything else could still be bound over the top of it.
        press(&mut app, KeyCode::F1);
        app.update();
        assert!(app.world().resource::<Heard>().captured.is_empty());
        assert_eq!(
            app.world().resource::<Heard>().refused,
            [(Control::PhysicalKey(KeyCode::F1), RefusedReason::Reserved)]
        );

        // A mapping is rebound within its family, so the pad cannot answer for the keyboard.
        #[cfg(feature = "gamepad")]
        {
            use bevy_input::gamepad::{GamepadButton, RawGamepadButtonChangedEvent};

            app.world_mut()
                .write_message(bevy_input::gamepad::RawGamepadEvent::Button(
                    RawGamepadButtonChangedEvent::new(
                        Entity::PLACEHOLDER,
                        GamepadButton::South,
                        1.0,
                    ),
                ));
            app.update();
            assert_eq!(
                app.world().resource::<Heard>().refused[1],
                (
                    Control::GamepadButton(GamepadButton::South),
                    RefusedReason::Family
                )
            );
        }

        // Refused, not cancelled: the player can pick something else.
        press(&mut app, KeyCode::KeyE);
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::PhysicalKey(KeyCode::KeyE)]
        );
    }

    /// A settings screen reached from a pause menu has the context it is rebinding live behind it,
    /// and what the player presses there must not also play the game. The spawned instance is what
    /// makes the test mean anything: with none, nothing could fire whether the control was taken or
    /// not.
    #[test]
    fn a_capture_suppresses_the_live_context_it_is_rebinding() {
        use crate::event::Fired;

        #[derive(Resource, Default)]
        struct Fires(usize);

        let mut app = app();
        app.init_resource::<Fires>();
        app.add_observer(|_: On<Fired<Jump>>, mut fires: ResMut<'_, Fires>| {
            fires.0 += 1;
        });

        app.world_mut().spawn(OnFoot);
        app.world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton));
        app.update();

        // Space is Jump's default binding, so an unsuppressed press would fire it.
        press(&mut app, KeyCode::Space);
        app.update();

        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::PhysicalKey(KeyCode::Space)]
        );
        assert_eq!(
            app.world()
                .resource::<crate::eval::ConsumedControls>()
                .claimant(Control::PhysicalKey(KeyCode::Space)),
            Some("capture")
        );
        assert_eq!(
            app.world().resource::<Fires>().0,
            0,
            "the control was taken from the game, not merely booked"
        );
    }

    /// The same, from the pad: a settings screen operated from the controller captures a button the
    /// game behind it also binds.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_capture_on_a_gamepad_button_does_not_reach_the_game() {
        use crate::event::Fired;
        use bevy_input::gamepad::{GamepadButton, RawGamepadButtonChangedEvent};

        #[derive(InputContext)]
        #[context(path = "capture_tests.pad", tick = Render)]
        struct Pad;

        #[derive(Resource, Default)]
        struct Fires(usize);

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.init_resource::<Heard>();
        app.init_resource::<Fires>();
        app.add_observer(|event: On<ControlCaptured>, mut heard: ResMut<'_, Heard>| {
            heard.captured.push(event.control);
        });
        app.add_observer(|_: On<Fired<Jump>>, mut fires: ResMut<'_, Fires>| {
            fires.0 += 1;
        });
        app.add_context::<Pad>(|controls| {
            controls.bind::<Jump>(GamepadButton::South).mappable();
        });

        app.world_mut().spawn(Pad);
        let target = &crate::mapping::mappings(app.world())[0];
        app.world_mut()
            .spawn(CaptureSession::for_mapping(target).expect("a button mapping"));
        app.update();

        app.world_mut()
            .write_message(bevy_input::gamepad::RawGamepadEvent::Button(
                RawGamepadButtonChangedEvent::new(Entity::PLACEHOLDER, GamepadButton::South, 1.0),
            ));
        app.update();

        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::GamepadButton(GamepadButton::South)]
        );
        assert_eq!(app.world().resource::<Fires>().0, 0);
    }

    /// A capture says which slot it fills, and a slot is addressed rather than appended: a screen
    /// offers whatever cells it draws without first asking how long the row happens to be.
    #[test]
    fn a_row_is_addressed_by_slot() {
        let mut app = app();
        let jump = mapping(&app, "capture_tests.jump");
        assert_eq!(jump.slots.len(), 1, "one default");

        // What a single-column table gets without asking.
        assert_eq!(
            CaptureSession::for_mapping(&jump)
                .expect("a button mapping")
                .slot(),
            0
        );
        assert!(CaptureSession::for_slot(&jump, 1).is_some(), "the next one");
        assert!(
            CaptureSession::for_slot(&jump, 4).is_some(),
            "and one well past the end: the row grows to reach it"
        );

        // A row the player may not change has nothing to capture for, at any slot — the one rule
        // left.
        let settings = mapping(&app, "capture_tests.settings");
        assert!(!settings.rebind_policy.is_rebindable());
        assert!(CaptureSession::for_slot(&settings, 0).is_none());

        // And the slot the session was made for is what reaches the observer.
        app.world_mut()
            .spawn(CaptureSession::for_slot(&jump, 1).expect("the empty second slot"));
        app.update();
        press(&mut app, KeyCode::KeyK);
        app.update();

        let heard = app.world().resource::<Heard>();
        assert_eq!(heard.captured, [Control::PhysicalKey(KeyCode::KeyK)]);
        assert_eq!(heard.slots, [1]);
        // The row travels with the slot: without it a screen knows which column was filled and not
        // which line of the table it belongs to.
        assert_eq!(heard.rows, [Some(jump.key)]);
    }

    /// A row holds a list, so *any* slot of it holding the control is a clash — a secondary binding
    /// is no less bound than a primary one.
    #[test]
    fn conflicts_name_the_mappings_that_hold_a_control() {
        let app = app();
        let jump = mapping(&app, "capture_tests.jump").key;

        let found = conflicts(
            app.world(),
            &Control::PhysicalKey(KeyCode::KeyW).into(),
            Some(jump),
        );
        assert_eq!(found.len(), 1);
        assert_eq!(
            alloc::string::ToString::to_string(&found[0].mapping),
            "capture_tests.move.up"
        );
        assert_eq!(found[0].action_path, "capture_tests.move");
        assert_eq!(
            found[0].overlap,
            ConflictOverlap::SameContext,
            "both are bound in on_foot, so they are certainly in each other's way"
        );

        // The secondary of a two-default row, which a `==` against a single control would miss.
        let secondary = conflicts(
            app.world(),
            &Control::PhysicalKey(KeyCode::KeyV).into(),
            Some(jump),
        );
        assert_eq!(secondary.len(), 1);
        assert_eq!(secondary[0].action_path, "capture_tests.crouch");

        // Asked from the menu's side, the same control is held by a row in another context — a menu
        // key and a gameplay key can share one quite deliberately, and whether that matters is a
        // question about the game's own activation rules.
        let confirm = mapping(&app, "capture_tests.confirm").key;
        let across = conflicts(
            app.world(),
            &Control::PhysicalKey(KeyCode::KeyC).into(),
            Some(confirm),
        );
        assert_eq!(across.len(), 1);
        assert_eq!(across[0].action_path, "capture_tests.crouch");
        assert_eq!(across[0].overlap, ConflictOverlap::OtherContext);

        // Nothing holds this one.
        assert!(
            conflicts(
                app.world(),
                &Control::PhysicalKey(KeyCode::KeyZ).into(),
                Some(jump)
            )
            .is_empty()
        );

        // And a mapping does not conflict with itself, so rebinding a control to where it already
        // is reports nothing rather than reporting the row the player is looking at.
        assert!(
            conflicts(
                app.world(),
                &Control::PhysicalKey(KeyCode::Space).into(),
                Some(jump)
            )
            .is_empty()
        );
    }

    /// A screen's own unconfirmed choice has to be able to clash with another one, and `conflicts`
    /// alone cannot see it because nothing has been applied. A row someone else owns is read the
    /// way applying reads it: neither cleared nor free.
    #[test]
    fn conflicts_pending_sees_what_has_not_been_confirmed() {
        let app = app();
        let mappings = crate::mapping::mappings(app.world());
        let up = mapping(&app, "capture_tests.move.up").key;
        let jump = mapping(&app, "capture_tests.jump").key;

        let mut pending = Overrides::new();
        pending.bind(
            DeviceFamily::KeyboardMouse,
            jump,
            [Control::PhysicalKey(KeyCode::KeyW)],
        );

        // Still on Space in the world, so the world-only query hears nothing.
        assert!(
            conflicts(
                app.world(),
                &Control::PhysicalKey(KeyCode::KeyW).into(),
                Some(up)
            )
            .is_empty()
        );
        let found = conflicts_pending(
            &mappings,
            &pending,
            &Control::PhysicalKey(KeyCode::KeyW).into(),
            Some(up),
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].action_path, "capture_tests.jump");

        let mut pending = Overrides::new();
        pending.set(DeviceFamily::KeyboardMouse, jump, Override::NotOurs);
        assert_eq!(
            conflicts_pending(
                &mappings,
                &pending,
                &Control::PhysicalKey(KeyCode::Space).into(),
                Some(up)
            )
            .len(),
            1,
            "NotOurs leaves the row reading as it did"
        );

        // Contrast with `Cleared`, which does free the control.
        pending.set(DeviceFamily::KeyboardMouse, jump, Override::Cleared);
        assert!(
            conflicts_pending(
                &mappings,
                &pending,
                &Control::PhysicalKey(KeyCode::Space).into(),
                Some(up)
            )
            .is_empty()
        );
    }

    /// `W` and `Ctrl+W` are two presses, so a row may hold one while another holds the other. The
    /// same chord is a clash in whatever order it was written.
    #[test]
    fn a_different_chord_on_the_same_control_is_no_clash() {
        use crate::binding::ModifierKey;
        use crate::present::ControlOrigin;

        let app = app();
        let mappings = crate::mapping::mappings(app.world());
        let jump = mapping(&app, "capture_tests.jump").key;
        let up = mapping(&app, "capture_tests.move.up").key;
        let chorded = |with: &[ModifierKey]| BoundSlot {
            control: Control::PhysicalKey(KeyCode::KeyW),
            with: with.iter().copied().map(ControlOrigin::Modifier).collect(),
        };

        // `move.up` holds a bare W.
        assert!(conflicts(app.world(), &chorded(&[ModifierKey::Ctrl]), Some(jump)).is_empty());

        let mut pending = Overrides::new();
        pending.bind(
            DeviceFamily::KeyboardMouse,
            jump,
            [chorded(&[ModifierKey::Ctrl, ModifierKey::Shift])],
        );
        let found = conflicts_pending(
            &mappings,
            &pending,
            &chorded(&[ModifierKey::Shift, ModifierKey::Ctrl]),
            Some(up),
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].action_path, "capture_tests.jump");
        assert!(
            conflicts_pending(
                &mappings,
                &pending,
                &chorded(&[ModifierKey::Shift]),
                Some(up)
            )
            .is_empty()
        );
    }

    /// A class is a property, not a list.
    #[test]
    fn a_control_class_is_decided_by_channel_shape() {
        assert!(ControlClass::AnyButton.contains(Control::PhysicalKey(KeyCode::KeyA)));
        assert!(!ControlClass::AnyButton.contains(Control::MouseMotion));
        assert!(ControlClass::AnyDelta.contains(Control::MouseMotion));

        for (shape, class) in [
            (ChannelShape::Button, ControlClass::AnyButton),
            (ChannelShape::Axis1, ControlClass::AnyAxis),
            (ChannelShape::Axis2, ControlClass::AnyStick),
            (ChannelShape::Delta2, ControlClass::AnyDelta),
        ] {
            assert_eq!(ControlClass::of(shape), class, "{shape:?}");
        }
    }

    /// The same key is a dead key on one press and a plain letter on the next, so the control alone
    /// cannot answer this.
    #[test]
    fn the_character_class_is_decided_by_the_event() {
        let typed = |text: Option<&str>, state: ButtonState| {
            crate::frame::RawEvent::Keyboard(KeyboardInput {
                key_code: KeyCode::KeyA,
                logical_key: Key::Character(text.unwrap_or_default().into()),
                state,
                text: text.map(Into::into),
                repeat: false,
                window: Entity::PLACEHOLDER,
            })
        };

        assert!(ClassFilter::Characters.matches(&typed(Some("a"), ButtonState::Pressed)));
        // A dead key on this press: same `KeyCode`, no text yet.
        assert!(!ClassFilter::Characters.matches(&typed(None, ButtonState::Pressed)));
        // Release is not a choice, the same rule every other binding follows.
        assert!(!ClassFilter::Characters.matches(&typed(Some("a"), ButtonState::Released)));

        // The shape classes read straight off the event's own control, same as `contains`.
        assert!(
            ClassFilter::Shape(ControlClass::AnyButton)
                .matches(&typed(Some("a"), ButtonState::Pressed))
        );
        assert!(
            !ClassFilter::Shape(ControlClass::AnyDelta)
                .matches(&typed(Some("a"), ButtonState::Pressed))
        );
    }

    /// A stick bound whole is a rebinding row like any other: pushing it is what a settings screen
    /// offers, and `Control::GamepadStick` is what comes back — never the bare axis that happened
    /// to cross the threshold first. A trigger deflects too and is not this stick; a continuous
    /// reading that does not fit is passed over in silence rather than complained about.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_pushed_stick_is_captured_whole() {
        use bevy_input::gamepad::{GamepadAxis, RawGamepadAxisChangedEvent};

        #[derive(InputContext)]
        #[context(path = "capture_tests.stick", tick = Render)]
        struct WithStick;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.init_resource::<Heard>();
        app.add_observer(|event: On<ControlCaptured>, mut heard: ResMut<'_, Heard>| {
            heard.captured.push(event.control);
        });
        app.add_observer(|event: On<CaptureRefused>, mut heard: ResMut<'_, Heard>| {
            heard.refused.push((event.control, event.reason));
        });
        app.add_context::<WithStick>(|controls| {
            controls
                .bind::<Move>(crate::binding::Stick::Left)
                .mappable();
        });

        let target = &crate::mapping::mappings(app.world())[0];
        assert_eq!(target.accepts, ChannelShape::Axis2);
        app.world_mut()
            .spawn(CaptureSession::for_mapping(target).expect("a stick mapping"));
        app.update();

        app.world_mut()
            .write_message(bevy_input::gamepad::RawGamepadEvent::Axis(
                RawGamepadAxisChangedEvent::new(Entity::PLACEHOLDER, GamepadAxis::LeftZ, 1.0),
            ));
        app.update();
        let heard = app.world().resource::<Heard>();
        assert!(heard.captured.is_empty(), "a trigger is not this stick");
        assert!(heard.refused.is_empty(), "and is not complained about");

        app.world_mut()
            .write_message(bevy_input::gamepad::RawGamepadEvent::Axis(
                RawGamepadAxisChangedEvent::new(Entity::PLACEHOLDER, GamepadAxis::LeftStickX, 0.8),
            ));
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::GamepadStick(crate::binding::Stick::Left)]
        );
    }

    /// Reserving and declaring a mapping say opposite things about one binding.
    #[test]
    #[should_panic(expected = "both mappable and reserved")]
    fn reserving_a_mappable_binding_is_refused() {
        #[derive(InputContext)]
        #[context(path = "capture_tests.contradictory", tick = Render)]
        struct Contradictory;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<Contradictory>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable().reserved();
        });
    }

    /// Reserving is per family: a reserved key says nothing about the pad.
    #[cfg(feature = "gamepad")]
    #[test]
    fn reserving_is_scoped_to_the_family_it_was_declared_in() {
        use bevy_input::gamepad::GamepadButton;

        let app = app();
        let reserved = app.world().resource::<ReservedControls>();
        assert!(reserved.contains(Control::PhysicalKey(KeyCode::F1)));
        assert!(!reserved.contains(Control::GamepadButton(GamepadButton::Select)));
        assert_eq!(
            reserved
                .claimant(Control::PhysicalKey(KeyCode::F1))
                .unwrap()
                .context,
            "capture_tests.on_foot"
        );
    }
}
