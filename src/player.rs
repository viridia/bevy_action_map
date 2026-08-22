//! Players, device pairing, and control schemes.
//!
//! A context instance is the live runtime state for one gameplay context. Use one when you want
//! the same bindings and the same action reads, but separate state for different players,
//! different replays, or different test setups.

use alloc::vec::Vec;
use core::marker::PhantomData;

use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::prelude::{Res, Resource};
use bevy_ecs::{schedule::IntoScheduleConfigs, system::SystemParam};

use crate::action::{ActionOutput, ActionState, InputAction, InputContext, Phase};
use crate::binding::ContextBuilder;
use crate::eval::evaluate_context;
use crate::plan::Plan;

/// The live state for one declared context.
///
/// A context instance owns the compiled bindings for a context and the current state for each
/// action in that context. In an app, one instance usually belongs to one player or gameplay role.
/// In tests and replays, it lets you drive the same context without a full `World`.
#[derive(Resource, Clone, Debug)]
pub struct ContextInstance<C> {
    pub(crate) plan: Plan<C>,
    pub(crate) actions: Vec<ActionState>,
    _marker: PhantomData<C>,
}

impl<C> ContextInstance<C> {
    pub(crate) fn new(plan: Plan<C>) -> Self {
        let actions = plan
            .bindings()
            .iter()
            .map(|_| ActionState::default())
            .collect();

        Self {
            plan,
            actions,
            _marker: PhantomData,
        }
    }

    fn action_state<A>(&self) -> &ActionState
    where
        A: InputAction,
    {
        let slot = self
            .plan
            .slot_for_action(A::id())
            .expect("action has not been bound in this context");
        &self.actions[slot]
    }

    /// Reads the typed action value.
    pub fn value<A>(&self) -> A::Output
    where
        A: InputAction,
        A::Output: ActionOutput,
    {
        A::Output::from_action_value(self.action_state::<A>().value)
            .expect("action value does not match its declared output shape")
    }

    /// Returns the current phase for an action.
    pub fn phase<A>(&self) -> Phase
    where
        A: InputAction,
    {
        self.action_state::<A>().phase
    }

    /// Returns `true` when the action was pressed this tick.
    pub fn fired<A>(&self) -> bool
    where
        A: InputAction<Output = bool>,
    {
        self.phase::<A>() == Phase::Fired
    }
}

/// System parameter for polling a context's actions.
#[derive(SystemParam)]
pub struct Actions<'w, C: InputContext> {
    state: Res<'w, ContextInstance<C>>,
}

impl<'w, C: InputContext> Actions<'w, C> {
    /// Reads the typed action value.
    pub fn value<A>(&self) -> A::Output
    where
        A: InputAction,
        A::Output: ActionOutput,
    {
        self.state.value::<A>()
    }

    /// Returns the current phase for an action.
    pub fn phase<A>(&self) -> Phase
    where
        A: InputAction,
    {
        self.state.phase::<A>()
    }

    /// Returns `true` when the action was pressed this tick.
    pub fn fired<A>(&self) -> bool
    where
        A: InputAction<Output = bool>,
    {
        self.state.fired::<A>()
    }
}

/// The plugin entry point for the mapping layer.
pub struct ActionMapPlugin;

impl Plugin for ActionMapPlugin {
    fn build(&self, _app: &mut App) {}
}

/// Extension methods for setting up contexts.
pub trait ActionMapAppExt {
    /// Adds one context with its bindings and polling system.
    fn add_context<C, F>(&mut self, configure: F) -> &mut Self
    where
        C: InputContext,
        F: FnOnce(&mut ContextBuilder<C>);
}

impl ActionMapAppExt for App {
    fn add_context<C, F>(&mut self, configure: F) -> &mut Self
    where
        C: InputContext,
        F: FnOnce(&mut ContextBuilder<C>),
    {
        let mut builder = ContextBuilder::<C>::default();
        configure(&mut builder);

        let plan = Plan::from_bindings(builder.finish());
        self.insert_resource(ContextInstance::<C>::new(plan));
        self.add_systems(
            PreUpdate,
            evaluate_context::<C>.after(crate::frame::sample_keyboard_input),
        );
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{InputAction, InputContext};
    use bevy_app::{App, Update};
    use bevy_ecs::prelude::Resource;
    use bevy_input::{
        ButtonState, InputPlugin, keyboard::Key, keyboard::KeyCode, keyboard::KeyboardInput,
    };

    #[derive(InputAction)]
    #[action(path = "tests.jump", output = bool, intent = Button)]
    struct Jump;

    #[derive(InputContext)]
    #[context(path = "tests.on_foot", tick = Fixed)]
    struct OnFoot;

    #[derive(Resource, Default)]
    struct Probe {
        value: bool,
        phase: Phase,
    }

    fn probe_jump(input: Actions<'_, OnFoot>, mut probe: bevy_ecs::system::ResMut<'_, Probe>) {
        probe.value = input.value::<Jump>();
        probe.phase = input.phase::<Jump>();
    }

    #[test]
    fn pressing_and_releasing_a_key_updates_the_action_state() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, crate::frame::InputFramePlugin, ActionMapPlugin));
        app.add_context::<OnFoot, _>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        });
        app.init_resource::<Probe>();
        app.add_systems(Update, probe_jump);

        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Space,
            logical_key: Key::Space,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: bevy_ecs::entity::Entity::PLACEHOLDER,
        });
        app.update();

        let probe = app.world().resource::<Probe>();
        assert!(probe.value);
        assert_eq!(probe.phase, Phase::Fired);

        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Space,
            logical_key: Key::Space,
            state: ButtonState::Released,
            text: None,
            repeat: false,
            window: bevy_ecs::entity::Entity::PLACEHOLDER,
        });
        app.update();

        let probe = app.world().resource::<Probe>();
        assert!(!probe.value);
        assert_eq!(probe.phase, Phase::Completed);
    }
}
