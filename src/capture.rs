//! "Press a control now": reading a control's identity rather than its value.
//!
//! Every other path through this crate turns a control into a *value* — a bool, an axis, a
//! direction — and throws the control away in the process, because a game wants to know that the
//! player jumped rather than which button they jumped with. Rebinding wants exactly the half that
//! gets discarded, so capture reads the input frame directly instead of going through a binding.
//!
//! That is not an implementation shortcut; it is what makes rebinding work in a game that is not
//! running (R19.5). A main-menu settings screen has no gameplay contexts spawned and no evaluator
//! stepping, and capture does not care: the frame is filled by the sampler either way.
//!
//! ```ignore
//! // The player activated a row on the settings screen.
//! commands.entity(row).insert(CaptureSession::for_slot(&slot));
//!
//! // …and the crate answers on that same entity, once.
//! commands.entity(row).observe(|captured: On<Captured>, world: &World| {
//!     let name = captured.control.fallback_label();
//!     let clashes = conflicts(world, captured.control, captured.slot);
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
//! - **Shape and scheme.** A slot holding a key accepts another key, not a stick axis and not a
//!   gamepad button — the first because the action cannot use it, the second because a rebind is
//!   scoped to one control scheme (R19.7) and moving a binding across schemes would mean moving it
//!   to a different slot.
//! - **Excluded** ([`excluding`](CaptureSession::excluding)): the screen's own controls, so that it
//!   stays operable while listening (R19.2). Silent, and deliberately so — an excluded control is
//!   not being refused, it is busy doing its normal job, which is how the key that cancels a
//!   capture gets through to cancel it.
//! - **Reserved** ([`reserved`](crate::binding::BindingHandle::reserved)): declared on a binding,
//!   global across its scheme. Loud, because a player who just pressed it meant to bind it and is
//!   owed the reason.

use alloc::vec::Vec;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Component, EntityEvent, Query, Res, ResMut, Resource};
use bevy_ecs::world::World;

