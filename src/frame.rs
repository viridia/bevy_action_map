//! The input frame: a normalized, timestamped record of what the devices did.
//!
//! The input frame is a timestamped event queue. Games usually do not want to react directly to
//! each raw input message as it arrives; they want one consistent snapshot of what happened during
//! the frame. That makes input easier to replay, easier to test, and easier to feed into later
//! mapping stages.
//!
//! Right now it only samples keyboard input, but the queue is already shaped so higher layers can
//! replay it later.
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

#[cfg(feature = "bevy_reflect")]
use bevy_reflect::Reflect;

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "keyboard")]
use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::prelude::Resource;
#[cfg(feature = "keyboard")]
use bevy_ecs::{message::MessageReader, schedule::IntoScheduleConfigs};
#[cfg(feature = "keyboard")]
use bevy_input::{InputSystems, keyboard::KeyboardInput};

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
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RawEvent {
    /// A keyboard event sampled from Bevy's keyboard message stream.
    Keyboard(KeyboardInput),
}

/// A raw input event paired with the timestamp it was sampled under.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TimedRawEvent {
    /// When this event was sampled.
    pub timestamp: Timestamp,
    /// The raw event itself.
    pub event: RawEvent,
}

/// A snapshot of the inputs collected for one sampling pass.
///
/// Games usually want to make input decisions from the frame as a whole, not from a stream of
/// callbacks arriving at arbitrary times. `InputFrame` gives them one ordered packet of raw input
/// messages, which is easier to replay, easier to test, and a better handoff to the mapping layer.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq, Resource)]
pub struct InputFrame {
    events: Vec<TimedRawEvent>,
    frame: u64,
    next_order: u32,
}

impl InputFrame {
    /// Returns the queued raw events in sampling order.
    pub fn events(&self) -> &[TimedRawEvent] {
        &self.events
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
        timestamp
    }
}

#[cfg(feature = "keyboard")]
/// Samples keyboard messages into the input frame queue.
pub fn sample_keyboard_input(
    mut frame: bevy_ecs::system::ResMut<InputFrame>,
    mut keyboard_inputs: MessageReader<KeyboardInput>,
) {
    frame.begin_sample();
    for event in keyboard_inputs.read() {
        frame.record(RawEvent::Keyboard(event.clone()));
    }
}

#[cfg(feature = "keyboard")]
/// Plugin that installs the keyboard-only input frame sampler.
pub struct InputFramePlugin;

#[cfg(feature = "keyboard")]
impl Plugin for InputFramePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputFrame>()
            .add_message::<KeyboardInput>()
            .add_systems(PreUpdate, sample_keyboard_input.after(InputSystems));
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

    #[cfg(feature = "keyboard")]
    #[test]
    fn plugin_samples_keyboard_messages_into_the_frame_queue() {
        use bevy_app::App;
        use bevy_input::{ButtonState, InputPlugin, keyboard::Key};

        let mut app = App::new();
        app.add_plugins((InputPlugin, InputFramePlugin));

        app.world_mut().write_message(KeyboardInput {
            key_code: bevy_input::keyboard::KeyCode::KeyA,
            logical_key: Key::Character("a".into()),
            state: ButtonState::Pressed,
            text: Some("a".into()),
            repeat: false,
            window: bevy_ecs::entity::Entity::PLACEHOLDER,
        });
        app.update();

        let frame = app.world().resource::<InputFrame>();
        assert_eq!(frame.events().len(), 1);
        assert_eq!(frame.events()[0].timestamp, Timestamp::new(1, 0));

        app.world_mut().write_message(KeyboardInput {
            key_code: bevy_input::keyboard::KeyCode::KeyA,
            logical_key: Key::Character("a".into()),
            state: ButtonState::Released,
            text: Some("a".into()),
            repeat: false,
            window: bevy_ecs::entity::Entity::PLACEHOLDER,
        });
        app.update();

        let frame = app.world().resource::<InputFrame>();
        assert_eq!(frame.events().len(), 2);
        assert_eq!(frame.events()[1].timestamp, Timestamp::new(2, 0));
    }
}
