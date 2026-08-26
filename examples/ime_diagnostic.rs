//! What `KeyboardInput` actually carries during IME composition, on real hardware.
//!
//! Run it and read the console: `cargo run --example ime_diagnostic`.
//!
//! `ControlClass::CharacterProducing` (chunk 25) needs to tell "this key produced a character" from
//! "this key did not" using nothing but one `KeyboardInput` event — no access to whatever IME state
//! winit is tracking underneath. `KeyboardInput::text` looks like the whole answer, but IME
//! composition arrives on a separate `bevy_window::Ime` channel this crate does not read, and
//! whether a key event fired *during* composition still carries text is winit- and
//! platform-specific. Reasoning about it from documentation is exactly the mistake chunk 8's
//! gamepad deadzone findings warn against — this measures it instead.
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
//! Paste the console output back — that is what turns the predicate in
//! `src/capture.rs`'s `character_producing` from a guess into a finding.

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
