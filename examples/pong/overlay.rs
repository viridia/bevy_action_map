//! The F1 debug panel — `common::debug_overlay` owns what it shows; this is only the toggle.

use bevy::prelude::*;
use bevy_action_map::prelude::*;

// `crate::common`, not `super::common`: `common` is declared once at whatever crate root is
// consuming `pong` (its own `main.rs` here, a different example's `main.rs` if this module is
// reused there), never nested inside `pong` itself — unlike `court`, `paddle`, `ball` and
// `score`, which are always siblings of each other and so reach one another with `super::`.
use crate::common::debug_overlay::{self, Showing};

/// Toggles the debug panel.
#[derive(InputAction)]
#[action(path = "pong.toggle_overlay", output = bool, intent = Button)]
pub struct ToggleOverlay;

/// A context of its own, unpaired, rather than a binding folded into [`super::paddle::Paddle`] —
/// toggling the panel is not gameplay and should not share a context with something that is.
#[derive(InputContext)]
#[context(path = "pong.debug", tick = Render)]
pub struct Debug;

pub fn plugin(app: &mut App) {
    app.add_plugins(debug_overlay::plugin);
    app.add_context::<Debug>(|controls| {
        controls.bind::<ToggleOverlay>(KeyCode::F1);
    });
    app.add_systems(Startup, shell.spawn());
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
