//! Every action, context and binding in the game.
//!
//! This is deliberately the whole of the input layer. If you want to know what Dead Zone does with
//! `bevy_action_map`, this file is it; everything else in the directory is ordinary game code that
//! reads these actions and never mentions a key or a button.

use bevy::prelude::*;
use bevy_action_map::prelude::*;
use bevy_input::{gamepad::GamepadButton, keyboard::KeyCode};

use crate::pause::Game;

/// How hard the engine is burning, from 0 to 1.
///
/// Analog on purpose. A trigger gives the player fine control over a ship that keeps its momentum,
/// and a key gives them all of it at once — the same action either way.
#[derive(InputAction)]
#[action(path = "dead_zone.thrust", output = f32, intent = Analog1)]
pub struct Thrust;

/// Which way the ship is turning, negative for anticlockwise.
#[derive(InputAction)]
#[action(path = "dead_zone.turn", output = f32, intent = Analog1)]
pub struct Turn;

#[derive(InputAction)]
#[action(path = "dead_zone.fire", output = bool, intent = Button)]
pub struct Fire;

/// Jump somewhere else on the field, at some risk.
///
/// Double-tapped rather than pressed, so that panicking on the fire button cannot fling the ship
/// across the screen by accident.
#[derive(InputAction)]
#[action(path = "dead_zone.hyperspace", output = bool, intent = Button)]
pub struct Hyperspace;

/// A harder burn, available once the engine has been running a while.
///
/// Bound to the same controls as [`Thrust`], with a hold condition. One control, two actions, and
/// the difference between them is entirely in when they are considered to have fired.
#[derive(InputAction)]
#[action(path = "dead_zone.afterburner", output = bool, intent = Button)]
pub struct Afterburner;

/// Open or close the pause menu.
#[derive(InputAction)]
#[action(path = "dead_zone.pause", output = bool, intent = Button)]
pub struct Pause;

/// The context a living ship flies under.
///
/// Fixed tick, because the ship integrates its own velocity and a frame-rate-dependent burn would
/// make the game play differently on different machines.
#[derive(InputContext, Component)]
#[context(path = "dead_zone.flying", tick = Fixed)]
pub struct Flying;

/// The controls that work whatever the game is doing.
///
/// Pause lives here rather than in [`Flying`] for the reason it always does: something has to hear
/// the button that unpauses, and the context the player was flying under is exactly the one that is
/// no longer listening. Anything else the player can always reach — a settings screen, a quit
/// prompt — belongs here beside it.
///
/// Render tick, because these answer at the frame rate rather than the simulation rate, and while
/// the game is paused there is no simulation to answer at.
#[derive(InputContext, Component)]
#[context(path = "dead_zone.shell", tick = Render)]
pub struct Shell;

/// Declares the control scheme.
///
/// Every action is bound twice, once for each device, and neither knows about the other. Reading
/// `Thrust` gives a number whether it came from a trigger or a key.
pub fn plugin(app: &mut App) {
    app.add_context::<Flying>(|controls| {
        controls.active_in_state(Game::Playing);

        // The trigger reports a fraction on a button channel, so it drives an analog action
        // directly. A key has only two positions, which is a coarse analog control rather than a
        // different kind of thing.
        controls.bind::<Thrust>(GamepadButton::RightTrigger2);
        controls.bind::<Thrust>(KeyCode::KeyW);
        controls.bind::<Thrust>(KeyCode::ArrowUp);

        // A stick axis is already signed; two keys need a composite to become one. The deadzone is
        // what stops a worn stick from turning the ship while the player is not touching it.
        controls
            .bind::<Turn>(GamepadAxis::LeftStickX)
            .dead_zone(DeadZone::radial(0.15));
        controls.bind::<Turn>(AxisButtons::ad());
        controls.bind::<Turn>(AxisButtons::left_right());

        controls.bind::<Fire>(GamepadButton::South);
        controls.bind::<Fire>(KeyCode::Space);

        // Hold the throttle for three quarters of a second and it opens up. `Started` fires the
        // moment the burn begins, so the exhaust can show it building before it arrives.
        controls
            .bind::<Afterburner>(GamepadButton::RightTrigger2)
            .hold(0.75);
        controls.bind::<Afterburner>(KeyCode::KeyW).hold(0.75);
        controls.bind::<Afterburner>(KeyCode::ArrowUp).hold(0.75);

        controls
            .bind::<Hyperspace>(GamepadButton::East)
            .multi_tap(2, 0.3);
        controls
            .bind::<Hyperspace>(KeyCode::ShiftLeft)
            .multi_tap(2, 0.3);
    });

    // No condition, so this one is live from the moment its entity exists and stays that way. Pause
    // is one action bound once, and the state it toggles is what the flying context follows.
    app.add_context::<Shell>(|controls| {
        controls.bind::<Pause>(KeyCode::Escape);
        controls.bind::<Pause>(GamepadButton::Start);
    });

    app.add_systems(Startup, spawn_shell);
}

/// The always-on controls have to belong to something, the same as the ship's do.
///
/// Nothing else is on this entity: a context that is not attached to a player or a world object is
/// still attached to an entity, because that is what an observer targets.
fn spawn_shell(mut commands: Commands) {
    commands.spawn(Shell);
}
