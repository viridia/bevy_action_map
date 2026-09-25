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
//! // The player activated a table cell on the settings screen. The session takes the shape of
//! // control the mapping can hold, so a stick row accepts a stick and a button row a button.
//! commands.entity(cell).insert(CaptureSession::for_mapping(&mapping));
//!
//! // …and the crate answers on that same entity, once.
//! commands.entity(cell).observe(|captured: On<ControlCaptured>, world: &World| {
//!     let name = captured.control.fallback_label();
//!     // Which row and cell this was for is whatever the screen put on the entity.
//! });
//! ```
//!
//! The session is a component so that "which row is listening" is answered by where the component
//! is, rather than by a screen holding that state beside a global session and keeping the two in
//! step. Put it on whatever entity the answer is useful on — usually the widget that will show it.
//! Removing the component cancels the capture; the crate removes it itself once something is taken.
//!
//! # It reports, it does not judge
//!
//! A capture ends on the control the player chose. Whether that control may go where the screen
//! means to put it is a separate question with one answer, and
//! [`Rebind::checked`](crate::overrides::Rebind::checked) is where a screen asks it — a key offered
//! for a gamepad row, a reserved control, a row the game marked fixed, a row grown past the ceiling
//! the game set. Asking there means a press and a loaded save file get the same answer, and a
//! screen can say what is wrong while the player is still looking at the cell they pressed.
//!
//! Two kinds of arrival never end a capture:
//!
//! - **Excluded** ([`excluding`](CaptureSession::excluding)): the screen's own controls, so it
//!   stays operable while listening. An excluded control is not being refused — it is busy doing
//!   its normal job, which is how the key that cancels a capture gets through to cancel it.
//! - **A reading nobody chose.** A stick at rest is not quite at rest and a hand on the desk moves
//!   the mouse, so a session listening for a button ignores a deflection past [`DEFLECTION`] or a
//!   twitch past [`MOUSE_MOTION`]. A session listening for that kind of control takes it, because
//!   for a stick row the deflection *is* the answer.

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
    accepts: ControlClass,
    excluded: Vec<Control>,
    // `false` until the session has seen one run of the capture system. Arming costs a frame and
    // buys the thing this would otherwise get wrong every time: the press that opened the capture
    // is still in the queue when the session arrives, so a session that read the queue immediately
    // would bind whichever key the player activated the row with.
    armed: bool,
    cursor: Option<FrameTimestamp>,
    // The control the player is holding down, which the capture will answer with once they let go.
    // A claim is an instant and a held control is a level, so a session that answered on the press
    // and vanished would leave the control down with nothing claiming it — and the game underneath
    // would read it on the very next frame. Holding it here is what lets the claim last as long as
    // the press does.
    pending: Option<Control>,
}

impl CaptureSession {
    /// Listens for a control this mapping could hold.
    ///
    /// Takes the shape from the mapping, which is what makes a stick row accept a stick pushed
    /// whole rather than one of its axes, and what keeps a drifting pad from answering a button
    /// row.
    ///
    /// Where the answer *goes* is the screen's to track. A mapping holds an ordered list of slots
    /// and a "primary and secondary" table is that list drawn as columns, so which row and which
    /// cell a capture is for is whatever the screen put on the entity it listens on — the answer
    /// comes back there.
    pub fn for_mapping(mapping: &ActionMapping) -> Self {
        Self::accepting(ControlClass::of(mapping.accepts))
    }

