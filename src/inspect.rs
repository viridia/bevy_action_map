//! Reading the whole input state without naming any of it.
//!
//! Everything else in this crate is typed: you name an action and get its value back in the shape
//! the action declared. That is the right trade for game code, which knows what it wants, and the
//! wrong one for a debug overlay, an editor, or anything else that has to show whatever it is
//! given. This module is the other half.
//!
//! [`dump`] walks every declared context, every entity carrying one, and every action bound in it,
//! and hands back a plain description of what it found — including, per action, why it is not
//! firing.
//!
//! ```ignore
//! fn overlay(world: &mut World) {
//!     for context in dump(world).contexts {
//!         println!("{} ({:?})", context.path, context.tick);
//!         for instance in context.instances {
//!             for action in instance.actions {
//!                 println!("  {} {:?} {:?}", action.path, action.state.phase, action.obstacle);
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! This allocates, and is meant to be called by a tool rather than by a game. Every frame is fine —
//! a debug overlay does exactly that — but it is not on the path your actions travel.

use alloc::vec::Vec;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use bevy_ecs::world::World;

use crate::action::{ActionId, ActionState, TickDomain};
use crate::context::Obstacle;

/// Everything the input layer currently holds.
#[derive(Clone, Debug)]
pub struct InputDump {
    /// Every context that has been declared, in declaration order.
    pub contexts: Vec<ContextDump>,
}

/// One declared context, and whatever is carrying it.
#[derive(Clone, Debug)]
pub struct ContextDump {
    /// The context's declared path.
    pub path: &'static str,
    /// Which schedule it is evaluated in.
    pub tick: TickDomain,
    /// Where it sits relative to other contexts in that schedule. Higher goes first.
    pub priority: i32,
    /// The entities carrying it. Empty means the context is declared and nobody has it, which is
    /// usually a mistake.
    pub instances: Vec<InstanceDump>,
}

/// One entity's copy of a context.
#[derive(Clone, Debug)]
pub struct InstanceDump {
    /// The entity carrying it.
    pub entity: Entity,
    /// Whether it is currently driving its actions.
    pub active: bool,
    /// Every action the context binds, in the order they were first bound.
    pub actions: Vec<ActionDump>,
}

/// One action, as it stands on one instance.
#[derive(Clone, Debug)]
pub struct ActionDump {
    /// The action's runtime identity.
    pub action: ActionId,
    /// Its declared path.
    pub path: &'static str,
    /// Value, phase, elapsed time and progress.
    pub state: ActionState,
    /// What is in the way, or [`Obstacle::None`] when it is firing.
    pub obstacle: Obstacle,
}

/// Which side of an override a reader wants: the rows in force, or the rows the game shipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverrideStage {
    /// The declaration, whatever has since been applied over it.
    Declared,
    /// What is actually in force: whatever has been applied, or the declaration where nothing has.
    Effective,
}

/// How to read the instances of one context whose type is no longer known.
///
/// Recorded when the context is declared, which is the last moment its type is available.
pub(crate) struct DeclaredContext {
    pub(crate) path: &'static str,
    pub(crate) tick: TickDomain,
    pub(crate) priority: i32,
    pub(crate) read: fn(&mut World) -> Vec<InstanceDump>,
    // Mappings come from the context's compiled bindings rather than from anything an entity
    // so unlike `read` this one needs no query and no exclusive access. `OverrideStage` picks
    // between the rows in force and the rows the game declared — the latter being what a reset
    // previews, and what an override is a diff against.
    pub(crate) mappings: fn(&World, OverrideStage) -> Vec<crate::mapping::Mapping>,
    // Tunables, on the same terms.
    pub(crate) tunables: fn(&World, OverrideStage) -> Vec<crate::mapping::Tunable>,
    // The same bindings as `mappings` the other way round, for a prompt asking what fires an
    // action. Separate because it answers about bindings rather than about rows a player edits:
    // a `private` binding is missing from one and present in the other.
    pub(crate) bindings: fn(&World) -> crate::present::ContextBindings,
    // Rewrites this context's bindings for an override set and swaps the result into every
    // instance. Exclusive because it writes both a resource and the components. The `Option`
    // carries a preset's rows, exempted from the rebindable-only refusal that would otherwise
    // stop a preset moving a `Fixed` row.
    pub(crate) apply: fn(
        &mut World,
        &crate::overrides::Overrides,
        Option<&crate::overrides::Overrides>,
    ) -> Vec<crate::overrides::OverrideProblem>,
    // Like `apply`, but reaches one named entity's own instance rather than every one — the entry
    // point `apply_overrides_for` walks. A context type the entity does not carry is a no-op here,
    // the same way a row nothing declares is a no-op for `apply`.
    pub(crate) apply_for_entity: fn(
        &mut World,
        Entity,
        &crate::overrides::Overrides,
        Option<&crate::overrides::Overrides>,
    ) -> Vec<crate::overrides::OverrideProblem>,
}

