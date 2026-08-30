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
//! the layout a target that [`sync_viewports`] never touches — nothing is drawn to it.

use bevy::camera::Viewport;
use bevy::prelude::*;
use bevy::ui::UiSystems;

use crate::protagonist::Protagonist;

/// One of the two panes in the split-screen layout, and the camera it drives.
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
struct Pane(u8);

/// The camera showing one protagonist — `0` or `1`, matching [`Protagonist`].
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
struct PlayerCamera(u8);

/// 1 world unit = 1 pixel by default, which reads as too zoomed out once each pane is only half
/// the window wide.
const ZOOM: f32 = 2.5;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (hud_camera.spawn(), layout.spawn()));
    // Chained rather than alongside the two above: `zoom_in` needs the cameras `cameras.spawn()`
    // creates to already be queryable, which only a command-flush between the two guarantees.
    app.add_systems(Startup, (cameras.spawn(), zoom_in).chain());
    app.add_systems(PostUpdate, sync_viewports.after(UiSystems::Layout));
    app.add_systems(Update, follow);
}

/// Renders nothing itself — see the module doc comment for why it exists.
fn hud_camera() -> impl Scene {
    bsn! {
        Camera2d
        Camera { order: -1 }
        IsDefaultUiCamera
    }
}

/// The invisible flexbox: two equal panes with a gap between them.
fn layout() -> impl Scene {
    bsn! {
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            column_gap: Val::Px(4.0),
        }
        Children [
            (Pane(0) Node { flex_grow: 1.0 }),
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

fn zoom_in(mut cameras: Query<&mut Projection, With<PlayerCamera>>) {
    for mut projection in &mut cameras {
        *projection = Projection::Orthographic(OrthographicProjection {
            scale: 1.0 / ZOOM,
            ..OrthographicProjection::default_2d()
        });
    }
}

/// Keeps each camera centered on the protagonist it shows.
fn follow(
    protagonists: Query<(&Protagonist, &Transform), Without<PlayerCamera>>,
    mut cameras: Query<(&PlayerCamera, &mut Transform), Without<Protagonist>>,
) {
    for (camera, mut transform) in &mut cameras {
        if let Some((_, target)) = protagonists.iter().find(|(p, _)| p.0 == camera.0) {
            transform.translation.x = target.translation.x;
            transform.translation.y = target.translation.y;
        }
    }
}

/// Reads each pane's computed rect and writes it onto the matching camera's viewport.
fn sync_viewports(
    panes: Query<(&Pane, &ComputedNode, &UiGlobalTransform)>,
    mut cameras: Query<(&PlayerCamera, &mut Camera)>,
) {
    for (pane, node, transform) in &panes {
        let size = node.size();
        if size.x <= 0.0 || size.y <= 0.0 {
            continue;
        }
        // `transform`'s translation is the pane's center, in physical pixels, top-left origin —
        // the same convention `Viewport::physical_position` uses.
        let top_left = (transform.translation - size / 2.0).max(Vec2::ZERO);

        for (camera, mut camera_component) in &mut cameras {
            if camera.0 != pane.0 {
                continue;
            }
            camera_component.viewport = Some(Viewport {
                physical_position: top_left.as_uvec2(),
                physical_size: size.as_uvec2(),
                ..default()
            });
        }
    }
}
