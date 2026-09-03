//! What the player changed, and putting it back into a running game.
//!
//! Everything else in this crate describes what a game *declared*. This describes what a player did
//! to it afterwards — a set of rows saying "move forward is `E` now" — and the one call that makes a
//! running game agree with it.
//!
//! ```ignore
//! let mut overrides = Overrides::new();
//! overrides.bind(Scheme::KeyboardMouse, forward.key, [Control::Key(KeyCode::KeyE)]);
//!
//! // Every context, every instance, effective immediately.
//! let problems = apply_overrides(world, &overrides);
//! ```
//!
//! # It is a diff, not a snapshot
//!
//! A row that is absent means "whatever the game shipped", so revising a default binding in a patch
//! reaches every player who never touched that row. That only works if the declared bindings survive
//! being overridden, and they do: applying compiles a *variant* of the declared plan and leaves the
//! declaration where it was. [`mappings`](crate::mapping::mappings) then answers what is bound now
//! and [`declared_mappings`](crate::mapping::declared_mappings) answers what the game shipped, which
//! is what a "reset to default" button compares against.
//!
//! Because absence already means the default, clearing a binding needs a value of its own — see
//! [`Override`], which has three.
//!
//! # Where it lives is yours
//!
//! [`Overrides`] is a plain value, not a resource. Put it in your own settings resource, hand it to
//! a settings screen as a working copy, send it to an account service, write it to a file. The crate
//! defines the structure and applies it, and has no opinion about the rest.
//!
//! # Applying never fails
//!
//! A saved override set outlives the build that wrote it, so it can name a mapping this build no
//! longer has or a control that no longer fits. Those rows are skipped and **reported** rather than
//! dropped in silence — [`apply_overrides`] hands back an [`OverrideProblem`] per row it could not
//! use, and applies everything else.

use alloc::collections::BTreeMap;
#[cfg(feature = "serialize")]
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;

use crate::action::ChannelShape;
use crate::binding::{BindingSpec, Control, MappedPart, apply_tunable_value, mapped_parts};
use crate::capture::{ControlClass, RefusedReason, admissible};
use crate::mapping::{Capacity, Mapping, MappingKey, Scheme, Tunable, TunableValue};

/// What a player did to one mapping.
///
/// Three states rather than two, because a diff against defaults makes *absence* meaningful: once a
/// missing row already says "use the default", a player who deliberately emptied a row has nothing
/// left to say with unless emptying has a value of its own.
#[derive(Clone, Debug, PartialEq)]
pub enum Override {
    /// The controls the player put in the mapping, in slot order.
    ///
    /// Position is which slot, so this is written and read in order: the first is the primary. It
    /// replaces the mapping's whole list rather than one position in it — a screen that edits a
    /// single cell edits the list and then writes the row.
    Controls(Vec<Control>),
    /// The player deliberately emptied the mapping.
    ///
    /// The action stays declared and stays readable; nothing fires it. Distinct from a missing row,
    /// which means the game's own default still applies.
    Cleared,
    /// Something outside this crate owns this mapping.
    ///
    /// A backend authoritative for the action owns its bindings and its own rebinding UI, so this
    /// crate neither applies a control here nor treats the row as one the player emptied.
    NotOurs,
}

/// Everything a player has changed, as a diff against what the game declared.
///
/// Rows are keyed by mapping and by scheme, because a mapping name is unique within a scheme and a
/// keyboard remap must not disturb the gamepad layout. Nothing here names a device: what a player
/// bound is a control on a device *class*, and which physical unit drives which player is a separate
/// question with a separate answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    rows: BTreeMap<(Scheme, MappingKey), Override>,
    // Keyed by the tunable's own declared key, resolved to the `&'static str` a running game holds
    // rather than kept as the owned `String` a save file loads — the same reason `rows` is keyed by
    // `MappingKey` rather than by name. Unlike a mapping row, a tunable has no "cleared" state: there
    // is nothing between "this value" and "no row, use the default", so a bare value is enough.
    tunables: BTreeMap<(Scheme, &'static str), TunableValue>,
}

impl Overrides {
    /// An empty set, which is a game running on exactly what it declared.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the player has changed nothing.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.tunables.is_empty()
    }

    /// Puts controls in a mapping.
    ///
    /// The whole list, in slot order. An empty list is [`Override::Cleared`] and is stored as such,
    /// since a row holding nothing and a row that is not there mean different things.
    pub fn bind(
        &mut self,
        scheme: Scheme,
        mapping: MappingKey,
        controls: impl IntoIterator<Item = Control>,
    ) {
        let controls: Vec<Control> = controls.into_iter().collect();
        self.set(
            scheme,
            mapping,
            if controls.is_empty() {
                Override::Cleared
            } else {
                Override::Controls(controls)
            },
        );
    }

    /// Sets a row directly, for the two states [`bind`](Self::bind) cannot express.
    pub fn set(&mut self, scheme: Scheme, mapping: MappingKey, value: Override) {
        self.rows.insert((scheme, mapping), value);
    }

    /// What the player did to one mapping, or `None` where they left it alone.
    pub fn get(&self, scheme: Scheme, mapping: MappingKey) -> Option<&Override> {
        self.rows.get(&(scheme, mapping))
    }

    /// Every row, in a stable order.
    pub fn iter(&self) -> impl Iterator<Item = (Scheme, MappingKey, &Override)> {
        self.rows
            .iter()
            .map(|(&(scheme, key), value)| (scheme, key, value))
    }

    /// Sets a tunable to `value`.
    pub fn tune(&mut self, scheme: Scheme, key: &'static str, value: TunableValue) {
        self.tunables.insert((scheme, key), value);
    }

    /// What the player set one tunable to, or `None` where they left it alone.
    pub fn get_tunable(&self, scheme: Scheme, key: &'static str) -> Option<TunableValue> {
        self.tunables.get(&(scheme, key)).copied()
    }

    /// Every tunable row, in a stable order.
    pub fn iter_tunables(&self) -> impl Iterator<Item = (Scheme, &'static str, TunableValue)> {
        self.tunables
            .iter()
            .map(|(&(scheme, key), &value)| (scheme, key, value))
    }

    /// Puts one tunable back to what the game declared.
    pub fn reset_tunable(&mut self, scheme: Scheme, key: &'static str) {
        self.tunables.remove(&(scheme, key));
    }

    /// Puts one mapping back to what the game declared.
    ///
    /// Removing the row *is* the reset, which is the whole benefit of storing a diff.
    pub fn reset(&mut self, scheme: Scheme, mapping: MappingKey) {
        self.rows.remove(&(scheme, mapping));
    }

    /// Puts every mapping of one action back to what the game declared.
    ///
    /// Takes the mapping list because a row is keyed by mapping alone, and which mappings belong to
    /// an action is a fact about the declaration rather than about the diff. An action bound to a
    /// composite has one row per direction, and this resets all of them.
    pub fn reset_action(&mut self, mappings: &[Mapping], action: crate::action::ActionId) {
        self.reset_matching(mappings, |mapping| mapping.action == action);
    }

    /// Puts every mapping declared in one context back to what the game declared.
    ///
    /// `context` is the path the context declared, which is what
    /// [`Mapping::context`](crate::mapping::Mapping::context) carries.
    pub fn reset_context(&mut self, mappings: &[Mapping], context: &str) {
        self.reset_matching(mappings, |mapping| mapping.context == context);
    }

    /// Puts everything back to what the game declared.
    pub fn reset_all(&mut self) {
        self.rows.clear();
        self.tunables.clear();
    }

    fn reset_matching(&mut self, mappings: &[Mapping], keep: impl Fn(&Mapping) -> bool) {
        for mapping in mappings.iter().filter(|mapping| keep(mapping)) {
            self.reset(mapping.scheme, mapping.key);
        }
    }
}

/// A row an override set named that could not be used, and why.
///
/// Reported rather than dropped: a saved set outlives the build that wrote it, and a player whose
/// binding quietly vanished is owed better than silence.
#[derive(Clone, Debug, PartialEq)]
pub struct OverrideProblem {
    /// The scheme the row was filed under.
    pub scheme: Scheme,
    /// The mapping the row named.
    pub mapping: MappingKey,
    /// What was wrong with it.
    pub kind: OverrideProblemKind,
}

/// What was wrong with an override row.
///
/// No longer `Copy` once a loaded control name has to be carried — clone a `kind` you want to hold
/// onto rather than moving it out from behind a reference.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum OverrideProblemKind {
    /// No mapping of that name in that scheme is declared any more.
    ///
    /// What a renamed or removed binding looks like from inside a file written by an older build.
    NoSuchMapping,
    /// The mapping exists and the player may not change it.
    NotRebindable,
    /// A control belongs to the other control scheme.
    ///
    /// A mapping is rebound within its own scheme, so a gamepad button cannot fill a keyboard row.
    WrongScheme {
        /// The control that does not belong.
        control: Control,
    },
    /// A control reports on a channel the mapping's action cannot use.
    WrongShape {
        /// The control that does not fit.
        control: Control,
        /// What the mapping accepts.
        accepts: ChannelShape,
    },
    /// A binding reserved one of the controls, so nothing may be bound over it.
    Reserved {
        /// The reserved control.
        control: Control,
    },
    /// More controls than the mapping has slots for.
    TooManyControls {
        /// How many the mapping holds.
        capacity: Capacity,
        /// How many the row named.
        given: usize,
    },
    /// The row is one direction of a composite, and the game shipped no second composite to put
    /// another control in.
    ///
    /// A movement binding is four keys, and a second "move forward" key is one part of a second set
    /// of four — so a row like this grows only when the whole composite does. Ship the alternative
    /// as a second `mappable` binding of the same action and the player gets a filled second slot on
    /// all four rows at once, which is how a keyboard table with two columns is actually written.
    CompositeCannotGrow,
    /// A saved control name this build does not recognize.
    ///
    /// What a control renamed or removed since the file was written looks like. Distinct from
    /// [`WrongScheme`](Self::WrongScheme) and [`WrongShape`](Self::WrongShape), which both name an
    /// actual [`Control`] — this one has none, because the text a loaded row held did not resolve
    /// to one at all.
    #[cfg(feature = "serialize")]
    UnknownControl {
        /// The text the file held, exactly as saved.
        name: String,
    },
}

