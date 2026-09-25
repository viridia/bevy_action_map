//! Base Disasteroids' way to remap the pad: a choice of presets, `Turn`'s dead zone and whether
//! `Thrust` holds or toggles, laid out under the pad's table on the controls screen.
//!
//! Like everything else on that screen, these only write into the working copy, and Confirm is what
//! applies it. See [`settings`](crate::settings).

use bevy::prelude::*;
use bevy::scene::{Ready, SceneList};
use bevy::ui::UiSystems;
use bevy::ui_widgets::{Activate, Button};
use bevy_action_map::mapping::{Tunable, TunableValue, fallback_label, tunables};
use bevy_action_map::overrides::Overrides;

use crate::actions::TURN_DEAD_ZONE_KEY;
use crate::common::widget_focus::{
    Adjusted, Stepper, decrement_pressed, focusable, increment_pressed,
};
use crate::settings::{
    CHANGEABLE, FIXED, HEADING, PendingOverrides, TITLE, append_children, pending_copy, presets,
};

/// The preset currently in effect, drawn distinct from the rest of the row — its own color, since
/// "selected" and "listening" are not the same fact about a control.
const SELECTED: Color = Color::srgb(0.25, 0.55, 0.35);

/// How far one press of the stepper moves `Turn`'s deadzone. The bounds are the tunable's own,
/// declared in `actions.rs` and read back off it, so the two cannot drift apart.
const DEAD_ZONE_STEP: f32 = 0.05;

/// The key `actions.rs` declared `Thrust`'s toggle tunable under. Named once so the row builder,
/// the press handler and the redraw below all agree with the declaration without repeating the
/// string in four places.
const HOLD_OR_TOGGLE_KEY: &str = "disasteroids.thrust.hold_or_toggle";

pub fn plugin(app: &mut App) {
    // Ahead of every UI system, for the reason `settings`' own redraw is.
    app.add_systems(
        PostUpdate,
        redraw
            .run_if(resource_changed::<PendingOverrides>)
            .before(UiSystems::Prepare),
    );
}

/// The presets, the stepper and the switch, for the slot under the pad's table.
///
/// They spawn blank and [`redraw`] paints them, which it does on the frame the screen opens, since
/// opening it seeds the working copy.
pub(crate) fn remapping() -> Box<dyn SceneList> {
    Box::new(bsn_list! {
        @{preset_row()}
        --
        // Two rows' worth of control on one line, for now: the screen is already at the height
        // budget the window allows, and neither reads worse side by side than stacked.
        Node { column_gap: Val::Px(24.0) }
        Children [
            @{dead_zone_row()}
            --
            @{hold_or_toggle_row()}
        ]
    })
}

/// The row of preset buttons, one for each preset once it has spawned.
fn preset_row() -> impl Scene {
    bsn! {
        on(fill_preset_row)
        Node { column_gap: Val::Px(10.0) }
    }
}

/// Built against the world, since that is what [`presets`] needs.
fn fill_preset_row(ready: On<Ready>, world: &World, mut commands: Commands) {
    let buttons: Vec<_> = presets(world)
        .into_iter()
        .map(|preset| preset_button(preset.name))
        .collect();
    append_children(&mut commands, ready.entity, buttons);
}

/// One preset's own button. [`redraw`] gives the one in effect [`SELECTED`] for its border and a
/// wash of the same color behind it — the swap a cell makes while it is listening.
fn preset_button(name: &'static str) -> impl Scene {
    bsn! {
        Button
        on(preset_pressed)
        @focusable()
        ~{PresetButton(name)}
        Text::new(fallback_label(name))
        TextFont { font_size: 14.0_f32 }
        TextColor(TITLE)
        BorderColor::all(FIXED)
        BackgroundColor(Color::NONE)
        Node {
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(12.0), Val::Px(3.0))},
        }
    }
}

/// A label and its stepper, the same "row names what it is, then draws the control" shape the two
/// tables already use.
fn dead_zone_row() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0) }
        Children [
            Text::new("Turn dead zone") TextFont { font_size: 13.0_f32 } TextColor(HEADING)
            --
            @{stepper()}
        ]
    }
}

/// One place the value's format is decided.
fn dead_zone_label(value: f32) -> String {
    format!("{value:.2}")
}

