//! The input frame: a normalized, timestamped record of what the devices did.
//!
//! The input frame is a timestamped event queue. Games usually do not want to react directly to
//! each raw input message as it arrives; they want one consistent snapshot of what happened during
//! the frame. That makes input easier to replay, easier to test, and easier to feed into later
//! mapping stages.
//!
//! Right now it samples keyboard input and mouse motion, but the queue is already shaped so higher
//! layers can replay it later.
//!
//! ```rust
//! use bevy::prelude::*;
//! use bevy_action_map::frame::{InputFrame, InputFramePlugin};
//!
//! fn main() {
//!     App::new()
//!         .add_plugins((MinimalPlugins, InputFramePlugin))
//!         .run();
//! }
//! ```

use alloc::vec::Vec;
use bevy_math::Vec2;

#[cfg(feature = "bevy_reflect")]
use bevy_reflect::Reflect;

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
use bevy_app::{App, FixedPreUpdate, Plugin, PreUpdate};
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::Resource;
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
use bevy_ecs::schedule::IntoScheduleConfigs;
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
use bevy_input::InputSystems;
#[cfg(feature = "gamepad")]
use bevy_input::gamepad::RawGamepadEvent;
#[cfg(feature = "keyboard")]
use bevy_input::keyboard::KeyboardInput;
#[cfg(feature = "mouse")]
use bevy_input::mouse::{MouseButtonInput, MouseMotion};

/// A monotonically increasing timestamp tagged with the sampling frame that produced it.
// Bevy's input events do not carry a stable order or a frame tag, so this wrapper gives us one
// place to record both before later layers consume the queue. It goes away once Bevy ships event
// timestamps.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp {
    frame: u64,
    order: u32,
}

impl Timestamp {
    /// Creates a timestamp for a specific sampled frame and event order.
    pub const fn new(frame: u64, order: u32) -> Self {
        Self { frame, order }
    }

    /// The sampled frame that produced this timestamp.
    pub const fn frame(self) -> u64 {
        self.frame
    }

    /// The order of this event within its sampled frame.
    pub const fn order(self) -> u32 {
        self.order
    }
}

/// A raw input event captured into the frame queue.
// Variants are gated by the feature that supplies their payload type, not by the feature that
// samples them: motion is a bare `Vec2`, so it stays, which also keeps this enum inhabited when
// every source feature is off and spares every `match` on it a catch-all arm.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum RawEvent {
    /// A keyboard event sampled from Bevy's keyboard message stream.
    #[cfg(feature = "keyboard")]
    Keyboard(KeyboardInput),
    /// A mouse button event sampled from Bevy's mouse message stream.
    #[cfg(feature = "mouse")]
    MouseButton(MouseButtonInput),
    /// Mouse motion sampled from Bevy's mouse message stream.
    MouseMotion(Vec2),
    /// A raw gamepad event sampled before Bevy's per-axis deadzone processing.
    #[cfg(feature = "gamepad")]
    Gamepad(RawGamepadEvent),
}

impl RawEvent {
    /// The physical control this event reports on, if it names one.
    ///
    /// `None` only for a gamepad connection event, which is about the device rather than any one
    /// control on it. Every other variant has exactly one control behind it, which lets a class
    /// binding match events by control without deriving it itself.
    pub fn control(&self) -> Option<crate::binding::Control> {
        use crate::binding::Control;
        match self {
            #[cfg(feature = "keyboard")]
            Self::Keyboard(event) => Some(Control::Key(event.key_code)),
            #[cfg(feature = "mouse")]
            Self::MouseButton(event) => Some(Control::MouseButton(event.button)),
            Self::MouseMotion(_) => Some(Control::MouseMotion),
            #[cfg(feature = "gamepad")]
            Self::Gamepad(RawGamepadEvent::Button(button)) => {
                Some(Control::GamepadButton(button.button))
            }
            #[cfg(feature = "gamepad")]
            Self::Gamepad(RawGamepadEvent::Axis(axis)) => Some(Control::GamepadAxis(axis.axis)),
            #[cfg(feature = "gamepad")]
            Self::Gamepad(RawGamepadEvent::Connection(_)) => None,
        }
    }
}

/// A raw input event paired with the timestamp it was sampled under.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct TimedRawEvent {
    /// When this event was sampled.
    pub timestamp: Timestamp,
    /// The raw event itself.
    pub event: RawEvent,
}

