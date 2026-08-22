//! Compiling bindings into the plan the evaluator runs against.
//!
//! A plan is the immutable compiled form of authored bindings.

use alloc::{collections::BTreeMap, vec::Vec};
use core::marker::PhantomData;

use crate::action::ActionId;
use crate::binding::BindingSpec;

/// The plan is the immutable runtime view of a context's authored bindings.
// This is a dense binding list plus a slot map from action id to binding index.
// We build it once when the context is declared, then share it with the
// evaluator so lookups stay O(1) without rebuilding per frame.
#[derive(Clone, Debug, Default)]
pub struct Plan<C> {
    bindings: Vec<BindingSpec>,
    slot_by_action: BTreeMap<ActionId, usize>,
    _marker: PhantomData<C>,
}

impl<C> Plan<C> {
    /// Compiles a plan from authored bindings.
    pub(crate) fn from_bindings(bindings: Vec<BindingSpec>) -> Self {
        let mut slot_by_action = BTreeMap::new();
        for (slot, binding) in bindings.iter().enumerate() {
            slot_by_action.insert(binding.action, slot);
        }

        Self {
            bindings,
            slot_by_action,
            _marker: PhantomData,
        }
    }

    pub(crate) fn bindings(&self) -> &[BindingSpec] {
        &self.bindings
    }

    pub(crate) fn slot_for_action(&self, action: ActionId) -> Option<usize> {
        self.slot_by_action.get(&action).copied()
    }
}
