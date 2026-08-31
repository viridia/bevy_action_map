//! Every action, context and binding in the game.
//!
//! Every context is a scene carrying both the [`InputContext`] component and the observers for
//! its actions — see [`shell`].

use bevy::prelude::*;
use bevy_action_map::prelude::*;
use bevy_input::{gamepad::GamepadButton, keyboard::KeyCode, mouse::MouseButton};

use crate::pause::{self, Game};
use crate::ship::RELOAD;

// The categories below are localization keys rather than words on screen: a rebinding screen
// groups by them, and the game's translation catalogue decides what "disasteroids.flight" reads as.

/// How hard the engine is burning, from 0 to 1.
///
/// Analog on purpose. A trigger gives the player fine control over a ship that keeps its momentum,
/// and a key gives them all of it at once — the same action either way.
#[derive(InputAction)]
#[action(path = "disasteroids.thrust", output = f32, intent = Analog1, category = "disasteroids.flight")]
pub struct Thrust;

/// Which way the ship is turning, negative for anticlockwise.
#[derive(InputAction)]
#[action(path = "disasteroids.turn", output = f32, intent = Analog1, category = "disasteroids.flight")]
pub struct Turn;

#[derive(InputAction)]
#[action(path = "disasteroids.fire", output = bool, intent = Button, category = "disasteroids.weapons")]
pub struct Fire;

/// Jump somewhere else on the field, at some risk.
///
/// Double-tapped rather than pressed, so that panicking on the fire button cannot fling the ship
/// across the screen by accident.
#[derive(InputAction)]
#[action(path = "disasteroids.hyperspace", output = bool, intent = Button, category = "disasteroids.flight")]
pub struct Hyperspace;

/// A harder burn, available once the engine has been running a while.
///
/// Bound to the same controls as [`Thrust`], with a hold condition. One control, two actions, and
/// the difference between them is entirely in when they are considered to have fired.
#[derive(InputAction)]
#[action(path = "disasteroids.afterburner", output = bool, intent = Button, category = "disasteroids.flight")]
pub struct Afterburner;

/// Open or close the pause menu.
#[derive(InputAction)]
#[action(path = "disasteroids.pause", output = bool, intent = Button, category = "disasteroids.system")]
pub struct Pause;

/// Show or hide the debug overlay.
#[derive(InputAction)]
#[action(path = "disasteroids.toggle_overlay", output = bool, intent = Button, category = "disasteroids.system")]
pub struct ToggleOverlay;

/// Open or close the controls screen.
#[derive(InputAction)]
#[action(path = "disasteroids.toggle_settings", output = bool, intent = Button, category = "disasteroids.system")]
pub struct ToggleSettings;

/// Which way the selection moves on a screen.
///
/// A direction rather than four buttons, so that a stick and a D-pad reach it through the same
/// action. What makes it a *menu* control rather than a movement one is the pair of things on its
/// bindings: the stick is rounded to a compass point, and the binding only fires when that point
/// changes.
#[derive(InputAction)]
#[action(path = "disasteroids.navigate", output = Vec2, intent = Directional2, category = "disasteroids.menu")]
pub struct Navigate;

/// Leaves the screen, discarding anything not yet confirmed.
///
/// While a row is listening for a new control, this cancels only that capture instead — press it
/// again with nothing listening to leave.
#[derive(InputAction)]
#[action(path = "disasteroids.back", output = bool, intent = Button, category = "disasteroids.menu")]
pub struct Back;

/// Applies every change made on the screen, and leaves it.
#[derive(InputAction)]
#[action(path = "disasteroids.confirm", output = bool, intent = Button, category = "disasteroids.menu")]
pub struct Confirm;

/// The context a living ship flies under.
///
/// Fixed tick, because the ship integrates its own velocity and a frame-rate-dependent burn would
/// make the game play differently on different machines.
#[derive(InputContext)]
#[context(path = "disasteroids.flying", tick = Fixed)]
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
#[derive(InputContext)]
#[context(path = "disasteroids.shell", tick = Render)]
pub struct Shell;

/// The controls a screen with a selection on it answers.
///
/// Declared at a higher priority than either of the others and `exclusive`, which is the whole of
/// how a screen takes the controls away from the game underneath it: `Flying` and `Shell` are
/// treated as inactive for as long as this context is, so the arrow keys move the selection instead
/// of turning the ship and nothing bound below has to be named here to stop answering. The moment
/// the screen is gone, both contexts pick up wherever their own conditions already had them.
///
/// It is also the context with no state condition of any kind. It does not need one: the screen's
/// root node *is* the entity carrying this context, so the context exists for exactly as long as
/// the screen does. Spawning is the activation.
///
/// Render tick, for [`Shell`]'s reason — a menu answers at the frame rate, and a menu over a paused
/// game has no simulation rate to answer at.
#[derive(InputContext)]
#[context(path = "disasteroids.menu", tick = Render, priority = 10, exclusive)]
pub struct Menu;

