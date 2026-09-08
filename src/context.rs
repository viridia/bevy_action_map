//! Contexts: declaring them, and the live state of one.
//!
//! A context groups the bindings that are active together — on foot, in a vehicle, in a menu. You
//! declare one with [`ActionMapAppExt::add_context`] and give it to an entity; that entity then
//! carries an [`InputContextState`] holding the current state of every action in the context.
//!
//! Put the context on whatever the input belongs to. One entity for a single-player game, one per
//! player for local multiplayer, or a bare entity for input that is not tied to anything in
//! particular. Each carries its own state, so two players never share one.
//!
//! A context is live from the moment an entity carries it unless you say otherwise. Say otherwise
//! with [`active_in_state`](crate::binding::InputContextBuilder::active_in_state) for a context
//! that comes and goes with a game state,
//! [`active_if`](crate::binding::InputContextBuilder::active_if) for one that follows any other
//! run condition, or [`activate`](InputContextState::activate) and
//! [`deactivate`](InputContextState::deactivate) to drive one instance yourself.
//!
//! Read the actions back with [`ContextActions`] where there is one instance of the context, and
//! with [`ActionsQuery`] where there may be several — one per player, or none at all because
//! whatever carried it was destroyed.

mod declare;
mod state;

#[cfg(test)]
mod fixtures;

pub use declare::{ActionMapAppExt, warn_if_undeclared};
pub use state::{ActionObstacle, ActionReading, ActionsQuery, ContextActions, InputContextState};

pub(crate) use declare::Activation;
