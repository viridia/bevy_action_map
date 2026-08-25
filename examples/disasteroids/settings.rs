//! The controls screen: every control the game has, and what it does.
//!
//! Press `F2` (or Y on a pad) to open it, and the same control again to close it. It is a help
//! screen and nothing more — there is no way to change a binding from here yet.
//!
//! Two tables, one per device, because a rebinding is made for the keyboard *or* for the pad and
//! never for both at once. Each is drawn from [`mappings`] alone: nothing below names an action, a
//! context or a key, so this file would work unchanged in a different game.

use bevy::prelude::*;
use bevy_action_map::mapping::fallback_label;
use bevy_action_map::prelude::*;

use crate::actions::ToggleSettings;
use crate::common::prompt_ui::PromptSpan;

/// A row the player may change, and the box drawn around such a cell.
const CHANGEABLE: Color = Color::srgb(0.75, 0.95, 0.8);
/// Everything that is listed to be read rather than changed.
const FIXED: Color = Color::srgb(0.55, 0.6, 0.62);
const HEADING: Color = Color::srgb(0.45, 0.7, 0.95);
const TITLE: Color = Color::srgb(0.9, 0.95, 1.0);

/// The width of the column holding what a row is called, and of each control column after it.
const NAME_WIDTH: f32 = 210.0;
const CONTROL_WIDTH: f32 = 155.0;

/// Whether the controls screen is up.
///
/// A state rather than a flag on a resource, for the reason [`Game`](crate::pause::Game) is one: the
/// screen is spawned by `OnEnter` and despawned by `OnExit`, and there is one fact about whether it
/// is showing rather than a screen and a flag that have to agree.
///
/// The game keeps running behind it, and still hears the controls: the panel covers the field, but
/// nothing here takes the throttle away from the ship. Standing the flying context down would be one
/// way to fix that, and the better one is a context for the screen itself — which is what a screen
/// that can be operated will need anyway, and what this one gets when it grows buttons.
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

/// The whole screen, as a scene.
///
/// [`mappings`] hands back every row the game has declared, in both schemes. Splitting them into two
/// tables and sorting each is the screen's business, which is why the crate does not do it.
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
        // Closing the screen is nothing but despawning it, which the state can do on its own.
        DespawnOnExit::<Settings>(Settings::Showing)
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
            lines.push(line(vec![Cell {
                // A category is a localization key on the same terms as a row's name, so a game
                // with a catalogue looks it up here exactly as `label` does below.
                text: mapping
                    .category
                    .map_or_else(|| String::from("Other"), fallback_label),
                width: NAME_WIDTH,
                color: HEADING,
                border: Color::NONE,
            }]));
        }
        lines.push(line(cells(&mapping, columns)));
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
        });
    }
    cells
}

/// What a mapping is called on screen.
///
/// The crate hands over a localization key rather than words, because that half of a row is as
/// translatable as the control beside it. A shipped game looks the key up in its catalogue; Dead
/// Zone has none, and answers for the two keys whose derived text is not what a player should read.
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
}

fn line(cells: Vec<Cell>) -> impl Scene {
    let cells: Vec<_> = cells.into_iter().map(cell).collect();
    bsn! {
        Node { column_gap: Val::Px(8.0) }
        Children [{cells}]
    }
}

fn cell(cell: Cell) -> impl Scene {
    // Every cell carries the same border and padding whether or not the border is visible, so the
    // columns line up down the table rather than shifting where a box begins.
    bsn! {
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
