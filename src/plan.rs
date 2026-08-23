//! Compiling bindings into the plan the evaluator runs against.
//!
//! A plan is the immutable compiled form of authored bindings.

use alloc::{collections::BTreeMap, vec::Vec};
use core::marker::PhantomData;

use crate::action::{ActionId, ChannelShape, Intent};
use crate::binding::{BindingModifier, BindingSource, BindingSpec};
use crate::condition::BindingCondition;
use crate::event::Dispatch;

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
    pub(crate) conditions: Vec<BindingCondition>,
    pub(crate) consume: bool,
    #[cfg(any(feature = "keyboard", feature = "gamepad"))]
    pub(crate) chord: alloc::vec::Vec<crate::binding::ButtonControl>,
    // How specific this binding is: one for the control it names, plus one per control it requires
    // alongside. The clash between two bindings on one control is decided by this and nothing else.
    pub(crate) chord_len: u8,
    // Where this binding keeps its working memory: the modifiers first, then the conditions. No
    // two share a slot, even when they are the same kind.
    pub(crate) scratch_base: usize,
}

impl CompiledBinding {
    pub(crate) fn scratch_len(&self) -> usize {
        // Modifiers, then conditions, then one more for the press this binding derived — which is
        // hysteretic, so it has to remember what it decided last tick.
        self.modifiers.len() + self.conditions.len() + 1
    }
}

/// The plan is the immutable runtime view of a context's authored bindings.
// One slot per action, not per binding: an action may be bound several times, and all of those
// bindings write the same state. Bindings are grouped by slot so the evaluator can fold each
// action's contributions in a single pass with no per-frame bookkeeping.
pub struct Plan<C> {
    bindings: Vec<CompiledBinding>,
    slot_intents: Vec<Intent>,
    // Parallel to `slot_intents`: how a transition on this slot becomes a typed event.
    slot_dispatch: Vec<Dispatch>,
    slot_by_action: BTreeMap<ActionId, usize>,
    scratch_count: usize,
    has_chords: bool,
    _marker: PhantomData<C>,
}

impl<C> Plan<C> {
    /// Compiles a plan from authored bindings.
    pub(crate) fn from_bindings(bindings: Vec<BindingSpec>) -> Self {
        let mut slot_intents: Vec<Intent> = Vec::new();
        let mut slot_dispatch: Vec<Dispatch> = Vec::new();
        let mut slot_by_action = BTreeMap::new();
        let mut compiled = Vec::with_capacity(bindings.len());
        let mut scratch_count = 0;

        for binding in bindings {
            let source_shape = binding.source.channel_shape();

            // A modifier may change what kind of quantity the value is, and the check has to run
            // against what the action actually receives rather than what the control reported.
            let mut shape = source_shape;
            for modifier in &binding.modifiers {
                if let Some(reshaped) = modifier.reshapes() {
                    assert!(
                        shape != ChannelShape::Delta2,
                        "`{}` reads a control as a rate, but a {:?} channel already reports a \
                         displacement — there is no rate here to integrate",
                        binding.path,
                        source_shape
                    );
                    shape = reshaped;
                }
            }

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
                slot_dispatch.push(binding.dispatch);
                slot_intents.len() - 1
            });

            let scratch_base = scratch_count;
            scratch_count += binding.modifiers.len() + binding.conditions.len() + 1;

            compiled.push(CompiledBinding {
                slot,
                source: binding.source,
                modifiers: binding.modifiers,
                conditions: binding.conditions,
                consume: binding.consume,
                #[cfg(any(feature = "keyboard", feature = "gamepad"))]
                chord_len: 1 + u8::try_from(binding.chord.len()).unwrap_or(u8::MAX),
                #[cfg(not(any(feature = "keyboard", feature = "gamepad")))]
                chord_len: 1,
                #[cfg(any(feature = "keyboard", feature = "gamepad"))]
                chord: binding.chord,
                scratch_base,
            });
        }

        // A stable sort is what makes declaration order the tiebreak between two contributions of
        // equal strength.
        compiled.sort_by_key(|binding| binding.slot);

        let has_chords = compiled.iter().any(|binding| binding.chord_len > 1);

        Self {
            bindings: compiled,
            slot_intents,
            slot_dispatch,
            slot_by_action,
            scratch_count,
            has_chords,
            _marker: PhantomData,
        }
    }

    pub(crate) fn bindings(&self) -> &[CompiledBinding] {
        &self.bindings
    }

    pub(crate) fn slot_count(&self) -> usize {
        self.slot_intents.len()
    }

    pub(crate) fn scratch_count(&self) -> usize {
        self.scratch_count
    }

    /// Whether any binding requires a control held alongside its own.
    ///
    /// A plan with none skips the clash pass entirely, which is most plans.
    pub(crate) fn has_chords(&self) -> bool {
        self.has_chords
    }

    pub(crate) fn intent_for_slot(&self, slot: usize) -> Intent {
        self.slot_intents[slot]
    }

    pub(crate) fn dispatch_for_slot(&self, slot: usize) -> Dispatch {
        self.slot_dispatch[slot]
    }

    pub(crate) fn slot_for_action(&self, action: ActionId) -> Option<usize> {
        self.slot_by_action.get(&action).copied()
    }
}
