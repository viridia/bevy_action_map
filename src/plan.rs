//! Compiling bindings into the plan the evaluator runs against.
//!
//! A plan is the immutable compiled form of authored bindings.

use alloc::{collections::BTreeMap, vec::Vec};
use core::marker::PhantomData;

use crate::action::{ActionId, ChannelShape, Intent};
use crate::binding::{BindingModifier, BindingSource, BindingSpec, Control};
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

/// Something wrong with a context's bindings, found when they were compiled.
///
/// Collected rather than reported one at a time, so that a context with three mistakes in it tells
/// you about three mistakes rather than about the first one three times.
#[derive(Clone, Debug, PartialEq)]
pub struct BindingDiagnostic {
    /// The declared path of the action whose binding is at fault.
    pub action: &'static str,
    /// What is wrong with it.
    pub kind: DiagnosticKind,
}

impl BindingDiagnostic {
    /// Whether this stops the context working, or is only suspicious.
    pub fn severity(&self) -> Severity {
        match self.kind {
            DiagnosticKind::IntentMismatch { .. }
            | DiagnosticKind::RateFromDelta { .. }
            | DiagnosticKind::ChainedRescaling { .. } => Severity::Error,
            DiagnosticKind::MixedSchemeMapping
            | DiagnosticKind::DuplicateMappingKey { .. }
            | DiagnosticKind::ReservedAndMappable => Severity::Error,
            DiagnosticKind::DuplicateBinding { .. }
            | DiagnosticKind::ConsumeDisagreement { .. } => Severity::Warning,
        }
    }
}

/// How much a [`BindingDiagnostic`] matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// The binding cannot work as written, and the context is refused.
    Error,
    /// The binding will do something, but probably not what was meant.
    Warning,
}

/// What is wrong with a binding.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DiagnosticKind {
    /// The action's intent cannot be served by the channel its control reports on.
    IntentMismatch {
        /// What the action asked for.
        intent: Intent,
        /// What the control offers, after any modifier that reshapes it.
        shape: ChannelShape,
    },
    /// A modifier asked to read a displacement as though it were a rate.
    RateFromDelta {
        /// The channel the control reports on.
        shape: ChannelShape,
    },
    /// More than one modifier in the chain stretches its value onto a new range.
    ChainedRescaling {
        /// How many of them do.
        count: usize,
    },
    /// The same action reads the same control twice in this context.
    DuplicateBinding {
        /// The control bound twice.
        control: Control,
    },
    /// Two bindings read one control and disagree about consuming it.
    ConsumeDisagreement {
        /// The control they share.
        control: Control,
        /// The action on the other side of the disagreement.
        other: &'static str,
    },
    /// Two mappings would answer to the same name.
    DuplicateMappingKey {
        /// The name they share.
        key: crate::rebind::MappingKey,
    },
    /// A mappable binding reads controls from more than one kind of device.
    MixedSchemeMapping,
    /// A binding is declared both rebindable and reserved, which cannot both be true.
    ReservedAndMappable,
}

impl core::fmt::Display for BindingDiagnostic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.kind {
            DiagnosticKind::IntentMismatch { intent, shape } => write!(
                f,
                "`{}` has intent {:?}, which a control reporting on a {:?} channel cannot serve{}",
                self.action,
                intent,
                shape,
                mismatch_hint(*intent, *shape)
            ),
            DiagnosticKind::RateFromDelta { shape } => write!(
                f,
                "`{}` reads a control as a rate, but a {:?} channel already reports a displacement \
                 — there is no rate here to integrate",
                self.action, shape
            ),
            DiagnosticKind::ChainedRescaling { count } => write!(
                f,
                "`{}` chains {count} rescaling modifiers; at most one may rescale, so all but one \
                 need `without_rescale`",
                self.action
            ),
            DiagnosticKind::DuplicateBinding { control } => write!(
                f,
                "`{}` reads {:?} twice in this context. Both contribute, which for a delta action \
                 doubles it and for the rest is one binding doing nothing",
                self.action, control
            ),
            DiagnosticKind::ConsumeDisagreement { control, other } => write!(
                f,
                "`{}` and `{other}` both read {control:?}, but only one of them consumes it — so \
                 whether a lower-priority context sees that control depends on which of the two \
                 fired",
                self.action
            ),
            DiagnosticKind::DuplicateMappingKey { key } => write!(
                f,
                "`{}` declares a mapping named `{key}`, and so does something else. A saved \
                 rebinding of one would land on the other; give one of them a name with \
                 `mappable_as`",
                self.action
            ),
            DiagnosticKind::MixedSchemeMapping => write!(
                f,
                "`{}` is mappable but reads controls from more than one kind of device, so there \
                 is no one scheme to rebind it in. Bind the devices separately, one mappable \
                 binding each",
                self.action
            ),
            DiagnosticKind::ReservedAndMappable => write!(
                f,
                "`{}` is declared both mappable and reserved. Reserving withholds a control from \
                 capture so that it cannot be rebound; a mapping exists so that it can. Keep whichever \
                 one you meant",
                self.action
            ),
        }
    }
}

