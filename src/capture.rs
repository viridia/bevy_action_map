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
//! commands.entity(cell).observe(|captured: On<Captured>, world: &World| {
//!     let name = captured.control.fallback_label();
//!     let clashes = conflicts(world, captured.control, captured.mapping);
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
//! - **Shape and scheme.** A mapping holding a key accepts another key, not a stick axis and not a
//!   gamepad button — the first because the action cannot use it, the second because a rebind is
//!   scoped to one control scheme, and moving a binding across schemes would mean moving it to a
//!   different mapping.
//! - **Excluded** ([`excluding`](CaptureSession::excluding)): the screen's own controls, so it
//!   stays operable while listening. Silent: an excluded control is not being refused, it is busy
//!   doing its normal job, which is how the key that cancels a capture gets through to cancel it.
//! - **Reserved** ([`reserved`](crate::binding::BindingHandle::reserved)): declared on a binding,
//!   global across its scheme. Loud, because a player who just pressed it meant to bind it and is
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
use crate::frame::{InputFrame, RawEvent, Timestamp};
use crate::mapping::{Mapping, MappingKey, Scheme};
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
    /// A keyboard key whose event carries text, once IME composition and dead keys are accounted
    /// for.
    ///
    /// Membership here cannot be read off a [`Control`]: the same key is a dead key on one press
    /// and a plain letter on the next, so what changes is whether that particular
    /// [`KeyboardInput`] carries text, not which key it was. See
    /// [`contains_event`](Self::contains_event), the only place this class can actually be tested.
    CharacterProducing,
}

impl ControlClass {
    /// Whether this control is a member.
    ///
    /// Always `false` for [`CharacterProducing`](Self::CharacterProducing): that class is a
    /// property of the *event* a control produced, not of the control's identity, so no control on
    /// its own is ever a member. Test [`contains_event`](Self::contains_event) instead.
    pub const fn contains(self, control: Control) -> bool {
        matches!(
            (self, control.shape()),
            (Self::AnyButton, ChannelShape::Button)
                | (Self::AnyAxis, ChannelShape::Axis1)
                | (Self::AnyStick, ChannelShape::Axis2)
                | (Self::AnyDelta, ChannelShape::Delta2)
        )
    }

