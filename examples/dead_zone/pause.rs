//! Pausing: a game state, and the contexts that follow it.

use bevy::prelude::*;
use bevy_action_map::prelude::*;

use crate::actions::{Pause, PauseMenu};

/// Whether the simulation is running.
///
/// The contexts follow this rather than the other way round, so there is only one fact about
/// whether the game is paused and nothing to keep in step with it.
#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Game {
    #[default]
    Playing,
    Paused,
}

/// Everything that only happens while the game is running.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Simulating;

#[derive(Component)]
struct PausedBanner;

pub fn plugin(app: &mut App) {
    app.init_state::<Game>();
    app.add_systems(Startup, spawn_menu_context);
    app.configure_sets(Update, Simulating.run_if(in_state(Game::Playing)));
    app.configure_sets(FixedUpdate, Simulating.run_if(in_state(Game::Playing)));
    app.add_observer(toggle);
    app.add_systems(OnEnter(Game::Paused), show_banner);
    app.add_systems(OnExit(Game::Paused), hide_banner);
}

/// The menu's controls have to belong to something, the same as the ship's do.
///
/// Nothing else is on this entity: a context that is not attached to a player or a world object is
/// still attached to an entity, because that is what an observer targets.
fn spawn_menu_context(mut commands: Commands) {
    commands.spawn(PauseMenu);
}

/// Flips the state; the contexts follow on their own.
///
/// An observer rather than a polled system: the press is a single event, and polling for it from
/// `Update` would see the same `Fired` twice on a frame where no fixed tick ran.
fn toggle(_: On<Fired<Pause>>, game: Res<State<Game>>, mut next: ResMut<NextState<Game>>) {
    next.set(match game.get() {
        Game::Playing => Game::Paused,
        Game::Paused => Game::Playing,
    });
}

fn show_banner(mut commands: Commands) {
    commands.spawn((
        PausedBanner,
        Text::new("PAUSED"),
        TextFont::from_font_size(48.0),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(42.0),
            left: Val::Percent(40.0),
            ..default()
        },
    ));
}

fn hide_banner(mut commands: Commands, banner: Query<Entity, With<PausedBanner>>) {
    for entity in banner {
        commands.entity(entity).try_despawn();
    }
}
