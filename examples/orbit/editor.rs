//! The controls panel: a tree with a row per mapping, and under each row the keys that drive it, as
//! a list the player lengthens and shortens.
//!
//! Every edit is applied and saved as it is made; there is no Confirm. A row's list is rebuilt from
//! [`mappings`] after each one, so the panel shows what the game reads.
//!
//! A list here is not a table of primary and secondary columns, so removing a key closes the gap:
//! the keys after it move up a place, and `Overrides::bind` is handed the list with its gaps
//! flattened out. A capture listening on a row that loses a key is cancelled, since the slot it was
//! going to fill has moved.
//!
//! Each key has a toggle per modifier, lit when the key needs that modifier held. Capturing a new
//! key keeps the modifiers: the key button sets the key and the toggles set the modifiers, so
//! recapturing changes only what the player pressed.

use bevy::ecs::system::Command;
use bevy::feathers::FeathersPlugins;
use bevy::feathers::controls::{
    ButtonVariant, FeathersButton, FeathersTreeItem, FeathersTreeItemChildren, FeathersTreeView,
};
use bevy::feathers::dark_theme::create_dark_theme;
use bevy::feathers::display::caption;
use bevy::feathers::theme::UiTheme;
use bevy::prelude::*;
use bevy::settings::{ReflectSettingsGroup, SaveSettings, SettingsGroup, SettingsPlugin};
use bevy::ui::Expanded;
use bevy::ui_widgets::{
    Activate, TreeItemActivate, TreeItemExpandChange, tree_view_expand_self_update,
    tree_view_self_update,
};
use bevy_action_map::overrides::{
    OverrideProblemKind, Overrides, Rebind, SavedOverrides, apply_overrides, resolve_saved,
    save_overrides,
};
use bevy_action_map::prelude::*;

use crate::{MouseOwner, Status, owner_text};

/// Reverse-DNS, per `SettingsPlugin`. Names the directory the settings file lives in.
const APP_ID: &str = "org.viridia.bevy_action_map.orbit";

/// The mouse's buttons stay with the panel while a capture listens, so the player can click the
/// capture button again to cancel, or click somewhere else entirely.
const MOUSE_BUTTONS: [Control; 3] = [
    Control::MouseButton(MouseButton::Left),
    Control::MouseButton(MouseButton::Right),
    Control::MouseButton(MouseButton::Middle),
];

/// What the panel keeps between sessions.
#[derive(Resource, SettingsGroup, Reflect, Default, Debug)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "controls")]
struct ControlSettings {
    overrides: SavedOverrides,
}

/// Everything the player has changed, which is what gets applied and saved.
#[derive(Resource, Default)]
struct Edits(Overrides);

/// A tree row standing for one mapping. A capture for the row listens on this entity, since the
/// buttons under it are rebuilt as the list changes.
#[derive(Component, Clone)]
struct EditorRow(MappingKey);

/// Which slot the capture on this row will fill.
#[derive(Component, Clone, Copy)]
struct Listening(usize);

/// What a button under a row does to that row's list.
#[derive(Component, Clone, Copy)]
struct SlotButton(MappingKey, Edit);

#[derive(Clone, Copy)]
enum Edit {
    /// Listens for a key to put in this slot, or stops listening if it already was.
    Capture(usize),
    /// Takes this slot out, closing the gap.
    Remove(usize),
    /// Listens for a key to put after the last one.
    Add,
    /// Adds this modifier to the slot's chord, or takes it out.
    Toggle(usize, ModifierKey),
}

const MODIFIERS: [ModifierKey; 4] = [
    ModifierKey::Ctrl,
    ModifierKey::Shift,
    ModifierKey::Alt,
    ModifierKey::Super,
];

/// A modifier as this platform's keyboard prints it.
fn modifier_label(modifier: ModifierKey) -> &'static str {
    match modifier {
        ModifierKey::Ctrl => "Ctrl",
        ModifierKey::Shift => "Shift",
        ModifierKey::Alt if cfg!(target_os = "macos") => "Opt",
        ModifierKey::Alt => "Alt",
        ModifierKey::Super if cfg!(target_os = "macos") => "Cmd",
        ModifierKey::Super if cfg!(target_os = "windows") => "Win",
        ModifierKey::Super => "Super",
    }
}

