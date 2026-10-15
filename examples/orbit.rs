//! A camera that takes the mouse, and gives it back.
//!
//! A turntable camera around a lit object. The arrow keys orbit and tilt, `W` `A` `S` `D` pan, and
//! `Q` and `E` zoom, smoothly for as long as they are held. Click the view and the mouse orbits as
//! well; `Escape` gives the mouse back.
//!
//! The two states are the point. With the cursor free, the mouse belongs to `bevy_picking`: the
//! panel is clickable and moving the mouse turns nothing. Click the view and the cursor is grabbed,
//! picking stops collecting input, and the mouse belongs to this crate until `Escape` or the window
//! losing focus hands it back. `Escape` is reserved, so no rebinding can take the way out.
//!
//! The arrow keys and the mouse drive one action, `Orbit`, in the same units. The mouse reports how
//! far it has moved; a key reports that it is held, and `per_second` turns that into how far it
//! moved this frame. Scale the mouse's value by a frame time as well, and the orbit speeds up and
//! slows down with the frame rate.
//!
//! The panel lists the bindings as the crate reports them, under the context each one lives in.

#![allow(missing_docs)]

use bevy::camera::Viewport;
use bevy::picking::PickingSettings;
use bevy::picking::events::PointerClick;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::ui::UiSystems;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowFocused};
use bevy_action_map::mapping::{self, ActionMapping};
use bevy_action_map::prelude::*;

/// How far the camera turns per pixel of mouse motion.
const RADIANS_PER_PIXEL: f32 = 0.005;
/// How fast a held key orbits, in the mouse's pixels per second, so the two share one action.
const ORBIT_KEY_RATE: f32 = 300.0;
/// How fast a held key pans, in camera distances per second.
const PAN_RATE: f32 = 0.6;
/// How fast a held key zooms: the distance scales by e to this, per second.
const ZOOM_RATE: f32 = 1.2;
/// Tilt limits, so the camera neither looks up through the ground nor flips over the top.
const MIN_ELEVATION: f32 = -10.0 * std::f32::consts::PI / 180.0;
const MAX_ELEVATION: f32 = 85.0 * std::f32::consts::PI / 180.0;
const MIN_DISTANCE: f32 = 1.5;
const MAX_DISTANCE: f32 = 30.0;

const PANEL_WIDTH: f32 = 300.0;
const PANEL_BG: Color = Color::srgb(0.11, 0.12, 0.14);
const HEADING: Color = Color::srgb(0.95, 0.75, 0.35);
const LABEL: Color = Color::srgb(0.85, 0.86, 0.88);
const CONTROL: Color = Color::srgb(0.55, 0.75, 0.95);

#[derive(InputAction)]
#[action(path = "orbit.orbit", output = Vec2, intent = Delta2)]
struct Orbit;

#[derive(InputAction)]
#[action(path = "orbit.pan", output = Vec2, intent = Delta2)]
struct Pan;

#[derive(InputAction)]
#[action(path = "orbit.zoom_in", output = f32, intent = Analog1)]
struct ZoomIn;

#[derive(InputAction)]
#[action(path = "orbit.zoom_out", output = f32, intent = Analog1)]
struct ZoomOut;

#[derive(InputAction)]
#[action(path = "orbit.release", output = bool, intent = Button)]
struct Release;

/// The keys, which move the camera whoever has the mouse.
#[derive(InputContext)]
#[context(path = "orbit.view", tick = Render)]
struct View;

/// The mouse, while the view has it.
#[derive(InputContext)]
#[context(path = "orbit.captured", tick = Render)]
struct Captured;

/// Who the mouse belongs to.
#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum MouseOwner {
    /// Picking: the cursor is visible and the UI takes clicks.
    #[default]
    Picking,
    /// This crate: the cursor is grabbed, and motion orbits the camera.
    Camera,
}

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "orbit".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }),
        ActionMapPlugin,
        bevy_remote_driver::RemoteDriverPlugin,
    ));
    app.init_state::<MouseOwner>();

    app.add_context::<View>(|controls| {
        controls
            .bind::<Orbit>(DirectionalButtons::arrow_keys())
            .per_second(ORBIT_KEY_RATE);
        controls
            .bind::<Pan>(DirectionalButtons::wasd())
            .per_second(PAN_RATE);
        controls.bind::<ZoomIn>(KeyCode::KeyE);
        controls.bind::<ZoomOut>(KeyCode::KeyQ);
    });
    app.add_context::<Captured>(|controls| {
        // Deactivating the context is what keeps the mouse from orbiting while picking has it.
        controls.active_in_state(MouseOwner::Camera);
        // No `per_second`: the mouse has already said how far.
        controls.bind::<Orbit>(MouseMove);
        controls.bind::<Release>(KeyCode::Escape).reserved();
    });

    app.add_systems(Startup, (scene.spawn(), spawn_contexts, spawn_ui))
        .add_systems(
            Update,
            (drive_camera, place_camera, release, release_on_focus_loss).chain(),
        )
        .add_systems(Update, show_owner.run_if(state_changed::<MouseOwner>))
        .add_systems(PostUpdate, follow_viewport_slot.after(UiSystems::Layout))
        .add_systems(OnEnter(MouseOwner::Camera), grab)
        .add_systems(OnExit(MouseOwner::Camera), let_go)
        .run();
}

