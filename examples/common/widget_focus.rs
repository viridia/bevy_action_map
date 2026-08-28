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
    /// What every [`Stepper`] is tagged with, via a required component ([`plugin`]).
    pub const STEPPER: WidgetKind = WidgetKind("stepper");
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

/// Root of a widget that changes a value by discrete steps — a stepper, in the sense of a numeric
/// input with a decrement and an increment side, not a stepped/paused simulation.
///
/// Carries no value of its own. [`Adjusted`] fires at this entity when a step is taken; what the
/// step is worth, its range, and its display are entirely the observer's business — this module
/// only ever asks for one step, in one direction, from one device or another.
#[derive(Component, Clone, Copy, Default)]
pub struct Stepper;

/// Fired at a stepper's own entity when it should move by one step. `delta` is `1.0` or `-1.0`,
/// never scaled or repeated here — a held control's repeat comes from the binding's own `pulse`,
/// which already re-fires [`Adjust`] on an interval, so there is no second rate to agree with.
#[derive(EntityEvent, Clone, Copy)]
pub struct Adjusted {
    /// The stepper this step applies to.
    #[event_target]
    pub entity: Entity,
    /// `1.0` to increment, `-1.0` to decrement.
    pub delta: f32,
}

/// Moves whatever stepper is focused, whichever device or chevron asked.
#[derive(InputAction)]
#[action(path = "common.widget_focus.adjust", output = f32, intent = Analog1)]
struct Adjust;

/// How long a held `Adjust` binding waits between repeats — the stepper's own equivalent of a
/// menu's `MENU_REPEAT`, kept local rather than shared: the two have no reason to move together.
const ADJUST_REPEAT: f32 = 0.2;

/// Live only while a stepper has focus, at the same priority as [`ButtonFocused`] and for the same
/// reason: it has to outrank `Menu` (priority 10, exclusive) to answer at all.
///
/// `Adjust` claims the pad's D-pad left and right specifically, not the whole pad — `Menu`'s own
/// `Navigate` reads the D-pad as one four-button composite for its own binding, and a claimed
/// control simply reads as unactuated to a lower-priority binding, so up and down still move the
/// selection while a stepper has focus and only left/right are taken. The keyboard has no such
/// redundancy — arrow keys are `Navigate`'s only source there — so `-`/`+` stand in instead.
#[derive(InputContext)]
#[context(path = "common.widget_focus.stepper_focused", tick = Render, priority = 20)]
pub struct StepperFocused;

fn stepper_focused() -> impl Scene {
    bsn! {
        StepperFocused
        on(adjust_focused_stepper)
    }
}

fn adjust_focused_stepper(
    fired: On<Fired<Adjust>>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    if let Some(entity) = focus.get() {
        commands.trigger(Adjusted {
            entity,
            delta: fired.value,
        });
    }
}

/// A chevron pressed — `bevy_ui_widgets::Activate`'s own doing, whether that was a mouse click or
/// `ButtonFocused` pressing a focused chevron. Finds the stepper by parentage rather than carrying
/// its entity: the chevron is spawned as the stepper's own direct child in the same scene.
///
/// Does not claim focus itself — a pointer press already does, the moment it lands, through
/// whatever intercepts `bevy_input_focus`'s own `AcquireFocus` for a focusable ancestor (a game's
/// own bridge for a non-`TabIndex` navigation scheme, or `acquire_focus_tab_index` for one that
/// uses `TabIndex`). By the time `Activate` fires here, the stepper is already focused.
fn chevron_pressed(
    pressed: Entity,
    delta: f32,
    parents: &Query<&ChildOf>,
    commands: &mut Commands,
) {
    if let Ok(parent) = parents.get(pressed) {
        commands.trigger(Adjusted {
            entity: parent.parent(),
            delta,
        });
    }
}

/// The decrement chevron's half of [`chevron_pressed`].
pub fn decrement_pressed(
    pressed: On<WidgetActivate>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    chevron_pressed(pressed.entity, -1.0, &parents, &mut commands);
}

/// The increment chevron's half of [`chevron_pressed`].
pub fn increment_pressed(
    pressed: On<WidgetActivate>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    chevron_pressed(pressed.entity, 1.0, &parents, &mut commands);
}

/// Wires up [`ButtonFocused`] and [`StepperFocused`], and spawns their one, permanent instance
/// each.
pub fn plugin(app: &mut App) {
    // Required rather than a tag every spawn site adds by hand — the same reason `bevy_ui_widgets`
    // itself uses required components for `Pressed`, `InteractionDisabled` and the rest: a fact
    // about *what a widget is* should not be something a call site can forget.
    app.register_required_components_with::<Button, WidgetKind>(|| WidgetKind::BUTTON);
    app.register_required_components_with::<Stepper, WidgetKind>(|| WidgetKind::STEPPER);

    app.add_context::<ButtonFocused>(|controls| {
        controls.active_if(focus_is(WidgetKind::BUTTON));
        controls.bind::<Activate>(KeyCode::Enter).press();
        controls.bind::<Activate>(KeyCode::Space).press();
        controls.bind::<Activate>(GamepadButton::South).press();
    });
    app.add_context::<StepperFocused>(|controls| {
        controls.active_if(focus_is(WidgetKind::STEPPER));
        controls
            .bind::<Adjust>(AxisButtons::new(KeyCode::Minus, KeyCode::Equal))
            .pulse(ADJUST_REPEAT)
            .consume();
        controls
            .bind::<Adjust>(AxisButtons::new(
                GamepadButton::DPadLeft,
                GamepadButton::DPadRight,
            ))
            .pulse(ADJUST_REPEAT)
            .consume();
    });
    app.add_systems(Startup, (button_focused.spawn(), stepper_focused.spawn()));
}