    /// Whether the control that produced `event` is a member, given what actually happened.
    ///
    /// For the three shape-based classes this is [`contains`](Self::contains) on the event's own
    /// control. For [`CharacterProducing`](Self::CharacterProducing) it reads the event itself.
    pub fn contains_event(self, event: &crate::frame::RawEvent) -> bool {
        match self {
            Self::CharacterProducing => character_producing(event),
            _ => event
                .control()
                .is_some_and(|control| self.contains(control)),
        }
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

// Measured against real input with `examples/ime_diagnostic.rs` (macOS), rather than reasoned from
// documentation. A kana input source composed correctly: every keystroke arrived as its own
// `Pressed` `KeyboardInput` with `text: Some(single kana character)`, and the matching `Released`
// always carried `text: None` — no `Pressed` event with `text: None` mid-composition. That is
// exactly what this predicate assumes.
//
// A dead key (Option+I then A, which should compose to `â`) looked like a counterexample at first —
// through this crate's bare diagnostic window it arrived as two independent plain letters, `i` then
// `a` — but the same keystroke through Bevy's own text-input example produced one composed
// character. So the gap was the diagnostic window not having IME composition enabled on it, not a
// shape this predicate fails to handle: wherever composition happens upstream, it already lands as
// one `KeyboardInput` with `text: Some(the composed character)`, single- or multi-character alike,
// which this predicate already recognizes without change.
//
// Gated to `Pressed` so a release never re-fires a class binding — the same rule every other
// binding follows, just stated once here since there is no per-control state to fall back on.
//
// Left genuinely unmeasured: committing a multi-candidate conversion (kana to kanji) via an IME's
// candidate popup. Reasoned rather than measured: it should be fine, since that commit happens
// through ordinary keystrokes this predicate already judges independently. Revisit if that turns
// out wrong.
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
/// Insert it on an entity; the crate fills the answer in as [`Captured`] on that same entity and
/// removes the component. Remove it yourself to cancel.
#[derive(Component, Clone, Debug)]
pub struct CaptureSession {
    mapping: Option<MappingKey>,
    slot: usize,
    accepts: ControlClass,
    scheme: Option<Scheme>,
    excluded: Vec<Control>,
    // `false` until the session has seen one run of the capture system. Arming costs a frame and
    // buys the thing this would otherwise get wrong every time: the press that opened the capture
    // is still in the queue when the session arrives, so a session that read the queue immediately
    // would bind whichever key the player activated the row with.
    armed: bool,
    cursor: Option<Timestamp>,
}

impl CaptureSession {
    /// Listens for a control for this mapping's first slot.
    ///
    /// Takes the shape and the scheme from the mapping, which is what makes a keyboard row accept a
    /// key and not a gamepad button, and a stick row accept a stick pushed whole rather than one of
    /// its axes.
    ///
    /// A mapping holds a list of slots, and this addresses the front of it — the "primary" column
    /// of a table with more than one. Use [`for_slot`](Self::for_slot) for the others.
    pub fn for_mapping(mapping: &Mapping) -> Option<Self> {
        Self::for_slot(mapping, 0)
    }

    /// Listens for a control for one numbered slot of this mapping.
    ///
    /// A mapping holds an ordered list of slots, and a "primary and secondary" table is that list
    /// drawn as columns — so which slot the player activated is what a capture has to carry, or the
    /// answer has nowhere to go but the front of the row.
    ///
    /// Returns `None` for a slot the mapping does not have: past its
    /// [`capacity`](crate::mapping::Mapping::capacity), or more than one past the controls it holds
    /// now. The second is what stops a capture leaving a hole in a list whose *order* is what
    /// primary and secondary mean. It also returns `None` for one the player may not change at all
    /// — see [`Rebinding`](crate::mapping::Rebinding).
    pub fn for_slot(mapping: &Mapping, slot: usize) -> Option<Self> {
        // A mapping the player cannot change has nothing to capture *for*. It is on the screen so
        // they can read it, and a screen that asked anyway would be offering a rebind it could not
        // then apply.
        if !mapping.rebinding.is_rebindable() {
            return None;
        }
        if !mapping.capacity.has_room_for(slot) || slot > mapping.slots.len() {
            return None;
        }
        Some(Self {
            mapping: Some(mapping.key),
            slot,
            ..Self::accepting(ControlClass::of(mapping.accepts)).within(mapping.scheme)
        })
    }

    /// Listens for any control of a class, without a mapping in mind.
    pub fn accepting(class: ControlClass) -> Self {
        Self {
            mapping: None,
            slot: 0,
            accepts: class,
            scheme: None,
            excluded: Vec::new(),
            armed: false,
            cursor: None,
        }
    }

    /// Restricts capture to one control scheme.
    pub fn within(mut self, scheme: Scheme) -> Self {
        self.scheme = Some(scheme);
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

    /// The scheme it is restricted to, if any.
    pub fn scheme(&self) -> Option<Scheme> {
        self.scheme
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
/// in a save file — and one control must get one answer either way. `scheme` is `None` for a
/// capture not restricted to one.
///
/// The order is the order the reasons come in, and reserved is asked before shape so that pressing
/// the settings key is answered with the reason it cannot be bound rather than with a complaint
/// about its channel.
pub(crate) fn admissible(
    control: Control,
    scheme: Option<Scheme>,
    accepts: ControlClass,
    reserved: bool,
) -> Result<(), RefusedReason> {
    if scheme.is_some_and(|scheme| scheme != control.scheme()) {
        return Err(RefusedReason::Scheme);
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
pub struct Captured {
    /// The entity whose capture this was.
    pub entity: Entity,
    /// The mapping it was for, if it was made for one.
    pub mapping: Option<MappingKey>,
    /// Which slot of that mapping the control belongs in.
    ///
    /// Zero unless the session named another, which is what a "primary and secondary" table does.
    /// Carried here because a mapping holds a list: without it, an override has nowhere to go but
    /// front of the row, and the secondary column could never be filled.
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
pub struct Refused {
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
    /// It belongs to a different control scheme than the one being rebound.
    Scheme,
    /// A binding reserved it, so nothing may be bound over it.
    Reserved,
}

/// A mapping that already holds the control in question.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// The mapping that holds it.
    pub mapping: MappingKey,
    /// The declared path of the action that mapping drives.
    pub action_path: &'static str,
    /// The declared path of the context it lives in.
    pub context: &'static str,
    /// Whether the two are certainly in each other's way, or only possibly.
    pub overlap: Overlap,
}

/// How much of a problem a [`Conflict`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlap {
    /// Both mappings are in one context, so both are always live together and the clash is real.
    SameContext,
    /// The mappings are in different contexts, which may never be active at the same time — a menu
    /// key and a gameplay key can share a control quite deliberately. Whether this matters is a
    /// question about the game's own activation rules, which this crate does not know.
    OtherContext,
}

/// Which mappings already hold a control.
///
/// This can be answered before anything is committed to: a screen calls this with what capture
/// just reported and decides what to say. Deciding what to *do* — reject, swap, unbind the
/// other — needs somewhere to write the answer, which is a separate matter.
///
/// `target` is the mapping being rebound, and is excluded from the result: a mapping does not
/// conflict with itself, and rebinding a control to where it already is should report nothing. The
/// whole mapping is excluded rather than the one slot, so putting a control in a row's second slot
/// while its first already holds it is not reported here — a repeat *within* one row is a question
/// for the conflict policy that applies a rebind, not for the detection that precedes it.
///
/// Conflicts are per scheme, so a keyboard binding never clashes with a gamepad one.
/// Comparison is at control granularity: two bindings that share a control but differ in their
/// chords are reported as an overlap even though arbitration would separate them. That errs toward
/// telling a player about something harmless rather than staying quiet about something real.
pub fn conflicts(world: &World, control: Control, target: Option<MappingKey>) -> Vec<Conflict> {
    conflicts_in(&crate::mapping::mappings(world), None, control, target)
}

/// Which mappings already hold a control, as a screen's own unconfirmed choices would leave things.
///
/// Same question as [`conflicts`], against a working copy rather than what is currently applied — a
/// settings screen holds its player's choices in a [`Overrides`] of its own until they confirm, and a
/// choice that has not been confirmed yet still has to be able to clash with another one that hasn't
/// either. `mappings` is the applied baseline (as `crate::mapping::mappings` returns), and `pending`
/// is laid over it: a row `pending` names reads as that row says, and everything else reads as
/// `mappings` already has it.
///
/// A backend-owned row (`Override::NotOurs` in `pending`) reads as `mappings` already has it
/// (unaffected, not cleared), matching how [`crate::overrides::apply_overrides`] treats it.
///
/// Resolving a conflict this finds is the caller's decision, made with [`Overrides::bind`] and
/// [`Overrides::get`] directly rather than through another crate API. A caller can refuse the
/// conflict by not writing the candidate row at all, allow the duplicate by writing it regardless,
/// or read the conflicting row's current list the same way this function does —
/// `pending.get(mapping.scheme, mapping.key)` falling back to `mapping.slots` — and `bind` it back
/// with the shared control removed, or with the candidate's own previous control put in its place
/// to trade the two. That same look at a row's own candidate list, before writing it, is how a
/// caller notices it would hold one control twice: that case never reaches this function, because a
/// mapping never conflicts with itself.
pub fn conflicts_pending(
    mappings: &[Mapping],
    pending: &Overrides,
    control: Control,
    target: Option<MappingKey>,
) -> Vec<Conflict> {
    conflicts_in(mappings, Some(pending), control, target)
}

/// The shared walk behind `conflicts` and `conflicts_pending`.
///
/// `pending` is `None` for the world-only form; `Some` layers a working copy over `mappings` before
/// asking the same question, which is why both forms produce identical results for identical inputs.
fn conflicts_in(
    mappings: &[Mapping],
    pending: Option<&Overrides>,
    control: Control,
    target: Option<MappingKey>,
) -> Vec<Conflict> {
    let target_context = target.and_then(|key| {
        mappings
            .iter()
            .find(|mapping| mapping.key == key)
            .map(|mapping| mapping.context)
    });

    mappings
        .iter()
        .filter(|mapping| {
            Some(mapping.key) != target && effective_slots(mapping, pending).contains(&control)
        })
        .map(|mapping| Conflict {
            mapping: mapping.key,
            action_path: mapping.action_path,
            context: mapping.context,
            overlap: if Some(mapping.context) == target_context {
                Overlap::SameContext
            } else {
                Overlap::OtherContext
            },
        })
        .collect()
}

/// What a mapping currently holds: `pending`'s row for it if there is one, else its own slots.
///
/// A row absent from `pending` means untouched (the common case, so borrowed rather than cloned); a
/// `NotOurs` row means the same, since something else owns it and this crate neither fills it in nor
/// reads it as cleared.
fn effective_slots<'a>(mapping: &'a Mapping, pending: Option<&'a Overrides>) -> &'a [Control] {
    match pending.and_then(|pending| pending.get(mapping.scheme, mapping.key)) {
        Some(Override::Controls(controls)) => controls,
        Some(Override::Cleared) => &[],
        Some(Override::NotOurs) | None => &mapping.slots,
    }
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
                control: Control::Key(key.key_code),
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
        #[cfg(feature = "keyboard")]
        RawEvent::FocusLost => None,
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
            // business at all, which is what lets it go on doing its job while a capture is live.
            if session.excluded.contains(&arrival.control) {
                continue;
            }

            if let Err(reason) = admissible(
                arrival.control,
                session.scheme,
                session.accepts,
                reserved.contains(arrival.control),
            ) {
                // Claimed even though it was refused: the player pressed it at a rebinding screen,
                // and whatever it would otherwise have done is not what they meant.
                consumed.claim_for_capture(arrival.control);
                if arrival.deliberate {
                    commands.trigger(Refused {
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
            commands.trigger(Captured {
                entity,
                mapping: session.mapping,
                slot: session.slot,
                control: arrival.control,
            });
            break;
        }
    }
}

#[cfg(all(test, feature = "keyboard"))]
mod tests {
    use super::*;

    use alloc::vec;
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
    #[action(path = "capture_tests.settings", output = bool, intent = Button)]
    struct OpenSettings;

    #[derive(InputContext)]
    #[context(path = "capture_tests.on_foot", tick = Render)]
    struct OnFoot;

    /// Everything a captured or refused control was reported as, in order.
    #[derive(Resource, Default)]
    struct Heard {
        captured: Vec<Control>,
        slots: Vec<usize>,
        refused: Vec<(Control, RefusedReason)>,
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.init_resource::<Heard>();
        app.add_observer(|event: On<Captured>, mut heard: ResMut<'_, Heard>| {
            heard.captured.push(event.control);
            heard.slots.push(event.slot);
        });
        app.add_observer(|event: On<Refused>, mut heard: ResMut<'_, Heard>| {
            heard.refused.push((event.control, event.reason));
        });
        app.add_context::<OnFoot>(|controls| {
            controls
                .bind::<Move>(crate::binding::DirectionalButtons::wasd())
                .mappable();
            // Room for a secondary, with only the primary shipped — so the tests below have both
            // a full slot and an empty one to aim at.
            controls.bind::<Jump>(KeyCode::Space).mappable_upto(2);
            controls.bind::<OpenSettings>(KeyCode::F1).reserved();
        });
        app
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut().write_message(KeyboardInput {
            key_code: key,
            logical_key: Key::Space,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }

    fn mapping(app: &App, key: &str) -> Mapping {
        crate::mapping::mappings(app.world())
            .into_iter()
            .find(|mapping| alloc::string::ToString::to_string(&mapping.key) == key)
            .expect("no such mapping")
    }

    /// The whole point: what comes back is the control's identity, which a binding would have
    /// turned into a value and discarded.
    #[test]
    fn a_capture_reports_the_control_that_was_pressed() {
        let mut app = app();
        let target = mapping(&app, "capture_tests.move.up");
        let row = app
            .world_mut()
            .spawn(CaptureSession::for_mapping(&target).expect("a button mapping"))
            .id();

        // The frame it arms in takes nothing, which is what stops it binding the key that opened it.
        press(&mut app, KeyCode::Enter);
        app.update();
        assert!(app.world().resource::<Heard>().captured.is_empty());
        assert!(app.world().get::<CaptureSession>(row).is_some());

        press(&mut app, KeyCode::KeyT);
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::Key(KeyCode::KeyT)]
        );
        // Answered once, and the component is gone — which is how a screen knows it has stopped
        // listening without being told separately.
        assert!(app.world().get::<CaptureSession>(row).is_none());
    }

    /// The reason capture reads the frame rather than a binding: no context is spawned here at
    /// all, and capture does not notice.
    #[test]
    fn capture_works_with_no_context_spawned() {
        let mut app = app();
        assert!(crate::mapping::mappings(app.world()).len() > 1);

        app.world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton));
        app.update();
        press(&mut app, KeyCode::KeyQ);
        app.update();

        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::Key(KeyCode::KeyQ)]
        );
    }

    /// Reserving would be worth little if the screen key merely had no mapping of its own —
    /// anything else could still be bound over the top of it.
    #[test]
    fn a_reserved_control_is_refused_out_loud() {
        let mut app = app();
        let target = mapping(&app, "capture_tests.jump");
        app.world_mut()
            .spawn(CaptureSession::for_mapping(&target).expect("a button mapping"));
        app.update();

        press(&mut app, KeyCode::F1);
        app.update();

        assert!(app.world().resource::<Heard>().captured.is_empty());
        assert_eq!(
            app.world().resource::<Heard>().refused,
            [(Control::Key(KeyCode::F1), RefusedReason::Reserved)]
        );

        // And the session is still listening, so the player can pick something else.
        press(&mut app, KeyCode::KeyE);
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::Key(KeyCode::KeyE)]
        );
    }

