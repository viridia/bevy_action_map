//! The F1 debug panel, and the one line beside it that is specific to this game.
//!
//! Press `F1` (or Select on a pad) to toggle it. The panel itself — every context, whether it is
//! active, and every action in it with its phase and what is stopping it from firing — is
//! `common::debug_overlay`, shared with every other example; this file is only the toggle wiring
//! and the debris count in the opposite corner.

use bevy::prelude::*;
use bevy_action_map::prelude::*;

use crate::actions::ToggleOverlay;
use crate::asteroids::Asteroid;
use crate::common::debug_overlay::{self, Showing};

#[derive(Component, Default, Clone)]
struct DebrisText;

pub fn plugin(app: &mut App) {
    app.add_plugins(debug_overlay::plugin);
    app.add_systems(Startup, debris_readout.spawn());
    app.add_systems(Update, redraw_debris);
}

/// Flips the overlay on and off.
///
/// Attached to the shell context's entity, so it hears the action wherever that lives.
pub(crate) fn toggle(_: On<Fired<ToggleOverlay>>, showing: ResMut<Showing>) {
    debug_overlay::toggle(showing);
}

/// The opposite corner from the shared panel below it — the two are unrelated text nodes with no
/// shared layout, so keeping them apart is simpler than making either aware of the other's size.
fn debris_readout() -> impl Scene {
    bsn! {
        DebrisText
        Text::new("")
        TextFont { font_size: 13.0_f32 }
        TextColor(Color::srgb(0.6, 0.9, 0.7))
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(8.0),
        }
    }
}

/// A separate node from the shared panel rather than a line inside it: the debris count is game
/// state, not input state, and the two do not need to share a string to appear together. Gated on
/// the same [`Showing`] flag as the panel below it, so the joke only costs screen space while the
/// panel is up to read it.
fn redraw_debris(
    showing: Res<Showing>,
    rocks: Query<Entity, With<Asteroid>>,
    mut text: Query<&mut Text, With<DebrisText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    if !showing.0 {
        text.0.clear();
        return;
    }
    let count = rocks.iter().count();
    text.0 = format!("kessler index: {count} ({})\n", debris_forecast(count));
}

/// How alarmed to be about the current debris count.
///
/// Kessler syndrome is the runaway case where orbital debris is dense enough that each collision
/// produces the fragments that cause the next one. Shooting a rock in half is that, deliberately —
/// six rocks become twenty-four if the player is thorough, and the forecast keeps up.
fn debris_forecast(rocks: usize) -> &'static str {
    match rocks {
        0 => "orbit clear",
        1..=6 => "nominal",
        7..=12 => "elevated",
        13..=18 => "cascading",
        _ => "Kessler syndrome",
    }
}
