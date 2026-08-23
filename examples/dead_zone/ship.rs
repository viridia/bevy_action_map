//! The player's ship: how it flies, and what it does when told to.

use bevy::prelude::*;
use bevy_action_map::prelude::*;
use std::f32::consts::TAU;

use crate::actions::{Fire, Flying, Hyperspace, Thrust, Turn};
use crate::field::{HALF_EXTENT, Lifetime, Velocity, Wraps};

const TURN_RATE: f32 = 3.2;
const ACCELERATION: f32 = 420.0;
/// Space is not really like this, but a ship that never slows down is miserable to fly.
const DRAG: f32 = 0.4;
const MUZZLE_SPEED: f32 = 620.0;
const RELOAD: f32 = 0.18;

#[derive(Component)]
pub struct Ship {
    reload: Timer,
}

#[derive(Component)]
pub struct Bullet;

/// The flame drawn behind the ship, scaled by how hard the engine is burning.
#[derive(Component)]
struct Exhaust;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_ship);
    app.add_systems(FixedUpdate, (fly, shoot, hyperspace));
    app.add_systems(Update, show_exhaust);
}

fn spawn_ship(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let hull = meshes.add(Triangle2d::new(
        Vec2::new(18.0, 0.0),
        Vec2::new(-12.0, 11.0),
        Vec2::new(-12.0, -11.0),
    ));
    // Symmetric about the nose-to-tail axis, or it burns off to one side.
    let flame = meshes.add(Triangle2d::new(
        Vec2::new(-12.0, -6.0),
        Vec2::new(-12.0, 6.0),
        Vec2::new(-30.0, 0.0),
    ));

    commands
        .spawn((
            Ship {
                reload: Timer::from_seconds(RELOAD, TimerMode::Once),
            },
            // The context is a component, so the ship simply carries its own controls.
            Flying,
            Mesh2d(hull),
            MeshMaterial2d(materials.add(Color::srgb(0.85, 0.9, 1.0))),
            Transform::default(),
            Velocity::default(),
            Wraps,
        ))
        .with_child((
            Exhaust,
            Mesh2d(flame),
            MeshMaterial2d(materials.add(Color::srgb(1.0, 0.6, 0.2))),
            Transform::from_scale(Vec3::ZERO),
        ));
}

fn fly(
    time: Res<Time>,
    input: Actions<Flying>,
    ships: Query<(&mut Transform, &mut Velocity), With<Ship>>,
) {
    let delta = time.delta_secs();
    let turn = input.value::<Turn>();
    let thrust = input.value::<Thrust>();

    for (mut transform, mut velocity) in ships {
        transform.rotate_z(-turn * TURN_RATE * delta);

        // The ship accelerates along its nose, which is what makes turning and thrusting two
        // separate decisions rather than one.
        let heading = (transform.rotation * Vec3::X).truncate();
        velocity.0 += heading * thrust * ACCELERATION * delta;
        velocity.0 *= 1.0 - DRAG * delta;
    }
}

fn shoot(
    time: Res<Time>,
    input: Actions<Flying>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    ships: Query<(&mut Ship, &Transform, &Velocity)>,
) {
    for (mut ship, transform, velocity) in ships {
        ship.reload.tick(time.delta());
        if !input.value::<Fire>() || !ship.reload.is_finished() {
            continue;
        }
        ship.reload.reset();

        let heading = (transform.rotation * Vec3::X).truncate();
        commands.spawn((
            Bullet,
            Mesh2d(meshes.add(Circle::new(2.5))),
            MeshMaterial2d(materials.add(Color::srgb(1.0, 0.95, 0.6))),
            Transform::from_translation(transform.translation + (heading * 20.0).extend(0.0)),
            // Inheriting the ship's velocity is what makes flying backwards while firing forwards
            // feel like it should.
            Velocity(velocity.0 + heading * MUZZLE_SPEED),
            Wraps,
            Lifetime(Timer::from_seconds(1.1, TimerMode::Once)),
        ));
    }
}

fn hyperspace(input: Actions<Flying>, ships: Query<(&mut Transform, &mut Velocity), With<Ship>>) {
    // `fired` is the press itself rather than the hold, so a held key jumps once.
    if !input.fired::<Hyperspace>() {
        return;
    }

    for (mut transform, mut velocity) in ships {
        let angle = rand_unit() * TAU;
        transform.translation = Vec3::new(
            (rand_unit() * 2.0 - 1.0) * HALF_EXTENT.x,
            (rand_unit() * 2.0 - 1.0) * HALF_EXTENT.y,
            0.0,
        );
        transform.rotation = Quat::from_rotation_z(angle);
        velocity.0 = Vec2::ZERO;
    }
}

fn show_exhaust(input: Actions<Flying>, exhaust: Query<&mut Transform, With<Exhaust>>) {
    let thrust = input.value::<Thrust>();
    for mut transform in exhaust {
        // Drawn every frame rather than every tick, and it reads the same action either way.
        transform.scale = Vec3::splat(thrust);
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