/// This crate's own on-disk format version.
///
/// Only one exists so far, so nothing yet reads it beyond requiring it be present. It exists so a
/// migration has somewhere to attach once a second version does.
#[cfg(feature = "serialize")]
const FORMAT_VERSION: u32 = 1;

/// The name a saved file uses for a scheme, stable independent of [`Scheme`]'s own variant names.
#[cfg(feature = "serialize")]
const fn scheme_name(scheme: Scheme) -> &'static str {
    match scheme {
        Scheme::KeyboardMouse => "keyboard_mouse",
        Scheme::Gamepad => "gamepad",
    }
}

#[cfg(feature = "serialize")]
fn scheme_from_name(name: &str) -> Option<Scheme> {
    match name {
        "keyboard_mouse" => Some(Scheme::KeyboardMouse),
        "gamepad" => Some(Scheme::Gamepad),
        _ => None,
    }
}

#[cfg(feature = "serialize")]
impl serde::Serialize for Override {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            // Bare words a person reads as neither a control nor a mistake — every real control
            // name carries a `/` (chunk 37), so the two can never collide with one (R17.7).
            Override::Cleared => serializer.serialize_str("cleared"),
            Override::NotOurs => serializer.serialize_str("external"),
            // A scalar is the same thing as a one-element list, and most rows hold one — a player
            // editing this by hand should not have to type brackets to say so (§10.1).
            Override::Controls(controls) if controls.len() == 1 => {
                serializer.serialize_str(controls[0].name().as_ref())
            }
            Override::Controls(controls) => {
                use serde::ser::SerializeSeq;
                let mut seq = serializer.serialize_seq(Some(controls.len()))?;
                for control in controls {
                    seq.serialize_element(control.name().as_ref())?;
                }
                seq.end()
            }
        }
    }
}

/// The `[bindings.*]` half of the file: one table per scheme, in `Scheme`'s own declared order
/// rather than sorted by name — so `gamepad` never jumps ahead of `keyboard_mouse` merely because
/// "g" sorts before "k".
#[cfg(feature = "serialize")]
struct BindingsTable<'a>(BTreeMap<Scheme, BTreeMap<String, &'a Override>>);

#[cfg(feature = "serialize")]
impl serde::Serialize for BindingsTable<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (scheme, rows) in &self.0 {
            map.serialize_entry(scheme_name(*scheme), rows)?;
        }
        map.end()
    }
}

/// A tunable's value, on the wire: a bare number or a bare bool — never the bounds, which the
/// game's own declaration carries and a save file has no business repeating.
#[cfg(feature = "serialize")]
impl serde::Serialize for TunableValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            TunableValue::Range { value, .. } => serializer.serialize_f32(*value),
            TunableValue::Bool(value) => serializer.serialize_bool(*value),
        }
    }
}

/// The `[tunables.*]` half of the file, shaped exactly like [`BindingsTable`].
#[cfg(feature = "serialize")]
struct TunablesTable(BTreeMap<Scheme, BTreeMap<&'static str, TunableValue>>);

#[cfg(feature = "serialize")]
impl serde::Serialize for TunablesTable {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (scheme, rows) in &self.0 {
            map.serialize_entry(scheme_name(*scheme), rows)?;
        }
        map.end()
    }
}

#[cfg(feature = "serialize")]
impl serde::Serialize for Overrides {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        // Grouped by scheme first — a keyboard remap has to read as visibly separate from the
        // pad's (§10.1) — and within a scheme sorted by the key's own text, so the file reads the
        // same way twice running.
        let mut by_scheme: BTreeMap<Scheme, BTreeMap<String, &Override>> = BTreeMap::new();
        for (scheme, key, value) in self.iter() {
            by_scheme
                .entry(scheme)
                .or_default()
                .insert(key.to_string(), value);
        }
        let mut tunables_by_scheme: BTreeMap<Scheme, BTreeMap<&'static str, TunableValue>> =
            BTreeMap::new();
        for (scheme, key, value) in self.iter_tunables() {
            tunables_by_scheme
                .entry(scheme)
                .or_default()
                .insert(key, value);
        }

        // "version" before either table: TOML requires a table's own key/value pairs to precede
        // any nested table header, so the order these are emitted in is load-bearing, not cosmetic.
        // `tunables` is left out entirely rather than written as an empty header when nothing has
        // been tuned, the same way a game that declares none pays nothing for the mechanism.
        let omit_tunables = tunables_by_scheme.is_empty();
        let mut map = serializer.serialize_map(Some(if omit_tunables { 2 } else { 3 }))?;
        map.serialize_entry("version", &FORMAT_VERSION)?;
        map.serialize_entry("bindings", &BindingsTable(by_scheme))?;
        if !omit_tunables {
            map.serialize_entry("tunables", &TunablesTable(tunables_by_scheme))?;
        }
        map.end()
    }
}

/// A row a saved file named that this build cannot turn into a [`MappingKey`] at all.
///
/// Distinct from [`OverrideProblem`]: every `OverrideProblem` names a mapping this build has, and
/// this one specifically does not. A [`MappingKey`] can only ever be one the game's own
/// [`declared_mappings`](crate::mapping::declared_mappings) already holds — it is derived from
/// `&'static` strings the game compiled in, not manufactured from a loaded one — so a name a save
/// wrote for an action since renamed or removed has nothing to become. It is reported rather than
/// dropped in silence, which is what carrying the raw text here does; a rewritten save simply omits
/// it.
#[cfg(feature = "serialize")]
#[derive(Clone, Debug, PartialEq)]
pub struct UnresolvedMapping {
    /// The scheme table the row was filed under.
    pub scheme: Scheme,
    /// The mapping name exactly as the file spelled it.
    pub name: String,
}

/// A tunable row a saved file named that this build cannot use, either because no declared tunable
/// answers to the name or because the value on file is the wrong shape for it — a bool where the
/// declared tunable wants a number, most likely a save written against an older declaration.
/// Reported rather than dropped in silence, exactly as [`UnresolvedMapping`] is; a rewritten save
/// simply omits it.
#[cfg(feature = "serialize")]
#[derive(Clone, Debug, PartialEq)]
pub struct UnresolvedTunable {
    /// The scheme table the row was filed under.
    pub scheme: Scheme,
    /// The tunable name exactly as the file spelled it.
    pub name: String,
}

#[cfg(feature = "serialize")]
#[derive(serde::Deserialize)]
struct RawOverrides {
    version: u32,
    #[serde(default)]
    bindings: BTreeMap<String, BTreeMap<String, RawOverride>>,
    #[serde(default)]
    tunables: BTreeMap<String, BTreeMap<String, RawTunableValue>>,
}

/// A tunable's value, before it has been matched against a declared tunable's shape.
#[cfg(feature = "serialize")]
enum RawTunableValue {
    Number(f32),
    Bool(bool),
}

#[cfg(feature = "serialize")]
impl<'de> serde::Deserialize<'de> for RawTunableValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ValueVisitor;

        impl<'de> serde::de::Visitor<'de> for ValueVisitor {
            type Value = RawTunableValue;

            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a number or a bool")
            }

            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(RawTunableValue::Bool(v))
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(RawTunableValue::Number(v as f32))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(RawTunableValue::Number(v as f32))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(RawTunableValue::Number(v as f32))
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

/// A row's value, before its mapping name has been resolved against anything.
#[cfg(feature = "serialize")]
enum RawOverride {
    Controls(Vec<String>),
    Cleared,
    NotOurs,
}

#[cfg(feature = "serialize")]
impl<'de> serde::Deserialize<'de> for RawOverride {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RowVisitor;

        impl<'de> serde::de::Visitor<'de> for RowVisitor {
            type Value = RawOverride;

            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a control name, a list of them, \"cleared\", or \"external\"")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(match v {
                    "cleared" => RawOverride::Cleared,
                    "external" => RawOverride::NotOurs,
                    other => RawOverride::Controls(alloc::vec![String::from(other)]),
                })
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut controls = Vec::new();
                while let Some(name) = seq.next_element::<String>()? {
                    controls.push(name);
                }
                Ok(RawOverride::Controls(controls))
            }
        }

        deserializer.deserialize_any(RowVisitor)
    }
}

