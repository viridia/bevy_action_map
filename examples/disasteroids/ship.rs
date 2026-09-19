//! The player's ship: how it flies, and what it does when told to.

use bevy::prelude::*;
use bevy_action_map::prelude::*;
use std::f32::consts::TAU;

use crate::actions::{Afterburner, Fire, Flying, Hyperspace, SmartBomb, Thrust, Turn};
use crate::asteroids::{Asteroid, Doomed};
use crate::common::prompt_ui::{IconPromptSpan, PromptFamily};
use crate::field::{HALF_EXTENT, Lifetime, Velocity, Wraps};
use crate::pause::Simulating;

const TURN_RATE: f32 = 3.2;
const ACCELERATION: f32 = 420.0;
/// What holding the throttle open eventually buys.
const AFTERBURNER: f32 = 2.2;
/// Space is not really like this, but a ship that never slows down is miserable to fly.
const DRAG: f32 = 0.4;
const MUZZLE_SPEED: f32 = 620.0;

/// How long the gun takes to come back around, in seconds.
///
/// A property of the weapon, so it lives here, but it is spent in
/// [`actions`](crate::actions::plugin) as the interval of `Fire`'s `pulse` condition — the one game
/// number this example's input layer has to know.
pub const RELOAD: f32 = 0.18;

/// How long the smart bomb takes to charge, in seconds.
pub const BOMB_CHARGE: f32 = 0.5;
/// How far from the ship the smart bomb reaches.
const BOMB_RADIUS: f32 = 260.0;
/// How long the ring takes to expand and fade.
const BLAST_DURATION: f32 = 0.3;
/// When a caught rock actually goes — a beat before the ring finishes, the way a hit lands before
/// the shockwave that caused it does.
const BLAST_FUSE: f32 = 0.25;

#[derive(Component, Default, Clone)]
pub struct Ship;

#[derive(Component, Default, Clone)]
pub struct Bullet;

/// The flame drawn behind the ship, scaled by how hard the engine is burning.
#[derive(Component, Default, Clone)]
struct Exhaust;

/// The fill of the charge meter at the bottom of the screen, tracked by width.
#[derive(Component, Default, Clone)]
struct BombMeterFill;

/// The smart bomb's ring, expanding and fading over [`BLAST_DURATION`].
#[derive(Component, Default, Clone)]
struct ShockRing;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (ship.spawn(), bomb_meter.spawn()));
    // `fly` is the only polled system: a schedule holds what has to happen on every tick whether
    // the player did anything or not, and what happens *because* they did is an observer on the
    // ship itself.
    app.add_systems(FixedUpdate, fly.in_set(Simulating));
    app.add_systems(
        Update,
        (show_exhaust, redraw_bomb_meter, animate_shock_ring),
    );
}

/// The ship and the flame that hangs off the back of it, as one scene.
///
/// The hull and the exhaust are one object as far as the game is concerned, and BSN lets them be
/// written that way: `Children [...]` nests the flame inside the ship rather than spawning it
/// separately and attaching it afterwards. The meshes and materials go in as `asset_value`, which
/// adds them to their `Assets` collection when the scene spawns — so this function needs neither
/// `Assets<Mesh>` nor `Assets<ColorMaterial>` to describe them.
///
/// The context and the observers for its actions sit together, the same as they do in
/// [`actions::shell`](crate::actions::shell): `Flying` is what makes the ship hear the controls,
/// and the two `on(...)` lines are everything that happens because it heard them.
fn ship() -> impl Scene {
    bsn! {
        Ship
        Flying
        on(shoot)
        on(hyperspace)
        on(smart_bomb)
        Mesh2d(asset_value(Triangle2d::new(
            Vec2::new(18.0, 0.0),
            Vec2::new(-12.0, 11.0),
            Vec2::new(-12.0, -11.0),
        )))
        MeshMaterial2d::<ColorMaterial>(asset_value(Color::srgb(0.85, 0.9, 1.0)))
        Velocity
        Wraps
        Children [
            Exhaust
            // Symmetric about the nose-to-tail axis, or it burns off to one side.
            Mesh2d(asset_value(Triangle2d::new(
                Vec2::new(-12.0, -6.0),
                Vec2::new(-12.0, 6.0),
                Vec2::new(-30.0, 0.0),
            )))
            MeshMaterial2d::<ColorMaterial>(asset_value(Color::srgb(1.0, 0.6, 0.2)))
            Transform::from_scale(Vec3::ZERO)
        ]
    }
}

