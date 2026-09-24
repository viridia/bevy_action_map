//! External backends for source input, or for supplying one device family's input directly.
//!
//! A backend can provide input frames the same way a keyboard or a gamepad does, or it can stand in
//! for a whole device family and supply each action's value itself, the way a platform's own input
//! service (Steam Input, say) does for the gamepad. Either way, a context reading the action does
//! not need to know which one produced the result.

use alloc::vec::Vec;

use bevy_ecs::component::Component;

use crate::action::{ActionId, ActionValue, ChannelShape, InputAction};
use crate::binding::{BindingInput, IntoBindingInput};
use crate::device::DeviceFamily;

/// Binds an action to the value an outside authority supplies for one device family.
///
/// Some platforms take a device family over entirely. Under Steam Input the player binds the pad in
/// Steam's own layout, and the game is handed a value per action rather than button presses to map.
/// `Authority` binds an action to that value, beside the context's bindings for the families the
/// authority does not own:
///
/// ```ignore
/// app.add_context::<Flying>(|controls| {
///     controls.bind::<Thrust>(KeyCode::KeyW);
///     controls.bind::<Thrust>(Authority(DeviceFamily::Gamepad));
///     controls.bind::<Fire>(KeyCode::Space).pulse(0.2);
///     controls.bind::<Fire>(Authority(DeviceFamily::Gamepad)).pulse(0.2);
/// });
/// ```
///
/// It combines with the action's other bindings exactly as a control would, and conditions and
/// modifiers chained onto it apply to the authority's value: the rate of fire above holds however
/// the trigger was pulled. Stick shaping is the authority's own business, so a dead zone does not
/// belong here.
///
/// An action that [`follow`](crate::binding::InputContextBuilder::follow)s one bound here reads the
/// same value, so the authority writes it once: an afterburner riding `Thrust` needs nothing
/// written for itself.
///
/// The values arrive through [`AuthorityValues`] on the context's entity. Binding a control of the
/// same family to the same action is refused, since the authority owns that family. A network peer
/// or a scripted player works the same way, standing in for whichever family a human would have
/// used.
///
/// An authority reports a level rather than a stream of presses, so a press and release between two
/// of its writes never reach the action; under Steam Input, that is a tap shorter than a frame.
/// Each change it does report is one edge, however many fixed ticks read it. A button already held
/// when the context is spawned or activated waits for a release, as a key would, so a menu opened
/// by that button does not close on the same press.
///
/// An action with a `Delta2` intent refuses an authority binding: a delta is counted once, and a
/// level sampled every tick would count it again on each one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Authority(pub DeviceFamily);

impl IntoBindingInput for Authority {
    type Inputs = [BindingInput; 1];

    fn into_binding_inputs(self) -> Self::Inputs {
        // Placeholders: `bind` stamps the action and its shape, which is the first place the action
        // is known.
        [BindingInput::Authority(
            self.0,
            ChannelShape::Button,
            ActionId::PLACEHOLDER,
        )]
    }
}

/// Values supplied by an authority outside this crate's mapping.
///
/// Put this on a context's entity to feed the actions that context binds to an [`Authority`].
/// Whatever owns the outside authority — a platform input service, a network peer, a scripted agent
/// — writes the value it resolved, and the binding then contributes it like any other: the action
/// fires, completes and cancels on the edges of what you write.
///
/// Values are levels, not events. What you write stands until you write something else, so a
/// backend polled once a tick writes what it read, at rest included. An action nobody has written
/// reads at rest, but is not yet supplied: if the first value you write for a button is already
/// held, it waits for a release before it fires, just as a key held down while a menu opens does
/// not press anything in the menu.
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
    // Linear, because an authority drives a handful of actions at most: a `Vec` of pairs beats any
    // map at this size, and keeps the component cheap to clone for a rollback snapshot.
    values: Vec<(ActionId, ActionValue)>,
}

impl AuthorityValues {
    /// An empty set of values, with every authority binding reading at rest.
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes this a copy of `source`, or empty where there is none, keeping the allocation.
    pub(crate) fn hold(&mut self, source: Option<&Self>) {
        match source {
            Some(source) => self.values.clone_from(&source.values),
            None => self.values.clear(),
        }
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
    /// What a backend says when it loses the device behind an action, or stops reporting it, rather
    /// than writing a zero that claims the player is holding it at centre. Supplying it again while
    /// it is held waits for a release.
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
            paddle.bind::<Serve>(Authority(DeviceFamily::Gamepad));
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
