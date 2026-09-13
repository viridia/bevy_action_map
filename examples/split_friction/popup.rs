//! The per-pane pause popup: presets, disconnecting, and getting back to the game.
//!
//! Either protagonist's own device opens it — [`OpenMenu`], bound in [`OnFoot`](crate::protagonist)
//! — which pauses both panes: `PopupMenu` is `exclusive`, so `OnFoot` is shadowed and neither
//! protagonist answers the stick or arrow keys while it is up. Only one popup exists at a time,
//! and that is not a scope choice: `common::widget_focus` reads a single global `InputFocus`, so a
//! second pane's buttons would have nothing to tell them apart from the first's. Bevy has no
//! per-player focus.
//!
//! The popup is spawned fresh each time it opens and despawned when it closes — [`split_screen`]
//! owns that half, giving it a root already sized and camera-targeted to the right pane the same
//! way the join prompt is. This module owns the popup's own content and behaviour.
//!
//! [`split_screen`]: crate::split_screen

use bevy::input_focus::AutoFocus;
use bevy::math::CompassOctant;
use bevy::prelude::*;
use bevy::ui::auto_directional_navigation::AutoDirectionalNavigator;
use bevy::ui_widgets::{Activate, Button};
use bevy_action_map::device::DeviceFamily;
use bevy_action_map::overrides::{Overrides, apply_overrides_for_with_preset};
use bevy_action_map::player::Paired;
use bevy_action_map::prelude::*;
use bevy_action_map::preset::Preset;
use bevy_input::{gamepad::GamepadButton, keyboard::KeyCode};

use crate::common::widget_focus::focusable;
use crate::protagonist::{ClaimedDevices, Move, OnFoot, Protagonist};
use crate::saved_pairings;

/// Whether a per-pane popup is open, and which pane's.
#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Popup {
    #[default]
    Closed,
    Open(u8),
}

/// Opens the popup for whichever protagonist's device pressed it.
///
/// No corresponding close binding here: once the popup is up, `PopupMenu`'s exclusivity shadows
/// `OnFoot` entirely, so a second press of this never reaches [`open_menu`]. `Back` and Return to
/// Game are what closes it.
#[derive(InputAction)]
#[action(path = "split_friction.open_menu", output = bool, intent = Button)]
pub struct OpenMenu;

/// Which way the selection moves inside the popup.
#[derive(InputAction)]
#[action(path = "split_friction.menu.navigate", output = Vec2, intent = Directional2)]
struct Navigate;

/// Leaves the popup without picking anything.
#[derive(InputAction)]
#[action(path = "split_friction.menu.back", output = bool, intent = Button)]
struct Back;

const MENU_DEAD_ZONE: f32 = 0.6;
const MENU_REPEAT: f32 = 0.25;

/// The popup's own controls, live only while it exists.
///
/// Exclusive, so `Move` cannot answer the stick or arrow keys while the popup is up. Priority 10,
/// matching Disasteroids' `Menu`: `common::widget_focus`'s `ButtonFocused` (priority 20) still
/// outranks it, so a focused button is pressed rather than swallowed.
///
/// No state condition, on the same terms as `Menu`: the popup's own root is the entity carrying
/// this context (see [`menu`]), so it exists for exactly as long as the popup does.
#[derive(InputContext)]
#[context(path = "split_friction.popup_menu", tick = Render, priority = 10, exclusive)]
struct PopupMenu;

/// Classic — the compiled-in default, nothing swapped.
pub const CLASSIC: &str = "split_friction.classic";
/// Southpaw — `Move`'s gamepad row moves to the right stick.
const SOUTHPAW: &str = "split_friction.southpaw";

/// Which preset a protagonist currently has applied.
///
/// Inserted by [`apply_preset`], which `protagonist::claim_slot` calls as it puts a pane into play,
/// so there is always an answer — even for a protagonist that has never opened the popup.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub struct ActivePreset(pub &'static str);

impl Default for ActivePreset {
    fn default() -> Self {
        ActivePreset(CLASSIC)
    }
}

fn preset_label(name: &str) -> &'static str {
    if name == SOUTHPAW {
        "Southpaw"
    } else {
        "Classic"
    }
}

