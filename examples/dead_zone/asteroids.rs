//! Rocks that drift, and break into smaller rocks when shot.

use bevy::prelude::*;
use std::f32::consts::TAU;

use crate::field::{HALF_EXTENT, Velocity, Wraps};
use crate::pause::Simulating;
use crate::ship::{Bullet, rand_unit};

/// How many times a rock has already been broken.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Large,
    Medium,
    Small,
}

impl Size {
    fn radius(self) -> f32 {
        match self {
            Self::Large => 44.0,
            Self::Medium => 26.0,
            Self::Small => 14.0,
        }
    }

    fn smaller(self) -> Option<Self> {
        match self {
            Self::Large => Some(Self::Medium),
            Self::Medium => Some(Self::Small),
            Self::Small => None,
        }
    }
}

#[derive(Component)]
pub struct Asteroid;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_field);
    app.add_systems(FixedUpdate, shatter_on_hit.in_set(Simulating));
}

fn spawn_field(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for _ in 0..6 {
        // Kept away from the middle so the ship does not start inside a rock.
        let edge = Vec2::new(
            (rand_unit() * 2.0 - 1.0) * HALF_EXTENT.x,
            (rand_unit() * 2.0 - 1.0) * HALF_EXTENT.y,
        );
        let position = if edge.length() < 160.0 {
            edge.normalize_or(Vec2::X) * 160.0
        } else {
            edge
        };
        spawn_asteroid(
            &mut commands,
            &mut meshes,
            &mut materials,
            Size::Large,
            position,
            drift(),
        );
    }
}

fn shatter_on_hit(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asteroids: Query<(Entity, &Transform, &Size), With<Asteroid>>,
    bullets: Query<(Entity, &Transform), With<Bullet>>,
) {
    for (bullet, bullet_at) in bullets.iter() {
        for (asteroid, asteroid_at, size) in asteroids.iter() {
            let hit = bullet_at.translation.distance(asteroid_at.translation) < size.radius();
            if !hit {
                continue;
            }

            commands.entity(bullet).try_despawn();
            commands.entity(asteroid).try_despawn();

            if let Some(smaller) = size.smaller() {
                for _ in 0..2 {
                    spawn_asteroid(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        smaller,
                        asteroid_at.translation.truncate(),
                        drift() * 1.6,
                    );
                }
            }
            break;
        }
    }
}

fn spawn_asteroid(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    size: Size,
    position: Vec2,
    velocity: Vec2,
) {
    commands.spawn((
        Asteroid,
        size,
        Mesh2d(meshes.add(RegularPolygon::new(size.radius(), 7))),
        MeshMaterial2d(materials.add(Color::srgb(0.45, 0.45, 0.5))),
        Transform::from_translation(position.extend(0.0))
            .with_rotation(Quat::from_rotation_z(rand_unit() * TAU)),
        Velocity(velocity),
        Wraps,
    ));
}

fn drift() -> Vec2 {
    let angle = rand_unit() * TAU;
    Vec2::from_angle(angle) * (30.0 + rand_unit() * 45.0)
}
