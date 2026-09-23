//! A small app for the driver's own plans to run against.
//!
//! A menu with a Start button that opens a panel, which Escape closes, and a line showing whatever
//! is typed. Run it under the client:
//!
//! ```sh
//! python3 bevy_remote_driver/client/run.py bevy_remote_driver/plans/testbed/smoke.json
//! ```

use bevy::{
    input::keyboard::KeyboardInput,
    prelude::*,
    ui_widgets::{Activate, Button},
};
use bevy_remote_driver::RemoteDriverPlugin;

const BACKGROUND: Color = Color::srgb(0.1, 0.1, 0.12);
const BUTTON: Color = Color::srgb(0.25, 0.3, 0.45);
const PANEL: Color = Color::srgb(0.35, 0.2, 0.2);

#[derive(Component, Default, Clone)]
struct Panel;

#[derive(Component, Default, Clone)]
struct Typed;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Driver testbed".into(),
                    resolution: (640, 400).into(),
                    ..default()
                }),
                ..default()
            }),
            RemoteDriverPlugin,
        ))
        .insert_resource(ClearColor(BACKGROUND))
        .add_systems(Startup, (camera.spawn(), menu.spawn()))
        .add_systems(Update, (type_into, close))
        .run();
}

fn camera() -> impl Scene {
    bsn! { Camera2d }
}

fn menu() -> impl Scene {
    bsn! {
        #Menu
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(16.0),
        }
        Children [
            Text::new("Driver testbed")
            --
            #Start
            Button
            on(start)
            Node { padding: {UiRect::axes(Val::Px(24.0), Val::Px(8.0))} }
            BackgroundColor(BUTTON)
            Children [ Text::new("Start") ]
            --
            #Typed
            Typed
            Text::new("")
        ]
    }
}

fn panel() -> impl Scene {
    bsn! {
        #Panel
        Panel
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(24.0),
            right: Val::Px(24.0),
            padding: {UiRect::all(Val::Px(12.0))},
        }
        BackgroundColor(PANEL)
        Children [ Text::new("Escape closes this") ]
    }
}

fn start(_: On<Activate>, mut commands: Commands, open: Query<(), With<Panel>>) {
    if open.is_empty() {
        commands.spawn_scene(panel());
    }
}

fn close(
    keys: Res<ButtonInput<KeyCode>>,
    open: Query<Entity, With<Panel>>,
    mut commands: Commands,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for panel in &open {
            commands.entity(panel).despawn();
        }
    }
}

fn type_into(mut keys: MessageReader<KeyboardInput>, mut typed: Single<&mut Text, With<Typed>>) {
    for key in keys.read() {
        if let (true, Some(text)) = (key.state.is_pressed(), &key.text) {
            typed.0.push_str(text);
        }
    }
}
