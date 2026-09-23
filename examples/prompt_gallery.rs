//! Every kind of prompt, on one screen, drawn as text, as icons inline in a line of text, and as
//! icons in a node of their own.
//!
//! `cargo run --example prompt_gallery`
//!
//! Each row binds one action one way — a key, a stick, a chord, a hold — and asks what fires it
//! three times: once as the words a player would read, and twice as art. Where there is no art for
//! a control, both icon columns fall back to the same words in brackets.
//!
//! An inline icon does not yet centre on the line of text around it, which is Bevy's to fix. The
//! block column is what a button or a row of hints would use, and lines up as any other node does.
//!
//! - `1` to `4` switch the pad brand the rows speak in: Xbox, PlayStation, Nintendo and Generic. No
//!   pad needs to be connected. Generic has no art for face buttons, so those rows fall back to
//!   text.
//! - `P` steps through the presets, and every row answers again under the bindings it moved to.
//!
//! Map is bound in a context the gallery itself shadows, and is named anyway: a prompt says what a
//! control does, not whether it does it this frame. Emote is bound to nothing at all, and its row
//! is empty on purpose.

#![allow(missing_docs)]

use bevy::ecs::schedule::SystemCondition;
use bevy::input::gamepad::GamepadButton;
use bevy::prelude::*;
use bevy_action_map::device::GamepadBrand;
use bevy_action_map::overrides::{Overrides, apply_overrides_with_preset};
use bevy_action_map::prelude::*;
use bevy_action_map::preset::Preset;

#[path = "common/mod.rs"]
mod common;

use common::prompt_ui::{
    self, IconPrompt, IconPromptSpan, PromptBrand, PromptFamily, PromptPick, PromptSpan,
};

const TITLE: Color = Color::srgb(0.85, 0.9, 0.92);
const LABEL: Color = Color::srgb(0.45, 0.5, 0.52);
const PROMPT: Color = Color::srgb(0.75, 0.82, 0.85);

const FONT_SIZE: f32 = 15.0;
const LABEL_WIDTH: f32 = 180.0;
const CELL_WIDTH: f32 = 220.0;
/// The height a block icon is scaled to: the size of the pre-scaled inline art, so the two columns
/// compare like for like.
const BLOCK_ICON: f32 = 25.0;

#[derive(InputAction)]
#[action(path = "prompt_gallery.interact", output = bool, intent = Button)]
struct Interact;

#[derive(InputAction)]
#[action(path = "prompt_gallery.fire", output = bool, intent = Button)]
struct Fire;

#[derive(InputAction)]
#[action(path = "prompt_gallery.jump", output = bool, intent = Button)]
struct Jump;

#[derive(InputAction)]
#[action(path = "prompt_gallery.next_weapon", output = bool, intent = Button)]
struct NextWeapon;

#[derive(InputAction)]
#[action(path = "prompt_gallery.throttle", output = f32, intent = Analog1)]
struct Throttle;

#[derive(InputAction)]
#[action(path = "prompt_gallery.look", output = Vec2, intent = Directional2)]
struct Look;

#[derive(InputAction)]
#[action(path = "prompt_gallery.walk", output = Vec2, intent = Directional2)]
struct Walk;

#[derive(InputAction)]
#[action(path = "prompt_gallery.quick_save", output = bool, intent = Button)]
struct QuickSave;

#[derive(InputAction)]
#[action(path = "prompt_gallery.ultimate", output = bool, intent = Button)]
struct Ultimate;

#[derive(InputAction)]
#[action(path = "prompt_gallery.reload", output = bool, intent = Button)]
struct Reload;

#[derive(InputAction)]
#[action(path = "prompt_gallery.dodge", output = bool, intent = Button)]
struct Dodge;

#[derive(InputAction)]
#[action(path = "prompt_gallery.map", output = bool, intent = Button)]
struct Map;

#[derive(InputAction)]
#[action(path = "prompt_gallery.emote", output = bool, intent = Button)]
struct Emote;

#[derive(InputAction)]
#[action(path = "prompt_gallery.show_xbox", output = bool, intent = Button)]
struct ShowXbox;