pub fn plugin(app: &mut App) {
    // Registration has to precede `SettingsPlugin`, which reads the type registry once, when it is
    // built.
    app.register_type::<ControlSettings>();
    app.add_plugins((FeathersPlugins, SettingsPlugin::new(APP_ID)))
        .insert_resource(UiTheme(create_dark_theme()))
        .init_resource::<Edits>()
        .add_observer(tree_view_self_update)
        .add_observer(tree_view_expand_self_update)
        .add_observer(toggle_on_activate)
        .add_observer(captured);
}

/// Puts last session's bindings back. In `Startup`, since resolving a saved row needs every
/// context declared.
pub fn load(world: &mut World) {
    let saved = world.resource::<ControlSettings>().overrides.clone();
    let declared = declared_mappings(world);
    let tunables = declared_tunables(world);
    let edits = match resolve_saved(&saved, &declared, &tunables) {
        Ok((edits, problems, unresolved)) => {
            for problem in problems {
                warn!("saved binding could not be applied: {problem:?}");
            }
            for row in unresolved {
                warn!("saved binding names nothing this build declares: {row:?}");
            }
            edits
        }
        Err(version) => {
            warn!("settings file is from a newer build, starting on the defaults: {version:?}");
            return;
        }
    };
    for problem in apply_overrides(world, &edits) {
        warn!("saved binding could not be applied: {problem:?}");
    }
    world.resource_mut::<Edits>().0 = edits;
}

/// The tree, with a row per mapping, and a button that puts every row back. A rebindable row's
/// list is filled by [`fill`] once the tree has spawned.
pub fn panel(mappings: &[ActionMapping]) -> impl Scene {
    let rows: Vec<_> = mappings.iter().map(row).collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_height: px(0),
            row_gap: px(8),
        }
        Children [
            #Editor
            @FeathersTreeView {
                @rows: {Box::new(rows) as Box<dyn SceneList>},
            }
            Node {
                flex_grow: 1.0,
                min_height: px(0),
            }
            --
            @FeathersButton
            #Reset
            on(reset)
            Children [ @caption("Reset to defaults") ]
        ]
    }
}

fn row(mapping: &ActionMapping) -> impl Scene {
    let key = mapping.key;
    let title = key.fallback_label();
    let rebindable = mapping.rebind_policy == RebindPolicy::Here;
    // A row the player cannot change says what drives it, in place of a list.
    let fixed = if rebindable {
        String::new()
    } else {
        controls_text(mapping)
    };
    bsn! {
        @FeathersTreeItem {
            @expandable: {rebindable},
            @label: bsn_list! {
                @caption({title})
                #title
                Node { flex_grow: 1.0 }
                --
                @caption({fixed})
            },
        }
        ~{Name::new(key.to_string())}
        ~{EditorRow(key)}
    }
}

fn controls_text(mapping: &ActionMapping) -> String {
    let labels: Vec<String> = mapping
        .controls()
        .into_iter()
        .flatten()
        .map(|control| control.fallback_label().into_owned())
        .collect();
    labels.join(", ")
}

/// Fills every rebindable row's list. Called once the tree has spawned.
pub fn fill(world: &mut World) {
    let rows: Vec<Entity> = world
        .query_filtered::<Entity, With<EditorRow>>()
        .iter(world)
        .collect();
    for row in rows {
        rebuild(world, row);
    }
}

/// Replaces a row's list with one line per slot as the row now stands, then the line that grows
/// it: the add button, or a line listening for the key that will go there.
fn rebuild(world: &mut World, row: Entity) {
    let key = world.get::<EditorRow>(row).expect("an editor row").0;
    let listening = world.get::<Listening>(row).map(|listening| listening.0);
    let Some(mapping) = find(world, key) else {
        return;
    };
    if mapping.rebind_policy != RebindPolicy::Here {
        return;
    }
    let container = world
        .get::<Children>(row)
        .into_iter()
        .flatten()
        .copied()
        .find(|child| world.get::<FeathersTreeItemChildren>(*child).is_some())
        .expect("a tree row has a container for its children");
    world.entity_mut(container).despawn_related::<Children>();

    let mut lines: Vec<(usize, String, Option<Vec<ModifierKey>>)> = mapping
        .slots
        .iter()
        .enumerate()
        .map(|(slot, bound)| {
            let text = match (listening == Some(slot), bound) {
                (true, _) => "Press a key".to_string(),
                (false, Some(bound)) => bound.control.fallback_label().into_owned(),
                (false, None) => "Empty".to_string(),
            };
            let held = bound.as_ref().map(held_modifiers).unwrap_or_default();
            (slot, text, Some(held))
        })
        .collect();
    let end = mapping.slots.len();
    // A slot still waiting for its first key has nothing to hold a modifier for.
    if listening == Some(end) {
        lines.push((end, "Press a key".into(), None));
    }

    for (slot, text, held) in lines {
        world
            .spawn_scene(slot_line(key, slot, text, held))
            .expect("a slot line spawns")
            .insert(ChildOf(container));
    }
    if listening != Some(end) {
        world
            .spawn_scene(add_line(key, "Add a key".into()))
            .expect("the add line spawns")
            .insert(ChildOf(container));
    }
}

