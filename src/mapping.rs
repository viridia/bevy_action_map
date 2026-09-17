//! The presentation model: mappings, tunables, and presets.
//!
//! The binding model is a developer's model. Negate, swizzle and response curves are adapters for
//! fitting a control to an action, and nobody rebinding "move forward" should meet one. What a
//! player gets is deliberately smaller: a list of **mappings**, each a named thing with the controls
//! that drive it.
//!
//! A mapping holds an ordered list of **slots**, each holding one control, because a rebinding row
//! usually has more than one — "Primary" and "Secondary" is the arrangement almost every game
//! ships. Declaring one action mappable twice in one family is how you ship both defaults: they
//! arrive as one row holding two controls rather than as two rows.
//!
//! A mapping is not a binding. A movement composite is written once and binds four keys, and each
//! becomes a mapping named for the direction it pushes, which is how every shipped game presents
//! movement and is why the composite itself never reaches the player.
//!
//! **Every binding is listed; changing one is what has to be asked for.** A player is entitled to
//! see what their controls do, so a binding appears here by saying nothing at all — as a row they
//! can read and not change. [`mappable`](crate::binding::BindingBuilder::mappable) is what makes a
//! row changeable, and [`private`](crate::binding::BindingBuilder::private) is what keeps a binding
//! out of the list altogether, for the ones that are genuinely the game's own business.
//!
//! Two actions may deliberately share one control (tap to dodge, hold to sprint), and the player
//! rebinds *the control* rather than either of them.
//! [`follow`](crate::binding::InputContextBuilder::follow) says so: it declares one action riding
//! another's bindings, one for one, contributing no row of its own and moving with them when the
//! player changes them.
//!
//! A **tunable** is the other half of the presentation model: a named, typed value — a range or a
//! switch — that adjusts one binding without ever showing the modifier it drives.
//! [`tunable_dead_zone`](crate::binding::BindingBuilder::tunable_dead_zone) and
//! [`hold_or_toggle`](crate::binding::InputContextBuilder::hold_or_toggle) both declare one; a
//! screen walks them with [`tunables`] the same way it walks mappings with [`mappings`].
//!
//! ```ignore
//! app.add_context::<OnFoot>(|controls| {
//!     controls.bind::<Move>(DirectionalButtons::wasd()).mappable();  // four mappings…
//!     controls.bind::<Jump>(KeyCode::Space).mappable();              // one mapping…
//!     controls.bind::<Jump>(KeyCode::KeyJ).mappable();               // …with a second slot
//!     controls.bind::<Look>(MouseMove);                              // listed; not changeable
//!     controls.bind::<Crouch>(KeyCode::KeyC).private();              // not listed at all
//!     controls.follow::<Lunge, Jump>(|binding| binding.hold(0.4));   // rides the Jump row
//! });
//! ```
//!
//! Then a screen walks the mappings with [`mappings`] and the tunables with [`tunables`], and needs
//! to know nothing else about this crate.

use alloc::string::String;
use alloc::vec::Vec;

use bevy_ecs::world::World;

use crate::action::{ActionId, ActionIntent, ChannelShape};
use crate::binding::{BindingInput, BindingModifier, BindingPart, BindingSpec, Control};
use crate::condition::ConditionDescriptor;
use crate::device::DeviceFamily;
use crate::inspect::OverrideStage;

/// What a mapping is called, in a form a translation catalogue can look up.
///
/// This is a **key, not display text**. A rebinding row has two halves — the name of the thing and
/// the control bound to it — and a crate that returned "Move Forward" for the first would make one
/// half of every row untranslatable. So the key is what is carried, and rendering it is the app's
/// business.
///
/// It is derived rather than declared: the action's path plus the part's name, both of which
/// already exist and are already stable. `gameplay.move` plus `up` is `gameplay.move.up`, and
/// nothing has to be kept in step with anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MappingKey {
    prefix: &'static str,
    part: BindingPart,
}

impl MappingKey {
    pub(crate) const fn new(prefix: &'static str, part: BindingPart) -> Self {
        Self { prefix, part }
    }

    /// Which part of its binding this mapping addresses.
    pub const fn part(self) -> BindingPart {
        self.part
    }

    /// Readable text for a game with no translation catalogue.
    ///
    /// Turns the key into something presentable — `gameplay.move.up` reads as "Move Up" — so that
    /// shipping translations is never the price of a legible rebinding screen. Use it as the
    /// fallback when a lookup misses, not in place of one.
    pub fn fallback_label(self) -> String {
        // The namespace is for keeping keys apart, not for reading, so only the last segment of the
        // prefix survives: `disasteroids.toggle_overlay` is "Toggle Overlay", not "Disasteroids …".
        words_of(last_segment(self.prefix).split('_').chain(self.part.name()))
    }
}

/// Readable text for a localization key, for a game with no translation catalogue.
///
/// A mapping's [`category`](ActionMapping::category) is a key on the same terms as its name, and
/// a screen that groups rows under headings has to render it. `gameplay.flight` reads as
/// "Flight". Use it as the fallback when a catalogue lookup misses, not in place of one.
pub fn fallback_label(key: &str) -> String {
    words_of(last_segment(key).split('_'))
}

/// The part of a key that is meant to be read: `gameplay.flight` is about flight, not the game.
fn last_segment(key: &str) -> &str {
    key.rsplit('.').next().unwrap_or(key)
}