/// The channel a binding's value actually arrives on, after any modifier that reshapes it.
fn effective_shape(binding: &BindingSpec) -> Result<ChannelShape, DiagnosticKind> {
    let source_shape = binding.source.channel_shape();
    let mut shape = source_shape;
    for modifier in &binding.modifiers {
        if let Some(reshaped) = modifier.reshapes() {
            if shape == ChannelShape::Delta2 {
                return Err(DiagnosticKind::RateFromDelta {
                    shape: source_shape,
                });
            }
            shape = reshaped;
        }
    }
    Ok(shape)
}

/// Everything wrong with a set of authored bindings.
///
/// Pure: it reads the bindings and nothing else, so a rebinding UI can ask about a binding the
/// player has not committed to yet.
pub(crate) fn diagnose(bindings: &[BindingSpec]) -> Vec<BindingDiagnostic> {
    let mut found = Vec::new();
    // Mapping keys have to be unique across the whole context, so they are gathered as we go
    // than compared pairwise like the checks below.
    //
    // Keyed by scheme as well as by name, and remembering *which action* claimed each, because
    // neither kind of repeat is a mistake on its own. Uniqueness is per scheme (R19.15), so one
    // action on a key and on a pad button is two rows in two tables. And one action reaching a name
    // twice within one scheme is a primary and a secondary, which merge into a single row holding
    // both. What is left — two *different* actions answering to one name — is the case where a
    // saved rebinding of one would land on the other, and is what R19.15 wants reported.
    let mut keys = alloc::collections::BTreeMap::new();

    for (index, binding) in bindings.iter().enumerate() {
        let at = |kind| BindingDiagnostic {
            action: binding.path,
            kind,
        };

        match effective_shape(binding) {
            Ok(shape) if !binding.intent.accepts(shape) => {
                found.push(at(DiagnosticKind::IntentMismatch {
                    intent: binding.intent,
                    shape,
                }));
            }
            Ok(_) => {}
            Err(kind) => found.push(at(kind)),
        }

        let rescaling = binding
            .modifiers
            .iter()
            .filter(|modifier| modifier.rescales())
            .count();
        if rescaling > 1 {
            found.push(at(DiagnosticKind::ChainedRescaling { count: rescaling }));
        }

        if binding.reserved && binding.mappable.is_some() {
            found.push(at(DiagnosticKind::ReservedAndMappable));
        }

        if let Some(declaration) = binding.mappable {
            let prefix = declaration.prefix.unwrap_or(binding.path);
            let mut scheme = None;
            let mut mixed = false;
            binding.source.for_each_part(|part, control| {
                let key = crate::rebind::MappingKey::new(prefix, part);
                let claimant = keys
                    .entry((control.scheme(), key))
                    .or_insert(binding.action);
                if *claimant != binding.action {
                    found.push(at(DiagnosticKind::DuplicateMappingKey { key }));
                }
                match scheme {
                    Some(seen) if seen != control.scheme() => mixed = true,
                    Some(_) => {}
                    None => scheme = Some(control.scheme()),
                }
            });
            if mixed {
                found.push(at(DiagnosticKind::MixedSchemeMapping));
            }
        }

        // Against the bindings before this one only, so a duplicated pair is reported once.
        for earlier in &bindings[..index] {
            if earlier.action == binding.action && earlier.source == binding.source {
                binding.source.for_each_control(|control| {
                    found.push(at(DiagnosticKind::DuplicateBinding { control }));
                });
                break;
            }
            if earlier.consume != binding.consume {
                earlier.source.for_each_control(|theirs| {
                    binding.source.for_each_control(|mine| {
                        if theirs == mine {
                            found.push(at(DiagnosticKind::ConsumeDisagreement {
                                control: mine,
                                other: earlier.path,
                            }));
                        }
                    });
                });
            }
        }
    }

    found
}

/// An authored binding with its action resolved to a state slot.
pub(crate) struct CompiledBinding {
    pub(crate) slot: usize,
    pub(crate) source: BindingSource,
    pub(crate) modifiers: Vec<BindingModifier>,
    pub(crate) conditions: Vec<BindingCondition>,
    pub(crate) consume: bool,
    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
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
    // Parallel again: the declared path of the action holding this slot, kept for the diagnostics
    // that have to name an action rather than identify one.
    slot_paths: Vec<&'static str>,
    // And its identity, for the reads that walk a context rather than naming what they want.
    slot_actions: Vec<ActionId>,
    slot_by_action: BTreeMap<ActionId, usize>,
    scratch_count: usize,
    has_chords: bool,
    _marker: PhantomData<C>,
}