use crate::action::ChannelShape;
use crate::binding::{ButtonThreshold, Control};
use crate::frame::{InputFrame, RawEvent, Timestamp};
use crate::rebind::{Scheme, Slot, SlotKey};

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
/// This is the shape half of the class vocabulary (R4.9), and it is the language capture filters
/// in. A class is defined by the channel a control reports on, never by an enumeration of
/// `KeyCode` and `GamepadButton` variants — which is what lets a device kind that does not exist
/// yet join a class the day its backend ships, rather than needing to be added to a list here
/// (R11.2).
///
/// The set of classes is closed, per R4.10: a class earns its place only where writing the members
/// out is not reasonable. "Any button-shaped control" qualifies because the device set is open.
/// "The arrow keys" does not — there are four of them, and naming them is clearer.
///
/// **There is no class of two-dimensional controls,** which the roadmap expected there to be. No
/// single control reports a position in two dimensions: a stick is two axes and a directional
/// composite is four buttons. Since a player rebinds one part at a time (R19.9), the case a
/// two-dimensional class would serve never reaches capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ControlClass {
    /// Anything with a pressed sense: keyboard keys, gamepad buttons, and analog triggers, which
    /// report a fraction on the same channel.
    AnyButton,
    /// Any single bipolar axis, such as one half of a stick.
    AnyAxis,
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
                | (Self::AnyDelta, ChannelShape::Delta2)
        )
    }

    /// The class of controls that can fill a slot expecting this channel.
    ///
    /// `None` for [`Axis2`](ChannelShape::Axis2), because no one control reports one — see the note
    /// on this type. A slot that accepts `Axis2` is a stick or a mouse bound whole, which §9.7
    /// gives a tunable rather than a rebinding row.
    pub const fn of(shape: ChannelShape) -> Option<Self> {
        match shape {
            ChannelShape::Button => Some(Self::AnyButton),
            ChannelShape::Axis1 => Some(Self::AnyAxis),
            ChannelShape::Delta2 => Some(Self::AnyDelta),
            ChannelShape::Axis2 => None,
        }
    }
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
/// that opens the settings screen must be refused while capturing for a slot declared anywhere.
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
    slot: Option<SlotKey>,
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
    /// Listens for a control that could fill this slot.
    ///
    /// Takes the shape and the scheme from the slot, which is what makes a keyboard row accept a
    /// key and not a gamepad button. Returns `None` for a slot no single control can fill — a stick
    /// or a mouse bound whole — because there is nothing for capture to offer there.
    pub fn for_slot(slot: &Slot) -> Option<Self> {
        Some(Self::accepting(ControlClass::of(slot.accepts)?).within(slot.scheme)).map(|session| {
            Self {
                slot: Some(slot.key),
                ..session
            }
        })
    }

    /// Listens for any control of a class, without a slot in mind.
    pub fn accepting(class: ControlClass) -> Self {
        Self {
            slot: None,
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
    /// This is what keeps the screen usable while it is listening (R19.2): the control that cancels
    /// a capture has to reach the thing that cancels it, which means capture must neither take it
    /// nor swallow it.
    pub fn excluding(mut self, controls: impl IntoIterator<Item = Control>) -> Self {
        self.excluded.extend(controls);
        self
    }

    /// The slot this capture is for, if it was made for one.
    pub fn slot(&self) -> Option<SlotKey> {
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

    /// What capture should do with a control that just arrived.
    fn verdict(&self, control: Control, reserved: &ReservedControls) -> Verdict {
        // Excluded first, and unconditionally: an excluded control is not capture's business at
        // all, which is what lets it go on doing its job while a capture is live.
        if self.excluded.contains(&control) {
            return Verdict::Ignore;
        }
        if self.scheme.is_some_and(|scheme| scheme != control.scheme()) {
            return Verdict::Refuse(RefusedReason::Scheme);
        }
        // Before the shape check, so that pressing the settings key is answered with the reason it
        // cannot be bound rather than with a complaint about its channel.
        if reserved.contains(control) {
            return Verdict::Refuse(RefusedReason::Reserved);
        }
        if !self.accepts.contains(control) {
            return Verdict::Refuse(RefusedReason::Shape);
        }
        Verdict::Take
    }
}

enum Verdict {
    Take,
    Refuse(RefusedReason),
    Ignore,
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
    /// The slot it was for, if it was made for one.
    pub slot: Option<SlotKey>,
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
    /// The slot it is for, if it was made for one.
    pub slot: Option<SlotKey>,
    /// The control that was refused.
    pub control: Control,
    /// Why it was refused.
    pub reason: RefusedReason,
}

/// Why capture would not take a control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefusedReason {
    /// It reports on a channel the slot's action cannot use.
    Shape,
    /// It belongs to a different control scheme than the one being rebound.
    Scheme,
    /// A binding reserved it, so nothing may be bound over it.
    Reserved,
}

/// A slot that already holds the control in question.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// The slot that holds it.
    pub slot: SlotKey,
    /// The declared path of the action that slot drives.
    pub action_path: &'static str,
    /// The declared path of the context it lives in.
    pub context: &'static str,
    /// Whether the two are certainly in each other's way, or only possibly.
    pub overlap: Overlap,
}

/// How much of a problem a [`Conflict`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlap {
    /// Both slots are in one context, so both are always live together and the clash is real.
    SameContext,
    /// The slots are in different contexts, which may never be active at the same time — a menu
    /// key and a gameplay key can share a control quite deliberately. Whether this matters is a
    /// question about the game's own activation rules, which this crate does not know.
    OtherContext,
}

