//! What the player changed, and putting it back into a running game.
//!
//! Everything else in this crate describes what a game *declared*. This describes what a player did
//! to it afterwards — a set of rows saying "move forward is `E` now" — and the one call that makes a
//! running game agree with it.
//!
//! ```ignore
//! let mut overrides = Overrides::new();
//! overrides.bind(Scheme::KeyboardMouse, forward.key, [Control::Key(KeyCode::KeyE)]);
//!
//! // Every context, every instance, effective immediately.
//! let problems = apply_overrides(world, &overrides);
//! ```
//!
//! # It is a diff, not a snapshot
//!
//! A row that is absent means "whatever the game shipped", so revising a default binding in a patch
//! reaches every player who never touched that row. That only works if the declared bindings survive
//! being overridden, and they do: applying compiles a *variant* of the declared plan and leaves the
//! declaration where it was. [`mappings`](crate::mapping::mappings) then answers what is bound now
//! and [`declared_mappings`](crate::mapping::declared_mappings) answers what the game shipped, which
//! is what a "reset to default" button compares against.
//!
//! Because absence already means the default, clearing a binding needs a value of its own — see
//! [`Override`], which has three.
//!
//! # Where it lives is yours
//!
//! [`Overrides`] is a plain value, not a resource. Put it in your own settings resource, hand it to
//! a settings screen as a working copy, send it to an account service, write it to a file. The crate
//! defines the structure and applies it, and has no opinion about the rest.
//!
//! # Applying never fails
//!
//! A saved override set outlives the build that wrote it, so it can name a mapping this build no
//! longer has or a control that no longer fits. Those rows are skipped and **reported** rather than
//! dropped in silence — [`apply_overrides`] hands back an [`OverrideProblem`] per row it could not
//! use, and applies everything else.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use bevy_ecs::world::World;

use crate::action::ChannelShape;
use crate::binding::{BindingSpec, Control, MappedPart, mapped_parts};
use crate::capture::ControlClass;
use crate::mapping::{Capacity, Mapping, MappingKey, Scheme};

/// What a player did to one mapping.
///
/// Three states rather than two, because a diff against defaults makes *absence* meaningful: once a
/// missing row already says "use the default", a player who deliberately emptied a row has nothing
/// left to say with unless emptying has a value of its own.
#[derive(Clone, Debug, PartialEq)]
pub enum Override {
    /// The controls the player put in the mapping, in slot order.
    ///
    /// Position is which slot, so this is written and read in order: the first is the primary. It
    /// replaces the mapping's whole list rather than one position in it — a screen that edits a
    /// single cell edits the list and then writes the row.
    Controls(Vec<Control>),
    /// The player deliberately emptied the mapping.
    ///
    /// The action stays declared and stays readable; nothing fires it. Distinct from a missing row,
    /// which means the game's own default still applies.
    Cleared,
    /// Something outside this crate owns this mapping.
    ///
    /// A backend authoritative for the action owns its bindings and its own rebinding UI, so this
    /// crate neither applies a control here nor treats the row as one the player emptied.
    NotOurs,
}

/// Everything a player has changed, as a diff against what the game declared.
///
/// Rows are keyed by mapping and by scheme, because a mapping name is unique within a scheme and a
/// keyboard remap must not disturb the gamepad layout. Nothing here names a device: what a player
/// bound is a control on a device *class*, and which physical unit drives which player is a separate
/// question with a separate answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    rows: BTreeMap<(Scheme, MappingKey), Override>,
}

impl Overrides {
    /// An empty set, which is a game running on exactly what it declared.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the player has changed nothing.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Puts controls in a mapping.
    ///
    /// The whole list, in slot order. An empty list is [`Override::Cleared`] and is stored as such,
    /// since a row holding nothing and a row that is not there mean different things.
    pub fn bind(
        &mut self,
        scheme: Scheme,
        mapping: MappingKey,
        controls: impl IntoIterator<Item = Control>,
    ) {
        let controls: Vec<Control> = controls.into_iter().collect();
        self.set(
            scheme,
            mapping,
            if controls.is_empty() {
                Override::Cleared
            } else {
                Override::Controls(controls)
            },
        );
    }

    /// Sets a row directly, for the two states [`bind`](Self::bind) cannot express.
    pub fn set(&mut self, scheme: Scheme, mapping: MappingKey, value: Override) {
        self.rows.insert((scheme, mapping), value);
    }

    /// What the player did to one mapping, or `None` where they left it alone.
    pub fn get(&self, scheme: Scheme, mapping: MappingKey) -> Option<&Override> {
        self.rows.get(&(scheme, mapping))
    }

    /// Every row, in a stable order.
    pub fn iter(&self) -> impl Iterator<Item = (Scheme, MappingKey, &Override)> {
        self.rows
            .iter()
            .map(|(&(scheme, key), value)| (scheme, key, value))
    }

