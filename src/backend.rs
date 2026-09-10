//! External backends for source input, or for supplying action values directly.
//!
//! A backend can provide input frames the same way a keyboard or a gamepad does, or it can bypass
//! this crate's own bindings and supply an action's value directly, the way a platform's own input
//! service (Steam Input, say) does. Either way, a context reading the action does not need to know
//! which one produced the result.

use alloc::vec::Vec;

use bevy_ecs::component::Component;

use crate::action::{ActionId, ActionValue, InputAction};

/// Action values supplied by an authority outside this crate's mapping.
///
/// Put this on a context's entity to drive the actions that context
/// [`delegate`](crate::binding::InputContextBuilder::delegate)s. Whatever owns the outside
/// authority — a platform input service, a network peer, a scripted agent — writes the value it
/// resolved, and the action then behaves like any other: it fires, completes and cancels on the
/// edges of what you write, and gameplay code reading it cannot tell the difference.
///
/// Values are levels, not events. What you write stands until you write something else, so a
/// backend polled once a tick simply writes what it read. An action nobody has written reads at
/// rest.
///
/// ```ignore
/// // Poll the outside authority once a tick, before the input map evaluates.
/// fn poll(mut paddles: Query<&mut AuthorityValues>, service: Res<InputService>) {
///     for mut values in &mut paddles {
///         values.set::<Move>(service.stick());
///         values.set::<Serve>(service.pressed(SERVE));
///     }
/// }
///
/// app.add_systems(PreUpdate, poll.before(ActionMapSystems::Evaluate));
/// ```
///
/// Write from a system ordered before evaluation. A value written after it lands on the next tick
/// instead, which for a level is a frame of lag rather than a lost input, but is rarely what you
/// meant.
#[derive(Component, Clone, Debug, Default)]
pub struct AuthorityValues {
    // Linear, because a context delegates a handful of actions at most: a `Vec` of pairs beats any
    // map at this size, and keeps the component cheap to clone for a rollback snapshot.
    values: Vec<(ActionId, ActionValue)>,
}

impl AuthorityValues {
    /// An empty set of values, with every delegated action reading at rest.
    pub fn new() -> Self {
        Self::default()
    }

    /// Supplies the value of one action.
    ///
    /// The value is typed by the action, so a button takes a `bool` and a stick takes a `Vec2`.
    pub fn set<A: InputAction>(&mut self, value: A::Output) -> &mut Self {
        let value = ActionValue::from_output(value);
        match self.entry(A::id()) {
            Some(held) => *held = value,
            None => self.values.push((A::id(), value)),
        }
        self
    }

    /// Stops supplying one action, which then reads at rest.
    ///
    /// What a backend says when it loses the device behind an action, rather than writing a zero
    /// that claims the player is holding it at centre.
    pub fn clear<A: InputAction>(&mut self) -> &mut Self {
        self.values.retain(|(action, _)| *action != A::id());
        self
    }

    /// The value currently supplied for one action, if any.
    pub fn get<A: InputAction>(&self) -> Option<A::Output> {
        self.value_of(A::id()).map(ActionValue::into_output)
    }

    pub(crate) fn value_of(&self, action: ActionId) -> Option<ActionValue> {
        self.values
            .iter()
            .find(|(held, _)| *held == action)
            .map(|&(_, value)| value)
    }

    fn entry(&mut self, action: ActionId) -> Option<&mut ActionValue> {
        self.values
            .iter_mut()
            .find(|(held, _)| *held == action)
            .map(|(_, value)| value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bevy_app::{App, FixedPreUpdate, FixedUpdate};
    use bevy_ecs::prelude::{IntoScheduleConfigs, Query, Res, ResMut, Resource};
    use bevy_input::InputPlugin;

    use crate::context::{ActionMapAppExt, ContextActions};
    use crate::{ActionMapPlugin, ActionMapSystems};
    use crate::{InputAction, InputContext};

    #[derive(InputAction)]
    #[action(path = "backend_tests.serve", output = bool, intent = Button)]
    struct Serve;

    #[derive(InputContext)]
    #[context(path = "backend_tests.paddle", tick = Fixed)]
    struct Paddle;

    #[derive(Resource, Default)]
    struct Fires(u32);

    fn count_fires(input: ContextActions<'_, '_, Paddle>, mut fires: ResMut<'_, Fires>) {
        if input.fired::<Serve>() {
            fires.0 += 1;
        }
    }

    fn supply(mut paddles: Query<'_, '_, &mut AuthorityValues>, held: Res<'_, Held>) {
        for mut values in &mut paddles {
            values.set::<Serve>(held.0);
        }
    }

    #[derive(Resource, Default)]
    struct Held(bool);

    /// The whole path, through the plugin rather than the evaluator alone: a system writes the
    /// component, and gameplay code reading the action sees a press it can act on. Nothing in the
    /// reading half knows an authority produced it, which is the promise the two-backend split is
    /// there to keep.
    #[test]
    fn a_value_written_before_evaluation_reaches_gameplay_code() {
        let mut app = App::new();
        app.add_plugins((InputPlugin, ActionMapPlugin));
        app.add_context::<Paddle>(|paddle| {
            paddle.delegate::<Serve>();
        });
        app.init_resource::<Fires>();
        app.init_resource::<Held>();
        app.add_systems(FixedPreUpdate, supply.before(ActionMapSystems::Evaluate));
        app.add_systems(FixedUpdate, count_fires);
        app.world_mut().spawn((Paddle, AuthorityValues::new()));

        run_tick(&mut app);
        assert_eq!(app.world().resource::<Fires>().0, 0);

        app.world_mut().resource_mut::<Held>().0 = true;
        run_tick(&mut app);
        assert_eq!(app.world().resource::<Fires>().0, 1);

        // Still held. A level that has not moved fires nothing further.
        run_tick(&mut app);
        assert_eq!(app.world().resource::<Fires>().0, 1);
    }

    // `bevy_time` is not a dependency, so the fixed loop never steps on its own.
    fn run_tick(app: &mut App) {
        app.world_mut().run_schedule(FixedPreUpdate);
        app.world_mut().run_schedule(FixedUpdate);
    }
}