/// The camera that draws the 3D view.
#[derive(Component, Default, Clone)]
struct SceneCamera;

/// The UI node whose rectangle the [`SceneCamera`]'s viewport follows.
#[derive(Component, Default, Clone)]
struct ViewportSlot;

/// Places its camera on a sphere around `focus`.
#[derive(Component, Clone)]
struct Turntable {
    focus: Vec3,
    /// Angle around the vertical axis; zero puts the camera on the +Z side.
    azimuth: f32,
    /// Angle above the horizontal.
    elevation: f32,
    distance: f32,
}

impl Default for Turntable {
    fn default() -> Self {
        Self {
            focus: Vec3::new(0.0, 0.8, 0.0),
            azimuth: 0.6,
            elevation: 0.45,
            distance: 7.0,
        }
    }
}

fn scene() -> impl SceneList {
    bsn_list! {
        #SceneCamera
        SceneCamera
        Camera3d
        Turntable
        --
        // Draws the UI over the whole window, on top of the scene camera.
        #UiCamera
        Camera2d
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
        }
        IsDefaultUiCamera
        --
        #Sun
        DirectionalLight {
            shadow_maps_enabled: true,
        }
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y)
        --
        #Ground
        Mesh3d(asset_value(Circle::new(4.0)))
        MeshMaterial3d::<StandardMaterial>(asset_value(Color::srgb(0.25, 0.28, 0.3)))
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
        --
        #Subject
        Mesh3d(asset_value(Torus::new(0.5, 1.0)))
        MeshMaterial3d::<StandardMaterial>(asset_value(Color::srgb(0.8, 0.45, 0.3)))
        Transform::from_xyz(0.0, 1.2, 0.0).with_rotation(Quat::from_rotation_x(1.0))
    }
}

fn spawn_contexts(mut commands: Commands) {
    commands.spawn(View);
    commands.spawn(Captured);
}

/// The panel, built from the mapping list, beside the node the 3D view fills.
fn spawn_ui(world: &mut World) {
    let rows: Vec<_> = binding_rows(&mapping::mappings(world));
    world
        .spawn_scene(bsn! {
            #Shell
            Node {
                width: percent(100),
                height: percent(100),
            }
            Pickable::IGNORE
            Children [
                #Panel
                Node {
                    width: {px(PANEL_WIDTH)},
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: px(12),
                }
                BackgroundColor(PANEL_BG)
                Children [
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                    }
                    Children [ {rows} ]
                    --
                    #Status
                    Status
                    Text({owner_text(MouseOwner::Picking)})
                    TextFont { font_size: 15.0_f32 }
                    TextColor(HEADING)
                ]
                --
                #View
                ViewportSlot
                Node {
                    flex_grow: 1.0,
                }
                on(capture_on_click)
            ]
        })
        .expect("the shell scene spawns");
}

/// The line at the foot of the panel, saying who has the mouse.
#[derive(Component, Default, Clone)]
struct Status;

fn owner_text(owner: MouseOwner) -> String {
    match owner {
        MouseOwner::Picking => "Click the view to orbit with the mouse".into(),
        MouseOwner::Camera => "Escape gives the mouse back".into(),
    }
}

fn show_owner(owner: Res<State<MouseOwner>>, mut status: Single<&mut Text, With<Status>>) {
    status.0 = owner_text(**owner);
}

/// One line per mapping, and a heading wherever the context changes.
fn binding_rows(mappings: &[ActionMapping]) -> Vec<impl Scene> {
    let mut rows = Vec::new();
    let mut context = "";
    for row in mappings {
        if row.context != context {
            context = row.context;
            rows.push(line(
                mapping::fallback_label(context),
                String::new(),
                HEADING,
            ));
        }
        let controls: Vec<String> = row
            .controls()
            .into_iter()
            .flatten()
            .map(|control| control.fallback_label().into_owned())
            .collect();
        rows.push(line(row.key.fallback_label(), controls.join(", "), LABEL));
    }
    rows
}