#[derive(InputAction)]
#[action(path = "prompt_gallery.show_playstation", output = bool, intent = Button)]
struct ShowPlayStation;

#[derive(InputAction)]
#[action(path = "prompt_gallery.show_nintendo", output = bool, intent = Button)]
struct ShowNintendo;

#[derive(InputAction)]
#[action(path = "prompt_gallery.show_generic", output = bool, intent = Button)]
struct ShowGeneric;

#[derive(InputAction)]
#[action(path = "prompt_gallery.next_preset", output = bool, intent = Button)]
struct NextPreset;

/// The bindings the rows describe. Nothing listens to them; they are here to be asked about.
///
/// Above [`Browse`], so its exclusivity does not reach them.
#[derive(InputContext)]
#[context(path = "prompt_gallery.sheet", tick = Render, priority = 20)]
struct Sheet;

/// The gallery's own keys.
///
/// `exclusive`, the way a menu over a game is, so that [`Underneath`] is shadowed for as long as
/// the gallery is up.
#[derive(InputContext)]
#[context(path = "prompt_gallery.browse", tick = Render, priority = 10, exclusive)]
struct Browse;

/// A game under the gallery, shadowed for as long as the gallery is up.
#[derive(InputContext)]
#[context(path = "prompt_gallery.underneath", tick = Render)]
struct Underneath;

/// The preset in effect, as its index in [`presets`] and the name it goes by.
#[derive(Resource)]
struct SelectedPreset {
    index: usize,
    name: &'static str,
}

/// The line naming the brand and preset the rows are drawn under.
#[derive(Component, Default, Clone)]
struct Status;

/// The name of the preset that leaves every binding as declared.
const DECLARED: &str = "prompt_gallery.declared";

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "prompt gallery".into(),
                resolution: (900, 640).into(),
                ..default()
            }),
            ..default()
        }),
        common::font::plugin,
        ActionMapPlugin,
        prompt_ui::plugin,
        bevy_remote_driver::RemoteDriverPlugin,
    ))
    // Every span below names its own family, so there is no primary device to state.
    .insert_resource(PromptDevice(None))
    .insert_resource(PromptBrand(GamepadBrand::Xbox))
    .insert_resource(SelectedPreset {
        index: 0,
        name: DECLARED,
    });

    app.add_context::<Sheet>(|controls| {
        controls.bind::<Interact>(KeyCode::KeyE);
        controls.bind::<Fire>(MouseButton::Left);
        controls.bind::<Jump>(GamepadButton::South);
        controls.bind::<NextWeapon>(GamepadButton::RightTrigger);
        controls.bind::<Throttle>(GamepadButton::RightTrigger2);
        controls.bind::<Look>(Stick::Right);
        controls.bind::<Walk>(DirectionalButtons::wasd());
        controls
            .bind::<QuickSave>(KeyCode::KeyS)
            .with(ModifierKey::Ctrl);
        controls
            .bind::<Ultimate>(GamepadButton::RightTrigger)
            .with(GamepadButton::LeftTrigger);
        controls.bind::<Reload>(GamepadButton::West).hold(0.5);
        controls.bind::<Dodge>(KeyCode::Space).multi_tap(2, 0.3);
    });

    app.add_context::<Browse>(|controls| {
        controls.bind::<ShowXbox>(KeyCode::Digit1);
        controls.bind::<ShowPlayStation>(KeyCode::Digit2);
        controls.bind::<ShowNintendo>(KeyCode::Digit3);
        controls.bind::<ShowGeneric>(KeyCode::Digit4);
        controls.bind::<NextPreset>(KeyCode::KeyP);
    });

    app.add_context::<Underneath>(|controls| {
        controls.bind::<Map>(KeyCode::KeyM);
    });

    app.add_systems(
        Startup,
        (camera.spawn(), gallery.spawn(), underneath.spawn()),
    )
    .add_systems(
        Update,
        show_status
            .run_if(resource_changed::<PromptBrand>.or_else(resource_changed::<SelectedPreset>)),
    )
    .run();
}

