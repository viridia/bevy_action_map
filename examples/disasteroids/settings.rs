//! The controls screen: every control the game has, and what it does, and what control it is
//! currently mapped to.
//!
//! Press `F2` (or Y on a pad) to open it. It can be operated end to end from a gamepad without
//! touching the keyboard — the stick and the D-pad move the selection, A presses what is selected,
//! X confirms and B cancels. Pressing a boxed cell listens for the next control and puts it there;
//! if that control already belongs to another row, this row takes it and the other row loses it.
//!
//! There are separate tables for keyboard and gamepad, because the rebinding strategies are
//! different: keyboard allows rebinding of individual keys, while gamepad allows a choice of
//! presets. This mirrors common practice.
//!
//! Selection movement is driven by [`AutoDirectionalNavigation`].
//!
//! # Working copy
//!
//! Nothing is applied to the running game until Confirm. Every capture and every preset press only
//! write into [`PendingOverrides`] — never into the view directly. [`redraw_pending`] is the one
//! place anything reads it back out and repaints a cell, run once per change rather than pushed by
//! whatever changed it, the same way [`prompt_ui`](crate::common::prompt_ui) keeps prompts true.

use bevy::input_focus::{AcquireFocus, AutoFocus, FocusCause, FocusGained, FocusLost, InputFocus};
use bevy::math::CompassOctant;
use bevy::prelude::*;
use bevy::ui::UiSystems;
use bevy::ui::auto_directional_navigation::{AutoDirectionalNavigation, AutoDirectionalNavigator};
use bevy::ui_widgets::{Activate, Button};
use bevy_action_map::mapping::{declared_mappings, fallback_label};
use bevy_action_map::overrides::{Override, Overrides, apply_overrides_with_preset};
use bevy_action_map::prelude::*;
use bevy_action_map::preset::Preset;
use bevy_input::{gamepad::GamepadButton, keyboard::KeyCode};

use crate::actions::{Back, Confirm, Menu, Navigate, ToggleSettings, Turn};
use crate::common::prompt_ui::{PromptScheme, PromptSpan};
use crate::common::widget_focus::{
    Adjusted, ButtonFocused, Stepper, decrement_pressed, increment_pressed,
};
use crate::pause::Simulating;

// Colors

/// A row the player may change, and the box drawn around such a cell.
const CHANGEABLE: Color = Color::srgb(0.75, 0.95, 0.8);
/// Everything that is listed to be read rather than changed.
const FIXED: Color = Color::srgb(0.55, 0.6, 0.62);
/// A follower's line: dimmer than [`FIXED`], since it is a fact about the row above rather than a
/// row the player reads on its own.
const SUBORDINATE: Color = Color::srgb(0.4, 0.44, 0.46);
const HEADING: Color = Color::srgb(0.45, 0.7, 0.95);
const TITLE: Color = Color::srgb(0.9, 0.95, 1.0);
/// The ring drawn around whatever the selection is on.
const FOCUS: Color = Color::srgb(1.0, 0.85, 0.3);
/// The background a cell shows while it is listening for the next control — without this, a
/// capture in progress and one that has not started look identical.
const LISTENING: Color = Color::srgb(0.4, 0.28, 0.05);
/// The preset currently in effect, drawn distinct from the rest of the row — its own color, since
/// "selected" and "listening" are not the same fact about a cell.
const SELECTED: Color = Color::srgb(0.25, 0.55, 0.35);

/// The width of the column holding what a row is called, and of each control column after it.
const NAME_WIDTH: f32 = 210.0;
const CONTROL_WIDTH: f32 = 155.0;
/// How far a follower's line sits under the row it rides.
const FOLLOWER_INDENT: f32 = 20.0;

/// Player-adjustable values with nowhere yet to land — the stepper's second sample point,
/// alongside [`Button`]. `dead_zone` does not reach `Turn`'s own binding, which still reads the
/// fixed `DeadZone::radial(0.15)` `actions.rs` declares at app-build time: making it live is
/// chunk 22's own "preference stage" work, not something a resource can do on its own. This proves
/// the stepper on a second kind of value; it is not a working setting yet.
///
/// Named `Prefs` rather than `Settings`, which this file already uses for the screen's own
/// visibility state.
#[derive(Resource, Clone, Copy)]
struct Prefs {
    dead_zone: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { dead_zone: 0.15 }
    }
}

const DEAD_ZONE_MIN: f32 = 0.0;
const DEAD_ZONE_MAX: f32 = 0.5;
const DEAD_ZONE_STEP: f32 = 0.05;

/// Whether the controls screen is up.
///
/// A state rather than a flag on a resource, for the reason [`Game`](crate::pause::Game) is one: the
/// screen is spawned by `OnEnter` and despawned by `OnExit`, and there is one fact about whether it
/// is showing rather than a screen and a flag that have to agree.
#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Settings {
    #[default]
    Hidden,
    Showing,
}

/// Every change the player has made on this visit to the screen, unconfirmed.
///
/// Reset empty whenever the screen opens. Confirm is the only path from here into the running game,
/// via [`apply_overrides_with_preset`]. Every capture and every preset press writes into `rows` and
/// nothing else — this is a model, and [`redraw_pending`] is the only thing that reads it back out
/// to repaint a cell, so what a row shows is always what Confirm would commit without either writer
/// having to know how to draw one.
#[derive(Resource, Default)]
struct PendingOverrides {
    /// Captures and a preset's rows alike, the whole working copy Confirm applies.
    rows: Overrides,
    /// The rows the currently selected preset authorized, replaced wholesale on each preset press
    /// and never accumulated — picking a new preset supersedes the last rather than layering onto
    /// it. Confirm hands this to [`apply_overrides_with_preset`] so a preset's own rows still move
    /// even though the row they name is `Fixed` everywhere a capture can reach.
    preset_rows: Overrides,
}

