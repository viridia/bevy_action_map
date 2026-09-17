//! Compiling bindings into the plan the evaluator runs against.

use alloc::{collections::BTreeMap, vec::Vec};
use core::marker::PhantomData;

use crate::action::{ActionId, ActionIntent, ChannelShape};
use crate::binding::{
    BindingInput, BindingModifier, BindingSpec, ClassBindingSpec, CombinedSpec, Control,
    DelegatedSpec,
};
use crate::capture::{ClassFilter, ControlClass};
use crate::condition::BindingCondition;
use crate::event::{ClassDispatch, Dispatch};

/// The part of a rejected binding's message that says what to do about it.
///
/// Only the two mistakes with a specific remedy get one; the rest are adequately explained by
/// naming the intent and the channel that cannot serve it.
fn mismatch_hint(intent: ActionIntent, shape: ChannelShape) -> &'static str {
    match (intent, shape) {
        (ActionIntent::Directional2, ChannelShape::Button | ChannelShape::Axis1) => {
            ". A single control carries no direction — bind a directional composite, whose parts \
             can be keyboard keys or D-pad buttons"
        }
        (ActionIntent::Delta2, _) | (_, ChannelShape::Delta2) => {
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
            DiagnosticKind::DuplicateMappingKey { .. }
            | DiagnosticKind::RebindingDisagreement { .. }
            | DiagnosticKind::ReservedAndMappable
            | DiagnosticKind::FollowsNothing { .. }
            | DiagnosticKind::FollowsUnlisted { .. }
            | DiagnosticKind::DuplicateTunableKey { .. }
            | DiagnosticKind::TunableShapeDisagreement { .. }
            | DiagnosticKind::BoundAndDelegated
            | DiagnosticKind::CombinedWithoutBindings => Severity::Error,
            DiagnosticKind::DuplicateBinding { .. }
            | DiagnosticKind::ConsumeDisagreement { .. }
            | DiagnosticKind::DuplicateClassBinding { .. }
            | DiagnosticKind::DeadZoneAtFullDeflection { .. } => Severity::Warning,
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
        intent: ActionIntent,
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
        key: crate::mapping::MappingKey,
    },
    /// Two bindings feeding one mapping disagree about whether the player may change it.
    RebindingDisagreement {
        /// The name they share.
        key: crate::mapping::MappingKey,
    },
    /// A binding is declared both rebindable and reserved, which cannot both be true.
    ReservedAndMappable,
    /// A binding follows an action that reads nothing like it in this context.
    FollowsNothing {
        /// The action it was told to follow.
        target: &'static str,
    },
    /// A binding follows one that is itself off the presentation list.
    FollowsUnlisted {
        /// The action it was told to follow.
        target: &'static str,
    },
    /// Two class bindings in one context watch the same class.
    DuplicateClassBinding {
        /// The shape class they share, or `None` for two bindings both claiming
        /// character-producing keys.
        class: Option<ControlClass>,
    },
    /// Two different actions declare a tunable under the same name in the same family.
    DuplicateTunableKey {
        /// The name they share.
        key: &'static str,
    },
    /// Two bindings sharing a tunable disagree about its shape.
    TunableShapeDisagreement {
        /// The name they share.
        key: &'static str,
    },
    /// A deadzone is declared at or beyond full deflection, where ordinary input never escapes it.
    DeadZoneAtFullDeflection {
        /// The declared lower bound.
        lower: f32,
    },
    /// An action is both bound in this context and delegated to an outside authority.
    BoundAndDelegated,
    /// An action shapes its combined value, and has no bindings in this context to combine.
    CombinedWithoutBindings,
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
            DiagnosticKind::RebindingDisagreement { key } => write!(
                f,
                "`{}` feeds the mapping `{key}` from two bindings that disagree about whether the \
                 player may change it — one is `mappable` and the other is not. One row cannot be \
                 both; say the same thing on every binding that feeds it",
                self.action
            ),
            DiagnosticKind::ReservedAndMappable => write!(
                f,
                "`{}` is declared both mappable and reserved. Reserving withholds a control from \
                 capture so that it cannot be rebound; a mapping exists so that it can. Keep whichever \
                 one you meant",
                self.action
            ),
            DiagnosticKind::FollowsNothing { target } => write!(
                f,
                "`{}` follows `{target}`, but no binding of `{target}` in this context reads the \
                 same controls. A binding rides the one it reads alongside, so the two must read \
                 the same thing — check the spelling, and check that both devices are bound",
                self.action
            ),
            DiagnosticKind::FollowsUnlisted { target } => write!(
                f,
                "`{}` follows `{target}`, which is itself off the controls screen, so there is no \
                 mapping to ride. Take `private` off the binding it follows, or make this one \
                 `private` too and accept that rebinding will not move it",
                self.action
            ),
            DiagnosticKind::DuplicateClassBinding { class } => {
                let watched = match class {
                    Some(class) => alloc::format!("the class {class:?}"),
                    None => "character-producing keys".into(),
                };
                write!(
                    f,
                    "`{}` binds {watched}, and so does something else in this context. The \
                     first one declared claims every matching control; the second can never fire",
                    self.action
                )
            }
            DiagnosticKind::DuplicateTunableKey { key } => write!(
                f,
                "`{}` declares a tunable named `{key}`, and so does something else. A saved \
                 change to one would land on the other; give one of them a name of its own",
                self.action
            ),
            DiagnosticKind::TunableShapeDisagreement { key } => write!(
                f,
                "`{}` shares the tunable `{key}` with another binding, but they disagree about \
                 its shape — a switch on one side and a range on the other, or two ranges with \
                 different bounds. Every binding sharing a tunable must agree",
                self.action
            ),
            DiagnosticKind::DeadZoneAtFullDeflection { lower } => write!(
                f,
                "`{}` declares a deadzone at {lower}, at or beyond full deflection. Ordinary \
                 input never escapes it — only a control whose magnitude overshoots 1.0, such as \
                 mouse motion, produces anything",
                self.action
            ),
            DiagnosticKind::BoundAndDelegated => write!(
                f,
                "`{}` is bound to a control in this context and also delegated to an outside \
                 authority. Only one of the two can decide what the action does; drop whichever \
                 is not the authority here",
                self.action
            ),
            DiagnosticKind::CombinedWithoutBindings => write!(
                f,
                "`{}` declares `combined`, but has no bindings in this context whose values it \
                 could combine. Bind it here, or drop the declaration",
                self.action
            ),
        }
    }
}