fn fly(
    time: Res<Time>,
    input: ContextActions<Flying>,
    ships: Query<(&mut Transform, &mut Velocity), With<Ship>>,
) {
    let delta = time.delta_secs();
    let turn = input.value::<Turn>();
    let thrust = input.value::<Thrust>();
    let boost = if input.value::<Afterburner>() {
        AFTERBURNER
    } else {
        1.0
    };

    for (mut transform, mut velocity) in ships {
        transform.rotate_z(-turn * TURN_RATE * delta);

        // The ship accelerates along its nose, which is what makes turning and thrusting two
        // separate decisions rather than one.
        let heading = (transform.rotation * Vec3::X).truncate();
        velocity.0 += heading * thrust * ACCELERATION * boost * delta;
        velocity.0 *= 1.0 - DRAG * delta;
    }
}

/// One shot, once per [`Fired`].
///
/// The rate of fire is not here. `Fire` is bound with a `pulse` condition, so holding the button
/// fires the action again every [`RELOAD`] seconds and this runs once for each — no reload timer on
/// the ship. `Fired` carries the entity whose context it came from, which is the ship, so a second
/// ship would shoot its own gun without this function learning anything new.
fn shoot(
    shot: On<Fired<Fire>>,
    mut commands: Commands,
    ships: Query<(&Transform, &Velocity), With<Ship>>,
) {
    let Ok((transform, velocity)) = ships.get(shot.entity) else {
        return;
    };
    commands.spawn_scene(bullet(*transform, velocity.0));
}

/// A shot, as a scene.
///
/// A scene function is an ordinary Rust function, so the muzzle arithmetic happens in front of the
/// `bsn!` block and goes in through `{...}`. Note what the observer above does not ask for: because
/// the mesh and the material are described rather than fetched, `shoot` needs no `Assets`
/// parameters and reads only what it actually cares about.
fn bullet(from: Transform, velocity: Vec2) -> impl Scene {
    let heading = (from.rotation * Vec3::X).truncate();
    bsn! {
        Bullet
        Mesh2d(asset_value(Circle::new(2.5)))
        MeshMaterial2d::<ColorMaterial>(asset_value(Color::srgb(1.0, 0.95, 0.6)))
        Transform::from_translation(from.translation + (heading * 20.0).extend(0.0))
        // Inheriting the ship's velocity is what makes flying backwards while firing forwards
        // feel like it should.
        Velocity({velocity + heading * MUZZLE_SPEED})
        Wraps
        Lifetime(Timer::from_seconds(1.1, TimerMode::Once))
    }
}

/// Somewhere else on the field, once per double-tap.
///
/// A jump is an edge, and a held key is not a stream of jumps. `Fired` is that edge: the only way
/// to run this twice is to double-tap twice.
fn hyperspace(
    jump: On<Fired<Hyperspace>>,
    mut ships: Query<(&mut Transform, &mut Velocity), With<Ship>>,
) {
    let Ok((mut transform, mut velocity)) = ships.get_mut(jump.entity) else {
        return;
    };

    let angle = rand_unit() * TAU;
    transform.translation = Vec3::new(
        (rand_unit() * 2.0 - 1.0) * HALF_EXTENT.x,
        (rand_unit() * 2.0 - 1.0) * HALF_EXTENT.y,
        0.0,
    );
    transform.rotation = Quat::from_rotation_z(angle);
    velocity.0 = Vec2::ZERO;
}

/// Marks every rock the blast reaches, and shows the ring that reached them.
///
/// `Fired` here is the hold's own doing: `hold_once` waits out [`BOMB_CHARGE`] and fires by itself,
/// so nothing here has to watch a timer to know the charge is done.
///
/// What gets caught is decided now, once, off where the rocks are at this instant — not off where
/// the ring's edge happens to be on some later frame. The alternative would have to tell a rock the
/// blast just made from one that was already there when it fired, which is the one thing this game
/// never needs to know.
fn smart_bomb(
    blast: On<Fired<SmartBomb>>,
    mut commands: Commands,
    ships: Query<&Transform, With<Ship>>,
    rocks: Query<(Entity, &Transform), With<Asteroid>>,
) {
    let Ok(ship_at) = ships.get(blast.entity) else {
        return;
    };
    for (rock, rock_at) in rocks {
        if ship_at.translation.distance(rock_at.translation) < BOMB_RADIUS {
            commands
                .entity(rock)
                .insert(Doomed(Timer::from_seconds(BLAST_FUSE, TimerMode::Once)));
        }
    }
    commands.spawn_scene(shock_ring(ship_at.translation));
}

