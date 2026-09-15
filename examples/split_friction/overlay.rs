//! The F1 debug panel — `common::debug_overlay` owns what it shows; this is only the toggle.

use bevy::prelude::*;
use bevy_action_map::prelude::*;

use crate::common::debug_overlay::{self, Showing};

/// Toggles the debug panel.
#[derive(InputAction)]
#[action(path = "split_friction.toggle_overlay", output = bool, intent = Button)]
pub struct ToggleOverlay;

/// Always active and never paired, so the panel opens whether or not either protagonist has been
/// claimed yet — debugging is not a thing a join gesture should gate.
#[derive(InputContext)]
#[context(path = "split_friction.debug", tick = Render)]
pub struct Debug;

pub fn plugin(app: &mut App) {
    app.add_plugins(debug_overlay::plugin);
    app.add_context::<Debug>(|controls| {
        controls.bind::<ToggleOverlay>(KeyCode::F1);
    });
    app.add_systems(Startup, shell.spawn());
    app.add_systems(Update, target_panel_camera);
}

fn shell() -> impl Scene {
    bsn! {
        Debug
        on(toggle)
    }
}

fn toggle(_: On<Fired<ToggleOverlay>>, showing: ResMut<Showing>) {
    debug_overlay::toggle(showing);
}

/// Gives the panel the explicit camera `debug_overlay::OverlayPanel` asks a multi-camera game for.
/// See `split_screen`'s own doc comment for the multi-`Camera2d` clearing behaviour that makes
/// `hud_camera` (this game's default) the one camera nothing should ride on unannounced. The
/// highest render order is the camera nothing draws over afterward, which is the same reasoning
/// `join_ui` uses to pick a `PlayerCamera` instead, without naming one, so this keeps working if
/// the split ever grows a third pane.
///
/// A plain `Update` system rather than ordered `Startup` wiring: the panel and the cameras are
/// spawned by two different plugins, and racing their `Startup` systems to land in the right order
/// is more moving parts than running this every frame until it finds both and stops.
fn target_panel_camera(
    mut commands: Commands,
    panel: Query<Entity, (With<debug_overlay::OverlayPanel>, Without<UiTargetCamera>)>,
    cameras: Query<(Entity, &Camera)>,
) {
    let Ok(panel) = panel.single() else {
        return;
    };
    let Some((camera, _)) = cameras.iter().max_by_key(|(_, camera)| camera.order) else {
        return;
    };
    commands.entity(panel).insert(UiTargetCamera(camera));
}