fn words_of<'a>(words: impl Iterator<Item = &'a str>) -> String {
    let mut label = String::new();
    for word in words {
        if !label.is_empty() {
            label.push(' ');
        }
        let mut characters = word.chars();
        if let Some(first) = characters.next() {
            label.extend(first.to_uppercase());
            label.push_str(characters.as_str());
        }
    }
    label
}

impl core::fmt::Display for MappingKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.prefix)?;
        if let Some(part) = self.part.name() {
            f.write_str(".")?;
            f.write_str(part)?;
        }
        Ok(())
    }
}

/// Whether the player may change what a mapping holds.
///
/// Appearing on a controls screen and being changeable there are two different things, and a great
/// many games want the first without the second. A pad's bindings are the usual case: a player still
/// needs to see what the buttons do, while the remapping itself belongs to a preset, to the console's
/// own settings, or to whatever software is driving the pad.
///
/// A screen reads this to decide whether a row is a button or a label. It is never a security
/// boundary: a game that does not want a control changed does not offer it. It says nothing about
/// whether the binding *works*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RebindPolicy {
    /// The player may change it, in this game's own screen.
    ///
    /// What [`mappable`](crate::binding::BindingBuilder::mappable) declares.
    Here,
    /// Shown so the player can see what the control does, and not changeable here.
    ///
    /// What a binding gets by saying nothing. Where the player *does* change it is the game's
    /// business to explain and its screen's to offer.
    Fixed,
}

impl RebindPolicy {
    /// Whether a capture may fill this mapping's slots.
    pub const fn is_rebindable(self) -> bool {
        matches!(self, Self::Here)
    }
}

/// One row of a rebinding screen.
///
/// Everything a screen needs to draw a row and file it under a heading, and nothing about how the
/// binding it came from is put together.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionMapping {
    /// What this mapping is called, as a key to look up.
    pub key: MappingKey,
    /// The action it drives.
    pub action: ActionId,
    /// That action's declared path.
    pub action_path: &'static str,
    /// What to file it under, if the action said.
    pub category: Option<&'static str>,
    /// Which set of devices it belongs to. A screen shows one family at a time.
    pub family: DeviceFamily,
    /// The kind of control it can hold, which is what a capture may accept for it.
    pub accepts: ChannelShape,
    /// The controls bound to it now, one per slot, in the order they were declared.
    ///
    /// Usually one. Two mappable bindings of the same action in the same family arrive here as one
    /// row with two slots filled rather than as two rows.
    ///
    /// **A slot may be empty.** A player who clears the primary of a two-control row leaves a gap
    /// rather than promoting the secondary into it, because position is what primary and secondary
    /// mean — so `None` is a cell the screen draws blank, and a later capture puts a control back in
    /// the same column it left. A row the player emptied entirely is simply short.
    ///
    /// How many cells to draw is the screen's decision, not this list's: a table offering a spare
    /// column draws one more cell than the row holds, and a capture fills it.
    pub slots: Vec<Option<Control>>,
    /// Whether the player may change what is in those slots.
    ///
    /// A screen draws a row of buttons for [`Here`](RebindPolicy::Here) and a row of labels for
    /// [`Fixed`](RebindPolicy::Fixed).
    pub rebind_policy: RebindPolicy,
    /// The path of the context the binding lives in.
    pub context: &'static str,
    /// Other actions riding this row's controls, declared with
    /// [`follow`](crate::binding::InputContextBuilder::follow).
    ///
    /// Empty for almost every mapping. Two actions sharing one control read as one thing to rebind,
    /// so a screen draws these as subordinate lines under the row rather than as rows of their own.
    pub followers: Vec<Follower>,
}

/// One other action riding a mapping's row, contributing no controls of its own.
///
/// What [`follow`](crate::binding::InputContextBuilder::follow) declares. The follower reads
/// exactly the controls its principal's row lists, so nothing here repeats them: its
/// [`condition`](Self::condition) is usually the whole of what a screen has to add.
#[derive(Clone, Debug, PartialEq)]
pub struct Follower {
    /// The action riding this row.
    pub action: ActionId,
    /// That action's declared path, mirroring the `action`/`action_path` pair
    /// [`ActionMapping`] itself carries. A localization key, so a catalogue answers it and
    /// [`fallback_label`](Self::fallback_label) derives "Afterburner" for a game without one.
    pub action_path: &'static str,
    /// What distinguishes this action's firing from a bare press of the row's controls, most often
    /// held a while longer. [`ConditionDescriptor::None`] means nothing distinguishes it: a follower
    /// declared with nothing for a screen to tell the player about it.
    pub condition: ConditionDescriptor,
}

impl Follower {
    /// Readable text for a game with no translation catalogue.
    ///
    /// Use it as the fallback when a catalogue lookup on [`action_path`](Self::action_path) misses,
    /// not in place of one.
    pub fn fallback_label(&self) -> String {
        fallback_label(self.action_path)
    }
}

/// Every mapping in the game, holding the controls that drive it **now**.
///
/// The list a rebinding screen is built from. Nothing here names an action or a context type, so a
/// screen written against it works for a game it was not compiled with.
///
/// Where a player has rebound something, this is what they rebound it to — the controls a row shows
/// are the controls that fire. Use [`declared_mappings`] for what the game shipped instead, which is
/// what a "reset to default" offers and what an override set is a diff against. With nothing
/// overridden the two are the same list, which is why the difference is easy to miss.
///
/// Sorting is the caller's: group by [`category`](ActionMapping::category) to draw headings, and
/// filter by [`family`](ActionMapping::family) to show one device's worth at a time. Filter by
/// [`context`](ActionMapping::context) for a screen that covers part of the game rather than all
/// of it — a vehicle's controls on their own, or everything except a debug context.
pub fn mappings(world: &World) -> Vec<ActionMapping> {
    gather_mappings(world, OverrideStage::Effective)
}

