//! The controls screen: every control the game has, and what it does, and what control it is
//! currently mapped to.
//!
//! Press `F2` (or Y on a pad) to open it. It can be operated end to end from a gamepad without
//! touching the keyboard — the stick and the D-pad move the selection, A presses what is selected,
//! X confirms, B cancels and L1 empties the selected cell. Pressing a boxed cell listens for the
//! next control and puts it there; if that control already belongs to another row, this row takes
//! it and the other row loses it, leaving a gap in the column it came out of rather than closing
//! up.
//!
//! There are separate tables for keyboard and gamepad, because the rebinding strategies are
//! different: keyboard allows rebinding of individual keys, while the pad is remapped by whatever
//! the build puts under its table. [`ControlsScreen`] is the layout around the tables, and each
//! build spawns it with a [`MappingColumn`] per family; base Disasteroids puts
//! [`pad_presets`](crate::pad_presets) under the pad's.
//!
//! Selection movement is driven by
//! [`AutoDirectionalNavigation`](bevy::ui::auto_directional_navigation::AutoDirectionalNavigation).
//!
//! # Working copy
//!
//! Nothing is applied to the running game until Confirm. Every capture and every preset press only
//! write into [`PendingOverrides`] — never into the view directly. [`redraw_pending`] is the one
//! place anything reads it back out and repaints a cell, run once per change rather than pushed by
//! whatever changed it, the same way [`prompt_ui`](crate::common::prompt_ui) keeps prompts true.
//!
//! The working copy is a preset's *name* and the rows the player moved by hand, kept apart — see
//! [`Controls`]. Confirm merges them, applies the result, and hands the same two facts to
//! [`saved_controls`](crate::saved_controls) to write down.

use bevy::input_focus::{AutoFocus, InputFocus};
use bevy::math::CompassOctant;
use bevy::prelude::*;
use bevy::scene::{Ready, SceneList};
use bevy::ui::UiSystems;
use bevy::ui::auto_directional_navigation::AutoDirectionalNavigator;
use bevy::ui_widgets::{Activate, Button};
use bevy_action_map::mapping::{TunableValue, fallback_label};
use bevy_action_map::overrides::{
    OverrideProblemKind, Overrides, Rebind, apply_overrides_with_preset,
};
use bevy_action_map::prelude::*;
use bevy_action_map::preset::Preset;
use bevy_input::{gamepad::GamepadButton, keyboard::KeyCode};

use crate::actions::{
    Back, Clear, Confirm, Menu, Navigate, TURN_DEAD_ZONE_KEY, ToggleSettings, Turn,
};
use crate::common::prompt_ui::{IconPrompt, PromptFamily, PromptSpan};
use crate::common::widget_focus::{ButtonFocused, focusable};
use crate::pause::Simulating;
use crate::saved_controls;

// Colors

/// A row the player may change, and the box drawn around such a cell.
pub(crate) const CHANGEABLE: Color = Color::srgb(0.75, 0.95, 0.8);
/// Everything that is listed to be read rather than changed.
pub(crate) const FIXED: Color = Color::srgb(0.55, 0.6, 0.62);
/// A follower's line: dimmer than [`FIXED`], since it is a fact about the row above rather than a
/// row the player reads on its own.
const SUBORDINATE: Color = Color::srgb(0.4, 0.44, 0.46);
pub(crate) const HEADING: Color = Color::srgb(0.45, 0.7, 0.95);
pub(crate) const TITLE: Color = Color::srgb(0.9, 0.95, 1.0);
/// The background a cell shows while it is listening for the next control — without this, a capture
/// in progress and one that has not started look identical.
const LISTENING: Color = Color::srgb(0.4, 0.28, 0.05);
/// Why a press did not take. A lighter relative of [`LISTENING`], because it is the same
/// conversation — the row asked for a control and this is the answer.
const REFUSED_TEXT: Color = Color::srgb(0.95, 0.72, 0.3);

/// The width of the column holding what a row is called, and of each control column after it.
const NAME_WIDTH: f32 = 210.0;
const CONTROL_WIDTH: f32 = 155.0;
/// The same, for the pad table, whose one column has room the keyboard's two do not.
///
/// Wide enough for a chord: "Left Bumper+Right Bumper" is half again as long as anything a single
/// control is called, and wrapping it would push every row under it down half a line.
const GAMEPAD_CONTROL_WIDTH: f32 = 230.0;

/// How tall a button's prompt icon is drawn. The buttons in a row stretch to the tallest, so this
/// sets the height of all three.
const BUTTON_ICON: f32 = 22.0;

/// How many control cells each table draws per row.
///
/// The crate has no opinion: a mapping is a list of controls and how many to offer is this screen's
/// decision. Two on the keyboard, so every changeable row has a spare cell for a second key. One on
/// the pad, whose rows are all fixed here — the console's own remapper owns that — so a second cell
/// would be a promise this screen cannot keep.
const KEYBOARD_COLUMNS: usize = 2;
const GAMEPAD_COLUMNS: usize = 1;
/// How far a follower's line sits under the row it rides.
const FOLLOWER_INDENT: f32 = 20.0;

// A row's own text size, and the gaps and padding sized against it. The screen is one unscrolled
// page, so a new row that does not fit is made room for here, not with scrolling or collapsing
// sections; named so that is a one-line change rather than a hunt through `table` and `cell`.
const ROW_FONT_SIZE: f32 = 13.0;
/// The vertical gap between one row and the next within a table.
const ROW_GAP: f32 = 4.0;
/// The vertical padding inside a cell's border, above and below its text.
const ROW_PADDING_V: f32 = 0.0;

