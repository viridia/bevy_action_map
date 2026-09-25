//! Every action, context and binding in the game, with Steam owning the pad.
//!
//! The actions and contexts are the base game's, and so is every keyboard and mouse binding. What
//! differs is the pad: where the base binds a button or a stick, this binds an [`Authority`] for
//! the gamepad family, and the player chooses the control in Steam's own layout instead. The
//! conditions stay, because a rate of fire or a bomb's charge time is the game's rule however the
//! button was pressed.

use bevy::prelude::*;
use bevy_action_map::prelude::*;
use bevy_input::{keyboard::KeyCode, mouse::MouseButton};

use crate::common::widget_focus::{
    ADJUST_REPEAT, Activate, Adjust, ButtonFocused, StepperFocused, WidgetKind, focus_is,
};
use crate::pause::{self, Game};
use crate::ship::{BOMB_CHARGE, RELOAD};
use crate::steam::{self, SteamActions};

#[derive(InputAction)]
#[action(path = "disasteroids.thrust", output = f32, intent = Analog1, category = "disasteroids.flight")]
pub struct Thrust;

#[derive(InputAction)]
#[action(path = "disasteroids.turn", output = f32, intent = Analog1, category = "disasteroids.flight")]
pub struct Turn;

#[derive(InputAction)]
#[action(path = "disasteroids.fire", output = bool, intent = Button, category = "disasteroids.weapons")]
pub struct Fire;

#[derive(InputAction)]
#[action(path = "disasteroids.smart_bomb", output = bool, intent = Button, category = "disasteroids.weapons")]
pub struct SmartBomb;

#[derive(InputAction)]
#[action(path = "disasteroids.hyperspace", output = bool, intent = Button, category = "disasteroids.flight")]
pub struct Hyperspace;

#[derive(InputAction)]
#[action(path = "disasteroids.afterburner", output = bool, intent = Button, category = "disasteroids.flight")]
pub struct Afterburner;

#[derive(InputAction)]
#[action(path = "disasteroids.pause", output = bool, intent = Button, category = "disasteroids.system")]
pub struct Pause;

#[derive(InputAction)]
#[action(path = "disasteroids.toggle_overlay", output = bool, intent = Button, category = "disasteroids.system")]
pub struct ToggleOverlay;

#[derive(InputAction)]
#[action(path = "disasteroids.toggle_settings", output = bool, intent = Button, category = "disasteroids.system")]
pub struct ToggleSettings;

#[derive(InputAction)]
#[action(path = "disasteroids.new_game", output = bool, intent = Button, category = "disasteroids.system")]
pub struct NewGame;

#[derive(InputAction)]
#[action(path = "disasteroids.navigate", output = Vec2, intent = Directional2, category = "disasteroids.menu")]
pub struct Navigate;

#[derive(InputAction)]
#[action(path = "disasteroids.back", output = bool, intent = Button, category = "disasteroids.menu")]
pub struct Back;

#[derive(InputAction)]
#[action(path = "disasteroids.confirm", output = bool, intent = Button, category = "disasteroids.menu")]
pub struct Confirm;

#[derive(InputAction)]
#[action(path = "disasteroids.clear", output = bool, intent = Button, category = "disasteroids.menu")]
pub struct Clear;

#[derive(InputContext)]
#[context(path = "disasteroids.flying", tick = Fixed)]
pub struct Flying;

#[derive(InputContext)]
#[context(path = "disasteroids.shell", tick = Render)]
pub struct Shell;

#[derive(InputContext)]
#[context(path = "disasteroids.menu", tick = Render, priority = 10, exclusive)]
pub struct Menu;

const MENU_REPEAT: f32 = 0.25;

/// A selection threshold rather than drift correction, which is why it survives onto an authority
/// binding: Steam's own dead zone keeps a resting stick at zero, and this keeps a light push from
/// counting as a choice.
const MENU_DEAD_ZONE: f32 = 0.6;

/// Declared by the base game on the stick's dead zone. Steam shapes the stick itself, so here it
/// names nothing and the settings screen's stepper has no range to offer.
pub const TURN_DEAD_ZONE_KEY: &str = "disasteroids.turn.stick_deadzone";

/// The two action sets in `game_actions_480.vdf`. `Flying` and `Shell` are live together and so
/// share a set; `Menu` is exclusive, so it can have its own (`docs/steam.md`'s appendix).
const GAMEPLAY: &str = "disasteroids.gameplay";
const MENU: &str = "disasteroids.menu";