/// A player-adjustable value on a binding, typed so a generic screen can render it without ever
/// seeing the modifier it drives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TunableValue {
    /// A number bounded between `min` and `max`, rendered as a slider.
    Range {
        /// The value now — the game's own default, or whatever a player set it to.
        value: f32,
        /// The lower bound a slider stops at.
        min: f32,
        /// The upper bound a slider stops at.
        max: f32,
    },
    /// An on/off switch, rendered as a checkbox.
    Bool(bool),
}

/// One player-adjustable value, as a rebinding screen's tunables section reads it.
///
/// [`tunable_dead_zone`](crate::binding::BindingBuilder::tunable_dead_zone) and
/// [`hold_or_toggle`](crate::binding::InputContextBuilder::hold_or_toggle) are what declares one.
/// [`key`](Self::key) is a localization key rather than text to show, the same courtesy
/// [`ActionMapping::key`] gets — render it through [`fallback_label`] for a game with no
/// catalogue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tunable {
    /// What this tunable is called, as a key to look up. Chosen by the game rather than derived —
    /// unlike a mapping's key, nothing about a modifier names itself.
    pub key: &'static str,
    /// The action whose binding this tunable adjusts.
    pub action: ActionId,
    /// That action's declared path.
    pub action_path: &'static str,
    /// What to file it under, if the action said.
    pub category: Option<&'static str>,
    /// Which set of devices the binding it adjusts belongs to.
    pub family: DeviceFamily,
    /// The path of the context the binding lives in.
    pub context: &'static str,
    /// The current value — the game's own default, or what a player has set it to.
    pub value: TunableValue,
}

/// Every tunable in the game, holding its value **now**.
///
/// The list a rebinding screen's tunables section is built from, the same way [`mappings`] is for
/// its rows. Where a player has adjusted one, this is what they set it to; use
/// [`declared_tunables`] for what the game shipped, which is what "reset to default" offers.
pub fn tunables(world: &World) -> Vec<Tunable> {
    gather_tunables(world, OverrideStage::Effective)
}

/// Every tunable in the game, holding the value the game itself declared.
///
/// [`tunables`] with anything the player changed left out — what "reset to default" would produce.
pub fn declared_tunables(world: &World) -> Vec<Tunable> {
    gather_tunables(world, OverrideStage::Declared)
}

/// Every mapping in the game, holding the controls the game itself declared.
///
/// [`mappings`] with anything the player changed left out — the same rows, in the same order, with
/// the shipped controls in their slots. What "reset to default" would produce, and what a screen
/// compares against to show which rows have been changed.
pub fn declared_mappings(world: &World) -> Vec<ActionMapping> {
    gather_mappings(world, OverrideStage::Declared)
}

/// The body the four public readers above share, once per output type.
fn gather_mappings(world: &World, stage: OverrideStage) -> Vec<ActionMapping> {
    let Some(declared) = world.get_resource::<crate::inspect::DeclaredContexts>() else {
        return Vec::new();
    };

    declared
        .0
        .iter()
        .flat_map(|context| (context.mappings)(world, stage))
        .collect()
}

// `gather_mappings` for the tunables half.
fn gather_tunables(world: &World, stage: OverrideStage) -> Vec<Tunable> {
    let Some(declared) = world.get_resource::<crate::inspect::DeclaredContexts>() else {
        return Vec::new();
    };

    declared
        .0
        .iter()
        .flat_map(|context| (context.tunables)(world, stage))
        .collect()
}

/// The binding whose mapping `bindings[index]` rides, if there is one.
///
/// A follower reads the same controls as the binding it follows, which is what makes matching on
/// the input the whole of the resolution: it settles the family and the controls together, and it
/// picks the right one of several bindings the target action may have. `None` is a plan-build error
/// rather than a silent no-op — see `DiagnosticKind::FollowsNothing`.
pub(crate) fn leader_of(bindings: &[BindingSpec], index: usize) -> Option<usize> {
    let follows = bindings[index].follows?;
    bindings
        .iter()
        .enumerate()
        .find(|&(other, spec)| {
            other != index
                && spec.action == follows.action
                && spec.input == bindings[index].input
                // A binding with no mapping has none to ride, which rules out a chain of followers
                // without needing to say so separately.
                && spec.mapping.is_some()
        })
        .map(|(other, _)| other)
}

/// One control an authored [`BindingSpec`] contributes to one
/// [`ActionMapping`](crate::mapping::ActionMapping) row.
///
/// This is a fact read off an existing binding, produced by [`mapped_parts`] rather than declared
/// on its own. `binding` and `part` say exactly where it came from: which entry in the binding
/// list, and which of that binding's parts (the whole thing, for a plain control; one direction,
/// for a composite).
///
/// The order these arrive in is the order a mapping's slots fill, which is what makes the first one
/// the primary.
#[derive(Clone, Copy)]
pub(crate) struct MappedPart {
    pub(crate) key: crate::mapping::MappingKey,
    pub(crate) family: crate::device::DeviceFamily,
    /// Index into the binding list this was read from.
    pub(crate) binding: usize,
    pub(crate) part: BindingPart,
    pub(crate) control: Control,
}