/// Whether the controls screen is up.
///
/// A state rather than a flag on a resource, for the reason [`Game`](crate::pause::Game) is one:
/// the screen is spawned by `OnEnter` and despawned by `OnExit`, and there is one fact about
/// whether it is showing rather than a screen and a flag that have to agree.
#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Settings {
    #[default]
    Hidden,
    Showing,
}

/// A choice of controls: the gamepad preset in effect, and the rows the player moved by hand.
///
/// The two are kept apart rather than accumulated into one override set, because they are stored
/// differently — see [`saved_controls`](crate::saved_controls). [`working_copy`] is where they
/// become the single [`Overrides`] the game actually runs on.
///
/// A name is a claim rather than a measurement, and that is the cost: the player is on Southpaw
/// until they pick something else, however far they have since moved from it. A screen wanting to
/// say "Southpaw (modified)" would have to compare the working copy against the preset's own rows,
/// which nothing here does.
#[derive(Clone)]
pub(crate) struct Controls {
    /// Always a preset this build declares — see [`resolve_preset`].
    pub preset: &'static str,
    /// The rows and tunables the player moved by hand, laid over whatever the preset says.
    pub captures: Overrides,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            preset: DEFAULT_PRESET,
            captures: Overrides::new(),
        }
    }
}

/// What the game is running: written on Confirm, and what each visit to the screen starts from.
///
/// [`saved_controls`](crate::saved_controls) keeps the file-shaped copy of the same choice, loads
/// it at startup and writes it back whenever this changes.
#[derive(Resource, Default)]
pub(crate) struct AppliedControls(pub Controls);

/// Every change the player has made on this visit to the screen, unconfirmed.
///
/// Seeded from [`AppliedControls`] whenever the screen opens, and Confirm is the only path from
/// here into the running game, via [`apply_overrides_with_preset`]. What a row shows is always what
/// Confirm would commit.
#[derive(Resource, Default)]
pub(crate) struct PendingOverrides(pub Controls);

/// The preset a player who has never picked one is on, and where a saved name this build no longer
/// declares lands.
pub(crate) const DEFAULT_PRESET: &str = "disasteroids.default";

/// The preset `name` names, or [`DEFAULT_PRESET`] where nothing does — a preset renamed or dropped
/// in a patch is an ordinary thing for a save file to have lived through.
pub(crate) fn resolve_preset(world: &World, name: &str) -> &'static str {
    presets(world)
        .into_iter()
        .find(|preset| preset.name == name)
        .map_or(DEFAULT_PRESET, |preset| preset.name)
}

/// What the working copy would commit, and the selected preset's own rows beside it.
///
/// The merge is the preset's rows with the captures over them. A capture wins where the two name
/// the same row, which on this screen they never do — every row a preset moves is `Fixed`, and a
/// capture only reaches a `Changeable` one — but the order is stated rather than left to chance.
///
/// The preset's rows come back separately because [`apply_overrides_with_preset`] wants both: the
/// merged set is what to apply, and the preset's own rows are what may bypass the "not rebindable
/// here" refusal on the way in.
pub(crate) fn working_copy(world: &World, controls: &Controls) -> (Overrides, Overrides) {
    let preset_rows = presets(world)
        .into_iter()
        .find(|preset| preset.name == controls.preset)
        .map_or_else(Overrides::new, |preset| preset.rows);

    let mut merged = preset_rows.clone();
    for (family, key, over) in controls.captures.iter() {
        merged.set(family, key, over.clone());
    }
    for (family, key, value) in controls.captures.iter_tunables() {
        merged.tune(family, key, value);
    }
    (merged, preset_rows)
}

/// [`working_copy`] against the unconfirmed copy, which is what everything on this screen draws.
pub(crate) fn pending_copy(world: &World) -> Overrides {
    working_copy(world, &world.resource::<PendingOverrides>().0).0
}

pub fn plugin(app: &mut App) {
    app.init_state::<Settings>();
    app.init_resource::<PendingOverrides>();
    app.init_resource::<AppliedControls>();
    app.init_resource::<Refusal>();
    // The screen itself is spawned by each build, since each fills its slot differently.
    app.add_systems(OnEnter(Settings::Showing), seed_pending);
    app.add_systems(OnExit(Settings::Showing), release_focus);
    // Ahead of every UI system, so a cell that changed this frame is laid out at the width its new
    // text wants rather than the width it used to be — the same reason `prompt_ui` runs where it
    // does. `resource_changed` alone is enough: the cells a fresh screen spawns already read
    // straight off the live mapping list, which is what an empty `PendingOverrides` already agrees
    // with, so there is no "just spawned, still stale" case to also catch here.
    app.add_systems(
        PostUpdate,
        redraw_pending
            .run_if(resource_changed::<PendingOverrides>)
            .before(UiSystems::Prepare),
    );
    app.add_systems(
        PostUpdate,
        redraw_refusal
            .run_if(resource_changed::<Refusal>)
            .before(UiSystems::Prepare),
    );

    // `Menu` being exclusive already stops the ship answering; this stops the simulation continuing
    // to run behind a screen nobody can see it through. A second, independent `run_if` on the set
    // `pause::plugin` already configures, rather than a state this file would have to remember to
    // hand back — `Simulating` composes the two conditions itself, so there is nothing to restore
    // if the game was already paused when the screen opened.
    app.configure_sets(Update, Simulating.run_if(in_state(Settings::Hidden)));
    app.configure_sets(FixedUpdate, Simulating.run_if(in_state(Settings::Hidden)));
}

/// Starts this visit from what the game is running, so a screen reopened after a Confirm shows the
/// preset that was chosen and the rows that were moved rather than an empty slate.
fn seed_pending(applied: Res<AppliedControls>, mut pending: ResMut<PendingOverrides>) {
    pending.0 = applied.0.clone();
}