    /// A control can be refusable twice over, and the reason it gets is the one it is owed: a
    /// player who pressed the settings key wants to hear that it is spoken for, not that its
    /// channel is wrong. Tested on the predicate rather than through capture because overrides
    /// answers a saved file from the same rule, and the two must not disagree.
    #[test]
    fn reserved_answers_before_shape() {
        assert_eq!(
            admissible(Control::Key(KeyCode::F1), None, ControlClass::AnyAxis, true),
            Err(RefusedReason::Reserved)
        );
    }

    /// An excluded control is not refused, it is invisible — which is what lets the key that
    /// cancels a capture reach the thing that cancels it.
    #[test]
    fn an_excluded_control_is_passed_over_in_silence() {
        let mut app = app();
        app.world_mut().spawn(
            CaptureSession::accepting(ControlClass::AnyButton)
                .excluding([Control::Key(KeyCode::Escape)]),
        );
        app.update();

        press(&mut app, KeyCode::Escape);
        app.update();

        let heard = app.world().resource::<Heard>();
        assert!(heard.captured.is_empty());
        assert!(heard.refused.is_empty(), "silent, not refused");
    }

    /// A mapping is rebound within its scheme, so the pad cannot answer for the keyboard.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_control_from_the_other_scheme_is_refused() {
        use bevy_input::gamepad::{GamepadButton, RawGamepadButtonChangedEvent};