/// The channel a binding's value actually arrives on, after any modifier that reshapes it.
fn effective_shape(binding: &BindingSpec) -> Result<ChannelShape, DiagnosticKind> {
    let input_shape = binding.input.channel_shape();
    let mut shape = input_shape;
    for modifier in &binding.modifiers {
        if let Some(reshaped) = modifier.reshapes() {
            if shape == ChannelShape::Delta2 {
                return Err(DiagnosticKind::RateFromDelta { shape: input_shape });
            }
            shape = reshaped;
        }
    }
    Ok(shape)
}

/// Whether two bindings sharing a tunable key agree about what it is — a switch on both sides, or
/// a range on both with identical bounds. The current *value* is not compared: two bindings can
/// declare the same default honestly and still be mid-diff on file, and only the shape is what a
/// shared runtime latch actually needs to agree on.
fn tunable_shapes_agree(a: crate::mapping::TunableValue, b: crate::mapping::TunableValue) -> bool {
    use crate::mapping::TunableValue::{Bool, Range};
    match (a, b) {
        (Bool(_), Bool(_)) => true,
        (
            Range {
                min: min_a,
                max: max_a,
                ..
            },
            Range {
                min: min_b,
                max: max_b,
                ..
            },
        ) => min_a == min_b && max_a == max_b,
        _ => false,
    }
}