fn camera() -> impl Scene {
    bsn! { Camera2d }
}

fn underneath() -> impl Scene {
    bsn! { Underneath }
}

/// The screen, which is also the entity carrying the gallery's two contexts.
fn gallery() -> impl Scene {
    use DeviceFamily::{Gamepad, KeyboardMouse};

    let rows = vec![
        row("Key", Interact::id(), KeyboardMouse, 1),
        row("Mouse button", Fire::id(), KeyboardMouse, 1),
        row("Face button", Jump::id(), Gamepad, 1),
        row("Bumper", NextWeapon::id(), Gamepad, 1),
        row("Trigger", Throttle::id(), Gamepad, 1),
        row("Stick", Look::id(), Gamepad, 1),
        // One prompt per direction, so four spans rather than one.
        row("Composite", Walk::id(), KeyboardMouse, 4),
        row("Keyboard chord", QuickSave::id(), KeyboardMouse, 1),
        row("Pad chord", Ultimate::id(), Gamepad, 1),
        row("Hold", Reload::id(), Gamepad, 1),
        row("Double-tap", Dodge::id(), KeyboardMouse, 1),
        // Shadowed by `Browse`, and named all the same, as a hint over a paused game would be.
        row("Shadowed", Map::id(), KeyboardMouse, 1),
        row("Unbound", Emote::id(), KeyboardMouse, 1),
    ];

    bsn! {
        Sheet
        Browse
        on(show::<ShowXbox>(GamepadBrand::Xbox))
        on(show::<ShowPlayStation>(GamepadBrand::PlayStation))
        on(show::<ShowNintendo>(GamepadBrand::Nintendo))
        on(show::<ShowGeneric>(GamepadBrand::Generic))
        on(next_preset)
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(10.0),
            padding: {UiRect::top(Val::Px(24.0))},
        }
        Children [
            Text::new("PROMPTS")
            TextFont { font_size: 24.0_f32 }
            TextColor(TITLE)
            --
            Status
            Text::new("")
            TextFont { font_size: {FONT_SIZE} }
            TextColor(PROMPT)
            --
            Text::new("1-4 switch brand   P next preset")
            TextFont { font_size: 13.0_f32 }
            TextColor(LABEL)
            --
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                margin: {UiRect::top(Val::Px(8.0))},
            }
            Children [
                {rows}
            ]
        ]
    }
}

/// One way of binding an action: its name, then what fires it as text, as inline icons and as block
/// icons.
///
/// `parts` is how many prompts the action answers with at once, which is one for everything but a
/// composite.
fn row(label: &'static str, action: ActionId, family: DeviceFamily, parts: u8) -> impl Scene {
    let words: Vec<_> = (0..parts).map(|n| word(action, family, n)).collect();
    let icons: Vec<_> = (0..parts).map(|n| icon(action, family, n)).collect();
    let blocks: Vec<_> = (0..parts).map(|n| block(action, family, n)).collect();
    bsn! {
        Node { align_items: AlignItems::Center }
        Children [
            Text({label.to_string()})
            TextFont { font_size: {FONT_SIZE} }
            TextColor(LABEL)
            Node { width: {Val::Px(LABEL_WIDTH)} }
            --
            Node { width: {Val::Px(CELL_WIDTH)}, column_gap: Val::Px(8.0) }
            Children [ {words} ]
            --
            Node { width: {Val::Px(CELL_WIDTH)}, column_gap: Val::Px(8.0), align_items: AlignItems::Center }
            Children [ {icons} ]
            --
            Node { width: {Val::Px(CELL_WIDTH)}, column_gap: Val::Px(8.0), align_items: AlignItems::Center }
            Children [ {blocks} ]
        ]
    }
}

fn word(action: ActionId, family: DeviceFamily, n: u8) -> impl Scene {
    bsn! {
        Text
        Children [
            PromptSpan({action})
            ~{PromptFamily(family)}
            ~{PromptPick::Nth(n)}
            TextFont { font_size: {FONT_SIZE} }
            TextColor(PROMPT)
        ]
    }
}