/// The raw input that has happened and not yet been read.
///
/// Games usually want to make input decisions from a coherent record of what the devices did, not
/// from a stream of callbacks arriving at arbitrary times. The frame keeps that record as an
/// ordered, timestamped queue, which is easier to replay, easier to test, and a better handoff to
/// the mapping layer.
///
/// It is a queue rather than a per-frame snapshot: a fixed timestep runs a variable number of
/// times per rendered frame, sometimes zero. A snapshot replaced each frame would lose a key that
/// was pressed and released between two simulation ticks. Events instead stay queued until read,
/// and each consumer asks for what has happened since it last looked, via
/// [`events_after`](InputFrame::events_after).
// Theory of operation. The queue is appended to once a frame, read independently by each consumer,
// and retired wholesale. Three rules keep those from interfering.
//
// Appending is monotonic. `record` stamps (frame, order) with order rising inside a sample and
// frame rising across samples, so the vector is always sorted by timestamp. `events_after` relies
// on that to binary-search for a consumer's cursor. Anything that inserted out of order, or sorted
// by something else, would not fail loudly — it would hand consumers the wrong slice.
//
// Reading is per consumer, not destructive. A consumer passes the last timestamp it saw and gets
// what came after; reading does not remove anything, because several consumers read the same
// events. A render-tick context and a fixed-tick context both act on the same press, and that is
// correct: they are answering different questions about it.
//
// Retiring is wholesale and happens after fixed evaluation. That instant is chosen because it is
// the only one at which every consumer is known to have read: render-tick contexts evaluate in
// PreUpdate, earlier in the same frame, and fixed-tick ones have just evaluated. Retiring at
// sample time instead — which is what this did originally — discards events before a fixed tick
// that has not run yet can see them, and is what made a 0-tick frame lose edges. The invariant is
// load-bearing and not local to this file: it holds only while evaluation stays in PreUpdate and
// FixedPreUpdate, so moving either schedule breaks it silently.
//
// Cursors and retirement look redundant and are not. Retirement alone fails when the simulation
// does not step: nothing is retired, next frame's sample appends to what is still queued, and a
// render context reads events it already acted on. Cursors alone fail by unbounded growth. So
// cursors give correctness and retirement gives a bound, and neither substitutes for the other.
//
// Window granularity is a property of the shim, not of the design. Timestamps carry a frame
// number, so every event in a frame compares equal on the only axis a window could split, and the
// first fixed tick to run necessarily takes all of them while later ticks in that frame take
// nothing. Magnitude is conserved and each edge is seen once, which is what R9.4 and R9.5 ask for;
// what is missing is attributing an event to the tick it truly fell in. Real timestamps
// (bevy#9087) change that policy alone.
//
// Reading from outside PreUpdate or FixedPreUpdate is a trap worth knowing about: by Update the
// queue has been retired if the simulation stepped this frame and is intact if it did not, so a
// reader there sees content that depends on the frame rate.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Resource)]
pub struct InputFrame {
    events: Vec<TimedRawEvent>,
    frame: u64,
    next_order: u32,
    dropped: u64,
}

/// How many events the queue holds before it starts dropping the oldest.
///
/// Reached only when nothing is draining the queue — normally a fixed tick retires it every frame.
const CAPACITY: usize = 4096;

impl InputFrame {
    /// Returns every event still queued, in sampling order.
    pub fn events(&self) -> &[TimedRawEvent] {
        &self.events
    }

    /// Returns the events sampled after `cursor`, or all of them for `None`.
    ///
    /// This is how a consumer reads its own window: pass the timestamp you last saw, and you get
    /// what has happened since, exactly once.
    pub fn events_after(&self, cursor: Option<Timestamp>) -> &[TimedRawEvent] {
        match cursor {
            Some(cursor) => {
                let consumed = self
                    .events
                    .partition_point(|event| event.timestamp <= cursor);
                &self.events[consumed..]
            }
            None => &self.events,
        }
    }

    /// Returns the timestamp of the most recently sampled event.
    pub fn latest(&self) -> Option<Timestamp> {
        self.events.last().map(|event| event.timestamp)
    }