pub fn plugin(app: &mut App) {
    app.init_state::<Settings>();
    app.init_resource::<PendingOverrides>();
    app.init_resource::<Prefs>();
    app.add_observer(acquire_focus_directional);
    app.add_systems(OnEnter(Settings::Showing), (reset_pending, show));
    app.add_systems(OnExit(Settings::Showing), release_focus);
    // Ahead of every UI system, so a cell that changed this frame is laid out at the width its new
    // text wants rather than the width it used to be — the same reason `prompt_ui` runs where it
    // does. `resource_changed` alone is enough: the cells a fresh screen spawns already read
    // straight off the live mapping list, which is what an empty `PendingOverrides` already agrees
    // with, so there is no "just spawned, still stale" case to also catch here.
    app.add_systems(
        PostUpdate,
        (
            redraw_pending.run_if(resource_changed::<PendingOverrides>),
            redraw_dead_zone.run_if(resource_changed::<Prefs>),
        )
            .before(UiSystems::Prepare),
    );

    // `Menu` being exclusive already stops the ship answering; this stops the simulation
    // continuing to run behind a screen nobody can see it through. A second, independent
    // `run_if` on the set `pause::plugin` already configures, rather than a state this file
    // would have to remember to hand back — `Simulating` composes the two conditions itself, so
    // there is nothing to restore if the game was already paused when the screen opened.
    app.configure_sets(Update, Simulating.run_if(in_state(Settings::Hidden)));
    app.configure_sets(FixedUpdate, Simulating.run_if(in_state(Settings::Hidden)));
}

/// Starts this visit with nothing changed.
fn reset_pending(mut pending: ResMut<PendingOverrides>) {
    *pending = PendingOverrides::default();
}

/// Opens the screen, and closes it again.
///
/// Attached twice: to the shell context's entity by [`actions::shell`](crate::actions::shell), like
/// the other controls the player can always reach, and to the screen's own root by [`screen`] below
/// — `Menu` binds `ToggleSettings` a second time so the same key that opened the screen also closes
/// it, without needing `Shell` to answer while `Menu` shadows it.
pub(crate) fn toggle(
    _: On<Fired<ToggleSettings>>,
    settings: Res<State<Settings>>,
    mut next: ResMut<NextState<Settings>>,
) {
    next.set(match settings.get() {
        Settings::Hidden => Settings::Showing,
        Settings::Showing => Settings::Hidden,
    });
}

/// Spawns the screen from what the world says is bound.
///
/// A system rather than `screen.spawn()`, because the scene is built out of the mapping list and so
/// needs the world to build it from.
fn show(world: &World, mut commands: Commands) {
    commands.spawn_scene(screen(world));
}

/// Forgets what was selected, because in a moment it will not exist.
///
/// The screen despawns itself — its root is scoped to the state — and leaving the focus pointing at
/// an entity that is gone would have the rest of the app dispatching keystrokes into a hole.
fn release_focus(mut focus: ResMut<InputFocus>) {
    focus.clear();
}

/// Moves the selection.
///
/// The value is a compass direction, because that is what the binding rounded it to; converting it
/// to one of Bevy's own octants is the whole of what this has to do. A value at rest is a change
/// like any other — the player let go — and there is no direction in it, which is what `Dir2`
/// refusing to be built from a zero vector says for us.
///
/// [`AutoDirectionalNavigator`] rather than the manual one: this screen declares no links, so every
/// answer comes from where the widgets are on screen.
pub(crate) fn navigate(fired: On<Fired<Navigate>>, mut nav: AutoDirectionalNavigator) {
    let Ok(direction) = Dir2::new(fired.value) else {
        return;
    };
    // Nothing to do about a direction with nothing in it: the selection is against an edge, which
    // is what the player will see when it does not move.
    let _ = nav.navigate(CompassOctant::from(direction));
}

/// Cancels a capture in progress, or — with none in progress — leaves the screen without applying
/// anything.
///
/// Two things one button does depending on state, rather than two buttons: a listening row and a
/// screen with unconfirmed changes are the same "not sure yet" the player is backing out of, one
/// level at a time.
pub(crate) fn back(
    _: On<Fired<Back>>,
    listening: Query<Entity, With<CaptureSession>>,
    mut commands: Commands,
    mut next: ResMut<NextState<Settings>>,
) {
    if let Some(entity) = listening.iter().next() {
        commands
            .entity(entity)
            .remove::<CaptureSession>()
            .insert(BackgroundColor(Color::NONE));
        return;
    }
    next.set(Settings::Hidden);
}

/// Applies the working copy to the running game, and leaves.
pub(crate) fn confirm(_: On<Fired<Confirm>>, mut commands: Commands) {
    commands.queue(apply_and_close);
}

/// The two ways Confirm is reached — the action above, and the button below — end here.
fn apply_and_close(world: &mut World) {
    let pending = world.resource::<PendingOverrides>();
    let rows = pending.rows.clone();
    let preset_rows = pending.preset_rows.clone();
    apply_overrides_with_preset(world, &rows, &preset_rows);
    world
        .resource_mut::<NextState<Settings>>()
        .set(Settings::Hidden);
}

/// Draws and erases the ring around the selection.
///
/// The [`Outline`] is already on every focusable, holding [`Color::NONE`]; these two change its
/// colour rather than inserting and removing the component, which is what Bevy's own documentation
/// asks for — a ring that moves every time the player nudges the stick would otherwise be an
/// archetype move every time as well.
fn ring_on(gained: On<FocusGained>, mut outlines: Query<&mut Outline>) {
    if let Ok(mut outline) = outlines.get_mut(gained.entity) {
        outline.color = FOCUS;
    }
}

