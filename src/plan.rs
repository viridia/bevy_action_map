//! Compiling bindings into the plan the evaluator runs against.
//!
//! A plan is the immutable compiled form of authored bindings.

use alloc::{collections::BTreeMap, vec::Vec};
use core::marker::PhantomData;

use crate::action::{ActionId, ChannelShape, Intent};
use crate::binding::{BindingModifier, BindingSource, BindingSpec, ClassBindingSpec, Control};
use crate::capture::ControlClass;
use crate::condition::BindingCondition;
use crate::event::{ClassDispatch, Dispatch};

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
            | DiagnosticKind::RebindingDisagreement { .. }
            | DiagnosticKind::ReservedAndMappable
            | DiagnosticKind::FollowsNothing { .. }
            | DiagnosticKind::FollowsUnlisted { .. }
            | DiagnosticKind::DuplicateTunableKey { .. }
            | DiagnosticKind::TunableShapeDisagreement { .. } => Severity::Error,
            DiagnosticKind::DuplicateBinding { .. }
            | DiagnosticKind::ConsumeDisagreement { .. }
            | DiagnosticKind::DuplicateClassBinding { .. } => Severity::Warning,
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
        key: crate::mapping::MappingKey,
    },
    /// Two bindings feeding one mapping disagree about whether the player may change it.
    RebindingDisagreement {
        /// The name they share.
        key: crate::mapping::MappingKey,
    },
    /// A mappable binding reads controls from more than one kind of device.
    MixedSchemeMapping,
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
        /// The class they share.
        class: ControlClass,
    },
    /// Two different actions declare a tunable under the same name in the same scheme.
    DuplicateTunableKey {
        /// The name they share.
        key: &'static str,
    },
    /// Two bindings sharing a tunable disagree about its shape.
    TunableShapeDisagreement {
        /// The name they share.
        key: &'static str,
    },
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
            DiagnosticKind::DuplicateClassBinding { class } => write!(
                f,
                "`{}` binds the class {class:?}, and so does something else in this context. The \
                 first one declared claims every matching control; the second can never fire",
                self.action
            ),
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
    // than compared pairwise like the checks below.
    //
    // Keyed by scheme as well as by name, and remembering *which action* claimed each, because
    // neither kind of repeat is a mistake on its own. Uniqueness is per scheme (R19.15), so one
    // action on a key and on a pad button is two rows in two tables. And one action reaching a name
    // twice within one scheme is a primary and a secondary, which merge into a single row holding
    // both. What is left — two *different* actions answering to one name — is the case where a
    // saved rebinding of one would land on the other, and is what R19.15 wants reported.
    let mut keys = alloc::collections::BTreeMap::new();
    // A tunable's key is unique per scheme for the same reason a mapping's is: two different
    // actions sharing one name is a saved change to one landing on the other. Two bindings of the
    // *same* action sharing one name is deliberate — `hold_or_toggle` declares exactly that, so
    // every eligible binding shares one runtime latch — provided they agree about the tunable's
    // shape, which is the one thing sharing a name cannot paper over.
    let mut tunable_keys: alloc::collections::BTreeMap<
        (crate::mapping::Scheme, &'static str),
        (ActionId, crate::mapping::TunableValue),
    > = alloc::collections::BTreeMap::new();

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

        // Reserving contradicts *rebindability*, not listing: a reserved control is one nothing may
        // be bound over, and showing the player which control that is helps rather than hurts.
        if binding.reserved
            && binding
                .mapping
                .is_some_and(|decl| decl.rebinding.is_rebindable())
        {
            found.push(at(DiagnosticKind::ReservedAndMappable));
        }

        // Resolution is by the controls the two bindings read, so the two ways it fails are "no
        // binding of that action reads this" and "one does, and has no mapping to lend". The second
        // is worth its own diagnostic because the fix is on the *other* binding.
        if let Some(follows) = binding.follows
            && crate::binding::leader_of(bindings, index).is_none()
        {
            let reads_the_same = bindings.iter().enumerate().any(|(other, spec)| {
                other != index && spec.action == follows.action && spec.source == binding.source
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
            let rebindable = declaration.rebinding.is_rebindable();
            let mut scheme = None;
            let mut mixed = false;
            binding.source.for_each_part(|part, control| {
                let key = crate::mapping::MappingKey::new(prefix, part);
                let (claimant, claimed_as) = keys
                    .entry((control.scheme(), key))
                    .or_insert((binding.action, declaration.rebinding));
                if *claimant != binding.action {
                    // Only where something is rebindable, because the hazard is a *saved* rebind of
                    // one row landing on another and a fixed row is never saved. Two fixed rows
                    // under one name are a display oddity; erroring on them would fail the build of
                    // games that want nothing to do with rebinding at all (R19.13).
                    if rebindable || claimed_as.is_rebindable() {
                        found.push(at(DiagnosticKind::DuplicateMappingKey { key }));
                    }
                } else if *claimed_as != declaration.rebinding {
                    found.push(at(DiagnosticKind::RebindingDisagreement { key }));
                }
                match scheme {
                    Some(seen) if seen != control.scheme() => mixed = true,
                    Some(_) => {}
                    None => scheme = Some(control.scheme()),
                }
            });
            // A mapping the player cannot change needs no one scheme to change it *in*; it is a row
            // in whichever table its first control belongs to, which is odd but harmless.
            if mixed && rebindable {
                found.push(at(DiagnosticKind::MixedSchemeMapping));
            }
        }

        if let Some(decl) = &binding.tunable
            && let Some(scheme) = crate::binding::binding_scheme(&binding.source)
        {
            match tunable_keys.entry((scheme, decl.key)) {
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

/// Everything wrong with a set of authored class bindings.
///
/// One check, deliberately: two class bindings in one context declaring the same
/// [`ControlClass`] mean the second can never fire, since arbitration between class bindings is
/// declaration order with no per-tick contest to decide it otherwise. Two *different* classes that
/// happen to overlap — `AnyButton` and `CharacterProducing` both match a character key — are not
/// reported; declaring both, in a chosen order, is how an app says "claim character keys first,
/// then everything else," the same tiebreak plain bindings already use.
pub(crate) fn diagnose_classes(bindings: &[ClassBindingSpec]) -> Vec<BindingDiagnostic> {
    let mut found = Vec::new();
    for (index, binding) in bindings.iter().enumerate() {
        if bindings[..index]
            .iter()
            .any(|earlier| earlier.class == binding.class)
        {
            found.push(BindingDiagnostic {
                action: binding.action_path,
                kind: DiagnosticKind::DuplicateClassBinding {
                    class: binding.class,
                },
            });
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
    // Set when this binding's tunable is shared with at least one other binding — `hold_or_toggle`
    // reaching a primary and a secondary key, most often — to the index of the plan's shared cell
    // for the group. `None` is the ordinary case: the modifier still has the private slot
    // `scratch_base` already gives it, and a binding with no sharing partner runs its own chain
    // exactly as before. A binding with `Some` skips its own chain entirely instead of running it
    // against a cell other bindings also write — see `resolve_shared_toggle`'s doc for why running
    // it per binding is unsafe rather than merely redundant.
    pub(crate) tunable_shared: Option<usize>,
}

impl CompiledBinding {
    pub(crate) fn scratch_len(&self) -> usize {
        // Modifiers, then conditions, then one more for the press this binding derived — which is
        // hysteretic, so it has to remember what it decided last tick.
        self.modifiers.len() + self.conditions.len() + 1
    }
}

/// An authored class binding, resolved to nothing but itself — there is no slot, because there is
/// nothing to hold between ticks.
///
/// No `action_path` here, unlike `BindingSpec`/`CompiledBinding`: the one diagnostic that needs to
/// name a class binding's action runs on the authored `ClassBindingSpec` list before compilation,
/// and evaluation never has to name one back to a person.
#[derive(Clone)]
pub(crate) struct CompiledClassBinding {
    pub(crate) class: ControlClass,
    pub(crate) consume: bool,
    pub(crate) dispatch: ClassDispatch,
}

/// What `Plan::slot_by_action` holds for an action this context does not bind, and therefore also
/// the ceiling on slots in one context — 65535 actions, which no plan approaches.
const UNBOUND: u16 = u16::MAX;

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
    // The reverse direction, as a direct index rather than a search: `ActionId` is dense, so the
    // id is the subscript and `UNBOUND` means this context does not bind it. Sized by the largest
    // id the context binds rather than by the registry, and held once per plan rather than per
    // instance, so the slack costs two bytes an id in one allocation.
    slot_by_action: Vec<u16>,
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
    // Design §4.1's second structure: consulted only when `indexed_controls` doesn't already claim
    // the control an event arrived on.
    class_bindings: Vec<CompiledClassBinding>,
    // Every control any binding above reads, deduped. Not an arbitration index — a class binding
    // never competes for a control on specificity, it simply yields whenever this set claims one.
    indexed_controls: Vec<Control>,
    _marker: PhantomData<C>,
}

impl<C> Plan<C> {
    /// Compiles a plan from authored bindings.
    ///
    // Compilation asks nothing about whether the bindings make sense: `diagnose` owns that, and
    // `add_context` runs it first and refuses the context rather than compiling a plan that cannot
    // work. Keeping the two apart is what lets a rebinding UI ask about bindings it has no
    // intention of installing.
    pub(crate) fn from_bindings(
        bindings: Vec<BindingSpec>,
        class_bindings: Vec<ClassBindingSpec>,
    ) -> Self {
        let mut plan = Self::compile(bindings, None);
        plan.class_bindings = class_bindings
            .into_iter()
            .map(|spec| CompiledClassBinding {
                class: spec.class,
                consume: spec.consume,
                dispatch: spec.dispatch,
            })
            .collect();
        plan
    }

    /// Compiles a variant of `template` — the same actions, driven by different controls.
    ///
    /// What an override applies as. The slot allocation is `template`'s rather than derived afresh,
    /// and both consequences are wanted:
    ///
    /// - an action whose every binding the player unbound **keeps its slot**, so reading it gives a
    ///   rest value rather than the "not bound in this context" warning, which is a typo diagnostic
    ///   and not what happened here;
    /// - slot indices stay put across the swap, so an instance's action states and require-reset
    ///   flags stay aligned with no rebuilding.
    ///
    /// A binding for an action the template does not have would still get a slot of its own, which
    /// cannot happen: a variant only rewrites the sources of bindings the template already holds.
    ///
    /// Class bindings are not part of the diff — they are never rebindable, so they carry over from
    /// `template` unchanged rather than being rebuilt from a list that would just be a copy of them.
    pub(crate) fn variant_of(template: &Self, bindings: Vec<BindingSpec>) -> Self {
        let mut plan = Self::compile(bindings, Some(template));
        plan.class_bindings.clone_from(&template.class_bindings);
        plan
    }

    fn compile(bindings: Vec<BindingSpec>, template: Option<&Self>) -> Self {
        let mut slot_intents: Vec<Intent> = Vec::new();
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

        // Bindings sharing a `Bool`-shaped tunable key, within one scheme, get one shared scratch
        // cell instead of each keeping its own — see `CompiledBinding::tunable_shared`. Computed up
        // front, against every binding at once, since a group is only a group once every member is
        // known; a `Range` tunable never joins one, because `DeadZone`'s modifier holds no runtime
        // state to share in the first place.
        let mut tunable_groups: BTreeMap<(crate::mapping::Scheme, &'static str), Vec<usize>> =
            BTreeMap::new();
        for (index, binding) in bindings.iter().enumerate() {
            let Some(decl) = &binding.tunable else {
                continue;
            };
            if !matches!(decl.default, crate::mapping::TunableValue::Bool(_)) {
                continue;
            }
            let Some(scheme) = crate::binding::binding_scheme(&binding.source) else {
                continue;
            };
            tunable_groups
                .entry((scheme, decl.key))
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
                    // The sentinel is not a slot, so the ceiling is one below it rather than
                    // `u16::MAX` — storing slot 65535 would encode as `UNBOUND` and read back as
                    // an action this context does not bind. App-build, not runtime (R24.4): a
                    // context this size is a mistake in the declaration, and there is no shipped
                    // build in which it happens.
                    slot_by_action[id] = u16::try_from(slot)
                        .ok()
                        .filter(|&slot| slot != UNBOUND)
                        .unwrap_or_else(|| panic!("a context may bind at most {UNBOUND} actions"));
                    slot
                }
                slot => usize::from(slot),
            };

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
                tunable_shared: tunable_shared.get(&index).copied(),
            });
        }

        // A stable sort is what makes declaration order the tiebreak between two contributions of
        // equal strength.
        compiled.sort_by_key(|binding| binding.slot);

        let has_chords = compiled.iter().any(|binding| binding.chord_len > 1);

        // Recomputed on every compile, including a variant's: an override rewrites which controls
        // these bindings read, so a rebind has to move a control between "indexed" and "not" along
        // with everything else — unlike `class_bindings`, which is never part of that diff.
        let mut indexed_controls: Vec<Control> = Vec::new();
        for binding in &compiled {
            binding.source.for_each_control(|control| {
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
            scratch_count,
            tunable_scratch_count,
            class_bindings: Vec::new(),
            indexed_controls,
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

    pub(crate) fn intent_for_slot(&self, slot: usize) -> Intent {
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

    /// Two different actions sharing one tunable name in one scheme is the tunable half of
    /// `DuplicateMappingKey`: a saved change to one would land on the other.
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

    /// Two bindings of *one* action may share a tunable key on purpose — that is what
    /// `hold_or_toggle` reaching a primary and a secondary control declares — but only if they
    /// agree about what the tunable is. A range on one side and a switch on the other has nothing
    /// coherent to share.
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

    struct CharacterInput;

    impl crate::event::ClassBinding for CharacterInput {
        const PATH: &'static str = "plan_tests.character_input";
    }

    struct AnyKey;

    impl crate::event::ClassBinding for AnyKey {
        const PATH: &'static str = "plan_tests.any_key";
    }

    /// A control a plain binding already names is never handed to the class list — computed once at
    /// compile time, not re-derived per event.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_plainly_bound_control_is_indexed() {
        use bevy_input::keyboard::KeyCode;

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<Jump>(KeyCode::Space);
        let (bindings, class_bindings) = builder.finish();
        let plan = Plan::<()>::from_bindings(bindings, class_bindings);

        assert!(plan.is_indexed(Control::Key(KeyCode::Space)));
        assert!(!plan.is_indexed(Control::Key(KeyCode::KeyA)));
    }

    /// Two class bindings watching the same class: the second can never fire, and that should be
    /// caught rather than discovered by a player.
    #[cfg(feature = "keyboard")]
    #[test]
    fn two_class_bindings_on_the_same_class_is_reported() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind_class::<CharacterInput>(crate::capture::ControlClass::CharacterProducing);
        builder.bind_class::<AnyKey>(crate::capture::ControlClass::CharacterProducing);

        let found = builder.diagnostics();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].severity(), Severity::Warning);
        assert_eq!(
            found[0].kind,
            DiagnosticKind::DuplicateClassBinding {
                class: crate::capture::ControlClass::CharacterProducing
            }
        );
    }

    /// Different classes overlapping is not a mistake — it's how an app says "claim these
    /// specifically, then everything else" — so nothing is reported.
    #[cfg(feature = "keyboard")]
    #[test]
    fn two_different_classes_is_fine_even_though_they_overlap() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind_class::<CharacterInput>(crate::capture::ControlClass::CharacterProducing);
        builder.bind_class::<AnyKey>(crate::capture::ControlClass::AnyButton);

        assert_eq!(builder.diagnostics(), &[]);
    }
}
