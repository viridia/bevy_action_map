//! The presentation layer, which lives under `examples/` until it can be a crate — see the module
//! doc there for why. It is pulled in by path so that it is tested rather than merely compiled: an
//! example only proves it builds, and every question below is about what a span ends up saying.
#![allow(missing_docs)]

use bevy::prelude::*;
use bevy_action_map::prelude::*;

#[path = "../examples/common/prompt_ui.rs"]
mod prompt_ui;

use prompt_ui::{IconPromptSpan, PromptClass, PromptFamily, PromptPick, PromptSpan, PromptUnbound};

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
        bevy::asset::AssetPlugin::default(),
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
    app.insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)));
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
    app.insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)));
    app.add_context::<Flying>(|controls| {
        controls.bind::<Jump>(KeyCode::Space);
        controls.bind::<Jump>(GamepadButton::South);
    });
    app.world_mut().spawn(Flying);

    let keyboard = app.world_mut().spawn(PromptSpan(Jump::id())).id();
    let pad = app
        .world_mut()
        .spawn((PromptSpan(Jump::id()), PromptFamily(DeviceFamily::Gamepad)))
        .id();

    assert_eq!(caption(&mut app, keyboard), "Space");
    assert_eq!(caption(&mut app, pad), "A");
}

/// A gamepad button is named in the pad's own words, and an unrecognized pad is named in Xbox's.
///
/// Guessing Xbox for an unrecognized pad is a presentation choice, and one this layer makes rather
/// than the crate — see `prompt_ui::labelling_brand`.
#[test]
fn a_gamepad_button_is_named_in_its_pads_own_words() {
    use bevy_action_map::device::{Brand, GamepadBrand};

    for (connected, expected) in [
        (None, "A"),
        (Some(GamepadBrand::Generic), "A"),
        (Some(GamepadBrand::Xbox), "A"),
        (Some(GamepadBrand::PlayStation), "Cross"),
        // Nintendo's face buttons sit mirrored, so physical South is B there — the prompt names
        // the button the player is looking at rather than the one in the same place on a Xbox pad.
        (Some(GamepadBrand::Nintendo), "B"),
    ] {
        let mut app = app();
        app.insert_resource(PromptDevice(Some(DeviceFamily::Gamepad)));
        app.add_context::<Flying>(|controls| {
            controls.bind::<Jump>(GamepadButton::South);
        });
        app.world_mut().spawn(Flying);
        if let Some(brand) = connected {
            app.world_mut().spawn(Brand(brand));
        }

        let span = app.world_mut().spawn(PromptSpan(Jump::id())).id();
        assert_eq!(caption(&mut app, span), expected, "brand {connected:?}");
    }
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
    app.insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)));
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
    app.insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)));

    let bare = app.world_mut().spawn(PromptSpan(Jump::id())).id();
    let told = app
        .world_mut()
        .spawn((PromptSpan(Jump::id()), PromptUnbound("unbound".to_string())))
        .id();

    assert_eq!(caption(&mut app, bare), "—");
    assert_eq!(caption(&mut app, told), "unbound");
}

/// A binding that only fires held says so in the caption, and as the whole formula rather than a
/// bare qualifier — "Hold Space" tells a player what to do; "Hold" alone tells them nothing they
/// could act on.
#[test]
fn a_held_binding_says_so_in_the_caption() {
    let mut app = app();
    app.insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)));
    app.add_context::<Flying>(|controls| {
        controls.bind::<Jump>(KeyCode::Space).hold(0.5);
    });
    app.world_mut().spawn(Flying);

    let span = app.world_mut().spawn(PromptSpan(Jump::id())).id();
    assert_eq!(caption(&mut app, span), "Hold Space");
}

/// The whole reason the crate raises a staleness signal: a prompt that was right when it was
/// spawned has to stop being wrong on its own.
#[test]
fn a_span_catches_up_when_the_answer_moves() {
    #[derive(Resource)]
    struct Flies;

    let mut app = app();
    app.insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)));
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

/// No art beats a blank caption: `IconPromptSpan` falls back to the same text `PromptSpan` would
/// show, bracketed.
#[test]
fn an_icon_prompt_falls_back_to_bracketed_text_when_nothing_fires_the_action() {
    let mut app = app();
    app.insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)));

    let span = app.world_mut().spawn(IconPromptSpan(Jump::id())).id();
    assert_eq!(caption(&mut app, span), "[—]");
}