fn ring_off(lost: On<FocusLost>, mut outlines: Query<&mut Outline>) {
    if let Ok(mut outline) = outlines.get_mut(lost.entity) {
        outline.color = Color::NONE;
    }
}

/// Every preset this game offers.
///
/// Built fresh against the world rather than declared once, the same way [`start_capture`] resolves
/// a row: a `MappingKey` cannot be built outside the crate, so `Preset::build` is what asks the
/// world what `Turn`'s gamepad row actually is, the same way `add_context` asks it what `Turn`
/// itself is.
fn presets(world: &World) -> Vec<Preset> {
    vec![
        Preset {
            name: "disasteroids.default",
            rows: Overrides::new(),
        },
        Preset::build(world, "disasteroids.southpaw", |southpaw| {
            southpaw.bind::<Turn>(
                Scheme::Gamepad,
                [Control::GamepadAxis(GamepadAxis::RightStickX)],
            );
        }),
    ]
}

/// The row named `scheme` and `key`, if any mapping in the list is.
fn row_named(rows: &[Mapping], scheme: Scheme, key: MappingKey) -> Option<&Mapping> {
    rows.iter()
        .find(|row| row.scheme == scheme && row.key == key)
}

/// Which of `presets` currently matches what is bound, if any.
///
/// Checked against the union of every row any preset in the list names, not just this preset's own
/// — a preset that names nothing (`Default`) is a claim that none of *them* have moved, which is
/// only answerable by looking at what the others would have changed. A row a preset does not name
/// reads as that row's own declared default, the same rule [`effective`] already applies to a
/// pending row nobody has touched.
fn selected_preset(
    presets: &[Preset],
    declared: &[Mapping],
    live: &[Mapping],
    pending: &Overrides,
) -> Option<&'static str> {
    let touched: Vec<(Scheme, MappingKey)> = presets
        .iter()
        .flat_map(|preset| preset.rows.iter().map(|(scheme, key, _)| (scheme, key)))
        .collect();

    presets
        .iter()
        .find(|preset| {
            touched.iter().all(|&(scheme, key)| {
                let Some(declared_row) = row_named(declared, scheme, key) else {
                    return false;
                };
                let Some(live_row) = row_named(live, scheme, key) else {
                    return false;
                };
                effective(declared_row, &preset.rows) == effective(live_row, pending)
            })
        })
        .map(|preset| preset.name)
}

/// The whole screen, as a scene.
///
/// [`mappings`] hands back every row the game has declared, in both schemes. Splitting them into two
/// tables and sorting each is the screen's business, which is why the crate does not do it.
///
/// The root carries [`Menu`] and the observers for its actions, which is the arrangement
/// [`actions::shell`](crate::actions::shell) already uses for the always-on controls. Here it buys
/// something the shell does not need: the context is the screen, so there is no activation
/// condition to write and nothing to switch off on the way out.
fn screen(world: &World) -> impl Scene {
    // `Menu`'s own bindings — the stick, the D-pad and the arrow keys that move the selection on
    // this very screen — are machinery for operating the settings screen, not controls a player
    // thinks of as part of the game. `mappings` cannot tell the two apart on its own, so this is
    // the one place the screen names a context: everything from here down still reads `Mapping`
    // alone. `ButtonFocused` is excluded for the same reason — `common::widget_focus`'s bridge, not
    // a control this screen's own player thinks of as bindable.
    let all: Vec<Mapping> = mappings(world)
        .into_iter()
        .filter(|mapping| mapping.context != Menu::PATH && mapping.context != ButtonFocused::PATH)
        .collect();
    let rows = |scheme| -> Vec<Mapping> {
        all.iter()
            .filter(|mapping| mapping.scheme == scheme)
            .cloned()
            .collect()
    };

    // A stick has no boxed cell of its own to press, so a preset is that table's whole remapping
    // story — the row of buttons below it is drawn distinct exactly where the current selection
    // reads as matching what is bound.
    let presets = presets(world);
    let declared = declared_mappings(world);
    let pending = world.resource::<PendingOverrides>().rows.clone();
    let selected = selected_preset(&presets, &declared, &all, &pending);
    let dead_zone = world.resource::<Prefs>().dead_zone;

    bsn! {
        // Closing the screen is nothing but despawning it, which the state can do on its own — and
        // that takes the context below with it.
        DespawnOnExit::<Settings>(Settings::Showing)
        Menu
        on(navigate)
        on(back)
        on(confirm)
        on(toggle)
        // Over the game and the debug overlay both, since it covers them.
        GlobalZIndex(10)
        BackgroundColor({Color::srgba(0.02, 0.02, 0.06, 0.97)})
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(22.0),
        }
        Children [
            (
                Text::new("CONTROLS")
                TextFont { font_size: 28.0_f32 }
                TextColor(TITLE)
            ),
            (
                Node { column_gap: Val::Px(64.0), align_items: AlignItems::Start }
                Children [
                    ({table("Keyboard & Mouse", rows(Scheme::KeyboardMouse))}),
                    (
                        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0) }
                        Children [
                            ({table("Gamepad", rows(Scheme::Gamepad))}),
                            ({preset_row(&presets, selected)}),
                            ({dead_zone_row(dead_zone)}),
                        ]
                    ),
                ]
            ),
            (
                Node { column_gap: Val::Px(16.0), margin: {UiRect::top(Val::Px(6.0))} }
                Children [
                    // Cancel first in the tree as well as on screen, so that the one the selection
                    // starts on is also the one the eye starts on.
                    ({cancel_button()}),
                    ({confirm_button()}),
                ]
            ),
            // The one thing on this screen that has to know an action. A span rather than a
            // lookup formatted into the sentence: the question is what would fire it *now*, so
            // the answer skips a context that is switched off and a control something else has
            // taken — and it changes while the screen is up, once this screen can rebind.
            (
                Text::new(
                    "Boxed cells are the ones this game offers for rebinding — press one, then \
                     press what you want bound there; everything else is listed so you can see \
                     what it does.\nPress "
                )
                Node {
                    margin: UiRect::axes(percent(10), px(0))
                }
                TextFont { font_size: 14.0_f32 }
                TextColor(FIXED)
                Children [
                    (
                        PromptSpan({ToggleSettings::id()})
                        TextFont { font_size: 14.0_f32 }
                        TextColor(TITLE)
                    ),
                    (
                        TextSpan::new(" to close.")
                        TextFont { font_size: 14.0_f32 }
                        TextColor(FIXED)
                    ),
                ]
            ),
        ]
    }
}

