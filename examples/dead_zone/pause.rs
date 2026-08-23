//! Pausing: a game state, and the context that follows it.

use bevy::prelude::*;
use bevy_action_map::prelude::*;

use crate::actions::Pause;

/// Whether the simulation is running.
///
/// [`Flying`](crate::actions::Flying) follows this rather than the other way round, so there is only
/// one fact about whether the game is paused and nothing to keep in step with it.
#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Game {
    #[default]
    Playing,
    Paused,
}

/// Everything that only happens while the game is running.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Simulating;

#[derive(Component, Default, Clone)]
struct PausedBanner;

pub fn plugin(app: &mut App) {
    app.init_state::<Game>();
    app.configure_sets(Update, Simulating.run_if(in_state(Game::Playing)));
    app.configure_sets(FixedUpdate, Simulating.run_if(in_state(Game::Playing)));
    app.add_systems(OnEnter(Game::Paused), banner.spawn());
    app.add_systems(OnExit(Game::Paused), hide_banner);
}

/// Flips the state; the flying context follows on its own.
///
/// An observer rather than a polled system: the press is a single event, and polling for it from
/// `Update` would see the same `Fired` twice on a frame where no fixed tick ran.
///
/// It is attached by [`actions::shell`](crate::actions::shell), which spawns the context this
/// listens to, rather than registered globally with `App::add_observer`. That is why this is
/// `pub(crate)`: the reaction is written next to the context whose action it answers, and pausing
/// is otherwise entirely this file's business.
pub(crate) fn toggle(
    _: On<Fired<Pause>>,
    game: Res<State<Game>>,
    mut next: ResMut<NextState<Game>>,
) {
    next.set(match game.get() {
        Game::Playing => Game::Paused,
        Game::Paused => Game::Playing,
    });
}

/// The banner, as a scene.
///
/// `.spawn()` is normally reserved for `Startup`, but `OnEnter` is the same kind of schedule for
/// this purpose: it runs once each time the game is paused, and spawns one banner when it does.
fn banner() -> impl Scene {
    bsn! {
        PausedBanner
        Text::new("PAUSED")
        TextFont { font_size: 48.0_f32 }
        // A patch: only the three fields that differ are named, and the rest of `Node` keeps its
        // defaults without a `..default()` to say so.
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(42.0),
            left: Val::Percent(40.0),
        }
    }
}

fn hide_banner(mut commands: Commands, banner: Query<Entity, With<PausedBanner>>) {
    for entity in banner {
        commands.entity(entity).try_despawn();
    }
}