/// Turns what a file said into what this build can use.
///
/// Each row's mapping name is matched against `declared`, since a `MappingKey` can only ever be one
/// the game already has. A name that matches nothing comes back in the returned `UnresolvedMapping`
/// list rather than being dropped in silence; a control name that does not parse becomes an
/// `OverrideProblem` instead, because by that point the mapping *did* resolve and there is a row to
/// file the problem against.
#[cfg(feature = "serialize")]
fn resolve(
    raw: RawOverrides,
    declared: &[Mapping],
    declared_tunables: &[Tunable],
) -> (
    Overrides,
    Vec<OverrideProblem>,
    Vec<UnresolvedMapping>,
    Vec<UnresolvedTunable>,
) {
    // Read for its presence (R17.3) — there is only one version so far, so nothing yet branches on
    // its value.
    let _version = raw.version;

    let mut overrides = Overrides::new();
    let mut problems = Vec::new();
    let mut unresolved = Vec::new();
    let mut unresolved_tunables = Vec::new();

    for (scheme_text, rows) in raw.bindings {
        // Not one of ours — a foreign or future top-level key. Nothing typed to report this
        // against, so R17.2's tolerance is all this can be: skip the table, keep the rest.
        let Some(scheme) = scheme_from_name(&scheme_text) else {
            continue;
        };
        for (name, row) in rows {
            let Some(mapping) = declared
                .iter()
                .find(|candidate| candidate.scheme == scheme && candidate.key.to_string() == name)
            else {
                unresolved.push(UnresolvedMapping { scheme, name });
                continue;
            };

            let names = match row {
                RawOverride::Cleared => {
                    overrides.set(scheme, mapping.key, Override::Cleared);
                    continue;
                }
                RawOverride::NotOurs => {
                    overrides.set(scheme, mapping.key, Override::NotOurs);
                    continue;
                }
                RawOverride::Controls(names) => names,
            };

            let mut controls = Vec::with_capacity(names.len());
            let mut all_known = true;
            for name in &names {
                match Control::from_name(name) {
                    Some(control) => controls.push(control),
                    None => {
                        all_known = false;
                        problems.push(OverrideProblem {
                            scheme,
                            mapping: mapping.key,
                            kind: OverrideProblemKind::UnknownControl { name: name.clone() },
                        });
                    }
                }
            }
            if all_known {
                // An empty list and `Cleared` mean the same thing (§10.1); `bind` already folds one
                // into the other, so a hand-edited `[]` reads exactly like the dedicated word does.
                overrides.bind(scheme, mapping.key, controls);
            }
        }
    }

    for (scheme_text, rows) in raw.tunables {
        let Some(scheme) = scheme_from_name(&scheme_text) else {
            continue;
        };
        for (name, raw_value) in rows {
            let Some(tunable) = declared_tunables
                .iter()
                .find(|candidate| candidate.scheme == scheme && candidate.key == name)
            else {
                unresolved_tunables.push(UnresolvedTunable { scheme, name });
                continue;
            };

            let value = match (tunable.value, raw_value) {
                (TunableValue::Range { min, max, .. }, RawTunableValue::Number(number)) => {
                    TunableValue::Range {
                        value: number.clamp(min, max),
                        min,
                        max,
                    }
                }
                (TunableValue::Bool(_), RawTunableValue::Bool(value)) => TunableValue::Bool(value),
                // The wrong shape for what this build declares — a bool where a range is wanted,
                // most likely a save written against an older declaration. Reported the same as a
                // name that resolves to nothing, since either way there is nothing usable here.
                _ => {
                    unresolved_tunables.push(UnresolvedTunable { scheme, name });
                    continue;
                }
            };
            overrides.tune(scheme, tunable.key, value);
        }
    }

    (overrides, problems, unresolved, unresolved_tunables)
}

/// Deserializes a saved override set, resolving it against what this build currently declares.
///
/// Needs `declared` because a [`MappingKey`] cannot be manufactured from a loaded string alone —
/// see [`UnresolvedMapping`].
///
/// ```ignore
/// let declared = declared_mappings(world);
/// let declared_tunables = mapping::declared_tunables(world);
/// let mut de = toml::Deserializer::new(&text);
/// let (overrides, problems, unresolved, unresolved_tunables) = OverridesLoader {
///     declared: &declared,
///     declared_tunables: &declared_tunables,
/// }
/// .deserialize(&mut de)?;
/// ```
#[cfg(feature = "serialize")]
pub struct OverridesLoader<'a> {
    /// What the game currently declares —
    /// [`declared_mappings`](crate::mapping::declared_mappings)'s own output, or a subset of it.
    pub declared: &'a [Mapping],
    /// What the game currently declares tunables as —
    /// [`declared_tunables`](crate::mapping::declared_tunables)'s own output, or a subset of it.
    pub declared_tunables: &'a [Tunable],
}

#[cfg(feature = "serialize")]
impl<'de> serde::de::DeserializeSeed<'de> for OverridesLoader<'_> {
    type Value = (
        Overrides,
        Vec<OverrideProblem>,
        Vec<UnresolvedMapping>,
        Vec<UnresolvedTunable>,
    );

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        let raw = <RawOverrides as serde::Deserialize>::deserialize(deserializer)?;
        Ok(resolve(raw, self.declared, self.declared_tunables))
    }
}

/// Makes a running game agree with an override set.
///
/// Every context, every instance, effective on the next tick. This is the only way an override
/// reaches a game, and loading a saved set at startup is simply the first call — there is no
/// separate startup path, because a backend that owns its bindings can rewrite them while the game
/// runs and a startup-only path would be wrong on the platform that needs it most.
///
/// Applying an override to a context cancels whatever it had in flight and makes each of its actions
/// wait to be seen at rest once, exactly as switching the context off and on again does. A player
/// holding the key they just rebound does not get a fresh press out of it.
///
/// Rows this build cannot use come back as [`OverrideProblem`]s; everything else is applied.
pub fn apply_overrides(world: &mut World, overrides: &Overrides) -> Vec<OverrideProblem> {
    apply_with(world, overrides, None)
}

/// Like [`apply_overrides`], but a preset's rows are exempt from the "not rebindable here"
/// refusal — the one way a `Fixed` row (every gamepad binding in a typical game) still moves.
///
/// `overrides` is the whole working copy to apply — a preset's rows and any manual captures
/// together, since applying always starts from the pristine declaration and a second call does
/// not layer onto the first. `preset` is only consulted to decide which of those rows may bypass
/// [`OverrideProblemKind::NotRebindable`]; pass the selected preset's own rows, or an empty
/// [`Overrides`] if none is selected.
pub fn apply_overrides_with_preset(
    world: &mut World,
    overrides: &Overrides,
    preset: &Overrides,
) -> Vec<OverrideProblem> {
    apply_with(world, overrides, Some(preset))
}

/// Like [`apply_overrides`], but reaches only one entity's own instance rather than every one.
///
/// For a game with more than one occupant sharing a context type (two split-screen players on
/// identical pads, say) where each keeps its own independently-persisted [`Overrides`] and wants
/// it to reach only its own entity. The declared baseline and the world-wide default new instances
/// inherit at spawn are both left untouched, so a freshly spawned third instance still gets the
/// unmodified default, and the diff this computes is against the same pristine declaration
/// [`apply_overrides`] diffs against.
///
/// A context type this entity does not carry is silently skipped, the same way a row naming a
/// mapping nothing declares comes back as [`OverrideProblemKind::NoSuchMapping`] rather than
/// panicking.
pub fn apply_overrides_for(
    world: &mut World,
    entity: Entity,
    overrides: &Overrides,
) -> Vec<OverrideProblem> {
    apply_for_entity_with(world, entity, overrides, None)
}

/// Like [`apply_overrides_for`], but a preset's rows are exempt from the "not rebindable here"
/// refusal, on the same terms as [`apply_overrides_with_preset`].
pub fn apply_overrides_for_with_preset(
    world: &mut World,
    entity: Entity,
    overrides: &Overrides,
    preset: &Overrides,
) -> Vec<OverrideProblem> {
    apply_for_entity_with(world, entity, overrides, Some(preset))
}

fn apply_for_entity_with(
    world: &mut World,
    entity: Entity,
    overrides: &Overrides,
    preset: Option<&Overrides>,
) -> Vec<OverrideProblem> {
    let Some(declared) = world.get_resource::<crate::inspect::DeclaredContexts>() else {
        return Vec::new();
    };
    // Collected first because each one takes the world exclusively in turn.
    let appliers: Vec<_> = declared
        .0
        .iter()
        .map(|context| context.apply_for_entity)
        .collect();

    let mut problems = Vec::new();
    for apply in appliers {
        problems.extend(apply(world, entity, overrides, preset));
    }

    // Same diagnostic `apply_with` reports, for the same reason: from inside any one context every
    // other context's rows look exactly as missing as a row that is genuinely gone.
    let declared = crate::mapping::declared_mappings(world);
    problems.extend(
        overrides
            .iter()
            .filter(|&(scheme, key, _)| {
                !declared
                    .iter()
                    .any(|row| row.key == key && row.scheme == scheme)
            })
            .map(|(scheme, mapping, _)| OverrideProblem {
                scheme,
                mapping,
                kind: OverrideProblemKind::NoSuchMapping,
            }),
    );

    // The one entity's own prompts may now name a different control.
    crate::present::PromptGeneration::bump(world);
    problems
}