/// Discards the working copy and leaves. Where the selection starts, since it is the one action a
/// player who opened this screen by accident is guaranteed to want.
///
/// The caption carries its own shortcut — `PromptScheme(Gamepad)` because a keyboard player reads
/// [`Back`]'s own binding (`Escape`) off the row it is bound on, and the pad has no such row here.
fn cancel_button() -> impl Scene {
    bsn! {
        Button
        on(cancel_pressed)
        AutoFocus
        focusable()
        Text::new("Cancel (")
        TextFont { font_size: 16.0_f32 }
        TextColor(TITLE)
        BorderColor::all(FIXED)
        Node {
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(18.0), Val::Px(5.0))},
        }
        Children [
            (
                PromptSpan({Back::id()})
                template_value(PromptScheme(Scheme::Gamepad))
                TextFont { font_size: 16.0_f32 }
                TextColor(TITLE)
            ),
            (TextSpan::new(")") TextFont { font_size: 16.0_f32 } TextColor(TITLE)),
        ]
    }
}

/// Applies the working copy to the running game and leaves.
fn confirm_button() -> impl Scene {
    bsn! {
        Button
        on(confirm_pressed)
        focusable()
        Text::new("Confirm (")
        TextFont { font_size: 16.0_f32 }
        TextColor(TITLE)
        BorderColor::all(FIXED)
        Node {
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(18.0), Val::Px(5.0))},
        }
        Children [
            (
                PromptSpan({Confirm::id()})
                template_value(PromptScheme(Scheme::Gamepad))
                TextFont { font_size: 16.0_f32 }
                TextColor(TITLE)
            ),
            (TextSpan::new(")") TextFont { font_size: 16.0_f32 } TextColor(TITLE)),
        ]
    }
}

/// A mouse click or `Enter` on a focused Cancel — the pad's B reaches the same outcome through
/// [`back`] instead, since B also has the mid-capture meaning Cancel does not.
fn cancel_pressed(_: On<Activate>, mut next: ResMut<NextState<Settings>>) {
    next.set(Settings::Hidden);
}

/// A mouse click or `Enter` on a focused Confirm — the pad's X reaches the same outcome through
/// [`confirm`] instead.
fn confirm_pressed(_: On<Activate>, mut commands: Commands) {
    commands.queue(apply_and_close);
}

/// The row of preset buttons under the gamepad table.
///
/// `+ use<>` on this and [`preset_button`] below: both build an owned scene from what they're
/// handed rather than holding onto it, but Rust 2024's default `impl Trait` capture rule would tie
/// the result to `presets`'s borrow anyway, which does not outlive the caller's own local of the
/// same name in [`screen`].
fn preset_row(presets: &[Preset], selected: Option<&'static str>) -> impl Scene + use<> {
    let buttons: Vec<_> = presets
        .iter()
        .map(|preset| preset_button(preset, Some(preset.name) == selected))
        .collect();
    bsn! {
        Node { column_gap: Val::Px(10.0) }
        Children [{buttons}]
    }
}

/// One preset's own button. The one currently in effect is drawn distinct from the rest — the same
/// border/background swap [`LISTENING`] uses for "a capture is happening here", with its own color,
/// since "selected" and "listening" are not the same fact about a cell.
fn preset_button(preset: &Preset, selected: bool) -> impl Scene + use<> {
    let name = preset.name;
    let label = fallback_label(name);
    let border = if selected { SELECTED } else { FIXED };
    let background = if selected {
        SELECTED.with_alpha(0.25)
    } else {
        Color::NONE
    };
    bsn! {
        Button
        on(preset_pressed)
        focusable()
        template_value(PresetButton(name))
        Text::new(label)
        TextFont { font_size: 15.0_f32 }
        TextColor(TITLE)
        BorderColor::all(border)
        BackgroundColor(background)
        Node {
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(12.0), Val::Px(4.0))},
        }
    }
}

/// A label and its stepper, the same "row names what it is, then draws the control" shape the two
/// tables already use.
fn dead_zone_row(value: f32) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0) }
        Children [
            (Text::new("Dead zone") TextFont { font_size: 14.0_f32 } TextColor(HEADING)),
            ({stepper(value)}),
        ]
    }
}