    /// Listens for any control of a class, without a mapping in mind.
    pub fn accepting(class: ControlClass) -> Self {
        Self {
            accepts: class,
            excluded: Vec::new(),
            armed: false,
            cursor: None,
            pending: None,
        }
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

    /// The class of control it will take.
    pub fn accepts(&self) -> ControlClass {
        self.accepts
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
/// Asked once, where a row is stored, so a control pressed at a rebinding screen and the same
/// control loaded from a save file cannot get different answers. `family` is `None` where a row is
/// not scoped to one.
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
/// Fired on the entity that carried the [`CaptureSession`], which is then removed — so which row
/// and cell this answers is whatever the screen put on that entity. One press, one answer: the
/// session ends whether or not the control turns out to be one the row may hold.
///
/// Nothing has been rebound. Whether the control may go where the screen means to put it is
/// [`Rebind::checked`](crate::overrides::Rebind::checked), and what it clashes with is
/// [`conflicts`].
#[derive(EntityEvent, Clone, Debug)]
pub struct ControlCaptured {
    /// The entity whose capture this was.
    pub entity: Entity,
    /// The control the player chose.
    pub control: Control,
}

/// Why a control may not fill a slot.
///
/// Internal: [`OverrideProblemKind`](crate::overrides::OverrideProblemKind) is what a caller sees,
/// and it says more — a row past the ceiling, a chord entry nobody can hold, a row the player may
/// not change at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RefusedReason {
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
/// A row absent from `pending` means untouched.
///
/// An empty slot holds nothing rather than holding "no control", so two rows with a gap apiece are
/// not a clash.
fn holds(mapping: &ActionMapping, pending: Option<&Overrides>, candidate: &BoundSlot) -> bool {
    let slots = match pending.and_then(|pending| pending.get(mapping.family, mapping.key)) {
        Some(Override::Slots(slots)) => slots,
        Some(Override::Cleared) => return false,
        None => &mapping.slots,
    };
    slots
        .iter()
        .flatten()
        .any(|slot| slot.clashes_with(candidate))
}

/// One control arriving, and what kind of arrival it is.
struct Arrival {
    control: Control,
    /// True for a press, false for a continuous reading that crossed its threshold. A reading
    /// nobody chose only answers a session listening for that class.
    deliberate: bool,
    /// Whether the control is still down once this event has been read, so the capture has to wait
    /// for it to come up. True of everything with a resting position; false of the mouse's motion,
    /// which is a displacement that has already finished and is never "held".
    holds: bool,
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
        // The press is what latches; `releases` below is what ends the capture. `repeat` is
        // neither, and must not read as a second press of a key already latched.
        #[cfg(feature = "keyboard")]
        RawEvent::Keyboard(key) => (key.state == bevy_input::ButtonState::Pressed && !key.repeat)
            .then_some(Arrival {
                control: Control::PhysicalKey(key.key_code),
                deliberate: true,
                holds: true,
            }),
        // A press, like a key: the player meant it.
        #[cfg(feature = "mouse")]
        RawEvent::MouseButton(button) => (button.state == bevy_input::ButtonState::Pressed)
            .then_some(Arrival {
                control: Control::MouseButton(button.button),
                deliberate: true,
                holds: true,
            }),
        RawEvent::MouseMotion(delta) => (delta.length() >= MOUSE_MOTION).then_some(Arrival {
            control: Control::MouseMotion,
            deliberate: false,
            // The one control with nothing to wait for: a motion is a displacement that has already
            // happened, so there is no level left for the game underneath to read.
            holds: false,
        }),
        #[cfg(feature = "gamepad")]
        RawEvent::Gamepad(event) => match event {
            // Our own threshold rather than whatever the backend synthesized, for the same reason
            // the evaluator uses its own (R14.2).
            bevy_input::gamepad::RawGamepadEvent::Button(button) => {
                threshold.pressed(button.value, false).then_some(Arrival {
                    control: Control::GamepadButton(button.button),
                    deliberate: true,
                    holds: true,
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
                        holds: true,
                    }
                }),
            bevy_input::gamepad::RawGamepadEvent::Connection(_) => None,
        },
        // Losing focus never arrives as a control a player meant to bind.
        #[cfg(any(feature = "keyboard", feature = "mouse"))]
        RawEvent::FocusLost => None,
    }
}

/// Whether this event says the control a session latched has come back up.
///
/// The mirror of [`arrival`], and written out rather than derived from it: a key `repeat` is not an
/// arrival either, and reading one as a release would answer the capture while the player still had
/// the key down — which is the whole thing the wait exists to prevent.
fn releases(event: &RawEvent, control: Control, threshold: &ButtonThreshold) -> bool {
    #[cfg(not(feature = "gamepad"))]
    let _ = threshold;

    match (event, control) {
        #[cfg(feature = "keyboard")]
        (RawEvent::Keyboard(key), Control::PhysicalKey(code)) => {
            key.key_code == code && key.state == bevy_input::ButtonState::Released
        }
        #[cfg(feature = "mouse")]
        (RawEvent::MouseButton(button), Control::MouseButton(which)) => {
            button.button == which && button.state == bevy_input::ButtonState::Released
        }
        #[cfg(feature = "gamepad")]
        (RawEvent::Gamepad(event), control) => match (event, control) {
            (
                bevy_input::gamepad::RawGamepadEvent::Button(button),
                Control::GamepadButton(which),
            ) => button.button == which && !threshold.pressed(button.value, true),
            (bevy_input::gamepad::RawGamepadEvent::Axis(axis), Control::GamepadAxis(which)) => {
                axis.axis == which && axis.value.abs() < DEFLECTION
            }
            // A stick is two axes and this watches whichever one reports. Letting go of one while
            // still holding the other ends the capture early, which costs nothing: the answer was
            // the stick either way, and the axis still deflected is claimed for that one frame.
            (bevy_input::gamepad::RawGamepadEvent::Axis(axis), Control::GamepadStick(stick)) => {
                crate::binding::Stick::containing(axis.axis) == Some(stick)
                    && axis.value.abs() < DEFLECTION
            }
            _ => false,
        },
        // Every control comes up when the window stops hearing them, and the evaluator clears its
        // held state to match. A session that kept waiting would never see the release, because it
        // is going to another window.
        #[cfg(any(feature = "keyboard", feature = "mouse"))]
        (RawEvent::FocusLost, _) => true,
        _ => false,
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
    mut consumed: ResMut<'_, crate::eval::ConsumedControls>,
    mut sessions: Query<'_, '_, (Entity, &mut CaptureSession, Option<&crate::player::Paired>)>,
) {
    for (entity, mut session, pairing) in &mut sessions {
        // A rebinding row on one player's pane answers to that player's devices only, or to every
        // device when nothing paired it.
        let devices = pairing.map(|paired| &**paired);
        if !session.armed {
            session.armed = true;
            session.cursor = frame.latest();
            continue;
        }

        // Re-claimed every frame for as long as the player holds it, which is the point of holding
        // it at all: the claim has to last as long as the press, or the game underneath reads the
        // control the moment this session stops claiming it.
        if let Some(pending) = session.pending {
            consumed.claim_for_capture(pending, devices);
        }

        for event in frame.events_after(session.cursor) {
            // Advanced past another player's event too, so the cursor means "looked at" rather than
            // "was allowed to see".
            session.cursor = Some(event.timestamp);
            if devices.is_some_and(|set| !set.contains(event.event.device())) {
                continue;
            }

            // Waiting on a control: the only thing worth reading is that control coming up.
            // Everything else the player does meanwhile is ignored rather than latched, so a second
            // press cannot change the answer out from under the first.
            if let Some(pending) = session.pending {
                if !releases(&event.event, pending, &threshold) {
                    continue;
                }
                answer(&mut commands, entity, pending);
                break;
            }

            let Some(arrival) = arrival(&event.event, &threshold, session.accepts) else {
                continue;
            };

            // Unconditional, and asked first: an excluded control is not capture's business at all.
            if session.excluded.contains(&arrival.control) {
                continue;
            }

            // A reading nobody chose only answers a session that wants that kind of control. For a
            // stick row the deflection *is* the answer; for a button row it is a pad on a desk, and
            // a capture that ended on it would cancel itself before the player touched anything.
            //
            // A deliberate press answers whatever the session is, admissible or not: one press, one
            // answer, and `Rebind::checked` is where the screen finds out which.
            if !arrival.deliberate && !session.accepts.contains(arrival.control) {
                continue;
            }

            // Claimed whether or not the row can hold it: the player pressed this at a rebinding
            // screen, and whatever it would otherwise have done is not what they meant.
            consumed.claim_for_capture(arrival.control, devices);
            if arrival.holds {
                session.pending = Some(arrival.control);
                continue;
            }
            answer(&mut commands, entity, arrival.control);
            break;
        }
    }
}

/// Hands one session's answer over and takes the session away.
///
/// Removed *before* the event, and both halves of that matter. An observer is entitled to do
/// anything to this entity, despawning it included — a settings row that closes on being answered
/// is an ordinary thing to write — so the crate must have finished with the entity before it hands
/// it over. It also means the observer sees the component already gone, so "is this row still
/// listening" reads the same from inside the observer as from anywhere else.
///
/// Fallible because one run can answer several sessions, and the first observer to run may despawn
/// a later one's entity.
fn answer(commands: &mut Commands<'_, '_>, entity: Entity, control: Control) {
    commands.entity(entity).try_remove::<CaptureSession>();
    commands.trigger(ControlCaptured { entity, control });
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

    /// Every control a capture reported, in order.
    #[derive(Resource, Default)]
    struct Heard {
        captured: Vec<Control>,
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.init_resource::<Heard>();
        app.add_observer(|event: On<ControlCaptured>, mut heard: ResMut<'_, Heard>| {
            heard.captured.push(event.control);
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

    fn release(app: &mut App, code: KeyCode) {
        app.world_mut()
            .write_message(key_event(code, ButtonState::Released));
    }

    /// A whole keystroke: down, a frame, up, a frame. A capture answers on the release, so a test
    /// that only presses is a test of a capture still waiting.
    fn tap(app: &mut App, code: KeyCode) {
        press(app, code);
        app.update();
        release(app, code);
        app.update();
    }

    /// The same, for a gamepad button: to full deflection and back to rest.
    #[cfg(feature = "gamepad")]
    fn tap_pad(app: &mut App, pad: Entity, button: bevy_input::gamepad::GamepadButton) {
        use bevy_input::gamepad::{RawGamepadButtonChangedEvent, RawGamepadEvent};

        for value in [1.0, 0.0] {
            app.world_mut().write_message(RawGamepadEvent::Button(
                RawGamepadButtonChangedEvent::new(pad, button, value),
            ));
            app.update();
        }
    }

    /// The same, for a stick axis: past [`DEFLECTION`] and back to centre.
    #[cfg(feature = "gamepad")]
    fn push_axis(app: &mut App, pad: Entity, axis: bevy_input::gamepad::GamepadAxis, to: f32) {
        use bevy_input::gamepad::{RawGamepadAxisChangedEvent, RawGamepadEvent};

        for value in [to, 0.0] {
            app.world_mut()
                .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                    pad, axis, value,
                )));
            app.update();
        }
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
            .spawn(CaptureSession::for_mapping(&target))
            .id();

        // The arming frame takes no control, which is what stops it binding the key the player
        // opened the row with.
        tap(&mut app, KeyCode::Enter);
        assert!(app.world().resource::<Heard>().captured.is_empty());
        assert!(
            app.world().get::<CaptureSession>(row).is_some(),
            "still listening"
        );

        // Held is not yet answered: the capture waits for the key to come up, so that the control
        // is no longer down when this session stops claiming it.
        press(&mut app, KeyCode::KeyT);
        app.update();
        assert!(
            app.world().resource::<Heard>().captured.is_empty(),
            "still held"
        );
        release(&mut app, KeyCode::KeyT);
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
        tap(&mut app, KeyCode::KeyM);
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
        tap(&mut app, KeyCode::KeyP);

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

        tap(&mut app, KeyCode::KeyN);

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
        }

        // And the session is still listening through all of it.
        tap(&mut app, KeyCode::KeyY);
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
            .spawn(CaptureSession::for_mapping(&target));
        clicking.update();
        for state in [ButtonState::Pressed, ButtonState::Released] {
            clicking.world_mut().write_message(MouseButtonInput {
                button: MouseButton::Left,
                state,
                window: Entity::PLACEHOLDER,
            });
            clicking.update();
        }
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

    /// One press, one answer. A deliberate press ends the capture whether or not the row can hold
    /// it, and the reason comes from the one place that decides: the row being stored.
    ///
    /// The control is still claimed, which is the half that matters for a reserved one — a player
    /// who presses the settings key at a rebinding screen must not open the settings screen.
    #[test]
    fn a_press_the_row_cannot_hold_still_ends_the_capture() {
        use crate::overrides::{OverrideProblemKind, Overrides, Rebind};

        let mut app = app();
        let jump = mapping(&app, "capture_tests.jump");
        let row = app
            .world_mut()
            .spawn(CaptureSession::for_mapping(&jump))
            .id();
        app.update();

        // `F1` opens the settings screen, so nothing may be bound over it.
        tap(&mut app, KeyCode::F1);
        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::PhysicalKey(KeyCode::F1)],
            "reported rather than swallowed"
        );
        assert!(
            app.world().get::<CaptureSession>(row).is_none(),
            "and the capture is over: the screen answers, not the crate"
        );

        let pending = Overrides::new();
        let slots = pending.with_cell(&jump, 0, Control::PhysicalKey(KeyCode::F1));
        assert_eq!(
            Rebind::checked(app.world(), &jump, slots).err(),
            Some(OverrideProblemKind::Reserved {
                control: Control::PhysicalKey(KeyCode::F1)
            }),
            "reserved is answered before shape, so the player hears why rather than about a channel"
        );

        // A mapping is rebound within its family, so the pad cannot answer for the keyboard — and
        // that is the store's judgement now, not the session's.
        #[cfg(feature = "gamepad")]
        {
            use bevy_input::gamepad::GamepadButton;

            let row = app
                .world_mut()
                .spawn(CaptureSession::for_mapping(&jump))
                .id();
            app.update();
            tap_pad(&mut app, Entity::PLACEHOLDER, GamepadButton::South);
            assert!(app.world().get::<CaptureSession>(row).is_none());

            let pad = Control::GamepadButton(GamepadButton::South);
            let slots = pending.with_cell(&jump, 0, pad);
            assert_eq!(
                Rebind::checked(app.world(), &jump, slots).err(),
                Some(OverrideProblemKind::WrongFamily { control: pad })
            );
        }
    }

