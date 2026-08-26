//! The controls screen: every control the game has, and what it does.
//!
//! Press `F2` (or Y on a pad) to open it, escape or B to leave it. It can be operated end to end
//! from a gamepad without touching the keyboard — the stick and the D-pad move the selection, A
//! presses what is selected — and nothing rebinds yet: pressing a control cell does nothing at all.
//!
//! Two tables, one per device, because a rebinding is made for the keyboard *or* for the pad and
//! never for both at once. Each is drawn from [`mappings`] alone: nothing below names an action, a
//! context or a key, so this file would work unchanged in a different game.
//!
//! # How the selection moves
//!
//! There are no navigation links anywhere in this file. Every focusable thing carries
//! [`AutoDirectionalNavigation`], and Bevy scores the candidates by where they are on screen — so
//! the table's own layout is the navigation graph, and a row added to it joins that graph without
//! anything being told. The one placement decision is [`AutoFocus`] on Cancel, which saves the
//! screen from having to name a first cell and then keep that name true as the table changes.

use bevy::input_focus::{AutoFocus, FocusGained, FocusLost, InputFocus};
use bevy::math::CompassOctant;
use bevy::prelude::*;
use bevy::ui::auto_directional_navigation::{AutoDirectionalNavigation, AutoDirectionalNavigator};
use bevy::ui_widgets::{Activate, Button};
use bevy_action_map::mapping::fallback_label;
use bevy_action_map::prelude::*;

use crate::actions::{Accept, Back, Menu, Navigate, ToggleSettings};
use crate::common::prompt_ui::PromptSpan;

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

/// The width of the column holding what a row is called, and of each control column after it.
const NAME_WIDTH: f32 = 210.0;
const CONTROL_WIDTH: f32 = 155.0;
/// How far a follower's line sits under the row it rides.
const FOLLOWER_INDENT: f32 = 20.0;

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

pub fn plugin(app: &mut App) {
    app.init_state::<Settings>();
    // Nothing the screen shows can change while it is up, so it is built once when it opens. Once a
    // row can be rebound, that is what stops being true.
    app.add_systems(OnEnter(Settings::Showing), show);
    app.add_systems(OnExit(Settings::Showing), release_focus);
}

/// Opens the screen, and closes it again.
///
/// Attached to the shell context's entity by [`actions::shell`](crate::actions::shell), like the
/// other controls the player can always reach.
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

/// Presses whatever is selected, on a device the widget layer cannot hear.
///
/// The bridge R22.9 says has to exist and has to be written by a third party. `bevy_ui_widgets`
/// does not depend on this crate and this crate does not depend on it; the game depends on both,
/// and so the game is the only place allowed to know that A on a pad means what `Enter` means to a
/// button. Four lines, and everything downstream of the [`Activate`] is identical whichever device
/// pressed it.
///
/// It answers only for the pad, because the keyboard half already works —
/// [`Swallowed`](crate::actions::Swallowed) records why that is a seam rather than a saving.
pub(crate) fn accept(_: On<Fired<Accept>>, focus: Res<InputFocus>, mut commands: Commands) {
    if let Some(entity) = focus.get() {
        commands.trigger(Activate { entity });
    }
}