    /// Puts one mapping back to what the game declared.
    ///
    /// Removing the row *is* the reset, which is the whole benefit of storing a diff.
    pub fn reset(&mut self, scheme: Scheme, mapping: MappingKey) {
        self.rows.remove(&(scheme, mapping));
    }

    /// Puts every mapping of one action back to what the game declared.
    ///
    /// Takes the mapping list because a row is keyed by mapping alone, and which mappings belong to
    /// an action is a fact about the declaration rather than about the diff. An action bound to a
    /// composite has one row per direction, and this resets all of them.
    pub fn reset_action(&mut self, mappings: &[Mapping], action: crate::action::ActionId) {
        self.reset_matching(mappings, |mapping| mapping.action == action);
    }

    /// Puts every mapping declared in one context back to what the game declared.
    ///
    /// `context` is the path the context declared, which is what
    /// [`Mapping::context`](crate::mapping::Mapping::context) carries.
    pub fn reset_context(&mut self, mappings: &[Mapping], context: &str) {
        self.reset_matching(mappings, |mapping| mapping.context == context);
    }

    /// Puts everything back to what the game declared.
    pub fn reset_all(&mut self) {
        self.rows.clear();
    }

    fn reset_matching(&mut self, mappings: &[Mapping], keep: impl Fn(&Mapping) -> bool) {
        for mapping in mappings.iter().filter(|mapping| keep(mapping)) {
            self.reset(mapping.scheme, mapping.key);
        }
    }
}

/// A row an override set named that could not be used, and why.
///
/// Reported rather than dropped: a saved set outlives the build that wrote it, and a player whose
/// binding quietly vanished is owed better than silence.
#[derive(Clone, Debug, PartialEq)]
pub struct OverrideProblem {
    /// The scheme the row was filed under.
    pub scheme: Scheme,
    /// The mapping the row named.
    pub mapping: MappingKey,
    /// What was wrong with it.
    pub kind: OverrideProblemKind,
}

/// What was wrong with an override row.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum OverrideProblemKind {
    /// No mapping of that name in that scheme is declared any more.
    ///
    /// What a renamed or removed binding looks like from inside a file written by an older build.
    NoSuchMapping,
    /// The mapping exists and the player may not change it.
    NotRebindable,
    /// A control belongs to the other control scheme.
    ///
    /// A mapping is rebound within its own scheme, so a gamepad button cannot fill a keyboard row.
    WrongScheme {
        /// The control that does not belong.
        control: Control,
    },
    /// A control reports on a channel the mapping's action cannot use.
    WrongShape {
        /// The control that does not fit.
        control: Control,
        /// What the mapping accepts.
        accepts: ChannelShape,
    },
    /// A binding reserved one of the controls, so nothing may be bound over it.
    Reserved {
        /// The reserved control.
        control: Control,
    },
    /// More controls than the mapping has slots for.
    TooManyControls {
        /// How many the mapping holds.
        capacity: Capacity,
        /// How many the row named.
        given: usize,
    },
    /// The row is one direction of a composite, and the game shipped no second composite to put
    /// another control in.
    ///
    /// A movement binding is four keys, and a second "move forward" key is one part of a second set
    /// of four — so a row like this grows only when the whole composite does. Ship the alternative
    /// as a second `mappable` binding of the same action and the player gets a filled second slot on
    /// all four rows at once, which is how a keyboard table with two columns is actually written.
    CompositeCannotGrow,
}

/// Makes a running game agree with an override set.
///
/// Every context, every instance, effective on the next tick. This is the only way an override
/// reaches a game, and loading a saved set at startup is simply the first call — there is no
/// separate startup path, because a backend that owns its bindings can rewrite them while the game
/// runs and a startup-only path would be wrong on the platform that needs it most.
///
/// Applying an override to a context cancels whatever it had in flight and makes each of its actions
/// wait to be seen at rest once, exactly as switching the context off and on again does. A player
/// holding the key they just rebound does not get a fresh press out of it.
///
/// Rows this build cannot use come back as [`OverrideProblem`]s; everything else is applied.
pub fn apply_overrides(world: &mut World, overrides: &Overrides) -> Vec<OverrideProblem> {
    apply_with(world, overrides, None)
}

/// Like [`apply_overrides`], but a preset's rows are exempt from the "not rebindable here"
/// refusal — the one way a `Fixed` row (every gamepad binding in a typical game) still moves.
///
/// `overrides` is the whole working copy to apply — a preset's rows and any manual captures
/// together, since applying always starts from the pristine declaration and a second call does
/// not layer onto the first. `preset` is only consulted to decide which of those rows may bypass
/// [`OverrideProblemKind::NotRebindable`]; pass the selected preset's own rows, or an empty
/// [`Overrides`] if none is selected.
pub fn apply_overrides_with_preset(
    world: &mut World,
    overrides: &Overrides,
    preset: &Overrides,
) -> Vec<OverrideProblem> {
    apply_with(world, overrides, Some(preset))
}

