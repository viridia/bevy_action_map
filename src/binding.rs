//! Bindings, composites, modifiers, and conditions.
//!
//! The first interactive stage only needs one simple binding shape: one keyboard key driving one
//! boolean action.

use alloc::vec::Vec;
use core::marker::PhantomData;

use bevy_input::keyboard::KeyCode;

use crate::action::{ActionId, InputAction};

/// One authored binding in the first end-to-end slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BindingSpec {
	pub(crate) action: ActionId,
	pub(crate) key_code: KeyCode,
}

/// Builder used by [`crate::player::ActionMapAppExt::add_context`].
pub struct ContextBuilder<C> {
	bindings: Vec<BindingSpec>,
	_marker: PhantomData<C>,
}

impl<C> Default for ContextBuilder<C> {
	fn default() -> Self {
		Self {
			bindings: Vec::new(),
			_marker: PhantomData,
		}
	}
}

impl<C> ContextBuilder<C> {
	/// Binds a boolean action to one keyboard key.
	pub fn bind<A>(&mut self, key_code: KeyCode) -> &mut Self
	where
		A: InputAction<Output = bool>,
	{
		self.bindings.push(BindingSpec {
			action: A::id(),
			key_code,
		});
		self
	}

	pub(crate) fn finish(self) -> Vec<BindingSpec> {
		self.bindings
	}
}
