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
//!
//! Each pane's join prompt and device label (chunk 27) need to sit *on top of* that pane's own
//! gameplay, not under it, which rules out drawing them through `hud_camera`: three `Camera2d`s
//! sharing one window (the two split cameras plus `hud_camera` on top) hit what looks like a Bevy
//! rendering bug — reproduced in isolation and reported upstream — where only the *last* camera's
//! `ClearColorConfig` is honored, and it clears the *entire* window rather than being confined to
//! its own `Viewport`, wiping out the other cameras' output. Instead, [`join_ui`] gives each pane's
//! prompt/label their own root UI node, `UiTargetCamera`'d directly at that pane's own
//! [`PlayerCamera`] — riding inside a render pass that already draws on top of that camera's own
//! world content, no extra camera or draw-order questions involved. `UiTargetCamera`'d UI measures
//! against the *whole window*, not that camera's own `Viewport` rect, so [`sync_viewports`] also
//! keeps each root's own `Node` (`Val::Percent`, not `Val::Px` — resolution-independent, so no
//! window scale-factor conversion needed) sized and positioned to match the pane it belongs to.

use bevy::camera::{ScalingMode, Viewport};
use bevy::prelude::*;
use bevy::ui::UiSystems;
use bevy_action_map::device::DeviceHandle;
use bevy_action_map::player::Paired;

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

/// "Waiting to join" over a pane's protagonist — visible until [`sync_join_ui`] finds it paired.
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
struct JoinPrompt(u8);

/// The device name at a pane's bottom edge — blank until [`sync_join_ui`] finds a pairing to name.
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
struct DeviceLabel(u8);

/// One pane's join-UI root — see the module doc comment for why it's `UiTargetCamera`'d at that
/// pane's own [`PlayerCamera`] rather than riding along on `hud_camera`. [`sync_viewports`] keeps
/// its `Node` sized and positioned to match the pane it belongs to.
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
struct PaneRoot(u8);

/// Never shows fewer world units than this on either axis, so shrinking the window — or a pane
/// simply being half the window wide — crops nothing; the view only ever gains more of the
/// dungeon on whichever axis has room to spare. 20×15 tiles.
const MIN_VISIBLE_WIDTH: f32 = 320.0;
const MIN_VISIBLE_HEIGHT: f32 = 240.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (hud_camera.spawn(), layout.spawn()));
    // Chained rather than alongside the two above: `fit_view` and `join_ui` need the cameras
    // `cameras.spawn()` creates to already be queryable, which only a command-flush between the
    // two guarantees.
    app.add_systems(Startup, (cameras.spawn(), fit_view, join_ui).chain());
    app.add_systems(PostUpdate, sync_viewports.after(UiSystems::Layout));
    app.add_systems(Update, (follow, sync_join_ui));
}

/// Renders nothing itself — see the module doc comment for why it exists.
fn hud_camera() -> impl Scene {
    bsn! {
        Camera2d
        Camera { order: -1 }
        IsDefaultUiCamera
    }
}

/// The invisible flexbox: two equal panes with the divider between them.
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

/// One pane's join UI, `UiTargetCamera`'d at `camera` — see the module doc comment.
///
/// `UiTargetCamera(camera)` is inserted after spawning rather than written into `pane_ui`'s own
/// `bsn!`: `bsn!`'s `FromTemplate` machinery requires `Default` of every component type a scene
/// spawns, which `UiTargetCamera` — a foreign type wrapping an `Entity`, with no sensible default —
/// doesn't have (the same constraint chunk 68 hit with `Paired`, met there by adding `Default`
/// since it was this crate's own type; not an option here).
fn join_ui(mut commands: Commands, cameras: Query<(Entity, &PlayerCamera)>) {
    for (camera, player) in &cameras {
        commands
            .spawn_scene(pane_ui(player.0))
            .insert(UiTargetCamera(camera));
    }
}

