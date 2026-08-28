//! A widget's *kind*, and the bridge that presses whichever one has focus.
//!
//! `bevy_ui_widgets` already activates a focused `Button` from a mouse click and from a
//! `FocusedInput<KeyboardInput>` its own `InputDispatchPlugin` dispatches — the keyboard half of
//! R8.4's "a focused widget claims controls" is built by somebody else. What is missing is the
//! gamepad half, and more generally the association between "a control on some device" and "what
//! kind of widget currently has focus" (R22.9): neither crate may depend on the other, so a game
//! depending on both is the only place that association can live.
//!
//! This module is that bridge, kept general enough to answer for any widget kind rather than only
//! `Button` — a candidate for eventually living in `bevy_ui_widgets` itself rather than here, see
//! <https://github.com/bevyengine/bevy/issues/25592>. Because it covers the keyboard as well as
//! the pad, this game disables `InputDispatchPlugin` entirely rather than split the seam between
//! two mechanisms that would otherwise both be reaching for the same keys.

use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate as WidgetActivate, Button};
use bevy_action_map::prelude::*;
use bevy_input::{gamepad::GamepadButton, keyboard::KeyCode};

/// A stable, well-known identifier for a widget's *kind*.
///
/// A plain string rather than matching on `Button` itself: R22.9 asks for a neutral identifier a
/// widget "would plausibly have anyway", not a fact about this one crate pairing, and a string is
/// the shape it names. [`plugin`] registers [`WidgetKind::BUTTON`] as a required component of
/// `Button`, so nothing that spawns one has to remember to tag it by hand.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct WidgetKind(pub &'static str);

impl WidgetKind {
    /// What every `Button` is tagged with, via a required component ([`plugin`]).
    pub const BUTTON: WidgetKind = WidgetKind("button");
}

/// True while the focused entity is tagged `kind`.
fn focus_is(kind: WidgetKind) -> impl Fn(Res<InputFocus>, Query<&WidgetKind>) -> bool {
    move |focus, kinds| {
        focus
            .get()
            .and_then(|entity| kinds.get(entity).ok())
            .is_some_and(|found| *found == kind)
    }
}

/// Presses whatever button is focused, whichever device asked.
#[derive(InputAction)]
#[action(path = "common.widget_focus.activate", output = bool, intent = Button)]
struct Activate;

/// Live only while a button has focus. Priority 20 rather than the default: the one screen in
/// this game where a button is ever focused is [`Menu`](crate::actions::Menu), which is
/// `exclusive` at priority 10 — exclusivity shadows a context at or below its own priority, so
/// this has to outrank it to answer at all. Nothing else in the game binds `Enter`, `Space` or the
/// pad's accept button, so nothing here needs `.consume()`.
#[derive(InputContext)]
#[context(path = "common.widget_focus.button_focused", tick = Render, priority = 20)]
pub struct ButtonFocused;

fn button_focused() -> impl Scene {
    bsn! {
        ButtonFocused
        on(press_focused_button)
    }
}

/// Turns `Activate` into the event `bevy_ui_widgets` itself listens for. The mapper does not need
/// to know what kind of widget is focused here — [`ButtonFocused`]'s own activation condition
/// already established that it is a button.
fn press_focused_button(_: On<Fired<Activate>>, focus: Res<InputFocus>, mut commands: Commands) {
    if let Some(entity) = focus.get() {
        commands.trigger(WidgetActivate { entity });
    }
}

/// Wires up [`ButtonFocused`] and spawns its one, permanent instance.
pub fn plugin(app: &mut App) {
    // Required rather than a tag every `Button` spawn site adds by hand — the same reason
    // `bevy_ui_widgets` itself uses required components for `Pressed`, `InteractionDisabled` and
    // the rest: a fact about *what a widget is* should not be something a call site can forget.
    app.register_required_components_with::<Button, WidgetKind>(|| WidgetKind::BUTTON);

    app.add_context::<ButtonFocused>(|controls| {
        controls.active_if(focus_is(WidgetKind::BUTTON));
        controls.bind::<Activate>(KeyCode::Enter).press();
        controls.bind::<Activate>(KeyCode::Space).press();
        controls.bind::<Activate>(GamepadButton::South).press();
    });
    app.add_systems(Startup, button_focused.spawn());
}