/// Which slots already hold a control.
///
/// The read-only half of R19.3, and the half that can be answered before anything is committed to:
/// a screen calls this with what capture just reported and decides what to say. Deciding what to
/// *do* — reject, swap, unbind the other — needs somewhere to write the answer, which is a
/// separate matter.
///
/// `target` is the slot being rebound, and is excluded from the result: a slot does not conflict
/// with itself, and rebinding a control to where it already is should report nothing.
///
/// Conflicts are per scheme, so a keyboard binding never clashes with a gamepad one (R19.3).
/// Comparison is at control granularity: two bindings that share a control but differ in their
/// chords are reported as an overlap even though arbitration would separate them. That errs toward
/// telling a player about something harmless rather than staying quiet about something real.
pub fn conflicts(world: &World, control: Control, target: Option<SlotKey>) -> Vec<Conflict> {
    let slots = crate::rebind::slots(world);
    let target_context = target.and_then(|key| {
        slots
            .iter()
            .find(|slot| slot.key == key)
            .map(|slot| slot.context)
    });

    slots
        .iter()
        .filter(|slot| slot.current == control && Some(slot.key) != target)
        .map(|slot| Conflict {
            slot: slot.key,
            action_path: slot.action_path,
            context: slot.context,
            overlap: if Some(slot.context) == target_context {
                Overlap::SameContext
            } else {
                Overlap::OtherContext
            },
        })
        .collect()
}

/// One control arriving, and whether the player meant it.
struct Arrival {
    control: Control,
    /// True for a press, false for a continuous reading that crossed its threshold. Only a
    /// deliberate arrival is worth refusing out loud.
    deliberate: bool,
}

