//! What the examples share.
//!
//! Not a Cargo target: a directory under `examples/` with no `main.rs` is not built as one, so this
//! is reached with `#[path = "../common/mod.rs"] mod common;` from each example that wants it.
//!
//! An example is allowed to use only part of this.
#![allow(dead_code)]

pub mod prompt_ui;
pub mod widget_focus;