/// One chevron on either side of the value, `justify_content: SpaceBetween` so the row's own width
/// is what spaces them rather than a gap that would also grow the digits between them.
///
/// The chevrons are `Button`s but not `focusable()`: this row is the one tab stop, the same
/// distinction [`common::widget_focus`](crate::common::widget_focus) draws between a stepper and
/// the widgets inside it — a click still presses one (`bevy_ui_widgets` sees to that on its own),
/// it just never moves the selection.
fn stepper(value: f32) -> impl Scene {
    bsn! {
        Stepper
        on(apply_dead_zone_delta)
        focusable()
        Node {
            width: Val::Px(130.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(8.0), Val::Px(4.0))},
        }
        BorderColor::all(CHANGEABLE)
        Children [
            (Button on(decrement_pressed) Text::new("<") TextFont { font_size: 16.0_f32 } TextColor(TITLE)),
            (
                DeadZoneValue
                Text::new(format!("{value:.2}"))
                TextFont { font_size: 15.0_f32 }
                TextColor(TITLE)
            ),
            (Button on(increment_pressed) Text::new(">") TextFont { font_size: 16.0_f32 } TextColor(TITLE)),
        ]
    }
}

/// Names the stepper's own value `Text`, so [`redraw_dead_zone`] can find it again — mirrors how
/// [`RowCell`] names a cell rather than a system holding onto the entity itself.
#[derive(Component, Default, Clone, Copy)]
struct DeadZoneValue;

/// Applies one step, clamped. Never touches the view directly — [`redraw_dead_zone`] is the one
/// place anything reads [`Settings`] back out, the same division [`redraw_pending`] keeps.
fn apply_dead_zone_delta(adjusted: On<Adjusted>, mut prefs: ResMut<Prefs>) {
    prefs.dead_zone =
        (prefs.dead_zone + adjusted.delta * DEAD_ZONE_STEP).clamp(DEAD_ZONE_MIN, DEAD_ZONE_MAX);
}

/// Repaints the stepper's value after [`apply_dead_zone_delta`] changes it — run once per change
/// rather than pushed by the observer that caused it, same as [`redraw_pending`].
fn redraw_dead_zone(prefs: Res<Prefs>, mut value: Query<&mut Text, With<DeadZoneValue>>) {
    if let Ok(mut text) = value.single_mut() {
        *text = Text::new(format!("{:.2}", prefs.dead_zone));
    }
}

