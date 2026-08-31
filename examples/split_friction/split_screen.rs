//! Two cameras sharing one window, each locked onto its own protagonist.
//!
//! The viewport rects come from an invisible `bevy_ui` layout rather than arithmetic on the window
//! size: a flexbox with a small column gap and two children, sized and positioned the way any other
//! `bevy_ui` layout would be, with [`sync_viewports`] reading the two children's computed rects back
//! out and writing them onto each camera's [`Viewport`]. Worth having this shape rather than
//! hand-computed rects — this game wants a real HUD eventually (health, a spawner-shrine indicator),
//! and it can reuse the same nodes.
//!
//! The layout's root needs a camera of its own to measure itself against — a plain `Val::Percent`
//! resolves against whatever camera the UI targets, and that must never be one of the two split
//! cameras: the moment `sync_viewports` narrowed one to half the window, the layout would measure
//! itself against half the window and shrink again next frame. [`hud_camera`] exists only to give
//! the layout a target that [`sync_viewports`] never touches — it draws only the divider between
//! the two panes, a real UI node rather than relying on the gap between the two split viewports
//! showing through as plain clear color, which read as garbage on at least one machine.

use bevy::camera::{ScalingMode, Viewport};
use bevy::prelude::*;
use bevy::ui::UiSystems;

use crate::protagonist::Protagonist;

/// One of the two panes in the split-screen layout, and the camera it drives.
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
struct Pane(u8);

/// The strip between the two panes — [`sync_viewports`] reads its edges as the exact split point,
/// rather than trusting each pane's own edge to land on the same pixel independently.
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
struct Divider;

const DIVIDER_WIDTH: f32 = 4.0;

/// The camera showing one protagonist — `0` or `1`, matching [`Protagonist`].
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
struct PlayerCamera(u8);

/// Never shows fewer world units than this on either axis, so shrinking the window — or a pane
/// simply being half the window wide — crops nothing; the view only ever gains more of the
/// dungeon on whichever axis has room to spare. 20×15 tiles.
const MIN_VISIBLE_WIDTH: f32 = 320.0;
const MIN_VISIBLE_HEIGHT: f32 = 240.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (hud_camera.spawn(), layout.spawn()));
    // Chained rather than alongside the two above: `fit_view` needs the cameras `cameras.spawn()`
    // creates to already be queryable, which only a command-flush between the two guarantees.
    app.add_systems(Startup, (cameras.spawn(), fit_view).chain());
    app.add_systems(PostUpdate, sync_viewports.after(UiSystems::Layout));
    app.add_systems(Update, follow);
}

/// Draws only the divider — see the module doc comment for why it needs a camera of its own.
fn hud_camera() -> impl Scene {
    bsn! {
        Camera2d
        Camera { order: -1 }
        IsDefaultUiCamera
    }
}

/// The invisible flexbox: two panes either side of the divider that marks where they split.
fn layout() -> impl Scene {
    bsn! {
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
        }
        Children [
            (Pane(0) Node { flex_grow: 1.0 }),
            (Divider Node { width: Val::Px(DIVIDER_WIDTH) } BackgroundColor(Color::BLACK)),
            (Pane(1) Node { flex_grow: 1.0 }),
        ]
    }
}

/// The two gameplay cameras, one per protagonist. Positioned by [`follow`] from the first tick on.
fn cameras() -> impl Scene {
    bsn! {
        Transform::default()
        Visibility::default()
        Children [
            (PlayerCamera(0) Camera2d Camera { order: 0 }),
            (PlayerCamera(1) Camera2d Camera { order: 1 }),
        ]
    }
}

fn fit_view(mut cameras: Query<&mut Projection, With<PlayerCamera>>) {
    for mut projection in &mut cameras {
        *projection = Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: MIN_VISIBLE_WIDTH,
                min_height: MIN_VISIBLE_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        });
    }
}

/// Keeps each camera centered on the protagonist it shows, snapped to whichever world-space
/// distance one screen pixel currently covers.
///
/// Unsnapped, a camera sits at whatever fractional pixel its protagonist's own float position
/// lands on, which shifts every tile's sampled texel by a fraction of a pixel as the camera moves —
/// visible, even with nearest filtering, as faint seams between tiles that weren't there while
/// standing still. The snap distance itself isn't a constant: [`fit_view`]'s `AutoMin` scaling
/// means how many world units one pixel covers depends on the pane's own current size, which is
/// read back from the camera's own resolved [`OrthographicProjection::area`] rather than assumed.
fn follow(
    protagonists: Query<(&Protagonist, &Transform), Without<PlayerCamera>>,
    mut cameras: Query<(&PlayerCamera, &Camera, &Projection, &mut Transform), Without<Protagonist>>,
) {
    for (camera, view, projection, mut transform) in &mut cameras {
        let Some((_, target)) = protagonists.iter().find(|(p, _)| p.0 == camera.0) else {
            continue;
        };
        let target = target.translation.truncate();

        let world_per_pixel = world_per_pixel(view, projection);
        transform.translation.x = snap(target.x, world_per_pixel);
        transform.translation.y = snap(target.y, world_per_pixel);
    }
}

/// How many world units this camera's current viewport spends on one screen pixel — `None` on the
/// odd frame where either isn't resolved yet (before the first layout pass, mainly).
fn world_per_pixel(camera: &Camera, projection: &Projection) -> Option<f32> {
    let Projection::Orthographic(ortho) = projection else {
        return None;
    };
    let pixels = camera.physical_viewport_size()?.x as f32;
    (pixels > 0.0).then(|| ortho.area.width() / pixels)
}

fn snap(value: f32, world_per_pixel: Option<f32>) -> f32 {
    match world_per_pixel {
        Some(step) if step > 0.0 && step.is_finite() => (value / step).round() * step,
        _ => value,
    }
}

/// Reads the two panes' and the divider's computed rects and writes each camera's viewport to
/// reach exactly to the divider's own edge — not to the pane's, which the flex layout may round to
/// a different pixel than the divider's edge lands on, leaving a sliver neither camera covers.
fn sync_viewports(
    panes: Query<(&Pane, &ComputedNode, &UiGlobalTransform)>,
    divider: Query<(&ComputedNode, &UiGlobalTransform), With<Divider>>,
    mut cameras: Query<(&PlayerCamera, &mut Camera)>,
) {
    let Ok((divider_node, divider_transform)) = divider.single() else {
        return;
    };
    let half_width = divider_node.size().x / 2.0;
    let divider_left = divider_transform.translation.x - half_width;
    let divider_right = divider_transform.translation.x + half_width;

    for (pane, node, transform) in &panes {
        let size = node.size();
        if size.x <= 0.0 || size.y <= 0.0 {
            continue;
        }
        // `transform`'s translation is the pane's center, in physical pixels, top-left origin —
        // the same convention `Viewport::physical_position` uses.
        let top = transform.translation.y - size.y / 2.0;
        let (left, right) = if pane.0 == 0 {
            (transform.translation.x - size.x / 2.0, divider_left)
        } else {
            (divider_right, transform.translation.x + size.x / 2.0)
        };
        let top_left = Vec2::new(left, top).max(Vec2::ZERO);
        let extent = Vec2::new(right - left, size.y);

        for (camera, mut camera_component) in &mut cameras {
            if camera.0 != pane.0 {
                continue;
            }
            camera_component.viewport = Some(Viewport {
                physical_position: top_left.as_uvec2(),
                physical_size: extent.as_uvec2(),
                ..default()
            });
        }
    }
}
