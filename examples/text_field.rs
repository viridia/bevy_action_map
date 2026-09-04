//! A focused text field beside a live gameplay context.
//!
//! Run it: `cargo run --example text_field`.
//!
//! `ControlClass::CharacterProducing` (chunk 25) shipped with no caller — `bind_characters` (chunk
//! 82, its replacement) had never had one either, until now. Two contexts are active at once:
//! `TextField` claims every character-producing key with `bind_characters` and consumes it;
//! `OnFoot`, at the default (lower) priority, binds the same `Space` to `Jump`.
//!
//! Type some letters into the box — they land in it the ordinary way, through
//! `EditableText::queue_edit`. Then press Space on its own: it lands in the field as a space
//! character, and the console stays silent about `Jump`. That is the whole test — a key a text
//! field and a gameplay action both want, decided by consumption rather than by the app checking
//! which one has focus.
//!
//! `InputDispatchPlugin` is disabled, the same call Disasteroids makes and for the same reason
//! (`docs/issues.md` 2.3): left enabled, `bevy_ui_widgets`' own text-input handling reads a bubbled
//! `FocusedInput<KeyboardInput>` that never asks the mapper anything, so Space would land in the
//! field *and* still reach `OnFoot` — the exact bypass this example exists to rule out. With it
//! disabled, `EditableText` is a plain data structure with nothing feeding it but the observers
//! below, which turn `ClassFired<TypedCharacter>` and the two editing actions into `TextEdit`s.
//!
//! The arrow keys drive `Move` in `OnFoot` and are untouched by any of this: they carry no text, so
//! `bind_characters` never sees them, and typing does not stop the player moving.

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
            Text::new(
                "Type here. Space goes into the field, not to Jump.\n\
                 Arrow keys still move the player — watch the console.",
            ),
            TextFont {
                font_size: 16.0.into(),
                ..default()
            },
        ))
        .id();

    let field = commands
        .spawn((
            TextField,
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
            TextCursorStyle::default(),
        ))
        .observe(append_character)
        .observe(backspace)
        .observe(submit)
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

fn move_player(input: Actions<OnFoot>, mut position: Local<Vec2>) {
    let movement = input.value::<Move>();
    if movement != Vec2::ZERO {
        *position += movement;
        info!("position: {position:?}");
    }
}

fn jump(_: On<Fired<Jump>>) {
    info!("Jump!");
}