/// One chevron on either side of the value, `justify_content: SpaceBetween` so the row's own width
/// is what spaces them rather than a gap that would also grow the digits between them.
///
/// The chevrons are `Button`s but not `focusable()`: this row is the one tab stop, the same
/// distinction [`common::widget_focus`](crate::common::widget_focus) draws between a stepper and
/// the widgets inside it — a click still presses one (`bevy_ui_widgets` sees to that on its own),
/// it just never moves the selection.
fn stepper() -> impl Scene {
    bsn! {
        Stepper
        on(apply_dead_zone_delta)
        @focusable()
        Node {
            width: Val::Px(130.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(8.0), Val::Px(3.0))},
        }
        BorderColor::all(CHANGEABLE)
        Children [
            Button on(decrement_pressed) Text::new("<") TextFont { font_size: 15.0_f32 } TextColor(TITLE)
            --
            DeadZoneValue
            Text::new("")
            TextFont { font_size: 14.0_f32 }
            TextColor(TITLE)
            --
            Button on(increment_pressed) Text::new(">") TextFont { font_size: 15.0_f32 } TextColor(TITLE)
        ]
    }
}

/// Names the stepper's own value `Text`, so [`redraw`] can find it again.
#[derive(Component, Default, Clone, Copy)]
struct DeadZoneValue;

/// `Turn`'s deadzone as the working copy has it, with the bounds the declaration gave it.
///
/// Returns the declared bounds alongside the value rather than constants of this file's own: the
/// range lives on the tunable, so a stepper that clamped to its own numbers could stop somewhere
/// the binding would not accept.
fn dead_zone_tunable(world: &World, pending: &Overrides) -> TunableValue {
    tunables(world)
        .into_iter()
        .find(|tunable| tunable.key == TURN_DEAD_ZONE_KEY)
        .map_or(
            TunableValue::Range {
                value: 0.0,
                min: 0.0,
                max: 0.0,
            },
            |tunable| effective_tunable(&tunable, pending),
        )
}

/// Applies one step, clamped to the tunable's declared range. Into the working copy, so a deadzone
/// the player is still deciding about does not move the ship underneath them.
fn apply_dead_zone_delta(adjusted: On<Adjusted>, mut commands: Commands) {
    let delta = adjusted.delta;
    commands.queue(move |world: &mut World| {
        let Some(tunable) = tunables(world)
            .into_iter()
            .find(|tunable| tunable.key == TURN_DEAD_ZONE_KEY)
        else {
            return;
        };
        let TunableValue::Range { value, min, max } =
            effective_tunable(&tunable, &pending_copy(world))
        else {
            return;
        };
        world.resource_mut::<PendingOverrides>().0.captures.tune(
            tunable.family,
            tunable.key,
            TunableValue::Range {
                value: (value + delta * DEAD_ZONE_STEP).clamp(min, max),
                min,
                max,
            },
        );
    });
}

/// What a tunable reads as with the working copy laid over it — the tunable half of what
/// `effective` already does for a mapping row's controls.
fn effective_tunable(tunable: &Tunable, pending: &Overrides) -> TunableValue {
    pending
        .get_tunable(tunable.family, tunable.key)
        .unwrap_or(tunable.value)
}

/// A label and a checkbox-shaped button, the same "row names what it is, then draws the control"
/// shape [`dead_zone_row`] uses.
fn hold_or_toggle_row() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0) }
        Children [
            Text::new("Thrust") TextFont { font_size: 13.0_f32 } TextColor(HEADING)
            --
            Button
            on(hold_or_toggle_pressed)
            @focusable()
            BorderColor::all(CHANGEABLE)
            Node {
                width: Val::Px(130.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: {UiRect::all(Val::Px(1.0))},
                border_radius: {BorderRadius::all(Val::Px(4.0))},
                padding: {UiRect::axes(Val::Px(8.0), Val::Px(3.0))},
            }
            Children [
                HoldOrToggleValue
                Text::new("")
                TextFont { font_size: 14.0_f32 }
                TextColor(TITLE)
            ]
        ]
    }
}

fn hold_or_toggle_label(active: bool) -> &'static str {
    if active { "Toggle" } else { "Hold" }
}