/// Leaves the screen without changing anything.
pub(crate) fn back(_: On<Fired<Back>>, mut next: ResMut<NextState<Settings>>) {
    next.set(Settings::Hidden);
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

/// The whole screen, as a scene.
///
/// [`mappings`] hands back every row the game has declared, in both schemes. Splitting them into two
/// tables and sorting each is the screen's business, which is why the crate does not do it.
///
/// The root carries [`Menu`] and the observers for its three actions, which is the arrangement
/// [`actions::shell`](crate::actions::shell) already uses for the always-on controls. Here it buys
/// something the shell does not need: the context is the screen, so there is no activation
/// condition to write and nothing to switch off on the way out.
fn screen(world: &World) -> impl Scene {
    let all = mappings(world);
    let rows = |scheme| -> Vec<Mapping> {
        all.iter()
            .filter(|mapping| mapping.scheme == scheme)
            .cloned()
            .collect()
    };

    // One list of two, rather than two children: the tables are the same kind of thing, and a
    // `Vec` of scenes is the scene list a `Children` block wants.
    let tables = vec![
        table("Keyboard & Mouse", rows(Scheme::KeyboardMouse)),
        table("Gamepad", rows(Scheme::Gamepad)),
    ];

    bsn! {
        // Closing the screen is nothing but despawning it, which the state can do on its own — and
        // that takes the context below with it.
        DespawnOnExit::<Settings>(Settings::Showing)
        Menu
        on(navigate)
        on(accept)
        on(back)
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
                Children [{tables}]
            ),
            (
                Node { column_gap: Val::Px(16.0), margin: {UiRect::top(Val::Px(6.0))} }
                Children [
                    // Cancel first in the tree as well as on screen, so that the one the selection
                    // starts on is also the one the eye starts on.
                    ({button("Cancel", true)}),
                    ({button("Confirm", false)}),
                ]
            ),
            // The one thing on this screen that has to know an action. A span rather than a
            // lookup formatted into the sentence: the question is what would fire it *now*, so
            // the answer skips a context that is switched off and a control something else has
            // taken — and it changes while the screen is up, once this screen can rebind.
            (
                Text::new(
                    "Boxed cells are the ones this game offers for rebinding; everything else \
                     is listed so you can see what it does.\nPress "
                )
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

/// One of the two buttons at the bottom.
///
/// Neither does anything but leave, because nothing on this screen can be changed yet and so there
/// is nothing for Confirm to commit that Cancel would have discarded. They differ in what they will
/// do rather than in what they do, and the screen is worth having as two buttons now because the
/// selection has to have somewhere to start.
fn button(caption: &'static str, focused_first: bool) -> impl Scene {
    let start_here = focused_first.then(|| bsn! { AutoFocus });
    bsn! {
        Button
        on(dismiss)
        {start_here}
        focusable()
        Text::new(caption)
        TextFont { font_size: 16.0_f32 }
        TextColor(TITLE)
        BorderColor::all(FIXED)
        Node {
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(18.0), Val::Px(5.0))},
        }
    }
}

/// What both bottom buttons do for now, whichever device pressed them.
///
/// One observer for the mouse, the keyboard and the pad alike: a click, `Enter` on the focused
/// button and A on the pad all arrive here as the same [`Activate`], the first two from the widget
/// itself and the third from [`accept`] above.
fn dismiss(_: On<Activate>, mut next: ResMut<NextState<Settings>>) {
    next.set(Settings::Hidden);
}

/// Everything a selection can land on carries these three.
///
/// [`AutoDirectionalNavigation`] is what makes an entity a candidate; the [`Outline`] is the ring,
/// kept colourless until the selection arrives so that showing it is a colour change rather than a
/// component insertion; and the two observers are what change it.
fn focusable() -> impl Scene {
    bsn! {
        AutoDirectionalNavigation
        Outline { width: Val::Px(2.0), offset: Val::Px(2.0), color: Color::NONE }
        on(ring_on)
        on(ring_off)
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
                    changeable: false,
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
        changeable: false,
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
            changeable: changeable && exists,
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
        changeable: false,
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
            changeable: false,
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
    /// Whether the selection stops here. True for exactly the boxed cells.
    changeable: bool,
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
    // stays the plain text it was. Pressing one does nothing yet — capture is what comes next —
    // but the selection reaching it is the half that had to work first.
    let selectable = cell.changeable.then(|| bsn! { Button focusable() });
    // Every cell carries the same border and padding whether or not the border is visible, so the
    // columns line up down the table rather than shifting where a box begins.
    bsn! {
        {selectable}
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
