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
///
/// Icons do load, since an icon chord is not drawn until its art has. The renderer is what usually
/// registers the PNG loader, so it is registered here by hand.
fn app() -> App {
    use bevy::image::{CompressedImageFormats, ImageLoader};

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::image::ImagePlugin::default(),
        bevy::input::InputPlugin,
        ActionMapPlugin,
        prompt_ui::plugin,
    ))
    .register_asset_loader(ImageLoader::new(CompressedImageFormats::empty()));
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

/// A prompt names the control and not how it is pressed: "Hold ⟨X⟩ to reload" is the sentence the
/// game writes around a prompt that reads "X".
#[test]
fn a_held_binding_is_named_by_its_control_alone() {
    let mut app = app();
    app.insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)));
    app.add_context::<Flying>(|controls| {
        controls.bind::<Jump>(KeyCode::Space).hold(0.5);
    });
    app.world_mut().spawn(Flying);

    let span = app.world_mut().spawn(PromptSpan(Jump::id())).id();
    assert_eq!(caption(&mut app, span), "Space");
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

/// What one icon span draws, child by child: the path of each icon's art, and the text between.
///
/// A chord's icons appear only once their art has loaded, which happens off the main thread, so
/// this updates until the span draws something: children, or a caption of its own.
fn icons(app: &mut App, span: Entity) -> Vec<String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        app.update();
        let world = app.world();
        let drawn = world
            .get::<Children>(span)
            .is_some_and(|children| !children.is_empty())
            || world
                .get::<TextSpan>(span)
                .is_some_and(|text| !text.0.is_empty());
        if drawn {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the span's art never loaded"
        );
        std::thread::yield_now();
    }
    drawn(app, span)
}

/// What one icon span draws right now, without waiting on any art.
fn drawn(app: &App, span: Entity) -> Vec<String> {
    let world = app.world();
    let Some(children) = world.get::<Children>(span) else {
        return Vec::new();
    };
    children
        .iter()
        .map(|child| match world.get::<InlineImage>(child) {
            Some(icon) => icon
                .image
                .path()
                .expect("art is asked for by path")
                .to_string(),
            None => world
                .get::<TextSpan>(child)
                .expect("a child is an icon or the text between two")
                .0
                .clone(),
        })
        .collect()
}

/// A chord drawn as icons names everything that has to be held, and draws it in the order it is
/// pressed: a player told to press S alone would save nothing.
#[test]
fn an_icon_prompt_draws_every_control_in_a_chord() {
    let mut app = app();
    app.insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)));
    app.add_context::<Flying>(|controls| {
        controls.bind::<Jump>(KeyCode::KeyS).with(ModifierKey::Ctrl);
    });
    app.world_mut().spawn(Flying);

    let span = app.world_mut().spawn(IconPromptSpan(Jump::id())).id();
    let chord = [
        "input_prompts_inline/keyboard_mouse/mod/ctrl.png",
        "+",
        "input_prompts_inline/keyboard_mouse/key/KeyS.png",
    ];
    assert_eq!(icons(&mut app, span), chord);
    assert_eq!(caption(&mut app, span), "");

    // Redrawn in place rather than added to.
    app.world_mut()
        .resource_mut::<PromptGeneration>()
        .set_changed();
    assert_eq!(icons(&mut app, span), chord);
}

/// A chord is not a keyboard thing, and a pad chord draws in the pad's own art.
#[test]
fn an_icon_prompt_draws_a_pad_chord_in_the_pads_art() {
    use bevy_action_map::device::GamepadBrand;
    use prompt_ui::PromptBrand;

    let mut app = app();
    app.insert_resource(PromptDevice(Some(DeviceFamily::Gamepad)));
    app.insert_resource(PromptBrand(GamepadBrand::PlayStation));
    app.add_context::<Flying>(|controls| {
        controls
            .bind::<Jump>(GamepadButton::RightTrigger)
            .with(GamepadButton::LeftTrigger);
    });
    app.world_mut().spawn(Flying);

    let span = app.world_mut().spawn(IconPromptSpan(Jump::id())).id();
    assert_eq!(
        icons(&mut app, span),
        [
            "input_prompts_inline/playstation/pad/LeftTrigger.png",
            "+",
            "input_prompts_inline/playstation/pad/RightTrigger.png",
        ]
    );
}

