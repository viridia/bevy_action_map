#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![no_std]

//! Input action mapping for Bevy.
//!
//! Use this crate when you want to turn physical input into gameplay actions, then read those
//! actions back in your systems. It covers device data, input frames, action mapping, and the
//! player-facing presentation layer.
//!
//! Start with [`action`] to define your actions and contexts, then wire them up through
//! [`binding`] and [`plan`]. Use [`present`] and [`rebind`] when you want to show players what is
//! bound.
//!
//! ```rust
//! use bevy_action_map::prelude::*;
//!
//! #[derive(bevy_action_map::InputAction)]
//! #[action(path = "gameplay.jump", output = bool, intent = Button)]
//! struct Jump;
//!
//! #[derive(bevy_action_map::InputContext)]
//! #[context(path = "gameplay.on_foot", tick = Fixed)]
//! struct OnFoot;
//! ```

extern crate self as bevy_action_map;

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

// L0
pub mod device;

// L1
pub mod frame;

// L2
pub mod action;
pub mod binding;
pub mod eval;
pub mod plan;
pub mod player;

// L3
pub mod present;
pub mod rebind;

pub mod backend;

#[cfg(feature = "focus")]
#[cfg_attr(docsrs, doc(cfg(feature = "focus")))]
pub mod focus;

/// System sets for the two stages of the input pipeline.
///
/// Order your own systems against these when you need to run at a specific point relative to
/// input. [`Sample`](ActionMapSystems::Sample) collects device messages into the input frame;
/// [`Evaluate`](ActionMapSystems::Evaluate) maps that frame onto action state.
///
/// Sampling runs in `PreUpdate`, after Bevy's own input systems. Evaluation runs in `PreUpdate`
/// for render-tick contexts and in `FixedPreUpdate` for fixed-tick ones, so a system reading
/// actions from `Update` or `FixedUpdate` always sees state that is current for its own schedule.
#[derive(bevy_ecs::schedule::SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionMapSystems {
    /// Collects raw device messages into the input frame.
    Sample,
    /// Maps the input frame onto action state.
    Evaluate,
}

/// The action-map prelude.
///
/// This includes the most common types in this crate, re-exported for your convenience.
pub mod prelude {
    pub use crate::ActionMapSystems;
    pub use crate::action::{
        ActionId, ActionOutput, ActionState, ActionValue, InputAction, InputContext, Intent, Phase,
        TickDomain,
    };
    // `InputContextBuilder` is deliberately absent: `add_context` hands one to a closure, so its
    // type is inferred and never written. Import it from `binding` to name it in a signature.
    #[cfg(feature = "keyboard")]
    pub use crate::binding::DirectionalKeys;
    #[cfg(feature = "gamepad")]
    pub use crate::binding::Stick;
    pub use crate::frame::{InputFrame, RawEvent, TimedRawEvent, Timestamp};
    pub use crate::player::{ActionMapAppExt, ActionMapPlugin, Actions, InputContextState};

    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    pub use crate::frame::{InputFramePlugin, sample_input};
}

pub use bevy_action_map_macros::{InputAction, InputContext};