/// A centered join prompt and a bottom-anchored device label, positioned by [`sync_viewports`].
fn pane_ui(index: u8) -> impl Scene {
    bsn! {
        PaneRoot(index)
        Node {
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
        }
        Children [
            (
                JoinPrompt(index)
                Node {
                    flex_grow: 1.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                }
                Children [
                    (
                        Text::new("Waiting to join\npress any button")
                        TextFont { font_size: 18.0_f32 }
                        TextColor(Color::WHITE)
                        TextLayout::justify(Justify::Center)
                        BackgroundColor({Color::BLACK.with_alpha(0.55)})
                        Node {
                            max_width: Val::Percent(70.0),
                            padding: {UiRect::axes(Val::Px(12.0), Val::Px(8.0))},
                        }
                    ),
                ]
            ),
            (
                DeviceLabel(index)
                Text::new("")
                TextFont { font_size: 14.0_f32 }
                TextColor(Color::WHITE)
                BackgroundColor({Color::BLACK.with_alpha(0.55)})
                Node {
                    margin: {UiRect::bottom(Val::Px(16.0))},
                    padding: {UiRect::axes(Val::Px(10.0), Val::Px(4.0))},
                }
            ),
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

/// Keeps each pane's join prompt and device label true to its protagonist's pairing state — the
/// prompt shows and the label reads blank while unpaired, and the reverse once [`pair_on_join`]
/// (`protagonist.rs`) claims a device for it.
fn sync_join_ui(
    protagonists: Query<(&Protagonist, Option<&Paired>)>,
    mut prompts: Query<(&JoinPrompt, &mut Visibility)>,
    mut labels: Query<(&DeviceLabel, &mut Text, &mut Visibility), Without<JoinPrompt>>,
) {
    let device_for = |index: u8| {
        protagonists
            .iter()
            .find(|(protagonist, _)| protagonist.0 == index)
            .and_then(|(_, paired)| paired)
            .and_then(|paired| paired.iter().next())
    };

    for (prompt, mut visibility) in &mut prompts {
        *visibility = if device_for(prompt.0).is_some() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    for (label, mut text, mut visibility) in &mut labels {
        match device_for(label.0) {
            Some(device) => {
                text.0 = device_name(device).into();
                *visibility = Visibility::Inherited;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

fn device_name(device: DeviceHandle) -> &'static str {
    match device {
        DeviceHandle::KeyboardMouse => "Keyboard",
        DeviceHandle::Gamepad(_) => "Gamepad",
    }
}

/// Reads the two panes' and the divider's computed rects and writes each camera's viewport to
/// reach exactly to the divider's own edge — not to the pane's, which the flex layout may round to
/// a different pixel than the divider's edge lands on, leaving a sliver neither camera covers. Each
/// pane's own [`PaneRoot`] (its join UI, `UiTargetCamera`'d at that pane's camera — see the module
/// doc comment) is kept sized and positioned to match, as a percentage of the window rather than
/// `Val::Px`: `UiTargetCamera`'d UI measures against the whole window regardless of which camera it
/// targets, so a percentage of that same window is what lands a pane-relative rect correctly
/// without converting through the window's own scale factor.
fn sync_viewports(
    panes: Query<(&Pane, &ComputedNode, &UiGlobalTransform)>,
    divider: Query<(&ComputedNode, &UiGlobalTransform), With<Divider>>,
    mut cameras: Query<(&PlayerCamera, &mut Camera)>,
    mut roots: Query<(&PaneRoot, &mut Node)>,
    windows: Query<&Window>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let window_size = Vec2::new(
        window.resolution.physical_width() as f32,
        window.resolution.physical_height() as f32,
    );
    if window_size.x <= 0.0 || window_size.y <= 0.0 {
        return;
    }

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

        for (root, mut root_node) in &mut roots {
            if root.0 != pane.0 {
                continue;
            }
            root_node.left = Val::Percent(top_left.x / window_size.x * 100.0);
            root_node.top = Val::Percent(top_left.y / window_size.y * 100.0);
            root_node.width = Val::Percent(extent.x / window_size.x * 100.0);
            root_node.height = Val::Percent(extent.y / window_size.y * 100.0);
        }
    }
}