pub fn plugin(app: &mut App) {
    const PAD: Authority = Authority(DeviceFamily::Gamepad);

    app.add_context::<Flying>(|controls| {
        controls.active_in_state(Game::Playing);

        controls.bind::<Thrust>(PAD);
        controls.bind::<Thrust>(KeyCode::KeyW).mappable();
        controls.bind::<Thrust>(KeyCode::ArrowUp).mappable();
        // Reaches the two keys and skips the authority, which is analog and has nothing to toggle.
        controls.hold_or_toggle::<Thrust>("disasteroids.thrust.hold_or_toggle");

        controls.bind::<Turn>(PAD);
        controls.bind::<Turn>(AxisButtons::ad()).mappable();
        controls.bind::<Turn>(AxisButtons::left_right()).mappable();

        controls.bind::<Fire>(PAD).pulse(RELOAD);
        controls
            .bind::<Fire>(KeyCode::Space)
            .pulse(RELOAD)
            .mappable();
        controls
            .bind::<Fire>(MouseButton::Left)
            .pulse(RELOAD)
            .mappable();

        // The base game asks for both bumpers here. Under Steam the chord, if any, is the player's
        // to set in the layout, and the charge time stays the game's.
        controls.bind::<SmartBomb>(PAD).hold_once(BOMB_CHARGE);
        controls
            .bind::<SmartBomb>(KeyCode::KeyB)
            .hold_once(BOMB_CHARGE)
            .mappable();

        // Rides Thrust's authority as it rides the keys: Steam supplies Thrust once, and the
        // afterburner reads it.
        controls.follow::<Afterburner, Thrust>(|binding| binding.hold(0.75));

        controls.bind::<Hyperspace>(PAD).multi_tap(2, 0.3);
        controls
            .bind::<Hyperspace>(KeyCode::ShiftLeft)
            .multi_tap(2, 0.3)
            .mappable();
    });

    app.add_context::<Shell>(|controls| {
        controls.bind::<Pause>(KeyCode::Escape).reserved();
        controls.bind::<Pause>(PAD);

        controls.bind::<ToggleOverlay>(KeyCode::F1);
        controls.bind::<ToggleOverlay>(PAD);

        controls.bind::<ToggleSettings>(KeyCode::F2).reserved();
        controls.bind::<ToggleSettings>(PAD);

        controls
            .bind::<NewGame>(KeyCode::KeyN)
            .with(ModifierKey::Ctrl);
    });

    app.add_context::<Menu>(|controls| {
        controls
            .bind::<Navigate>(PAD)
            .dead_zone(DeadZone::radial(MENU_DEAD_ZONE))
            .compass(CompassPoints::Four);
        controls.bind::<Navigate>(DirectionalButtons::arrow_keys());
        controls
            .combined::<Navigate>()
            .on_change()
            .pulse(MENU_REPEAT);

        controls.bind::<Back>(PAD).press();
        controls.bind::<Back>(KeyCode::Escape).press();

        controls.bind::<Confirm>(PAD).press();

        controls.bind::<Clear>(KeyCode::Delete).press();
        controls.bind::<Clear>(KeyCode::Backspace).press();
        controls.bind::<Clear>(PAD).press();

        controls.bind::<ToggleSettings>(KeyCode::F2);
        controls.bind::<ToggleSettings>(PAD);
    });

    // `widget_focus::plugin`'s two contexts, with the pad's button an authority. The pad has no
    // stepper to adjust here: the dead-zone stepper is not on this build's screen.
    app.add_context::<ButtonFocused>(|controls| {
        controls.active_if(focus_is(WidgetKind::BUTTON));
        controls.bind::<Activate>(KeyCode::Enter).press();
        controls.bind::<Activate>(KeyCode::Space).press();
        controls.bind::<Activate>(PAD).press();
    });
    app.add_context::<StepperFocused>(|controls| {
        controls.active_if(focus_is(WidgetKind::STEPPER));
        controls
            .bind::<Adjust>(AxisButtons::new(KeyCode::Minus, KeyCode::Equal))
            .pulse(ADJUST_REPEAT)
            .consume()
            .private();
    });

    // Every context entity is where the backend writes, and the linked modules that spawn them know
    // nothing of Steam.
    app.register_required_components::<Flying, AuthorityValues>();
    app.register_required_components::<Shell, AuthorityValues>();
    app.register_required_components::<Menu, AuthorityValues>();
    app.register_required_components::<ButtonFocused, AuthorityValues>();

    // Afterburner is absent: it follows Thrust, so Steam has nothing to supply for it.
    app.insert_resource(SteamActions::new(
        GAMEPLAY,
        vec![
            (
                GAMEPLAY,
                vec![
                    steam::axis::<Thrust>(),
                    steam::axis::<Turn>(),
                    steam::button::<Fire>(),
                    steam::button::<SmartBomb>(),
                    steam::button::<Hyperspace>(),
                    steam::button::<Pause>(),
                    steam::button::<ToggleOverlay>(),
                    steam::button::<ToggleSettings>(),
                ],
            ),
            (
                MENU,
                vec![
                    steam::stick::<Navigate>(),
                    steam::button::<Confirm>(),
                    steam::button::<Back>(),
                    steam::button::<Clear>(),
                    steam::button_as::<ToggleSettings>("disasteroids.menu.toggle_settings"),
                    steam::button::<Activate>(),
                ],
            ),
        ],
    ));
    app.add_systems(Update, choose_set);

    app.add_systems(Startup, shell.spawn());
}

/// The menu set while a screen is up, the gameplay set otherwise.
///
/// Read a frame late, which costs nothing: the screen's own context is exclusive from the moment it
/// spawns, whatever Steam is still reporting.
fn choose_set(menus: Query<(), With<Menu>>, mut table: ResMut<SteamActions>) {
    let set = if menus.is_empty() { GAMEPLAY } else { MENU };
    if table.set != set {
        table.set = set;
    }
}

fn shell() -> impl Scene {
    bsn! {
        Shell
        on(pause::toggle)
        on(crate::overlay::toggle)
        on(crate::settings::toggle)
        on(crate::asteroids::new_game)
    }
}
