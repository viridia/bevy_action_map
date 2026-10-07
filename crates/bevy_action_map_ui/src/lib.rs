//! UI for [`bevy_action_map`](https://docs.rs/bevy_action_map): drawing the input prompts that tell
//! a player which button performs an action.
//!
//! [`PromptArt`] is where an icon prompt finds its art. The crate ships none: each art set a game
//! uses is a provider added to it.
#![forbid(unsafe_code)]

mod art;

pub use art::{IconLayout, PromptArt};