/// How long a held direction waits before it starts repeating, and how long between repeats after.
///
/// One number for both, because the change fires on the crossing and the pulse's clock starts on
/// that same tick — so the first repeat lands one interval later, and every one after is evenly
/// spaced behind it.
const MENU_REPEAT: f32 = 0.25;

/// How far the stick must travel before it counts as pointing anywhere.
///
/// Much larger than the one on [`Turn`], and for the opposite reason. A flight deadzone is as small
/// as the hardware allows, so that fine control survives; a menu deadzone is as large as the player
/// will tolerate, so that reaching for one direction cannot brush past its neighbour on the way.
const MENU_DEAD_ZONE: f32 = 0.6;

/// The key [`Turn`]'s adjustable deadzone is declared under. Named once so the declaration and the
/// settings screen agree without repeating the string.
pub const TURN_DEAD_ZONE_KEY: &str = "disasteroids.turn.stick_deadzone";

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
        // The keyboard bindings are the ones a player may change. The pad binding above is listed
        // and fixed — the screen shows what the trigger does and offers no button to change it,
        // because console and Steam remapping already own the pad and offering both is two answers
        // to one question.
        //
        // Two of them, both plainly `mappable`: they derive one mapping name, so this is *one*
        // row holding a primary and a secondary rather than two rows the player has to be told are
        // the same thing. Its capacity grows to two because it holds two, with nobody saying so.
        controls.bind::<Thrust>(KeyCode::KeyW).mappable();
        controls.bind::<Thrust>(KeyCode::ArrowUp).mappable();
        // Declared once for the action, after every binding above: it reaches both keys and shares
        // one latch between them, and skips the trigger on its own — `Thrust` is analog, so a
        // `GamepadButton` here reads as a continuous fraction rather than a plain press, and
        // toggling that would flatten it. The trigger already has the better answer to the fatigue
        // a toggle exists for: it can rest at partial travel instead of being held down fully.
        controls.hold_or_toggle::<Thrust>("disasteroids.thrust.hold_or_toggle");

        // A stick axis is already signed; two keys need a composite to become one. The deadzone is
        // what the mechanic wants, and the player may move it: a smaller one turns more readily, a
        // larger one asks for a firmer push. The range reaches zero, which is only a sensible thing
        // to offer once something is removing the hardware's own drift underneath — this game has
        // no calibration step yet, so a player who takes it to zero gets their stick's drift and
        // that is the honest answer rather than a floor pretending to be the mechanic's choice.
        controls
            .bind::<Turn>(GamepadAxis::LeftStickX)
            .dead_zone(DeadZone::radial(0.15))
            .tunable_dead_zone(TURN_DEAD_ZONE_KEY, 0.0..=0.5);
        // Two keys make one axis, so the player sees two rows rather than one — "turn negative"
        // and "turn positive" — which is the same reason a movement composite is four. Each of
        // those two rows then holds two controls, because both composites are mappable and the
        // parts derive the same names: A and Left share a row, D and Right share the other.
        controls.bind::<Turn>(AxisButtons::ad()).mappable();
        controls.bind::<Turn>(AxisButtons::left_right()).mappable();

        // `pulse` fires the action again every interval for as long as the button is down, so
        // holding fire is a stream of separate `Fired`s rather than one long one — which is what
        // lets `shoot` be an observer with no timer of its own. The interval is the ship's rate of
        // fire, which is the one game number the input layer has to know.
        controls.bind::<Fire>(GamepadButton::South).pulse(RELOAD);
        // Two keyboard-and-mouse defaults, so Fire is one row with both slots filled. A mouse
        // button is the same kind of thing as a key here — one scheme, one channel — which is why
        // it needs no special handling to sit in the second slot.
        controls
            .bind::<Fire>(KeyCode::Space)
            .pulse(RELOAD)
            .mappable();
        controls
            .bind::<Fire>(MouseButton::Left)
            .pulse(RELOAD)
            .mappable();

        // Hold the throttle for three quarters of a second and it opens up. `Started` fires the
        // moment the burn begins, so the exhaust can show it building before it arrives.
        //
        // One binding of Afterburner per binding Thrust has above — pad, W and the up arrow —
        // generated from Thrust's own declarations rather than retyped: no second row under a
        // second name, and rebinding Thrust to some other key takes the afterburner with it. It
        // works even though the pad row is fixed and has nothing to rewrite. Declared last, after
        // every one of Thrust's bindings, because `follow` only sees what its leader has declared
        // so far — this is what makes it cover all three rather than whichever came first.
        controls.follow::<Afterburner, Thrust>(|binding| binding.hold(0.75));

        controls
            .bind::<Hyperspace>(GamepadButton::East)
            .multi_tap(2, 0.3);
        // Room for two, one shipped: the second slot is one a settings screen draws blank and the
        // player fills, rather than one the game had to have a default for.
        controls
            .bind::<Hyperspace>(KeyCode::ShiftLeft)
            .multi_tap(2, 0.3)
            .mappable_upto(2);
    });

    // No condition, so this one is live from the moment its entity exists and stays that way. Pause
    // is one action bound once, and the state it toggles is what the flying context follows.
    app.add_context::<Shell>(|controls| {
        controls.bind::<Pause>(KeyCode::Escape);
        controls.bind::<Pause>(GamepadButton::Start);

        controls.bind::<ToggleOverlay>(KeyCode::F1);
        controls.bind::<ToggleOverlay>(GamepadButton::Select);

        // Listed and fixed, like the two above: the screen that shows what the controls are is not
        // itself something the player rebinds from inside it.
        controls.bind::<ToggleSettings>(KeyCode::F2);
        controls.bind::<ToggleSettings>(GamepadButton::North);
    });

    // No binding here needs `.consume()`: `Menu` is `exclusive` (see its own doc comment), so
    // `Flying` and `Shell` cannot answer anything while this context is up regardless of which
    // controls it names. What is left to bind is only what the screen itself does with a control,
    // not what it needs to keep from the game underneath.
    app.add_context::<Menu>(|controls| {
        // The pair chunk 29 exists for. A stick reports a position, and a position is off centre
        // every tick it is held — so a binding on it fires every tick. Rounding to four points
        // turns that position into one of four answers, and `on_change` narrows the firing to the
        // ticks on which the answer moved. Four rather than eight because this drives a table,
        // where a diagonal is a way of asking for one of its neighbours rather than a direction of
        // its own.
        controls
            .bind::<Navigate>(Stick::Left)
            .dead_zone(DeadZone::radial(MENU_DEAD_ZONE))
            .compass(CompassPoints::Four)
            .on_change()
            .pulse(MENU_REPEAT);
        // A D-pad is already quantised, so it needs no compass — but it needs the same `on_change`,
        // because a held button is held every tick exactly as a stick is. The two bindings behave
        // identically from the selection's point of view, which is the test of whether the rounding
        // was the right shape.
        controls
            .bind::<Navigate>(DirectionalButtons::dpad())
            .on_change()
            .pulse(MENU_REPEAT);
        controls
            .bind::<Navigate>(DirectionalButtons::arrow_keys())
            .on_change()
            .pulse(MENU_REPEAT);

        controls.bind::<Back>(GamepadButton::East).press();
        controls.bind::<Back>(KeyCode::Escape).press();

        controls.bind::<Confirm>(GamepadButton::West).press();

        // Duplicated from `Shell` rather than reached through it, because `Shell` is exactly what
        // this context shadows while it is up — the inconsistency with `Pause`, which stays
        // unreachable until the screen closes, is deliberate: closing the screen this way is common
        // enough to want a direct route, and pausing from inside it is not.
        controls.bind::<ToggleSettings>(KeyCode::F2);
        controls.bind::<ToggleSettings>(GamepadButton::North);
    });

    app.add_systems(Startup, shell.spawn());
}

/// The always-on controls, as a scene: the context, and what listens to it.
///
/// This is the arrangement worth copying. A context is an ordinary component, and a transition is
/// an [`EntityEvent`] aimed at the entity carrying that component — so the entity, the context on
/// it, and the observers for the context's actions all belong in one `bsn!` block. Everything about
/// who hears escape is these four lines. Nothing is registered against the app, and there is no
/// second place that has to be kept in step with this one.
///
/// A context that is not attached to a player or a world object is still attached to an entity,
/// because that is what an observer targets. Which entity is then a design choice rather than a
/// formality: this one is spawned at startup and lives forever, and [`Menu`]'s is the settings
/// screen's own root node — so that context comes and goes with the screen, and nothing has to
/// remember to switch it off.
fn shell() -> impl Scene {
    bsn! {
        Shell
        on(pause::toggle)
        on(crate::overlay::toggle)
        on(crate::settings::toggle)
    }
}
