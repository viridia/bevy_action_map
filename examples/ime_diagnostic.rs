//! What `KeyboardInput` actually carries during IME composition, on real hardware.
//!
//! Run it and read the console: `cargo run --example ime_diagnostic`.
//!
//! `ControlClass::CharacterProducing` has to tell "this key produced a character" from "this key
//! did not" using one `KeyboardInput` event and no access to whatever IME state winit is tracking
//! underneath. `KeyboardInput::text` looks like the whole answer, but IME composition arrives on a
//! separate `bevy_window::Ime` channel this crate does not read, and whether a key event fired
//! *during* composition still carries text is winit- and platform-specific. This measures it
//! rather than reasoning about it from documentation.
//!
//! What to do: run this, then
//!
//! 1. Type a few plain ASCII characters and watch the ordinary case.
//! 2. Switch the OS input method to Japanese, Chinese, or Korean, and type something that needs
//!    composition (for Japanese, romaji like `nihongo` that IME turns into kana). Watch what
//!    `text` and `repeat` read on every keystroke of the composition, and what (if anything) prints
//!    when the composition commits.
//! 3. If your keyboard layout has one, try a dead key (e.g. `´` on an international US layout) and
//!    see what the following keypress reports.
//!
//! The console output is what `character_producing` in `src/capture.rs` has to be written against.

#![allow(missing_docs)]

use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "ime_diagnostic — watch the console".into(),
                resolution: (520, 160).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(bevy_remote_driver::RemoteDriverPlugin)
        .add_systems(Update, print_keyboard_events)
        .run();
}

fn print_keyboard_events(mut events: MessageReader<KeyboardInput>) {
    for event in events.read() {
        println!(
            "key_code={:?} logical_key={:?} state={:?} text={:?} repeat={}",
            event.key_code, event.logical_key, event.state, event.text, event.repeat
        );
    }
}