    /// Returns how many events have been dropped because the queue was full.
    ///
    /// Anything above zero means input was discarded before something read it.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Removes all queued events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Starts a new sampled frame and resets the per-frame event order.
    pub fn begin_sample(&mut self) {
        self.frame = self
            .frame
            .checked_add(1)
            .expect("input frame counter exhausted u64");
        self.next_order = 0;
    }

    /// Records a raw event in the current sampled frame.
    pub fn record(&mut self, event: RawEvent) -> Timestamp {
        let timestamp = Timestamp::new(self.frame, self.next_order);
        self.next_order = self
            .next_order
            .checked_add(1)
            .expect("input frame event order exhausted u32");
        self.events.push(TimedRawEvent { timestamp, event });

        if self.events.len() > CAPACITY {
            let excess = self.events.len() - CAPACITY;
            self.events.drain(..excess);
            self.dropped = self.dropped.saturating_add(excess as u64);
        }

        timestamp
    }
}

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
/// Samples keyboard, mouse, and gamepad messages into the input frame queue.
pub fn sample_input(
    mut frame: bevy_ecs::system::ResMut<InputFrame>,
    #[cfg(feature = "keyboard")] mut keyboard_inputs: MessageReader<KeyboardInput>,
    #[cfg(feature = "mouse")] mut mouse_button_inputs: MessageReader<MouseButtonInput>,
    #[cfg(feature = "mouse")] mut mouse_motion_inputs: MessageReader<MouseMotion>,
    #[cfg(feature = "gamepad")] mut gamepad_inputs: MessageReader<RawGamepadEvent>,
) {
    frame.begin_sample();
    #[cfg(feature = "keyboard")]
    for event in keyboard_inputs.read() {
        frame.record(RawEvent::Keyboard(event.clone()));
    }

    #[cfg(feature = "mouse")]
    for event in mouse_button_inputs.read() {
        frame.record(RawEvent::MouseButton(*event));
    }

    #[cfg(feature = "mouse")]
    for event in mouse_motion_inputs.read() {
        frame.record(RawEvent::MouseMotion(event.delta));
    }

    #[cfg(feature = "gamepad")]
    for event in gamepad_inputs.read() {
        frame.record(RawEvent::Gamepad(event.clone()));
    }
}

