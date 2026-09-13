//! What the controls screen keeps between sessions: the gamepad preset the player chose, and the
//! rows they moved by hand.
//!
//! The crate hands out a `SavedOverrides` and no opinion about where the bytes go; here they go into
//! `bevy_settings`, one section:
//!
//! ```toml
//! [controls]
//! preset = "disasteroids.southpaw"
//!
//! [controls.overrides]
//! action_map_version = 1
//!
//! [controls.overrides.bindings.keyboard_mouse]
//! "disasteroids.fire" = ["key/KeyJ"]
//! ```
//!
//! The choice is two facts, so the group holds two fields and `SavedOverrides` is one of them
//! rather than the whole body. A `SettingsGroup` has to be a resource, and `SavedOverrides` is
//! neither a resource nor this example's type to make one of.
//!
//! Storing the preset's *name* and not its rows is the point of the arrangement. A preset is game
//! content that changes between builds; save its resolved rows and a patch that retunes Southpaw
//! leaves the player on the definition that shipped the day they picked it, still labelled Southpaw
//! but no longer matching it.

use bevy::ecs::system::Command;
use bevy::prelude::*;
use bevy::settings::{ReflectSettingsGroup, SaveSettings, SettingsGroup, SettingsPlugin};
use bevy_action_map::mapping::{declared_mappings, declared_tunables};
use bevy_action_map::overrides::{
    SavedOverrides, apply_overrides_with_preset, resolve_saved, save_overrides,
};

use crate::settings::{AppliedControls, Controls, resolve_preset, working_copy};

/// Reverse-DNS, per `SettingsPlugin`. Names the directory the settings file lives in.
const APP_ID: &str = "org.bevy.bevy_action_map.disasteroids";

/// What the controls screen keeps between sessions.
#[derive(Resource, SettingsGroup, Reflect, Default, Debug)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "controls")]
struct ControlSettings {
    /// The preset the player picked, empty until they have picked one.
    preset: String,
    /// The rows they moved by hand.
    overrides: SavedOverrides,
}

pub fn plugin(app: &mut App) {
    // Registration has to precede `SettingsPlugin`, which reads the whole type registry once when
    // it is built and never again. Registering the group brings `SavedOverrides` and everything it
    // reaches with it, so a saved row deserializes back to a type rather than being skipped.
    app.register_type::<ControlSettings>();
    app.add_plugins(SettingsPlugin::new(APP_ID));

    app.add_systems(Startup, load);
}

/// Puts last session's controls back before the game starts.
///
/// `Startup` rather than a plugin-build step, because resolving a saved row needs the mapping list
/// and that is only complete once every context has been declared. Nothing has spawned a context
/// entity yet at this point, which is fine: `apply_overrides_with_preset` rewrites the declared
/// baseline, and every instance spawned after it inherits the result.
fn load(world: &mut World) {
    let stored = world.resource::<ControlSettings>();
    let preset = stored.preset.clone();
    let saved = stored.overrides.clone();

    let declared = declared_mappings(world);
    let tunables = declared_tunables(world);
    // A row the file names and this build cannot place is reported and skipped, and the rest is
    // applied. A save file outliving the build that wrote it is ordinary, and refusing to start
    // over a renamed action would be a worse answer than losing that one row.
    let captures = match resolve_saved(&saved, &declared, &tunables) {
        Ok((captures, problems, unresolved)) => {
            for problem in problems {
                warn!("saved binding could not be applied: {problem:?}");
            }
            for row in unresolved {
                warn!("saved binding names nothing this build declares: {row:?}");
            }
            captures
        }
        Err(version) => {
            warn!("settings file is from a newer build, starting on the defaults: {version:?}");
            return;
        }
    };

    let controls = Controls {
        preset: resolve_preset(world, &preset),
        captures,
    };
    let (merged, preset_rows) = working_copy(world, &controls);
    apply_overrides_with_preset(world, &merged, &preset_rows);
    world.resource_mut::<AppliedControls>().0 = controls;
}

/// Writes a confirmed choice to the file. Called from the controls screen's Confirm, which is the
/// only thing that changes it.
///
/// The captures alone, never the merged copy: the preset's own rows are recoverable from its name,
/// and would go stale the moment the preset was edited.
///
/// Saved outright rather than debounced, unlike a pane's device in Split Friction. Confirm is one
/// press a player made deliberately, not a slider they are still dragging, and the write itself
/// happens off the main thread either way.
pub fn store(world: &mut World, controls: &Controls) {
    let saved = save_overrides(&controls.captures);
    let mut settings = world.resource_mut::<ControlSettings>();
    settings.preset = controls.preset.to_string();
    settings.overrides = saved;
    SaveSettings::IfChanged.apply(world);
}
