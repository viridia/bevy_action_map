//! Bindings, composites, modifiers, and conditions.
//!
//! The first interactive stage needs three source shapes: one keyboard key for a button action,
//! one mouse-motion source for look, and one four-key directional composite for movement.

use alloc::vec::Vec;
use core::marker::PhantomData;

use bevy_input::keyboard::KeyCode;
use bevy_math::Vec2;

use crate::action::{ActionId, InputAction};

/// Named parts for a 2D directional composite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirectionalKeys {
    /// The key that contributes positive Y.
    pub up: KeyCode,
    /// The key that contributes negative Y.
    pub down: KeyCode,
    /// The key that contributes negative X.
    pub left: KeyCode,
    /// The key that contributes positive X.
    pub right: KeyCode,
}

impl DirectionalKeys {
    /// Creates a directional composite from the four movement keys.
    pub const fn new(up: KeyCode, down: KeyCode, left: KeyCode, right: KeyCode) -> Self {
        Self {
            up,
            down,
            left,
            right,
        }
    }
}

/// One authored binding in the first end-to-end slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BindingSpec {
    pub(crate) action: ActionId,
    pub(crate) source: BindingSource,
}

/// The binding source used by the first interactive stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BindingSource {
    Button(KeyCode),
    Directional2(DirectionalKeys),
    MouseMotion,
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
            source: BindingSource::Button(key_code),
        });
        self
    }

    /// Binds a 2D action to mouse motion.
    pub fn bind_mouse_motion<A>(&mut self) -> &mut Self
    where
        A: InputAction<Output = Vec2>,
    {
        self.bindings.push(BindingSpec {
            action: A::id(),
            source: BindingSource::MouseMotion,
        });
        self
    }

    /// Binds a 2D action to four named directional keys.
    pub fn bind_directional<A>(&mut self, keys: DirectionalKeys) -> &mut Self
    where
        A: InputAction<Output = Vec2>,
    {
        self.bindings.push(BindingSpec {
            action: A::id(),
            source: BindingSource::Directional2(keys),
        });
        self
    }

    pub(crate) fn finish(self) -> Vec<BindingSpec> {
        self.bindings
    }
}