fn apply_with(
    world: &mut World,
    overrides: &Overrides,
    preset: Option<&Overrides>,
) -> Vec<OverrideProblem> {
    let Some(declared) = world.get_resource::<crate::inspect::DeclaredContexts>() else {
        return Vec::new();
    };
    // Collected first because each one takes the world exclusively in turn.
    let appliers: Vec<_> = declared.0.iter().map(|context| context.apply).collect();

    let mut problems = Vec::new();
    for apply in appliers {
        problems.extend(apply(world, overrides, preset));
    }

    // Reported here rather than per context, because "no context declares this" is the only form
    // the question has an answer in: from inside any one context every other context's rows look
    // exactly as missing as a row that is genuinely gone.
    let declared = crate::mapping::declared_mappings(world);
    problems.extend(
        overrides
            .iter()
            .filter(|&(scheme, key, _)| {
                !declared
                    .iter()
                    .any(|row| row.key == key && row.scheme == scheme)
            })
            .map(|(scheme, mapping, _)| OverrideProblem {
                scheme,
                mapping,
                kind: OverrideProblemKind::NoSuchMapping,
            }),
    );

    // Every prompt on screen may now name a different control, which is the one thing about a
    // rebind that is invisible until someone rebinds with a HUD up.
    crate::present::PromptGeneration::bump(world);
    problems
}

/// The pure half: authored bindings and an override set in, rewritten bindings and rows out.
///
/// Separate from the ECS work so that it can be reasoned about and tested without a `World`.
pub(crate) fn rewrite(
    declared: &[BindingSpec],
    rows: &[Mapping],
    tunables: &[Tunable],
    overrides: &Overrides,
    preset: Option<&Overrides>,
    reserved: &[Control],
    context: &'static str,
) -> (
    Vec<BindingSpec>,
    Vec<Mapping>,
    Vec<Tunable>,
    Vec<OverrideProblem>,
) {
    let mut variant = declared.to_vec();
    let mut problems = Vec::new();
    let mut dropped = alloc::collections::BTreeSet::new();
    let mut grown: Vec<BindingSpec> = Vec::new();

    // Both computed against the *declared* bindings and never re-derived as we go: `leader_of`
    // matches a follower to its leader by the controls the two read, so once a source has been
    // rewritten the two no longer look alike and the link would be lost half way through the pass.
    let parts = mapped_parts(declared);
    let leaders: Vec<Option<usize>> = (0..declared.len())
        .map(|index| crate::binding::leader_of(declared, index))
        .collect();

    for row in rows {
        let Some(over) = overrides.get(row.scheme, row.key) else {
            continue;
        };
        let wanted: &[Control] = match over {
            // The defaults stand, and deliberately are not read as an empty row: nobody cleared
            // this, somebody else owns it.
            Override::NotOurs => continue,
            Override::Cleared => &[],
            Override::Controls(controls) => controls,
        };

        let contributors: Vec<_> = parts
            .iter()
            .filter(|part| {
                part.key == row.key
                    && part.scheme == row.scheme
                    && declared[part.binding].action == row.action
            })
            .collect();

        let preset_authorized =
            preset.is_some_and(|preset| preset.get(row.scheme, row.key).is_some());
        if let Some(kind) = refusal(
            row,
            wanted,
            reserved,
            &contributors,
            declared,
            preset_authorized,
        ) {
            problems.push(OverrideProblem {
                scheme: row.scheme,
                mapping: row.key,
                kind,
            });
            continue;
        }

        for (slot, &control) in wanted.iter().enumerate() {
            match contributors.get(slot) {
                // A slot the defaults already fill: the binding stays where it is and reads
                // something else.
                Some(part) => {
                    variant[part.binding].source.set_part(part.part, control);
                    rewrite_followers(declared, &leaders, &mut variant, part.binding);
                }
                // A slot the game shipped nothing for — the empty secondary of a `mappable_upto(2)`
                // row. The last binding feeding the row is cloned onto the new control, so the
                // secondary behaves like the primary rather than like a bare source with no
                // modifiers or conditions on it.
                None => {
                    let Some(last) = contributors.last() else {
                        continue;
                    };
                    grown.push(clone_onto(&variant[last.binding], last.part, control));
                    for (follower, _) in
                        followers_of(declared, &leaders, last.binding).collect::<Vec<_>>()
                    {
                        grown.push(clone_onto(&variant[follower], last.part, control));
                    }
                }
            }
        }

        // Slots the row no longer has. The binding goes rather than being left reading something
        // stale, and its followers go with it — a rider whose leader is gone has nothing to ride.
        for part in contributors.iter().skip(wanted.len()) {
            dropped.insert(part.binding);
            dropped.extend(followers_of(declared, &leaders, part.binding).map(|(index, _)| index));
        }
    }

    variant.extend(grown);
    let mut variant: Vec<BindingSpec> = variant
        .into_iter()
        .enumerate()
        .filter(|(index, _)| !dropped.contains(index))
        .map(|(_, binding)| binding)
        .collect();

    // Tunables never add or drop a binding — only a field on a modifier already there — so this
    // runs after the control rewrite above rather than interleaved with it.
    for tunable in tunables {
        let Some(value) = overrides.get_tunable(tunable.scheme, tunable.key) else {
            continue;
        };
        for binding in &mut variant {
            let Some(decl) = &binding.tunable else {
                continue;
            };
            // Scheme as well as key: sharing is scoped to one scheme (`hold_or_toggle` reaching a
            // keyboard row shares nothing with a same-named gamepad tunable), and a key match alone
            // would move a keyboard override onto a gamepad binding that only happens to share text.
            if decl.key != tunable.key
                || crate::binding::binding_scheme(&binding.source) != Some(tunable.scheme)
            {
                continue;
            }
            apply_tunable_value(&mut binding.modifiers[decl.modifier_index], value);
        }
    }

    let current = current_rows(&variant, rows, context);
    let current_tunables = crate::binding::tunables_of(&variant, context);
    (variant, current, current_tunables, problems)
}

/// Why this row cannot take these controls, if it cannot.
///
/// The whole row is refused rather than partly applied: half a rebind is worse than none, and the
/// player still has the default. `preset_authorized` is the one exception to the rebindable-only
/// rule below it: a preset moves a `Fixed` row on purpose, which is the whole point of one.
fn refusal(
    row: &Mapping,
    wanted: &[Control],
    reserved: &[Control],
    contributors: &[&MappedPart],
    declared: &[BindingSpec],
    preset_authorized: bool,
) -> Option<OverrideProblemKind> {
    if !row.rebinding.is_rebindable() && !preset_authorized {
        return Some(OverrideProblemKind::NotRebindable);
    }
    // Capacity first: "this row has one slot" is both simpler and truer than anything below it
    // about why a second control has nowhere to go.
    if let Capacity::UpTo(limit) = row.capacity
        && wanted.len() > limit
    {
        return Some(OverrideProblemKind::TooManyControls {
            capacity: row.capacity,
            given: wanted.len(),
        });
    }
    // A slot the defaults left empty is filled by copying the binding beside it, and that only
    // works where the binding reads one control. Copy a *composite* and its other parts land in
    // their own rows a second time — "Move Down: S | S", a wrong screen rather than an untidy one.
    if wanted.len() > contributors.len()
        && let Some(last) = contributors.last()
        && parts_in(&declared[last.binding].source) > 1
    {
        return Some(OverrideProblemKind::CompositeCannotGrow);
    }
    // `None` is a mapping no single control can fill — a stick or a mouse bound whole — which is
    // never something a capture offered, so a row naming one came from somewhere else.
    let accepts = ControlClass::of(row.accepts);
    for &control in wanted {
        // Shared with capture, which is what stops one control getting two different reasons
        // depending on whether it arrived from a press or from a file.
        match admissible(
            control,
            Some(row.scheme),
            accepts,
            reserved.contains(&control),
        ) {
            Ok(()) => {}
            Err(RefusedReason::Scheme) => {
                return Some(OverrideProblemKind::WrongScheme { control });
            }
            Err(RefusedReason::Reserved) => return Some(OverrideProblemKind::Reserved { control }),
            Err(RefusedReason::Shape) => {
                return Some(OverrideProblemKind::WrongShape {
                    control,
                    accepts: row.accepts,
                });
            }
        }
    }
    None
}

/// How many presentation rows one binding feeds: one for a plain control, four for a directional
/// composite.
fn parts_in(source: &crate::binding::BindingSource) -> usize {
    let mut count = 0;
    source.for_each_part(|_, _| count += 1);
    count
}

/// Every binding riding `leader`'s mapping, and the slot of the leader list it was found at.
fn followers_of<'a>(
    declared: &'a [BindingSpec],
    leaders: &'a [Option<usize>],
    leader: usize,
) -> impl Iterator<Item = (usize, &'a BindingSpec)> {
    leaders
        .iter()
        .enumerate()
        .filter(move |&(_, resolved)| *resolved == Some(leader))
        .map(|(index, _)| (index, &declared[index]))
}

/// Moves every rider of `leader` onto the control the leader just took.
///
/// Without it a rebind separates two actions that were declared to share a control: the throttle
/// moves and the afterburner stays on the old key, where whatever the player binds next quietly
/// acquires an afterburner.
fn rewrite_followers(
    declared: &[BindingSpec],
    leaders: &[Option<usize>],
    variant: &mut [BindingSpec],
    leader: usize,
) {
    let riders: Vec<usize> = followers_of(declared, leaders, leader)
        .map(|(index, _)| index)
        .collect();
    // A follower reads exactly what its leader reads — that identity is how the link was resolved
    // in the first place — so keeping it true is an assignment rather than a second rewrite.
    for rider in riders {
        variant[rider].source = variant[leader].source;
    }
}