/// Everything wrong with a set of authored bindings.
///
/// Pure: it reads the bindings and nothing else, so a rebinding UI can ask about a binding the
/// player has not committed to yet.
pub(crate) fn diagnose(bindings: &[BindingSpec]) -> Vec<BindingDiagnostic> {
    let mut found = Vec::new();
    // Mapping keys have to be unique across the whole context, so they are gathered as we go
    // rather than compared pairwise like the checks below.
    //
    // Keyed by family as well as by name, and remembering *which action* claimed each, because
    // neither kind of repeat is a mistake on its own. Uniqueness is per family (R19.15), so one
    // action on a key and on a pad button is two rows in two tables. And one action reaching a name
    // twice within one family is a primary and a secondary, which merge into a single row holding
    // both. What is left — two *different* actions answering to one name — is the case where a
    // saved rebinding of one would land on the other, and is what wants reporting.
    let mut keys = alloc::collections::BTreeMap::new();
    // A tunable's key is unique per family for the same reason a mapping's is: two different
    // actions sharing one name is a saved change to one landing on the other. Two bindings of the
    // *same* action sharing one name is deliberate — `hold_or_toggle` declares exactly that, so
    // every eligible binding shares one runtime latch — provided they agree about the tunable's
    // shape, which is the one thing sharing a name cannot paper over.
    let mut tunable_keys: alloc::collections::BTreeMap<
        (crate::device::DeviceFamily, &'static str),
        (ActionId, crate::mapping::TunableValue),
    > = alloc::collections::BTreeMap::new();
    // Where the diagnostics of the current `bind` call begin.
    let mut declaration_start = 0;

    for (index, binding) in bindings.iter().enumerate() {
        let before = found.len();
        if !binding.continues_declaration {
            declaration_start = before;
        }
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

        // Only the declared value: a player driving `lower` there at runtime through
        // `tunable_dead_zone` is a case this build-time check cannot reach.
        for modifier in &binding.modifiers {
            if let BindingModifier::DeadZone(dead_zone) = modifier
                && dead_zone.lower >= 1.0
            {
                found.push(at(DiagnosticKind::DeadZoneAtFullDeflection {
                    lower: dead_zone.lower,
                }));
            }
        }

        // Reserving contradicts *rebindability*, not listing: a reserved control is one nothing may
        // be bound over, and showing the player which control that is helps rather than hurts.
        if binding.reserved
            && binding
                .mapping
                .is_some_and(|decl| decl.rebind_policy.is_rebindable())
        {
            found.push(at(DiagnosticKind::ReservedAndMappable));
        }

        // Resolution is by the controls the two bindings read, so the two ways it fails are "no
        // binding of that action reads this" and "one does, and has no mapping to lend". The second
        // is worth its own diagnostic because the fix is on the *other* binding.
        if let Some(follows) = binding.follows
            && crate::mapping::leader_of(bindings, index).is_none()
        {
            let reads_the_same = bindings.iter().enumerate().any(|(other, spec)| {
                other != index && spec.action == follows.action && spec.input == binding.input
            });
            found.push(at(if reads_the_same {
                DiagnosticKind::FollowsUnlisted {
                    target: follows.path,
                }
            } else {
                DiagnosticKind::FollowsNothing {
                    target: follows.path,
                }
            }));
        }

        if let Some(declaration) = binding.mapping {
            let prefix = declaration.prefix.unwrap_or(binding.path);
            let rebindable = declaration.rebind_policy.is_rebindable();
            binding.input.for_each_part(|part, control| {
                let key = crate::mapping::MappingKey::new(prefix, part);
                let (claimant, claimed_as) = keys
                    .entry((control.family(), key))
                    .or_insert((binding.action, declaration.rebind_policy));
                if *claimant != binding.action {
                    // Only where something is rebindable, because the hazard is a *saved* rebind of
                    // one row landing on another and a fixed row is never saved. Two fixed rows
                    // under one name are a display oddity; erroring on them would fail the build of
                    // games that want nothing to do with rebinding at all (R19.13).
                    if rebindable || claimed_as.is_rebindable() {
                        found.push(at(DiagnosticKind::DuplicateMappingKey { key }));
                    }
                } else if *claimed_as != declaration.rebind_policy {
                    found.push(at(DiagnosticKind::RebindingDisagreement { key }));
                }
            });
        }

        if let Some(decl) = &binding.tunable
            && let Some(family) = crate::mapping::binding_family(&binding.input)
        {
            match tunable_keys.entry((family, decl.key)) {
                alloc::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert((binding.action, decl.default));
                }
                alloc::collections::btree_map::Entry::Occupied(entry) => {
                    let &(claimant, shape) = entry.get();
                    if claimant != binding.action {
                        found.push(at(DiagnosticKind::DuplicateTunableKey { key: decl.key }));
                    } else if !tunable_shapes_agree(shape, decl.default) {
                        found.push(at(DiagnosticKind::TunableShapeDisagreement {
                            key: decl.key,
                        }));
                    }
                }
            }
        }

        // Against the bindings before this one only, so a duplicated pair is reported once.
        for earlier in &bindings[..index] {
            if earlier.action == binding.action && earlier.input == binding.input {
                binding.input.for_each_control(|control| {
                    found.push(at(DiagnosticKind::DuplicateBinding { control }));
                });
                break;
            }
            if earlier.consume != binding.consume {
                earlier.input.for_each_control(|theirs| {
                    binding.input.for_each_control(|mine| {
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

        // A composite's parts share every combinator, so they are wrong together, and one report
        // of what the author wrote once is enough.
        if binding.continues_declaration {
            let (reported, new) = found.split_at(before);
            let new: Vec<_> = new
                .iter()
                .filter(|diagnostic| !reported[declaration_start..].contains(diagnostic))
                .cloned()
                .collect();
            found.truncate(before);
            found.extend(new);
        }
    }

    found
}

/// Everything wrong with a set of authored class bindings.
///
/// One check, deliberately: two class bindings in one context watching the same filter mean the
/// second can never fire, since arbitration between class bindings is declaration order with no
/// per-tick contest to decide it otherwise. Two *different* filters that happen to overlap —
/// `AnyButton` and the character-producing door both match a letter key — are not reported;
/// declaring both, in a chosen order, is how an app says "claim character keys first, then
/// everything else," the same tiebreak plain bindings already use.
pub(crate) fn diagnose_classes(bindings: &[ClassBindingSpec]) -> Vec<BindingDiagnostic> {
    let mut found = Vec::new();
    for (index, binding) in bindings.iter().enumerate() {
        if bindings[..index]
            .iter()
            .any(|earlier| earlier.filter == binding.filter)
        {
            found.push(BindingDiagnostic {
                action: binding.action_path,
                kind: DiagnosticKind::DuplicateClassBinding {
                    class: match binding.filter {
                        ClassFilter::Shape(class) => Some(class),
                        ClassFilter::Characters => None,
                    },
                },
            });
        }
    }
    found
}

/// Whether anything a context delegated is also bound in it.
///
/// The only way the two declarations can contradict each other. Everything else a binding carries —
/// a modifier, a condition, a mapping row — has no delegated counterpart to disagree with, because
/// `delegate` offers no way to say it.
pub(crate) fn diagnose_delegated(
    bindings: &[BindingSpec],
    delegated: &[DelegatedSpec],
) -> Vec<BindingDiagnostic> {
    delegated
        .iter()
        .filter(|spec| bindings.iter().any(|binding| binding.action == spec.action))
        .map(|spec| BindingDiagnostic {
            action: spec.path,
            kind: DiagnosticKind::BoundAndDelegated,
        })
        .collect()
}

/// Whether a `combined` declaration has anything to combine, and whether its chain rescales twice.
///
/// Refused rather than ignored: a clamp that silently never runs is the mistake most worth hearing
/// about. A delegated action lands here too, since it has no bindings.
pub(crate) fn diagnose_combined(
    bindings: &[BindingSpec],
    combined: &[CombinedSpec],
) -> Vec<BindingDiagnostic> {
    let mut found = Vec::new();
    for spec in combined {
        let at = |kind| BindingDiagnostic {
            action: spec.path,
            kind,
        };
        let rescales = |modifiers: &[BindingModifier]| {
            modifiers
                .iter()
                .filter(|modifier| modifier.rescales())
                .count()
        };
        let own = bindings
            .iter()
            .filter(|binding| binding.action == spec.action);
        let Some(upstream) = own.map(|binding| rescales(&binding.modifiers)).max() else {
            found.push(at(DiagnosticKind::CombinedWithoutBindings));
            continue;
        };
        // The stage is the tail of every binding's chain, so it rescales twice if it rescales after
        // a binding that already did. A binding's own double is `diagnose`'s to report.
        let stage = rescales(&spec.modifiers);
        if stage > 0 && upstream + stage > 1 {
            found.push(at(DiagnosticKind::ChainedRescaling {
                count: upstream + stage,
            }));
        }
    }
    found
}

/// What `combined` declared for one slot, with its working memory placed.
// `Default` is the slot that declared nothing: two empty `Vec`s, which allocate nothing.
#[derive(Clone, Default)]
pub(crate) struct CompiledStage {
    pub(crate) modifiers: Vec<BindingModifier>,
    pub(crate) conditions: Vec<BindingCondition>,
    // After every binding's scratch, so the bindings' own layout is the same with or without it.
    pub(crate) scratch_base: usize,
}

impl CompiledStage {
    pub(crate) fn is_empty(&self) -> bool {
        self.modifiers.is_empty() && self.conditions.is_empty()
    }
}

/// An authored binding with its action resolved to a state slot.
pub(crate) struct CompiledBinding {
    pub(crate) slot: usize,
    pub(crate) input: BindingInput,
    pub(crate) modifiers: Vec<BindingModifier>,
    pub(crate) conditions: Vec<BindingCondition>,
    pub(crate) consume: bool,
    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    pub(crate) chord: alloc::vec::Vec<crate::binding::ChordEntry>,
    // How specific this binding is: one for the control it names, plus one per control it requires
    // alongside. The clash between two bindings on one control is decided by this and nothing else.
    pub(crate) chord_len: u8,
    // Where this binding keeps its working memory: the modifiers, then the conditions, then the
    // press it derived. No two share a slot, even when they are the same kind.
    pub(crate) scratch_base: usize,
    // Only a `Button` action thresholds a value into a press, so only its bindings get a slot to
    // remember one in.
    pub(crate) press_slot: bool,
    // Set when this binding's tunable is shared with at least one other binding — `hold_or_toggle`
    // reaching a primary and a secondary key, most often — to the index of the plan's shared cell
    // for the group. `None` is the ordinary case: the modifier keeps the private slot
    // `scratch_base` already gives it, and the binding runs its own chain. A binding with `Some`
    // skips its own chain entirely instead of running it against a cell other bindings also write —
    // see `resolve_shared_toggle`'s doc for why running it per binding is unsafe rather than merely
    // redundant.
    pub(crate) tunable_shared: Option<usize>,
}

impl CompiledBinding {
    pub(crate) fn scratch_len(&self) -> usize {
        // The press gets a slot of its own because it is hysteretic: it has to remember what it
        // decided last tick.
        self.modifiers.len() + self.conditions.len() + usize::from(self.press_slot)
    }
}

/// An authored class binding, carried through compilation unchanged — there is no slot, because
/// there is nothing to hold between ticks.
///
/// No `action_path` here, unlike `BindingSpec`/`CompiledBinding`: the one diagnostic that needs to
/// name a class binding's action runs on the authored `ClassBindingSpec` list before compilation,
/// and evaluation never has to name one back to a person.
#[derive(Clone)]
pub(crate) struct CompiledClassBinding {
    pub(crate) filter: ClassFilter,
    pub(crate) consume: bool,
    pub(crate) dispatch: ClassDispatch,
}

/// What `Plan::slot_by_action` holds for an action this context does not bind, and therefore also
/// the ceiling on slots in one context — 65535 actions, which no plan approaches.
const UNBOUND: u16 = u16::MAX;

/// Encodes a freshly allocated slot for the reverse index.
///
/// The sentinel is not a slot, so the ceiling is one below it rather than `u16::MAX` — storing slot
/// 65535 would encode as `UNBOUND` and read back as an action this context does not have.
/// App-build, not runtime (R24.4): a context this size is a mistake in the declaration, and there
/// is no shipped build in which it happens.
fn encode_slot(slot: usize) -> u16 {
    u16::try_from(slot)
        .ok()
        .filter(|&slot| slot != UNBOUND)
        .unwrap_or_else(|| panic!("a context may hold at most {UNBOUND} actions"))
}

/// The plan is the immutable runtime view of a context's authored bindings.
// One slot per action, not per binding: an action may be bound several times, and all of those
// bindings write the same state. Bindings are grouped by slot so the evaluator can fold each
// action's contributions in a single pass with no per-frame bookkeeping.
pub struct Plan<C> {
    bindings: Vec<CompiledBinding>,
    slot_intents: Vec<ActionIntent>,
    // Parallel to `slot_intents`: how a transition on this slot becomes a typed event.
    slot_dispatch: Vec<Dispatch>,
    // Parallel again: the declared path of the action holding this slot, kept for the diagnostics
    // that have to name an action rather than identify one.
    slot_paths: Vec<&'static str>,
    // And its identity, for the reads that walk a context rather than naming what they want.
    slot_actions: Vec<ActionId>,
    // The reverse direction, as a direct index rather than a search: `ActionId` is dense, so the
    // id is the subscript and `UNBOUND` means this context does not bind it. Sized by the largest
    // id the context binds rather than by the registry, and held once per plan rather than per
    // instance, so the slack costs two bytes an id in one allocation.
    slot_by_action: Vec<u16>,
    // Parallel to `slot_intents`: what `combined` runs on the slot's folded value. Held per slot
    // rather than as a list to search, so a slot that declared nothing costs one emptiness check.
    stages: Vec<CompiledStage>,
    // The slots an outside authority writes rather than the fold. Held as a list rather than a bit
    // per slot because the evaluator only ever walks it, and the overwhelmingly common plan
    // delegates nothing.
    delegated_slots: Vec<usize>,
    scratch_count: usize,
    // One cell per group of bindings sharing a tunable — see `CompiledBinding::tunable_shared`.
    // Most plans have none.
    tunable_scratch_count: usize,
    // Read only by the clash pass, which no build without device features has: no controls means
    // no binding can carry a chord. Computed unconditionally so the builder needs no `cfg`.
    #[cfg_attr(
        not(any(feature = "keyboard", feature = "mouse", feature = "gamepad")),
        allow(dead_code)
    )]
    has_chords: bool,
    // TD5.4's second structure: consulted only when `indexed_controls` doesn't
    // already claim the control an event arrived on.
    class_bindings: Vec<CompiledClassBinding>,
    // Every control any binding above reads, deduped. Not an arbitration index — a class binding
    // never competes for a control on specificity; it yields whenever this set claims one.
    indexed_controls: Vec<Control>,
    _marker: PhantomData<C>,
}

impl<C> Plan<C> {
    /// Compiles a plan from authored bindings.
    // Compilation takes the bindings as sound; `diagnose` is what decides whether they are.
    pub(crate) fn from_bindings(
        bindings: Vec<BindingSpec>,
        class_bindings: Vec<ClassBindingSpec>,
    ) -> Self {
        let mut plan = Self::compile(bindings, None);
        plan.class_bindings = class_bindings
            .into_iter()
            .map(|spec| CompiledClassBinding {
                filter: spec.filter,
                consume: spec.consume,
                dispatch: spec.dispatch,
            })
            .collect();
        plan
    }

    /// Compiles a variant of `template` — the same actions, driven by different controls.
    ///
    /// What an override applies as. The slot allocation is `template`'s rather than derived afresh,
    /// so an action whose every binding the player unbound keeps its slot and reads at rest rather
    /// than raising the "not bound in this context" warning, which is a typo diagnostic and not
    /// what happened here; and slot indices stay put across the swap, so an instance's action
    /// states and require-reset flags stay aligned with no rebuilding.
    ///
    /// A binding for an action the template does not have would still get a slot of its own, which
    /// cannot happen: a variant only rewrites the inputs of bindings the template already holds.
    ///
    /// Class bindings are not part of the diff — they are never rebindable, so they carry over from
    /// `template` unchanged rather than being rebuilt from a list that would be a copy of them.
    pub(crate) fn variant_of(template: &Self, bindings: Vec<BindingSpec>) -> Self {
        let mut plan = Self::compile(bindings, Some(template));
        plan.class_bindings.clone_from(&template.class_bindings);
        // Carried over for the same reason class bindings are: an override rewrites which controls
        // a binding reads, and a delegated action has none to rewrite. The slots themselves survive
        // already, since `compile` starts from the template's slot tables.
        plan.delegated_slots.clone_from(&template.delegated_slots);
        // Never rebindable either, but not copied whole: the bindings' scratch may have changed
        // length, and the stages' sits after it.
        plan.stages.clone_from(&template.stages);
        plan.place_stages();
        plan
    }

    /// Attaches what `combined` declared to the slots its actions hold.
    ///
    /// After compilation, as `delegate` is, and for the same reason. An action with no slot is
    /// never reached: `diagnose_combined` refuses the context first.
    pub(crate) fn combine(&mut self, combined: Vec<CombinedSpec>) {
        self.stages
            .resize_with(self.slot_intents.len(), CompiledStage::default);
        for spec in combined {
            let Some(slot) = self.slot_for_action(spec.action) else {
                continue;
            };
            self.stages[slot].modifiers.extend(spec.modifiers);
            self.stages[slot].conditions.extend(spec.conditions);
        }
        self.place_stages();
    }

    fn place_stages(&mut self) {
        for stage in self.stages.iter_mut().filter(|stage| !stage.is_empty()) {
            stage.scratch_base = self.scratch_count;
            self.scratch_count += stage.modifiers.len() + stage.conditions.len();
        }
    }

    fn compile(bindings: Vec<BindingSpec>, template: Option<&Self>) -> Self {
        let mut slot_intents: Vec<ActionIntent> = Vec::new();
        let mut slot_dispatch: Vec<Dispatch> = Vec::new();
        let mut slot_paths: Vec<&'static str> = Vec::new();
        let mut slot_actions: Vec<ActionId> = Vec::new();
        let mut slot_by_action: Vec<u16> = Vec::new();
        if let Some(template) = template {
            slot_intents.clone_from(&template.slot_intents);
            slot_dispatch.clone_from(&template.slot_dispatch);
            slot_paths.clone_from(&template.slot_paths);
            slot_actions.clone_from(&template.slot_actions);
            slot_by_action.clone_from(&template.slot_by_action);
        }
        let mut compiled = Vec::with_capacity(bindings.len());
        let mut scratch_count = 0;

        // Bindings sharing a `Bool`-shaped tunable key, within one family, get one shared scratch
        // cell instead of each keeping its own — see `CompiledBinding::tunable_shared`. Computed up
        // front, against every binding at once, since a group is only a group once every member is
        // known; a `Range` tunable never joins one, because `DeadZone`'s modifier holds no runtime
        // state to share in the first place.
        let mut tunable_groups: BTreeMap<(crate::device::DeviceFamily, &'static str), Vec<usize>> =
            BTreeMap::new();
        for (index, binding) in bindings.iter().enumerate() {
            let Some(decl) = &binding.tunable else {
                continue;
            };
            if !matches!(decl.default, crate::mapping::TunableValue::Bool(_)) {
                continue;
            }
            let Some(family) = crate::mapping::binding_family(&binding.input) else {
                continue;
            };
            tunable_groups
                .entry((family, decl.key))
                .or_default()
                .push(index);
        }
        let mut tunable_shared: BTreeMap<usize, usize> = BTreeMap::new();
        let mut tunable_scratch_count = 0;
        for indices in tunable_groups.values() {
            if indices.len() < 2 {
                continue;
            }
            let scratch_index = tunable_scratch_count;
            tunable_scratch_count += 1;
            for &index in indices {
                tunable_shared.insert(index, scratch_index);
            }
        }

        for (index, binding) in bindings.into_iter().enumerate() {
            let id = binding.action.index() as usize;
            if id >= slot_by_action.len() {
                slot_by_action.resize(id + 1, UNBOUND);
            }
            let slot = match slot_by_action[id] {
                UNBOUND => {
                    slot_intents.push(binding.intent);
                    slot_dispatch.push(binding.dispatch);
                    slot_paths.push(binding.path);
                    slot_actions.push(binding.action);
                    let slot = slot_intents.len() - 1;
                    slot_by_action[id] = encode_slot(slot);
                    slot
                }
                slot => usize::from(slot),
            };

            let scratch_base = scratch_count;
            let press_slot = binding.intent == ActionIntent::Button;
            scratch_count +=
                binding.modifiers.len() + binding.conditions.len() + usize::from(press_slot);

            compiled.push(CompiledBinding {
                slot,
                input: binding.input,
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
                press_slot,
                tunable_shared: tunable_shared.get(&index).copied(),
            });
        }

        // Contiguous per slot, which is how the fold visits one action's bindings together.
        compiled.sort_by_key(|binding| binding.slot);

        let has_chords = compiled.iter().any(|binding| binding.chord_len > 1);
        let slot_count = slot_intents.len();

        // Recomputed on every compile, including a variant's: an override rewrites which controls
        // these bindings read, so a rebind has to move a control between "indexed" and "not" along
        // with everything else — unlike `class_bindings`, which is never part of that diff.
        let mut indexed_controls: Vec<Control> = Vec::new();
        for binding in &compiled {
            binding.input.for_each_control(|control| {
                if !indexed_controls.contains(&control) {
                    indexed_controls.push(control);
                }
            });
        }

        Self {
            bindings: compiled,
            slot_intents,
            slot_dispatch,
            slot_paths,
            slot_actions,
            slot_by_action,
            stages: alloc::vec![CompiledStage::default(); slot_count],
            delegated_slots: Vec::new(),
            scratch_count,
            tunable_scratch_count,
            class_bindings: Vec::new(),
            indexed_controls,
            has_chords,
            _marker: PhantomData,
        }
    }

    /// Gives each delegated action a state slot of its own, with no binding behind it.
    ///
    /// Applied after compilation rather than during it, so that the slots bindings allocated keep
    /// the indices they already have and a variant compiled from this plan needs no rebuilding.
    pub(crate) fn delegate(&mut self, delegated: Vec<DelegatedSpec>) {
        for spec in delegated {
            let id = spec.action.index() as usize;
            if id >= self.slot_by_action.len() {
                self.slot_by_action.resize(id + 1, UNBOUND);
            }
            // Never an action a binding already holds: `diagnose_delegated` reports that as an
            // error and `add_context` refuses the context before anything is compiled.
            let slot = match self.slot_by_action[id] {
                UNBOUND => {
                    self.slot_intents.push(spec.intent);
                    self.slot_dispatch.push(spec.dispatch);
                    self.slot_paths.push(spec.path);
                    self.slot_actions.push(spec.action);
                    self.stages.push(CompiledStage::default());
                    let slot = self.slot_intents.len() - 1;
                    self.slot_by_action[id] = encode_slot(slot);
                    slot
                }
                slot => usize::from(slot),
            };
            if !self.delegated_slots.contains(&slot) {
                self.delegated_slots.push(slot);
            }
        }
    }

    pub(crate) fn bindings(&self) -> &[CompiledBinding] {
        &self.bindings
    }

    pub(crate) fn stage(&self, slot: usize) -> &CompiledStage {
        &self.stages[slot]
    }

    pub(crate) fn delegated_slots(&self) -> &[usize] {
        &self.delegated_slots
    }

    pub(crate) fn slot_count(&self) -> usize {
        self.slot_intents.len()
    }

    pub(crate) fn scratch_count(&self) -> usize {
        self.scratch_count
    }

    /// How many groups of bindings share a tunable with each other — see
    /// `CompiledBinding::tunable_shared`.
    pub(crate) fn tunable_scratch_count(&self) -> usize {
        self.tunable_scratch_count
    }

    /// Whether any binding requires a control held alongside its own.
    ///
    /// A plan with none skips the clash pass entirely, which is most plans.
    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    pub(crate) fn has_chords(&self) -> bool {
        self.has_chords
    }

    pub(crate) fn intent_for_slot(&self, slot: usize) -> ActionIntent {
        self.slot_intents[slot]
    }

    pub(crate) fn dispatch_for_slot(&self, slot: usize) -> Dispatch {
        self.slot_dispatch[slot]
    }

    pub(crate) fn slot_for_action(&self, action: ActionId) -> Option<usize> {
        // An id interned after this plan compiled indexes past the end, which means what the
        // sentinel means: not bound here. So the miss needs no separate case.
        match self.slot_by_action.get(action.index() as usize) {
            Some(&UNBOUND) | None => None,
            Some(&slot) => Some(usize::from(slot)),
        }
    }

    /// The declared paths of every action this context binds, in slot order.
    pub(crate) fn bound_paths(&self) -> &[&'static str] {
        &self.slot_paths
    }

    /// The identity of every action this context binds, in slot order.
    pub(crate) fn slot_actions(&self) -> &[ActionId] {
        &self.slot_actions
    }

    /// This context's class bindings, in declaration order — the order they arbitrate in.
    pub(crate) fn class_bindings(&self) -> &[CompiledClassBinding] {
        &self.class_bindings
    }

    /// Whether some plain binding in this context already reads `control`.
    ///
    /// A class binding yields to this unconditionally; see the note on `indexed_controls`.
    pub(crate) fn is_indexed(&self, control: Control) -> bool {
        self.indexed_controls.contains(&control)
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

    // Bound twice to the same control: harmless for a button, doubling for a delta, and a mistake
    // either way. Reported against the second one, once, rather than once per binding in the pair.
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
                control: Control::PhysicalKey(KeyCode::Space)
            }
        );
    }

    // `Custom`'s `rescales()` dispatches through the wrapped modifier's own trait impl rather than
    // a match arm on `BindingModifier`, so a hand-written modifier that rescales has to be counted
    // like a built-in one. A stale blanket impl there fails silently.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_custom_modifier_still_counts_toward_chained_rescaling() {
        use crate::action::{ActionValue, Scratch};
        use crate::binding::Modifier;
        use bevy_input::keyboard::KeyCode;

        struct AlsoRescales;

        impl Modifier for AlsoRescales {
            fn apply(
                &self,
                value: ActionValue,
                _scratch: &mut Scratch,
                _delta: f32,
            ) -> ActionValue {
                value
            }

            fn rescales(&self) -> bool {
                true
            }
        }

        let mut builder = InputContextBuilder::<()>::default();
        builder
            .bind::<Jump>(KeyCode::Space)
            .rescale(0.0, 1.0)
            .custom(AlsoRescales);

        let found = builder.diagnostics();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, DiagnosticKind::ChainedRescaling { count: 2 });
    }

    // Two bindings on one control where only one consumes it. Whether a lower-priority context
    // ever sees that control then depends on which of the two fired, which is not a thing anyone
    // can reason about from the declaration.
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
                control: Control::PhysicalKey(KeyCode::Escape),
                other: "plan_tests.menu",
            }
        );
    }

    // Bindings that touch different controls are none of each other's business, however their
    // consume flags read.
    #[cfg(feature = "keyboard")]
    #[test]
    fn consuming_a_control_nobody_else_reads_is_fine() {
        use bevy_input::keyboard::KeyCode;

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<MenuToggle>(KeyCode::Escape).consume();
        builder.bind::<Jump>(KeyCode::Space);

        assert_eq!(builder.diagnostics(), &[]);
    }

    // Two different actions sharing one tunable name in one family: the tunable half of
    // `DuplicateMappingKey`.
    #[cfg(feature = "keyboard")]
    #[test]
    fn two_actions_cannot_share_a_tunable_key() {
        use bevy_input::keyboard::KeyCode;

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<Jump>(KeyCode::Space);
        builder.hold_or_toggle::<Jump>("plan_tests.shared_toggle");
        builder.bind::<MenuToggle>(KeyCode::Escape);
        builder.hold_or_toggle::<MenuToggle>("plan_tests.shared_toggle");

        let found = builder.diagnostics();
        assert!(
            found.iter().any(|d| matches!(
                &d.kind,
                DiagnosticKind::DuplicateTunableKey { key } if *key == "plan_tests.shared_toggle"
            )),
            "{found:?}"
        );
    }

    // A deadzone declared at or beyond full deflection reads centered for every ordinary input, so
    // it is reported even though it compiles and runs.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_dead_zone_at_full_deflection_is_reported() {
        use bevy_input::keyboard::KeyCode;

        use crate::binding::{AxisButtons, DeadZone};

        let mut builder = InputContextBuilder::<()>::default();
        builder
            .bind::<Jump>(AxisButtons::ad())
            .dead_zone(DeadZone::radial(1.0));

        let found = builder.diagnostics();
        assert!(
            found.iter().any(|d| matches!(
                d.kind,
                DiagnosticKind::DeadZoneAtFullDeflection { lower } if lower == 1.0
            )),
            "{found:?}"
        );

        let mut builder = InputContextBuilder::<()>::default();
        builder
            .bind::<Jump>(KeyCode::KeyA)
            .dead_zone(DeadZone::radial(0.1));
        assert!(
            !builder
                .diagnostics()
                .iter()
                .any(|d| matches!(d.kind, DiagnosticKind::DeadZoneAtFullDeflection { .. }))
        );
    }

    // Two bindings of *one* action may share a tunable key on purpose — that is what
    // `hold_or_toggle` reaching a primary and a secondary control declares — but only if they
    // agree about what the tunable is. A range on one side and a switch on the other has nothing
    // coherent to share.
    #[cfg(feature = "keyboard")]
    #[test]
    fn sharing_a_tunable_key_with_a_different_shape_is_reported() {
        use bevy_input::keyboard::KeyCode;

        use crate::binding::{AxisButtons, DeadZone};

        let mut builder = InputContextBuilder::<()>::default();
        builder
            .bind::<Jump>(AxisButtons::ad())
            .dead_zone(DeadZone::radial(0.1))
            .tunable_dead_zone("plan_tests.shared", 0.0..=0.5);
        // A second, unrelated binding for `hold_or_toggle` to find — the composite above is not
        // eligible for it (nothing analog to lose is the wrong question for a two-key axis; there
        // is no single press to toggle either).
        builder.bind::<Jump>(KeyCode::Space);
        builder.hold_or_toggle::<Jump>("plan_tests.shared");

        let found = builder.diagnostics();
        assert!(
            found.iter().any(|d| matches!(
                &d.kind,
                DiagnosticKind::TunableShapeDisagreement { key } if *key == "plan_tests.shared"
            )),
            "{found:?}"
        );
    }

    // Three mistakes cost one run to find, not three.
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

    struct CharacterInput;

    impl crate::event::ClassBinding for CharacterInput {
        const PATH: &'static str = "plan_tests.character_input";
    }

    struct AnyKey;

    impl crate::event::ClassBinding for AnyKey {
        const PATH: &'static str = "plan_tests.any_key";
    }

    // A control a plain binding already names is never handed to the class list — computed once at
    // compile time, not re-derived per event.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_plainly_bound_control_is_indexed() {
        use bevy_input::keyboard::KeyCode;

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<Jump>(KeyCode::Space);
        let (bindings, class_bindings, _) = builder.finish();
        let plan = Plan::<()>::from_bindings(bindings, class_bindings);

        assert!(plan.is_indexed(Control::PhysicalKey(KeyCode::Space)));
        assert!(!plan.is_indexed(Control::PhysicalKey(KeyCode::KeyA)));
    }

    // Two class bindings watching the same filter: the second can never fire, and that should be
    // caught rather than discovered by a player.
    #[cfg(feature = "keyboard")]
    #[test]
    fn two_class_bindings_on_the_same_filter_is_reported() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind_characters::<CharacterInput>();
        builder.bind_characters::<AnyKey>();

        let found = builder.diagnostics();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].severity(), Severity::Warning);
        assert_eq!(
            found[0].kind,
            DiagnosticKind::DuplicateClassBinding { class: None }
        );
    }

    // Different filters overlapping is deliberate, so it is not reported.
    #[cfg(feature = "keyboard")]
    #[test]
    fn two_different_filters_is_fine_even_though_they_overlap() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind_characters::<CharacterInput>();
        builder.bind_class::<AnyKey>(crate::capture::ControlClass::AnyButton);

        assert_eq!(builder.diagnostics(), &[]);
    }

    // A clamp that never runs is the mistake worth hearing about, whether the action was never
    // bound here or was handed to an authority instead.
    #[test]
    fn combining_an_action_with_no_bindings_is_refused() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.combined::<Move>().clamp_magnitude();
        builder.delegate::<Jump>();
        builder.combined::<Jump>().press();

        let found = builder.diagnostics();
        assert_eq!(found.len(), 2, "{found:?}");
        for diagnostic in &found {
            assert_eq!(diagnostic.kind, DiagnosticKind::CombinedWithoutBindings);
            assert_eq!(diagnostic.severity(), Severity::Error);
        }
    }

    // The stage runs after every binding's chain, so a rescale there stacks on one a binding
    // already did. Neither declaration is wrong on its own.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_stage_rescaling_after_a_binding_that_did_is_refused() {
        use crate::action::{ActionValue, Scratch};
        use crate::binding::{DeadZone, DirectionalButtons, Modifier};

        struct Rescales;
        impl Modifier for Rescales {
            fn apply(&self, value: ActionValue, _: &mut Scratch, _: f32) -> ActionValue {
                value
            }

            fn rescales(&self) -> bool {
                true
            }
        }

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<Move>(DirectionalButtons::wasd());
        builder.combined::<Move>().custom(Rescales);
        assert_eq!(builder.diagnostics(), &[], "nothing upstream rescales");

        builder
            .bind::<Move>(DirectionalButtons::arrow_keys())
            .dead_zone(DeadZone::radial(0.1));
        let found = builder.diagnostics();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, DiagnosticKind::ChainedRescaling { count: 2 });
    }

    // Declaring it again adds to it, as binding an action twice does, and the stage's working
    // memory sits after every binding's.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_stage_accumulates_and_its_scratch_follows_the_bindings() {
        use crate::binding::DirectionalButtons;

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<Move>(DirectionalButtons::wasd());
        builder.combined::<Move>().clamp_magnitude();
        builder
            .bind::<Move>(DirectionalButtons::arrow_keys())
            .press();
        builder.combined::<Move>().on_change();
        let combined = builder.take_combined();
        let (bindings, class_bindings, _) = builder.finish();
        let mut plan = Plan::<()>::from_bindings(bindings, class_bindings);
        plan.combine(combined);

        let stage = plan.stage(
            plan.slot_for_action(<Move as crate::action::InputAction>::id())
                .unwrap(),
        );
        assert_eq!((stage.modifiers.len(), stage.conditions.len()), (1, 1));
        // The arrows' four conditions. A direction has no press to remember, so neither composite
        // needs a slot for one.
        assert_eq!(stage.scratch_base, 4);
        assert_eq!(plan.scratch_count(), 6);
    }

    // A player emptying a binding shrinks the bindings' scratch, and the stage has to move down
    // with it rather than keep an offset past the end.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_variant_places_the_stage_after_its_own_bindings() {
        use crate::binding::DirectionalButtons;

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<Move>(DirectionalButtons::wasd());
        builder
            .bind::<Move>(DirectionalButtons::arrow_keys())
            .press();
        builder.combined::<Move>().on_change();
        let combined = builder.take_combined();
        let (bindings, class_bindings, _) = builder.finish();
        let mut template = Plan::<()>::from_bindings(bindings.clone(), class_bindings);
        template.combine(combined);

        // WASD emptied, and one arrow left with its condition.
        let variant = Plan::variant_of(&template, bindings[4..5].to_vec());
        let slot = variant
            .slot_for_action(<Move as crate::action::InputAction>::id())
            .unwrap();
        assert_eq!(variant.stage(slot).conditions.len(), 1, "carried over");
        assert_eq!(variant.stage(slot).scratch_base, 1);
        assert_eq!(variant.scratch_count(), 2);
    }
}
