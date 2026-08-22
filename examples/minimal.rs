#![allow(missing_docs)]

use bevy::prelude::*;
use bevy_action_map::prelude::*;

#[derive(bevy_action_map::InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
struct Jump;

#[derive(bevy_action_map::InputContext)]
#[context(path = "gameplay.on_foot", tick = Render)]
struct OnFoot;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, InputFramePlugin, ActionMapPlugin))
        .add_context::<OnFoot, _>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        })
        .add_systems(Update, print_jump)
        .run();
}

fn print_jump(input: Actions<'_, OnFoot>) {
    if input.fired::<Jump>() {
        println!("Jump fired");
    }
}