/// A copy of `binding` reading `control` in place of the control at `part`.
fn clone_onto(binding: &BindingSpec, part: crate::binding::Part, control: Control) -> BindingSpec {
    let mut grown = binding.clone();
    grown.source.set_part(part, control);
    grown
}

/// The presentation rows for a variant, keyed to the declared ones.
///
/// Derived from the rewritten bindings rather than patched, so the rows and the plan cannot disagree
/// about what is bound — with two exceptions the derivation cannot express on its own. A row the
/// player emptied has no bindings left and so derives nothing at all; it has to stay on the screen,
/// holding nothing, or there is nowhere to bind it back. And capacity is raised, never lowered
/// (R19.9): a row rebound down to one control still derives from one binding, so its capacity is
/// widened back against what was declared rather than taken from the derived row as-is, or the
/// second slot a rebind just vacated could never be filled again.
fn current_rows(
    variant: &[BindingSpec],
    declared: &[Mapping],
    context: &'static str,
) -> Vec<Mapping> {
    let derived = crate::binding::mappings_of(variant, context);
    declared
        .iter()
        .map(|row| {
            derived
                .iter()
                .find(|current| {
                    current.key == row.key
                        && current.scheme == row.scheme
                        && current.action == row.action
                })
                .map(|current| Mapping {
                    capacity: crate::binding::widest(current.capacity, row.capacity),
                    ..current.clone()
                })
                .unwrap_or_else(|| Mapping {
                    slots: Vec::new(),
                    followers: row.followers.clone(),
                    ..row.clone()
                })
        })
        .collect()
}

#[cfg(all(test, feature = "keyboard"))]
mod tests {
    use super::*;

    use alloc::string::ToString;
    use bevy_app::App;
    use bevy_ecs::entity::Entity;
    use bevy_input::keyboard::KeyCode;

    use crate::action::{InputAction as _, Phase};
    use crate::binding::DirectionalButtons;
    use crate::context::{ActionMapAppExt, InputContextState};
    use crate::mapping::{Rebinding, declared_mappings, mappings};
    use crate::present::{BindingTable, PromptScope, Prompts as _};
    use crate::{ActionMapPlugin, InputAction, InputContext};

    #[derive(InputAction)]
    #[action(path = "override_tests.move", output = bevy_math::Vec2, intent = Directional2)]
    struct Move;

    #[derive(InputAction)]
    #[action(path = "override_tests.jump", output = bool, intent = Button)]
    struct Jump;

    #[derive(InputAction)]
    #[action(path = "override_tests.lunge", output = bool, intent = Button)]
    struct Lunge;

    #[derive(InputAction)]
    #[action(path = "override_tests.look", output = bevy_math::Vec2, intent = Delta2)]
    struct Look;

    #[derive(InputAction)]
    #[action(path = "override_tests.settings", output = bool, intent = Button)]
    struct OpenSettings;

    #[derive(InputContext)]
    #[context(path = "override_tests.playing", tick = Render)]
    struct Playing;

