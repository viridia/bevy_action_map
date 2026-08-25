//! Prompts as text spans: "Press ⟨whatever fires Thrust⟩ to thrust", with nothing in the template
//! naming a control.
//!
//! **This is not one example's code, and it is not meant to stay here.** It is the presentation
//! layer, and it lives under `examples/` because of where its dependencies sit: `TextSpan` is
//! `bevy_text` and `Text` is `bevy_ui`, and `bevy_ui` already depends on `bevy_input` and
//! `bevy_input_focus`. A mapping crate that depended on it would invert that, and would foreclose
//! `bevy_ui` ever using action maps itself. So `bevy_action_map` keeps the lookup and the staleness
//! signal, which cost nothing, and everything that draws lives out here until it can be a crate of
//! its own. The gate on promoting it is Bevy deciding to take the crate upstream, which is the
//! point at which the workspace has to be arranged properly anyway.
//!
//! # What is here
//!
//! [`PromptSpan`] names an action and fills in its own string. The companions beside it narrow the
//! answer — which device, which kind of control, which of several — and each is a separate
//! component rather than a field, so that a template says which question it is asking and so that
//! a new narrowing is additive. A span with no companions renders the strongest control that would
//! fire the action on the device the game speaks for.
//!
//! # Why one control
//!
//! A span shows one, and joining several is the app's business. Whether two controls read as
//! "W / Up", "W or Up" or as two table cells is a question about the screen they sit on, and a
//! game that wants all of them has [`Prompts`] and a `join`. The load-bearing half is that a
//! prompt is a hint rather than a manual: "Press W to thrust" is the sentence, and "Press W or Up
//! Arrow to thrust" is a worse one even where both are true.

use bevy::ecs::schedule::SystemCondition;
use bevy::prelude::*;
use bevy::ui::UiSystems;
use bevy_action_map::prelude::*;

/// Renders the control that would currently fire an action.
///
/// The string is filled in for you, and rewritten when it stops being true — when a binding
/// changes, when the context holding it switches on or off, or when nothing carries that context
/// any more.
#[derive(Component, Clone, Copy, Default)]
#[require(TextSpan)]
pub struct PromptSpan(pub ActionId);

/// Which device family one span speaks for, overriding [`PromptDevice`].
///
/// What a settings screen's gamepad column wants: those rows name pad controls whatever the rest
/// of the game's prompts speak for.
#[derive(Component, Clone, Copy)]
pub struct PromptScheme(pub Scheme);

/// Narrows to one kind of control, for a prompt with room to name a button and not a stick.
#[derive(Component, Clone, Copy)]
pub struct PromptClass(pub ControlClass);

/// Which one, where several controls fire the action.
///
/// **Not the settings screen's primary and secondary.** This indexes what would fire the action
/// *now*: consumption has already removed whatever a stronger context took, and a composite
/// answers once per direction, so the second entry here is as likely to be "the key that turns the
/// other way" as it is to be a second binding. The declared columns are [`mappings`]' business.
#[derive(Component, Clone, Copy, Default)]
pub enum PromptPick {
    /// The strongest control that fires it, which is what a hint wants.
    #[default]
    First,
    /// The one at this index, counting from zero.
    Nth(u8),
}

/// What to render when nothing fires the action.
///
/// Defaults to an em dash. A blank is worse: "Press  to thrust" reads as a bug in the game rather
/// than as an unbound control, which is what it is.
#[derive(Component, Clone)]
pub struct PromptUnbound(pub String);

/// Draws prompts, and keeps them true.
pub fn plugin(app: &mut App) {
    // Ahead of every UI system, so a caption that changed this frame is laid out at the width it
    // will be drawn at rather than at the width it used to be.
    app.add_systems(
        PostUpdate,
        refresh_prompts.before(UiSystems::Prepare).run_if(
            resource_changed::<PromptGeneration>.or_else(any_match_filter::<Added<PromptSpan>>),
        ),
    );
}

/// Everything one span needs in order to ask its question.
type PromptQuery = (
    Entity,
    &'static PromptSpan,
    Option<&'static PromptScheme>,
    Option<&'static PromptClass>,
    Option<&'static PromptPick>,
    Option<&'static PromptUnbound>,
);

/// Rewrites every prompt on screen.
///
/// Every one of them rather than the ones that changed: a rebind or a context switching over can
/// move any prompt in the game, and asking is the only way to find out which. What keeps that
/// affordable is that it does not run at all on a frame where nothing said the answer moved — the
/// run condition is one resource comparison, and no prompt is read until it passes.
///
/// Exclusive because the lookup reads the whole world. It walks every declared context, and the
/// types of those are long gone by the time anything wants a prompt.
fn refresh_prompts(world: &mut World) {
    let device = world.get_resource::<PromptDevice>().map_or_else(
        || {
            bevy::log::warn_once!(
                "prompts are being drawn with no `PromptDevice`, so they name whichever control \
                 was declared first rather than one this game chose. Insert \
                 `PromptDevice(Some(scheme))` to say which device your prompts speak for, or \
                 `PromptDevice(None)` to say that this game genuinely has no primary one."
            );
            None
        },
        |device| device.0,
    );

    let mut spans = world.query::<PromptQuery>();
    let captions: Vec<(Entity, String)> = spans
        .iter(world)
        .map(|(entity, span, scheme, class, pick, unbound)| {
            let mut scope = PromptScope::ANY;
            if let Some(scheme) = scheme.map(|scheme| scheme.0).or(device) {
                scope = scope.on(scheme);
            }
            if let Some(class) = class {
                scope = scope.of(class.0);
            }

            let index = match pick.copied().unwrap_or_default() {
                PromptPick::First => 0,
                PromptPick::Nth(index) => usize::from(index),
            };

            let text = BindingTable::new(world)
                .prompts(span.0, scope)
                .get(index)
                .map_or_else(
                    || unbound.map_or_else(|| "—".to_string(), |text| text.0.clone()),
                    caption,
                );
            (entity, text)
        })
        .collect();

    for (entity, text) in captions {
        world.entity_mut(entity).insert(TextSpan::new(text));
    }
}

/// One prompt as a string, whatever must be held alongside it first.
///
/// A binding that needs a modifier says so, because a prompt that dropped it would caption
/// `Ctrl+S` as "S" — wrong rather than merely terse.
fn caption(prompt: &Prompt) -> String {
    let mut caption = String::new();
    for held in &prompt.with {
        caption.push_str(&held.fallback_label());
        caption.push('+');
    }
    caption.push_str(&prompt.origin.fallback_label());
    caption
}