/// Opens the screen, and closes it again.
///
/// Attached twice: to the shell context's entity by [`actions::shell`](crate::actions::shell), like
/// the other controls the player can always reach, and to the screen's own root by [`ControlsScreen`] below
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

/// Empties the selected cell — the one gesture on this screen that makes a gap on purpose.
///
/// The cell keeps its column: clearing a primary leaves the secondary in the second box rather than
/// sliding it up, which is the difference between "this cell is empty" and "this row is shorter".
/// Only a boxed cell can be cleared, since [`RebindCell`] is what marks the ones this screen owns.
fn clear_cell(
    _: On<Fired<Clear>>,
    focus: Res<InputFocus>,
    cells: Query<&RebindCell>,
    mut commands: Commands,
) {
    let Some(focused) = focus.get() else {
        return;
    };
    let Ok(&RebindCell(family, key, slot)) = cells.get(focused) else {
        return;
    };
    commands.queue(move |world: &mut World| {
        let Some(row) = mappings(world)
            .into_iter()
            .find(|row| row.family == family && row.key == key)
        else {
            return;
        };
        // Into the captures rather than the merged copy, on the same terms as a steal: the player
        // made this, so it outlives whichever preset is selected.
        world
            .resource_mut::<PendingOverrides>()
            .0
            .captures
            .unbind(&row, slot);
    });
}

/// The two ways Confirm is reached — the action above, and the button below — end here.
///
/// The one place the working copy leaves this screen, so it is also the one place the choice is
/// written down: to the running game, to [`AppliedControls`] for the next visit, and to the
/// settings file for the next launch.
fn apply_and_close(world: &mut World) {
    let controls = world.resource::<PendingOverrides>().0.clone();
    let (merged, preset_rows) = working_copy(world, &controls);
    // Nothing a capture wrote can turn up here: `resolve_capture` asks
    // `Rebind::checked_with_preset` before it stores anything, and turns the press down on screen.
    // What is left is a row this build no longer agrees with — a saved file naming a control that
    // has since moved or gone — so the console is the right place for it, since the player cannot
    // act on it either way.
    for problem in apply_overrides_with_preset(world, &merged, &preset_rows) {
        warn!("`{}` was not applied: {:?}", problem.mapping, problem.kind);
    }
    saved_controls::store(world, &controls);
    world.resource_mut::<AppliedControls>().0 = controls;
    world
        .resource_mut::<NextState<Settings>>()
        .set(Settings::Hidden);
}

/// Every preset this game offers.
///
/// Built fresh against the world rather than declared once, the same way [`start_capture`] resolves
/// a row: a `MappingKey` cannot be built outside the crate, so `Preset::build` is what asks the
/// world what `Turn`'s gamepad row actually is, the same way `add_context` asks it what `Turn`
/// itself is.
///
/// Here rather than beside the preset buttons, because a saved choice names a preset whether or not
/// the build offers the buttons to pick one.
pub(crate) fn presets(world: &World) -> Vec<Preset> {
    vec![
        Preset {
            name: DEFAULT_PRESET,
            rows: Overrides::new(),
        },
        Preset::build(world, "disasteroids.southpaw", |southpaw| {
            southpaw.bind::<Turn>(
                DeviceFamily::Gamepad,
                [Control::GamepadAxis(GamepadAxis::RightStickX)],
            );
            // A preset is not only rebound controls: southpaw asks for a looser dead zone on the
            // stick it just moved turning onto.
            southpaw.tune(
                DeviceFamily::Gamepad,
                TURN_DEAD_ZONE_KEY,
                TunableValue::Range {
                    value: 0.25,
                    min: 0.0,
                    max: 0.5,
                },
            );
        }),
    ]
}

/// The row named `family` and `key`, if any mapping in the list is.
fn row_named(
    rows: &[ActionMapping],
    family: DeviceFamily,
    key: MappingKey,
) -> Option<&ActionMapping> {
    rows.iter()
        .find(|row| row.family == family && row.key == key)
}

/// The whole screen, around a column for each device family the build lists.
///
/// A build spawns it on entering [`Settings::Showing`]:
///
/// ```ignore
/// bsn! {
///     @ControlsScreen {
///         @columns: bsn_list! {
///             @MappingColumn { @family: DeviceFamily::KeyboardMouse }
///             --
///             @MappingColumn {
///                 @family: DeviceFamily::Gamepad,
///                 @below: {pad_presets::remapping()},
///             }
///         },
///     }
/// }
/// ```
///
/// The root carries [`Menu`] and the observers for its actions, which is the arrangement
/// [`actions::shell`](crate::actions::shell) already uses for the always-on controls. Here it buys
/// something the shell does not need: the context is the screen, so there is no activation
/// condition to write and nothing to switch off on the way out.
#[derive(SceneComponent, Default, Clone)]
#[scene(ControlsScreenProps)]
pub(crate) struct ControlsScreen;

/// What a build puts on the [`ControlsScreen`].
pub(crate) struct ControlsScreenProps {
    /// Laid out side by side, left to right: ordinarily one [`MappingColumn`] per family.
    pub columns: Box<dyn SceneList>,
}

impl Default for ControlsScreenProps {
    fn default() -> Self {
        Self {
            columns: Box::new(bsn_list! {}),
        }
    }
}