/// Every mapped part of every listed binding, in slot order.
///
/// Two different passes read this instead of walking `bindings` themselves: [`mappings_of`] turns
/// it into the [`ActionMapping`](crate::mapping::ActionMapping) list a settings screen reads, and
/// [`rewrite`](crate::overrides::rewrite) walks it to find exactly which binding and part to change
/// when a player's override lands. The two must agree on what each row holds, or the player's new
/// control ends up in a slot the screen is not showing it in.
pub(crate) fn mapped_parts(bindings: &[BindingSpec]) -> Vec<MappedPart> {
    let mut parts = Vec::new();
    for (index, binding) in bindings.iter().enumerate() {
        let Some(declaration) = binding.mapping else {
            continue;
        };
        let prefix = declaration.prefix.unwrap_or(binding.path);
        binding.input.for_each_part(|part, control| {
            parts.push(MappedPart {
                key: crate::mapping::MappingKey::new(prefix, part),
                family: control.family(),
                binding: index,
                part,
                control,
            });
        });
    }
    parts
}

/// The `ActionMapping` list for one binding list: one row per mappable part.
///
/// Called on the bindings a context declares (from `mappings`) and again, unchanged, on the
/// rewritten bindings a variant plan holds once an override has been applied (from
/// `overrides::rewrite`) — same rule either way, so that a row built one way and a row rewritten
/// another can never disagree about what is bound.
///
/// Empty for a game that declares none, which is the default and costs nothing.
///
/// Bindings that derive the same key in the same family for the same action are merged into one
/// mapping holding both controls. Merging is keyed by family as well as by name, so the keyboard
/// and gamepad rows stay separate; and by action, so two different actions landing on one name is
/// still reported as a collision.
pub(crate) fn mappings_of(
    bindings: &[BindingSpec],
    context: &'static str,
) -> Vec<crate::mapping::ActionMapping> {
    let mut mappings: Vec<crate::mapping::ActionMapping> = Vec::new();
    for entry in mapped_parts(bindings) {
        let binding = &bindings[entry.binding];
        // `mapped_parts` yields nothing for a binding without one.
        let Some(declaration) = binding.mapping else {
            continue;
        };

        if let Some(mapping) = mappings.iter_mut().find(|mapping| {
            mapping.key == entry.key
                && mapping.family == entry.family
                && mapping.action == binding.action
        }) {
            mapping.slots.push(Some(entry.control));
            // Bindings that disagree about whether the player may change the row are a plan-build
            // error, so the first one's `rebind_policy` wins here only so that the value is
            // deterministic while the context is being refused.
            continue;
        }

        mappings.push(crate::mapping::ActionMapping {
            key: entry.key,
            action: binding.action,
            action_path: binding.path,
            category: binding.category,
            // A composite's part holds a button, though it reports an axis or a direction; a whole
            // binding holds whatever its own input does.
            accepts: match entry.part {
                BindingPart::Whole => binding.input.channel_shape(),
                _ => ChannelShape::Button,
            },
            family: entry.family,
            // An author cannot declare a gap, so a derived row is dense; only an override makes one.
            slots: alloc::vec![Some(entry.control)],
            rebind_policy: declaration.rebind_policy,
            context,
            followers: Vec::new(),
        });
    }

    // A second pass rather than folded into the first: a follower's row is found by the
    // *leader's* declaration, and `leader_of` wants the whole binding list resolved, not just
    // whatever has been pushed to `mappings` so far.
    for (index, binding) in bindings.iter().enumerate() {
        let Some(leader_index) = leader_of(bindings, index) else {
            // No binding to ride, or `follows` was not declared at all — either way `diagnose`
            // owns reporting it, and this pass has nothing to attach.
            continue;
        };
        let leader = &bindings[leader_index];
        // `leader_of` only returns a binding whose `mapping` is `Some`, so this always matches.
        let Some(declaration) = leader.mapping else {
            continue;
        };
        let prefix = declaration.prefix.unwrap_or(leader.path);
        let condition = crate::condition::describe(&binding.conditions);
        leader.input.for_each_part(|part, control| {
            let key = crate::mapping::MappingKey::new(prefix, part);
            if let Some(mapping) = mappings.iter_mut().find(|mapping| {
                mapping.key == key
                    && mapping.family == control.family()
                    && mapping.action == leader.action
            }) {
                // A follower rides every one of its leader's bindings, and a row with two slots is
                // two of them. Without this check it would be pushed once per slot, and a screen
                // would draw the same sub-row twice.
                if !mapping
                    .followers
                    .iter()
                    .any(|follower| follower.action == binding.action)
                {
                    mapping.followers.push(crate::mapping::Follower {
                        action: binding.action,
                        action_path: binding.path,
                        condition,
                    });
                }
            }
        });
    }
    mappings
}

/// The `Tunable` list for one binding list: one row per declared tunable.
///
/// Called on both binding lists the way `mappings_of` is, and for the same reason. The current
/// value is read straight off the modifier it targets rather than kept separately, so a row and the
/// binding it describes cannot drift apart.
pub(crate) fn tunables_of(
    bindings: &[BindingSpec],
    context: &'static str,
) -> Vec<crate::mapping::Tunable> {
    let mut tunables: Vec<crate::mapping::Tunable> = Vec::new();
    for binding in bindings {
        let Some(decl) = &binding.tunable else {
            continue;
        };
        let family = binding_family(&binding.input)
            .expect("a tunable's binding must resolve to at least one control");
        // Several bindings may declare the same key — `hold_or_toggle` reaching a primary and a
        // secondary key is the ordinary case — and they are one row to the player, not two. Every
        // sharer's value is kept in step by `rewrite` (below), so the first one found stands for
        // the whole group.
        if tunables
            .iter()
            .any(|tunable| tunable.key == decl.key && tunable.family == family)
        {
            continue;
        }
        tunables.push(crate::mapping::Tunable {
            key: decl.key,
            action: binding.action,
            action_path: binding.path,
            category: binding.category,
            family,
            context,
            value: current_tunable_value(&binding.modifiers[decl.modifier_index], decl.default),
        });
    }
    tunables
}