    /// `Move` on WASD (four rows), `Jump` on Space with room for a secondary and a `Lunge` riding
    /// it, `Look` on the mouse (listed, unchangeable), and a reserved settings key.
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Playing>(|controls| {
            controls.bind::<Move>(DirectionalButtons::wasd()).mappable();
            controls.bind::<Jump>(KeyCode::Space).mappable_upto(2);
            controls.follow::<Lunge, Jump>(|binding| binding.hold(0.4));
            controls.bind::<Look>(crate::binding::MouseMove);
            controls.bind::<OpenSettings>(KeyCode::F1).reserved();
        });
        app
    }

    fn row(app: &App, name: &str) -> Mapping {
        mappings(app.world())
            .into_iter()
            .find(|mapping| mapping.key.to_string() == name)
            .unwrap_or_else(|| panic!("no mapping named {name}"))
    }

    fn slots(app: &App, name: &str) -> Vec<Control> {
        row(app, name).slots
    }

    fn bind(app: &App, name: &str, controls: &[Control]) -> Overrides {
        let target = row(app, name);
        let mut overrides = Overrides::new();
        overrides.bind(target.scheme, target.key, controls.iter().copied());
        overrides
    }

    /// A row the player changed reads back changed.
    #[test]
    fn applying_an_override_moves_a_row() {
        let mut app = app();
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW)]
        );

        let overrides = bind(
            &app,
            "override_tests.move.up",
            &[Control::Key(KeyCode::KeyI)],
        );
        let problems = apply_overrides(app.world_mut(), &overrides);

        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyI)]
        );
        // And only that part of the composite: the other three keys are where they were.
        assert_eq!(
            slots(&app, "override_tests.move.left"),
            [Control::Key(KeyCode::KeyA)]
        );
    }

    /// A diff has to be taken against the defaults, so the defaults have to still be there after
    /// the first apply — otherwise a revised default never again reaches a player who has changed
    /// anything.
    #[test]
    fn the_defaults_survive_being_overridden() {
        let mut app = app();
        let overrides = bind(
            &app,
            "override_tests.move.up",
            &[Control::Key(KeyCode::KeyI)],
        );
        apply_overrides(app.world_mut(), &overrides);

        let declared = declared_mappings(app.world())
            .into_iter()
            .find(|mapping| mapping.key.to_string() == "override_tests.move.up")
            .expect("the row is still declared");
        assert_eq!(declared.slots, [Control::Key(KeyCode::KeyW)], "still W");

        // And applying a second time is a diff against the same defaults, not against the first
        // apply — so going back to a row nobody overrode restores the shipped control.
        apply_overrides(app.world_mut(), &Overrides::new());
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW)]
        );
    }

    /// `Lunge` is `Jump` held; rebinding Jump has to take Lunge with it, or the two actions the
    /// game declared as sharing a control stop sharing one.
    #[test]
    fn a_follower_moves_with_the_row_it_rides() {
        let mut app = app();
        // A prompt is a runtime question, so something has to be carrying the context
        // before it has an answer at all.
        app.world_mut().spawn(Playing);
        let overrides = bind(&app, "override_tests.jump", &[Control::Key(KeyCode::KeyK)]);
        apply_overrides(app.world_mut(), &overrides);

        let jump = row(&app, "override_tests.jump");
        assert_eq!(jump.slots, [Control::Key(KeyCode::KeyK)]);
        // The follower is still on the row rather than orphaned onto a row of its own...
        assert_eq!(jump.followers.len(), 1);
        assert_eq!(jump.followers[0].action, Lunge::id());
        // ...and it is the new control it reads, which is the half that is a gameplay bug when it
        // is missing.
        let fires = BindingTable::new(app.world());
        let prompts = fires.prompts(Lunge::id(), PromptScope::ANY);
        assert_eq!(prompts.len(), 1);
        assert_eq!(
            prompts[0].origin.control(),
            Some(Control::Key(KeyCode::KeyK))
        );
    }

    /// Clearing is not the same as never having touched the row: the action stays declared and
    /// readable, and nothing fires it.
    #[test]
    fn a_cleared_row_leaves_the_action_bound_but_silent() {
        let mut app = app();
        let target = row(&app, "override_tests.jump");
        let mut overrides = Overrides::new();
        overrides.set(target.scheme, target.key, Override::Cleared);
        apply_overrides(app.world_mut(), &overrides);

        // The row is still on the screen, holding nothing — or there would be nowhere to bind it
        // back from.
        let jump = row(&app, "override_tests.jump");
        assert!(jump.slots.is_empty());
        assert_eq!(jump.rebinding, Rebinding::Here);

        // And the action still has a slot, so reading it is a rest value rather than the "not bound
        // in this context" warning, which is a typo diagnostic and not what happened.
        let entity = app.world_mut().spawn(Playing).id();
        let state = app
            .world()
            .get::<InputContextState<Playing>>(entity)
            .unwrap();
        assert!(
            state.is_bound::<Jump>(),
            "unbound is not the same as cleared"
        );
        assert!(!state.value::<Jump>());
    }

    /// A row the game shipped one default for and left room in. The new binding is a copy of the one
    /// beside it, so a secondary behaves like the primary rather than like a bare control with the
    /// conditions stripped off it.
    #[test]
    fn a_grown_slot_copies_the_binding_beside_it() {
        let mut app = app();
        // A prompt is a runtime question, so something has to be carrying the context
        // before it has an answer at all.
        app.world_mut().spawn(Playing);
        let overrides = bind(
            &app,
            "override_tests.jump",
            &[Control::Key(KeyCode::Space), Control::Key(KeyCode::KeyK)],
        );
        apply_overrides(app.world_mut(), &overrides);

        assert_eq!(
            slots(&app, "override_tests.jump"),
            [Control::Key(KeyCode::Space), Control::Key(KeyCode::KeyK)]
        );
        // The follower rides both, and is still one sub-row rather than two.
        let jump = row(&app, "override_tests.jump");
        assert_eq!(jump.followers.len(), 1);
        let prompts = BindingTable::new(app.world()).prompts(Lunge::id(), PromptScope::ANY);
        assert_eq!(
            prompts
                .iter()
                .map(|prompt| prompt.origin.control())
                .collect::<Vec<_>>(),
            [
                Some(Control::Key(KeyCode::Space)),
                Some(Control::Key(KeyCode::KeyK))
            ],
            "the rider was copied onto the new slot too"
        );
    }

    /// And the other direction: a row that had two and now has one drops the right binding, and
    /// takes the rider on it with it.
    #[test]
    fn a_shortened_row_drops_the_binding_it_no_longer_has() {
        let mut app = app();
        // A prompt is a runtime question, so something has to be carrying the context
        // before it has an answer at all.
        app.world_mut().spawn(Playing);
        let grown = bind(
            &app,
            "override_tests.jump",
            &[Control::Key(KeyCode::Space), Control::Key(KeyCode::KeyK)],
        );
        apply_overrides(app.world_mut(), &grown);

        let shrunk = bind(&app, "override_tests.jump", &[Control::Key(KeyCode::KeyK)]);
        apply_overrides(app.world_mut(), &shrunk);

        assert_eq!(
            slots(&app, "override_tests.jump"),
            [Control::Key(KeyCode::KeyK)]
        );
        let prompts = BindingTable::new(app.world()).prompts(Lunge::id(), PromptScope::ANY);
        assert_eq!(
            prompts
                .iter()
                .map(|prompt| prompt.origin.control())
                .collect::<Vec<_>>(),
            [Some(Control::Key(KeyCode::KeyK))],
            "Space is gone from the rider as well as from the row"
        );
    }

    /// Swapping a plan cancels what it had in flight, exactly as switching the context off does. A
    /// hold on a control that is no longer bound has to resolve rather than stay held for good.
    #[test]
    fn applying_cancels_what_was_in_flight() {
        use bevy_input::{ButtonState, keyboard::Key, keyboard::KeyboardInput};

        let mut app = app();
        let entity = app.world_mut().spawn(Playing).id();
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Space,
            logical_key: Key::Space,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        assert_eq!(
            app.world()
                .get::<InputContextState<Playing>>(entity)
                .unwrap()
                .phase::<Jump>(),
            Phase::Fired
        );

        let overrides = bind(&app, "override_tests.jump", &[Control::Key(KeyCode::KeyK)]);
        apply_overrides(app.world_mut(), &overrides);

        let state = app
            .world()
            .get::<InputContextState<Playing>>(entity)
            .unwrap();
        assert_eq!(state.phase::<Jump>(), Phase::Canceled);
        assert!(
            state.is_active(),
            "cancelling is not switching the context off"
        );
    }

    /// The bug a per-entity-only answer has: a context spawned after a rebind must be bound the way
    /// the player left it, not the way the game shipped.
    #[test]
    fn an_instance_spawned_after_a_rebind_gets_the_new_bindings() {
        let mut app = app();
        let overrides = bind(&app, "override_tests.jump", &[Control::Key(KeyCode::KeyK)]);
        apply_overrides(app.world_mut(), &overrides);

        app.world_mut().spawn(Playing);
        app.update();

        let prompts = BindingTable::new(app.world()).prompts(Jump::id(), PromptScope::ANY);
        assert_eq!(prompts.len(), 1);
        assert_eq!(
            prompts[0].origin.control(),
            Some(Control::Key(KeyCode::KeyK))
        );
    }

    /// A per-entity apply reaches only the one entity it names — not every other instance of the
    /// same context type, and not the world's own declared/current view, which a freshly spawned
    /// third instance still reads.
    #[test]
    fn apply_overrides_for_reaches_only_its_own_entity() {
        use bevy_input::{ButtonState, keyboard::Key, keyboard::KeyboardInput};

        let mut app = app();
        let player_a = app.world_mut().spawn(Playing).id();
        let player_b = app.world_mut().spawn(Playing).id();

        let overrides = bind(&app, "override_tests.jump", &[Control::Key(KeyCode::KeyK)]);
        let problems = apply_overrides_for(app.world_mut(), player_a, &overrides);
        assert!(problems.is_empty(), "{problems:?}");

        // Spawned after the apply, so it proves the world-wide default was never touched.
        let player_c = app.world_mut().spawn(Playing).id();

        // `adopt` re-arms require-reset on every slot (R7.5: a player holding the key they just
        // rebound must not get a fresh press out of the swap), so the rebound slot needs one tick
        // observed at rest before a press can register — an ordinary idle frame, since nothing was
        // held to begin with.
        app.update();

        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyK,
            logical_key: Key::Character("k".into()),
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        assert_eq!(
            app.world()
                .get::<InputContextState<Playing>>(player_a)
                .unwrap()
                .phase::<Jump>(),
            Phase::Fired,
            "the named entity was remapped to K"
        );
        assert_eq!(
            app.world()
                .get::<InputContextState<Playing>>(player_b)
                .unwrap()
                .phase::<Jump>(),
            Phase::Idle,
            "a sibling instance never asked for K and is still listening on Space"
        );
        assert_eq!(
            app.world()
                .get::<InputContextState<Playing>>(player_c)
                .unwrap()
                .phase::<Jump>(),
            Phase::Idle,
            "spawned after the apply, and still the world's unmodified default"
        );

        // The world-wide declared/current tables read back unchanged: no other entity, and no
        // future one, ever sees Jump listed on anything but Space.
        assert_eq!(
            slots(&app, "override_tests.jump"),
            [Control::Key(KeyCode::Space)]
        );
    }

    /// A binding *changing* is one of the things that makes a prompt on screen stale, and without
    /// this a caption goes on naming the key the player just replaced.
    #[test]
    fn applying_says_prompts_may_have_changed() {
        let mut app = app();
        app.world_mut().spawn(Playing);
        app.update();
        let before = app
            .world()
            .get_resource::<crate::present::PromptGeneration>()
            .map_or(0, |generation| generation.0);

        let overrides = bind(&app, "override_tests.jump", &[Control::Key(KeyCode::KeyK)]);
        apply_overrides(app.world_mut(), &overrides);

        let after = app
            .world()
            .get_resource::<crate::present::PromptGeneration>()
            .map_or(0, |generation| generation.0);
        assert!(after > before, "a rebind said nothing about prompts");
    }

    /// A saved set outlives the build that wrote it, so every one of these is a thing a file can
    /// say — and each is reported rather than dropped, while everything else still applies.
    #[test]
    fn every_unusable_row_is_reported_rather_than_dropped() {
        let mut app = app();
        let jump = row(&app, "override_tests.jump");
        let up = row(&app, "override_tests.move.up");
        let look = row(&app, "override_tests.look");
        let gone = MappingKey::new("override_tests.no_such_action", crate::binding::Part::Whole);

        let mut overrides = Overrides::new();
        overrides.bind(Scheme::KeyboardMouse, gone, [Control::Key(KeyCode::KeyZ)]);
        overrides.bind(look.scheme, look.key, [Control::MouseMotion]);
        overrides.bind(jump.scheme, jump.key, [Control::Key(KeyCode::F1)]);
        overrides.bind(
            up.scheme,
            up.key,
            [Control::Key(KeyCode::KeyI), Control::Key(KeyCode::KeyO)],
        );

        let problems = apply_overrides(app.world_mut(), &overrides);
        let kinds: Vec<_> = problems
            .iter()
            .map(|problem| problem.kind.clone())
            .collect();

        assert!(kinds.contains(&OverrideProblemKind::NoSuchMapping));
        assert!(
            kinds.contains(&OverrideProblemKind::NotRebindable),
            "{kinds:?}"
        );
        assert!(kinds.contains(&OverrideProblemKind::Reserved {
            control: Control::Key(KeyCode::F1)
        }));
        assert!(kinds.contains(&OverrideProblemKind::TooManyControls {
            capacity: Capacity::UpTo(1),
            given: 2
        }));

        // Refused whole, never half: every one of those rows still holds what it shipped with.
        assert_eq!(
            slots(&app, "override_tests.jump"),
            [Control::Key(KeyCode::Space)]
        );
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW)]
        );
    }

    /// A control that reports on the wrong channel cannot fill the row, and a mouse motion is the
    /// clearest case: it has no press for a button action to read.
    #[test]
    fn a_control_of_the_wrong_shape_is_refused() {
        let mut app = app();
        let overrides = bind(&app, "override_tests.jump", &[Control::MouseMotion]);
        let problems = apply_overrides(app.world_mut(), &overrides);

        assert_eq!(
            problems
                .iter()
                .map(|problem| problem.kind.clone())
                .collect::<Vec<_>>(),
            [OverrideProblemKind::WrongShape {
                control: Control::MouseMotion,
                accepts: ChannelShape::Button
            }]
        );
    }

    /// Removing a row *is* the reset, which is the whole benefit of storing a diff — and it works
    /// at each of the four granularities: one row, one action, one context, or everything.
    #[test]
    fn resetting_puts_a_row_back_to_what_the_game_declared() {
        let mut app = app();
        let rows = mappings(app.world());
        let mut overrides = Overrides::new();
        for target in &rows {
            if target.rebinding.is_rebindable() {
                overrides.bind(target.scheme, target.key, [Control::Key(KeyCode::KeyZ)]);
            }
        }

        // One row.
        let up = row(&app, "override_tests.move.up");
        overrides.reset(up.scheme, up.key);
        assert!(overrides.get(up.scheme, up.key).is_none());

        // Every row of one action, which for a composite is all four directions.
        overrides.reset_action(&rows, Move::id());
        assert!(
            !rows
                .iter()
                .filter(|r| r.action == Move::id())
                .any(|r| { overrides.get(r.scheme, r.key).is_some() })
        );

        // Every row of one context, and then the lot.
        overrides.reset_context(&rows, "override_tests.playing");
        assert!(overrides.is_empty());

        overrides.bind(up.scheme, up.key, [Control::Key(KeyCode::KeyZ)]);
        overrides.reset_all();
        assert!(overrides.is_empty());

        apply_overrides(app.world_mut(), &overrides);
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW)]
        );
    }

    /// A preset moves a `Fixed` row a capture cannot, exempting exactly the rows it names from
    /// `NotRebindable` and nothing else.
    #[test]
    fn a_preset_moves_a_fixed_row_a_capture_cannot() {
        let mut app = app();
        let target = row(&app, "override_tests.settings");
        assert_eq!(target.rebinding, Rebinding::Fixed);

        let mut preset = Overrides::new();
        preset.bind(target.scheme, target.key, [Control::Key(KeyCode::F2)]);

        // Refused without a preset: a bare `apply_overrides` treats this row exactly as a capture
        // screen would.
        let problems = apply_overrides(app.world_mut(), &preset);
        assert_eq!(
            problems
                .iter()
                .map(|problem| problem.kind.clone())
                .collect::<Vec<_>>(),
            [OverrideProblemKind::NotRebindable]
        );
        assert_eq!(
            slots(&app, "override_tests.settings"),
            [Control::Key(KeyCode::F1)]
        );

        // The same row moves once the same rows are named as the preset authorizing it.
        let problems = apply_overrides_with_preset(app.world_mut(), &preset, &preset);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&app, "override_tests.settings"),
            [Control::Key(KeyCode::F2)]
        );
    }

    /// A backend owning an action is neither a row the player cleared nor a row they never touched,
    /// which is exactly why there are three states and not two.
    #[test]
    fn a_row_someone_else_owns_is_left_alone() {
        let mut app = app();
        let target = row(&app, "override_tests.jump");
        let mut overrides = Overrides::new();
        overrides.set(target.scheme, target.key, Override::NotOurs);

        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty());
        assert_eq!(
            slots(&app, "override_tests.jump"),
            [Control::Key(KeyCode::Space)],
            "not ours is not cleared"
        );
    }

    /// A movement row grows only when the whole composite does: a second "forward" key is one part
    /// of a second set of four. Copying the composite instead would put the other three directions
    /// in their own rows twice over — "Move Down: S | S" — which is a wrong screen rather than an
    /// untidy one, so the row is refused and the shipped controls stand.
    #[test]
    fn one_direction_of_a_composite_cannot_grow_a_slot_on_its_own() {
        #[derive(InputContext)]
        #[context(path = "override_tests.wide", tick = Render)]
        struct Wide;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Wide>(|controls| {
            controls
                .bind::<Move>(DirectionalButtons::wasd())
                .mappable_upto(2);
        });

        let up = row(&app, "override_tests.move.up");
        let mut overrides = Overrides::new();
        overrides.bind(
            up.scheme,
            up.key,
            [Control::Key(KeyCode::KeyW), Control::Key(KeyCode::KeyI)],
        );
        let problems = apply_overrides(app.world_mut(), &overrides);

        assert_eq!(
            problems
                .iter()
                .map(|problem| problem.kind.clone())
                .collect::<Vec<_>>(),
            [OverrideProblemKind::CompositeCannotGrow]
        );
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW)]
        );
        assert_eq!(
            slots(&app, "override_tests.move.down"),
            [Control::Key(KeyCode::KeyS)],
            "and the other three directions are untouched"
        );
    }

    /// The remedy the refusal above points at, and proof it is a real one: a second composite is
    /// how a two-column movement table is actually written, and each direction then rebinds its own
    /// secondary independently.
    #[test]
    fn a_second_composite_is_how_a_movement_row_gets_a_secondary() {
        #[derive(InputContext)]
        #[context(path = "override_tests.two_sets", tick = Render)]
        struct TwoSets;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<TwoSets>(|controls| {
            controls.bind::<Move>(DirectionalButtons::wasd()).mappable();
            controls
                .bind::<Move>(DirectionalButtons::arrow_keys())
                .mappable();
        });

        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW), Control::Key(KeyCode::ArrowUp)]
        );

        let up = row(&app, "override_tests.move.up");
        let mut overrides = Overrides::new();
        overrides.bind(
            up.scheme,
            up.key,
            [Control::Key(KeyCode::KeyW), Control::Key(KeyCode::KeyI)],
        );
        let problems = apply_overrides(app.world_mut(), &overrides);

        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            [Control::Key(KeyCode::KeyW), Control::Key(KeyCode::KeyI)],
            "the secondary moved and the primary did not"
        );
        assert_eq!(
            slots(&app, "override_tests.move.down"),
            [
                Control::Key(KeyCode::KeyS),
                Control::Key(KeyCode::ArrowDown)
            ],
            "and the other rows kept both of theirs"
        );
    }

    /// Capacity is raised by the author and never lowered by a player (R19.9): two `mappable()`
    /// bindings of one action merge into a two-slot row, and rebinding it down to one control must
    /// not narrow that back to one — `current_rows` used to take the derived row's capacity as-is,
    /// and the derived row only sees the one binding that survived the rebind.
    #[test]
    fn rebinding_a_row_down_does_not_shrink_its_capacity() {
        #[derive(InputContext)]
        #[context(path = "override_tests.two_slots", tick = Render)]
        struct TwoSlots;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<TwoSlots>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(KeyCode::KeyJ).mappable();
        });

        let jump = row(&app, "override_tests.jump");
        assert_eq!(
            jump.capacity,
            Capacity::UpTo(2),
            "two mappable bindings merge into one two-slot row"
        );

        let overrides = bind(&app, "override_tests.jump", &[Control::Key(KeyCode::Space)]);
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty(), "{problems:?}");

        let jump = row(&app, "override_tests.jump");
        assert_eq!(jump.slots, [Control::Key(KeyCode::Space)]);
        assert_eq!(
            jump.capacity,
            Capacity::UpTo(2),
            "the vacated secondary must stay fillable"
        );
    }

    /// The other shape of the same rule, already right by accident: a row cleared to nothing finds
    /// no derived row to widen against and falls back to the declared one whole, capacity included.
    /// A test of its own so the accident does not become a regression once the case above is fixed.
    #[test]
    fn clearing_a_row_does_not_shrink_its_capacity() {
        #[derive(InputContext)]
        #[context(path = "override_tests.cleared_slots", tick = Render)]
        struct ClearedSlots;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<ClearedSlots>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(KeyCode::KeyJ).mappable();
        });

        let jump = row(&app, "override_tests.jump");
        let mut overrides = Overrides::new();
        overrides.set(jump.scheme, jump.key, Override::Cleared);
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty(), "{problems:?}");

        let jump = row(&app, "override_tests.jump");
        assert!(jump.slots.is_empty());
        assert_eq!(jump.capacity, Capacity::UpTo(2));
    }

    /// A key match alone must not move an override across schemes: `hold_or_toggle` reaching both
    /// a keyboard and a gamepad binding under one name declares two independent tunables, one per
    /// scheme's own table, not one shared across devices.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_tunable_override_does_not_cross_schemes() {
        use bevy_input::gamepad::GamepadButton;

        #[derive(InputAction)]
        #[action(path = "override_tests.thrust", output = bool, intent = Button)]
        struct Thrust;

        #[derive(InputContext)]
        #[context(path = "override_tests.cross_scheme", tick = Render)]
        struct CrossScheme;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<CrossScheme>(|controls| {
            controls.bind::<Thrust>(KeyCode::Space);
            controls.bind::<Thrust>(GamepadButton::South);
            controls.hold_or_toggle::<Thrust>("override_tests.thrust.hold_or_toggle");
        });

        let mut overrides = Overrides::new();
        overrides.tune(
            Scheme::KeyboardMouse,
            "override_tests.thrust.hold_or_toggle",
            TunableValue::Bool(true),
        );
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty(), "{problems:?}");

        let tunables = crate::mapping::tunables(app.world());
        let keyboard = tunables
            .iter()
            .find(|t| t.scheme == Scheme::KeyboardMouse)
            .expect("a keyboard row");
        let gamepad = tunables
            .iter()
            .find(|t| t.scheme == Scheme::Gamepad)
            .expect("a gamepad row");
        assert_eq!(keyboard.value, TunableValue::Bool(true));
        assert_eq!(
            gamepad.value,
            TunableValue::Bool(false),
            "the gamepad row must not have moved"
        );
    }

    /// The file a person edits by hand, pinned by a golden document rather than by an intention
    /// nobody rechecks.
    #[cfg(all(feature = "gamepad", feature = "serialize"))]
    mod persistence {
        use super::*;

        use bevy_input::gamepad::GamepadButton;
        use serde::de::DeserializeSeed;

        #[derive(InputAction)]
        #[action(path = "persist_tests.move", output = bevy_math::Vec2, intent = Directional2)]
        struct Move;

        #[derive(InputAction)]
        #[action(path = "persist_tests.jump", output = bool, intent = Button)]
        struct Jump;

        #[derive(InputAction)]
        #[action(path = "persist_tests.settings", output = bool, intent = Button)]
        struct OpenSettings;

        #[derive(InputContext)]
        #[context(path = "persist_tests.playing", tick = Render)]
        struct Playing;

        /// `Move` on WASD (four keyboard rows, none of them overridden below), `Jump` on Space with
        /// room for a secondary and on the pad's South button, and a settings key an external
        /// backend will claim.
        fn declared() -> Vec<Mapping> {
            let mut app = App::new();
            app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
            app.add_context::<Playing>(|controls| {
                controls.bind::<Move>(DirectionalButtons::wasd()).mappable();
                controls.bind::<Jump>(KeyCode::Space).mappable_upto(2);
                controls.bind::<Jump>(GamepadButton::South).mappable();
                controls.bind::<OpenSettings>(KeyCode::F1).mappable();
            });
            declared_mappings(app.world())
        }

        fn mapping_key(declared: &[Mapping], scheme: Scheme, name: &str) -> MappingKey {
            declared
                .iter()
                .find(|mapping| mapping.scheme == scheme && mapping.key.to_string() == name)
                .unwrap_or_else(|| panic!("no mapping named {name} in {scheme:?}"))
                .key
        }

        const GOLDEN: &str = "version = 1\n\
            \n\
            [bindings.keyboard_mouse]\n\
            \"persist_tests.jump\" = [\"key/Space\", \"key/KeyJ\"]\n\
            \"persist_tests.move.up\" = \"key/KeyI\"\n\
            \"persist_tests.settings\" = \"external\"\n\
            \n\
            [bindings.gamepad]\n\
            \"persist_tests.jump\" = \"cleared\"\n";

        /// What `Overrides` writes is a document a person would be willing to write by hand, and
        /// reading it back produces the identical value — a scalar for the row that holds one
        /// control, a list for the row that holds two, and the two three-state words neither of
        /// which could ever be mistaken for a control name.
        #[test]
        fn a_saved_override_set_round_trips_through_a_legible_file() {
            let declared = declared();
            let mut overrides = Overrides::new();
            overrides.bind(
                Scheme::KeyboardMouse,
                mapping_key(&declared, Scheme::KeyboardMouse, "persist_tests.move.up"),
                [Control::Key(KeyCode::KeyI)],
            );
            overrides.bind(
                Scheme::KeyboardMouse,
                mapping_key(&declared, Scheme::KeyboardMouse, "persist_tests.jump"),
                [Control::Key(KeyCode::Space), Control::Key(KeyCode::KeyJ)],
            );
            overrides.set(
                Scheme::KeyboardMouse,
                mapping_key(&declared, Scheme::KeyboardMouse, "persist_tests.settings"),
                Override::NotOurs,
            );
            overrides.set(
                Scheme::Gamepad,
                mapping_key(&declared, Scheme::Gamepad, "persist_tests.jump"),
                Override::Cleared,
            );

            let text = toml::to_string(&overrides).expect("serializes");
            assert_eq!(text, GOLDEN);

            let (loaded, problems, unresolved, unresolved_tunables) = OverridesLoader {
                declared: &declared,
                declared_tunables: &[],
            }
            .deserialize(toml::Deserializer::new(&text))
            .expect("deserializes");

            assert!(problems.is_empty(), "{problems:?}");
            assert!(unresolved.is_empty(), "{unresolved:?}");
            assert!(unresolved_tunables.is_empty(), "{unresolved_tunables:?}");
            assert_eq!(loaded, overrides);
        }

        /// A name this build cannot turn into a `Control` is reported rather than dropped in
        /// silence, and the row after it in the same file still loads.
        #[test]
        fn an_unknown_control_is_reported_and_the_rest_still_loads() {
            let declared = declared();
            let text = "version = 1\n\
                \n\
                [bindings.keyboard_mouse]\n\
                \"persist_tests.move.up\" = \"key/DoesNotExist\"\n\
                \"persist_tests.jump\" = \"key/Space\"\n";

            let (loaded, problems, unresolved, unresolved_tunables) = OverridesLoader {
                declared: &declared,
                declared_tunables: &[],
            }
            .deserialize(toml::Deserializer::new(text))
            .expect("deserializes");

            assert!(unresolved.is_empty(), "{unresolved:?}");
            assert!(unresolved_tunables.is_empty(), "{unresolved_tunables:?}");
            assert_eq!(
                problems
                    .iter()
                    .map(|problem| problem.kind.clone())
                    .collect::<Vec<_>>(),
                [OverrideProblemKind::UnknownControl {
                    name: "key/DoesNotExist".into()
                }]
            );
            // Refused whole: the row that named it holds nothing rather than half a rebind.
            assert_eq!(
                loaded.get(
                    Scheme::KeyboardMouse,
                    mapping_key(&declared, Scheme::KeyboardMouse, "persist_tests.move.up")
                ),
                None
            );
            // And the row after it in the file still loaded.
            assert_eq!(
                loaded.get(
                    Scheme::KeyboardMouse,
                    mapping_key(&declared, Scheme::KeyboardMouse, "persist_tests.jump")
                ),
                Some(&Override::Controls(alloc::vec![Control::Key(
                    KeyCode::Space
                )]))
            );
        }

        /// A renamed or removed action's row comes back named rather than vanishing without a
        /// trace.
        #[test]
        fn an_unresolved_mapping_name_is_reported_by_name() {
            let declared = declared();
            let text = "version = 1\n\
                \n\
                [bindings.keyboard_mouse]\n\
                \"persist_tests.no_such_action\" = \"key/KeyZ\"\n";

            let (loaded, problems, unresolved, unresolved_tunables) = OverridesLoader {
                declared: &declared,
                declared_tunables: &[],
            }
            .deserialize(toml::Deserializer::new(text))
            .expect("deserializes");

            assert!(problems.is_empty(), "{problems:?}");
            assert!(unresolved_tunables.is_empty(), "{unresolved_tunables:?}");
            assert!(loaded.is_empty());
            assert_eq!(
                unresolved,
                [UnresolvedMapping {
                    scheme: Scheme::KeyboardMouse,
                    name: "persist_tests.no_such_action".into()
                }]
            );
        }

        fn declared_hold_or_toggle() -> Vec<Tunable> {
            let mut app = App::new();
            app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
            app.add_context::<Playing>(|controls| {
                controls.bind::<Jump>(KeyCode::Space).mappable();
                controls.hold_or_toggle::<Jump>("persist_tests.jump.hold_or_toggle");
            });
            crate::mapping::declared_tunables(app.world())
        }

        /// A tunable round-trips through the same file a mapping does.
        #[test]
        fn a_tunable_round_trips_through_a_saved_file() {
            let declared = declared();
            let tunables = declared_hold_or_toggle();

            let mut overrides = Overrides::new();
            overrides.tune(
                Scheme::KeyboardMouse,
                "persist_tests.jump.hold_or_toggle",
                TunableValue::Bool(true),
            );
            let text = toml::to_string(&overrides).expect("serializes");

            let (loaded, problems, unresolved, unresolved_tunables) = OverridesLoader {
                declared: &declared,
                declared_tunables: &tunables,
            }
            .deserialize(toml::Deserializer::new(&text))
            .expect("deserializes");

            assert!(problems.is_empty(), "{problems:?}");
            assert!(unresolved.is_empty(), "{unresolved:?}");
            assert!(unresolved_tunables.is_empty(), "{unresolved_tunables:?}");
            assert_eq!(loaded, overrides);
        }

        /// A name this build has no tunable for is reported by name rather than dropped, the same
        /// as an unresolved mapping.
        #[test]
        fn an_unresolved_tunable_name_is_reported_by_name() {
            let declared = declared();
            let text = "version = 1\n\
                \n\
                [tunables.keyboard_mouse]\n\
                \"persist_tests.no_such_tunable\" = true\n";

            let (loaded, problems, unresolved, unresolved_tunables) = OverridesLoader {
                declared: &declared,
                declared_tunables: &[],
            }
            .deserialize(toml::Deserializer::new(text))
            .expect("deserializes");

            assert!(problems.is_empty(), "{problems:?}");
            assert!(unresolved.is_empty(), "{unresolved:?}");
            assert!(loaded.is_empty());
            assert_eq!(
                unresolved_tunables,
                [UnresolvedTunable {
                    scheme: Scheme::KeyboardMouse,
                    name: "persist_tests.no_such_tunable".into()
                }]
            );
        }

        /// A saved value outside the range the game currently declares is clamped rather than
        /// refused — unlike a mapping row, there is no wrong *shape* for a number to be, only a
        /// stale bound, and a slider does not need an error to know where its ends are.
        #[test]
        fn a_range_tunable_outside_its_bounds_is_clamped_on_load() {
            let mut app = App::new();
            app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
            app.add_context::<Playing>(|controls| {
                controls
                    .bind::<Move>(crate::binding::Stick::Left)
                    .dead_zone(crate::binding::DeadZone::radial(0.15))
                    .tunable_dead_zone("persist_tests.move.stick_deadzone", 0.0..=0.5);
            });
            let tunables = crate::mapping::declared_tunables(app.world());

            let text = "version = 1\n\
                \n\
                [tunables.gamepad]\n\
                \"persist_tests.move.stick_deadzone\" = 5.0\n";

            let (loaded, problems, unresolved, unresolved_tunables) = OverridesLoader {
                declared: &[],
                declared_tunables: &tunables,
            }
            .deserialize(toml::Deserializer::new(text))
            .expect("deserializes");

            assert!(problems.is_empty(), "{problems:?}");
            assert!(unresolved.is_empty(), "{unresolved:?}");
            assert!(unresolved_tunables.is_empty(), "{unresolved_tunables:?}");
            assert_eq!(
                loaded.get_tunable(Scheme::Gamepad, "persist_tests.move.stick_deadzone"),
                Some(TunableValue::Range {
                    value: 0.5,
                    min: 0.0,
                    max: 0.5,
                })
            );
        }
    }
}