impl ControlsScreen {
    fn scene(props: ControlsScreenProps) -> impl Scene {
        bsn! {
            // Closing the screen is nothing but despawning it, which the state can do on its own —
            // and that takes the context below with it.
            #Settings
            DespawnOnExit::<Settings>(Settings::Showing)
            Menu
            on(navigate)
            on(back)
            on(confirm)
            on(clear_cell)
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
                row_gap: Val::Px(14.0),
            }
            Children [
                Text::new("CONTROLS")
                TextFont { font_size: 27.0_f32 }
                TextColor(TITLE)
                --
                Node { column_gap: Val::Px(48.0), align_items: AlignItems::Start }
                Children [
                    {props.columns}
                ]
                --
                Node { column_gap: Val::Px(16.0), margin: {UiRect::top(Val::Px(4.0))} }
                Children [
                    // Cancel first in the tree as well as on screen, so that the one the selection
                    // starts on is also the one the eye starts on — which is also why Reset is not
                    // first, since a selection landing on it would put the destructive one under
                    // the player's thumb before they had read the screen. Left to right they run by
                    // increasing commitment: leave, change everything back, commit.
                    @{cancel_button()}
                    --
                    @{reset_button()}
                    --
                    @{confirm_button()}
                ]
                --
                // The one thing on this screen that has to know an action. A span rather than a
                // lookup formatted into the sentence, because the answer changes while the screen
                // is up, as the player rebinds.
                Text::new(
                    "Boxed cells are the ones this game offers for rebinding — press one, then \
                     press what you want bound there; everything else is listed so you can see \
                     what it does.\nPress "
                )
                // Three spans rather than one sentence with the controls written into it, so they
                // follow the player's own rebinding while the screen is up.
                Node {
                    margin: UiRect::axes(percent(10), px(0))
                }
                TextFont { font_size: 13.0_f32 }
                TextColor(FIXED)
                Children [
                    PromptSpan(Clear)
                    TextFont { font_size: 13.0_f32 }
                    TextColor(TITLE)
                    --
                    TextSpan::new(" to empty the selected cell, or ")
                    TextFont { font_size: 13.0_f32 }
                    TextColor(FIXED)
                    --
                    PromptSpan(ToggleSettings)
                    TextFont { font_size: 13.0_f32 }
                    TextColor(TITLE)
                    --
                    TextSpan::new(" to close.")
                    TextFont { font_size: 13.0_f32 }
                    TextColor(FIXED)
                ]
                --
                // Empty until a press is turned down. A line that is always present, rather than
                // one spawned and despawned, so nothing below it moves when a refusal appears.
                Text::new("")
                RefusalLine
                Node {
                    margin: UiRect::axes(percent(10), px(0))
                }
                TextFont { font_size: 13.0_f32 }
                TextColor(REFUSED_TEXT)
            ]
        }
    }
}

/// Discards the working copy and leaves. Where the selection starts, since it is the one action a
/// player who opened this screen by accident is guaranteed to want.
///
/// The caption carries its own shortcut — `PromptFamily(Gamepad)` because a keyboard player reads
/// [`Back`]'s own binding (`Escape`) off the row it is bound on, and the pad has no such row here.
fn cancel_button() -> impl Scene {
    bsn! {
        #Cancel
        Button
        on(cancel_pressed)
        AutoFocus
        @focusable()
        BorderColor::all(FIXED)
        Node {
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(16.0), Val::Px(4.0))},
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
        }
        Children [
            Text::new("Cancel")
            TextFont { font_size: 15.0_f32 }
            TextColor(TITLE)
            --
            IconPrompt(Back)
            ~{PromptFamily(DeviceFamily::Gamepad)}
            TextFont { font_size: 15.0_f32 }
            TextColor(TITLE)
            Node { height: {Val::Px(BUTTON_ICON)} }
        ]
    }
}

/// Applies the working copy to the running game and leaves.
fn confirm_button() -> impl Scene {
    bsn! {
        #Confirm
        Button
        on(confirm_pressed)
        @focusable()
        BorderColor::all(FIXED)
        Node {
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(16.0), Val::Px(4.0))},
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
        }
        Children [
            Text::new("Confirm")
            TextFont { font_size: 15.0_f32 }
            TextColor(TITLE)
            --
            IconPrompt(Confirm)
            ~{PromptFamily(DeviceFamily::Gamepad)}
            TextFont { font_size: 15.0_f32 }
            TextColor(TITLE)
            Node { height: {Val::Px(BUTTON_ICON)} }
        ]
    }
}

/// Puts every row, every tunable and the preset back to what the game declared.
///
/// No caption shortcut, unlike the two beside it: Cancel and Confirm answer a pad button of their
/// own, and this is reached by selecting it and pressing A, the same as a preset button. A control
/// that resets everything is not one to put a single press away.
fn reset_button() -> impl Scene {
    bsn! {
        #Reset
        Button
        on(reset_pressed)
        @focusable()
        BorderColor::all(FIXED)
        // Laid out as the other two are, with no icon, so that stretched to their height it centres
        // its caption as they do.
        Node {
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(16.0), Val::Px(4.0))},
            align_items: AlignItems::Center,
        }
        Children [
            Text::new("Reset")
            TextFont { font_size: 15.0_f32 }
            TextColor(TITLE)
        ]
    }
}

/// A mouse click or `Enter` on a focused Cancel — the pad's B reaches the same outcome through
/// [`back`] instead, since B also has the mid-capture meaning Cancel does not.
fn cancel_pressed(_: On<Activate>, mut next: ResMut<NextState<Settings>>) {
    next.set(Settings::Hidden);
}

/// Empties the working copy, which is what "everything the game declared" is: the default preset
/// names no rows, and no captures lie over it.
///
/// It needs no confirmation of its own: Cancel still walks away from it, and nothing has reached
/// the running game until Confirm.
fn reset_pressed(_: On<Activate>, mut pending: ResMut<PendingOverrides>) {
    pending.0 = Controls::default();
}

/// A mouse click or `Enter` on a focused Confirm — the pad's X reaches the same outcome through
/// [`confirm`] instead.
fn confirm_pressed(_: On<Activate>, mut commands: Commands) {
    commands.queue(apply_and_close);
}