impl<C> Plan<C> {
    /// Compiles a plan from authored bindings.
    ///
    // Compilation asks nothing about whether the bindings make sense: `diagnose` owns that, and
    // `add_context` runs it first and refuses the context rather than compiling a plan that cannot
    // work. Keeping the two apart is what lets a rebinding UI ask about bindings it has no
    // intention of installing.
    pub(crate) fn from_bindings(bindings: Vec<BindingSpec>) -> Self {
        let mut slot_intents: Vec<Intent> = Vec::new();
        let mut slot_dispatch: Vec<Dispatch> = Vec::new();
        let mut slot_paths: Vec<&'static str> = Vec::new();
        let mut slot_actions: Vec<ActionId> = Vec::new();
        let mut slot_by_action = BTreeMap::new();
        let mut compiled = Vec::with_capacity(bindings.len());
        let mut scratch_count = 0;

        for binding in bindings {
            let slot = *slot_by_action.entry(binding.action).or_insert_with(|| {
                slot_intents.push(binding.intent);
                slot_dispatch.push(binding.dispatch);
                slot_paths.push(binding.path);
                slot_actions.push(binding.action);
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
                #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
                chord_len: 1 + u8::try_from(binding.chord.len()).unwrap_or(u8::MAX),
                #[cfg(not(any(feature = "keyboard", feature = "mouse", feature = "gamepad")))]
                chord_len: 1,
                #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
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
            slot_paths,
            slot_actions,
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

    /// The declared paths of every action this context binds, in slot order.
    pub(crate) fn bound_paths(&self) -> &[&'static str] {
        &self.slot_paths
    }

    /// The identity of every action this context binds, in slot order.
    pub(crate) fn slot_actions(&self) -> &[ActionId] {
        &self.slot_actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding::InputContextBuilder;

    #[derive(crate::InputAction)]
    #[action(path = "plan_tests.jump", output = bool, intent = Button)]
    struct Jump;

    #[derive(crate::InputAction)]
    #[action(path = "plan_tests.menu", output = bool, intent = Button)]
    struct MenuToggle;

    #[derive(crate::InputAction)]
    #[action(path = "plan_tests.move", output = bevy_math::Vec2, intent = Directional2)]
    struct Move;

    /// Bound twice to the same control: harmless for a button, doubling for a delta, and a mistake
    /// either way. Reported against the second one, once, rather than once per binding in the pair.
    #[cfg(feature = "keyboard")]
    #[test]
    fn one_control_bound_twice_is_reported_once() {
        use bevy_input::keyboard::KeyCode;

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<Jump>(KeyCode::Space);
        builder.bind::<Jump>(KeyCode::Space);

        let found = builder.diagnostics();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].severity(), Severity::Warning);
        assert_eq!(
            found[0].kind,
            DiagnosticKind::DuplicateBinding {
                control: Control::Key(KeyCode::Space)
            }
        );
    }

    /// Two bindings on one control where only one consumes it. Whether a lower-priority context
    /// ever sees that control then depends on which of the two fired, which is not a thing anyone
    /// can reason about from the declaration.
    #[cfg(feature = "keyboard")]
    #[test]
    fn disagreeing_about_consuming_one_control_is_reported() {
        use bevy_input::keyboard::KeyCode;

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<MenuToggle>(KeyCode::Escape).consume();
        builder.bind::<Jump>(KeyCode::Escape);

        let found = builder.diagnostics();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].kind,
            DiagnosticKind::ConsumeDisagreement {
                control: Control::Key(KeyCode::Escape),
                other: "plan_tests.menu",
            }
        );
    }

    /// Bindings that touch different controls are none of each other's business, however their
    /// consume flags read.
    #[cfg(feature = "keyboard")]
    #[test]
    fn consuming_a_control_nobody_else_reads_is_fine() {
        use bevy_input::keyboard::KeyCode;

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<MenuToggle>(KeyCode::Escape).consume();
        builder.bind::<Jump>(KeyCode::Space);

        assert_eq!(builder.diagnostics(), &[]);
    }

    /// The reason this is a list rather than an assertion: three mistakes should cost one run to
    /// find, not three runs to find one at a time.
    #[cfg(feature = "keyboard")]
    #[test]
    fn every_problem_is_reported_together() {
        use bevy_input::keyboard::KeyCode;

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<Move>(KeyCode::KeyW);
        builder.bind::<Move>(KeyCode::KeyA);
        builder.bind::<Jump>(KeyCode::Space);
        builder.bind::<Jump>(KeyCode::Space);

        let found = builder.diagnostics();
        assert_eq!(found.len(), 3, "{found:?}");
        assert_eq!(
            found
                .iter()
                .filter(|d| d.severity() == Severity::Error)
                .count(),
            2,
            "both directional bindings are refused"
        );
        assert_eq!(
            found
                .iter()
                .filter(|d| d.severity() == Severity::Warning)
                .count(),
            1,
            "and the duplicate is only suspicious"
        );
    }
}
