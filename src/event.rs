//! Transition events: what an action just did, delivered to an observer.
//!
//! Polling an action asks "what is it doing now". Observing one asks "tell me when it changes", and
//! for something that happens on a single tick — a jump, a shot fired, a menu confirmed — the
//! second question is usually the one you meant.
//!
//! Each event targets the entity carrying the context, so an observer added to that entity hears
//! only its own player's input:
//!
//! ```ignore
//! commands.spawn(Flying).observe(|fire: On<Fired<Fire>>| {
//!     info!("pew");
//! });
//! ```
//!
//! An observer added with `App::add_observer` hears every entity's, which is what you want for
//! something global like a pause key.

use core::marker::PhantomData;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, EntityEvent};

use crate::action::{ActionOutput, ActionValue, InputAction, Phase};
use crate::frame::RawEvent;

/// An action became active this tick.
///
/// For a plain button this is the press. Once conditions exist it is the moment the condition was
/// satisfied, which for a hold is when the hold has lasted long enough rather than when the button
/// went down.
#[derive(EntityEvent)]
pub struct Fired<A: InputAction> {
    /// The entity carrying the context this action belongs to.
    pub entity: Entity,
    /// The action's value at the moment it fired.
    pub value: A::Output,
}

/// A condition on the action began, but has not been satisfied yet.
///
/// A hold that has just been pressed. Use it to show that something is charging; the matching
/// [`Fired`] arrives if the player sees it through, and [`Canceled`] if they do not.
///
/// Without conditions this never happens, because there is nothing for a press to be the start
/// *of* — a plain binding fires immediately.
#[derive(EntityEvent)]
pub struct Started<A: InputAction> {
    /// The entity carrying the context this action belongs to.
    pub entity: Entity,
    /// The action's value at the moment it started, which is usually rest.
    pub value: A::Output,
}

/// An action stopped being active after having been active.
#[derive(EntityEvent)]
pub struct Completed<A: InputAction> {
    /// The entity carrying the context this action belongs to.
    pub entity: Entity,
    /// The action's value at the moment it completed, which is usually rest.
    pub value: A::Output,
}

/// An action was abandoned before it completed.
///
/// This is what a context deactivating mid-hold produces: the action did not finish, and treating
/// it as though it had would fire whatever the player was in the middle of.
#[derive(EntityEvent)]
pub struct Canceled<A: InputAction> {
    /// The entity carrying the context this action belongs to.
    pub entity: Entity,
    /// The action's value at the moment it was abandoned.
    pub value: A::Output,
}

/// Turns one logged transition into the typed event for its action.
///
/// The plan stores one of these per slot. It is the only place the concrete action type survives to
/// — the evaluator works in `ActionId`s and slots, which cannot name a generic event on their own.
pub(crate) type Dispatch = fn(&mut Commands<'_, '_>, Entity, Phase, ActionValue);

pub(crate) fn dispatch_for<A: InputAction>(
    commands: &mut Commands<'_, '_>,
    entity: Entity,
    phase: Phase,
    value: ActionValue,
) {
    let value = A::Output::from_action_value(value);
    match phase {
        Phase::Started => commands.trigger(Started::<A> { entity, value }),
        Phase::Fired => commands.trigger(Fired::<A> { entity, value }),
        Phase::Completed => commands.trigger(Completed::<A> { entity, value }),
        Phase::Canceled => commands.trigger(Canceled::<A> { entity, value }),
        // Not edges: nothing changed, so there is nothing to tell an observer about.
        Phase::Idle | Phase::Ongoing => {}
    }
}

/// Identifies one class binding, the way [`InputAction`] identifies an action.
///
/// Deliberately not `InputAction`: a class binding is never combined into per-tick action state,
/// carries no modifiers or conditions, and has nothing to hold between ticks. It only needs a
/// stable name.
///
/// ```rust
/// use bevy_action_map::event::ClassBinding;
///
/// struct CharacterInput;
///
/// impl ClassBinding for CharacterInput {
///     const PATH: &'static str = "ui.character_input";
/// }
/// ```
pub trait ClassBinding: Send + Sync + 'static {
    /// Stable path used to identify this class binding, mainly in diagnostics.
    const PATH: &'static str;
}

/// A control matching a bound class arrived, and nothing else in the context already claimed it.
///
/// The only event a class binding produces — there is no `Started`, `Completed` or `Canceled` here,
/// because there is no condition in progress to report on. `event` is the original raw event,
/// unaltered: a text widget matches [`RawEvent::Keyboard`] and reads its key, text and repeat flag
/// straight off it to build whatever its own focus-input event wants, rather than this crate
/// guessing at one payload shape that would fit every consumer.
#[derive(EntityEvent)]
pub struct ClassFired<A: ClassBinding> {
    /// The entity carrying the context this class binding belongs to.
    pub entity: Entity,
    /// The event a member of the bound class produced.
    pub event: RawEvent,
    _marker: PhantomData<A>,
}

/// Turns one logged class hit into the typed event for its class binding.
///
/// The plan stores one of these per class binding, mirroring `Dispatch` and for the same reason: it
/// is the only place the concrete `ClassBinding` type survives to.
pub(crate) type ClassDispatch = fn(&mut Commands<'_, '_>, Entity, RawEvent);

pub(crate) fn class_dispatch_for<A: ClassBinding>(
    commands: &mut Commands<'_, '_>,
    entity: Entity,
    event: RawEvent,
) {
    commands.trigger(ClassFired::<A> {
        entity,
        event,
        _marker: PhantomData,
    });
}