fn held_modifiers(bound: &BoundSlot) -> Vec<ModifierKey> {
    bound
        .with
        .iter()
        .filter_map(|entry| match entry {
            ControlOrigin::Modifier(modifier) => Some(*modifier),
            _ => None,
        })
        .collect()
}

/// One slot: a button showing its key that listens for a new one, its modifier toggles, and a
/// button that removes it.
fn slot_line(
    key: MappingKey,
    slot: usize,
    text: String,
    held: Option<Vec<ModifierKey>>,
) -> impl Scene {
    let toggles: Vec<_> = held
        .map(|held| {
            MODIFIERS
                .iter()
                .map(|&modifier| modifier_toggle(key, slot, modifier, held.contains(&modifier)))
                .collect()
        })
        .unwrap_or_default();
    bsn! {
        @FeathersTreeItem {
            @label: bsn_list! {
                @FeathersButton
                ~{SlotButton(key, Edit::Capture(slot))}
                ~{Name::new(slot.to_string())}
                on(edit)
                Node { flex_grow: 1.0 }
                Children [ @caption({text}) #label ]
                --
                Node { column_gap: px(2) }
                Children [ {toggles} ]
                --
                @FeathersButton
                ~{SlotButton(key, Edit::Remove(slot))}
                ~{Name::new(format!("remove {slot}"))}
                on(edit)
                Children [ @caption("x") ]
            },
        }
    }
}

/// Named by the modifier's own name rather than its label, which differs by platform.
fn modifier_toggle(key: MappingKey, slot: usize, modifier: ModifierKey, lit: bool) -> impl Scene {
    let label = modifier_label(modifier);
    bsn! {
        @FeathersButton {
            @variant: {ButtonVariant::selected(lit)},
        }
        ~{SlotButton(key, Edit::Toggle(slot, modifier))}
        ~{Name::new(format!("{modifier:?} {slot}"))}
        on(edit)
        Children [ @caption(label) ]
    }
}

fn add_line(key: MappingKey, text: String) -> impl Scene {
    bsn! {
        @FeathersTreeItem {
            @label: bsn_list! {
                @FeathersButton
                ~{SlotButton(key, Edit::Add)}
                #add
                on(edit)
                Node { flex_grow: 1.0 }
                Children [ @caption({text}) ]
            },
        }
    }
}

/// Clicking a row's title opens or closes it, as the chevron does.
fn toggle_on_activate(
    activate: On<TreeItemActivate>,
    rows: Query<Has<Expanded>, With<EditorRow>>,
    mut commands: Commands,
) {
    if let Ok(expanded) = rows.get(activate.item) {
        commands.trigger(TreeItemExpandChange {
            tree: activate.tree,
            item: activate.item,
            expanded: !expanded,
        });
    }
}

fn edit(activate: On<Activate>, buttons: Query<&SlotButton>, mut commands: Commands) {
    let Ok(&SlotButton(key, edit)) = buttons.get(activate.entity) else {
        return;
    };
    commands.queue(move |world: &mut World| apply_edit(world, key, edit));
}

fn apply_edit(world: &mut World, key: MappingKey, edit: Edit) {
    let Some(row) = row_entity(world, key) else {
        return;
    };
    let Some(mapping) = find(world, key) else {
        return;
    };
    match edit {
        Edit::Capture(slot) => listen(world, row, &mapping, slot),
        Edit::Add => listen(world, row, &mapping, mapping.slots.len()),
        Edit::Remove(slot) => {
            stop_listening(world, row);
            let mut slots = world.resource::<Edits>().0.slots_of(&mapping);
            if slot < slots.len() {
                slots.remove(slot);
                world.resource_mut::<Edits>().0.bind(
                    mapping.family,
                    key,
                    slots.into_iter().flatten(),
                );
                commit(world);
            }
            rebuild(world, row);
        }
        Edit::Toggle(slot, modifier) => {
            let mut slots = world.resource::<Edits>().0.slots_of(&mapping);
            if let Some(Some(bound)) = slots.get_mut(slot) {
                let entry = ControlOrigin::Modifier(modifier);
                match bound.with.iter().position(|held| *held == entry) {
                    Some(index) => {
                        bound.with.remove(index);
                    }
                    None => bound.with.push(entry),
                }
                write_checked(world, &mapping, slots);
            }
            rebuild(world, row);
        }
    }
}

/// Starts a capture for one slot of a row, ending any other first. Pressing the button that is
/// already listening stops it instead.
fn listen(world: &mut World, row: Entity, mapping: &ActionMapping, slot: usize) {
    let already = world
        .get::<Listening>(row)
        .is_some_and(|listening| listening.0 == slot);
    let others: Vec<Entity> = world
        .query_filtered::<Entity, With<Listening>>()
        .iter(world)
        .collect();
    for other in others {
        stop_listening(world, other);
        rebuild(world, other);
    }
    if !already {
        world.entity_mut(row).insert((
            CaptureSession::for_mapping(mapping).excluding(MOUSE_BUTTONS),
            Listening(slot),
        ));
    }
    rebuild(world, row);
}

fn stop_listening(world: &mut World, row: Entity) {
    world
        .entity_mut(row)
        .remove::<(CaptureSession, Listening)>();
}

fn captured(captured: On<ControlCaptured>, mut commands: Commands) {
    let row = captured.entity;
    let control = captured.control;
    commands.queue(move |world: &mut World| fill_slot(world, row, control));
}

/// Puts the captured key in the slot its row was listening for, or says why it cannot go there.
fn fill_slot(world: &mut World, row: Entity, control: Control) {
    let Some(Listening(slot)) = world.entity_mut(row).take::<Listening>() else {
        return;
    };
    let key = world.get::<EditorRow>(row).expect("an editor row").0;
    let Some(mapping) = find(world, key) else {
        return;
    };
    // A filled slot keeps what it was held with, so the modifiers survive a new key.
    let slots = world
        .resource::<Edits>()
        .0
        .with_cell(&mapping, slot, control);
    write_checked(world, &mapping, slots);
    rebuild(world, row);
}

/// Writes a row's new slots and commits them, or says on the status line why the row refused. A
/// write that succeeds clears the last refusal, which is about an edit the player has moved on from.
fn write_checked(world: &mut World, mapping: &ActionMapping, slots: Vec<Option<BoundSlot>>) {
    let message = match Rebind::checked(world, mapping, slots) {
        Ok(rebind) => {
            rebind.write(&mut world.resource_mut::<Edits>().0);
            commit(world);
            owner_text(**world.resource::<State<MouseOwner>>())
        }
        Err(problem) => explain(problem),
    };
    let mut status = world.query_filtered::<&mut Text, With<Status>>();
    if let Ok(mut text) = status.single_mut(world) {
        text.0 = message;
    }
}

fn reset(_activate: On<Activate>, mut commands: Commands) {
    commands.queue(|world: &mut World| {
        let rows: Vec<Entity> = world
            .query_filtered::<Entity, With<EditorRow>>()
            .iter(world)
            .collect();
        for row in rows {
            stop_listening(world, row);
        }
        world.resource_mut::<Edits>().0 = Overrides::default();
        commit(world);
        fill(world);
    });
}

/// Applies the edits to the running game and writes them to the settings file.
fn commit(world: &mut World) {
    let edits = world.resource::<Edits>().0.clone();
    for problem in apply_overrides(world, &edits) {
        warn!("binding could not be applied: {problem:?}");
    }
    world.resource_mut::<ControlSettings>().overrides = save_overrides(&edits);
    SaveSettings::IfChanged.apply(world);
}

/// A refusal in words a player can act on.
fn explain(problem: OverrideProblemKind) -> String {
    match problem {
        OverrideProblemKind::Reserved { control } => {
            format!(
                "{} gives the mouse back, so it stays as it is",
                control.fallback_label()
            )
        }
        OverrideProblemKind::WrongFamily { control }
        | OverrideProblemKind::WrongShape { control, .. } => {
            format!("{} is not a key", control.fallback_label())
        }
        _ => "That cannot go there".into(),
    }
}

fn find(world: &World, key: MappingKey) -> Option<ActionMapping> {
    mappings(world)
        .into_iter()
        .find(|mapping| mapping.family == DeviceFamily::KeyboardMouse && mapping.key == key)
}

fn row_entity(world: &mut World, key: MappingKey) -> Option<Entity> {
    world
        .query::<(Entity, &EditorRow)>()
        .iter(world)
        .find(|(_, row)| row.0 == key)
        .map(|(entity, _)| entity)
}
