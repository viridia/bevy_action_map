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
//! It shows a game how, rather than covering every case, and a game adapts it. Where it takes the
//! simple route over the exact one, that is a choice for an example, not a limit of the crate.
//!
//! # What is here
//!
//! [`ActionPrompt`] names a context and an action, and draws whatever fires it. The companions
//! beside it narrow the answer — which device, which kind of control, which of several — and each
//! is a separate component rather than a field, so that a template says which question it is asking
//! and so that a new narrowing is additive. [`InlineIconSize`] is a companion of the same kind that
//! says how large an inline icon is drawn. A span with no companions renders the strongest control
//! bound to the action on the device the game speaks for.
//!
//! # Why one control
//!
//! A span shows one, and joining several is the app's business. Whether two controls read as
//! "W / Up", "W or Up" or as two table cells is a question about the screen they sit on, and a
//! game that wants all of them has [`Prompts`] and a `join`. What settles it is that a prompt is a
//! hint rather than a manual: "Press W to thrust" is the sentence, and "Press W or Up Arrow to
//! thrust" is a worse one even where both are true.

use std::borrow::Cow;
use std::marker::PhantomData;

use bevy::asset::AssetPath;
use bevy::ecs::schedule::SystemCondition;
use bevy::ecs::template::{Template, TemplateContext};
use bevy::prelude::*;
use bevy::text::{EmSize, RemSize};
use bevy::ui::{ComputedUiRenderTargetInfo, UiSystems};
use bevy_action_map::device::{Brand, GamepadBrand};
use bevy_action_map::prelude::*;
use bevy_action_map_ui::{IconLayout, PromptArt};

/// Renders the control an action is bound to in one context.
///
/// The context is the one the player is in when they read the prompt, since the same action can be
/// bound differently in two of them.
///
/// What is drawn is filled in for you, and redrawn when it stops being true — when a binding
/// changes, or when the context starts or stops being carried. A context that is switched off, or
/// shadowed by a menu over it, still answers, so hiding a hint while it does not apply is the
/// game's call.
///
/// A scene writes one with a template that names the context and action as types:
/// `~PromptSpan::<Menu, Close>`, `~IconPromptSpan::<Menu, Close>` or `~IconPrompt::<Menu, Close>`.
/// Code that has only the ids, such as a screen listing every registered action, spawns this
/// directly. Either way the entity is given the text or UI components its form draws with when the
/// prompt is first written, keeping any the scene set.
#[derive(Component, Clone, Copy, Default)]
pub struct ActionPrompt {
    /// The context the action is looked up in.
    pub context: ContextId,
    /// The action whose control is shown.
    pub action: ActionId,
    /// How the control is drawn.
    pub form: PromptForm,
}

/// How an [`ActionPrompt`] draws its control.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum PromptForm {
    /// The control's name, as a text span in a line of text.
    #[default]
    Text,
    /// An icon inline in a line of text, as a text span.
    ///
    /// A chord draws every control in it as children of this span, joined by `+`, so the whole
    /// chord stays one run that moves with its sentence. The `+` takes the span's own `TextFont`
    /// and `TextColor`.
    ///
    /// The icons are sized from the span's font, so a larger `TextFont` draws larger icons, and an
    /// [`InlineIconSize`] beside the span sets how much larger. A font size or an icon size
    /// relative to the window is read when the icons go in, and a resize leaves them at that size
    /// until the prompt's answer next changes.
    ///
    /// A new answer waits for its art: the span goes on drawing the old one until every icon in the
    /// new one has loaded, so the line never reflows around an icon that is still loading.
    ///
    /// Falls back to the same text [`PromptForm::Text`] would show, bracketed, wherever nothing has
    /// art for the control, such as an unrecognized pad brand or a control the art simply does not
    /// cover, so a caption never goes blank for want of an icon. A chord falls back whole if any
    /// control in it has no art. The brackets are only there for that fallback: an icon reads as a
    /// control on its own and does not need them, so a caller wraps neither in its own punctuation.
    InlineIcon,
    /// An icon in a UI node of its own.
    ///
    /// What a button's caption or a row of hints wants, where [`PromptForm::InlineIcon`] is for a
    /// prompt in the middle of a sentence. It lays out like any other node: in a row with
    /// `align_items: AlignItems::Center`, it lines up with the label beside it.
    ///
    /// The icons fill the node's height, so size them by giving its `Node` one. The art is scaled
    /// down from a large original, which keeps it sharp on a high-density display. Without a
    /// height, the art is drawn at its own size.
    ///
    /// A chord is a row of icons joined by `+`, and the `+` takes this node's `TextFont` and
    /// `TextColor`, as does the text it falls back to. Otherwise it behaves as an inline icon does:
    /// it keeps drawing the old answer until the new one's art has loaded, and falls back to
    /// bracketed text where there is no art.
    BlockIcon,
}