fn line(label: String, controls: String, color: Color) -> impl Scene {
    bsn! {
        Node {
            justify_content: JustifyContent::SpaceBetween,
        }
        Children [
            Text({label})
            TextFont { font_size: 15.0_f32 }
            TextColor({color})
            --
            Text({controls})
            TextFont { font_size: 15.0_f32 }
            TextColor(CONTROL)
        ]
    }
}

/// Moves the camera by what the actions report this frame.
fn drive_camera(
    view: ContextActions<View>,
    captured: ContextActions<Captured>,
    time: Res<Time>,
    mut turntable: Single<&mut Turntable>,
) {
    // The same quantity from either context, so they add.
    let orbit = (view.value::<Orbit>() + captured.value::<Orbit>()) * RADIANS_PER_PIXEL;
    let pan = view.value::<Pan>();
    let zoom = view.value::<ZoomOut>() - view.value::<ZoomIn>();
    if orbit == Vec2::ZERO && pan == Vec2::ZERO && zoom == 0.0 {
        return;
    }
    let turntable = &mut **turntable;
    turntable.azimuth = (turntable.azimuth + orbit.x).rem_euclid(std::f32::consts::TAU);
    turntable.elevation = (turntable.elevation + orbit.y).clamp(MIN_ELEVATION, MAX_ELEVATION);
    let right = Vec3::new(turntable.azimuth.cos(), 0.0, -turntable.azimuth.sin());
    turntable.focus += (right * pan.x + Vec3::Y * pan.y) * turntable.distance;
    // A held zoom key is a rate on one axis, which `per_second` does not produce, so it is
    // integrated here.
    turntable.distance = (turntable.distance * (zoom * ZOOM_RATE * time.delta_secs()).exp())
        .clamp(MIN_DISTANCE, MAX_DISTANCE);
}

fn place_camera(mut cameras: Query<(&Turntable, &mut Transform), Changed<Turntable>>) {
    for (turntable, mut transform) in &mut cameras {
        let offset = Vec3::new(
            turntable.elevation.cos() * turntable.azimuth.sin(),
            turntable.elevation.sin(),
            turntable.elevation.cos() * turntable.azimuth.cos(),
        ) * turntable.distance;
        *transform = Transform::from_translation(turntable.focus + offset)
            .looking_at(turntable.focus, Vec3::Y);
    }
}

/// Observer for the view's node: a click hands the mouse to the camera.
fn capture_on_click(click: On<PointerClick>, mut owner: ResMut<NextState<MouseOwner>>) {
    if click.button == PointerButton::Primary {
        owner.set(MouseOwner::Camera);
    }
}

fn release(captured: ContextActions<Captured>, mut owner: ResMut<NextState<MouseOwner>>) {
    if captured.fired::<Release>() {
        owner.set(MouseOwner::Picking);
    }
}

/// Alt-tab gives the mouse back, as `Escape` does. The crate releases held controls on focus loss
/// by itself; the grab is the app's, so letting go of it is too.
fn release_on_focus_loss(
    mut focus: MessageReader<WindowFocused>,
    mut owner: ResMut<NextState<MouseOwner>>,
) {
    if focus.read().any(|event| !event.focused) {
        owner.set(MouseOwner::Picking);
    }
}

/// The grab alone leaves picking reading the buttons, so its input is switched off as well.
fn grab(
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut picking: ResMut<PickingSettings>,
) {
    cursor.grab_mode = CursorGrabMode::Locked;
    cursor.visible = false;
    picking.is_input_enabled = false;
}

fn let_go(
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut picking: ResMut<PickingSettings>,
) {
    cursor.grab_mode = CursorGrabMode::None;
    cursor.visible = true;
    picking.is_input_enabled = true;
}

/// Fits the scene camera's viewport to the [`ViewportSlot`] node, so the 3D view fills whatever
/// space the layout gives that node.
fn follow_viewport_slot(
    slot: Single<(&ComputedNode, &UiGlobalTransform), With<ViewportSlot>>,
    mut camera: Single<&mut Camera, With<SceneCamera>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    // The node's center and size are both in physical pixels.
    let (node, transform) = *slot;
    let center = transform.affine().translation;
    let half = node.size() * 0.5;
    let bounds = window.physical_size().as_vec2();
    let min = (center - half).clamp(Vec2::ZERO, bounds).as_uvec2();
    let max = (center + half).clamp(Vec2::ZERO, bounds).as_uvec2();
    let size = max.saturating_sub(min);
    // A zero-sized viewport is a render error; keep the last one until the node has a size.
    if size.x == 0 || size.y == 0 {
        return;
    }
    if camera
        .viewport
        .as_ref()
        .is_some_and(|v| v.physical_position == min && v.physical_size == size)
    {
        return;
    }
    camera.viewport = Some(Viewport {
        physical_position: min,
        physical_size: size,
        ..default()
    });
}
