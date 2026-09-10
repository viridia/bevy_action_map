//! The left paddle, and the authority that drives it.
//!
//! An authority is anything outside this crate that resolves an action itself — a platform's own
//! input service, a network peer, or, here, twenty lines of ball-chasing. It writes an
//! [`AuthorityValues`] component on the context's entity, and the action it writes fires, completes
//! and cancels on the edges of that value exactly as a bound one does. [`super::pong::paddle::walk`]
//! is the proof: the same system moves both paddles and asks nothing about where either value
//! came from.
//!
//! Two contexts rather than one, because a context that binds an action may not also delegate it —
//! declaring both is a plan-build error. [`Robot`] binds nothing at all; Pong's own `Paddle` keeps
//! its full control list for the human on the right.
//!
//! No device pairing here, unlike the base. Pairing exists to keep two humans on one machine out of
//! each other's controls, and the robot reads no device to be kept out of.

use bevy::prelude::*;
use bevy_action_map::prelude::*;

use crate::pong::ball::Ball;
use crate::pong::court::HALF_EXTENT;
use crate::pong::paddle::{HEIGHT, INSET, Move, Paddle, Side, paddle, walk};

/// The left paddle's context: one action, no controls, and an authority to supply it.
#[derive(InputContext)]
#[context(path = "pong_robot.robot", tick = Fixed)]
pub struct Robot;

pub fn plugin(app: &mut App) {
    app.add_context::<Paddle>(crate::pong::paddle::bindings);
    app.add_context::<Robot>(|controls| {
        controls.delegate::<Move>();
    });

    app.add_systems(Startup, spawn);
    app.add_systems(FixedUpdate, (walk::<Paddle>, walk::<Robot>));
    // Before evaluation, in the schedule the two contexts tick in: a value written after it would
    // land on the following tick instead.
    app.add_systems(
        FixedPreUpdate,
        track_ball.before(ActionMapSystems::Evaluate),
    );
}

/// The same scene on both sides, differing only in which context reads for it. The human's paddle
/// is deliberately unpaired, so one player can use the keyboard or a pad without a join gesture.
fn spawn(mut commands: Commands) {
    commands
        .spawn_scene(paddle(Side::LEFT, -(HALF_EXTENT.x - INSET)))
        .insert((Robot, AuthorityValues::new()));
    commands
        .spawn_scene(paddle(Side::RIGHT, HALF_EXTENT.x - INSET))
        .insert(Paddle);
}

/// The whole authority: where the ball is, relative to the paddle chasing it.
///
/// `Move` is an `Analog1` action, so what it wants is a rate in `-1.0..=1.0` rather than a
/// coordinate. Dividing by the paddle's own half-height gives one: full speed while the ball is
/// more than half a paddle away, easing to nothing as it arrives, which is the same measure
/// `ball::bounce` takes to decide the angle it leaves at.
fn track_ball(
    ball: Query<&Transform, With<Ball>>,
    mut robots: Query<(&Transform, &mut AuthorityValues), With<Robot>>,
) {
    let Ok(ball) = ball.single() else {
        return;
    };
    for (transform, mut values) in &mut robots {
        let error = ball.translation.y - transform.translation.y;
        values.set::<Move>((error / (HEIGHT / 2.0)).clamp(-1.0, 1.0));
    }
}
