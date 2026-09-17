//! A focused text field beside a live gameplay context.
//!
//! Run it: `cargo run --example text_field`.
//!
//! `bind_characters` had never had a caller before this example. Three contexts are active at once:
//! `TextCommands` holds the editing shortcuts, every one of them a chord; `TextField` claims every
//! character-producing key with `bind_characters` and consumes it; `OnFoot`, at the lowest
//! priority, binds the same `Space` to `Jump` and the arrow keys to `Move`.
//!
//! Type some letters into the box — they land in it the ordinary way, through
//! `EditableText::queue_edit`. Then press Space on its own: it lands in the field as a space
//! character, and the console stays silent about `Jump`. That is the first test — a key a text
//! field and a gameplay action both want, decided by consumption rather than by the app checking
//! which one has focus.
//!
//! The arrow keys are the second, and they are where the chords show. Press Left on its own and the
//! player moves, exactly as before. Hold Shift and press Left and the selection extends instead,
//! because `TextCommands` evaluates first, fires, and consumes the arrow for that tick — so
//! `OnFoot` reads a control nobody pressed. Release Shift and the player moves again. Nothing
//! declares that hand-off; it falls out of a chord being satisfied or not.
//!
//! Hold both Ctrl and Shift and the cursor moves a whole word and selects it. Three bindings read
//! the same arrow key — one requiring Shift, one Ctrl, one both — and the longest chord takes it,
//! which is also not declared anywhere.
//!
//! Why the shortcuts live in a context of their own: a class binding yields any control an ordinary
//! binding in the *same* plan names, so `Ctrl+A` declared beside `bind_characters` would stop the
//! letter A typing at all. Split across two contexts, the class binding never sees the conflict,
//! and `Ctrl+A` reaches the field only because it consumes the key before the character binding is
//! asked.
//!
//! `InputDispatchPlugin` is disabled, the same call Disasteroids makes and for the same reason:
//! left enabled, `bevy_ui_widgets`' own text-input handling reads a bubbled
//! `FocusedInput<KeyboardInput>` that never asks the mapper anything, so Space would land in the
//! field *and* still reach `OnFoot` — the exact bypass this example exists to rule out. With it
//! disabled, `EditableText` is a plain data structure with nothing feeding it but the observers
//! below, which turn `ClassFired<TypedCharacter>` and each editing action into a `TextEdit`.

#![allow(missing_docs)]

use bevy::input_focus::{InputDispatchPlugin, InputFocus};
use bevy::prelude::*;
use bevy::text::{EditableText, TextCursorStyle, TextEdit};
use bevy::ui_widgets::TextInput;
use bevy_action_map::prelude::*;
use bevy_input::keyboard::{KeyCode, KeyboardInput};

struct TypedCharacter;

impl ClassBinding for TypedCharacter {
    const PATH: &'static str = "text_field.typed_character";
}

#[derive(InputAction)]
#[action(path = "text_field.backspace", output = bool, intent = Button)]
struct Backspace;

#[derive(InputAction)]
#[action(path = "text_field.submit", output = bool, intent = Button)]
struct Submit;

// Priority 10 over `OnFoot`'s default: the field has to evaluate first for its consuming class
// binding to take `Space` away from `Jump` this tick, not the next one.
#[derive(InputContext)]
#[context(path = "text_field.field", tick = Render, priority = 10)]
struct TextField;

/// What a modifier means on this platform, since the crate has no platform modifier yet.
///
/// macOS puts the editing commands on Command and word motion on Option; everywhere else both are
/// Control. Naming a side would be wrong for either: a player who reaches for the right-hand
/// Control expects `Ctrl+A` to work, which is what [`ModifierKey`] says and `KeyCode` cannot.
#[cfg(target_os = "macos")]
const COMMAND: ModifierKey = ModifierKey::Super;
#[cfg(not(target_os = "macos"))]
const COMMAND: ModifierKey = ModifierKey::Ctrl;
#[cfg(target_os = "macos")]
const WORD: ModifierKey = ModifierKey::Alt;
#[cfg(not(target_os = "macos"))]
const WORD: ModifierKey = ModifierKey::Ctrl;