/// Every preset Split Friction offers.
///
/// Built fresh against the world rather than declared once — the same reason Disasteroids' own
/// `presets` is: a `MappingKey` cannot be built outside the crate, so `Preset::build` is what asks
/// the world what `Move`'s gamepad row actually is. A no-op for a keyboard-paired protagonist:
/// `Move` has no keyboard row for either preset to touch, so cycling changes nothing to look at,
/// which is what "device-specific" comes down to rather than a hidden button.
fn presets(world: &World) -> Vec<Preset> {
    vec![
        Preset {
            name: CLASSIC,
            rows: Overrides::new(),
        },
        Preset::build(world, SOUTHPAW, |southpaw| {
            southpaw.bind::<Move>(DeviceFamily::Gamepad, [Control::GamepadStick(Stick::Right)]);
        }),
    ]
}

pub fn plugin(app: &mut App) {
    app.init_state::<Popup>();
    app.add_observer(open_menu);
    app.add_context::<PopupMenu>(|controls| {
        controls
            .bind::<Navigate>(Stick::Left)
            .dead_zone(DeadZone::radial(MENU_DEAD_ZONE))
            .compass(CompassPoints::Four)
            .on_change()
            .pulse(MENU_REPEAT);
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
    });
    app.add_systems(Update, redraw_preset_label);
}

fn open_menu(
    fired: On<Fired<OpenMenu>>,
    protagonists: Query<&Protagonist>,
    mut next: ResMut<NextState<Popup>>,
) {
    if let Ok(&Protagonist(index)) = protagonists.get(fired.entity) {
        next.set(Popup::Open(index));
    }
}

/// Moves the selection. [`AutoDirectionalNavigator`] rather than the manual one: the popup
/// declares no links, so every answer comes from where the buttons are on screen.
fn navigate(fired: On<Fired<Navigate>>, mut nav: AutoDirectionalNavigator) {
    let Ok(direction) = Dir2::new(fired.value) else {
        return;
    };
    let _ = nav.navigate(CompassOctant::from(direction));
}

fn back(_: On<Fired<Back>>, mut next: ResMut<NextState<Popup>>) {
    next.set(Popup::Closed);
}

/// Names the button's own protagonist, so a press can find its way back to the right pane.
#[derive(Component, Clone, Copy)]
struct PopupTarget(Entity);

/// The Select Preset button, so [`redraw_preset_label`] knows which one to rewrite.
#[derive(Component, Clone, Copy, Default)]
struct SelectPresetButton;

const TITLE: Color = Color::srgb(0.9, 0.95, 1.0);
const FIXED: Color = Color::srgb(0.55, 0.6, 0.62);

/// The popup itself, built fresh against `entity`'s own current preset.
pub(crate) fn menu(entity: Entity, current: ActivePreset) -> impl Scene {
    bsn! {
        PopupMenu
        on(navigate)
        on(back)
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            row_gap: Val::Px(6.0),
            padding: {UiRect::all(Val::Px(16.0))},
            border_radius: {BorderRadius::all(Val::Px(6.0))},
        }
        BackgroundColor({Color::BLACK.with_alpha(0.85)})
        Children [
            @{select_preset_button(entity, current)}
            --
            @{reset_presets_button(entity)}
            --
            @{disconnect_button(entity)}
            --
            @{return_button()}
        ]
    }
}

fn select_preset_button(entity: Entity, current: ActivePreset) -> impl Scene {
    bsn! {
        Button
        on(select_preset_pressed)
        AutoFocus
        @focusable()
        ~{PopupTarget(entity)}
        SelectPresetButton
        Text::new(format!("Select Preset: {}", preset_label(current.0)))
        TextFont { font_size: 14.0_f32 }
        TextColor(TITLE)
        BorderColor::all(FIXED)
        Node {
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(14.0), Val::Px(4.0))},
        }
    }
}

fn reset_presets_button(entity: Entity) -> impl Scene {
    bsn! {
        Button
        on(reset_presets_pressed)
        @focusable()
        ~{PopupTarget(entity)}
        Text::new("Reset Presets")
        TextFont { font_size: 14.0_f32 }
        TextColor(TITLE)
        BorderColor::all(FIXED)
        Node {
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(14.0), Val::Px(4.0))},
        }
    }
}

fn disconnect_button(entity: Entity) -> impl Scene {
    bsn! {
        Button
        on(disconnect_pressed)
        @focusable()
        ~{PopupTarget(entity)}
        Text::new("Disconnect")
        TextFont { font_size: 14.0_f32 }
        TextColor(TITLE)
        BorderColor::all(FIXED)
        Node {
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(14.0), Val::Px(4.0))},
        }
    }
}