/// Rewrites every row to match the pending working copy.
///
/// Every one of them rather than only the row a capture or a preset press actually named: a steal
/// can move any row, and a preset can move several at once, so asking "which one" buys nothing a
/// full pass does not already answer just as cheaply. What keeps that affordable is the run
/// condition this is registered with: it does not run at all on a frame where
/// [`PendingOverrides`] did not change.
///
/// Exclusive, because it reads every tagged cell in the table alongside the mapping list and the
/// pending copy all at once. Whatever fills the slot under the pad table redraws itself.
fn redraw_pending(world: &mut World) {
    let live = mappings(world);
    let pending = pending_copy(world);

    let mut principals = world.query::<(&RowCell, &mut Text)>();
    for (cell, mut text) in principals.iter_mut(world) {
        let Some(row) = row_named(&live, cell.0, cell.1) else {
            continue;
        };
        *text = Text::new(
            pending
                .slots_of(row)
                .get(cell.2)
                .cloned()
                .flatten()
                .map_or_else(String::new, |slot| cell_text(slot.control, &slot.with)),
        );
    }

    let mut followers = world.query::<(&FollowerCell, &mut Text)>();
    for (cell, mut text) in followers.iter_mut(world) {
        let Some(row) = row_named(&live, cell.0, cell.1) else {
            continue;
        };
        *text = Text::new(
            pending
                .slots_of(row)
                .get(cell.2)
                .cloned()
                .flatten()
                .map_or_else(String::new, |slot| {
                    cell.3.fallback_format(&cell_text(slot.control, &slot.with))
                }),
        );
    }
}

/// One device family's table, with whatever the build lays out under it.
#[derive(SceneComponent, Default, Clone)]
#[scene(MappingColumnProps)]
pub(crate) struct MappingColumn;

/// What a build puts in a [`MappingColumn`].
pub(crate) struct MappingColumnProps {
    /// The family whose rows the table lists.
    pub family: DeviceFamily,
    /// Laid out under the table: whatever remaps this family besides its cells. Empty by default.
    pub below: Box<dyn SceneList>,
}

impl Default for MappingColumnProps {
    fn default() -> Self {
        Self {
            family: DeviceFamily::KeyboardMouse,
            below: Box::new(bsn_list! {}),
        }
    }
}

impl MappingColumn {
    fn scene(props: MappingColumnProps) -> impl Scene {
        bsn! {
            Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0) }
            Children [
                @{table(props.family)}
                --
                {props.below}
            ]
        }
    }
}

/// One family's table: its heading, with the rows filled in under it by [`fill_table`] once it has
/// spawned.
fn table(family: DeviceFamily) -> impl Scene {
    // A row appears in both tables under the same key, so the family is what tells the two apart
    // for a test selecting one. Named off the family rather than the heading above it: the heading
    // is display text and may be translated, and a selector that moved with it would be a test
    // broken by a wording change.
    let (id, title) = match family {
        DeviceFamily::KeyboardMouse => ("KeyboardMouse", "Keyboard & Mouse"),
        DeviceFamily::Gamepad => ("Gamepad", "Gamepad"),
    };
    bsn! {
        ~{Name::new(id)}
        ~{MappingTable(family)}
        on(fill_table)
        Node { flex_direction: FlexDirection::Column, row_gap: {Val::Px(ROW_GAP)} }
        Children [
            Text::new(title)
            TextFont { font_size: 17.0_f32 }
            TextColor(TITLE)
            Node { margin: {UiRect::bottom(Val::Px(4.0))} }
        ]
    }
}

/// The table listing one family's rows.
#[derive(Component, Clone, Copy)]
struct MappingTable(DeviceFamily);

/// Lists the table's rows under its heading, read from the mapping list as it stands.
///
/// On [`Ready`] rather than on add, since that is when the heading exists to be listed after.
fn fill_table(ready: On<Ready>, world: &World, mut commands: Commands) {
    let Some(&MappingTable(family)) = world.get::<MappingTable>(ready.entity) else {
        return;
    };
    append_children(&mut commands, ready.entity, table_lines(world, family));
}

/// Spawns `scenes` as `parent`'s last children when `commands` is applied.
///
/// Not `queue_spawn_related_scenes`, which waits for the next scene spawn: the screen opens after
/// that has run, so the rows would miss the redraw the opening frame paints them with.
pub(crate) fn append_children(commands: &mut Commands, parent: Entity, scenes: impl SceneList) {
    commands.queue(
        move |world: &mut World| match world.spawn_scene_list(scenes) {
            Ok(children) => {
                world.entity_mut(parent).add_children(&children);
            }
            Err(error) => error!("the controls screen could not fill {parent}: {error}"),
        },
    );
}

/// One family's rows, grouped by category.
fn table_lines(world: &World, family: DeviceFamily) -> Vec<impl Scene + use<>> {
    let (columns, control_width) = match family {
        DeviceFamily::KeyboardMouse => (KEYBOARD_COLUMNS, CONTROL_WIDTH),
        DeviceFamily::Gamepad => (GAMEPAD_COLUMNS, GAMEPAD_CONTROL_WIDTH),
    };
    // `Menu`'s own bindings — the stick, the D-pad and the arrow keys that move the selection on
    // this very screen — are machinery for operating the settings screen, not controls a player
    // thinks of as part of the game. `mappings` cannot tell the two apart on its own, so this is
    // the one place the screen names a context: everything from here down still reads
    // `ActionMapping` alone. `ButtonFocused` is excluded for the same reason —
    // `common::widget_focus`'s bridge, not a control this screen's own player thinks of as
    // bindable.
    let mut rows: Vec<ActionMapping> = mappings(world)
        .into_iter()
        .filter(|mapping| {
            mapping.family == family
                && mapping.context != Menu::PATH
                && mapping.context != ButtonFocused::PATH
        })
        .collect();
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
                None,
            ));
        }
        lines.push(line(
            cells(&mapping, columns, control_width),
            0.0,
            Some(mapping.key),
        ));
        // A follower is drawn under the row it rides: indented and dimmed rather than a row of its
        // own, and not activatable — a follower is not separately rebindable, and a button that did
        // nothing would say otherwise.
        for follower in &mapping.followers {
            lines.push(line(
                follower_cells(&mapping, follower, columns, control_width),
                FOLLOWER_INDENT,
                None,
            ));
        }
    }
    lines
}