fn apply_with(
    world: &mut World,
    overrides: &Overrides,
    preset: Option<&Overrides>,
) -> Vec<OverrideProblem> {
    let Some(declared) = world.get_resource::<crate::inspect::DeclaredContexts>() else {
        return Vec::new();
    };
    // Collected first because each one takes the world exclusively in turn.
    let appliers: Vec<_> = declared.0.iter().map(|context| context.apply).collect();

    let mut problems = Vec::new();
    for apply in appliers {
        problems.extend(apply(world, overrides, preset));
    }

    // Reported here rather than per context, because "no context declares this" is the only form
    // the question has an answer in: from inside any one context every other context's rows look
    // exactly as missing as a row that is genuinely gone.
    let declared = crate::mapping::declared_mappings(world);
    problems.extend(
        overrides
            .iter()
            .filter(|&(scheme, key, _)| {
                !declared
                    .iter()
                    .any(|row| row.key == key && row.scheme == scheme)
            })
            .map(|(scheme, mapping, _)| OverrideProblem {
                scheme,
                mapping,
                kind: OverrideProblemKind::NoSuchMapping,
            }),
    );

    // Every prompt on screen may now name a different control, which is the one thing about a
    // rebind that is invisible until someone rebinds with a HUD up.
    crate::present::PromptGeneration::bump(world);
    problems
}

/// The pure half: authored bindings and an override set in, rewritten bindings and rows out.
///
/// Separate from the ECS work so that it can be reasoned about and tested without a `World`, and
/// because §10.1 asks for exactly this function.
pub(crate) fn rewrite(
    declared: &[BindingSpec],
    rows: &[Mapping],
    overrides: &Overrides,
    preset: Option<&Overrides>,
    reserved: &[Control],
    context: &'static str,
) -> (Vec<BindingSpec>, Vec<Mapping>, Vec<OverrideProblem>) {
    let mut variant = declared.to_vec();
    let mut problems = Vec::new();
    let mut dropped = alloc::collections::BTreeSet::new();
    let mut grown: Vec<BindingSpec> = Vec::new();

    // Both computed against the *declared* bindings and never re-derived as we go: `leader_of`
    // matches a follower to its leader by the controls the two read, so once a source has been
    // rewritten the two no longer look alike and the link would be lost half way through the pass.
    let parts = mapped_parts(declared);
    let leaders: Vec<Option<usize>> = (0..declared.len())
        .map(|index| crate::binding::leader_of(declared, index))
        .collect();

    for row in rows {
        let Some(over) = overrides.get(row.scheme, row.key) else {
            continue;
        };
        let wanted: &[Control] = match over {
            // The defaults stand, and deliberately are not read as an empty row: nobody cleared
            // this, somebody else owns it.
            Override::NotOurs => continue,
            Override::Cleared => &[],
            Override::Controls(controls) => controls,
        };

        let contributors: Vec<_> = parts
            .iter()
            .filter(|part| {
                part.key == row.key
                    && part.scheme == row.scheme
                    && declared[part.binding].action == row.action
            })
            .collect();

        let preset_authorized =
            preset.is_some_and(|preset| preset.get(row.scheme, row.key).is_some());
        if let Some(kind) = refusal(
            row,
            wanted,
            reserved,
            &contributors,
            declared,
            preset_authorized,
        ) {
            problems.push(OverrideProblem {
                scheme: row.scheme,
                mapping: row.key,
                kind,
            });
            continue;
        }

        for (slot, &control) in wanted.iter().enumerate() {
            match contributors.get(slot) {
                // A slot the defaults already fill: the binding stays where it is and reads
                // something else.
                Some(part) => {
                    variant[part.binding].source.set_part(part.part, control);
                    rewrite_followers(declared, &leaders, &mut variant, part.binding);
                }
                // A slot the game shipped nothing for — the empty secondary of a `mappable_upto(2)`
                // row. The last binding feeding the row is cloned onto the new control, so the
                // secondary behaves like the primary rather than like a bare source with no
                // modifiers or conditions on it.
                None => {
                    let Some(last) = contributors.last() else {
                        continue;
                    };
                    grown.push(clone_onto(&variant[last.binding], last.part, control));
                    for (follower, _) in
                        followers_of(declared, &leaders, last.binding).collect::<Vec<_>>()
                    {
                        grown.push(clone_onto(&variant[follower], last.part, control));
                    }
                }
            }
        }

        // Slots the row no longer has. The binding goes rather than being left reading something
        // stale, and its followers go with it — a rider whose leader is gone has nothing to ride.
        for part in contributors.iter().skip(wanted.len()) {
            dropped.insert(part.binding);
            dropped.extend(followers_of(declared, &leaders, part.binding).map(|(index, _)| index));
        }
    }

    variant.extend(grown);
    let variant: Vec<BindingSpec> = variant
        .into_iter()
        .enumerate()
        .filter(|(index, _)| !dropped.contains(index))
        .map(|(_, binding)| binding)
        .collect();

    let current = current_rows(&variant, rows, context);
    (variant, current, problems)
}