/// Names the preset a button selects, so a press can find its rows again — mirrors how
/// [`RebindCell`] names a row by key rather than carrying the row's own data.
#[derive(Component, Clone, Copy)]
struct PresetButton(&'static str);

/// Writes the pressed preset's rows into the working copy. Nothing else — no cell this observer
/// could name is touched directly; [`redraw_pending`] notices the change and repaints everything
/// that might have moved, including every preset button's own highlight.
///
/// Every row *any* registered preset names is cleared first, not just the ones this preset itself
/// names: picking a new preset supersedes whatever the last one wrote rather than layering onto it,
/// and a preset that names nothing (`Default`) is a claim about all of them, the same reading
/// [`selected_preset`] already gives that case. Skipping this step is exactly the bug an earlier
/// version of this function had — `Default`'s own rows are empty, so writing only what it names
/// wrote nothing at all, and whatever the last preset had moved simply stayed moved.
fn preset_pressed(activate: On<Activate>, buttons: Query<&PresetButton>, mut commands: Commands) {
    let Ok(&PresetButton(name)) = buttons.get(activate.entity) else {
        return;
    };
    commands.queue(move |world: &mut World| {
        let presets = presets(world);
        let Some(preset) = presets.iter().find(|preset| preset.name == name) else {
            return;
        };
        let touched: Vec<(Scheme, MappingKey)> = presets
            .iter()
            .flat_map(|preset| preset.rows.iter().map(|(scheme, key, _)| (scheme, key)))
            .collect();

        let mut pending = world.resource_mut::<PendingOverrides>();
        for (scheme, key) in touched {
            pending.rows.reset(scheme, key);
        }
        for (scheme, key, over) in preset.rows.iter() {
            pending.rows.set(scheme, key, over.clone());
        }
        pending.preset_rows = preset.rows.clone();
    });
}

/// Rewrites every row and every preset button to match the pending working copy.
///
/// Every one of them rather than only the row a capture or a preset press actually named: a steal
/// can move any row, and a preset can move several at once, so asking "which one" buys nothing a
/// full pass does not already answer just as cheaply. What keeps that affordable is the run
/// condition this is registered with — it does not run at all on a frame where
/// [`PendingOverrides`] did not change — the same trade `prompt_ui::refresh_prompts` makes for the
/// same reason.
///
/// Exclusive, because it reads every tagged cell in the table alongside the mapping list, the
/// declared defaults, the preset list and the pending copy all at once.
fn redraw_pending(world: &mut World) {
    let live = mappings(world);
    let declared = declared_mappings(world);
    let presets = presets(world);
    let pending = world.resource::<PendingOverrides>().rows.clone();

    let mut principals = world.query::<(&RowCell, &mut Text)>();
    for (cell, mut text) in principals.iter_mut(world) {
        let Some(row) = row_named(&live, cell.0, cell.1) else {
            continue;
        };
        *text = Text::new(
            effective(row, &pending)
                .get(cell.2)
                .map(|control| control.fallback_label().into_owned())
                .unwrap_or_default(),
        );
    }

    let mut followers = world.query::<(&FollowerCell, &mut Text)>();
    for (cell, mut text) in followers.iter_mut(world) {
        let Some(row) = row_named(&live, cell.0, cell.1) else {
            continue;
        };
        *text = Text::new(
            effective(row, &pending)
                .get(cell.2)
                .map_or_else(String::new, |control| {
                    cell.3.fallback_format(&control.fallback_label())
                }),
        );
    }

    let selected = selected_preset(&presets, &declared, &live, &pending);
    let mut buttons = world.query::<(&PresetButton, &mut BorderColor, &mut BackgroundColor)>();
    for (button, mut border, mut background) in buttons.iter_mut(world) {
        let is_selected = Some(button.0) == selected;
        *border = BorderColor::all(if is_selected { SELECTED } else { FIXED });
        *background = BackgroundColor(if is_selected {
            SELECTED.with_alpha(0.25)
        } else {
            Color::NONE
        });
    }
}

/// Everything a selection can land on carries these three.
///
/// [`AutoDirectionalNavigation`] is what makes an entity a candidate, and what
/// [`acquire_focus_directional`] answers a click through; the [`Outline`] is the ring, kept
/// colourless until the selection arrives so that showing it is a colour change rather than a
/// component insertion; the two observers are what change it.
fn focusable() -> impl Scene {
    bsn! {
        AutoDirectionalNavigation
        Outline { width: Val::Px(2.0), offset: Val::Px(2.0), color: Color::NONE }
        on(ring_on)
        on(ring_off)
    }
}

/// Claims focus for a [`focusable`] widget the moment a pointer presses it, rather than after
/// `Activate` fires at release.
///
/// `bevy_input_focus`'s own `click_to_focus` triggers a bubbling `AcquireFocus` on every pointer
/// press, before `bevy_ui_widgets` has decided whether a click landed. This screen's selection is
/// driven by [`AutoDirectionalNavigation`] rather than `TabIndex`, so nothing intercepted that
/// request — it bubbled all the way to the window and *cleared* focus, restored only once
/// `Activate` fired at release. Reclaiming after the fact is a visible blink whenever press and
/// release land on different entities, which a widget with interactive children of its own (a
/// stepper's two chevrons) makes routine rather than rare. This is
/// `bevy_input_focus::tab_navigation::acquire_focus_tab_index`'s own fix, `AutoDirectionalNavigation`
/// standing in for `TabIndex`.
fn acquire_focus_directional(
    mut acquire: On<AcquireFocus>,
    focusable: Query<(), With<AutoDirectionalNavigation>>,
    mut focus: ResMut<InputFocus>,
) {
    if focusable.contains(acquire.focused_entity) {
        acquire.propagate(false);
        if focus.get() != Some(acquire.focused_entity) {
            focus.set(acquire.focused_entity, FocusCause::Pressed);
        }
    }
}

/// One device's worth of rows, under a heading and grouped by category.
///
/// The column count is the data's rather than the screen's: a row says how many controls it can
/// hold, and the widest row in the table decides how many cells every row draws. That is what makes
/// the keyboard table three columns wide — name, primary, secondary — and the pad table two, with
/// nothing here saying so.
fn table(title: &'static str, mut rows: Vec<Mapping>) -> impl Scene {
    let columns = rows.iter().map(slots_in).max().unwrap_or(1);
    // Stable, so rows keep the order the game declared them in within each category.
    rows.sort_by_key(|mapping| (mapping.category.is_none(), mapping.category));

    let mut lines = Vec::new();
    let mut category = None;
    for mapping in rows {
        if category != Some(mapping.category) {
            category = Some(mapping.category);
            lines.push(line(
                vec![Cell {
                    // A category is a localization key on the same terms as a row's name, so a game
                    // with a catalogue looks it up here exactly as `label` does below.
                    text: mapping
                        .category
                        .map_or_else(|| String::from("Other"), fallback_label),
                    width: NAME_WIDTH,
                    color: HEADING,
                    border: Color::NONE,
                    role: CellRole::Label,
                }],
                0.0,
            ));
        }
        lines.push(line(cells(&mapping, columns), 0.0));
        // Chunk 44 gave `Afterburner` a link to `Thrust` and nowhere to be drawn; this is where.
        // Indented and dimmed rather than a row of its own, and not activatable — a follower is not
        // separately rebindable, and a button that did nothing would say otherwise.
        for follower in &mapping.followers {
            lines.push(line(
                follower_cells(&mapping, follower, columns),
                FOLLOWER_INDENT,
            ));
        }
    }

    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0) }
        Children [
            (
                Text::new(title)
                TextFont { font_size: 18.0_f32 }
                TextColor(TITLE)
                Node { margin: {UiRect::bottom(Val::Px(6.0))} }
            ),
            {lines},
        ]
    }
}

/// How many controls a row can hold, which for a row nobody may change is however many it holds.
fn slots_in(mapping: &Mapping) -> usize {
    mapping.capacity.slots().unwrap_or(mapping.slots.len())
}

/// One row: what it is called, then a cell per column.
fn cells(mapping: &Mapping, columns: usize) -> Vec<Cell> {
    let changeable = mapping.rebinding.is_rebindable();
    let color = if changeable { CHANGEABLE } else { FIXED };
    let mut cells = vec![Cell {
        text: label(mapping.key),
        width: NAME_WIDTH,
        color,
        border: Color::NONE,
        role: CellRole::Label,
    }];

    for column in 0..columns {
        // A slot this row does not have is blank and unboxed; one it has and has not filled is an
        // empty box, which is what a spare secondary looks like before the player uses it.
        let exists = column < slots_in(mapping);
        cells.push(Cell {
            text: mapping
                .slots
                .get(column)
                .map(|control| control.fallback_label().into_owned())
                .unwrap_or_default(),
            width: CONTROL_WIDTH,
            color,
            border: if changeable && exists {
                CHANGEABLE.with_alpha(0.35)
            } else {
                Color::NONE
            },
            // Exactly the cells the box is drawn around, which is what the box was always
            // promising: the selection can reach what the player may change, and skips the rest.
            // A slot that exists but is not changeable still gets an identity — `Fixed` rather
            // than `Label` — because a preset may still move it even though a capture never will.
            role: if !exists {
                CellRole::Label
            } else if changeable {
                CellRole::Changeable(mapping.scheme, mapping.key, column)
            } else {
                CellRole::Fixed(mapping.scheme, mapping.key, column)
            },
        });
    }
    cells
}