/// One control as a cell reads it, with whatever has to be held in front of it.
///
/// The same composition the shortcut captions use, and that agreement is the point: a binding the
/// game captions `Ctrl+N` has to list as `Ctrl+N` here too, or one binding is described two ways in
/// one game.
fn cell_text(control: Control, with: &[ControlOrigin]) -> String {
    let mut text = String::new();
    for held in with {
        text.push_str(&held.fallback_label());
        text.push('+');
    }
    text.push_str(&control.fallback_label());
    text
}

/// One row: what it is called, then a cell per column.
fn cells(mapping: &ActionMapping, columns: usize, control_width: f32) -> Vec<Cell> {
    let changeable = mapping.rebind_policy.is_rebindable();
    let color = if changeable { CHANGEABLE } else { FIXED };
    let mut cells = vec![Cell {
        text: label(mapping.key),
        width: NAME_WIDTH,
        color,
        border: Color::NONE,
        role: CellRole::Label,
    }];

    for column in 0..columns {
        // Every cell of a changeable row is one the player can put a control in, filled or not — an
        // empty box is what a spare secondary looks like before it is used. A fixed row shows what
        // it holds and stops: no capture will ever reach the cell after it, so a box there would be
        // a promise this screen cannot keep. Past the end of the row and emptied by the player draw
        // the same: a blank cell. Which one it is matters to the crate, not to the table.
        let held = mapping.slots.get(column).and_then(Option::as_ref);
        let filled = held.is_some();
        cells.push(Cell {
            text: held.map_or_else(String::new, |slot| cell_text(slot.control, &slot.with)),
            width: control_width,
            color,
            border: if changeable {
                CHANGEABLE.with_alpha(0.35)
            } else {
                Color::NONE
            },
            // Exactly the cells the box is drawn around, which is what the box was always
            // promising: the selection can reach what the player may change, and skips the rest. A
            // filled cell that is not changeable still gets an identity — `Fixed` rather than
            // `Label` — because a preset may still move it even though a capture never will.
            role: if changeable {
                CellRole::Changeable(mapping.family, mapping.key, column)
            } else if filled {
                CellRole::Fixed(mapping.family, mapping.key, column)
            } else {
                CellRole::Label
            },
        });
    }
    cells
}

