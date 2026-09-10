//! The two paddles: static device pairing, movement, and the bindings behind it.
//!
//! Player one is always the keyboard, paired the instant the game starts. Player two is always the
//! first gamepad the OS reports — [`pair_gamepad`] claims it the moment one shows up, whether that
//! is before the first frame or an hour into the rally, and never looks again. There is no join
//! gesture (chunk 66's `ControlClass::AnyButton`) and no race to resolve: only one side is ever
//! waiting, so there is only ever one slot to fill.
//!
//! Both paddles read the same [`Paddle`] context, bound to both device families at once. [`Paired`]
//! (chunk 26's device routing) is what keeps player two's stick from moving player one's paddle,
//! not two different contexts — the same arrangement Split Friction's `OnFoot` uses.
//!
//! A pad unplugged mid-rally goes quiet rather than freezing the game — the crate cancels whatever
//! it was holding on disconnect — but nothing here shows a reconnect prompt or restores the pairing
//! if a different pad is plugged back in; that is chunk 103's, not this one's.

use bevy::prelude::*;
use bevy_action_map::binding::InputContextBuilder;
use bevy_action_map::device::DeviceHandle;
use bevy_action_map::player::Paired;
use bevy_action_map::prelude::*;

use super::court::HALF_EXTENT;

/// How fast a paddle is trying to move, positive toward the top of the court.
#[derive(InputAction)]
#[action(path = "pong.move", output = f32, intent = Analog1)]
pub struct Move;

/// The context both paddles move under. Fixed tick throughout this example, since the ball's
/// physics wants a simulation rate independent of the frame rate.
#[derive(InputContext)]
#[context(path = "pong.paddle", tick = Fixed)]
pub struct Paddle;

/// Which side of the court a paddle guards — `0` for the left, `1` for the right, spawn order.
#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
pub struct Side(pub u8);

impl Side {
    pub const LEFT: Self = Self(0);
    pub const RIGHT: Self = Self(1);
}

pub const WIDTH: f32 = 12.0;
pub const HEIGHT: f32 = 80.0;
pub const INSET: f32 = 30.0;
const SPEED: f32 = 380.0;

pub fn plugin(app: &mut App) {
    app.add_context::<Paddle>(bindings);
    app.add_systems(Startup, spawn);
    app.add_systems(FixedUpdate, walk::<Paddle>);
    app.add_systems(Update, pair_gamepad);
}

/// Everything [`Move`] is bound to, as a function rather than a closure inside [`plugin`], so a
/// variant that declares [`Paddle`] itself gets the same controls without copying them.
pub fn bindings(controls: &mut InputContextBuilder<Paddle>) {
    controls.bind::<Move>(AxisButtons::new(KeyCode::KeyS, KeyCode::KeyW));
    controls.bind::<Move>(AxisButtons::new(KeyCode::ArrowDown, KeyCode::ArrowUp));
    controls.bind::<Move>(GamepadAxis::LeftStickY);
    controls.bind::<Move>(AxisButtons::new(
        GamepadButton::DPadDown,
        GamepadButton::DPadUp,
    ));
}

/// Both paddles are the same scene, [`paddle`]; player one gets a [`Paddle`] and a [`Paired`]
/// added straight onto the spawned entity, since there is nothing to wait for. Player two starts
/// with neither, and stays that way until [`pair_gamepad`] finds it a device.
fn spawn(mut commands: Commands) {
    commands
        .spawn_scene(paddle(Side::LEFT, -(HALF_EXTENT.x - INSET)))
        .insert((Paddle, Paired::to(DeviceHandle::KeyboardMouse)));
    commands.spawn_scene(paddle(Side::RIGHT, HALF_EXTENT.x - INSET));
}

/// The scene both paddles are, whatever is driving them.
pub fn paddle(side: Side, x: f32) -> impl Scene {
    bsn! {
        template_value(side)
        Mesh2d(asset_value(Rectangle::new(WIDTH, HEIGHT)))
        MeshMaterial2d::<ColorMaterial>(asset_value(Color::WHITE))
        Transform::from_xyz(x, 0.0, 0.0)
    }
}

/// Moves every paddle reading context `C` at the rate its [`Move`] action reports.
///
/// Generic over the context, so a variant that drives one paddle from somewhere else registers this
/// once per context rather than writing a second copy of it.
pub fn walk<C: InputContext + Component>(
    time: Res<Time>,
    input: ActionsQuery<C>,
    mut paddles: Query<&mut Transform, With<Side>>,
) {
    let delta = time.delta_secs();
    let limit = HALF_EXTENT.y - HEIGHT / 2.0;
    for (entity, state) in input.iter() {
        let Ok(mut transform) = paddles.get_mut(entity) else {
            continue;
        };
        let y = transform.translation.y + state.value::<Move>() * SPEED * delta;
        transform.translation.y = y.clamp(-limit, limit);
    }
}

/// Claims the first gamepad the game ever sees for player two.
///
/// `Added<Gamepad>` rather than a connection event: it reports a pad already connected when the
/// game starts exactly the same way it reports one plugged in mid-rally, so this needs only one
/// code path for both. `unpaired.single()` is safe because player one is paired at spawn — the only
/// entity this query can ever find is player two's paddle, before it has one, and there is nothing
/// left to find once it does.
fn pair_gamepad(
    mut commands: Commands,
    gamepads: Query<Entity, Added<Gamepad>>,
    unpaired: Query<Entity, (With<Side>, Without<Paired>)>,
) {
    let Some(gamepad) = gamepads.iter().next() else {
        return;
    };
    let Ok(entity) = unpaired.single() else {
        return;
    };
    commands
        .entity(entity)
        .insert((Paddle, Paired::to(DeviceHandle::Gamepad(gamepad))));
}