/// The family a binding's input belongs to.
pub(crate) fn binding_family(input: &BindingInput) -> Option<crate::device::DeviceFamily> {
    let mut family = None;
    input.for_each_part(|_, control| family = Some(control.family()));
    family
}

/// Whether this binding's raw value is always a plain press — `ActionValue::Bool` every tick, never
/// a continuous fraction — which is what `hold_or_toggle` needs to be safe: toggling a value that
/// carries real analog information would flatten it.
///
/// A key or a mouse button always qualifies — neither has anything but a press to report. A gamepad
/// button is the interesting case (R2.10): the same control reads as `Bool` when the action wants
/// a plain press and as a continuous `Axis1` fraction otherwise (see `BindingInput::GamepadButton`
/// in `eval.rs`), so it qualifies only when `intent` is `ActionIntent::Button`. A composite's part,
/// an axis or motion reports something other than `Bool` outright and never qualifies.
pub(crate) fn always_reports_bool(input: &BindingInput, intent: ActionIntent) -> bool {
    #[cfg(not(feature = "gamepad"))]
    let _ = intent;
    match input {
        #[cfg(feature = "keyboard")]
        BindingInput::Button(_) => true,
        #[cfg(feature = "mouse")]
        BindingInput::MouseButton(_) => true,
        #[cfg(feature = "gamepad")]
        BindingInput::GamepadButton(_) => intent == ActionIntent::Button,
        _ => false,
    }
}

/// Writes a tunable's new value into the modifier it targets — the write counterpart of
/// [`current_tunable_value`], used when an override is applied. A shape mismatch is a no-op, for
/// the reason given on that function.
pub(crate) fn apply_tunable_value(
    modifier: &mut BindingModifier,
    value: crate::mapping::TunableValue,
) {
    match (modifier, value) {
        (
            BindingModifier::DeadZone(dead_zone),
            crate::mapping::TunableValue::Range { value, .. },
        ) => {
            dead_zone.lower = value;
        }
        (BindingModifier::Toggle { active }, crate::mapping::TunableValue::Bool(value)) => {
            *active = value;
        }
        _ => {}
    }
}

/// Reads a tunable's live value back off the modifier it targets — the bounds travel from the
/// declared default, since a player adjusts the value but never the range it moves within.
fn current_tunable_value(
    modifier: &BindingModifier,
    default: crate::mapping::TunableValue,
) -> crate::mapping::TunableValue {
    match (modifier, default) {
        (
            BindingModifier::DeadZone(dead_zone),
            crate::mapping::TunableValue::Range { min, max, .. },
        ) => crate::mapping::TunableValue::Range {
            value: dead_zone.lower,
            min,
            max,
        },
        (BindingModifier::Toggle { active }, crate::mapping::TunableValue::Bool(_)) => {
            crate::mapping::TunableValue::Bool(*active)
        }
        // Unreachable through the crate's own API: `declare_tunable` always pairs a modifier with
        // the value shape that targets it. Falling back to the default rather than panicking is
        // cheap insurance against a future tunable-declaring method getting that pairing wrong.
        _ => default,
    }
}

#[cfg(all(test, feature = "keyboard"))]
mod tests {
    use super::*;

    use alloc::string::ToString;
    use bevy_app::App;
    use bevy_input::keyboard::KeyCode;

    use crate::binding::DirectionalButtons;
    use crate::context::ActionMapAppExt;
    use crate::{ActionMapPlugin, InputAction, InputContext};

    #[derive(InputAction)]
    #[action(path = "mapping_tests.move", output = bevy_math::Vec2, intent = Directional2, category = "mapping_tests.movement")]
    struct Move;

    #[derive(InputAction)]
    #[action(path = "mapping_tests.jump", output = bool, intent = Button)]
    struct Jump;

    #[derive(InputAction)]
    #[action(path = "mapping_tests.toggle_overlay", output = bool, intent = Button)]
    struct ToggleOverlay;

    #[derive(InputContext)]
    #[context(path = "mapping_tests.on_foot", tick = Fixed)]
    struct OnFoot;

    /// The heart of it: a composite is four rows rather than one, because a player rebinds "move
    /// forward" and a movement binding has no single control to put beside it.
    #[test]
    fn a_composite_becomes_one_slot_per_part() {
        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|controls| {
            controls.bind::<Move>(DirectionalButtons::wasd()).mappable();
            controls.bind::<Jump>(KeyCode::Space).mappable();
            // Declared `private`, so the player never sees it. Without that it would be listed and
            // would disagree with the line above about being rebindable, which is an error.
            controls.bind::<Jump>(KeyCode::Enter).private();
        });

        let mappings = mappings(app.world());
        let names: Vec<String> = mappings
            .iter()
            .map(|mapping| mapping.key.to_string())
            .collect();
        assert_eq!(
            names,
            [
                "mapping_tests.move.up",
                "mapping_tests.move.down",
                "mapping_tests.move.left",
                "mapping_tests.move.right",
                "mapping_tests.jump",
            ]
        );

