//! The countdown before each serve, and the serve itself.
//!
//! A rally is in one of three states, and [`gate`] is the only thing that turns that into input: it
//! switches `Move` and [`Serve`] off and on to match. Neither is ever unbound, and nothing that
//! reads them checks the rally first. [`serve`] launches the ball on any press it sees, because
//! outside [`Rally::Waiting`] there is no press to see.
//!
//! Switching `Serve` back on does not count a button already held as a press, so holding it through
//! the countdown serves nothing. `Move` is analog and has no press to hold back: a key held through
//! the countdown moves the paddle the moment it ends.

use bevy::prelude::*;
use bevy_action_map::prelude::*;

use crate::pong::ball::{self, Ball, Velocity};
use crate::pong::paddle::{self, Move, Paddle};
use crate::pong::score::Score;

/// Launches the ball from center.
#[derive(InputAction)]
#[action(path = "pong_countdown.serve", output = bool, intent = Button)]
pub struct Serve;

const COUNTDOWN_SECS: f32 = 3.0;

#[derive(Resource)]
enum Rally {
    /// Nobody moves or serves until the timer runs out.
    Countdown(Timer),
    /// The paddles move, and the ball waits for a serve.
    Waiting,
    InPlay,
}

impl Rally {
    fn countdown() -> Self {
        Self::Countdown(Timer::from_seconds(COUNTDOWN_SECS, TimerMode::Once))
    }
}

#[derive(Component, Clone, Default)]
struct Announcement;

pub fn plugin(app: &mut App) {
    app.add_context::<Paddle>(|controls| {
        paddle::bindings(controls);
        controls.bind::<Serve>(KeyCode::Space);
        controls.bind::<Serve>(GamepadButton::South);
    });
    app.insert_resource(Rally::countdown());

    app.add_systems(Startup, (paddle::spawn, spawn_ball, announcement.spawn()));
    // Before evaluation, so a paddle is never read on a tick its switches have not caught up with.
    app.add_systems(FixedPreUpdate, gate.before(ActionMapSystems::Evaluate));
    app.add_systems(
        FixedUpdate,
        (
            count_down,
            serve,
            paddle::walk::<Paddle>,
            ball::fly,
            ball::bounce,
            check_goal,
        )
            .chain(),
    );
    app.add_systems(Update, (paddle::pair_gamepad, redraw));
}

fn spawn_ball(mut commands: Commands) {
    commands.spawn_scene(ball::ball(Vec2::ZERO));
}

/// Switches each paddle's actions to match the rally.
///
/// Every tick rather than when the rally changes, so a pad paired mid-countdown is frozen along
/// with the keyboard. Compared before writing, because a write through `Mut` marks the state
/// changed for every subscriber whether or not a switch moved.
fn gate(rally: Res<Rally>, mut paddles: Query<&mut InputContextState<Paddle>>) {
    let (moving, serving) = match *rally {
        Rally::Countdown(_) => (false, false),
        Rally::Waiting => (true, true),
        Rally::InPlay => (true, false),
    };
    for mut input in &mut paddles {
        switch::<Move>(&mut input, moving);
        switch::<Serve>(&mut input, serving);
    }
}

fn switch<A: InputAction>(input: &mut Mut<InputContextState<Paddle>>, on: bool) {
    if input.is_enabled::<A>() == on {
        return;
    }
    if on {
        input.enable::<A>();
    } else {
        input.disable::<A>();
    }
}

fn count_down(time: Res<Time>, mut rally: ResMut<Rally>) {
    if let Rally::Countdown(timer) = &mut *rally
        && timer.tick(time.delta()).is_finished()
    {
        *rally = Rally::Waiting;
    }
}

fn serve(
    input: ActionsQuery<Paddle>,
    mut rally: ResMut<Rally>,
    mut ball: Query<&mut Velocity, With<Ball>>,
) {
    if !input.iter().any(|(_, state)| state.fired::<Serve>()) {
        return;
    }
    let Ok(mut velocity) = ball.single_mut() else {
        return;
    };
    velocity.0 = ball::serve();
    *rally = Rally::InPlay;
}

fn check_goal(
    mut ball: Query<(&mut Transform, &mut Velocity), With<Ball>>,
    mut score: ResMut<Score>,
    mut rally: ResMut<Rally>,
) {
    let Ok((mut transform, mut velocity)) = ball.single_mut() else {
        return;
    };
    let Some(side) = ball::scorer(transform.translation.x) else {
        return;
    };
    score.add(side);
    transform.translation = Vec3::ZERO;
    velocity.0 = Vec2::ZERO;
    *rally = Rally::countdown();
}

fn announcement() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(30.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            justify_content: JustifyContent::Center,
        }
        Children [
            Announcement
            Text::new("")
            TextFont { font_size: 28.0_f32 }
            TextColor(Color::WHITE)
        ]
    }
}

fn redraw(rally: Res<Rally>, mut text: Query<&mut Text, With<Announcement>>) {
    if !rally.is_changed() {
        return;
    }
    let line = match &*rally {
        Rally::Countdown(timer) => format!("{}", timer.remaining_secs().ceil()),
        Rally::Waiting => "Space, or South on the pad, to serve".to_string(),
        Rally::InPlay => String::new(),
    };
    for mut text in &mut text {
        if text.0 != line {
            text.0.clone_from(&line);
        }
    }
}