/// The blast ring, as a scene: an annulus at full size, shrunk to a point until
/// [`animate_shock_ring`] lets it grow.
///
/// `Lifetime` gives it the clock both that system and its own despawn read — one timer answering
/// both "how far along" and "when to remove this" rather than two numbers that could disagree.
fn shock_ring(at: Vec3) -> impl Scene {
    bsn! {
        ShockRing
        Mesh2d(asset_value(Annulus::new(BOMB_RADIUS - 6.0, BOMB_RADIUS)))
        MeshMaterial2d::<ColorMaterial>(asset_value(Color::srgba(0.6, 0.85, 1.0, 0.8)))
        Transform {
            translation: {at},
            scale: {Vec3::splat(0.02)},
        }
        Lifetime(Timer::from_seconds(BLAST_DURATION, TimerMode::Once))
    }
}

/// Grows the ring outward and fades it out over its own lifetime.
fn animate_shock_ring(
    rings: Query<(&mut Transform, &Lifetime, &MeshMaterial2d<ColorMaterial>), With<ShockRing>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (mut transform, age, material) in rings {
        let fraction = (age.elapsed_secs() / BLAST_DURATION).clamp(0.0, 1.0);
        transform.scale = Vec3::splat(0.02 + fraction * 0.98);
        if let Some(mut material) = materials.get_mut(&material.0) {
            material.color.set_alpha(0.8 * (1.0 - fraction));
        }
    }
}

/// The charge meter, as a scene: a track, the fill [`redraw_bomb_meter`] resizes, and the pad
/// chord that charges it.
///
/// Centred at the bottom of the screen rather than tucked in a corner — this is the one control
/// with something to show while it is happening, so it gets the reading's own line.
fn bomb_meter() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(24.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
        }
        Children [
            // The pad's binding, because it is the one worth a picture: both bumpers, drawn as one
            // chord.
            Text
            Children [
                IconPromptSpan(SmartBomb)
                ~{PromptFamily(DeviceFamily::Gamepad)}
                TextFont { font_size: 15.0_f32 }
                TextColor(Color::srgb(0.5, 0.6, 0.6))
            ]
            --
            Node {
                width: Val::Px(200.0),
                height: Val::Px(8.0),
                border: {UiRect::all(Val::Px(1.0))},
            }
            BorderColor::all(Color::srgb(0.5, 0.6, 0.6))
            Children [
                BombMeterFill
                Node { width: Val::Percent(0.0), height: Val::Percent(100.0) }
                BackgroundColor(Color::srgb(0.6, 0.85, 1.0))
            ]
        ]
    }
}

/// Reads the same number [`SmartBomb`]'s own hold condition already keeps, rather than timing the
/// charge a second time here.
fn redraw_bomb_meter(
    input: ContextActions<Flying>,
    mut fill: Query<&mut Node, With<BombMeterFill>>,
) {
    let Ok(mut fill) = fill.single_mut() else {
        return;
    };
    fill.width = Val::Percent(input.progress::<SmartBomb>() * 100.0);
}

fn show_exhaust(
    input: ContextActions<Flying>,
    exhaust: Query<&mut Transform, With<Exhaust>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    flame: Query<&MeshMaterial2d<ColorMaterial>, With<Exhaust>>,
) {
    let thrust = input.value::<Thrust>();
    // `Started` and `Building` mean the burn is charging but has not opened up yet, so the flame
    // can grow before the ship actually goes anywhere.
    let charging = matches!(
        input.phase::<Afterburner>(),
        ActionPhase::Started | ActionPhase::Building
    );
    let boosting = input.value::<Afterburner>();

    for mut transform in exhaust {
        // Drawn every frame rather than every tick, and it reads the same action either way.
        let stretch = if boosting { 1.9 } else { 1.0 };
        transform.scale = Vec3::new(thrust * stretch, thrust, thrust);
    }

    for material in flame {
        if let Some(mut material) = materials.get_mut(&material.0) {
            material.color = match (boosting, charging) {
                (true, _) => Color::srgb(0.6, 0.85, 1.0),
                (_, true) => Color::srgb(1.0, 0.85, 0.4),
                _ => Color::srgb(1.0, 0.6, 0.2),
            };
        }
    }
}

/// A cheap uniform in `0.0..1.0`, so the example needs no random-number dependency.
pub fn rand_unit() -> f32 {
    use std::cell::Cell;
    use std::num::Wrapping;

    thread_local! {
        static STATE: Cell<Wrapping<u32>> = const { Cell::new(Wrapping(0x9E37_79B9)) };
    }

    STATE.with(|state| {
        let mut x = state.get();
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        state.set(x);
        (x.0 >> 8) as f32 / (1u32 << 24) as f32
    })
}
