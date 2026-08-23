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

/// The context the pause menu runs under.
///
/// Render tick, because a menu answers at the frame rate rather than the simulation rate — and
/// while it is up there is no simulation to answer at.
#[derive(InputContext, Component)]
#[context(path = "dead_zone.pause_menu", tick = Render)]
pub struct PauseMenu;

/// Declares the control scheme.
///
/// Every action is bound twice, once for each device, and neither knows about the other. Reading
/// `Thrust` gives a number whether it came from a trigger or a key.
pub fn plugin(app: &mut App) {
    app.add_context_in_state::<Flying>(Game::Playing, |controls| {
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

        controls.bind::<Pause>(KeyCode::Escape);
        controls.bind::<Pause>(GamepadButton::Start);
    });

    // The same control, bound again in the context that the first one hands over to. This is the
    // arrangement that goes wrong without require-reset: the key that closes the menu is still
    // down when flying resumes, and a context that started reading it now would see a press meant
    // for the menu. Following the state is what makes that somebody else's problem — neither
    // context is told when to activate, and neither can be told wrongly.
    app.add_context_in_state::<PauseMenu>(Game::Paused, |controls| {
        controls.bind::<Pause>(KeyCode::Escape);
        controls.bind::<Pause>(GamepadButton::Start);
    });
}