fn icon(action: ActionId, family: DeviceFamily, n: u8) -> impl Scene {
    bsn! {
        Text
        Children [
            IconPromptSpan({action})
            ~{PromptFamily(family)}
            ~{PromptPick::Nth(n)}
            TextFont { font_size: {FONT_SIZE} }
            TextColor(PROMPT)
        ]
    }
}

fn block(action: ActionId, family: DeviceFamily, n: u8) -> impl Scene {
    bsn! {
        IconPrompt({action})
        ~{PromptFamily(family)}
        ~{PromptPick::Nth(n)}
        TextFont { font_size: {FONT_SIZE} }
        TextColor(PROMPT)
        Node { height: {Val::Px(BLOCK_ICON)} }
    }
}

/// An observer that switches the rows to one brand whenever its action fires.
fn show<A: InputAction>(brand: GamepadBrand) -> impl Fn(On<Fired<A>>, Commands) + Clone {
    move |_, mut commands| {
        commands.insert_resource(PromptBrand(brand));
        PromptGeneration::invalidate(&mut commands);
    }
}

/// Applies the preset after the one in effect, wrapping around.
///
/// Nothing here redraws a row: applying the preset is a change the prompts are already told about.
fn next_preset(_: On<Fired<NextPreset>>, mut commands: Commands) {
    commands.queue(|world: &mut World| {
        let presets = presets(world);
        let index = (world.resource::<SelectedPreset>().index + 1) % presets.len();
        let preset = &presets[index];
        for problem in apply_overrides_with_preset(world, &preset.rows, &preset.rows) {
            warn!("`{}` was not applied: {:?}", problem.mapping, problem.kind);
        }
        *world.resource_mut::<SelectedPreset>() = SelectedPreset {
            index,
            name: preset.name,
        };
    });
}

/// The bindings as declared, and a preset moving every row a preset can reach.
///
/// Every row but the composite, whose four parts are four mappings and so have to be named one at a
/// time, and the two empty ones.
fn presets(world: &World) -> Vec<Preset> {
    use DeviceFamily::{Gamepad, KeyboardMouse};

    vec![
        Preset {
            name: DECLARED,
            rows: Overrides::new(),
        },
        Preset::build(world, "prompt_gallery.remapped", |remapped| {
            remapped
                .bind::<Interact>(KeyboardMouse, [Control::PhysicalKey(KeyCode::KeyF)])
                .bind::<Fire>(KeyboardMouse, [Control::MouseButton(MouseButton::Right)])
                .bind::<Jump>(Gamepad, [Control::GamepadButton(GamepadButton::East)])
                .bind::<NextWeapon>(
                    Gamepad,
                    [Control::GamepadButton(GamepadButton::LeftTrigger)],
                )
                .bind::<Throttle>(
                    Gamepad,
                    [Control::GamepadButton(GamepadButton::LeftTrigger2)],
                )
                .bind::<Look>(Gamepad, [Control::GamepadStick(Stick::Left)])
                .bind::<QuickSave>(
                    KeyboardMouse,
                    [BoundSlot {
                        control: Control::PhysicalKey(KeyCode::KeyS),
                        with: vec![ControlOrigin::Modifier(ModifierKey::Alt)],
                    }],
                )
                .bind::<Ultimate>(
                    Gamepad,
                    [BoundSlot {
                        control: Control::GamepadButton(GamepadButton::RightTrigger2),
                        with: vec![ControlOrigin::Ours(Control::GamepadButton(
                            GamepadButton::LeftTrigger2,
                        ))],
                    }],
                )
                .bind::<Reload>(Gamepad, [Control::GamepadButton(GamepadButton::North)])
                .bind::<Dodge>(KeyboardMouse, [Control::PhysicalKey(KeyCode::KeyQ)]);
        }),
    ]
}

fn show_status(
    brand: Res<PromptBrand>,
    preset: Res<SelectedPreset>,
    mut status: Single<&mut Text, With<Status>>,
) {
    status.0 = format!("{}   {}", brand.0, preset.name);
}
