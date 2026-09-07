//! What the single-concept demos build on.
//!
//! `pong` is a real Cargo example target as well as a shared base: its own `main.rs` reaches this
//! file with `#[path = "mod.rs"] mod pong;`, the same line a *different* example uses to reach it —
//! `#[path = "../pong/mod.rs"] mod pong;`. A variant that wants the game unchanged calls
//! [`plugin`]; one that wants to swap a piece calls the sub-plugins it keeps and adds a module of
//! its own in place of, say, [`ball`].
//!
//! [`ball`], [`court`], [`paddle`] and [`score`] reach each other with `super::`, not `crate::`,
//! which is what lets them resolve the same way nested under `pong::` in someone else's crate as
//! they do at the root of this one.

use bevy::prelude::*;

pub mod ball;
pub mod court;
pub mod paddle;
pub mod score;

/// Every module, wired up exactly as an ordinary game of Pong wires them.
pub fn plugin(app: &mut App) {
    app.add_plugins((court::plugin, paddle::plugin, ball::plugin, score::plugin));
}
