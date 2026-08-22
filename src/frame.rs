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

use bevy_app::{App, Plugin, PreUpdate};
#[cfg(feature = "keyboard")]
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_input::InputSystems;
#[cfg(feature = "gamepad")]
use bevy_input::gamepad::RawGamepadEvent;
#[cfg(feature = "keyboard")]
use bevy_input::keyboard::KeyboardInput;
#[cfg(feature = "mouse")]
use bevy_input::mouse::MouseMotion;

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
#[derive(Clone, Debug, PartialEq)]
pub enum RawEvent {
    /// A keyboard event sampled from Bevy's keyboard message stream.
    Keyboard(KeyboardInput),
    /// Mouse motion sampled from Bevy's mouse message stream.
    MouseMotion(Vec2),
    /// A raw gamepad event sampled before Bevy's per-axis deadzone processing.
    #[cfg(feature = "gamepad")]
    Gamepad(RawGamepadEvent),
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

/// A snapshot of the inputs collected for one sampling pass.
///
/// Games usually want to make input decisions from the frame as a whole, not from a stream of
/// callbacks arriving at arbitrary times. `InputFrame` gives them one ordered packet of raw input
/// messages, which is easier to replay, easier to test, and a better handoff to the mapping layer.
#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Resource)]
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

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
/// Samples keyboard, mouse, and gamepad messages into the input frame queue.
pub fn sample_keyboard_input(
    mut frame: bevy_ecs::system::ResMut<InputFrame>,
    #[cfg(feature = "keyboard")] mut keyboard_inputs: MessageReader<KeyboardInput>,
    #[cfg(feature = "mouse")] mut mouse_motion_inputs: MessageReader<MouseMotion>,
    #[cfg(feature = "gamepad")] mut gamepad_inputs: MessageReader<RawGamepadEvent>,
) {
    frame.clear();
    frame.begin_sample();
    #[cfg(feature = "keyboard")]
    for event in keyboard_inputs.read() {
        frame.record(RawEvent::Keyboard(event.clone()));
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

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
/// Plugin that installs the keyboard, mouse, and gamepad input frame sampler.
pub struct InputFramePlugin;

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
impl Plugin for InputFramePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputFrame>()
            .add_systems(PreUpdate, sample_keyboard_input.after(InputSystems));

        #[cfg(feature = "keyboard")]
        {
            app.add_message::<KeyboardInput>();
        }

        #[cfg(feature = "mouse")]
        {
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

        let frame = app.world().resource::<InputFrame>();
        assert_eq!(
            frame.events().len(),
            if cfg!(feature = "keyboard") && cfg!(feature = "gamepad") {
                3
            } else if cfg!(feature = "keyboard") || cfg!(feature = "gamepad") {
                2
            } else {
                1
            }
        );
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

        let frame = app.world().resource::<InputFrame>();
        assert_eq!(
            frame.events().len(),
            if cfg!(feature = "keyboard") { 1 } else { 0 }
        );
        #[cfg(feature = "keyboard")]
        assert_eq!(frame.events()[0].timestamp, Timestamp::new(2, 0));
    }
}