/// Turns one raw event into the control a player would say they just used, if any.
fn arrival(event: &RawEvent, threshold: &ButtonThreshold) -> Option<Arrival> {
    #[cfg(not(feature = "gamepad"))]
    let _ = threshold;

    match event {
        // Presses only. Capturing on release would let go of a key the player is still holding, and
        // `repeat` would bind the same key several times over while they waited.
        #[cfg(feature = "keyboard")]
        RawEvent::Keyboard(key) => (key.state == bevy_input::ButtonState::Pressed && !key.repeat)
            .then_some(Arrival {
                control: Control::Key(key.key_code),
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
                .then_some(Arrival {
                    control: Control::GamepadAxis(axis.axis),
                    deliberate: false,
                }),
            bevy_input::gamepad::RawGamepadEvent::Connection(_) => None,
        },
    }
}

/// Reads the frame on behalf of every live capture session.
///
/// Runs between sampling and evaluation, which is what lets it claim what it saw before any context
/// gets to act on it (R19.5).
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

            let Some(arrival) = arrival(&event.event, &threshold) else {
                continue;
            };

            match session.verdict(arrival.control, &reserved) {
                Verdict::Ignore => continue,
                Verdict::Refuse(reason) => {
                    // Claimed even though it was refused: the player pressed it at a rebinding
                    // screen, and whatever it would otherwise have done is not what they meant.
                    consumed.claim_for_capture(arrival.control);
                    if arrival.deliberate {
                        commands.trigger(Refused {
                            entity,
                            slot: session.slot,
                            control: arrival.control,
                            reason,
                        });
                    }
                }
                Verdict::Take => {
                    consumed.claim_for_capture(arrival.control);
                    // Removed *before* the event, and both halves of that matter. An observer is
                    // entitled to do anything to this entity, despawning it included — a settings
                    // row that closes on being answered is an ordinary thing to write — so the
                    // crate must have finished with the entity before it hands it over. It also
                    // means the observer sees the component already gone, so "is this row still
                    // listening" reads the same from inside the observer as from anywhere else.
                    //
                    // Fallible because one run can answer several sessions, and the first
                    // observer to run may despawn a later one's entity.
                    commands.entity(entity).try_remove::<CaptureSession>();
                    commands.trigger(Captured {
                        entity,
                        slot: session.slot,
                        control: arrival.control,
                    });
                    break;
                }
            }
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
        refused: Vec<(Control, RefusedReason)>,
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.init_resource::<Heard>();
        app.add_observer(|event: On<Captured>, mut heard: ResMut<'_, Heard>| {
            heard.captured.push(event.control);
        });
        app.add_observer(|event: On<Refused>, mut heard: ResMut<'_, Heard>| {
            heard.refused.push((event.control, event.reason));
        });
        app.add_context::<OnFoot>(|controls| {
            controls
                .bind::<Move>(crate::binding::DirectionalButtons::wasd())
                .mappable();
            controls.bind::<Jump>(KeyCode::Space).mappable();
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

    fn slot(app: &App, key: &str) -> Slot {
        crate::rebind::slots(app.world())
            .into_iter()
            .find(|slot| alloc::string::ToString::to_string(&slot.key) == key)
            .expect("no such slot")
    }

    /// The whole point: what comes back is the control's identity, which a binding would have
    /// turned into a value and discarded.
    #[test]
    fn a_capture_reports_the_control_that_was_pressed() {
        let mut app = app();
        let target = slot(&app, "capture_tests.move.up");
        let row = app
            .world_mut()
            .spawn(CaptureSession::for_slot(&target).expect("a button slot"))
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

    /// R19.5, and the reason capture reads the frame rather than a binding: no context is spawned
    /// here at all, and capture does not notice.
    #[test]
    fn capture_works_with_no_context_spawned() {
        let mut app = app();
        assert!(crate::rebind::slots(app.world()).len() > 1);

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

    /// The half of OQ-10 that does the work. Reserving would be worth little if the screen key
    /// merely had no slot of its own — anything else could still be bound over the top of it.
    #[test]
    fn a_reserved_control_is_refused_out_loud() {
        let mut app = app();
        let target = slot(&app, "capture_tests.jump");
        app.world_mut()
            .spawn(CaptureSession::for_slot(&target).expect("a button slot"));
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

    /// A slot is rebound within its scheme (R19.7), so the pad cannot answer for the keyboard.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_control_from_the_other_scheme_is_refused() {
        use bevy_input::gamepad::{GamepadButton, RawGamepadButtonChangedEvent};

        let mut app = app();
        let target = slot(&app, "capture_tests.jump");
        app.world_mut()
            .spawn(CaptureSession::for_slot(&target).expect("a button slot"));
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

    /// R19.5's other half: what the player presses at a rebinding screen must not also play the
    /// game.
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

    /// The read-only half of R19.3.
    #[test]
    fn conflicts_name_the_slots_that_already_hold_a_control() {
        let app = app();
        let jump = slot(&app, "capture_tests.jump").key;

        let found = conflicts(app.world(), Control::Key(KeyCode::KeyW), Some(jump));
        assert_eq!(found.len(), 1);
        assert_eq!(
            alloc::string::ToString::to_string(&found[0].slot),
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

        // And a slot does not conflict with itself, so rebinding a control to where it already is
        // reports nothing rather than reporting the row the player is looking at.
        assert!(conflicts(app.world(), Control::Key(KeyCode::Space), Some(jump)).is_empty());
    }

    /// A class is a property, not a list — which is the whole of R4.9's first bullet.
    #[test]
    fn classes_are_decided_by_the_channel_a_control_reports_on() {
        assert!(ControlClass::AnyButton.contains(Control::Key(KeyCode::KeyA)));
        assert!(!ControlClass::AnyButton.contains(Control::MouseMotion));
        assert!(ControlClass::AnyDelta.contains(Control::MouseMotion));

        assert_eq!(
            ControlClass::of(ChannelShape::Button),
            Some(ControlClass::AnyButton)
        );
        // The one with no answer, and the reason there is no two-dimensional class.
        assert_eq!(ControlClass::of(ChannelShape::Axis2), None);
    }

    /// A stick bound whole is not a rebinding row, so `for_slot` says so rather than making one
    /// that can never be filled.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_slot_no_single_control_can_fill_has_no_capture() {
        #[derive(InputContext)]
        #[context(path = "capture_tests.stick", tick = Render)]
        struct WithStick;

        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<WithStick>(|controls| {
            controls
                .bind::<Move>(crate::binding::Stick::Left)
                .mappable();
        });

        let target = &crate::rebind::slots(app.world())[0];
        assert_eq!(target.accepts, ChannelShape::Axis2);
        assert!(CaptureSession::for_slot(target).is_none());
    }

    /// Reserving and declaring a slot say opposite things about one binding.
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

    /// Nothing about capture depends on a slot, so a game can ask "what did they just press" for
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
    /// A guard rather than a reproduction, and worth being straight about which: the bug this was
    /// written for showed up under `DefaultPlugins` and not here, because whether an observer's
    /// deferred commands run before or after the ones already queued depends on the executor. The
    /// test below is the one that actually fails without the fix. This one states the contract.
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