/// One follower's line: its own name, then the principal's controls with the follower's own
/// condition formatted in — "Hold W" under "W", not the bare word "hold". A follower has no slots
/// of its own to draw; a blank column here is the row above not having filled that slot either.
fn follower_cells(
    mapping: &ActionMapping,
    follower: &Follower,
    columns: usize,
    control_width: f32,
) -> Vec<Cell> {
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
            .and_then(Option::as_ref)
            .map_or_else(String::new, |slot| {
                follower
                    .condition
                    .fallback_format(&cell_text(slot.control, &slot.with))
            });
        cells.push(Cell {
            text,
            width: control_width,
            color: SUBORDINATE,
            border: Color::NONE,
            role: CellRole::Follower(mapping.family, mapping.key, column, follower.condition),
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
    /// Drawn around the cells the player can press to rebind, and around the empty ones they can
    /// fill. [`Color::NONE`] for the rest.
    border: Color,
    role: CellRole,
}

/// What kind of thing a cell is, which is also what identifies it for [`redraw_pending`] and, where
/// a capture applies, for starting one.
///
/// `MappingKey` alone does not name a row — the same key is shared by a keyboard mapping and a
/// gamepad one for the same action (it is derived from the action's path and part, and says nothing
/// about the family), so every role that names a row carries its `DeviceFamily` too.
#[derive(Clone, Copy)]
enum CellRole {
    /// A heading, a row's name, or a follower's name. Read, never pressed, and never moved by
    /// anything this screen does.
    Label,
    /// A control the player may press to capture a new one into this slot.
    Changeable(DeviceFamily, MappingKey, usize),
    /// A control filled in but not player-capturable here: every gamepad row, since a preset rather
    /// than this screen's own capture is that table's whole remapping story. Still named, because a
    /// preset can still move it and [`redraw_pending`] has to find it again when one does.
    Fixed(DeviceFamily, MappingKey, usize),
    /// A follower's line under one column of the row above it, carrying the condition its caption
    /// is formatted with. A capture on that column has to reformat this cell too, not just the
    /// principal one.
    Follower(DeviceFamily, MappingKey, usize, ConditionDescriptor),
}

/// `indent` is nonzero for exactly a follower's line — the mark of a row that is a fact about the
/// principal above it rather than a row of its own, since every cell in it is already
/// [`SUBORDINATE`]. A parameter rather than a second function, since `table` below collects
/// ordinary and follower lines into one `Vec`.
fn line(cells: Vec<Cell>, indent: f32, id: Option<MappingKey>) -> impl Scene {
    let cells: Vec<_> = cells.into_iter().map(cell).collect();
    // Only a principal row is named, which is what a test selects a cell under. A heading and a
    // follower are read, never operated, so neither is given one.
    let name = id.map(|key| bsn! { ~{Name::new(key.to_string())} });
    bsn! {
        @name
        Node { column_gap: Val::Px(8.0), margin: {UiRect::left(Val::Px(indent))} }
        Children [
            {cells}
        ]
    }
}

fn cell(cell: Cell) -> impl Scene {
    // A changeable cell is a button and a stop for the selection; a fixed one is neither, and stays
    // the plain text it was. Three independent splices rather than branching to one of three
    // different `bsn!` blocks — each of those would be its own opaque type, and this way there is
    // exactly one `impl Scene` this function ever returns.
    //
    // `BackgroundColor` starts at `Color::NONE` here rather than being absent, so `start_capture`
    // and its two ways out (`captured`, `back`'s mid-capture branch) are all just writing the same
    // component, never inserting or removing it.
    let selectable = matches!(cell.role, CellRole::Changeable(..)).then(|| {
        bsn! {
            Button
            on(start_capture)
            on(captured)
            BackgroundColor(Color::NONE)
            @focusable()
        }
    });
    // `Option` only forwards `Scene`, not `Component`, so the tag is a one-entry `bsn!` of its own
    // rather than a bare value. `~{}` inside it: these are plain data tags with no sensible
    // `Default`, and the bare tuple-constructor form `bsn!` otherwise expects patches a `Template`,
    // which needs one.
    let rebind_tag = if let CellRole::Changeable(family, key, slot) = cell.role {
        Some(bsn! { ~{RebindCell(family, key, slot)} })
    } else {
        None
    };
    // Every principal cell gets this one, `Changeable` and `Fixed` alike — `redraw_pending` finds a
    // row's cells this way regardless of who is allowed to capture into them, since a preset moves
    // a `Fixed` row `RebindCell` was never attached to.
    let row_tag = match cell.role {
        CellRole::Changeable(family, key, slot) | CellRole::Fixed(family, key, slot) => {
            Some(bsn! { ~{RowCell(family, key, slot)} })
        }
        CellRole::Label | CellRole::Follower(..) => None,
    };
    let follower_tag = if let CellRole::Follower(family, key, slot, condition) = cell.role {
        Some(bsn! { ~{FollowerCell(family, key, slot, condition)} })
    } else {
        None
    };
    // The slot number, under the row's own name: a test says which column of which row it means,
    // and a `Fixed` cell is named as well as a changeable one, since a preset moves those and a
    // test watching a preset take effect has to read them.
    let name = match cell.role {
        CellRole::Changeable(_, _, slot) | CellRole::Fixed(_, _, slot) => {
            Some(bsn! { ~{Name::new(slot.to_string())} })
        }
        CellRole::Label | CellRole::Follower(..) => None,
    };
    // Every cell carries the same border and padding whether or not the border is visible, so the
    // columns line up down the table rather than shifting where a box begins.
    bsn! {
        @selectable
        @rebind_tag
        @row_tag
        @follower_tag
        @name
        Text({cell.text})
        TextFont { font_size: {ROW_FONT_SIZE} }
        TextColor({cell.color})
        BorderColor::all(cell.border)
        Node {
            width: {Val::Px(cell.width)},
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(3.0))},
            padding: {UiRect::axes(Val::Px(6.0), Val::Px(ROW_PADDING_V))},
        }
    }
}

/// Names the row and slot a boxed cell would capture for, so a press knows what to start and a
/// result knows which row to write into.
///
/// `DeviceFamily` first because `MappingKey` alone does not name a row — see [`CellRole`].
#[derive(Component, Clone, Copy)]
struct RebindCell(DeviceFamily, MappingKey, usize);

/// Names the row and slot a principal cell displays, whether or not it is capturable — the identity
/// [`redraw_pending`] finds any cell by. Separate from [`RebindCell`], which additionally marks "and
/// this one is interactive": every `RebindCell` is also a `RowCell`, but a `Fixed` row's cell is a
/// `RowCell` with no `RebindCell` beside it, since nothing on this screen may capture into one.
#[derive(Component, Clone, Copy)]
struct RowCell(DeviceFamily, MappingKey, usize);

/// Names the row, column and condition a follower's cell renders, so a capture on that column can
/// reformat this cell's caption along with the principal's.
#[derive(Component, Clone, Copy)]
struct FollowerCell(DeviceFamily, MappingKey, usize, ConditionDescriptor);

/// Starts listening for the control that will fill this cell, and paints it [`LISTENING`] so the
/// player can tell a capture started at all — the only signal on this screen that one has, since a
/// press otherwise looks identical to one that did nothing.
///
/// The session lives on the cell entity itself, per [`CaptureSession`]'s own recommendation — it is
/// the thing that will show the answer. `Back`'s two controls are excluded so a two-stage cancel
/// works from inside a capture: without this, pressing B here would be captured as this row's new
/// binding instead of reaching [`back`] and cancelling the capture.
fn start_capture(activate: On<Activate>, cells: Query<&RebindCell>, mut commands: Commands) {
    let Ok(&RebindCell(family, key, _)) = cells.get(activate.entity) else {
        return;
    };
    let entity = activate.entity;
    commands.queue(move |world: &mut World| {
        let Some(mapping) = mappings(world)
            .into_iter()
            .find(|row| row.family == family && row.key == key)
        else {
            return;
        };
        // Whatever the last capture had to say about, the player has moved on from.
        world.resource_mut::<Refusal>().0 = None;
        world.entity_mut(entity).insert((
            CaptureSession::for_mapping(&mapping).excluding([
                Control::PhysicalKey(KeyCode::Escape),
                Control::GamepadButton(GamepadButton::East),
            ]),
            BackgroundColor(LISTENING),
        ));
    });
}

