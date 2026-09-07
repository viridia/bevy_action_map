//! The court: its size, and the dashed center line that draws it.

use bevy::prelude::*;

/// Half the width and height of the court, in world units.
pub const HALF_EXTENT: Vec2 = Vec2::new(480.0, 300.0);

const LINE_COLOR: Color = Color::srgb(0.4, 0.45, 0.5);
const DASH_SIZE: Vec2 = Vec2::new(4.0, 14.0);
const DASH_GAP: f32 = 10.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, net.spawn());
}

/// The one piece of court furniture a rally needs to read as tennis-shaped rather than two paddles
/// floating in the dark.
fn net() -> impl SceneList {
    let step = DASH_SIZE.y + DASH_GAP;
    let count = (HALF_EXTENT.y * 2.0 / step) as i32;
    (0..count)
        .map(|i| dash(-HALF_EXTENT.y + step * i as f32 + DASH_SIZE.y / 2.0))
        .collect::<Vec<_>>()
}

fn dash(y: f32) -> impl Scene {
    bsn! {
        Mesh2d(asset_value(Rectangle::from_size(DASH_SIZE)))
        MeshMaterial2d::<ColorMaterial>(asset_value(LINE_COLOR))
        Transform::from_xyz(0.0, y, 0.0)
    }
}
