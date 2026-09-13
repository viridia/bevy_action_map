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

use std::borrow::Cow;

use bevy::ecs::schedule::SystemCondition;
use bevy::prelude::*;
use bevy::text::InlineBox;
use bevy::ui::UiSystems;
use bevy_action_map::device::{Brand, GamepadBrand};
use bevy_action_map::prelude::*;

/// Renders the control that would currently fire an action.
///
/// The string is filled in for you, and rewritten when it stops being true — when a binding
/// changes, when the context holding it switches on or off, or when nothing carries that context
/// any more.
#[derive(Component, Clone, Copy, Default)]
#[require(TextSpan)]
pub struct PromptSpan(pub ActionId);

/// Renders the control that would currently fire an action as an icon, inline in a line of text.
///
/// Falls back to the same text [`PromptSpan`] would show, bracketed, wherever nothing has art for
/// the control — an unrecognized pad brand, or a control the atlas simply does not cover — so a
/// caption never goes blank for want of an icon. The brackets are only there for that fallback: an
/// icon reads as a control on its own and does not need them, so a caller wraps neither in its own
/// punctuation. No `#[require]`: which of `InlineImage` or `TextSpan` the entity carries is the
/// fallback decision itself, so [`refresh_icon_prompts`] sets whichever applies rather than always
/// carrying both.
#[derive(Component, Clone, Copy, Default)]
pub struct IconPromptSpan(pub ActionId);

/// Which device family one span speaks for, overriding [`PromptDevice`].
///
/// What a settings screen's gamepad column wants: those rows name pad controls whatever the rest
/// of the game's prompts speak for.
#[derive(Component, Clone, Copy)]
pub struct PromptFamily(pub DeviceFamily);

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
    app.init_resource::<IconManifest>();
    // Ahead of every UI system, so a caption that changed this frame is laid out at the width it
    // will be drawn at rather than at the width it used to be.
    app.add_systems(
        PostUpdate,
        (
            refresh_prompts.run_if(
                resource_changed::<PromptGeneration>.or_else(any_match_filter::<Added<PromptSpan>>),
            ),
            refresh_icon_prompts.run_if(
                resource_changed::<PromptGeneration>
                    .or_else(any_match_filter::<Added<IconPromptSpan>>),
            ),
        )
            .before(UiSystems::Prepare),
    );
}

/// [`PromptDevice`]'s family, warning once if a game never set one.
///
/// Shared by [`refresh_prompts`] and [`refresh_icon_prompts`] so the warning fires from one call
/// site rather than two.
fn active_family(world: &World) -> Option<DeviceFamily> {
    world.get_resource::<PromptDevice>().map_or_else(
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
    )
}

/// Whichever connected pad's brand a prompt should speak in, `Generic` if none says.
///
/// The first one found: a game with two pads of different brands on one line has no answer that is
/// right for both, and picking one beats naming none.
fn connected_brand(world: &mut World) -> GamepadBrand {
    let mut brands = world.query::<&Brand>();
    brands
        .iter(world)
        .next()
        .map_or(GamepadBrand::Generic, |brand| brand.0)
}

/// The brand a *text* prompt writes in, which is not quite the brand of the pad.
///
/// An unrecognized pad resolves to `Generic`, and `Generic` has no word of its own for a face
/// button — so a prompt asking it plainly gets "South Button", which is not what anybody calls that
/// button. Aftermarket PC pads are overwhelmingly Xbox-labelled, and on the pads that are not, the
/// letter still points at the right physical button, so "A" is the better guess by a wide margin.
///
/// A presentation choice, and deliberately this side of the crate: `Generic` genuinely means "no
/// brand-specific name", which is a fact. What to *show* when there is none is a game's call (D53).
/// [`refresh_icon_prompts`] does not do this — art that says Xbox on an unrecognized pad would be
/// claiming something, where a word is only labelling one.
fn labelling_brand(world: &mut World) -> GamepadBrand {
    match connected_brand(world) {
        GamepadBrand::Generic => GamepadBrand::Xbox,
        brand => brand,
    }
}