        // Each part carries the controls it currently holds, which is what a read-only screen
        // shows. One apiece here: nothing declared a second mappable binding.
        assert_eq!(
            mappings[0].slots,
            [Some(Control::PhysicalKey(KeyCode::KeyW))]
        );
        assert_eq!(
            mappings[4].slots,
            [Some(Control::PhysicalKey(KeyCode::Space))]
        );

        // The category comes from the action, so the four movement rows file together.
        assert_eq!(mappings[0].category, Some("mapping_tests.movement"));
        assert_eq!(mappings[4].category, None);

        // A part of a composite holds a button whatever the composite reports as a whole.
        assert_eq!(mappings[0].accepts, ChannelShape::Button);
        assert_eq!(mappings[0].family, DeviceFamily::KeyboardMouse);
    }

    /// A tunable is enumerable the same way a mapping is, and starts at the value its own
    /// declaration gave it.
    #[test]
    fn a_declared_tunable_is_enumerable_at_its_default() {
        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.hold_or_toggle::<Jump>("mapping_tests.jump.hold_or_toggle");
        });

        let tunables = tunables(app.world());
        assert_eq!(tunables.len(), 1);
        assert_eq!(tunables[0].key, "mapping_tests.jump.hold_or_toggle");
        assert_eq!(tunables[0].action_path, "mapping_tests.jump");
        assert_eq!(tunables[0].family, DeviceFamily::KeyboardMouse);
        assert_eq!(tunables[0].value, TunableValue::Bool(false));
    }

    /// Two bindings sharing a `hold_or_toggle` key are one row to the player, not two — the
    /// presentation half of the shared-latch mechanism `hold_or_toggle` is built on.
    #[test]
    fn two_bindings_sharing_a_tunable_key_are_one_row() {
        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(KeyCode::Enter).mappable();
            controls.hold_or_toggle::<Jump>("mapping_tests.jump.hold_or_toggle");
        });

        let tunables = tunables(app.world());
        assert_eq!(tunables.len(), 1, "{tunables:?}");
    }

    /// Applying a tunable override moves `tunables`' answer and leaves `declared_tunables`' alone —
    /// the same split [`mappings`] and [`declared_mappings`] draw for a rebound control.
    #[test]
    fn applying_a_tunable_override_moves_the_current_value_only() {
        use crate::overrides::{Overrides, apply_overrides};

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.hold_or_toggle::<Jump>("mapping_tests.jump.hold_or_toggle");
        });

        let mut overrides = Overrides::new();
        overrides.tune(
            DeviceFamily::KeyboardMouse,
            "mapping_tests.jump.hold_or_toggle",
            TunableValue::Bool(true),
        );
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty(), "{problems:?}");

        assert_eq!(tunables(app.world())[0].value, TunableValue::Bool(true));
        assert_eq!(
            declared_tunables(app.world())[0].value,
            TunableValue::Bool(false),
            "what the game shipped, unmoved by the override"
        );
    }

    /// A binding nobody said anything about is *listed and fixed*: the player can read what it does
    /// and cannot change it. Seeing your controls is not something a game should have to opt into;
    /// changing them is.
    #[test]
    fn a_binding_is_listed_but_not_rebindable_by_default() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.silent", tick = Fixed)]
        struct Silent;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Silent>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
        });

        let mappings = mappings(app.world());
        assert_eq!(mappings.len(), 1);
        assert_eq!(
            mappings[0].slots,
            [Some(Control::PhysicalKey(KeyCode::Space))]
        );
        assert_eq!(mappings[0].rebind_policy, RebindPolicy::Fixed);
        assert!(!mappings[0].rebind_policy.is_rebindable());
    }

    /// `private` is the way out of the list, and the only way: a game with an internal binding it
    /// would rather not explain says so once.
    #[test]
    fn a_private_binding_is_not_listed_at_all() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.hidden", tick = Fixed)]
        struct Hidden;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Hidden>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).private();
        });

        assert!(mappings(app.world()).is_empty());
    }

    /// One binding cannot be both hidden from the player and changeable by them, and saying so is an
    /// authoring mistake catchable in the expression that makes it.
    #[test]
    #[should_panic(expected = "both `mappable` and `private`")]
    fn a_binding_cannot_be_private_and_mappable() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.contradictory", tick = Fixed)]
        struct Contradictory;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Contradictory>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable().private();
        });
    }

    /// And two bindings feeding one row cannot disagree about it either, which the builder cannot
    /// see and the plan can.
    #[test]
    #[should_panic(expected = "disagree about whether the")]
    fn two_bindings_feeding_one_mapping_cannot_disagree() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.disagreeing", tick = Fixed)]
        struct Disagreeing;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Disagreeing>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(KeyCode::Enter);
        });
    }

    #[test]
    fn a_key_reads_sensibly_without_a_catalogue() {
        let label = |key: MappingKey| key.fallback_label();

        assert_eq!(
            label(MappingKey::new("gameplay.jump", BindingPart::Whole)),
            "Jump"
        );
        assert_eq!(
            label(MappingKey::new("gameplay.move", BindingPart::Up)),
            "Move Up"
        );
        assert_eq!(
            label(MappingKey::new(
                "disasteroids.toggle_overlay",
                BindingPart::Whole
            )),
            "Toggle Overlay",
            "the namespace is for keeping keys apart, not for reading"
        );
        assert_eq!(
            label(MappingKey::new("gameplay.lean", BindingPart::Negative)),
            "Lean Negative"
        );
    }

    /// A category is a key too, and a screen that draws headings needs the same courtesy the row
    /// names get.
    #[test]
    fn a_category_reads_sensibly_without_a_catalogue() {
        assert_eq!(fallback_label("gameplay.flight"), "Flight");
        assert_eq!(fallback_label("gameplay.fine_control"), "Fine Control");
        assert_eq!(fallback_label("weapons"), "Weapons");
        assert_eq!(fallback_label(""), "");
    }

    /// Two mappable bindings of one action in one family are a default primary and secondary, which
    /// is how a shipped game writes that — so they merge into one row holding two controls rather
    /// than becoming two rows both called Jump.
    #[test]
    fn two_bindings_of_one_action_share_one_slot() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.two_defaults", tick = Fixed)]
        struct TwoDefaults;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<TwoDefaults>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(KeyCode::Enter).mappable();
        });

        let mappings = mappings(app.world());
        assert_eq!(mappings.len(), 1, "one row, not two");
        assert_eq!(
            mappings[0].slots,
            [
                Some(Control::PhysicalKey(KeyCode::Space)),
                Some(Control::PhysicalKey(KeyCode::Enter))
            ],
            "in the order they were declared, which is what makes the first one primary"
        );
    }

    /// The collision that survives the merge above: *different* actions answering to one name, where
    /// a saved rebinding of one would land on the other.
    #[test]
    #[should_panic(expected = "declares a mapping named")]
    fn two_actions_cannot_share_a_slot_name() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.colliding", tick = Fixed)]
        struct Colliding;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Colliding>(|controls| {
            controls
                .bind::<Jump>(KeyCode::Space)
                .mappable_as("mapping_tests.go");
            controls
                .bind::<ToggleOverlay>(KeyCode::Enter)
                .mappable_as("mapping_tests.go");
        });
    }

    /// The same name in two families is not a collision, and this is the ordinary way to write a
    /// game that offers rebinding on both devices: one key and one button, both mappable, both
    /// called `jump`. They land in separate tables, so nothing can be confused for anything.
    #[cfg(feature = "gamepad")]
    #[test]
    fn one_name_in_two_families_is_two_rows_rather_than_a_collision() {
        use bevy_input::gamepad::GamepadButton;

        #[derive(InputContext)]
        #[context(path = "mapping_tests.both_devices", tick = Fixed)]
        struct BothDevices;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<BothDevices>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(GamepadButton::South).mappable();
        });

        let mappings = mappings(app.world());
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].key, mappings[1].key, "one name…");
        assert_eq!(mappings[0].family, DeviceFamily::KeyboardMouse);
        assert_eq!(mappings[1].family, DeviceFamily::Gamepad, "…two families");
    }

    /// A second `mappable*` on one binding must not quietly discard the name the first one gave it,
    /// in either order.
    #[test]
    fn naming_a_slot_survives_a_second_mappable() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.named_twice", tick = Fixed)]
        struct NamedTwice;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<NamedTwice>(|controls| {
            controls
                .bind::<Jump>(KeyCode::Space)
                .mappable_as("mapping_tests.leap")
                .mappable();
            controls
                .bind::<ToggleOverlay>(KeyCode::F1)
                .mappable()
                .mappable_as("mapping_tests.peek");
        });

        let mappings = mappings(app.world());
        assert_eq!(mappings[0].key.to_string(), "mapping_tests.leap");
        assert_eq!(mappings[1].key.to_string(), "mapping_tests.peek");
    }

    /// And the same collision across two contexts, which no single plan can see.
    #[test]
    #[should_panic(expected = "already uses")]
    fn one_action_mappable_in_two_contexts_collides() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.first", tick = Fixed)]
        struct First;

        #[derive(InputContext)]
        #[context(path = "mapping_tests.second", tick = Fixed)]
        struct Second;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<First>(|controls| {
            controls.bind::<ToggleOverlay>(KeyCode::F1).mappable();
        });
        app.add_context::<Second>(|controls| {
            controls.bind::<ToggleOverlay>(KeyCode::F2).mappable();
        });
    }

    /// The remedy the diagnostic names, and proof it works.
    #[test]
    fn a_name_of_your_own_settles_a_collision() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.renamed_a", tick = Fixed)]
        struct RenamedA;

        #[derive(InputContext)]
        #[context(path = "mapping_tests.renamed_b", tick = Fixed)]
        struct RenamedB;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<RenamedA>(|controls| {
            controls.bind::<Move>(DirectionalButtons::wasd()).mappable();
        });
        app.add_context::<RenamedB>(|controls| {
            controls
                .bind::<Move>(DirectionalButtons::arrow_keys())
                .mappable_as("mapping_tests.move_alt");
        });

        let names: Vec<String> = mappings(app.world())
            .iter()
            .map(|mapping| mapping.key.to_string())
            .collect();
        assert!(names.contains(&String::from("mapping_tests.move.up")));
        assert!(names.contains(&String::from("mapping_tests.move_alt.up")));
    }

    #[derive(InputAction)]
    #[action(path = "mapping_tests.lunge", output = bool, intent = Button)]
    struct Lunge;

    /// The point of the whole thing: a second action on one control is one row, and the row is the
    /// principal's. Two rows holding identical keys is what the player would otherwise be shown, and
    /// it reads as a bug.
    #[test]
    fn a_follower_is_not_a_row_of_its_own() {
        use crate::action::InputAction as _;

        #[derive(InputContext)]
        #[context(path = "mapping_tests.riding", tick = Fixed)]
        struct Riding;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Riding>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.follow::<Lunge, Jump>(|binding| binding.hold(0.4));
        });

        let mappings = mappings(app.world());
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].key.to_string(), "mapping_tests.jump");
        assert_eq!(
            mappings[0].slots,
            [Some(Control::PhysicalKey(KeyCode::Space))],
            "a follower contributes no control of its own to the row it rides"
        );

        // Not a row, but not invisible either: the row it rides knows it is there.
        assert_eq!(mappings[0].followers.len(), 1);
        let follower = &mappings[0].followers[0];
        assert_eq!(follower.action, Lunge::id());
        assert_eq!(follower.action_path, "mapping_tests.lunge");
        assert_eq!(follower.fallback_label(), "Lunge");
    }

    /// A follower's hold is what a screen draws beside the row it rides, and it travels on the
    /// follower rather than being derived again from the world.
    #[test]
    fn a_follower_carries_its_own_condition() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.riding_held", tick = Fixed)]
        struct RidingHeld;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<RidingHeld>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.follow::<Lunge, Jump>(|binding| binding.hold(0.4));
        });

        let mappings = mappings(app.world());
        assert_eq!(
            mappings[0].followers[0].condition,
            ConditionDescriptor::Hold { duration: 0.4 }
        );
    }

    /// `follow` reads every binding `Jump` has by the time it runs, so an action bound on both
    /// devices gets one follower per device from a single call, with neither side naming one.
    #[test]
    fn a_follower_covers_every_binding_the_leader_has() {
        use bevy_input::gamepad::GamepadButton;

        #[derive(InputContext)]
        #[context(path = "mapping_tests.riding_both", tick = Fixed)]
        struct RidingBoth;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<RidingBoth>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(GamepadButton::South).mappable();
            controls.follow::<Lunge, Jump>(|binding| binding.hold(0.4));
        });

        // Two rows, one per family, and neither of them is Lunge's.
        let mappings = mappings(app.world());
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].family, DeviceFamily::KeyboardMouse);
        assert_eq!(mappings[1].family, DeviceFamily::Gamepad);
        assert!(
            mappings
                .iter()
                .all(|mapping| mapping.action_path == "mapping_tests.jump")
        );
    }

    /// Disasteroids' actual shape: one row with a primary and a secondary, and a follower riding
    /// both slots from the one `follow` call that declared it. The row is one thing to the player,
    /// so it gets one sub-row, not two identical ones — and there is no way through `follow` to
    /// declare a rider on one slot and not the other.
    #[test]
    fn a_follower_riding_every_slot_of_a_row_is_still_one_sub_row() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.riding_every_slot", tick = Fixed)]
        struct RidingEverySlot;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<RidingEverySlot>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(KeyCode::KeyJ).mappable();
            controls.follow::<Lunge, Jump>(|binding| binding.hold(0.4));
        });

        let mappings = mappings(app.world());
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].slots.len(), 2);
        assert_eq!(mappings[0].followers.len(), 1);
    }

    /// Following a row the player cannot change is allowed: there is nothing to rewrite, and
    /// keeping the duplicate off the screen is the whole of what it buys.
    #[test]
    fn a_fixed_binding_can_be_followed() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.riding_fixed", tick = Fixed)]
        struct RidingFixed;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<RidingFixed>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
            controls.follow::<Lunge, Jump>(|binding| binding.hold(0.4));
        });

        let mappings = mappings(app.world());
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].rebind_policy, RebindPolicy::Fixed);
    }

    /// `follow` runs against whatever the leader has declared *so far*, not its final shape — the
    /// ordering rule that lets a follower ride only some of a leader's devices on purpose. Called
    /// before `Jump` has any binding at all, there is nothing yet to ride.
    #[test]
    #[should_panic(expected = "has no bindings")]
    fn following_an_action_with_no_bindings_yet_is_refused() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.riding_nothing", tick = Fixed)]
        struct RidingNothing;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<RidingNothing>(|controls| {
            controls.follow::<Lunge, Jump>(|binding| binding.hold(0.4));
        });
    }

    /// A separate refusal because the fix is on the *other* binding: there is a binding reading the
    /// same control, and it has no mapping to lend.
    #[test]
    #[should_panic(expected = "off the controls screen")]
    fn following_a_private_binding_is_refused() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.riding_a_ghost", tick = Fixed)]
        struct RidingAGhost;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<RidingAGhost>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).private();
            controls.follow::<Lunge, Jump>(|binding| binding.hold(0.4));
        });
    }

    /// A binding has a mapping or rides one, and saying both is catchable where it is written.
    /// `follow` declares `follows` before `configure` runs, so this is the only order reachable
    /// through it — asking for `mappable` inside `configure` finds the conflict already there.
    #[test]
    #[should_panic(expected = "both `follows` and `mappable`")]
    fn a_binding_cannot_follow_and_be_mappable() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.owning_and_riding", tick = Fixed)]
        struct OwningAndRiding;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<OwningAndRiding>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.follow::<Lunge, Jump>(|binding| binding.mappable());
        });
    }

    /// Following your own action is riding the mapping you are declaring, which has no coherent
    /// reading at all.
    #[test]
    #[should_panic(expected = "cannot follow its own action")]
    fn a_binding_cannot_follow_its_own_action() {
        #[derive(InputContext)]
        #[context(path = "mapping_tests.riding_itself", tick = Fixed)]
        struct RidingItself;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<RidingItself>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.follow::<Jump, Jump>(|binding| binding);
        });
    }
}