/// Why this row cannot take these controls, if it cannot.
///
/// The whole row is refused rather than partly applied: half a rebind is worse than none, and the
/// player still has the default. `preset_authorized` is the one exception to the rebindable-only
/// rule below it: a preset moves a `Fixed` row on purpose, which is the whole point of one.
fn refusal(
    row: &Mapping,
    wanted: &[Control],
    reserved: &[Control],
    contributors: &[&MappedPart],
    declared: &[BindingSpec],
    preset_authorized: bool,
) -> Option<OverrideProblemKind> {
    if !row.rebinding.is_rebindable() && !preset_authorized {
        return Some(OverrideProblemKind::NotRebindable);
    }
    // Capacity first: "this row has one slot" is both simpler and truer than anything below it
    // about why a second control has nowhere to go.
    if let Capacity::UpTo(limit) = row.capacity
        && wanted.len() > limit
    {
        return Some(OverrideProblemKind::TooManyControls {
            capacity: row.capacity,
            given: wanted.len(),
        });
    }
    // A slot the defaults left empty is filled by copying the binding beside it, and that only
    // works where the binding reads one control. Copy a *composite* and its other parts land in
    // their own rows a second time — "Move Down: S | S", a wrong screen rather than an untidy one.
    if wanted.len() > contributors.len()
        && let Some(last) = contributors.last()
        && parts_in(&declared[last.binding].source) > 1
    {
        return Some(OverrideProblemKind::CompositeCannotGrow);
    }
    // `None` is a mapping no single control can fill — a stick or a mouse bound whole — which is
    // never something a capture offered, so a row naming one came from somewhere else.
    let accepts = ControlClass::of(row.accepts);
    for &control in wanted {
        if control.scheme() != row.scheme {
            return Some(OverrideProblemKind::WrongScheme { control });
        }
        if !accepts.is_some_and(|class| class.contains(control)) {
            return Some(OverrideProblemKind::WrongShape {
                control,
                accepts: row.accepts,
            });
        }
        if reserved.contains(&control) {
            return Some(OverrideProblemKind::Reserved { control });
        }
    }
    None
}

/// How many player-facing rows one binding feeds: one for a plain control, four for a directional
/// composite.
fn parts_in(source: &crate::binding::BindingSource) -> usize {
    let mut count = 0;
    source.for_each_part(|_, _| count += 1);
    count
}

/// Every binding riding `leader`'s mapping, and the slot of the leader list it was found at.
fn followers_of<'a>(
    declared: &'a [BindingSpec],
    leaders: &'a [Option<usize>],
    leader: usize,
) -> impl Iterator<Item = (usize, &'a BindingSpec)> {
    leaders
        .iter()
        .enumerate()
        .filter(move |&(_, resolved)| *resolved == Some(leader))
        .map(|(index, _)| (index, &declared[index]))
}

/// Moves every rider of `leader` onto the control the leader just took.
///
/// The half chunk 44 left undone. Without it a rebind separates two actions that were declared to
/// share a control: the throttle moves and the afterburner stays on the old key, where whatever the
/// player binds next quietly acquires an afterburner.
fn rewrite_followers(
    declared: &[BindingSpec],
    leaders: &[Option<usize>],
    variant: &mut [BindingSpec],
    leader: usize,
) {
    let riders: Vec<usize> = followers_of(declared, leaders, leader)
        .map(|(index, _)| index)
        .collect();
    // A follower reads exactly what its leader reads — that identity is how the link was resolved
    // in the first place — so keeping it true is an assignment rather than a second rewrite.
    for rider in riders {
        variant[rider].source = variant[leader].source;
    }
}

/// A copy of `binding` reading `control` in place of the control at `part`.
fn clone_onto(binding: &BindingSpec, part: crate::binding::Part, control: Control) -> BindingSpec {
    let mut grown = binding.clone();
    grown.source.set_part(part, control);
    grown
}

/// The player-facing rows for a variant, keyed to the declared ones.
///
/// Derived from the rewritten bindings rather than patched, so the rows and the plan cannot disagree
/// about what is bound — with one exception the derivation cannot express on its own: a row the
/// player emptied has no bindings left and so derives nothing at all. It has to stay on the screen,
/// holding nothing, or there is nowhere to bind it back.
fn current_rows(
    variant: &[BindingSpec],
    declared: &[Mapping],
    context: &'static str,
) -> Vec<Mapping> {
    let derived = crate::binding::mappings_of(variant, context);
    declared
        .iter()
        .map(|row| {
            derived
                .iter()
                .find(|current| {
                    current.key == row.key
                        && current.scheme == row.scheme
                        && current.action == row.action
                })
                .cloned()
                .unwrap_or_else(|| Mapping {
                    slots: Vec::new(),
                    followers: row.followers.clone(),
                    ..row.clone()
                })
        })
        .collect()
}

#[cfg(all(test, feature = "keyboard"))]
mod tests {
    use super::*;

    use alloc::string::ToString;
    use bevy_app::App;
    use bevy_ecs::entity::Entity;
    use bevy_input::keyboard::KeyCode;