/// One follower's line: its own name, then the principal's controls with the follower's own
/// condition formatted in — "Hold W" under "W", not the bare word "hold". A follower has no slots
/// of its own to draw; a blank column here is the row above not having filled that slot either.
fn follower_cells(mapping: &Mapping, follower: &Follower, columns: usize) -> Vec<Cell> {
    let mut cells = vec![Cell {
        text: follower.fallback_label(),
        width: NAME_WIDTH,
        color: SUBORDINATE,
        border: Color::NONE,
        role: CellRole::Label,
    }];

    for column in 0..columns {
        let text = mapping
            .slots
            .get(column)
            .map_or_else(String::new, |control| {
                follower
                    .condition
                    .fallback_format(&control.fallback_label())
            });
        cells.push(Cell {
            text,
            width: CONTROL_WIDTH,
            color: SUBORDINATE,
            border: Color::NONE,
            role: CellRole::Follower(mapping.scheme, mapping.key, column, follower.condition),
        });
    }
    cells
}

/// What a mapping is called on screen.
///
/// The crate hands over a localization key rather than words, because that half of a row is as
/// translatable as the control beside it. A shipped game looks the key up in its catalogue;
/// Disasteroids has none, and answers for the two keys whose derived text is not what a player
/// should read.
fn label(key: MappingKey) -> String {
    match key.to_string().as_str() {
        "disasteroids.turn.negative" => String::from("Turn Left"),
        "disasteroids.turn.positive" => String::from("Turn Right"),
        _ => key.fallback_label(),
    }
}

/// One cell of a table, whatever it holds: a heading, a row's name, or a control.
struct Cell {
    text: String,
    width: f32,
    color: Color,
    /// Drawn around the cells the player will be able to press once rebinding lands, and around the
    /// empty ones they will be able to fill. [`Color::NONE`] for the rest.
    border: Color,
    role: CellRole,
}

/// What kind of thing a cell is, which is also what identifies it for [`redraw_pending`] and, where
/// a capture applies, for starting one.
///
/// `MappingKey` alone does not name a row — the same key is shared by a keyboard mapping and a
/// gamepad one for the same action (it is derived from the action's path and part, and says nothing
/// about the scheme), so every role that names a row carries its `Scheme` too.
#[derive(Clone, Copy)]
enum CellRole {
    /// A heading, a row's name, or a follower's name — read, never pressed, and never moved by
    /// anything this screen does.
    Label,
    /// A control the player may press to capture a new one into this slot.
    Changeable(Scheme, MappingKey, usize),
    /// A control filled in but not player-capturable here — every gamepad row, since a preset
    /// rather than this screen's own capture is that table's whole remapping story. Still named,
    /// because a preset can still move it and [`redraw_pending`] has to find it again when one does.
    Fixed(Scheme, MappingKey, usize),
    /// A follower's line under one column of the row above it, carrying the condition its caption
    /// is formatted with — a capture on that column has to reformat this cell too, not just the
    /// principal one.
    Follower(Scheme, MappingKey, usize, ConditionDescriptor),
}

/// `indent` is nonzero for exactly a follower's line — the mark of a row that is a fact about the
/// principal above it rather than a row of its own, since every cell in it is already
/// [`SUBORDINATE`]. A parameter rather than a second function: two `impl Scene` functions are two
/// different opaque types, and `table` below builds one `Vec` holding both ordinary and follower
/// lines.
fn line(cells: Vec<Cell>, indent: f32) -> impl Scene {
    let cells: Vec<_> = cells.into_iter().map(cell).collect();
    bsn! {
        Node { column_gap: Val::Px(8.0), margin: {UiRect::left(Val::Px(indent))} }
        Children [{cells}]
    }
}

fn cell(cell: Cell) -> impl Scene {
    // A changeable cell is a button and a stop for the selection; a fixed one is neither, and
    // stays the plain text it was. Three independent splices rather than branching to one of three
    // different `bsn!` blocks — each of those would be its own opaque type, and this way there is
    // exactly one `impl Scene` this function ever returns.
    // `BackgroundColor` starts at `Color::NONE` here rather than being absent, so `start_capture`
    // and its two ways out (`captured`, `back`'s mid-capture branch) are all just writing the same
    // component, never inserting or removing it.
    let selectable = matches!(cell.role, CellRole::Changeable(..)).then(|| {
        bsn! {
            Button
            on(start_capture)
            on(captured)
            BackgroundColor(Color::NONE)
            focusable()
        }
    });
    // `template_value` rather than the bare tuple-constructor form `bsn!` otherwise expects: these
    // two are plain data tags with no sensible `Default`, and `bsn!`'s own `Type(args)` syntax needs
    // one (it patches a template, which `template_value` sidesteps by handing over an already-built
    // value).
    let rebind_tag = if let CellRole::Changeable(scheme, key, slot) = cell.role {
        Some(template_value(RebindCell(scheme, key, slot)))
    } else {
        None
    };
    // Every principal cell gets this one, `Changeable` and `Fixed` alike — `redraw_pending` finds a
    // row's cells this way regardless of who is allowed to capture into them, since a preset moves
    // a `Fixed` row `RebindCell` was never attached to.
    let row_tag = match cell.role {
        CellRole::Changeable(scheme, key, slot) | CellRole::Fixed(scheme, key, slot) => {
            Some(template_value(RowCell(scheme, key, slot)))
        }
        CellRole::Label | CellRole::Follower(..) => None,
    };
    let follower_tag = if let CellRole::Follower(scheme, key, slot, condition) = cell.role {
        Some(template_value(FollowerCell(scheme, key, slot, condition)))
    } else {
        None
    };
    // Every cell carries the same border and padding whether or not the border is visible, so the
    // columns line up down the table rather than shifting where a box begins.
    bsn! {
        {selectable}
        {rebind_tag}
        {row_tag}
        {follower_tag}
        Text({cell.text})
        TextFont { font_size: 15.0_f32 }
        TextColor({cell.color})
        BorderColor::all(cell.border)
        Node {
            width: {Val::Px(cell.width)},
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(3.0))},
            padding: {UiRect::axes(Val::Px(6.0), Val::Px(2.0))},
        }
    }
}

