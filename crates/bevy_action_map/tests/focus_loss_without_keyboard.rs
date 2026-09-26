//! `KeyboardFocusLost` must clear a held mouse button even when this crate's own `keyboard`
//! feature is off. A real windowed game gets the message regardless — `bevy_window` asks for
//! `bevy_input/keyboard` in its own `Cargo.toml`, so Cargo unifies the feature on across the whole
//! build no matter what a game's manifest requests — but this crate's isolated device
//! matrix does not pull `bevy_window` in, so `--features mouse,gamepad` (no `keyboard`) is the one
//! configuration that actually exercises the gap.

#![cfg(feature = "mouse")]

use bevy_action_map::prelude::*;
use bevy_app::{App, Update};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use bevy_ecs::system::ResMut;
use bevy_input::keyboard::KeyboardFocusLost;
use bevy_input::mouse::{MouseButton, MouseButtonInput};
use bevy_input::{ButtonState, InputPlugin};

#[derive(InputAction)]
#[action(path = "tests.fire", output = bool, intent = Button)]
struct Fire;

#[derive(InputContext)]
#[context(path = "tests.on_foot", tick = Render)]
struct OnFoot;

#[derive(Resource, Default)]
struct Probe {
    value: bool,
    phase: ActionPhase,
}

fn probe_fire(input: ContextActions<OnFoot>, mut probe: ResMut<Probe>) {
    probe.value = input.value::<Fire>();
    probe.phase = input.phase::<Fire>();
}

#[test]
fn focus_loss_clears_a_held_mouse_button_with_no_keyboard_feature() {
    let mut app = App::new();
    app.add_plugins((InputPlugin, ActionMapPlugin));
    app.add_context::<OnFoot>(|context| {
        context.bind::<Fire>(MouseButton::Left);
    });
    app.world_mut().spawn(OnFoot);
    app.init_resource::<Probe>();
    app.add_systems(Update, probe_fire);

    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Pressed,
        window: Entity::PLACEHOLDER,
    });
    app.update();

    let probe = app.world().resource::<Probe>();
    assert!(probe.value);
    assert_eq!(probe.phase, ActionPhase::Fired);

    app.world_mut().write_message(KeyboardFocusLost);
    app.update();

    let probe = app.world().resource::<Probe>();
    assert!(
        !probe.value,
        "a mouse button held through alt-tab must clear even with no keyboard feature"
    );
    assert_eq!(probe.phase, ActionPhase::Canceled);
}