    use crate::action::{InputAction as _, Phase};
    use crate::binding::DirectionalButtons;
    use crate::context::{ActionMapAppExt, InputContextState};
    use crate::mapping::{Rebinding, declared_mappings, mappings};
    use crate::present::{BindingTable, PromptScope, Prompts as _};
    use crate::{ActionMapPlugin, InputAction, InputContext};

    #[derive(InputAction)]
    #[action(path = "override_tests.move", output = bevy_math::Vec2, intent = Directional2)]
    struct Move;

    #[derive(InputAction)]
    #[action(path = "override_tests.jump", output = bool, intent = Button)]
    struct Jump;

    #[derive(InputAction)]
    #[action(path = "override_tests.lunge", output = bool, intent = Button)]
    struct Lunge;

    #[derive(InputAction)]
    #[action(path = "override_tests.look", output = bevy_math::Vec2, intent = Delta2)]
    struct Look;

    #[derive(InputAction)]
    #[action(path = "override_tests.settings", output = bool, intent = Button)]
    struct OpenSettings;

    #[derive(InputContext)]
    #[context(path = "override_tests.playing", tick = Render)]
    struct Playing;

    /// `Move` on WASD (four rows), `Jump` on Space with room for a secondary and a `Lunge` riding
    /// it, `Look` on the mouse (listed, unchangeable), and a reserved settings key.
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Playing>(|controls| {
            controls.bind::<Move>(DirectionalButtons::wasd()).mappable();
            controls.bind::<Jump>(KeyCode::Space).mappable_upto(2);
            controls.follow::<Lunge, Jump>(|binding| binding.hold(0.4));
            controls.bind::<Look>(crate::binding::MouseMove);
            controls.bind::<OpenSettings>(KeyCode::F1).reserved();
        });
        app
    }

    fn row(app: &App, name: &str) -> Mapping {
        mappings(app.world())
            .into_iter()
            .find(|mapping| mapping.key.to_string() == name)
            .unwrap_or_else(|| panic!("no mapping named {name}"))
    }

    fn slots(app: &App, name: &str) -> Vec<Control> {
        row(app, name).slots
    }

    fn bind(app: &App, name: &str, controls: &[Control]) -> Overrides {
        let target = row(app, name);
        let mut overrides = Overrides::new();
        overrides.bind(target.scheme, target.key, controls.iter().copied());
        overrides
    }

    /// The whole point of the chunk: a row the player changed reads back changed.
    #[test]
    fn applying_an_override_moves_a_row() {
        let mut app = app();
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW)]
        );

        let overrides = bind(
            &app,
            "override_tests.move.up",
            &[Control::Key(KeyCode::KeyI)],
        );
        let problems = apply_overrides(app.world_mut(), &overrides);

        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyI)]
        );
        // And only that part of the composite: the other three keys are where they were.
        assert_eq!(
            slots(&app, "override_tests.move.left"),
            [Control::Key(KeyCode::KeyA)]
        );
    }

    /// R17.1. A diff has to be taken against the defaults, so the defaults have to still be there
    /// after the first apply — otherwise a revised default never again reaches a player who has
    /// changed anything.
    #[test]
    fn the_defaults_survive_being_overridden() {
        let mut app = app();
        let overrides = bind(
            &app,
            "override_tests.move.up",
            &[Control::Key(KeyCode::KeyI)],
        );
        apply_overrides(app.world_mut(), &overrides);

        let declared = declared_mappings(app.world())
            .into_iter()
            .find(|mapping| mapping.key.to_string() == "override_tests.move.up")
            .expect("the row is still declared");
        assert_eq!(declared.slots, [Control::Key(KeyCode::KeyW)], "still W");

        // And applying a second time is a diff against the same defaults, not against the first
        // apply — so going back to a row nobody overrode restores the shipped control.
        apply_overrides(app.world_mut(), &Overrides::new());
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW)]
        );
    }

    /// R19.16, and the half chunk 44 left undone. `Lunge` is `Jump` held; rebinding Jump has to take
    /// Lunge with it, or the two actions the game declared as sharing a control stop sharing one.
    #[test]
    fn a_follower_moves_with_the_row_it_rides() {
        let mut app = app();
        // A prompt is a runtime question, so something has to be carrying the context
        // before it has an answer at all.
        app.world_mut().spawn(Playing);
        let overrides = bind(&app, "override_tests.jump", &[Control::Key(KeyCode::KeyK)]);
        apply_overrides(app.world_mut(), &overrides);

        let jump = row(&app, "override_tests.jump");
        assert_eq!(jump.slots, [Control::Key(KeyCode::KeyK)]);
        // The follower is still on the row rather than orphaned onto a row of its own...
        assert_eq!(jump.followers.len(), 1);
        assert_eq!(jump.followers[0].action, Lunge::id());
        // ...and it is the new control it reads, which is the half that is a gameplay bug when it
        // is missing.
        let fires = BindingTable::new(app.world());
        let prompts = fires.prompts(Lunge::id(), PromptScope::ANY);
        assert_eq!(prompts.len(), 1);
        assert_eq!(
            prompts[0].origin.control(),
            Some(Control::Key(KeyCode::KeyK))
        );
    }

    /// R17.7's middle state. Clearing is not the same as never having touched the row: the action
    /// stays declared and readable, and nothing fires it.
    #[test]
    fn a_cleared_row_leaves_the_action_bound_but_silent() {
        let mut app = app();
        let target = row(&app, "override_tests.jump");
        let mut overrides = Overrides::new();
        overrides.set(target.scheme, target.key, Override::Cleared);
        apply_overrides(app.world_mut(), &overrides);

        // The row is still on the screen, holding nothing — or there would be nowhere to bind it
        // back from.
        let jump = row(&app, "override_tests.jump");
        assert!(jump.slots.is_empty());
        assert_eq!(jump.rebinding, Rebinding::Here);

        // And the action still has a slot, so reading it is a rest value rather than the "not bound
        // in this context" warning, which is a typo diagnostic and not what happened.
        let entity = app.world_mut().spawn(Playing).id();
        let state = app
            .world()
            .get::<InputContextState<Playing>>(entity)
            .unwrap();
        assert!(
            state.is_bound::<Jump>(),
            "unbound is not the same as cleared"
        );
        assert!(!state.value::<Jump>());
    }

    /// A row the game shipped one default for and left room in. The new binding is a copy of the one
    /// beside it, so a secondary behaves like the primary rather than like a bare control with the
    /// conditions stripped off it.
    #[test]
    fn a_grown_slot_copies_the_binding_beside_it() {
        let mut app = app();
        // A prompt is a runtime question, so something has to be carrying the context
        // before it has an answer at all.
        app.world_mut().spawn(Playing);
        let overrides = bind(
            &app,
            "override_tests.jump",
            &[Control::Key(KeyCode::Space), Control::Key(KeyCode::KeyK)],
        );
        apply_overrides(app.world_mut(), &overrides);

        assert_eq!(
            slots(&app, "override_tests.jump"),
            [Control::Key(KeyCode::Space), Control::Key(KeyCode::KeyK)]
        );
        // The follower rides both, and is still one sub-row rather than two.
        let jump = row(&app, "override_tests.jump");
        assert_eq!(jump.followers.len(), 1);
        let prompts = BindingTable::new(app.world()).prompts(Lunge::id(), PromptScope::ANY);
        assert_eq!(
            prompts
                .iter()
                .map(|prompt| prompt.origin.control())
                .collect::<Vec<_>>(),
            [
                Some(Control::Key(KeyCode::Space)),
                Some(Control::Key(KeyCode::KeyK))
            ],
            "the rider was copied onto the new slot too"
        );
    }

    /// And the other direction: a row that had two and now has one drops the right binding, and
    /// takes the rider on it with it.
    #[test]
    fn a_shortened_row_drops_the_binding_it_no_longer_has() {
        let mut app = app();
        // A prompt is a runtime question, so something has to be carrying the context
        // before it has an answer at all.
        app.world_mut().spawn(Playing);
        let grown = bind(
            &app,
            "override_tests.jump",
            &[Control::Key(KeyCode::Space), Control::Key(KeyCode::KeyK)],
        );
        apply_overrides(app.world_mut(), &grown);

        let shrunk = bind(&app, "override_tests.jump", &[Control::Key(KeyCode::KeyK)]);
        apply_overrides(app.world_mut(), &shrunk);

        assert_eq!(
            slots(&app, "override_tests.jump"),
            [Control::Key(KeyCode::KeyK)]
        );
        let prompts = BindingTable::new(app.world()).prompts(Lunge::id(), PromptScope::ANY);
        assert_eq!(
            prompts
                .iter()
                .map(|prompt| prompt.origin.control())
                .collect::<Vec<_>>(),
            [Some(Control::Key(KeyCode::KeyK))],
            "Space is gone from the rider as well as from the row"
        );
    }

    /// Swapping a plan cancels what it had in flight, exactly as switching the context off does. A
    /// hold on a control that is no longer bound has to resolve rather than stay held for good.
    #[test]
    fn applying_cancels_what_was_in_flight() {
        use bevy_input::{ButtonState, keyboard::Key, keyboard::KeyboardInput};

        let mut app = app();
        let entity = app.world_mut().spawn(Playing).id();
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Space,
            logical_key: Key::Space,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        assert_eq!(
            app.world()
                .get::<InputContextState<Playing>>(entity)
                .unwrap()
                .phase::<Jump>(),
            Phase::Fired
        );

        let overrides = bind(&app, "override_tests.jump", &[Control::Key(KeyCode::KeyK)]);
        apply_overrides(app.world_mut(), &overrides);

        let state = app
            .world()
            .get::<InputContextState<Playing>>(entity)
            .unwrap();
        assert_eq!(state.phase::<Jump>(), Phase::Canceled);
        assert!(
            state.is_active(),
            "cancelling is not switching the context off"
        );
    }

    /// The bug a per-entity-only answer has: a context spawned after a rebind must be bound the way
    /// the player left it, not the way the game shipped.
    #[test]
    fn an_instance_spawned_after_a_rebind_gets_the_new_bindings() {
        let mut app = app();
        let overrides = bind(&app, "override_tests.jump", &[Control::Key(KeyCode::KeyK)]);
        apply_overrides(app.world_mut(), &overrides);

        app.world_mut().spawn(Playing);
        app.update();

        let prompts = BindingTable::new(app.world()).prompts(Jump::id(), PromptScope::ANY);
        assert_eq!(prompts.len(), 1);
        assert_eq!(
            prompts[0].origin.control(),
            Some(Control::Key(KeyCode::KeyK))
        );
    }

    /// R18.5's clause nothing could reach until now: a binding *changing* is the third thing that
    /// makes a prompt on screen stale, and without this a caption goes on naming the key the player
    /// just replaced.
    #[test]
    fn applying_says_prompts_may_have_changed() {
        let mut app = app();
        app.world_mut().spawn(Playing);
        app.update();
        let before = app
            .world()
            .get_resource::<crate::present::PromptGeneration>()
            .map_or(0, |generation| generation.0);

        let overrides = bind(&app, "override_tests.jump", &[Control::Key(KeyCode::KeyK)]);
        apply_overrides(app.world_mut(), &overrides);

        let after = app
            .world()
            .get_resource::<crate::present::PromptGeneration>()
            .map_or(0, |generation| generation.0);
        assert!(after > before, "a rebind said nothing about prompts");
    }

    /// R17.2. A saved set outlives the build that wrote it, so every one of these is a thing a file
    /// can say — and each is reported rather than dropped, while everything else still applies.
    #[test]
    fn every_unusable_row_is_reported_rather_than_dropped() {
        let mut app = app();
        let jump = row(&app, "override_tests.jump");
        let up = row(&app, "override_tests.move.up");
        let look = row(&app, "override_tests.look");
        let gone = MappingKey::new("override_tests.no_such_action", crate::binding::Part::Whole);

        let mut overrides = Overrides::new();
        overrides.bind(Scheme::KeyboardMouse, gone, [Control::Key(KeyCode::KeyZ)]);
        overrides.bind(look.scheme, look.key, [Control::MouseMotion]);
        overrides.bind(jump.scheme, jump.key, [Control::Key(KeyCode::F1)]);
        overrides.bind(
            up.scheme,
            up.key,
            [Control::Key(KeyCode::KeyI), Control::Key(KeyCode::KeyO)],
        );

        let problems = apply_overrides(app.world_mut(), &overrides);
        let kinds: Vec<_> = problems.iter().map(|problem| problem.kind).collect();

        assert!(kinds.contains(&OverrideProblemKind::NoSuchMapping));
        assert!(
            kinds.contains(&OverrideProblemKind::NotRebindable),
            "{kinds:?}"
        );
        assert!(kinds.contains(&OverrideProblemKind::Reserved {
            control: Control::Key(KeyCode::F1)
        }));
        assert!(kinds.contains(&OverrideProblemKind::TooManyControls {
            capacity: Capacity::UpTo(1),
            given: 2
        }));

        // Refused whole, never half: every one of those rows still holds what it shipped with.
        assert_eq!(
            slots(&app, "override_tests.jump"),
            [Control::Key(KeyCode::Space)]
        );
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW)]
        );
    }

    /// A control that reports on the wrong channel cannot fill the row, and a mouse motion is the
    /// clearest case: it has no press for a button action to read.
    #[test]
    fn a_control_of_the_wrong_shape_is_refused() {
        let mut app = app();
        let overrides = bind(&app, "override_tests.jump", &[Control::MouseMotion]);
        let problems = apply_overrides(app.world_mut(), &overrides);

        assert_eq!(
            problems
                .iter()
                .map(|problem| problem.kind)
                .collect::<Vec<_>>(),
            [OverrideProblemKind::WrongShape {
                control: Control::MouseMotion,
                accepts: ChannelShape::Button
            }]
        );
    }

    /// R19.4. Removing a row *is* the reset, which is the whole benefit of storing a diff — and it
    /// works at each of the four granularities the requirement names.
    #[test]
    fn resetting_puts_a_row_back_to_what_the_game_declared() {
        let mut app = app();
        let rows = mappings(app.world());
        let mut overrides = Overrides::new();
        for target in &rows {
            if target.rebinding.is_rebindable() {
                overrides.bind(target.scheme, target.key, [Control::Key(KeyCode::KeyZ)]);
            }
        }

        // One row.
        let up = row(&app, "override_tests.move.up");
        overrides.reset(up.scheme, up.key);
        assert!(overrides.get(up.scheme, up.key).is_none());

        // Every row of one action, which for a composite is all four directions.
        overrides.reset_action(&rows, Move::id());
        assert!(
            !rows
                .iter()
                .filter(|r| r.action == Move::id())
                .any(|r| { overrides.get(r.scheme, r.key).is_some() })
        );

        // Every row of one context, and then the lot.
        overrides.reset_context(&rows, "override_tests.playing");
        assert!(overrides.is_empty());

        overrides.bind(up.scheme, up.key, [Control::Key(KeyCode::KeyZ)]);
        overrides.reset_all();
        assert!(overrides.is_empty());

        apply_overrides(app.world_mut(), &overrides);
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW)]
        );
    }

    /// R19.12's mechanism: a preset moves a `Fixed` row a capture cannot, exempting exactly the
    /// rows it names from `NotRebindable` and nothing else.
    #[test]
    fn a_preset_moves_a_fixed_row_a_capture_cannot() {
        let mut app = app();
        let target = row(&app, "override_tests.settings");
        assert_eq!(target.rebinding, Rebinding::Fixed);

        let mut preset = Overrides::new();
        preset.bind(target.scheme, target.key, [Control::Key(KeyCode::F2)]);

        // Refused without a preset: a bare `apply_overrides` treats this row exactly as a capture
        // screen would.
        let problems = apply_overrides(app.world_mut(), &preset);
        assert_eq!(
            problems
                .iter()
                .map(|problem| problem.kind)
                .collect::<Vec<_>>(),
            [OverrideProblemKind::NotRebindable]
        );
        assert_eq!(
            slots(&app, "override_tests.settings"),
            [Control::Key(KeyCode::F1)]
        );

        // The same row moves once the same rows are named as the preset authorizing it.
        let problems = apply_overrides_with_preset(app.world_mut(), &preset, &preset);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&app, "override_tests.settings"),
            [Control::Key(KeyCode::F2)]
        );
    }

    /// A backend owning an action is neither a row the player cleared nor a row they never touched,
    /// which is exactly why there are three states and not two.
    #[test]
    fn a_row_someone_else_owns_is_left_alone() {
        let mut app = app();
        let target = row(&app, "override_tests.jump");
        let mut overrides = Overrides::new();
        overrides.set(target.scheme, target.key, Override::NotOurs);

        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty());
        assert_eq!(
            slots(&app, "override_tests.jump"),
            [Control::Key(KeyCode::Space)],
            "not ours is not cleared"
        );
    }

    /// A movement row grows only when the whole composite does: a second "forward" key is one part
    /// of a second set of four. Copying the composite instead would put the other three directions
    /// in their own rows twice over — "Move Down: S | S" — which is a wrong screen rather than an
    /// untidy one, so the row is refused and the shipped controls stand.
    #[test]
    fn one_direction_of_a_composite_cannot_grow_a_slot_on_its_own() {
        #[derive(InputContext)]
        #[context(path = "override_tests.wide", tick = Render)]
        struct Wide;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Wide>(|controls| {
            controls
                .bind::<Move>(DirectionalButtons::wasd())
                .mappable_upto(2);
        });

        let up = row(&app, "override_tests.move.up");
        let mut overrides = Overrides::new();
        overrides.bind(
            up.scheme,
            up.key,
            [Control::Key(KeyCode::KeyW), Control::Key(KeyCode::KeyI)],
        );
        let problems = apply_overrides(app.world_mut(), &overrides);

        assert_eq!(
            problems
                .iter()
                .map(|problem| problem.kind)
                .collect::<Vec<_>>(),
            [OverrideProblemKind::CompositeCannotGrow]
        );
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW)]
        );
        assert_eq!(
            slots(&app, "override_tests.move.down"),
            [Control::Key(KeyCode::KeyS)],
            "and the other three directions are untouched"
        );
    }

    /// The remedy the refusal above points at, and proof it is a real one: a second composite is
    /// how a two-column movement table is actually written, and each direction then rebinds its own
    /// secondary independently.
    #[test]
    fn a_second_composite_is_how_a_movement_row_gets_a_secondary() {
        #[derive(InputContext)]
        #[context(path = "override_tests.two_sets", tick = Render)]
        struct TwoSets;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<TwoSets>(|controls| {
            controls.bind::<Move>(DirectionalButtons::wasd()).mappable();
            controls
                .bind::<Move>(DirectionalButtons::arrow_keys())
                .mappable();
        });

        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW), Control::Key(KeyCode::ArrowUp)]
        );

        let up = row(&app, "override_tests.move.up");
        let mut overrides = Overrides::new();
        overrides.bind(
            up.scheme,
            up.key,
            [Control::Key(KeyCode::KeyW), Control::Key(KeyCode::KeyI)],
        );
        let problems = apply_overrides(app.world_mut(), &overrides);

        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW), Control::Key(KeyCode::KeyI)],
            "the secondary moved and the primary did not"
        );
        assert_eq!(
            slots(&app, "override_tests.move.down"),
            [
                Control::Key(KeyCode::KeyS),
                Control::Key(KeyCode::ArrowDown)
            ],
            "and the other rows kept both of theirs"
        );
    }
}
