//! The ball: flight, bouncing off the court and the paddles, and scoring when it gets past one.

use bevy::prelude::*;

use super::court::HALF_EXTENT;
use super::paddle::{HEIGHT as PADDLE_HEIGHT, Side, WIDTH as PADDLE_WIDTH};
use super::score::Score;

const RADIUS: f32 = 7.0;
const SPEED: f32 = 260.0;
/// How far off dead center a paddle can send the ball, at the paddle's own edge.
const MAX_BOUNCE_ANGLE: f32 = 60.0_f32 * core::f32::consts::PI / 180.0;

#[derive(Component, Clone, Default)]
struct Ball;

#[derive(Component, Clone, Copy, Default, Deref, DerefMut)]
struct Velocity(Vec2);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, ball.spawn());
    app.add_systems(FixedUpdate, (fly, bounce, check_goal).chain());
}

fn ball() -> impl Scene {
    bsn! {
        Ball
        Mesh2d(asset_value(Circle::new(RADIUS)))
        MeshMaterial2d::<ColorMaterial>(asset_value(Color::WHITE))
        Velocity(serve())
    }
}

fn fly(time: Res<Time>, mut ball: Query<(&mut Transform, &Velocity), With<Ball>>) {
    let Ok((mut transform, velocity)) = ball.single_mut() else {
        return;
    };
    transform.translation += (velocity.0 * time.delta_secs()).extend(0.0);
}

/// Reflects the ball off the top and bottom walls, and off either paddle.
///
/// A paddle only turns the ball around if it was actually heading toward that paddle — otherwise a
/// ball grazing past a paddle's own edge on its way to the other side would bounce off the back of
/// it, which reads as the paddle reaching through the court. What the ball leaves a paddle with is
/// entirely a function of where it landed on the paddle, per [`depart`] — its incoming velocity
/// never enters the calculation, which is the classic Pong rule.
fn bounce(
    mut ball: Query<(&mut Transform, &mut Velocity), With<Ball>>,
    paddles: Query<(&Side, &Transform), Without<Ball>>,
) {
    let Ok((mut transform, mut velocity)) = ball.single_mut() else {
        return;
    };
    let limit = HALF_EXTENT.y - RADIUS;
    if transform.translation.y > limit && velocity.0.y > 0.0 {
        transform.translation.y = limit;
        velocity.0.y = -velocity.0.y;
    } else if transform.translation.y < -limit && velocity.0.y < 0.0 {
        transform.translation.y = -limit;
        velocity.0.y = -velocity.0.y;
    }

    let pos = transform.translation.truncate();
    for (side, paddle) in &paddles {
        let approaching = if *side == Side::LEFT {
            velocity.0.x < 0.0
        } else {
            velocity.0.x > 0.0
        };
        if !approaching {
            continue;
        }
        let paddle_pos = paddle.translation.truncate();
        let within_x = (pos.x - paddle_pos.x).abs() < PADDLE_WIDTH / 2.0 + RADIUS;
        let within_y = (pos.y - paddle_pos.y).abs() < PADDLE_HEIGHT / 2.0 + RADIUS;
        if within_x && within_y {
            let offset = ((pos.y - paddle_pos.y) / (PADDLE_HEIGHT / 2.0)).clamp(-1.0, 1.0);
            velocity.0 = depart(*side, offset);
        }
    }
}

fn check_goal(
    mut ball: Query<(&mut Transform, &mut Velocity), With<Ball>>,
    mut score: ResMut<Score>,
) {
    let Ok((mut transform, mut velocity)) = ball.single_mut() else {
        return;
    };
    let x = transform.translation.x;
    if x - RADIUS > HALF_EXTENT.x {
        score.add(Side::LEFT);
    } else if x + RADIUS < -HALF_EXTENT.x {
        score.add(Side::RIGHT);
    } else {
        return;
    }
    transform.translation = Vec3::ZERO;
    velocity.0 = serve();
}

/// The ball's velocity leaving a paddle: away from that side, at a fixed speed, angled by how far
/// off center it was hit and nothing else.
fn depart(side: Side, offset: f32) -> Vec2 {
    let angle = offset * MAX_BOUNCE_ANGLE;
    let dir = if side == Side::LEFT { 1.0 } else { -1.0 };
    Vec2::new(dir * angle.cos(), angle.sin()) * SPEED
}

/// A fresh serve toward a random side, angled at random within the same range a paddle hit can
/// produce.
fn serve() -> Vec2 {
    let side = if rand_unit() < 0.5 {
        Side::LEFT
    } else {
        Side::RIGHT
    };
    depart(side, rand_unit() * 2.0 - 1.0)
}

/// A cheap uniform in `0.0..1.0`, so the example needs no random-number dependency.
fn rand_unit() -> f32 {
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