// `TextCursorStyle`'s defaults are drawn for a light background: a slate cursor and a pale sky
// selection vanish against this window's dark one, which is no way to show off a selection. White
// text stays readable over the blue, so `selected_text_color` is left alone.
const CURSOR_COLOR: Color = Color::srgb(1.0, 1.0, 1.0);
const SELECTION_COLOR: Color = Color::srgb(0.15, 0.35, 0.75);
const UNFOCUSED_SELECTION_COLOR: Color = Color::srgb(0.24, 0.26, 0.32);

#[derive(InputAction)]
#[action(path = "text_field.select_all", output = bool, intent = Button)]
struct SelectAll;

#[derive(InputAction)]
#[action(path = "text_field.copy", output = bool, intent = Button)]
struct Copy;

#[derive(InputAction)]
#[action(path = "text_field.cut", output = bool, intent = Button)]
struct Cut;

#[derive(InputAction)]
#[action(path = "text_field.paste", output = bool, intent = Button)]
struct Paste;

#[derive(InputAction)]
#[action(path = "text_field.extend_left", output = bool, intent = Button)]
struct ExtendLeft;

#[derive(InputAction)]
#[action(path = "text_field.extend_right", output = bool, intent = Button)]
struct ExtendRight;

#[derive(InputAction)]
#[action(path = "text_field.word_left", output = bool, intent = Button)]
struct WordLeft;

#[derive(InputAction)]
#[action(path = "text_field.word_right", output = bool, intent = Button)]
struct WordRight;

#[derive(InputAction)]
#[action(path = "text_field.word_extend_left", output = bool, intent = Button)]
struct WordExtendLeft;

#[derive(InputAction)]
#[action(path = "text_field.word_extend_right", output = bool, intent = Button)]
struct WordExtendRight;

// Above `TextField`, so a chord takes its key before the class binding that would otherwise type
// it.
#[derive(InputContext)]
#[context(path = "text_field.commands", tick = Render, priority = 20)]
struct TextCommands;

#[derive(InputAction)]
#[action(path = "gameplay.move", output = Vec2, intent = Directional2)]
struct Move;

#[derive(InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
struct Jump;