/// Discards events every consumer has already read.
///
/// Runs after fixed-tick evaluation, which is the moment at which that is true of everything:
/// render-tick contexts drained in `PreUpdate`, earlier in the same frame, and fixed-tick ones
/// have just drained now.
pub fn retire_read_events(mut frame: bevy_ecs::system::ResMut<'_, InputFrame>) {
    frame.clear();
}

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
/// Plugin that installs the keyboard, mouse, and gamepad input frame sampler.
pub struct InputFramePlugin;

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
impl Plugin for InputFramePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputFrame>()
            .add_systems(
                PreUpdate,
                sample_input
                    .in_set(crate::ActionMapSystems::Sample)
                    .after(InputSystems),
            )
            .add_systems(
                FixedPreUpdate,
                retire_read_events.after(crate::ActionMapSystems::Evaluate),
            );

        #[cfg(feature = "keyboard")]
        {
            app.add_message::<KeyboardInput>();
        }

        #[cfg(feature = "mouse")]
        {
            app.add_message::<MouseButtonInput>();
            app.add_message::<MouseMotion>();
        }

        #[cfg(feature = "gamepad")]
        {
            app.add_message::<RawGamepadEvent>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_order_events_within_a_frame() {
        let mut frame = InputFrame::default();
        frame.begin_sample();

        let first = frame.record(RawEvent::Keyboard(KeyboardInput {
            key_code: bevy_input::keyboard::KeyCode::Space,
            logical_key: bevy_input::keyboard::Key::Space,
            state: bevy_input::ButtonState::Pressed,
            text: None,
            repeat: false,
            window: bevy_ecs::entity::Entity::PLACEHOLDER,
        }));
        let second = frame.record(RawEvent::Keyboard(KeyboardInput {
            key_code: bevy_input::keyboard::KeyCode::Space,
            logical_key: bevy_input::keyboard::Key::Space,
            state: bevy_input::ButtonState::Released,
            text: None,
            repeat: false,
            window: bevy_ecs::entity::Entity::PLACEHOLDER,
        }));

        assert_eq!(first, Timestamp::new(1, 0));
        assert_eq!(second, Timestamp::new(1, 1));
        assert_eq!(frame.events().len(), 2);
        assert_eq!(frame.events()[0].timestamp, first);
        assert_eq!(frame.events()[1].timestamp, second);
    }

    #[test]
    fn records_mouse_motion_events() {
        let mut frame = InputFrame::default();
        frame.begin_sample();

        let timestamp = frame.record(RawEvent::MouseMotion(Vec2::new(2.0, -3.0)));

        assert_eq!(timestamp, Timestamp::new(1, 0));
        assert!(
            matches!(frame.events()[0].event, RawEvent::MouseMotion(delta) if delta == Vec2::new(2.0, -3.0))
        );
    }

    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    #[test]
    fn plugin_samples_keyboard_mouse_and_gamepad_messages_into_the_frame_queue() {
        use bevy_app::App;
        use bevy_input::{ButtonState, InputPlugin, mouse::MouseMotion};

        let mut app = App::new();
        app.add_plugins((InputPlugin, InputFramePlugin));

        #[cfg(feature = "keyboard")]
        {
            use bevy_input::keyboard::Key;

            app.world_mut().write_message(KeyboardInput {
                key_code: bevy_input::keyboard::KeyCode::KeyA,
                logical_key: Key::Character("a".into()),
                state: ButtonState::Pressed,
                text: Some("a".into()),
                repeat: false,
                window: bevy_ecs::entity::Entity::PLACEHOLDER,
            });
        }

        app.world_mut().write_message(MouseMotion {
            delta: bevy_math::Vec2::new(1.5, -2.0),
        });
        #[cfg(feature = "gamepad")]
        app.world_mut()
            .write_message(bevy_input::gamepad::RawGamepadEvent::Axis(
                bevy_input::gamepad::RawGamepadAxisChangedEvent::new(
                    bevy_ecs::entity::Entity::PLACEHOLDER,
                    bevy_input::gamepad::GamepadAxis::LeftStickX,
                    0.75,
                ),
            ));
        app.update();

        let keyboard_events = usize::from(cfg!(feature = "keyboard"));
        let first_frame_events = 1 + keyboard_events + usize::from(cfg!(feature = "gamepad"));

        let frame = app.world().resource::<InputFrame>();
        assert_eq!(frame.events().len(), first_frame_events);
        assert_eq!(frame.events()[0].timestamp, Timestamp::new(1, 0));
        #[cfg(any(feature = "keyboard", feature = "gamepad"))]
        assert_eq!(frame.events()[1].timestamp, Timestamp::new(1, 1));

        #[cfg(feature = "keyboard")]
        {
            use bevy_input::keyboard::Key;

            app.world_mut().write_message(KeyboardInput {
                key_code: bevy_input::keyboard::KeyCode::KeyA,
                logical_key: Key::Character("a".into()),
                state: ButtonState::Released,
                text: Some("a".into()),
                repeat: false,
                window: bevy_ecs::entity::Entity::PLACEHOLDER,
            });
        }

        app.update();

        // Nothing retired the queue, so the second frame's events are appended to the first's
        // rather than replacing them. This is what lets a fixed tick that runs zero times this
        // frame still see everything when it eventually runs.
        let frame = app.world().resource::<InputFrame>();
        assert_eq!(frame.events().len(), first_frame_events + keyboard_events);
        #[cfg(feature = "keyboard")]
        assert_eq!(
            frame.events()[first_frame_events].timestamp,
            Timestamp::new(2, 0)
        );

        // A consumer that read the first frame is offered only what came after it.
        let boundary = Timestamp::new(1, first_frame_events as u32 - 1);
        assert_eq!(frame.events_after(Some(boundary)).len(), keyboard_events);
        assert_eq!(frame.events_after(frame.latest()).len(), 0);
        assert_eq!(frame.events_after(None).len(), frame.events().len());
    }

    #[test]
    fn a_full_queue_drops_the_oldest_and_says_how_many() {
        let mut frame = InputFrame::default();
        frame.begin_sample();

        for _ in 0..CAPACITY + 10 {
            frame.record(RawEvent::MouseMotion(Vec2::ZERO));
        }

        assert_eq!(frame.events().len(), CAPACITY);
        assert_eq!(frame.dropped(), 10);
        // The survivors are the newest, so what is lost is the input nobody got to in time.
        assert_eq!(frame.events()[0].timestamp, Timestamp::new(1, 10));
    }
}
