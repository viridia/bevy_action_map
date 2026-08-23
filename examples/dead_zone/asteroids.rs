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

#[derive(Component, Default, Clone)]
pub struct Asteroid;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, starting_rocks.spawn());
    app.add_systems(FixedUpdate, shatter_on_hit.in_set(Simulating));
}

/// The opening spread of rocks: a `SceneList` rather than a `Scene`, because these are six sibling
/// entities and not one object with parts.
///
/// A `Vec` of scenes is itself a `SceneList`, so the loop that used to spawn is now a loop that
/// builds.
fn starting_rocks() -> impl SceneList {
    (0..6)
        .map(|_| {
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
            asteroid(Size::Large, position, drift())
        })
        .collect::<Vec<_>>()
}

fn shatter_on_hit(
    mut commands: Commands,
    asteroids: Query<(Entity, &Transform, &Size), With<Asteroid>>,
    bullets: Query<(Entity, &Transform), With<Bullet>>,
) {
    for (bullet, bullet_at) in bullets.iter() {
        for (rock, rock_at, size) in asteroids.iter() {
            let hit = bullet_at.translation.distance(rock_at.translation) < size.radius();
            if !hit {
                continue;
            }

            commands.entity(bullet).try_despawn();
            commands.entity(rock).try_despawn();

            if let Some(smaller) = size.smaller() {
                for _ in 0..2 {
                    commands.spawn_scene(asteroid(
                        smaller,
                        rock_at.translation.truncate(),
                        drift() * 1.6,
                    ));
                }
            }
            break;
        }
    }
}

/// One rock, as a scene.
///
/// The same function serves the opening field and every shatter after it — which is the point of a
/// scene being a value: `starting_rocks` collects six of these into a list, and `shatter_on_hit`
/// hands two at a time straight to `spawn_scene`.
fn asteroid(size: Size, position: Vec2, velocity: Vec2) -> impl Scene {
    bsn! {
        Asteroid
        // `Size` is a runtime value rather than a variant written into the block, so it goes in as
        // a template value. Naming the variant directly — `Size::Large` — would work too.
        template_value(size)
        Mesh2d(asset_value(RegularPolygon::new(size.radius(), 7)))
        MeshMaterial2d::<ColorMaterial>(asset_value(Color::srgb(0.45, 0.45, 0.5)))
        // A patch, so the two fields that matter are set and `scale` keeps its default. This is why
        // the builder chain (`from_translation(..).with_rotation(..)`) is no longer needed.
        Transform {
            translation: {position.extend(0.0)},
            rotation: {Quat::from_rotation_z(rand_unit() * TAU)},
        }
        Velocity({velocity})
        Wraps
    }
}

fn drift() -> Vec2 {
    let angle = rand_unit() * TAU;
    Vec2::from_angle(angle) * (30.0 + rand_unit() * 45.0)
}