/// The scope a prompt's own companions narrow it to, and which of possibly several answers it asks
/// for — shared by [`refresh_prompts`] and [`refresh_icon_prompts`].
fn scope_and_index(
    device: Option<DeviceFamily>,
    scheme: Option<&PromptFamily>,
    class: Option<&PromptClass>,
    pick: Option<&PromptPick>,
) -> (PromptScope, usize) {
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
    (scope, index)
}

/// Everything one span needs in order to ask its question.
type PromptQuery = (
    Entity,
    &'static PromptSpan,
    Option<&'static PromptFamily>,
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
    let device = active_family(world);
    let brand = labelling_brand(world);

    let mut spans = world.query::<PromptQuery>();
    let captions: Vec<(Entity, String)> = spans
        .iter(world)
        .map(|(entity, span, scheme, class, pick, unbound)| {
            let (scope, index) = scope_and_index(device, scheme, class, pick);
            let text = BindingTable::new(world)
                .prompts(span.0, scope)
                .get(index)
                .map_or_else(
                    || unbound.map_or_else(|| "—".to_string(), |text| text.0.clone()),
                    |prompt| caption(prompt, brand),
                );
            (entity, text)
        })
        .collect();

    for (entity, text) in captions {
        world.entity_mut(entity).insert(TextSpan::new(text));
    }
}

/// One prompt as a string, whatever must be held alongside it and whatever timing it wants first.
///
/// A binding that needs a modifier says so, because a prompt that dropped it would caption `Ctrl+S`
/// as "S" — wrong rather than merely terse. A binding that only fires held says so too:
/// `prompt.condition` is `ConditionDescriptor::None` for almost everything, and where it is not,
/// its fallback renderer is what turns "W" into "Hold W" rather than a bare, uninterpretable
/// "Hold".
fn caption(prompt: &Prompt, brand: GamepadBrand) -> String {
    let mut control = String::new();
    for held in &prompt.with {
        control.push_str(&branded(held, brand));
        control.push('+');
    }
    control.push_str(&branded(&prompt.origin, brand));
    prompt.condition.fallback_format(&control)
}

/// One control's name, in the pad's own words where it has any.
///
/// Only a control this crate knows can be renamed: a foreign one arrived with a label already, and
/// whatever reported it is the only thing that knows what its buttons are called.
fn branded(origin: &ControlOrigin, brand: GamepadBrand) -> Cow<'_, str> {
    match origin {
        ControlOrigin::Ours(control) => control.fallback_label_for_brand(brand),
        other => other.fallback_label(),
    }
}

/// Which (tier, control) pairs have art, in both `assets/input_prompts/` and
/// `assets/input_prompts_inline/` alike — the two mirror each other's coverage exactly, one
/// downscaled copy per full-size original.
///
/// Parsed once from the manifest `scripts/import_input_prompts.py` writes, so resolution never
/// opens a file to discover one is missing.
#[derive(Resource)]
struct IconManifest(std::collections::HashSet<String>);

impl Default for IconManifest {
    fn default() -> Self {
        const MANIFEST: &str = include_str!("../../assets/input_prompts/manifest.txt");
        Self(
            MANIFEST
                .lines()
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_string)
                .collect(),
        )
    }
}

/// The path segment a tier's art is filed under.
fn tier_str(tier: GlyphTier) -> &'static str {
    match tier {
        GlyphTier::KeyboardMouse => "keyboard_mouse",
        GlyphTier::Gamepad(GamepadBrand::Xbox) => "xbox",
        GlyphTier::Gamepad(GamepadBrand::PlayStation) => "playstation",
        GlyphTier::Gamepad(GamepadBrand::Nintendo) => "nintendo",
        GlyphTier::Gamepad(GamepadBrand::Generic) => "generic",
    }
}

