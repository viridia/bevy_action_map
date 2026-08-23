//! Compiling bindings into the plan the evaluator runs against.
//!
//! A plan is the immutable compiled form of authored bindings.

use alloc::{collections::BTreeMap, vec::Vec};
use core::marker::PhantomData;

use crate::action::{ActionId, ChannelShape, Intent};
use crate::binding::{BindingModifier, BindingSource, BindingSpec};

/// The part of a rejected binding's message that says what to do about it.
///
/// Only the two mistakes with a specific remedy get one; the rest are adequately explained by
/// naming the intent and the channel that cannot serve it.
fn mismatch_hint(intent: Intent, shape: ChannelShape) -> &'static str {
    match (intent, shape) {
        (Intent::Directional2, ChannelShape::Button | ChannelShape::Axis1) => {
            ". A single control carries no direction — bind a directional composite, whose parts \
             can be keyboard keys or D-pad buttons"
        }
        (Intent::Delta2, _) | (_, ChannelShape::Delta2) => {
            ". A delta is a displacement that has already happened and a position is a rate, so \
             one cannot stand in for the other without an explicit conversion"
        }
        _ => "",
    }
}

/// An authored binding with its action resolved to a state slot.
pub(crate) struct CompiledBinding {
    pub(crate) slot: usize,
    pub(crate) source: BindingSource,
    pub(crate) modifiers: Vec<BindingModifier>,
}

/// The plan is the immutable runtime view of a context's authored bindings.
// One slot per action, not per binding: an action may be bound several times, and all of those
// bindings write the same state. Bindings are grouped by slot so the evaluator can fold each
// action's contributions in a single pass with no per-frame bookkeeping.
pub struct Plan<C> {
    bindings: Vec<CompiledBinding>,
    slot_intents: Vec<Intent>,
    slot_by_action: BTreeMap<ActionId, usize>,
    _marker: PhantomData<C>,
}

impl<C> Plan<C> {
    /// Compiles a plan from authored bindings.
    pub(crate) fn from_bindings(bindings: Vec<BindingSpec>) -> Self {
        let mut slot_intents: Vec<Intent> = Vec::new();
        let mut slot_by_action = BTreeMap::new();
        let mut compiled = Vec::with_capacity(bindings.len());

        for binding in bindings {
            let shape = binding.source.channel_shape();
            assert!(
                binding.intent.accepts(shape),
                "`{}` has intent {:?}, which a control reporting on a {:?} channel cannot serve{}",
                binding.path,
                binding.intent,
                shape,
                mismatch_hint(binding.intent, shape)
            );

            // Stretching a value onto a new range means any later threshold stops corresponding to
            // a physical control position, so the stages of a deadzone chain only compose while at
            // most one of them does it.
            let rescaling = binding
                .modifiers
                .iter()
                .filter(|modifier| modifier.rescales())
                .count();
            assert!(
                rescaling <= 1,
                "binding for {} chains {rescaling} rescaling modifiers; at most one may rescale, \
                 so all but one need `without_rescale`",
                binding.path
            );

            let slot = *slot_by_action.entry(binding.action).or_insert_with(|| {
                slot_intents.push(binding.intent);
                slot_intents.len() - 1
            });

            compiled.push(CompiledBinding {
                slot,
                source: binding.source,
                modifiers: binding.modifiers,
            });
        }

        // A stable sort is what makes declaration order the tiebreak between two contributions of
        // equal strength.
        compiled.sort_by_key(|binding| binding.slot);

        Self {
            bindings: compiled,
            slot_intents,
            slot_by_action,
            _marker: PhantomData,
        }
    }

    pub(crate) fn bindings(&self) -> &[CompiledBinding] {
        &self.bindings
    }

    pub(crate) fn slot_count(&self) -> usize {
        self.slot_intents.len()
    }

    pub(crate) fn intent_for_slot(&self, slot: usize) -> Intent {
        self.slot_intents[slot]
    }

    pub(crate) fn slot_for_action(&self, action: ActionId) -> Option<usize> {
        self.slot_by_action.get(&action).copied()
    }
}