/// Every context declared so far, in declaration order.
#[derive(Resource, Default)]
pub(crate) struct DeclaredContexts(pub(crate) Vec<DeclaredContext>);

/// Reads every declared context and the state of everything carrying one.
///
/// Takes the world exclusively because the contexts are separate component types, and there is no
/// way to query a type you cannot name without asking the world to build the query for you.
pub fn dump(world: &mut World) -> InputDump {
    let Some(declared) = world.remove_resource::<DeclaredContexts>() else {
        return InputDump {
            contexts: Vec::new(),
        };
    };

    let contexts = declared
        .0
        .iter()
        .map(|context| ContextDump {
            path: context.path,
            tick: context.tick,
            priority: context.priority,
            instances: (context.read)(world),
        })
        .collect();

    // Taken out and put back so that each reader can borrow the world mutably in turn.
    world.insert_resource(declared);
    InputDump { contexts }
}

#[cfg(all(test, feature = "keyboard"))]
mod tests {
    use super::*;

    use bevy_app::App;
    use bevy_input::keyboard::KeyCode;

    use crate::context::ActionMapAppExt;
    use crate::{ActionMapPlugin, InputAction, InputContext};

    #[derive(InputAction)]
    #[action(path = "inspect_tests.jump", output = bool, intent = Button)]
    struct Jump;

    #[derive(InputAction)]
    #[action(path = "inspect_tests.crouch", output = bool, intent = Button)]
    struct Crouch;

    #[derive(InputContext)]
    #[context(path = "inspect_tests.on_foot", tick = Fixed)]
    struct OnFoot;

    #[derive(InputContext)]
    #[context(path = "inspect_tests.menu", tick = Render, priority = 10)]
    struct Menu;

    /// The whole point of the dump: everything is named at run time. Nothing here calls
    /// `value::<Jump>()`, and a tool built on this works for actions it was never compiled against.
    #[test]
    fn the_dump_names_every_context_and_action_without_naming_a_type() {
        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
            context.bind::<Crouch>(KeyCode::ControlLeft);
        });
        app.add_context::<Menu>(|context| {
            context.bind::<Jump>(KeyCode::Enter);
        });
        let player = app.world_mut().spawn(OnFoot).id();
        app.update();

        let dump = dump(app.world_mut());
        assert_eq!(dump.contexts.len(), 2);

        let on_foot = &dump.contexts[0];
        assert_eq!(on_foot.path, "inspect_tests.on_foot");
        assert_eq!(on_foot.tick, TickDomain::Fixed);
        assert_eq!(on_foot.priority, 0);
        assert_eq!(on_foot.instances.len(), 1);
        assert_eq!(on_foot.instances[0].entity, player);
        assert!(on_foot.instances[0].active);
        assert_eq!(
            on_foot.instances[0]
                .actions
                .iter()
                .map(|action| action.path)
                .collect::<Vec<_>>(),
            ["inspect_tests.jump", "inspect_tests.crouch"],
            "in the order they were bound"
        );

        // Declared, carried by nobody — which is the shape of a context somebody forgot to spawn,
        // and visible here rather than having to be inferred from nothing happening.
        let menu = &dump.contexts[1];
        assert_eq!(menu.path, "inspect_tests.menu");
        assert_eq!(menu.priority, 10);
        assert!(menu.instances.is_empty());
    }

    // A dump carries the answer to "so why is nothing happening", the question anyone reading an
    // overlay actually has.
    #[test]
    fn the_dump_carries_the_obstacle_for_each_action() {
        use crate::context::{InputContextState, Obstacle};

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        let player = app.world_mut().spawn(OnFoot).id();
        app.update();

        let obstacle =
            |app: &mut App| dump(app.world_mut()).contexts[0].instances[0].actions[0].obstacle;
        assert_eq!(obstacle(&mut app), Obstacle::NoInput);

        app.world_mut()
            .get_mut::<InputContextState<OnFoot>>(player)
            .unwrap()
            .deactivate();
        assert_eq!(obstacle(&mut app), Obstacle::ContextInactive);
    }

    // A world with no contexts declared is not an error, and asking is not a panic.
    #[test]
    fn dumping_a_world_with_no_contexts_is_empty() {
        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.update();

        assert!(dump(app.world_mut()).contexts.is_empty());
    }
}