/// Where a resolved glyph's inline-sized art lives, for `AssetServer::load`.
///
/// `input_prompts_inline/`, not `input_prompts/`: Bevy's `InlineImage` sizes itself from the
/// loaded image's own pixel dimensions with no resize hook (bevyengine/bevy#25710), so an inline
/// glyph needs art pre-scaled to sit inline with a line of text rather than towering over it.
fn inline_icon_path(glyph: Glyph) -> String {
    let Glyph::Own(tier, control) = glyph else {
        unreachable!("`Glyph` has one variant today");
    };
    format!(
        "input_prompts_inline/{}/{}.png",
        tier_str(tier),
        control.name()
    )
}

/// Everything one icon span needs in order to ask its question — mirrors [`PromptQuery`].
type IconPromptQuery = (
    Entity,
    &'static IconPromptSpan,
    Option<&'static PromptFamily>,
    Option<&'static PromptClass>,
    Option<&'static PromptPick>,
    Option<&'static PromptUnbound>,
);

/// What one icon span resolved to: an image to load, or text to fall back to.
enum Resolved {
    Icon(String),
    Text(String),
}

/// Rewrites every icon prompt on screen — the same staleness contract as [`refresh_prompts`],
/// answering the same lookup, but choosing between an icon and text rather than only ever text.
///
/// Which gamepad's brand a control's icon draws in is read the way [`split_screen`]'s device label
/// already does: the first connected pad's `Brand`, since nothing here plays more than one at once.
///
/// [`split_screen`]: ../split_friction/split_screen/index.html
fn refresh_icon_prompts(world: &mut World) {
    let mut spans = world.query::<IconPromptQuery>();
    // Nothing to draw, so nothing to ask `AssetServer` or `Brand` for either — a game that never
    // spawns an `IconPromptSpan` should not have to carry either just because this system shares
    // `PromptSpan`'s own staleness signal.
    if spans.iter(world).next().is_none() {
        return;
    }

    let device = active_family(world);
    // Two brands, deliberately: art is resolved from what the pad actually is, since a picture
    // saying Xbox on a pad that is not one claims more than a word does. The text it falls back to
    // is only a word, so it names buttons on the same terms `refresh_prompts` does.
    let brand = connected_brand(world);
    let labelled = labelling_brand(world);
    let manifest = &world.resource::<IconManifest>().0;
    let resolved: Vec<(Entity, Resolved)> = spans
        .iter(world)
        .map(|(entity, span, scheme, class, pick, unbound)| {
            let (scope, index) = scope_and_index(device, scheme, class, pick);
            let prompts = BindingTable::new(world).prompts(span.0, scope);
            let resolved = match prompts.get(index) {
                None => {
                    Resolved::Text(unbound.map_or_else(|| "—".to_string(), |text| text.0.clone()))
                }
                Some(prompt) => match prompt.origin {
                    ControlOrigin::Ours(control) => {
                        resolve_glyph(control, brand, |tier, control| {
                            manifest.contains(&format!("{}/{}", tier_str(tier), control.name()))
                        })
                        .map_or_else(
                            || Resolved::Text(caption(prompt, labelled)),
                            |glyph| Resolved::Icon(inline_icon_path(glyph)),
                        )
                    }
                    ControlOrigin::Foreign { .. } => Resolved::Text(caption(prompt, labelled)),
                },
            };
            (entity, resolved)
        })
        .collect();

    let asset_server = world.resource::<AssetServer>().clone();
    for (entity, resolved) in resolved {
        let mut entity = world.entity_mut(entity);
        match resolved {
            Resolved::Icon(path) => {
                entity.remove::<TextSpan>();
                entity.insert(InlineImage {
                    image: asset_server.load(path),
                    ..default()
                });
            }
            Resolved::Text(text) => {
                entity.remove::<(InlineImage, InlineBox)>();
                // An icon reads as a control on its own — a small, self-contained badge — but bare
                // text sitting in a button caption does not, so the fallback gets the visual
                // grouping an icon does not need.
                entity.insert(TextSpan::new(format!("[{text}]")));
            }
        }
    }
}