/// Writes what was captured into the working copy, stealing the control from whatever else already
/// held it, patches every cell either row's change touched, and clears [`LISTENING`] off the cell
/// that was — the crate has already removed `CaptureSession` itself by the time this runs.
fn captured(captured: On<ControlCaptured>, cells: Query<&RebindCell>, mut commands: Commands) {
    let Ok(&RebindCell(family, key, slot)) = cells.get(captured.entity) else {
        return;
    };
    let control = captured.control;
    let entity = captured.entity;
    commands.queue(move |world: &mut World| {
        world
            .entity_mut(entity)
            .insert(BackgroundColor(Color::NONE));
        resolve_capture(world, family, key, slot, control);
    });
}

/// A captured control is stolen from whatever else already holds it, rather than being refused or
/// left to duplicate.
///
/// Every row a steal can find is guaranteed to share `family` with the row captured into: `control`
/// is itself family-specific (a key can never sit in a gamepad row's slots), so nothing outside
/// this family can ever hold it. That is what makes looking `clash.mapping` back up against
/// `family` rather than a bare key search safe, and it is also why `conflicts_pending` needs no
/// family parameter of its own.
fn resolve_capture(
    world: &mut World,
    family: DeviceFamily,
    key: MappingKey,
    slot: usize,
    control: Control,
) {
    let all = mappings(world);
    let Some(target) = all
        .iter()
        .find(|row| row.family == family && row.key == key)
        .cloned()
    else {
        return;
    };
    // Conflicts are read against the merged copy, so a control a preset moved onto a row still
    // counts as taken; the steal itself is written into the captures, since the player made it. The
    // preset's own rows come back too, because they are what the check below exempts.
    let controls = world.resource::<PendingOverrides>().0.clone();
    let (working, preset_rows) = working_copy(world, &controls);

    // The cell the player pressed is the cell that gets it, and `with_cell` grows the row to reach
    // that column if it has to — what lets the second cell be filled on a row whose first one is
    // empty, the state a steal leaves behind. The new control keeps whatever the cell was held
    // with, and that whole press is what is stolen from elsewhere.
    let mut slots = working.with_cell(&target, slot, control);
    let candidate = slots[slot].clone().expect("the cell just filled");

    // Asked before anything is stolen: a press the row cannot hold must not empty other rows on its
    // way to being turned down. Nothing below this line can be refused, which is why `confirm` has
    // no capture of its own left to report.
    if let Err(problem) = Rebind::checked_with_preset(world, &preset_rows, &target, slots.clone()) {
        world.resource_mut::<Refusal>().0 = Some(explain(problem));
        return;
    }

    let mut pending = world.resource_mut::<PendingOverrides>();
    for clash in conflicts_pending(&all, &working, &candidate, Some(key)) {
        let Some(other) = all
            .iter()
            .find(|row| row.family == family && row.key == clash.mapping)
        else {
            continue;
        };
        let mut others = working.slots_of(other);
        take_from(&mut others, &candidate);
        pending.0.captures.bind(other.family, other.key, others);
    }

    // The row steals from itself too. `conflicts_pending` answers about *other* rows, so a press
    // this row already holds in another column is invisible to the loop above, and without this the
    // player gets one press in two cells of one row.
    take_from(&mut slots, &candidate);
    slots[slot] = Some(candidate);
    pending.0.captures.bind(target.family, target.key, slots);
}

/// What a refused press is called on screen.
///
/// Empty for as long as the last thing the player did worked, which is nearly always.
#[derive(Resource, Default)]
struct Refusal(Option<String>);

/// A refusal in the words a player can act on, rather than the name of its variant.
fn explain(problem: OverrideProblemKind) -> String {
    match problem {
        OverrideProblemKind::Reserved { control } => format!(
            "{} is how you reach this screen — it cannot be bound to anything else",
            control.fallback_label()
        ),
        OverrideProblemKind::WrongFamily { control } => format!(
            "{} is the wrong kind of device for this row",
            control.fallback_label()
        ),
        OverrideProblemKind::WrongShape { control, .. } => format!(
            "{} is the wrong kind of control for what this does",
            control.fallback_label()
        ),
        OverrideProblemKind::NotRebindable => "this row is not one the game lets you change".into(),
        OverrideProblemKind::TooManyControls { limit, .. } => {
            format!("a row holds at most {limit} controls")
        }
        // The rest cannot arise from a single captured press: they are what a *saved file* can be
        // wrong about, and the screen renders the variant name rather than inventing a sentence for
        // a case a player cannot reach.
        other => format!("{other:?}"),
    }
}

/// Paints whatever [`Refusal`] currently says, and blanks the line when it is clear.
///
/// The line exists only while the screen is up, and the resource changes when it is inserted at
/// startup and again when a capture clears it — so "no line to paint" is the ordinary case here,
/// not a failure.
fn redraw_refusal(refusal: Res<Refusal>, mut text: Query<&mut Text, With<RefusalLine>>) {
    if let Ok(mut text) = text.single_mut() {
        *text = Text::new(refusal.0.clone().unwrap_or_default());
    }
}

/// The one line that shows why a press did not take.
#[derive(Component, Clone, Default)]
struct RefusalLine;

/// Empties whichever cells of a row answer the same press as `candidate`, leaving the gap where it
/// was.
///
/// Emptied in place rather than removed: taking a control out of a row must not promote that row's
/// secondary into the column the player was looking at. `bind` drops the empty again if it was the
/// last thing the row held.
fn take_from(slots: &mut [Option<BoundSlot>], candidate: &BoundSlot) {
    for held in slots {
        if held
            .as_ref()
            .is_some_and(|slot| slot.clashes_with(candidate))
        {
            *held = None;
        }
    }
}