        let mut app = app();
        let target = mapping(&app, "capture_tests.jump");
        app.world_mut()
            .spawn(CaptureSession::for_mapping(&target).expect("a button mapping"));
        app.update();

        app.world_mut()
            .write_message(bevy_input::gamepad::RawGamepadEvent::Button(
                RawGamepadButtonChangedEvent::new(Entity::PLACEHOLDER, GamepadButton::South, 1.0),
            ));
        app.update();

        assert_eq!(
            app.world().resource::<Heard>().refused,
            [(
                Control::GamepadButton(GamepadButton::South),
                RefusedReason::Scheme
            )]
        );
    }

    /// What the player presses at a rebinding screen must not also play the game.
    #[test]
    fn a_captured_control_is_taken_from_the_game() {
        let mut app = app();
        app.world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton));
        app.update();

        press(&mut app, KeyCode::Space);
        app.update();

        assert_eq!(
            app.world()
                .resource::<crate::eval::ConsumedControls>()
                .claimant(Control::Key(KeyCode::Space)),
            Some("capture")
        );
    }

    #[test]
    fn conflicts_name_the_slots_that_already_hold_a_control() {
        let app = app();
        let jump = mapping(&app, "capture_tests.jump").key;

        let found = conflicts(app.world(), Control::Key(KeyCode::KeyW), Some(jump));
        assert_eq!(found.len(), 1);
        assert_eq!(
            alloc::string::ToString::to_string(&found[0].mapping),
            "capture_tests.move.up"
        );
        assert_eq!(found[0].action_path, "capture_tests.move");
        assert_eq!(
            found[0].overlap,
            Overlap::SameContext,
            "both are bound in on_foot, so they are certainly in each other's way"
        );

        // Nothing holds this one.
        assert!(conflicts(app.world(), Control::Key(KeyCode::KeyZ), Some(jump)).is_empty());

        // And a mapping does not conflict with itself, so rebinding a control to where it already
        // reports nothing rather than reporting the row the player is looking at.
        assert!(conflicts(app.world(), Control::Key(KeyCode::Space), Some(jump)).is_empty());
    }

    /// A row holds a list, so *any* slot of it holding the control is a clash — a secondary binding
    /// is no less bound than a primary one.
    #[test]
    fn a_conflict_is_found_in_any_slot_of_a_row() {
        #[derive(InputContext)]
        #[context(path = "capture_tests.two_defaults", tick = Render)]
        struct TwoDefaults;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<TwoDefaults>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(KeyCode::Enter).mappable();
            controls.bind::<OpenSettings>(KeyCode::F1).mappable();
        });

        let settings = crate::mapping::mappings(app.world())[1].key;
        // The secondary, which a `==` against a single control would have missed.
        let found = conflicts(app.world(), Control::Key(KeyCode::Enter), Some(settings));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].action_path, "capture_tests.jump");
    }

    /// The whole point of a pending-aware query: a screen's own unconfirmed choice has to be able to
    /// clash with another one, and `conflicts` alone cannot see it because nothing has been applied.
    #[test]
    fn conflicts_pending_sees_a_row_the_player_has_not_confirmed_yet() {
        let app = app();
        let mappings = crate::mapping::mappings(app.world());
        let up = mapping(&app, "capture_tests.move.up").key;
        let jump = mapping(&app, "capture_tests.jump").key;

        let mut pending = Overrides::new();
        pending.bind(Scheme::KeyboardMouse, jump, [Control::Key(KeyCode::KeyW)]);

        // Still on Space in the world, so the world-only query hears nothing.
        assert!(conflicts(app.world(), Control::Key(KeyCode::KeyW), Some(up)).is_empty());

        let found = conflicts_pending(&mappings, &pending, Control::Key(KeyCode::KeyW), Some(up));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].action_path, "capture_tests.jump");
    }

    /// Read the same way applying does: a row someone else owns is neither cleared nor untouched,
    /// and a pending `NotOurs` must not read as freeing up its control.
    #[test]
    fn a_pending_not_ours_row_still_holds_its_control() {
        let app = app();
        let mappings = crate::mapping::mappings(app.world());
        let jump = mapping(&app, "capture_tests.jump").key;
        let up = mapping(&app, "capture_tests.move.up").key;

        let mut pending = Overrides::new();
        pending.set(Scheme::KeyboardMouse, jump, Override::NotOurs);
        let found = conflicts_pending(&mappings, &pending, Control::Key(KeyCode::Space), Some(up));
        assert_eq!(found.len(), 1, "NotOurs leaves the row reading as it did");
        assert_eq!(found[0].action_path, "capture_tests.jump");

        // Contrast with `Cleared`, which does free the control.
        pending.set(Scheme::KeyboardMouse, jump, Override::Cleared);
        assert!(
            conflicts_pending(&mappings, &pending, Control::Key(KeyCode::Space), Some(up))
                .is_empty()
        );
    }

    /// A mapping holds a list, so a capture says which slot it fills — otherwise the answer has
    /// nowhere to go but the front of the row and a secondary column could never be filled.
    #[test]
    fn a_capture_reports_the_slot_it_was_made_for() {
        let mut app = app();
        let target = mapping(&app, "capture_tests.jump");
        assert_eq!(target.slots.len(), 1, "one default…");
        assert_eq!(
            target.capacity,
            crate::mapping::Capacity::UpTo(2),
            "…two slots"
        );

        app.world_mut()
            .spawn(CaptureSession::for_slot(&target, 1).expect("the empty second slot"));
        app.update();
        press(&mut app, KeyCode::KeyK);
        app.update();

        let heard = app.world().resource::<Heard>();
        assert_eq!(heard.captured, [Control::Key(KeyCode::KeyK)]);
        assert_eq!(heard.slots, [1]);
    }

    /// The default, and what a single-column table gets without asking.
    #[test]
    fn a_capture_for_a_mapping_is_a_capture_for_its_first_slot() {
        let app = app();
        let target = mapping(&app, "capture_tests.jump");
        assert_eq!(
            CaptureSession::for_mapping(&target)
                .expect("a button mapping")
                .slot(),
            0
        );
    }

    /// A slot the mapping does not have gets no capture, rather than one whose answer is dropped
    /// or would leave a hole in a list whose order is what primary and secondary mean.
    #[test]
    fn a_slot_the_mapping_does_not_have_has_no_capture() {
        let app = app();
        let jump = mapping(&app, "capture_tests.jump");
        // Two slots, so the third is past the end of the row.
        assert!(CaptureSession::for_slot(&jump, 2).is_none());

        // And one slot, so only the one is addressable — a plain `mappable` said nothing about
        // wanting a second.
        let up = mapping(&app, "capture_tests.move.up");
        assert_eq!(up.capacity, crate::mapping::Capacity::UpTo(1));
        assert!(CaptureSession::for_slot(&up, 0).is_some());
        assert!(CaptureSession::for_slot(&up, 1).is_none());
    }

    /// Capacity is a ceiling, not permission to skip: the next empty slot is reachable and the one
    /// after it is not, because filling that one would leave the slot between them empty for good.
    #[test]
    fn a_capture_cannot_leave_a_hole_in_the_row() {
        #[derive(InputContext)]
        #[context(path = "capture_tests.roomy", tick = Render)]
        struct Roomy;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<Roomy>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable_upto(3);
        });

        let target = &crate::mapping::mappings(app.world())[0];
        assert!(
            CaptureSession::for_slot(target, 1).is_some(),
            "the next one"
        );
        assert!(
            CaptureSession::for_slot(target, 2).is_none(),
            "within capacity, but it would leave slot 2 empty behind it"
        );
    }

    /// The keyboard-and-mouse scheme is one scheme, so a mouse button fills a mapping a key holds.
    /// That is what a player expects of "fire on left click" and what a scheme check would get
    /// wrong if it compared devices rather than schemes.
    #[cfg(feature = "mouse")]
    #[test]
    fn a_mouse_button_can_be_captured_for_a_keyboard_mapping() {
        use bevy_input::mouse::{MouseButton, MouseButtonInput};

        let mut app = app();
        let target = mapping(&app, "capture_tests.jump");
        app.world_mut()
            .spawn(CaptureSession::for_mapping(&target).expect("a button mapping"));
        app.update();

        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::MouseButton(MouseButton::Left)]
        );
    }

    /// A release is not a choice, for the same reason a key's is not: capturing on one would take a
    /// button the player is still holding down.
    #[cfg(feature = "mouse")]
    #[test]
    fn releasing_a_mouse_button_is_not_a_capture() {
        use bevy_input::mouse::{MouseButton, MouseButtonInput};

        let mut app = app();
        app.world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton));
        app.update();

        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Middle,
            state: ButtonState::Released,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        let heard = app.world().resource::<Heard>();
        assert!(heard.captured.is_empty());
        assert!(heard.refused.is_empty());
    }

    /// A class is a property, not a list.
    #[test]
    fn classes_are_decided_by_the_channel_a_control_reports_on() {
        assert!(ControlClass::AnyButton.contains(Control::Key(KeyCode::KeyA)));
        assert!(!ControlClass::AnyButton.contains(Control::MouseMotion));
        assert!(ControlClass::AnyDelta.contains(Control::MouseMotion));

        assert_eq!(
            ControlClass::of(ChannelShape::Button),
            ControlClass::AnyButton
        );
        assert_eq!(
            ControlClass::of(ChannelShape::Axis2),
            ControlClass::AnyStick
        );
    }

    /// `CharacterProducing` cannot be decided from a bare control, so `contains` always says no,
    /// and only `contains_event` can actually answer.
    #[cfg(feature = "keyboard")]
    #[test]
    fn character_producing_is_a_property_of_the_event_not_the_control() {
        assert!(!ControlClass::CharacterProducing.contains(Control::Key(KeyCode::KeyA)));

        let key = |text: Option<&str>, state: ButtonState| {
            crate::frame::RawEvent::Keyboard(KeyboardInput {
                key_code: KeyCode::KeyA,
                logical_key: Key::Character(text.unwrap_or_default().into()),
                state,
                text: text.map(Into::into),
                repeat: false,
                window: Entity::PLACEHOLDER,
            })
        };

        assert!(
            ControlClass::CharacterProducing.contains_event(&key(Some("a"), ButtonState::Pressed))
        );
        // A dead key on this press: same `KeyCode`, no text yet.
        assert!(!ControlClass::CharacterProducing.contains_event(&key(None, ButtonState::Pressed)));
        // Release is not a choice, the same rule every other binding follows.
        assert!(
            !ControlClass::CharacterProducing
                .contains_event(&key(Some("a"), ButtonState::Released))
        );

        // The other three classes read straight off the event's own control, same as `contains`.
        assert!(ControlClass::AnyButton.contains_event(&key(Some("a"), ButtonState::Pressed)));
        assert!(!ControlClass::AnyDelta.contains_event(&key(Some("a"), ButtonState::Pressed)));
    }

    /// A stick bound whole is now a rebinding row like any other: pushing it is what a settings
    /// screen offers, and `Control::GamepadStick` is what comes back — never the bare axis that
    /// happened to cross the threshold first.
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
        app.add_observer(|event: On<Captured>, mut heard: ResMut<'_, Heard>| {
            heard.captured.push(event.control);
        });
        app.add_observer(|event: On<Refused>, mut heard: ResMut<'_, Heard>| {
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
            .spawn(CaptureSession::for_mapping(target).expect("a stick mapping now captures"));
        app.update();

        // A trigger deflects too, and is not this stick: a continuous reading past its threshold
        // is refused in silence, the same as everything else that shape does not fit (`DEFLECTION`
        // and `MOUSE_MOTION`'s own doc explains why nothing is said out loud for these).
        app.world_mut()
            .write_message(bevy_input::gamepad::RawGamepadEvent::Axis(
                RawGamepadAxisChangedEvent::new(Entity::PLACEHOLDER, GamepadAxis::LeftZ, 1.0),
            ));
        app.update();
        let heard = app.world().resource::<Heard>();
        assert!(heard.captured.is_empty());
        assert!(heard.refused.is_empty());

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

    /// Reserving is per scheme: a reserved key says nothing about the pad.
    #[cfg(feature = "gamepad")]
    #[test]
    fn reserving_is_scoped_to_the_scheme_it_was_declared_in() {
        use bevy_input::gamepad::GamepadButton;

        let app = app();
        let reserved = app.world().resource::<ReservedControls>();
        assert!(reserved.contains(Control::Key(KeyCode::F1)));
        assert!(!reserved.contains(Control::GamepadButton(GamepadButton::Select)));
        assert_eq!(
            reserved
                .claimant(Control::Key(KeyCode::F1))
                .unwrap()
                .context,
            "capture_tests.on_foot"
        );
    }

    /// Nothing about capture depends on a mapping, so a game can ask "what did they just press" for
    /// its own reasons.
    #[test]
    fn a_session_without_a_slot_reports_no_slot() {
        let mut app = app();
        app.world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton));
        app.update();
        press(&mut app, KeyCode::KeyB);
        app.update();

        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::Key(KeyCode::KeyB)]
        );
    }

    /// An observer may do anything to the entity it is handed, despawning it included — a settings
    /// row that closes on being answered is an ordinary thing to write.
    ///
    /// This is a guard, not a reproduction: the bug this was written for showed up under
    /// `DefaultPlugins` and not here, because whether an observer's deferred commands run before
    /// or after the ones already queued depends on the executor. The test below is the one that
    /// actually fails without the fix; this one states the contract.
    #[test]
    fn an_observer_may_despawn_the_entity_it_is_answered_on() {
        let mut app = app();
        // Anything the crate does wrong to a despawned entity arrives through the error handler,
        // which warns by default and would let this pass unnoticed.
        app.set_error_handler(bevy_ecs::error::panic);
        app.add_observer(|captured: On<Captured>, mut commands: Commands<'_, '_>| {
            // Deferred rather than inline, which is what an observer wanting the whole world has to
            // do — reading `conflicts` needs `&World` — and is the shape the failure arrived in.
            let entity = captured.entity;
            commands.queue(move |world: &mut World| {
                world.despawn(entity);
            });
        });

        let row = app
            .world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton))
            .id();
        app.update();
        press(&mut app, KeyCode::KeyP);
        app.update();

        assert!(app.world().get_entity(row).is_err(), "the observer had it");
        // And a second frame, in case anything was left queued against it.
        app.update();
    }

    /// The component is gone by the time the observer runs, so "is this row still listening" reads
    /// the same inside the observer as anywhere else.
    #[test]
    fn the_session_is_already_removed_when_the_observer_runs() {
        #[derive(Resource, Default)]
        struct StillThere(Option<bool>);

        let mut app = app();
        app.init_resource::<StillThere>();
        app.add_observer(
            |captured: On<Captured>,
             sessions: Query<'_, '_, &CaptureSession>,
             mut seen: ResMut<'_, StillThere>| {
                seen.0 = Some(sessions.get(captured.entity).is_ok());
            },
        );

        app.world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton));
        app.update();
        press(&mut app, KeyCode::KeyR);
        app.update();

        assert_eq!(app.world().resource::<StillThere>().0, Some(false));
    }

    /// Cancelling is removing the component, and a cancelled session hears nothing more.
    #[test]
    fn removing_the_component_cancels() {
        let mut app = app();
        let row = app
            .world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton))
            .id();
        app.update();

        app.world_mut().entity_mut(row).remove::<CaptureSession>();
        press(&mut app, KeyCode::KeyM);
        app.update();

        assert!(app.world().resource::<Heard>().captured.is_empty());
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
            vec![Control::Key(KeyCode::KeyN); 2]
        );
        assert!(app.world().get::<CaptureSession>(first).is_none());
        assert!(app.world().get::<CaptureSession>(second).is_none());
    }
}
