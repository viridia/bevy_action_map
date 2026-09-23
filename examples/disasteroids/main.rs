//! Disasteroids — an asteroids-like game, playable on the keyboard or a gamepad.
//!
//! The point of the example is [`actions`], which holds the entire input layer: fourteen actions,
//! three contexts, and the bindings that drive them from either device. Nothing else in the game
//! mentions a key or a button.
//!
//! Fly with `W`/arrow-up and `A`/`D` or the arrow keys, fire with space, jump with left shift, and
//! pause with escape. On a pad, the right trigger is the throttle and it is analog — the ship burns
//! as hard as you pull it.
//!
//! `F2`, or Y on a pad, opens [`settings`] — the controls screen, which lists every one of those
//! bindings without being told about any of them, and which can be moved around with the arrow keys
//! or the stick and operated end to end from the pad.
//!
//! The three contexts in [`actions`] are the arrangement worth copying: `Flying` is live only while
//! the game is playing, `Shell` has no activation condition at all, and `Menu` is higher priority
//! and `exclusive`, so while the settings screen is up the arrow keys move the selection instead of
//! turning the ship. Each context's own doc says why it is shaped that way.

#![allow(missing_docs)]

use bevy::prelude::*;
use bevy_action_map::prelude::*;

mod actions;
mod asteroids;
mod field;
mod overlay;
mod pause;
mod saved_controls;
mod settings;
mod ship;

#[path = "../common/mod.rs"]
mod common;

use common::prompt_ui::{self, PromptFamily, PromptSpan};
use common::widget_focus;

fn main() {
    App::new()
        .add_plugins((
            // `InputDispatchPlugin` is disabled because `widget_focus::plugin` answers a focused
            // `Button` for both devices; leaving both in place puts two mechanisms on the same
            // keys. See `common::widget_focus`.
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Disasteroids".into(),
                        resolution: (
                            field::HALF_EXTENT.x as u32 * 2,
                            field::HALF_EXTENT.y as u32 * 2,
                        )
                            .into(),
                        ..default()
                    }),
                    ..default()
                })
                .build()
                .disable::<bevy::input_focus::InputDispatchPlugin>(),
            common::font::plugin,
            bevy_action_map::ActionMapPlugin,
            // Not in `DefaultPlugins`, unlike the focus and widget plugins beside it. It holds the
            // navigation graph, which this game never writes an edge to — but the automatic
            // navigator consults that graph before falling back to where things are on screen, so
            // it has to exist.
            bevy::input_focus::directional_navigation::DirectionalNavigationPlugin,
            bevy_remote_driver::RemoteDriverPlugin,
        ))
        .add_plugins((
            actions::plugin,
            field::plugin,
            ship::plugin,
            asteroids::plugin,
            pause::plugin,
            overlay::plugin,
            settings::plugin,
            // After `settings::plugin`, which owns the resources its startup load writes into, and
            // after `actions::plugin`, whose contexts it resolves a saved row against.
            saved_controls::plugin,
            prompt_ui::plugin,
            widget_focus::plugin,
        ))
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.05)))
        // Disasteroids is a desktop game: its prompts name keys even when a pad is plugged in, and
        // the pad's own controls are listed on the settings screen rather than advertised in the
        // corner. Nothing infers this — a crate guessing it would be wrong silently.
        .insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)))
        .add_systems(Startup, (camera.spawn(), hint.spawn()))
        .run();
}

/// The camera, as a one-entity scene.
///
/// Every spawn in this example is written this way: a plain function returning `impl Scene`, and
/// either `.spawn()` to make a `Startup` system out of it or `Commands::spawn_scene` to spawn one
/// mid-game. `bsn!` is doing very little here, but the shape is the same at every size.
fn camera() -> impl Scene {
    bsn! { Camera2d }
}

/// A line in the corner naming the two screens, since a game with no menu advertises nothing.
///
/// The keys in it are spans rather than text, which is the same reason the controls screen captions
/// its own close button that way: text naming a control goes stale the moment somebody changes what
/// the control is, and a span is told when that happens.
///
/// Back in `Startup`, and that is the span's doing. Formatting the caption here meant asking what
/// fires an action before the entity carrying that context existed, which is a question with a
/// different answer depending on which `Startup` system ran first; a span asks after the fact and
/// asks again whenever the answer moves, so the ordering stops mattering.
fn hint() -> impl Scene {
    // Named bare: `bsn!` reads a path such as `actions::NewGame` as a patch rather than a value,
    // which leaves the span on the default id.
    use actions::{NewGame, ToggleOverlay, ToggleSettings};

    const LABEL: Color = Color::srgb(0.35, 0.4, 0.42);
    const KEY: Color = Color::srgb(0.62, 0.7, 0.72);

    bsn! {
        Text
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            left: Val::Px(8.0),
        }
        // Every span carries its own font and colour: a `TextSpan` requires both, so they are not
        // inherited from the `Text` above and a span that omits them is drawn at Bevy's default
        // size in white.
        Children [
            PromptSpan(ToggleOverlay)
            TextFont { font_size: 13.0_f32 }
            TextColor(KEY)
            --
            TextSpan::new(" debug overlay   ")
            TextFont { font_size: 13.0_f32 }
            TextColor(LABEL)
            --
            PromptSpan(ToggleSettings)
            TextFont { font_size: 13.0_f32 }
            TextColor(KEY)
            --
            TextSpan::new(" controls   ")
            TextFont { font_size: 13.0_f32 }
            TextColor(LABEL)
            --
            // Pinned to the keyboard, unlike the two above: this is the one shortcut with no pad
            // binding, and a player holding a pad is still better told what the key is than shown
            // the dash that stands for "nothing on this device".
            //
            // It captions `Ctrl+N`, because a prompt carries what has to be held alongside the
            // control. The controls screen lists the same binding from the same declaration, and
            // the two reading the same thing is the point of having both.
            PromptSpan(NewGame)
            ~{PromptFamily(DeviceFamily::KeyboardMouse)}
            TextFont { font_size: 13.0_f32 }
            TextColor(KEY)
            --
            TextSpan::new(" new game")
            TextFont { font_size: 13.0_f32 }
            TextColor(LABEL)
        ]
    }
}