#[derive(InputContext)]
#[context(path = "gameplay.on_foot", tick = Render)]
struct OnFoot;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "text_field — type, then press Space on its own".into(),
                    resolution: (480, 220).into(),
                    ..default()
                }),
                ..default()
            })
            .build()
            .disable::<InputDispatchPlugin>(),
        ActionMapPlugin,
    ));
    // Every binding here consumes: a chord that fires has to take its key from whatever reads it
    // next, which is the character class for the letters and `Move` for the arrows.
    app.add_context::<TextCommands>(|controls| {
        controls
            .bind::<SelectAll>(KeyCode::KeyA)
            .with(COMMAND)
            .consume();
        controls.bind::<Copy>(KeyCode::KeyC).with(COMMAND).consume();
        controls.bind::<Cut>(KeyCode::KeyX).with(COMMAND).consume();
        controls
            .bind::<Paste>(KeyCode::KeyV)
            .with(COMMAND)
            .consume();

        controls
            .bind::<ExtendLeft>(KeyCode::ArrowLeft)
            .with(ModifierKey::Shift)
            .consume();
        controls
            .bind::<ExtendRight>(KeyCode::ArrowRight)
            .with(ModifierKey::Shift)
            .consume();

        controls
            .bind::<WordLeft>(KeyCode::ArrowLeft)
            .with(WORD)
            .consume();
        controls
            .bind::<WordRight>(KeyCode::ArrowRight)
            .with(WORD)
            .consume();

        // Two entries, so these out-rank the single-modifier bindings above on the same key.
        controls
            .bind::<WordExtendLeft>(KeyCode::ArrowLeft)
            .with(WORD)
            .with(ModifierKey::Shift)
            .consume();
        controls
            .bind::<WordExtendRight>(KeyCode::ArrowRight)
            .with(WORD)
            .with(ModifierKey::Shift)
            .consume();
    });
    app.add_context::<TextField>(|controls| {
        controls.bind_characters::<TypedCharacter>().consume();
        controls.bind::<Backspace>(KeyCode::Backspace).consume();
        controls.bind::<Submit>(KeyCode::Enter).consume();
    });
    app.add_context::<OnFoot>(|controls| {
        controls.bind::<Move>(DirectionalButtons::arrow_keys());
        controls.bind::<Jump>(KeyCode::Space);
    });
    app.add_systems(Startup, setup);
    app.add_systems(Update, move_player);
    app.add_observer(jump);

    app.run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    let root = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let instructions = commands
        .spawn((
            // The modifier names itself rather than being spelled out twice: the same label the
            // crate would put in a prompt, so it stays right on a platform where `WORD` differs.
            Text::new(format!(
                "Type here. Space goes into the field, not to Jump.\n\
                 Arrow keys move the player — watch the console.\n\
                 Shift+arrow selects, {}+arrow moves by word.",
                ControlOrigin::Modifier(WORD).fallback_label(),
            )),
            TextFont {
                font_size: 16.0.into(),
                ..default()
            },
        ))
        .id();

    let field = commands
        .spawn((
            TextField,
            // Both contexts ride the field entity, so every action fires at the observers below.
            TextCommands,
            Node {
                width: Val::Px(280.0),
                border: UiRect::all(Val::Px(2.0)),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BorderColor::all(Color::WHITE),
            TextInput,
            EditableText {
                visible_width: Some(24.0),
                allow_newlines: false,
                ..default()
            },
            TextLayout::no_wrap(),
            TextFont {
                font_size: 18.0.into(),
                ..default()
            },
            TextCursorStyle {
                color: CURSOR_COLOR,
                selection_color: SELECTION_COLOR,
                unfocused_selection_color: UNFOCUSED_SELECTION_COLOR,
                ..default()
            },
        ))
        .observe(append_character)
        .observe(backspace)
        .observe(submit)
        .observe(queues::<SelectAll>(TextEdit::SelectAll))
        .observe(queues::<Copy>(TextEdit::Copy))
        .observe(queues::<Cut>(TextEdit::Cut))
        .observe(queues::<Paste>(TextEdit::Paste))
        .observe(queues::<ExtendLeft>(TextEdit::Left(true)))
        .observe(queues::<ExtendRight>(TextEdit::Right(true)))
        .observe(queues::<WordLeft>(TextEdit::WordLeft(false)))
        .observe(queues::<WordRight>(TextEdit::WordRight(false)))
        .observe(queues::<WordExtendLeft>(TextEdit::WordLeft(true)))
        .observe(queues::<WordExtendRight>(TextEdit::WordRight(true)))
        .id();

    commands.entity(root).add_children(&[instructions, field]);
    // Not what keeps `Space` out of `on_focused_keyboard_input` — `InputDispatchPlugin` being
    // disabled already does that. This is only so the caret and IME position track the field.
    commands.insert_resource(InputFocus::from_entity(field));

    commands.spawn(OnFoot);
}

fn append_character(fired: On<ClassFired<TypedCharacter>>, mut fields: Query<&mut EditableText>) {
    let RawEvent::Keyboard(KeyboardInput {
        text: Some(text), ..
    }) = &fired.event
    else {
        return;
    };
    if let Ok(mut field) = fields.get_mut(fired.entity) {
        field.queue_edit(TextEdit::Insert(text.clone()));
    }
}

/// An observer that queues one fixed edit whenever its action fires.
///
/// Ten shortcuts that differ only in which `TextEdit` they queue, so the observer is written once
/// and the edit is the argument.
fn queues<A: InputAction>(
    edit: TextEdit,
) -> impl Fn(On<Fired<A>>, Query<&mut EditableText>) + Clone {
    move |fired, mut fields| {
        if let Ok(mut field) = fields.get_mut(fired.entity) {
            field.queue_edit(edit.clone());
        }
    }
}

fn backspace(fired: On<Fired<Backspace>>, mut fields: Query<&mut EditableText>) {
    if let Ok(mut field) = fields.get_mut(fired.entity) {
        field.queue_edit(TextEdit::Backspace);
    }
}

fn submit(fired: On<Fired<Submit>>, mut fields: Query<&mut EditableText>) {
    if let Ok(mut field) = fields.get_mut(fired.entity) {
        info!("submitted: {}", field.value());
        field.clear();
    }
}

fn move_player(input: ContextActions<OnFoot>, mut position: Local<Vec2>) {
    let movement = input.value::<Move>();
    if movement != Vec2::ZERO {
        *position += movement;
        info!("position: {position:?}");
    }
}

fn jump(_: On<Fired<Jump>>) {
    info!("Jump!");
}
