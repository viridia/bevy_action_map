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

/// The action-map prelude.
///
/// This includes the most common types in this crate, re-exported for your convenience.
pub mod prelude {
    pub use crate::action::{
        ActionId, ActionOutput, ActionValue, InputAction, InputContext, Intent, TickDomain,
    };
    pub use crate::frame::{InputFrame, RawEvent, TimedRawEvent, Timestamp};

    #[cfg(feature = "keyboard")]
    pub use crate::frame::{InputFramePlugin, sample_keyboard_input};
}

pub use bevy_action_map_macros::{InputAction, InputContext};
