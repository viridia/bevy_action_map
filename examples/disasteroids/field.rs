//! The playfield: its size, and the wrapping that makes it a torus.
//!
//! Everything here is spawned from a `bsn!` block somewhere, which is why each component derives
//! `Default` and `Clone`: that pair is what BSN asks of a component before it will let you write
//! the type's name in a scene.

use bevy::prelude::*;

use crate::pause::Simulating;

/// Half the width and height of the field, in world units.
pub const HALF_EXTENT: Vec2 = Vec2::new(640.0, 360.0);

/// Anything that leaves one edge of the field and comes back on the other.
#[derive(Component, Default, Clone)]
pub struct Wraps;

/// A velocity in world units per second.
#[derive(Component, Default, Clone, Deref, DerefMut)]
pub struct Velocity(pub Vec2);

/// How long an entity has left before it removes itself.
#[derive(Component, Default, Clone, Deref, DerefMut)]
pub struct Lifetime(pub Timer);

pub fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, (integrate, wrap).chain().in_set(Simulating));
    app.add_systems(Update, expire.in_set(Simulating));
}

fn integrate(time: Res<Time>, movers: Query<(&mut Transform, &Velocity)>) {
    let delta = time.delta_secs();
    for (mut transform, velocity) in movers {
        transform.translation += (velocity.0 * delta).extend(0.0);
    }
}

fn wrap(wrapping: Query<&mut Transform, With<Wraps>>) {
    for mut transform in wrapping {
        let position = transform.translation.truncate();
        // rem_euclid maps onto `0..size`, so shift into that range and back again.
        let size = HALF_EXTENT * 2.0;
        let wrapped = (position + HALF_EXTENT).rem_euclid(size) - HALF_EXTENT;
        if wrapped != position {
            transform.translation = wrapped.extend(transform.translation.z);
        }
    }
}

fn expire(time: Res<Time>, mut commands: Commands, expiring: Query<(Entity, &mut Lifetime)>) {
    for (entity, mut lifetime) in expiring {
        if lifetime.tick(time.delta()).just_finished() {
            commands.entity(entity).try_despawn();
        }
    }
}
