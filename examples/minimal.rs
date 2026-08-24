#![allow(missing_docs)]

use bevy::prelude::*;
use bevy_action_map::prelude::*;

#[derive(InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
struct Jump;

#[derive(InputContext)]
#[context(path = "gameplay.on_foot", tick = Render)]
struct OnFoot;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ActionMapPlugin))
        .add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        })
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(OnFoot);
        })
        .add_systems(Update, print_jump)
        .run();
}

fn print_jump(input: Actions<OnFoot>) {
    if input.fired::<Jump>() {
        println!("Jump fired");
    }
}