/// Names the checkbox's own label `Text`, mirroring [`DeadZoneValue`].
#[derive(Component, Default, Clone, Copy)]
struct HoldOrToggleValue;

/// Flips the tunable in the working copy. Reads `tunables` fresh rather than trusting a captured
/// value, so two presses in the same visit agree with each other.
fn hold_or_toggle_pressed(_: On<Activate>, mut commands: Commands) {
    commands.queue(|world: &mut World| {
        let Some(tunable) = tunables(world)
            .into_iter()
            .find(|tunable| tunable.key == HOLD_OR_TOGGLE_KEY)
        else {
            return;
        };
        let TunableValue::Bool(active) = effective_tunable(&tunable, &pending_copy(world)) else {
            return;
        };
        world.resource_mut::<PendingOverrides>().0.captures.tune(
            tunable.family,
            tunable.key,
            TunableValue::Bool(!active),
        );
    });
}

/// Names the preset a button selects, so a press can find its rows again — mirrors how a rebinding
/// cell names a row by key rather than carrying the row's own data.
#[derive(Component, Clone, Copy)]
struct PresetButton(&'static str);

/// Names the pressed preset in the working copy, and does nothing else.
///
/// Recording the name is the whole of it, because
/// [`working_copy`](crate::settings::working_copy) resolves the rows fresh every time it is asked.
/// That is what makes picking a preset supersede the last rather than layer onto it, with no
/// register of which rows any preset touches: `Default` names no rows, so switching back to it
/// leaves nothing of Southpaw behind.
///
/// The captures the preset itself names are dropped, though — a preset press is a fresh statement
/// about the rows it covers, and a player who nudged the dead zone and then pressed Southpaw again
/// is asking for Southpaw's dead zone. Captures the preset does not name are left alone, which is
/// what keeps a keyboard rebind from being thrown away by a gamepad preset.
fn preset_pressed(activate: On<Activate>, buttons: Query<&PresetButton>, mut commands: Commands) {
    let Ok(&PresetButton(name)) = buttons.get(activate.entity) else {
        return;
    };
    commands.queue(move |world: &mut World| {
        let Some(preset) = presets(world)
            .into_iter()
            .find(|preset| preset.name == name)
        else {
            return;
        };
        let mut pending = world.resource_mut::<PendingOverrides>();
        for (family, key, _) in preset.rows.iter() {
            pending.0.captures.reset(family, key);
        }
        for (family, key, _) in preset.rows.iter_tunables() {
            pending.0.captures.reset_tunable(family, key);
        }
        pending.0.preset = preset.name;
    });
}

/// Paints the selected preset, the dead zone and the switch from the working copy.
///
/// Exclusive, for the reason [`settings`](crate::settings)' own redraw is: it reads the tunables and
/// the pending copy alongside the widgets it paints.
fn redraw(world: &mut World) {
    let live_tunables = tunables(world);
    let pending = pending_copy(world);
    let selected = world.resource::<PendingOverrides>().0.preset;

    let mut buttons = world.query::<(&PresetButton, &mut BorderColor, &mut BackgroundColor)>();
    for (button, mut border, mut background) in buttons.iter_mut(world) {
        let is_selected = button.0 == selected;
        *border = BorderColor::all(if is_selected { SELECTED } else { FIXED });
        *background = BackgroundColor(if is_selected {
            SELECTED.with_alpha(0.25)
        } else {
            Color::NONE
        });
    }

    if let Some(tunable) = live_tunables
        .iter()
        .find(|tunable| tunable.key == HOLD_OR_TOGGLE_KEY)
        && let TunableValue::Bool(active) = effective_tunable(tunable, &pending)
    {
        let mut checkbox = world.query_filtered::<&mut Text, With<HoldOrToggleValue>>();
        if let Ok(mut text) = checkbox.single_mut(world) {
            *text = Text::new(hold_or_toggle_label(active));
        }
    }

    if let TunableValue::Range { value, .. } = dead_zone_tunable(world, &pending) {
        let mut stepper = world.query_filtered::<&mut Text, With<DeadZoneValue>>();
        if let Ok(mut text) = stepper.single_mut(world) {
            *text = Text::new(dead_zone_label(value));
        }
    }
}