/// Names the row and slot a boxed cell would capture for, so a press knows what to start and a
/// result knows which row to write into.
///
/// `Scheme` first because `MappingKey` alone does not name a row — see [`CellRole`].
#[derive(Component, Clone, Copy)]
struct RebindCell(Scheme, MappingKey, usize);

/// Names the row and slot a principal cell displays, whether or not it is capturable — the identity
/// [`redraw_pending`] finds any cell by. Separate from [`RebindCell`], which additionally marks "and
/// this one is interactive": every `RebindCell` is also a `RowCell`, but a `Fixed` row's cell is a
/// `RowCell` with no `RebindCell` beside it, since nothing on this screen may capture into one.
#[derive(Component, Clone, Copy)]
struct RowCell(Scheme, MappingKey, usize);

/// Names the row, column and condition a follower's cell renders, so a capture on that column can
/// reformat this cell's caption along with the principal's.
#[derive(Component, Clone, Copy)]
struct FollowerCell(Scheme, MappingKey, usize, ConditionDescriptor);

/// Starts listening for the control that will fill this cell, and paints it [`LISTENING`] so the
/// player can tell a capture started at all — the only signal on this screen that one has, since a
/// press otherwise looks identical to one that did nothing.
///
/// The session lives on the cell entity itself, per [`CaptureSession`]'s own recommendation — it is
/// the thing that will show the answer. `Back`'s two controls are excluded so a two-stage cancel
/// works from inside a capture: without this, pressing B here would be captured as this row's new
/// binding instead of reaching [`back`] and cancelling the capture.
fn start_capture(activate: On<Activate>, cells: Query<&RebindCell>, mut commands: Commands) {
    let Ok(&RebindCell(scheme, key, slot)) = cells.get(activate.entity) else {
        return;
    };
    let entity = activate.entity;
    commands.queue(move |world: &mut World| {
        let Some(mapping) = mappings(world)
            .into_iter()
            .find(|row| row.scheme == scheme && row.key == key)
        else {
            return;
        };
        let Some(session) = CaptureSession::for_slot(&mapping, slot) else {
            return;
        };
        world.entity_mut(entity).insert((
            session.excluding([
                Control::Key(KeyCode::Escape),
                Control::GamepadButton(GamepadButton::East),
            ]),
            BackgroundColor(LISTENING),
        ));
    });
}

/// Writes what was captured into the working copy, stealing the control from whatever else already
/// held it, patches every cell either row's change touched, and clears [`LISTENING`] off the cell
/// that was — the crate has already removed `CaptureSession` itself by the time this runs.
fn captured(captured: On<Captured>, cells: Query<&RebindCell>, mut commands: Commands) {
    let Ok(&RebindCell(scheme, key, slot)) = cells.get(captured.entity) else {
        return;
    };
    let control = captured.control;
    let entity = captured.entity;
    commands.queue(move |world: &mut World| {
        world
            .entity_mut(entity)
            .insert(BackgroundColor(Color::NONE));
        resolve_capture(world, scheme, key, slot, control);
    });
}

/// What a mapping currently holds, the working copy laid over its declared slots — [`Overrides`]'s
/// own three-state rule, read the same way [`apply_overrides_with_preset`] and `conflicts_pending`
/// both do.
fn effective(mapping: &Mapping, pending: &Overrides) -> Vec<Control> {
    match pending.get(mapping.scheme, mapping.key) {
        Some(Override::Controls(controls)) => controls.clone(),
        Some(Override::Cleared) => Vec::new(),
        Some(Override::NotOurs) | None => mapping.slots.clone(),
    }
}

/// The policy chunk 31 picked for R19.3: a captured control is stolen from whatever else already
/// holds it rather than being refused or left to duplicate.
///
/// Writes into [`PendingOverrides`] and nothing else — no cell is touched from here; see
/// [`redraw_pending`], which notices the change and repaints whatever it finds moved.
///
/// Every row a steal can find is guaranteed to share `scheme` with the row captured into: `control`
/// is itself scheme-specific (a key can never sit in a gamepad row's slots), so nothing outside this
/// scheme can ever hold it. That is what makes looking `clash.mapping` back up against `scheme`
/// rather than a bare key search safe, and it is also why `conflicts_pending` needs no scheme
/// parameter of its own.
fn resolve_capture(
    world: &mut World,
    scheme: Scheme,
    key: MappingKey,
    slot: usize,
    control: Control,
) {
    let all = mappings(world);
    let Some(target) = all
        .iter()
        .find(|row| row.scheme == scheme && row.key == key)
        .cloned()
    else {
        return;
    };
    let mut pending = world.resource_mut::<PendingOverrides>();

    for clash in conflicts_pending(&all, &pending.rows, control, Some(key)) {
        let Some(other) = all
            .iter()
            .find(|row| row.scheme == scheme && row.key == clash.mapping)
        else {
            continue;
        };
        let mut controls = effective(other, &pending.rows);
        controls.retain(|&held| held != control);
        pending.rows.bind(other.scheme, other.key, controls);
    }

    let mut controls = effective(&target, &pending.rows);
    if slot < controls.len() {
        controls[slot] = control;
    } else {
        controls.push(control);
    }
    pending.rows.bind(target.scheme, target.key, controls);
}
