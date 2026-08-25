//! The presentation layer, which lives under `examples/` until it can be a crate — see the module
//! doc there for why. It is pulled in by path so that it is tested rather than merely compiled: an
//! example only proves it builds, and every question below is about what a span ends up saying.
#![allow(missing_docs)]

use bevy::prelude::*;
use bevy_action_map::prelude::*;

#[path = "../examples/common/prompt_ui.rs"]
mod prompt_ui;

use prompt_ui::{PromptClass, PromptPick, PromptScheme, PromptSpan, PromptUnbound};

#[derive(InputAction)]
#[action(path = "prompt_ui_tests.jump", output = bool, intent = Button)]
struct Jump;

#[derive(InputAction)]
#[action(path = "prompt_ui_tests.turn", output = f32, intent = Analog1)]
struct Turn;

#[derive(InputContext)]
#[context(path = "prompt_ui_tests.flying", tick = Render)]
struct Flying;

/// Headless: nothing here draws, and a `TextSpan` is a component whether or not anything renders
/// it. What is being tested is the string, which is the whole of what this layer decides.
fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::input::InputPlugin,
        ActionMapPlugin,
        prompt_ui::plugin,
    ));
    app
}

/// What one span says.
fn caption(app: &mut App, span: Entity) -> String {
    app.update();
    app.world()
        .get::<TextSpan>(span)
        .expect("a prompt span keeps its `TextSpan`")
        .0
        .clone()
}

#[test]
fn a_span_says_what_fires_the_action() {
    let mut app = app();
    app.insert_resource(PromptDevice(Some(Scheme::KeyboardMouse)));
    app.add_context::<Flying>(|controls| {
        controls.bind::<Jump>(KeyCode::Space);
    });
    app.world_mut().spawn(Flying);

    let span = app.world_mut().spawn(PromptSpan(Jump::id())).id();
    assert_eq!(caption(&mut app, span), "Space");
}

/// The device a bare span speaks for is the game's answer rather than ours, and a span that has its
/// own answer overrides it — which is what a settings screen's gamepad column is.
#[test]
fn a_scheme_beside_the_span_overrides_the_games_device() {
    let mut app = app();
    app.insert_resource(PromptDevice(Some(Scheme::KeyboardMouse)));
    app.add_context::<Flying>(|controls| {
        controls.bind::<Jump>(KeyCode::Space);
        controls.bind::<Jump>(GamepadButton::South);
    });
    app.world_mut().spawn(Flying);

    let keyboard = app.world_mut().spawn(PromptSpan(Jump::id())).id();
    let pad = app
        .world_mut()
        .spawn((PromptSpan(Jump::id()), PromptScheme(Scheme::Gamepad)))
        .id();

    assert_eq!(caption(&mut app, keyboard), "Space");
    assert_eq!(caption(&mut app, pad), "South Button");
}

/// A prompt with room to name a button and not a stick.
#[test]
fn a_class_beside_the_span_narrows_to_one_kind_of_control() {
    let mut app = app();
    app.insert_resource(PromptDevice(None));
    app.add_context::<Flying>(|controls| {
        controls.bind::<Turn>(bevy_action_map::binding::AxisButtons::ad());
        controls.bind::<Turn>(GamepadAxis::LeftStickX);
    });
    app.world_mut().spawn(Flying);

    let button = app
        .world_mut()
        .spawn((PromptSpan(Turn::id()), PromptClass(ControlClass::AnyButton)))
        .id();
    assert_eq!(caption(&mut app, button), "A");
}

/// Indexing what would fire the action now, which for a composite is one entry per direction. The
/// test says so because the name does not: this is not the secondary column.
#[test]
fn a_pick_takes_the_one_after_the_first() {
    let mut app = app();
    app.insert_resource(PromptDevice(Some(Scheme::KeyboardMouse)));
    app.add_context::<Flying>(|controls| {
        controls.bind::<Turn>(bevy_action_map::binding::AxisButtons::ad());
    });
    app.world_mut().spawn(Flying);

    let second = app
        .world_mut()
        .spawn((PromptSpan(Turn::id()), PromptPick::Nth(1)))
        .id();
    assert_eq!(caption(&mut app, second), "D");
}

/// Nothing bound is a real answer, and the em dash is what a sentence with a hole in it needs.
#[test]
fn an_action_nothing_fires_renders_a_placeholder() {
    let mut app = app();
    app.insert_resource(PromptDevice(Some(Scheme::KeyboardMouse)));

    let bare = app.world_mut().spawn(PromptSpan(Jump::id())).id();
    let told = app
        .world_mut()
        .spawn((PromptSpan(Jump::id()), PromptUnbound("unbound".to_string())))
        .id();

    assert_eq!(caption(&mut app, bare), "—");
    assert_eq!(caption(&mut app, told), "unbound");
}

/// R18.5, which is the whole reason the crate raises a signal: a prompt that was right when it was
/// spawned has to stop being wrong on its own.
#[test]
fn a_span_catches_up_when_the_answer_moves() {
    #[derive(Resource)]
    struct Flies;

    let mut app = app();
    app.insert_resource(PromptDevice(Some(Scheme::KeyboardMouse)));
    app.add_context::<Flying>(|controls| {
        controls.bind::<Jump>(KeyCode::Space);
        controls.active_if(resource_exists::<Flies>);
    });
    app.world_mut().spawn(Flying);

    let span = app.world_mut().spawn(PromptSpan(Jump::id())).id();
    // A context switched off fires nothing, so there is nothing to press and the span says so.
    assert_eq!(caption(&mut app, span), "—");

    app.insert_resource(Flies);
    assert_eq!(caption(&mut app, span), "Space");

    app.world_mut().remove_resource::<Flies>();
    assert_eq!(caption(&mut app, span), "—");
}