/// Defines a template that builds an [`ActionPrompt`] of one form, from a context and an action
/// named as types.
macro_rules! prompt_template {
    ($(#[$doc:meta])* $name:ident, $form:expr) => {
        $(#[$doc])*
        pub struct $name<C, A>(PhantomData<fn() -> (C, A)>);

        impl<C, A> Default for $name<C, A> {
            fn default() -> Self {
                Self(PhantomData)
            }
        }

        impl<C: InputContext, A: InputAction> Template for $name<C, A> {
            type Output = ActionPrompt;

            fn build_template(&self, _: &mut TemplateContext) -> Result<ActionPrompt> {
                Ok(ActionPrompt {
                    context: ContextId::of::<C>(),
                    action: A::id(),
                    form: $form,
                })
            }

            fn clone_template(&self) -> Self {
                Self::default()
            }
        }
    };
}

prompt_template! {
    /// A prompt that names its control in text, written `~PromptSpan::<Menu, Close>` as a child of
    /// a `Text`.
    PromptSpan, PromptForm::Text
}

prompt_template! {
    /// A prompt that draws its control as an icon in a line of text, written
    /// `~IconPromptSpan::<Menu, Close>` as a child of a `Text`. See [`PromptForm::InlineIcon`].
    IconPromptSpan, PromptForm::InlineIcon
}

prompt_template! {
    /// A prompt that draws its control as an icon in a node of its own, written
    /// `~IconPrompt::<Menu, Close>`. See [`PromptForm::BlockIcon`].
    IconPrompt, PromptForm::BlockIcon
}

/// Which device family one span speaks for, overriding [`PromptDevice`].
///
/// What a settings screen's gamepad column wants: those rows name pad controls whatever the rest of
/// the game's prompts speak for.
#[derive(Component, Clone, Copy)]
pub struct PromptFamily(pub DeviceFamily);

/// Narrows to one kind of control, for a prompt with room to name a button and not a stick.
#[derive(Component, Clone, Copy)]
pub struct PromptClass(pub ControlClass);

/// Which one, where several controls fire the action.
///
/// **Not the settings screen's primary and secondary.** This indexes the prompt lookup's answer,
/// which answers a composite once per direction, so the second entry here is as likely to be "the
/// key that turns the other way" as it is to be a second binding. The declared columns are
/// [`mappings`]' business.
#[derive(Component, Clone, Copy, Default)]
pub enum PromptPick {
    /// The strongest control that fires it, which is what a hint wants.
    #[default]
    First,
    /// The one at this index, counting from zero.
    Nth(u8),
}

/// What to render for an unbound action.
///
/// Defaults to an em dash. A blank is worse: "Press  to thrust" reads as a bug in the game rather
/// than as an unbound control, which is what it is.
#[derive(Component, Clone)]
pub struct PromptUnbound(pub String);

/// How tall an inline icon prompt, one of [`PromptForm::InlineIcon`], draws its icons.
///
/// Measured against the span's font: `Val::Em(2.0)` and `Val::Percent(200.0)` both stand an icon
/// twice the font size. `Val::Px` is a fixed height whatever the font, and `Val::Auto` draws the
/// art at its own size. Without one, an icon stands at `Val::Em(5.0 / 3.0)`, which suits art that
/// leaves a margin around each glyph.
///
/// A block icon prompt ignores it: its `Node`'s height sizes the icons.
#[derive(Component, Clone, Copy)]
pub struct InlineIconSize(pub Val);

impl Default for InlineIconSize {
    // Kenney's art draws each glyph in the middle three quarters of its square, so this puts the
    // glyph itself a little taller than the letters beside it.
    fn default() -> Self {
        Self(Val::Em(5.0 / 3.0))
    }
}

/// Which brand pad prompts speak in, whatever pad is connected.
///
/// Without it, prompts follow the first connected pad. Set it where the pad is not the answer: a
/// screen showing every brand's art, or a player who would rather read the buttons of a pad other
/// than the one the game detected. Changing it is yours to announce, with
/// [`PromptGeneration::invalidate`], exactly as changing [`PromptDevice`] is.
#[derive(Resource, Clone, Copy)]
pub struct PromptBrand(pub GamepadBrand);

/// Who every prompt asks: the mapper's own tables unless a backend that owns some of the bindings
/// says otherwise.
#[derive(Resource, Clone, Copy)]
pub struct PromptSource(pub fn(&World, ContextId, ActionId, PromptScope) -> Vec<Prompt>);

impl Default for PromptSource {
    fn default() -> Self {
        Self(|world, context, action, scope| {
            BindingTable::new(world).prompts(context, action, scope)
        })
    }
}

/// Draws prompts, and keeps them true. The art an icon prompt draws comes from [`PromptArt`]'s
/// providers, added by plugins of their own.
pub fn plugin(app: &mut App) {
    app.init_resource::<PromptArt>();
    app.init_resource::<PromptSource>();
    // Ahead of every UI system, so a caption that changed this frame is laid out at the width it
    // will be drawn at rather than at the width it used to be.
    app.add_systems(
        PostUpdate,
        (
            refresh_prompts.run_if(
                resource_changed::<PromptGeneration>
                    .or_else(any_match_filter::<Added<ActionPrompt>>),
            ),
            swap_in_icons
                .after(refresh_prompts)
                .run_if(any_with_component::<PendingIcons>),
        )
            .before(UiSystems::Prepare),
    );
}

/// [`PromptDevice`]'s family, warning once if a game never set one.
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

/// The brand a prompt should speak in: [`PromptBrand`] where the game set one, and otherwise the
/// first connected pad's, `Generic` if none says.
///
/// The first one found: a game with two pads of different brands on one line has no answer that is
/// right for both, and picking one beats naming none.
fn connected_brand(world: &mut World) -> GamepadBrand {
    if let Some(brand) = world.get_resource::<PromptBrand>() {
        return brand.0;
    }
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
/// brand-specific name", which is a fact. What to *show* when there is none is a game's call.
/// Icon art does not do this — art that says Xbox on an unrecognized pad would be
/// claiming something, where a word is only labelling one.
fn labelling_brand(world: &mut World) -> GamepadBrand {
    match connected_brand(world) {
        GamepadBrand::Generic => GamepadBrand::Xbox,
        brand => brand,
    }
}

/// The scope a prompt's own companions narrow it to, and which of possibly several answers it asks
/// for.
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

/// Everything one prompt needs in order to ask its question.
type PromptQuery = (
    Entity,
    &'static ActionPrompt,
    Option<&'static PromptFamily>,
    Option<&'static PromptClass>,
    Option<&'static PromptPick>,
    Option<&'static PromptUnbound>,
);

/// What one prompt resolved to: an image to load per control in the chord, in the order they are
/// drawn, or text.
enum Resolved {
    Icons(Vec<AssetPath<'static>>),
    Text(String),
}

/// Rewrites every prompt on screen.
///
/// Every one of them rather than the ones that changed: a rebind or a context switching over can
/// move any prompt in the game, and asking is the only way to find out which. What keeps that
/// affordable is that it does not run at all on a frame where nothing said the answer moved — the
/// run condition is one resource comparison, and no prompt is read until it passes.
///
/// Exclusive because the lookup reads the whole world. It walks every declared context, and the
/// types of those are long gone by the time anything wants a prompt.
///
/// Which gamepad's brand a control's icon draws in is read the way [`split_screen`]'s device label
/// already does: the first connected pad's `Brand`, since nothing here plays more than one at once,
/// unless [`PromptBrand`] overrides it.
///
/// [`split_screen`]: ../split_friction/split_screen/index.html
fn refresh_prompts(world: &mut World) {
    let mut prompts = world.query::<PromptQuery>();
    if prompts.iter(world).next().is_none() {
        return;
    }

    let device = active_family(world);
    // Two brands, deliberately: art is resolved from what the pad actually is, text from what the
    // button is called — see `labelling_brand`.
    let brand = connected_brand(world);
    let labelled = labelling_brand(world);
    let art = world.resource::<PromptArt>();
    let source = world.resource::<PromptSource>().0;
    let resolved: Vec<(Entity, PromptForm, Resolved)> = prompts
        .iter(world)
        .map(|(entity, prompt, scheme, class, pick, unbound)| {
            let (scope, index) = scope_and_index(device, scheme, class, pick);
            let answers = source(world, prompt.context, prompt.action, scope);
            let Some(answer) = answers.get(index) else {
                let text = unbound.map_or_else(|| "—".to_string(), |text| text.0.clone());
                return (entity, prompt.form, Resolved::Text(text));
            };
            let layout = match prompt.form {
                PromptForm::Text => {
                    return (
                        entity,
                        prompt.form,
                        Resolved::Text(caption(answer, labelled)),
                    );
                }
                PromptForm::InlineIcon => IconLayout::Inline,
                PromptForm::BlockIcon => IconLayout::Block,
            };
            let has_art = |tier, origin: &ControlOrigin| art.has_art(tier, origin, layout);
            // All or none: a chord drawn half as art and half as bracketed words reads as two
            // separate answers.
            let resolved = answer
                .with
                .iter()
                .chain([&answer.origin])
                .map(|origin| {
                    resolve_glyph(origin, brand, has_art).and_then(|glyph| art.path(&glyph, layout))
                })
                .collect::<Option<Vec<_>>>()
                .map_or_else(
                    || Resolved::Text(caption(answer, labelled)),
                    Resolved::Icons,
                );
            (entity, prompt.form, resolved)
        })
        .collect();

    for (entity, form, resolved) in resolved {
        let mut entity = world.entity_mut(entity);
        // Whatever the scene set wins: a block's `Node` carries its height.
        match form {
            PromptForm::Text | PromptForm::InlineIcon => {
                entity.insert_if_new(TextSpan::default());
            }
            PromptForm::BlockIcon => {
                entity.insert_if_new((Node::default(), TextFont::default(), TextColor::default()));
            }
        }
        match resolved {
            Resolved::Icons(paths) => {
                let asset_server = entity.resource::<AssetServer>();
                let icons = paths.into_iter().map(|path| asset_server.load(path));
                let icons = PendingIcons(icons.collect());
                entity.insert(icons);
            }
            Resolved::Text(text) if form == PromptForm::Text => {
                entity.insert(TextSpan::new(text));
            }
            Resolved::Text(text) => {
                // A chord still waiting on its art is no longer the answer.
                entity.remove::<PendingIcons>();
                entity.despawn_children();
                // The brackets belong to the fallback, not to a prompt — see
                // `PromptForm::InlineIcon`.
                let text = format!("[{text}]");
                if form == PromptForm::BlockIcon {
                    let font = entity.get::<TextFont>().cloned().unwrap_or_default();
                    let color = entity.get::<TextColor>().copied().unwrap_or_default();
                    entity.with_child(block_text(text, font, color));
                } else {
                    entity.insert(TextSpan::new(text));
                }
            }
        }
    }
}

/// One prompt as a string, with whatever must be held alongside it.
///
/// A binding that needs a modifier says so, because a prompt that dropped it would caption `Ctrl+S`
/// as "S", which is wrong rather than merely terse. How the control is pressed is not said: "Hold
/// ⟨X⟩ to reload" is the sentence around a prompt that reads "X", and the game writes it.
fn caption(prompt: &Prompt, brand: GamepadBrand) -> String {
    let mut control = String::new();
    for held in &prompt.with {
        control.push_str(&branded(held, brand));
        control.push('+');
    }
    control.push_str(&branded(&prompt.origin, brand));
    control
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

/// Text inside a block icon prompt: the `+` in a chord, or the whole of a fallback.
///
/// Centred on its own, since the prompt's node leaves its children stretched to its height and a
/// stretched text node draws at the top.
fn block_text(text: impl Into<String>, font: TextFont, color: TextColor) -> impl Bundle {
    (
        Text::new(text),
        font,
        color,
        Node {
            align_self: AlignSelf::Center,
            ..default()
        },
    )
}

/// A chord of icons waiting on its art, while the prompt goes on drawing whatever it drew before.
///
/// Swapping the children as soon as the answer changes would lay the line out around icons still
/// loading, which take no space, and then reflow it when the art lands. A newer answer replaces
/// this one, so a prompt never swaps in a chord that has gone stale.
#[derive(Component)]
struct PendingIcons(Vec<Handle<Image>>);

/// Replaces a prompt's children with its pending chord once every icon in it has loaded.
///
/// Before UI layout, so each icon's box is sized from an image already in memory in the frame it
/// first appears. A chord whose art is already loaded, such as one a rebind left unchanged, swaps
/// in the same frame [`refresh_prompts`] resolved it.
///
/// An icon that fails to load leaves its chord pending, with the previous one still drawn: the
/// art's provider has already said the file exists, so a failure is a broken install, and Bevy logs
/// it.
#[expect(
    clippy::type_complexity,
    reason = "one query of everything a span is drawn from"
)]
fn swap_in_icons(
    mut commands: Commands,
    spans: Query<(
        Entity,
        &PendingIcons,
        &TextFont,
        &TextColor,
        &ActionPrompt,
        Option<&InlineIconSize>,
    )>,
    parents: Query<&ChildOf>,
    targets: Query<&ComputedUiRenderTargetInfo>,
    rem: Option<Res<RemSize>>,
    images: Res<Assets<Image>>,
) {
    let rem = rem.map_or_else(RemSize::default, |rem| *rem);
    for (entity, pending, font, color, prompt, size) in &spans {
        let block = prompt.form == PromptForm::BlockIcon;
        if !pending.0.iter().all(|icon| images.contains(icon)) {
            continue;
        }
        // Against the text's own target, which is what Bevy resolves the font against.
        let viewport = parents
            .iter_ancestors(entity)
            .find_map(|ancestor| targets.get(ancestor).ok())
            .map_or(Vec2::ZERO, ComputedUiRenderTargetInfo::logical_size);
        // In logical pixels, which `InlineImage` takes, so at a scale factor of 1; the font is
        // what a percentage is of. `Auto` cannot resolve, and leaves the height unset.
        let font_size = font.font_size.eval(viewport, rem);
        let size = size.copied().unwrap_or_default().0;
        let height = size
            .resolve(1.0, font_size, viewport, EmSize(font_size), rem)
            .ok();
        let mut span = commands.entity(entity);
        span.remove::<PendingIcons>().despawn_related::<Children>();
        if !block {
            span.insert(TextSpan::default());
        }
        let icons = pending.0.clone();
        let (font, color) = (font.clone(), *color);
        span.with_children(|chord| {
            for (n, icon) in icons.into_iter().enumerate() {
                if block {
                    if n > 0 {
                        chord.spawn(block_text("+", font.clone(), color));
                    }
                    chord.spawn((
                        ImageNode::new(icon),
                        Node {
                            height: Val::Percent(100.0),
                            ..default()
                        },
                    ));
                } else {
                    if n > 0 {
                        chord.spawn((TextSpan::new("+"), font.clone(), color));
                    }
                    chord.spawn(InlineImage {
                        image: icon,
                        height,
                        ..default()
                    });
                }
            }
        });
    }
}