    /// A menu whose own navigation is bound to the keys being rebound: the case a settings screen
    /// operable from the keyboard always has, and the one an instant claim got wrong.
    ///
    /// The press frame was never the problem. The frame *after* it was: the key is still in the
    /// evaluator's held set, and a session that had answered and gone left nothing claiming it, so
    /// `on_change` saw the direction appear and the table moved under the player.
    #[test]
    fn a_held_control_does_not_act_once_the_capture_has_taken_it() {
        use crate::event::Fired;

        #[derive(InputAction)]
        #[action(path = "capture_tests.navigate", output = bevy_math::Vec2, intent = Directional2)]
        struct Navigate;

        #[derive(InputContext)]
        #[context(path = "capture_tests.nav_menu", tick = Render)]
        struct NavMenu;

        #[derive(Resource, Default)]
        struct Moves(usize);

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.init_resource::<Heard>();
        app.init_resource::<Moves>();
        app.add_observer(|event: On<ControlCaptured>, mut heard: ResMut<'_, Heard>| {
            heard.captured.push(event.control);
        });
        app.add_observer(|_: On<Fired<Navigate>>, mut moves: ResMut<'_, Moves>| {
            moves.0 += 1;
        });
        app.add_context::<NavMenu>(|controls| {
            controls.bind::<Navigate>(crate::binding::DirectionalButtons::arrow_keys());
            controls.combined::<Navigate>().on_change();
        });
        app.world_mut().spawn(NavMenu);
        app.world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton));
        app.update();

        press(&mut app, KeyCode::ArrowUp);
        app.update();
        // Held for a while, as a player who has not let go yet holds it.
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Moves>().0, 0, "while it is held");

        release(&mut app, KeyCode::ArrowUp);
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::PhysicalKey(KeyCode::ArrowUp)]
        );

        app.update();
        assert_eq!(
            app.world().resource::<Moves>().0,
            0,
            "and after: the capture took it, so the menu never saw it"
        );
    }

    /// A pad on a desk drifts and a hand on a desk moves the mouse, so an arrival nobody chose must
    /// not end a capture that was not listening for that kind of control. Without this the drifting
    /// pad cancels every keyboard rebind in the game.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_reading_nobody_chose_leaves_a_button_session_listening() {
        use bevy_input::gamepad::{GamepadAxis, RawGamepadAxisChangedEvent};

        let mut app = app();
        let row = app
            .world_mut()
            .spawn(CaptureSession::accepting(ControlClass::AnyButton))
            .id();
        app.update();

        app.world_mut()
            .write_message(bevy_input::gamepad::RawGamepadEvent::Axis(
                RawGamepadAxisChangedEvent::new(Entity::PLACEHOLDER, GamepadAxis::LeftStickX, 1.0),
            ));
        app.update();
        assert!(app.world().resource::<Heard>().captured.is_empty());
        assert!(
            app.world().get::<CaptureSession>(row).is_some(),
            "still listening: the player has not chosen anything yet"
        );

        tap(&mut app, KeyCode::KeyE);
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
            app.world()
                .resource::<crate::eval::ConsumedControls>()
                .claimant(Control::PhysicalKey(KeyCode::Space), None),
            Some("capture")
        );

        // Still down, and still claimed a frame later. This is the half a claim made only on the
        // press frame would get wrong: the key stays in the evaluator's held set, so a session that
        // had already answered and gone would leave it there for the game to read.
        app.update();
        assert_eq!(
            app.world()
                .resource::<crate::eval::ConsumedControls>()
                .claimant(Control::PhysicalKey(KeyCode::Space), None),
            Some("capture"),
            "the claim lasts as long as the press"
        );

        release(&mut app, KeyCode::Space);
        app.update();
        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::PhysicalKey(KeyCode::Space)]
        );

        // And the frame after the session is gone, with nothing claiming anything.
        app.update();
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
        use bevy_input::gamepad::GamepadButton;

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
        app.world_mut().spawn(CaptureSession::for_mapping(target));
        app.update();

        tap_pad(&mut app, Entity::PLACEHOLDER, GamepadButton::South);
        // A frame past the release, where an instant claim would have let the button through.
        app.update();

        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::GamepadButton(GamepadButton::South)]
        );
        assert_eq!(app.world().resource::<Fires>().0, 0);
    }

    /// Two panes of a split-screen settings screen listening at once. A session paired to a pad
    /// answers to that pad, so neither player's press fills the other's row, and neither claim
    /// reaches the other's game.
    #[cfg(feature = "gamepad")]
    #[test]
    fn two_players_rebinding_at_once_do_not_take_each_others_presses() {
        use crate::device::DeviceHandle;
        use crate::player::Paired;
        use bevy_input::gamepad::{GamepadButton, RawGamepadButtonChangedEvent};

        #[derive(InputContext)]
        #[context(path = "capture_tests.split_pad", tick = Render)]
        struct Pad;

        #[derive(Resource, Default)]
        struct Filled(Vec<(Entity, Control)>);

        let pad_a = Entity::from_bits(1);
        let pad_b = Entity::from_bits(2);

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.init_resource::<Filled>();
        app.add_observer(
            |event: On<ControlCaptured>, mut filled: ResMut<'_, Filled>| {
                filled.0.push((event.entity, event.control));
            },
        );
        app.add_context::<Pad>(|controls| {
            controls.bind::<Jump>(GamepadButton::South).mappable();
        });
        app.world_mut().spawn(Pad);

        let target = crate::mapping::mappings(app.world())[0].clone();
        let session = |app: &mut App, pad| {
            app.world_mut()
                .spawn((
                    CaptureSession::for_mapping(&target),
                    Paired::to(DeviceHandle::Gamepad(pad)),
                ))
                .id()
        };
        let row_a = session(&mut app, pad_a);
        let row_b = session(&mut app, pad_b);
        app.update();

        // Different buttons, so which row got which press is visible in the answer rather than only
        // in the order the two arrived. Both pressed together and both released together, since the
        // answer comes on the release.
        for value in [1.0, 0.0] {
            for (pad, button) in [(pad_a, GamepadButton::East), (pad_b, GamepadButton::North)] {
                app.world_mut()
                    .write_message(bevy_input::gamepad::RawGamepadEvent::Button(
                        RawGamepadButtonChangedEvent::new(pad, button, value),
                    ));
            }
            app.update();
        }
        app.update();

        let mut filled = app.world().resource::<Filled>().0.clone();
        filled.sort_by_key(|&(entity, _)| entity);
        let mut expected = [
            (row_a, Control::GamepadButton(GamepadButton::East)),
            (row_b, Control::GamepadButton(GamepadButton::North)),
        ];
        expected.sort_by_key(|&(entity, _)| entity);
        assert_eq!(filled, expected, "each pane took its own player's press");
    }

    /// A fixed row is capturable, because a capture no longer decides where its answer goes. What
    /// used to be a `None` from the constructor is now an answer from the store, and it says more:
    /// the constructor could only refuse the whole row, and this names the row it refused.
    #[test]
    fn a_fixed_row_captures_and_the_store_turns_it_down() {
        use crate::overrides::{OverrideProblemKind, Overrides, Rebind};

        let mut app = app();
        let settings = mapping(&app, "capture_tests.settings");
        assert!(!settings.rebind_policy.is_rebindable());

        app.world_mut()
            .spawn(CaptureSession::for_mapping(&settings));
        app.update();
        tap(&mut app, KeyCode::KeyK);
        assert_eq!(
            app.world().resource::<Heard>().captured,
            [Control::PhysicalKey(KeyCode::KeyK)],
            "the session was made and it answered"
        );

        let slots = Overrides::new().with_cell(&settings, 0, Control::PhysicalKey(KeyCode::KeyK));
        assert_eq!(
            Rebind::checked(app.world(), &settings, slots).err(),
            Some(OverrideProblemKind::NotRebindable)
        );
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
    /// alone cannot see it because nothing has been applied. A row the player emptied frees what it
    /// held.
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

        // `Cleared` frees the control.
        let mut pending = Overrides::new();
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
        app.add_context::<WithStick>(|controls| {
            controls
                .bind::<Move>(crate::binding::Stick::Left)
                .mappable();
        });

        let target = &crate::mapping::mappings(app.world())[0];
        assert_eq!(target.accepts, ChannelShape::Axis2);
        app.world_mut().spawn(CaptureSession::for_mapping(target));
        app.update();

        app.world_mut()
            .write_message(bevy_input::gamepad::RawGamepadEvent::Axis(
                RawGamepadAxisChangedEvent::new(Entity::PLACEHOLDER, GamepadAxis::LeftZ, 1.0),
            ));
        app.update();
        let heard = app.world().resource::<Heard>();
        assert!(heard.captured.is_empty(), "a trigger is not this stick");

        // Pushed, then let go: a deflected stick is as held as a pressed button, so the answer
        // comes when it returns to centre.
        push_axis(&mut app, Entity::PLACEHOLDER, GamepadAxis::LeftStickX, 0.8);
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
