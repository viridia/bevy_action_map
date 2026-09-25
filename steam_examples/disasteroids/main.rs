//! Disasteroids with Steam Input owning the pad.
//!
//! Everything but this file, [`actions`], [`glyphs`] and [`steam`] is the base game's own, linked
//! in by path: those modules name `crate::actions`, so they fly on whichever actions the including
//! crate declares. The keyboard plays exactly as it does there. The pad is read through Steam, and
//! bound in Steam's layout rather than by the game — the README says how to set that up.

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy_action_map::prelude::*;

mod actions;
mod glyphs;
mod steam;

#[path = "../../examples/disasteroids/asteroids.rs"]
mod asteroids;
#[path = "../../examples/disasteroids/field.rs"]
mod field;
#[path = "../../examples/disasteroids/overlay.rs"]
mod overlay;
#[path = "../../examples/disasteroids/pause.rs"]
mod pause;
#[path = "../../examples/disasteroids/saved_controls.rs"]
mod saved_controls;
#[path = "../../examples/disasteroids/settings.rs"]
mod settings;
#[path = "../../examples/disasteroids/ship.rs"]
mod ship;

#[path = "../../examples/common/mod.rs"]
mod common;

use common::prompt_ui::{self, IconPromptSpan, PromptFamily, PromptSpan};
use common::widget_focus;

fn main() {
    App::new()
        .add_plugins((
            glyphs::plugin,
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Disasteroids (Steam)".into(),
                        resolution: (
                            field::HALF_EXTENT.x as u32 * 2,
                            field::HALF_EXTENT.y as u32 * 2,
                        )
                            .into(),
                        ..default()
                    }),
                    ..default()
                })
                // The prompt glyphs are the base game's, and this crate has no `assets/` of its
                // own. Absolute, so a binary started outside Cargo finds them too.
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../assets").into(),
                    ..default()
                })
                .build()
                .disable::<bevy::input_focus::InputDispatchPlugin>(),
            common::font::plugin,
            bevy_action_map::ActionMapPlugin,
            bevy::input_focus::directional_navigation::DirectionalNavigationPlugin,
        ))
        .add_plugins((
            actions::plugin,
            steam::plugin,
            field::plugin,
            ship::plugin,
            asteroids::plugin,
            pause::plugin,
            overlay::plugin,
            settings::plugin,
            saved_controls::plugin,
            prompt_ui::plugin,
            widget_focus::prepare_widgets,
        ))
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.05)))
        .insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)))
        .add_systems(Startup, (camera.spawn(), hint.spawn()))
        .add_systems(
            OnEnter(settings::Settings::Showing),
            controls_screen.spawn(),
        )
        .run();
}

fn camera() -> impl Scene {
    bsn! { Camera2d }
}

/// Both families' tables, with the way into Steam's binding panel under the pad's. The base game's
/// presets and dead-zone stepper are left out: every pad row is Steam's, so they could only be
/// refused.
fn controls_screen() -> impl Scene {
    use settings::{ControlsScreen, MappingColumn};

    bsn! {
        @ControlsScreen {
            @columns: bsn_list! {
                @MappingColumn { @family: DeviceFamily::KeyboardMouse }
                --
                @MappingColumn {
                    @family: DeviceFamily::Gamepad,
                    @below: {steam::binding_panel()},
                }
            },
        }
    }
}

fn hint() -> impl Scene {
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
        Children [
            IconPromptSpan(ToggleOverlay)
            TextFont { font_size: 13.0_f32 }
            TextColor(KEY)
            --
            TextSpan::new(" debug overlay   ")
            TextFont { font_size: 13.0_f32 }
            TextColor(LABEL)
            --
            IconPromptSpan(ToggleSettings)
            TextFont { font_size: 13.0_f32 }
            TextColor(KEY)
            --
            TextSpan::new(" controls   ")
            TextFont { font_size: 13.0_f32 }
            TextColor(LABEL)
            --
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