fn return_button() -> impl Scene {
    bsn! {
        Button
        on(return_pressed)
        @focusable()
        Text::new("Return to Game")
        TextFont { font_size: 14.0_f32 }
        TextColor(TITLE)
        BorderColor::all(FIXED)
        Node {
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(14.0), Val::Px(4.0))},
        }
    }
}

/// Puts one pane on `name`, both in what it is bound to and in what it says it is on.
///
/// The one place a pane's preset changes, so a press, a reset and a pairing restored from settings
/// all reach the bindings the same way. A name no preset answers to falls back to [`CLASSIC`],
/// which is what a save file written before a preset was renamed or dropped will hold.
pub(crate) fn apply_preset(world: &mut World, pane: Entity, name: &str) {
    let chosen = presets(world)
        .into_iter()
        .find(|preset| preset.name == name)
        .unwrap_or(Preset {
            name: CLASSIC,
            rows: Overrides::new(),
        });
    // Per entity, so the other pane keeps whatever it was on and a pane joining later still gets
    // the game's own declaration rather than this one's choice.
    apply_overrides_for_with_preset(world, pane, &chosen.rows, &chosen.rows);
    world.entity_mut(pane).insert(ActivePreset(chosen.name));
    saved_pairings::store_preset(world, pane, chosen.name);
}

/// Cycles to the next preset in [`presets`], wrapping past the end back to the first.
fn select_preset_pressed(
    activate: On<Activate>,
    targets: Query<&PopupTarget>,
    mut commands: Commands,
) {
    let Ok(&PopupTarget(entity)) = targets.get(activate.entity) else {
        return;
    };
    commands.queue(move |world: &mut World| {
        let names: Vec<&'static str> = presets(world).iter().map(|preset| preset.name).collect();
        let current = world
            .get::<ActivePreset>(entity)
            .copied()
            .unwrap_or_default();
        let mut cycle = names.iter().copied().cycle();
        cycle.find(|&name| name == current.0);
        let Some(next) = cycle.next() else {
            return;
        };
        apply_preset(world, entity, next);
    });
}

fn reset_presets_pressed(
    activate: On<Activate>,
    targets: Query<&PopupTarget>,
    mut commands: Commands,
) {
    let Ok(&PopupTarget(entity)) = targets.get(activate.entity) else {
        return;
    };
    commands.queue(move |world: &mut World| apply_preset(world, entity, CLASSIC));
}

/// Drops the pane's pairing and frees its device — the clean way to reset it between test runs,
/// without touching a settings file or restarting.
///
/// Also drops `AwaitingReconnect`: a player choosing to leave while their old pad is still missing
/// must not leave that marker standing, or a pad connecting later would silently resurrect a slot
/// they meant to abandon.
fn disconnect_pressed(
    activate: On<Activate>,
    targets: Query<&PopupTarget>,
    paired: Query<&Paired>,
    mut claimed: ResMut<ClaimedDevices>,
    mut commands: Commands,
    mut next: ResMut<NextState<Popup>>,
) {
    let Ok(&PopupTarget(entity)) = targets.get(activate.entity) else {
        return;
    };
    if let Ok(pairing) = paired.get(entity) {
        for device in pairing.iter() {
            claimed.release(device);
        }
    }
    // `KnownDevice` goes too, which is what separates leaving on purpose from a pad falling out:
    // a pane that dropped its pad deliberately is not offered it again, this session or the next.
    commands.entity(entity).remove::<(
        OnFoot,
        Paired,
        ActivePreset,
        crate::reconnect::AwaitingReconnect,
        crate::reconnect::KnownDevice,
    )>();
    next.set(Popup::Closed);
}

fn return_pressed(_: On<Activate>, mut next: ResMut<NextState<Popup>>) {
    next.set(Popup::Closed);
}

/// Keeps the Select Preset button's own label true to the preset actually applied — the same
/// change-detection-driven redraw `sync_join_ui` uses, rather than a press handler reaching into
/// the view directly.
fn redraw_preset_label(
    changed: Query<(Entity, &ActivePreset), Changed<ActivePreset>>,
    mut buttons: Query<(&PopupTarget, &mut Text), With<SelectPresetButton>>,
) {
    for (entity, active) in &changed {
        for (&PopupTarget(target), mut text) in &mut buttons {
            if target == entity {
                *text = Text::new(format!("Select Preset: {}", preset_label(active.0)));
            }
        }
    }
}