/// One control without art sends the whole chord to text: half a chord in pictures and half in
/// bracketed words reads as two answers.
#[test]
fn an_icon_prompt_falls_back_whole_when_one_control_has_no_art() {
    let mut app = app();
    app.insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)));
    app.add_context::<Flying>(|controls| {
        controls
            .bind::<Jump>(KeyCode::NumpadMultiply)
            .with(ModifierKey::Ctrl);
    });
    app.world_mut().spawn(Flying);

    let span = app.world_mut().spawn(IconPromptSpan(Jump::id())).id();
    assert_eq!(icons(&mut app, span), Vec::<String>::new());
    assert_eq!(caption(&mut app, span), "[Ctrl+Numpad *]");
}

/// A Mac labels Alt as Option and Super as Command, so its art does too.
#[test]
fn an_icon_prompt_draws_a_macs_own_modifier_keys_on_a_mac() {
    let mut app = app();
    app.insert_resource(PromptDevice(Some(DeviceFamily::KeyboardMouse)));
    app.add_context::<Flying>(|controls| {
        controls.bind::<Jump>(KeyCode::KeyQ).with(ModifierKey::Alt);
    });
    app.world_mut().spawn(Flying);

    let span = app.world_mut().spawn(IconPromptSpan(Jump::id())).id();
    let alt = if cfg!(target_os = "macos") {
        "input_prompts_inline/macos/mod/alt.png"
    } else {
        "input_prompts_inline/keyboard_mouse/mod/alt.png"
    };
    assert_eq!(
        icons(&mut app, span),
        [alt, "+", "input_prompts_inline/keyboard_mouse/key/KeyQ.png"]
    );
}

/// A new chord waits for its art with the old one still drawn, so the line is never laid out around
/// icons still loading. Loaded art reaches `Assets<Image>` no sooner than the next frame, so the
/// update that changes the answer cannot be the one that shows it.
#[test]
fn an_icon_prompt_keeps_its_old_chord_until_the_new_art_loads() {
    use bevy_action_map::device::GamepadBrand;
    use prompt_ui::PromptBrand;

    let mut app = app();
    app.insert_resource(PromptDevice(Some(DeviceFamily::Gamepad)));
    app.insert_resource(PromptBrand(GamepadBrand::Xbox));
    app.add_context::<Flying>(|controls| {
        controls
            .bind::<Jump>(GamepadButton::RightTrigger)
            .with(GamepadButton::LeftTrigger);
    });
    app.world_mut().spawn(Flying);

    let span = app.world_mut().spawn(IconPromptSpan(Jump::id())).id();
    let xbox = [
        "input_prompts_inline/xbox/pad/LeftTrigger.png",
        "+",
        "input_prompts_inline/xbox/pad/RightTrigger.png",
    ];
    assert_eq!(icons(&mut app, span), xbox);

    app.insert_resource(PromptBrand(GamepadBrand::Nintendo));
    app.world_mut()
        .resource_mut::<PromptGeneration>()
        .set_changed();
    app.update();
    assert_eq!(drawn(&app, span), xbox);

    // Once the new art is in, `icons` stops waiting as soon as anything is drawn, which the old
    // chord already is, so wait on the new one by name.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while drawn(&app, span) == xbox {
        assert!(
            std::time::Instant::now() < deadline,
            "the new art never loaded"
        );
        std::thread::yield_now();
        app.update();
    }
    assert_eq!(
        drawn(&app, span),
        [
            "input_prompts_inline/nintendo/pad/LeftTrigger.png",
            "+",
            "input_prompts_inline/nintendo/pad/RightTrigger.png",
        ]
    );
}
