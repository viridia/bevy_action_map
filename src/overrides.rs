//! What the player changed, and putting it back into a running game.
//!
//! Everything else in this crate describes what a game *declared*. This describes what a player did
//! to it afterwards — a set of rows saying "move forward is `E` now" — and the one call that makes a
//! running game agree with it.
//!
//! ```ignore
//! let mut overrides = Overrides::new();
//! overrides.bind(DeviceFamily::KeyboardMouse, forward.key, [Control::PhysicalKey(KeyCode::KeyE)]);
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
#[cfg(feature = "bevy_reflect")]
use bevy_ecs::reflect::ReflectResource;
use bevy_ecs::world::World;
#[cfg(feature = "serialize")]
use bevy_reflect::{Reflect, ReflectDeserialize, ReflectSerialize};

use crate::action::ChannelShape;
use crate::binding::{BindingSpec, Control};
use crate::capture::{ControlClass, RefusedReason, admissible};
use crate::device::DeviceFamily;
use crate::mapping::{ActionMapping, BoundSlot, MappingKey, RebindPolicy, Tunable, TunableValue};
use crate::mapping::{apply_tunable_value, mapped_parts};
use crate::present::ControlOrigin;

/// What a player did to one mapping.
///
/// Emptying a row has a value of its own, because a diff against defaults makes *absence*
/// meaningful: once a missing row already says "use the default", a player who deliberately emptied
/// a row would have nothing left to say it with.
#[derive(Clone, Debug, PartialEq)]
pub enum Override {
    /// What the player put in the mapping, in slot order.
    ///
    /// Position is which slot, so this is written and read in order: the first is the primary. It
    /// replaces the mapping's whole list rather than one position in it — a screen that edits a
    /// single cell edits the list and then writes the row.
    ///
    /// Each slot says everything about what is bound there, including what is held with it. A slot
    /// with an empty [`with`](BoundSlot::with) binds its control on its own, whatever chord the
    /// game declared for that position.
    ///
    /// `None` is a slot the player emptied while a later one still holds something. That is what
    /// keeps the secondary of a cleared primary where it is instead of promoting it, and it is the
    /// only way a gap arises: a game cannot declare one. A row with nothing left is
    /// [`Cleared`](Self::Cleared) rather than a list of empties.
    Slots(Vec<Option<BoundSlot>>),
    /// The player deliberately emptied the mapping.
    ///
    /// The action stays declared and stays readable; nothing fires it. Distinct from a missing row,
    /// which means the game's own default still applies.
    Cleared,
}

/// Everything a player has changed, as a diff against what the game declared.
///
/// Rows are keyed by mapping and by family, because a mapping name is unique within a family and a
/// keyboard remap must not disturb the gamepad layout. Nothing here names a device: what a player
/// bound is a control on a device *class*, and which physical unit drives which player is a separate
/// question with a separate answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    rows: BTreeMap<(DeviceFamily, MappingKey), Override>,
    // Keyed by the tunable's own declared key, resolved to the `&'static str` a running game holds
    // rather than kept as the owned `String` a save file loads — the same reason `rows` is keyed by
    // `MappingKey` rather than by name. Unlike a mapping row, a tunable has no "cleared" state: there
    // is nothing between "this value" and "no row, use the default", so a bare value is enough.
    tunables: BTreeMap<(DeviceFamily, &'static str), TunableValue>,
}

impl Overrides {
    /// An empty set, which is a game running on exactly what it declared.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether every mapping and tunable is still what the game declared.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.tunables.is_empty()
    }

    /// Puts controls in a mapping.
    ///
    /// The whole list, in slot order. Takes bare controls, or the [`BoundSlot`]s a row is made of
    /// where a slot is held with something, so a screen editing one cell of a row writes the row
    /// back as it stands — `None` for a cell the player emptied whose position still matters:
    ///
    /// ```ignore
    /// overrides.bind(family, jump, [Control::PhysicalKey(KeyCode::Space)]);
    /// let j = BoundSlot::from(Control::PhysicalKey(KeyCode::KeyJ));
    /// overrides.bind(family, jump, [None, Some(j)]);
    ///
    /// let ctrl_s = BoundSlot {
    ///     control: Control::PhysicalKey(KeyCode::KeyS),
    ///     with: vec![ControlOrigin::Modifier(ModifierKey::Ctrl)],
    /// };
    /// overrides.bind(family, save, [ctrl_s]);
    /// ```
    ///
    /// A bare control is bound on its own: where the game declared `Ctrl+S`, binding `D` gives `D`,
    /// not `Ctrl+D`. To carry the chord across, edit the slot's [`control`](BoundSlot::control)
    /// rather than replacing the slot.
    ///
    /// Trailing empties are dropped: a row is as long as its last filled slot, and how many cells
    /// to draw beside it is the screen's business rather than something a saved row should carry. A
    /// list with nothing left in it is [`Override::Cleared`] and is stored as such, since a row
    /// holding nothing and a row that is not there mean different things.
    ///
    /// Interior empties are kept, because a fixed-column table means something by *which* column a
    /// control sits in. A growable list — an editor's shortcuts for one command, where position is
    /// an artefact of iteration — wants the opposite, and closes the gaps on the way in:
    ///
    /// ```ignore
    /// overrides.bind(family, key, row);                        // a table: keep the gaps
    /// overrides.bind(family, key, row.into_iter().flatten());  // a list: close them
    /// ```
    pub fn bind<S: Into<Option<BoundSlot>>>(
        &mut self,
        family: DeviceFamily,
        mapping: MappingKey,
        slots: impl IntoIterator<Item = S>,
    ) {
        let mut slots: Vec<Option<BoundSlot>> = slots.into_iter().map(Into::into).collect();
        while slots.last().is_some_and(Option::is_none) {
            slots.pop();
        }
        self.set(
            family,
            mapping,
            if slots.is_empty() {
                Override::Cleared
            } else {
                Override::Slots(slots)
            },
        );
    }

    /// Empties one slot of a row, leaving the slots after it where they are.
    ///
    /// What a "clear this cell" button does. The slots after the one emptied keep their positions,
    /// so clearing the primary of a two-control row leaves the secondary in the second column — a
    /// row is a table, and removing an element from the middle of it would promote the secondary
    /// into a column the player was not looking at.
    ///
    /// `mapping` is the row as it stands, which is where the slots this set does not mention come
    /// from — pass the row from [`mappings`](crate::mapping::mappings), and rows this set has
    /// already changed are read from the set rather than from it. A slot the row does not reach is
    /// already empty, so clearing one does nothing.
    ///
    /// The row normalizes on the way in as it does for [`bind`](Self::bind): clearing the last
    /// filled slot shortens the row, and clearing the only one leaves
    /// [`Override::Cleared`](Override::Cleared).
    pub fn unbind(&mut self, mapping: &ActionMapping, slot: usize) {
        let mut slots = self.slots_of(mapping);
        if slot >= slots.len() {
            return;
        }
        slots[slot] = None;
        self.bind(mapping.family, mapping.key, slots);
    }

    /// What this set makes of one row: its own slots where it has changed the row, and the
    /// declared ones where it has not.
    ///
    /// Rows read the way applying reads them, so a screen showing an unconfirmed working copy shows
    /// what confirming it would produce.
    ///
    /// This is the list to edit and hand back to [`bind`](Self::bind). An untouched row comes back
    /// exactly as [`slots`](ActionMapping::slots) holds it, chords and all, so writing it back after
    /// changing one cell changes only that cell.
    pub fn slots_of(&self, mapping: &ActionMapping) -> Vec<Option<BoundSlot>> {
        match self.get(mapping.family, mapping.key) {
            Some(Override::Slots(slots)) => slots.clone(),
            Some(Override::Cleared) => Vec::new(),
            None => mapping.slots.clone(),
        }
    }

    /// This row with `control` in one of its cells, ready for [`Rebind::checked`].
    ///
    /// The cell the player pressed is the cell that gets the control, whether or not the row reaches
    /// that far yet: a row that does not is grown, and the cells skipped on the way are left empty.
    /// Writing to the third cell of a row holding one control gives a row of three with a blank in
    /// the middle, so a table can offer whatever cells it draws without first asking how long the
    /// row happens to be.
    ///
    /// A cell already holding something keeps whatever it was held with, so retyping the control of
    /// a `Ctrl+S` cell leaves the `Ctrl`. An empty cell takes the control on its own.
    ///
    /// This computes a row and changes nothing. [`Rebind::checked`] is what says whether the row may
    /// be held, and writing it is [`Rebind::write`].
    pub fn with_cell(
        &self,
        mapping: &ActionMapping,
        slot: usize,
        control: Control,
    ) -> Vec<Option<BoundSlot>> {
        let mut slots = self.slots_of(mapping);
        if slot >= slots.len() {
            slots.resize(slot + 1, None);
        }
        match &mut slots[slot] {
            Some(filled) => filled.control = control,
            empty => *empty = Some(control.into()),
        }
        slots
    }

    /// Sets a row directly, for the two states [`bind`](Self::bind) cannot express.
    pub fn set(&mut self, family: DeviceFamily, mapping: MappingKey, value: Override) {
        self.rows.insert((family, mapping), value);
    }

    /// What the player did to one mapping, or `None` where they left it alone.
    pub fn get(&self, family: DeviceFamily, mapping: MappingKey) -> Option<&Override> {
        self.rows.get(&(family, mapping))
    }

    /// Every row, in a stable order.
    pub fn iter(&self) -> impl Iterator<Item = (DeviceFamily, MappingKey, &Override)> {
        self.rows
            .iter()
            .map(|(&(family, key), value)| (family, key, value))
    }

    /// Sets a tunable to `value`.
    pub fn tune(&mut self, family: DeviceFamily, key: &'static str, value: TunableValue) {
        self.tunables.insert((family, key), value);
    }

    /// What the player set one tunable to, or `None` where they left it alone.
    pub fn get_tunable(&self, family: DeviceFamily, key: &'static str) -> Option<TunableValue> {
        self.tunables.get(&(family, key)).copied()
    }

    /// Every tunable row, in a stable order.
    pub fn iter_tunables(
        &self,
    ) -> impl Iterator<Item = (DeviceFamily, &'static str, TunableValue)> {
        self.tunables
            .iter()
            .map(|(&(family, key), &value)| (family, key, value))
    }

    /// Puts one tunable back to what the game declared.
    pub fn reset_tunable(&mut self, family: DeviceFamily, key: &'static str) {
        self.tunables.remove(&(family, key));
    }

    /// Puts one mapping back to what the game declared.
    ///
    /// Removing the row *is* the reset, which is the whole benefit of storing a diff.
    pub fn reset(&mut self, family: DeviceFamily, mapping: MappingKey) {
        self.rows.remove(&(family, mapping));
    }

    /// Puts every mapping of one action back to what the game declared.
    ///
    /// Takes the mapping list because a row is keyed by mapping alone, and which mappings belong to
    /// an action is a fact about the declaration rather than about the diff. An action bound to a
    /// composite has one row per direction, and this resets all of them.
    pub fn reset_action(&mut self, mappings: &[ActionMapping], action: crate::action::ActionId) {
        self.reset_matching(mappings, |mapping| mapping.action == action);
    }

    /// Puts every mapping declared in one context back to what the game declared.
    ///
    /// `context` is the path the context declared, which is what
    /// [`ActionMapping::context`](crate::mapping::ActionMapping::context) carries.
    pub fn reset_context(&mut self, mappings: &[ActionMapping], context: &str) {
        self.reset_matching(mappings, |mapping| mapping.context == context);
    }

    /// Puts everything back to what the game declared.
    pub fn reset_all(&mut self) {
        self.rows.clear();
        self.tunables.clear();
    }

    fn reset_matching(
        &mut self,
        mappings: &[ActionMapping],
        keep: impl Fn(&ActionMapping) -> bool,
    ) {
        for mapping in mappings.iter().filter(|mapping| keep(mapping)) {
            self.reset(mapping.family, mapping.key);
        }
    }
}

/// How far one mapping's row may reach, for a game that reads override sets it did not write.
///
/// A mapping is an ordered list with no length of its own: how many controls a row *ought* to hold
/// is a question its settings screen answers, by deciding how many cells to draw. This is the other
/// question — how much a file is allowed to say. An override set from a cloud save, a shared
/// profile or a hand-edited settings file can name ten thousand controls for one row, and a game
/// that reads such a file wants a point past which it stops.
///
/// **Insert it to have one; without it there is no limit.** A game whose save files are its own
/// already trusts them, and leaving this out keeps that:
///
/// ```ignore
/// app.insert_resource(MaxSlots(8));
/// ```
///
/// Only [`apply_overrides`] and its variants enforce it, so a row past the limit comes back as
/// [`OverrideProblemKind::TooManyControls`] and the rest of the set still applies.
///
/// **Set it at least as high as your widest table.** A capture fills the cell the player pressed,
/// so a screen drawing more columns than this allows will capture a control and then have the row
/// turned down when it is applied. The crate warns once when a capture opens for a slot the ceiling
/// would refuse, because the two numbers are both yours and only the game can reconcile them.
///
/// **What your game declares is never limited.** A row holds however many controls its bindings
/// give it, whatever this says — the limit is about what an override set may *do* to a row, not how
/// long one is allowed to be. So a fixed row listing a dozen controls is yours to ship, and a
/// preset moving a row is held to the limit like anything else applied.
#[cfg_attr(
    feature = "bevy_reflect",
    derive(bevy_reflect::Reflect),
    reflect(Resource)
)]
#[derive(bevy_ecs::resource::Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaxSlots(pub usize);

/// A row an override set named that could not be used, and why.
///
/// A player whose binding quietly vanished is owed better than silence, so a row this build cannot
/// use is reported rather than dropped.
#[derive(Clone, Debug, PartialEq)]
pub struct OverrideProblem {
    /// The family the row was filed under.
    pub family: DeviceFamily,
    /// The mapping the row named.
    pub mapping: MappingKey,
    /// What was wrong with it.
    pub kind: OverrideProblemKind,
}

/// What was wrong with an override row.
///
/// Not `Copy`, since a variant can carry a loaded control's name: clone a `kind` you want to keep
/// rather than moving it out from behind a reference.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum OverrideProblemKind {
    /// No mapping of that name in that family is declared any more.
    ///
    /// What a renamed or removed binding looks like from inside a file written by an older build.
    NoSuchMapping,
    /// The mapping exists and the player may not change it.
    NotRebindable,
    /// The mapping belongs to an outside authority, and is changed in that authority's own screen.
    ///
    /// A screen never meets this if it reads [`RebindPolicy::Delegated`] off the row first. A file
    /// written before the authority took the row over is the usual way to reach it, and a preset
    /// does not exempt it.
    Delegated,
    /// A control belongs to the other device family.
    ///
    /// A mapping is rebound within its own family, so a gamepad button cannot fill a keyboard row.
    WrongFamily {
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
    /// A slot asks for something to be held that a player cannot hold.
    ///
    /// A chord is made of modifiers and buttons. A stick, a mouse's motion, or a control only an
    /// external backend knows has no pressed state for a chord to wait on.
    NotChordable {
        /// The entry that cannot be held.
        entry: ControlOrigin,
    },
    /// A row reaching further than [`MaxSlots`] allows.
    TooManyControls {
        /// The ceiling the game set.
        limit: usize,
        /// How many slots the row reached. An empty slot counts: it is a column the row has, and
        /// the ceiling is about how far a row reaches rather than how full it is.
        given: usize,
    },
    /// A saved slot naming a control, or something held with one, that this build does not
    /// recognize.
    ///
    /// What a control renamed or removed since the file was written looks like. Distinct from
    /// [`WrongFamily`](Self::WrongFamily) and [`WrongShape`](Self::WrongShape), which both name an
    /// actual [`Control`] — this one has none, because the text a loaded row held did not resolve
    /// to one at all.
    #[cfg(feature = "serialize")]
    UnknownControl {
        /// The text the file held for the slot, exactly as saved.
        name: String,
    },
}

/// This crate's own persistence-format version. Any other is refused outright rather than resolved
/// as this one — see [`UnsupportedVersion`] (D58).
#[cfg(feature = "serialize")]
const FORMAT_VERSION: u32 = 1;

/// The word a saved file uses for an emptied row, and for an emptied slot inside one.
///
/// One word at both levels because it means the same thing at both: nothing is bound here. Every
/// real control name carries a `/`, so this cannot collide with one.
#[cfg(feature = "serialize")]
const CLEARED: &str = "cleared";

/// The name a saved file uses for a device family, stable independent of [`DeviceFamily`]'s own
/// variant names.
#[cfg(feature = "serialize")]
const fn family_name(family: DeviceFamily) -> &'static str {
    match family {
        DeviceFamily::KeyboardMouse => "keyboard_mouse",
        DeviceFamily::Gamepad => "gamepad",
    }
}

#[cfg(feature = "serialize")]
fn family_from_name(name: &str) -> Option<DeviceFamily> {
    match name {
        "keyboard_mouse" => Some(DeviceFamily::KeyboardMouse),
        "gamepad" => Some(DeviceFamily::Gamepad),
        _ => None,
    }
}

/// How a saved file writes one slot: what is held first, then the control, joined by `+`.
///
/// Entries are written under the same names a catalogue looks them up by, so a modifier is
/// `mod/ctrl` and `Ctrl+S` is `mod/ctrl+key/KeyS`.
#[cfg(feature = "serialize")]
fn slot_name(slot: &BoundSlot) -> String {
    let mut name = String::new();
    for entry in &slot.with {
        name.push_str(&entry.name());
        name.push('+');
    }
    name.push_str(&slot.control.name());
    name
}

/// Reads back a slot written by [`slot_name`], or `None` where any part of it names nothing this
/// build knows.
#[cfg(feature = "serialize")]
fn slot_from_name(text: &str) -> Option<BoundSlot> {
    let mut names = Vec::new();
    let mut rest = text;
    loop {
        // `char/` is the one name that can hold a `+`, and it is always one character long, so it
        // is measured rather than split.
        let end = match rest
            .strip_prefix("char/")
            .and_then(|tail| tail.chars().next())
        {
            Some(character) => "char/".len() + character.len_utf8(),
            None => rest.find('+').unwrap_or(rest.len()),
        };
        let (name, tail) = rest.split_at(end);
        names.push(name);
        match tail.strip_prefix('+') {
            Some(tail) => rest = tail,
            None if tail.is_empty() => break,
            None => return None,
        }
    }
    let control = Control::from_name(names.pop()?)?;
    let with = names
        .into_iter()
        .map(origin_from_name)
        .collect::<Option<Vec<_>>>()?;
    Some(BoundSlot { control, with })
}

/// One chord entry's name, read back. Whether it is something a player can hold is for applying to
/// decide, as it is for a slot built in code.
#[cfg(feature = "serialize")]
fn origin_from_name(name: &str) -> Option<ControlOrigin> {
    #[cfg(feature = "keyboard")]
    if let Some(modifier) = crate::present::modifier_from_name(name) {
        return Some(ControlOrigin::Modifier(modifier));
    }
    Control::from_name(name).map(ControlOrigin::Ours)
}

/// A row's saved value: the portable counterpart to [`Override`].
///
/// Written only as a value inside [`SavedOverrides::bindings`]'s nested map, never as a document's
/// own top level. That placement is what keeps the wire form the two compact shapes below rather
/// than bevy_reflect's generic enum representation, `{"Slots": [...]}` and the like.
#[cfg(feature = "serialize")]
#[derive(Reflect, Clone, Debug, PartialEq)]
#[reflect(Serialize, Deserialize)]
pub enum SavedRow {
    /// What the player put in the mapping, in slot order. See [`Override::Slots`].
    ///
    /// One string per slot: the control's name, after whatever is held with it and a `+` apiece, so
    /// `Ctrl+S` is `"mod/ctrl+key/KeyS"`.
    Slots(Vec<String>),
    /// The player deliberately emptied the mapping. Written and read as `"cleared"`.
    Cleared,
}

#[cfg(feature = "serialize")]
impl serde::Serialize for SavedRow {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            // A bare word a person reads as neither a control nor a mistake — every real control
            // name carries a `/`, so the two can never collide (R17.7).
            SavedRow::Cleared => serializer.serialize_str(CLEARED),
            // A scalar is the same thing as a one-element list, and most rows hold one — a player
            // editing this by hand should not have to type brackets to say so (TD10.3).
            SavedRow::Slots(names) if names.len() == 1 => serializer.serialize_str(&names[0]),
            SavedRow::Slots(names) => names.serialize(serializer),
        }
    }
}

#[cfg(feature = "serialize")]
impl<'de> serde::Deserialize<'de> for SavedRow {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RowVisitor;

        impl<'de> serde::de::Visitor<'de> for RowVisitor {
            type Value = SavedRow;

            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a control name, a list of them, or \"cleared\"")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(match v {
                    CLEARED => SavedRow::Cleared,
                    other => SavedRow::Slots(alloc::vec![String::from(other)]),
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
                Ok(SavedRow::Slots(controls))
            }
        }

        deserializer.deserialize_any(RowVisitor)
    }
}

/// A tunable's saved value: the portable counterpart to [`TunableValue`] — a bare number or a bare
/// bool, never the bounds, which the game's own declaration carries and a saved set has no business
/// repeating. Reached only as a nested field value, on the same terms as [`SavedRow`].
#[cfg(feature = "serialize")]
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Serialize, Deserialize)]
pub enum SavedTunableValue {
    /// See [`TunableValue::Range`].
    Number(f32),
    /// See [`TunableValue::Bool`].
    Bool(bool),
}

#[cfg(feature = "serialize")]
impl serde::Serialize for SavedTunableValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            SavedTunableValue::Number(value) => serializer.serialize_f32(*value),
            SavedTunableValue::Bool(value) => serializer.serialize_bool(*value),
        }
    }
}

#[cfg(feature = "serialize")]
impl<'de> serde::Deserialize<'de> for SavedTunableValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ValueVisitor;

        impl<'de> serde::de::Visitor<'de> for ValueVisitor {
            type Value = SavedTunableValue;

            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a number or a bool")
            }

            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(SavedTunableValue::Bool(v))
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(SavedTunableValue::Number(v as f32))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(SavedTunableValue::Number(v as f32))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(SavedTunableValue::Number(v as f32))
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

/// The portable, reflectable shape of an override set.
///
/// [`Overrides`] itself cannot be this shape: its rows are keyed by [`MappingKey`], which only a
/// game's own compiled-in strings can construct. `SavedOverrides` is the stand-in a save file or a
/// `Reflect`-based settings layer (`bevy_settings` and similar) stores instead, holding plain owned
/// strings. Turning one into a live [`Overrides`] is [`resolve_saved`]; the reverse is
/// [`save_overrides`].
///
/// `action_map_version` rather than a bare `version`, and no field beyond `bindings`/`tunables`: a
/// settings layer may place these fields beside an unrelated struct's under one shared table, so
/// nothing here claims a name likely to collide with someone else's.
#[cfg(feature = "serialize")]
#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct SavedOverrides {
    /// This build's persistence-format version. See [`resolve_saved`].
    pub action_map_version: u32,
    /// One table per family, each a map from a mapping's declared path to its saved row.
    pub bindings: BTreeMap<String, BTreeMap<String, SavedRow>>,
    /// One table per family, each a map from a tunable's declared key to its saved value.
    pub tunables: BTreeMap<String, BTreeMap<String, SavedTunableValue>>,
}

/// A player who has changed nothing, written by the build reading it — not a zeroed version this
/// build would then refuse.
///
/// A settings layer builds one of these for a game with no settings file yet, and hands it to
/// [`resolve_saved`] on the first launch like any other.
#[cfg(feature = "serialize")]
impl Default for SavedOverrides {
    fn default() -> Self {
        Self {
            action_map_version: FORMAT_VERSION,
            bindings: BTreeMap::new(),
            tunables: BTreeMap::new(),
        }
    }
}

/// Turns a live [`Overrides`] into its portable, reflectable shape, stamped with the version this
/// build writes. The reverse of [`resolve_saved`].
#[cfg(feature = "serialize")]
pub fn save_overrides(overrides: &Overrides) -> SavedOverrides {
    let mut bindings: BTreeMap<String, BTreeMap<String, SavedRow>> = BTreeMap::new();
    for (family, key, value) in overrides.iter() {
        let row = match value {
            // An empty slot is the same word a whole emptied row uses. `bind` has already dropped
            // any trailing ones, so the word only ever appears where a filled slot follows it.
            Override::Slots(slots) => SavedRow::Slots(
                slots
                    .iter()
                    .map(|slot| {
                        slot.as_ref()
                            .map_or_else(|| String::from(CLEARED), slot_name)
                    })
                    .collect(),
            ),
            Override::Cleared => SavedRow::Cleared,
        };
        bindings
            .entry(family_name(family).to_string())
            .or_default()
            .insert(key.to_string(), row);
    }

    let mut tunables: BTreeMap<String, BTreeMap<String, SavedTunableValue>> = BTreeMap::new();
    for (family, key, value) in overrides.iter_tunables() {
        let saved = match value {
            TunableValue::Range { value, .. } => SavedTunableValue::Number(value),
            TunableValue::Bool(value) => SavedTunableValue::Bool(value),
        };
        tunables
            .entry(family_name(family).to_string())
            .or_default()
            .insert(key.to_string(), saved);
    }

    SavedOverrides {
        action_map_version: FORMAT_VERSION,
        bindings,
        tunables,
    }
}

/// Which table an [`Unresolved`] row was read from.
#[cfg(feature = "serialize")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnresolvedKind {
    /// A `bindings` row.
    Mapping,
    /// A `tunables` row.
    Tunable,
}

/// A row a saved file named that this build cannot place at all.
///
/// Distinct from [`OverrideProblem`], which always names a mapping this build still has. A name
/// matching no current mapping or tunable has nothing to become: what an action renamed or removed
/// since the file was written looks like from inside an older save. A tunable row lands here too
/// when the value on file is the wrong shape for it, a bool where the declared tunable wants a
/// number. The raw text is carried so the row is reported rather than dropped in silence; a
/// rewritten save omits it.
#[cfg(feature = "serialize")]
#[derive(Clone, Debug, PartialEq)]
pub struct Unresolved {
    /// The family table the row was filed under.
    pub family: DeviceFamily,
    /// The name exactly as the file spelled it.
    pub name: String,
    /// Which table it came from.
    pub kind: UnresolvedKind,
}

/// A [`SavedOverrides`] named a persistence-format version this build never shipped.
///
/// Returned by [`resolve_saved`] instead of resolving anything. The case is a rollback, a second
/// machine, or a Steam beta branch: a save from a build that came later, read by one that came
/// before it. There is no migration path, because there has never been a second version for one to
/// convert from.
#[cfg(feature = "serialize")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnsupportedVersion {
    /// The version [`SavedOverrides::action_map_version`] named.
    pub found: u32,
    /// The one version this build resolves.
    pub supported: u32,
}

/// What resolving a [`SavedOverrides`] against a game's current declarations produces: the usable
/// diff, plus a problem list per way a row could fail to carry over.
#[cfg(feature = "serialize")]
pub type ResolvedOverrides = (Overrides, Vec<OverrideProblem>, Vec<Unresolved>);

/// Turns a loaded [`SavedOverrides`] into what this build can use.
///
/// Each row's mapping or tunable name is matched against what the game declares, since a
/// `MappingKey` or a tunable key can only ever be one it already has. A name that matches nothing
/// comes back in the returned [`Unresolved`] list rather than being dropped in silence; a control
/// name that does not parse becomes an `OverrideProblem` instead, because by that point the mapping
/// *did* resolve and there is a row to file the problem against.
///
/// A version this build never shipped refuses the whole set at once, rather than resolving whatever
/// rows happen to look familiar — see [`UnsupportedVersion`].
///
/// ```ignore
/// let declared = declared_mappings(world);
/// let declared_tunables = mapping::declared_tunables(world);
/// let (overrides, problems, unresolved) =
///     resolve_saved(&saved, &declared, &declared_tunables)?;
/// ```
#[cfg(feature = "serialize")]
pub fn resolve_saved(
    data: &SavedOverrides,
    declared: &[ActionMapping],
    declared_tunables: &[Tunable],
) -> Result<ResolvedOverrides, UnsupportedVersion> {
    if data.action_map_version != FORMAT_VERSION {
        return Err(UnsupportedVersion {
            found: data.action_map_version,
            supported: FORMAT_VERSION,
        });
    }

    // `data` may be a live resource the caller keeps, so the rows are cloned rather than moved out.
    let bindings = data.bindings.clone();
    let tunables = data.tunables.clone();

    let mut overrides = Overrides::new();
    let mut problems = Vec::new();
    let mut unresolved = Vec::new();

    for (family_text, rows) in bindings {
        // Not one of ours — a foreign or future family name. Nothing typed to report this
        // against, so R17.2's tolerance is all this can be: skip the table, keep the rest.
        let Some(family) = family_from_name(&family_text) else {
            continue;
        };
        for (name, row) in rows {
            let Some(mapping) = declared
                .iter()
                .find(|candidate| candidate.family == family && candidate.key.to_string() == name)
            else {
                unresolved.push(Unresolved {
                    family,
                    name,
                    kind: UnresolvedKind::Mapping,
                });
                continue;
            };

            let names = match row {
                SavedRow::Cleared => {
                    overrides.set(family, mapping.key, Override::Cleared);
                    continue;
                }
                SavedRow::Slots(names) => names,
            };

            let mut slots = Vec::with_capacity(names.len());
            let mut all_known = true;
            for name in &names {
                // The word inside a list is a slot the player emptied, and a short list read back
                // means the rest are empty too — the trailing ones are not written.
                if name == CLEARED {
                    slots.push(None);
                    continue;
                }
                match slot_from_name(name) {
                    Some(slot) => slots.push(Some(slot)),
                    None => {
                        all_known = false;
                        problems.push(OverrideProblem {
                            family,
                            mapping: mapping.key,
                            kind: OverrideProblemKind::UnknownControl { name: name.clone() },
                        });
                    }
                }
            }
            if all_known {
                // `bind` folds a list with nothing left in it into `Cleared`, so a hand-edited `[]`
                // or `["cleared"]` reads exactly like the bare word does.
                overrides.bind(family, mapping.key, slots);
            }
        }
    }

    for (family_text, rows) in tunables {
        let Some(family) = family_from_name(&family_text) else {
            continue;
        };
        for (name, saved_value) in rows {
            let Some(tunable) = declared_tunables
                .iter()
                .find(|candidate| candidate.family == family && candidate.key == name)
            else {
                unresolved.push(Unresolved {
                    family,
                    name,
                    kind: UnresolvedKind::Tunable,
                });
                continue;
            };

            let value = match (tunable.value, saved_value) {
                (TunableValue::Range { min, max, .. }, SavedTunableValue::Number(number)) => {
                    TunableValue::Range {
                        value: number.clamp(min, max),
                        min,
                        max,
                    }
                }
                (TunableValue::Bool(_), SavedTunableValue::Bool(value)) => {
                    TunableValue::Bool(value)
                }
                // The wrong shape for what this build declares — a bool where a range is wanted,
                // most likely a save written against an older declaration. Reported the same as a
                // name that resolves to nothing, since either way there is nothing usable here.
                _ => {
                    unresolved.push(Unresolved {
                        family,
                        name,
                        kind: UnresolvedKind::Tunable,
                    });
                    continue;
                }
            };
            overrides.tune(family, tunable.key, value);
        }
    }

    Ok((overrides, problems, unresolved))
}

/// A row that may be held, and the only thing that writes one into a set.
///
/// A rebinding screen has a question to ask before it stores what a player pressed: a key cannot
/// fill a gamepad row, a reserved control is spoken for, a row the game marked fixed is not the
/// player's to change. [`checked`](Self::checked) asks all of it at once and answers with an
/// [`OverrideProblemKind`] naming what is wrong, so a screen can say so while the player is still
/// looking at the cell they pressed.
///
/// ```ignore
/// let slots = pending.with_cell(&row, cell, control);
/// match Rebind::checked(world, &row, slots) {
///     Ok(rebind) => rebind.write(&mut pending),
///     Err(problem) => show(problem),
/// }
/// ```
///
/// [`Overrides::bind`] writes whatever it is handed and asks nothing, which is what a set loaded
/// from a file needs — a file can hold anything, and [`apply_overrides`] is what judges one. This
/// is the door for a screen, where the answer is wanted before the write rather than after it.
#[must_use = "a checked rebind does nothing until it is written"]
#[derive(Clone, Debug)]
pub struct Rebind {
    family: DeviceFamily,
    mapping: MappingKey,
    slots: Vec<Option<BoundSlot>>,
}

impl Rebind {
    /// Checks a row against everything the world forbids.
    ///
    /// `slots` is the row as it would stand — [`Overrides::with_cell`] builds one from a cell and a
    /// control, and a screen that edits a row some other way builds its own.
    ///
    /// Use [`checked_with_preset`](Self::checked_with_preset) where the screen offers presets, or a
    /// fixed row a preset legitimately moves is refused.
    pub fn checked(
        world: &World,
        mapping: &ActionMapping,
        slots: Vec<Option<BoundSlot>>,
    ) -> Result<Self, OverrideProblemKind> {
        Self::check(world, mapping, slots, false)
    }

    /// As [`checked`](Self::checked), for a row a preset authorized.
    ///
    /// A preset moves rows a rebinding screen never offers a button for, which is the whole point
    /// of one — so pass the selected preset and a row it names is exempt from
    /// [`NotRebindable`](OverrideProblemKind::NotRebindable). Pass an empty [`Overrides`] where no
    /// preset is selected.
    pub fn checked_with_preset(
        world: &World,
        preset: &Overrides,
        mapping: &ActionMapping,
        slots: Vec<Option<BoundSlot>>,
    ) -> Result<Self, OverrideProblemKind> {
        let authorized = preset.get(mapping.family, mapping.key).is_some();
        Self::check(world, mapping, slots, authorized)
    }

    fn check(
        world: &World,
        mapping: &ActionMapping,
        slots: Vec<Option<BoundSlot>>,
        preset_authorized: bool,
    ) -> Result<Self, OverrideProblemKind> {
        // Absent resources are the ordinary case rather than an error: a game that reserved no
        // control and set no ceiling forbids nothing.
        let reserved: Vec<Control> = world
            .get_resource::<crate::capture::ReservedControls>()
            .map(|reserved| reserved.iter().map(|entry| entry.control).collect())
            .unwrap_or_default();
        let limits = Limits {
            reserved: &reserved,
            max_slots: world.get_resource::<MaxSlots>().map(|max| max.0),
        };
        // The same predicate `apply_overrides` runs, so a screen and a save file cannot disagree
        // about one control.
        if let Some(kind) = refusal(mapping, &slots, &limits, preset_authorized) {
            return Err(kind);
        }
        Ok(Self {
            family: mapping.family,
            mapping: mapping.key,
            slots,
        })
    }

    /// The row this would write, for a screen that wants to show it before committing.
    pub fn slots(&self) -> &[Option<BoundSlot>] {
        &self.slots
    }

    /// Writes it, on the same terms as [`Overrides::bind`] — trailing empties dropped, and a row
    /// with nothing left stored as [`Override::Cleared`].
    pub fn write(self, overrides: &mut Overrides) {
        overrides.bind(self.family, self.mapping, self.slots);
    }
}

/// Makes a running game agree with an override set.
///
/// Every context, every instance, effective on the next tick. This is the only way an override
/// reaches a game; loading a saved set at startup is the first call rather than a path of its own.
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

    // Same diagnostic `apply_with` reports, for the same reason.
    let declared = crate::mapping::declared_mappings(world);
    problems.extend(
        overrides
            .iter()
            .filter(|&(family, key, _)| {
                !declared
                    .iter()
                    .any(|row| row.key == key && row.family == family)
            })
            .map(|(family, mapping, _)| OverrideProblem {
                family,
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
            .filter(|&(family, key, _)| {
                !declared
                    .iter()
                    .any(|row| row.key == key && row.family == family)
            })
            .map(|(family, mapping, _)| OverrideProblem {
                family,
                mapping,
                kind: OverrideProblemKind::NoSuchMapping,
            }),
    );

    // Every prompt on screen may now name a different control, which is the one thing about a
    // rebind that is invisible until someone rebinds with a HUD up.
    crate::present::PromptGeneration::bump(world);
    problems
}

/// What the world forbids, as against what any one context declared.
///
/// Both halves are read off resources and neither belongs to the bindings being rewritten, which is
/// why they travel together rather than as two more arguments.
pub(crate) struct Limits<'a> {
    /// Controls withheld from capture everywhere, from `ReservedControls`.
    pub(crate) reserved: &'a [Control],
    /// The most controls one row may hold, from [`MaxSlots`]. `None` where the game set none.
    pub(crate) max_slots: Option<usize>,
}

/// The pure half: authored bindings and an override set in, rewritten bindings and rows out.
///
/// Separate from the ECS work so that it can be reasoned about and tested without a `World`.
pub(crate) fn rewrite(
    declared: &[BindingSpec],
    rows: &[ActionMapping],
    tunables: &[Tunable],
    overrides: &Overrides,
    preset: Option<&Overrides>,
    limits: &Limits<'_>,
    context: &'static str,
) -> (
    Vec<BindingSpec>,
    Vec<ActionMapping>,
    Vec<Tunable>,
    Vec<OverrideProblem>,
) {
    let mut variant = declared.to_vec();
    let mut problems = Vec::new();
    let mut dropped = alloc::collections::BTreeSet::new();
    let mut grown: Vec<BindingSpec> = Vec::new();
    // What each row was actually given, for the rows that took it — `current_rows` needs this to
    // put the holes back, since nothing in a binding list records which column a control sits in.
    let mut accepted: Vec<Option<Vec<Option<BoundSlot>>>> = alloc::vec![None; rows.len()];

    // Both computed against the *declared* bindings and never re-derived as we go: `leader_of`
    // matches a follower to its leader by the controls the two read, so once an input has been
    // rewritten the two no longer look alike and the link would be lost half way through the pass.
    let parts = mapped_parts(declared);
    let leaders: Vec<Option<usize>> = (0..declared.len())
        .map(|index| crate::mapping::leader_of(declared, index))
        .collect();

    for (index, row) in rows.iter().enumerate() {
        let Some(over) = overrides.get(row.family, row.key) else {
            continue;
        };
        let wanted: &[Option<BoundSlot>] = match over {
            Override::Cleared => &[],
            Override::Slots(slots) => slots,
        };

        let contributors: Vec<_> = parts
            .iter()
            .filter(|part| {
                part.key == row.key
                    && part.family == row.family
                    && declared[part.binding].action == row.action
            })
            .collect();

        let preset_authorized =
            preset.is_some_and(|preset| preset.get(row.family, row.key).is_some());
        if let Some(kind) = refusal(row, wanted, limits, preset_authorized) {
            problems.push(OverrideProblem {
                family: row.family,
                mapping: row.key,
                kind,
            });
            continue;
        }
        accepted[index] = Some(wanted.to_vec());

        for (position, slot) in wanted.iter().enumerate() {
            match (contributors.get(position), slot) {
                // A slot the defaults already fill: the binding stays where it is and reads
                // something else, held with whatever the slot says.
                (Some(part), Some(slot)) => {
                    place(&mut variant[part.binding], part.part, slot);
                    rewrite_followers(declared, &leaders, &mut variant, part.binding);
                }
                // A slot the defaults fill that the player emptied, with something still bound
                // after it. The binding goes, exactly as it does for a slot past the end of the
                // row, and the slots after it keep their positions because position is which slot.
                (Some(part), None) => {
                    dropped.insert(part.binding);
                    dropped.extend(
                        followers_of(declared, &leaders, part.binding).map(|(index, _)| index),
                    );
                }
                // A slot the game shipped nothing for — the empty secondary a screen drew beside
                // the primary. The last binding feeding the row is cloned onto the new slot, so the
                // secondary behaves like the primary rather than like a bare input with no
                // modifiers or conditions on it. Its chord is the slot's own, not the primary's.
                (None, Some(slot)) => {
                    let Some(last) = contributors.last() else {
                        continue;
                    };
                    grown.push(clone_onto(&variant[last.binding], last.part, slot));
                    for (follower, _) in
                        followers_of(declared, &leaders, last.binding).collect::<Vec<_>>()
                    {
                        grown.push(clone_onto(&variant[follower], last.part, slot));
                    }
                }
                // An empty slot the defaults never filled either, so the row already agrees. `bind`
                // drops these from the end of a row, which makes this an interior one.
                (None, None) => {}
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
        let Some(value) = overrides.get_tunable(tunable.family, tunable.key) else {
            continue;
        };
        for binding in &mut variant {
            let Some(decl) = &binding.tunable else {
                continue;
            };
            // DeviceFamily as well as key: sharing is scoped to one family (`hold_or_toggle` reaching a
            // keyboard row shares nothing with a same-named gamepad tunable), and a key match alone
            // would move a keyboard override onto a gamepad binding that only happens to share text.
            if decl.key != tunable.key
                || crate::mapping::binding_family(&binding.input) != tunable.family
            {
                continue;
            }
            apply_tunable_value(&mut binding.modifiers[decl.modifier_index], value);
        }
    }

    let current = current_rows(&variant, rows, &accepted, context);
    let current_tunables = crate::mapping::tunables_of(&variant, context);
    (variant, current, current_tunables, problems)
}

/// Why this row cannot take these controls, if it cannot.
///
/// The whole row is refused rather than partly applied: half a rebind is worse than none, and the
/// player still has the default. `preset_authorized` is the one exception to the rebindable-only
/// rule below it: a preset moves a `Fixed` row on purpose, which is the whole point of one.
fn refusal(
    row: &ActionMapping,
    wanted: &[Option<BoundSlot>],
    limits: &Limits<'_>,
    preset_authorized: bool,
) -> Option<OverrideProblemKind> {
    // Ahead of the preset exemption, and relied on by `rewrite`: a delegated row has no binding
    // with a control in it for an override to rewrite.
    if row.rebind_policy == RebindPolicy::Delegated {
        return Some(OverrideProblemKind::Delegated);
    }
    if !row.rebind_policy.is_rebindable() && !preset_authorized {
        return Some(OverrideProblemKind::NotRebindable);
    }
    // The ceiling first: "this game reads at most eight" is both simpler and truer than anything
    // below it about why a file naming ten thousand is not worth inspecting control by control.
    if let Some(limit) = limits.max_slots
        && wanted.len() > limit
    {
        return Some(OverrideProblemKind::TooManyControls {
            limit,
            given: wanted.len(),
        });
    }
    let accepts = ControlClass::of(row.accepts);
    for slot in wanted.iter().flatten() {
        let control = slot.control;
        // Shared with capture, which is what stops one control getting two different reasons
        // depending on whether it arrived from a press or from a file.
        match admissible(
            control,
            Some(row.family),
            accepts,
            limits.reserved.contains(&control),
        ) {
            Ok(()) => {}
            Err(RefusedReason::Family) => {
                return Some(OverrideProblemKind::WrongFamily { control });
            }
            Err(RefusedReason::Reserved) => return Some(OverrideProblemKind::Reserved { control }),
            Err(RefusedReason::Shape) => {
                return Some(OverrideProblemKind::WrongShape {
                    control,
                    accepts: row.accepts,
                });
            }
        }
        // Neither family nor reservation is asked of a chord entry, as neither is asked of one a
        // game declares.
        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        let unchordable = slot.with.iter().find(|entry| chord_entry(entry).is_none());
        #[cfg(not(any(feature = "keyboard", feature = "mouse", feature = "gamepad")))]
        let unchordable = slot.with.first();
        if let Some(entry) = unchordable {
            return Some(OverrideProblemKind::NotChordable {
                entry: entry.clone(),
            });
        }
    }
    None
}

/// What a binding's chord holds for one entry of a slot's, if a player can hold it.
#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
fn chord_entry(entry: &ControlOrigin) -> Option<crate::binding::ChordEntry> {
    match entry {
        ControlOrigin::Ours(control) => crate::binding::ButtonControl::try_from(*control)
            .ok()
            .map(crate::binding::ChordEntry::Control),
        #[cfg(feature = "keyboard")]
        ControlOrigin::Modifier(modifier) => Some(crate::binding::ChordEntry::Modifier(*modifier)),
        ControlOrigin::Foreign { .. } => None,
    }
}

/// Makes `binding` read `slot`: its control at `part`, held with its chord.
///
/// The chord is replaced rather than kept, since a slot says everything about what is bound there.
/// `refusal` has already turned down a slot with an entry that cannot be held.
fn place(binding: &mut BindingSpec, part: crate::binding::BindingPart, slot: &BoundSlot) {
    binding.input.set_part(part, slot.control);
    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    {
        binding.chord = slot.with.iter().filter_map(chord_entry).collect();
    }
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

/// Moves every rider of `leader` onto the control the leader just took, and the chord with it.
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
    // A follower reads exactly what its leader reads, which is how the link was resolved in the
    // first place.
    for rider in riders {
        variant[rider].input = variant[leader].input;
        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        {
            variant[rider].chord = variant[leader].chord.clone();
        }
    }
}

/// A copy of `binding` reading `slot` in place of whatever it read at `part`.
fn clone_onto(
    binding: &BindingSpec,
    part: crate::binding::BindingPart,
    slot: &BoundSlot,
) -> BindingSpec {
    let mut grown = binding.clone();
    place(&mut grown, part, slot);
    grown
}

/// The presentation rows for a variant, keyed to the declared ones.
///
/// Derived from the rewritten bindings rather than patched, so the rows and the plan cannot
/// disagree about what is bound — with two exceptions the derivation cannot express on its own.
///
/// A row the player emptied has no bindings left and so derives nothing at all; it has to stay on
/// the screen holding nothing, or there is nowhere to bind it back.
///
/// And **a gap has no binding to derive from.** A binding list says what is bound, never in which
/// column, so an emptied primary and a row that only ever held a secondary compile to the same one
/// binding. `accepted` is what the player asked for on each row that was not refused, and it is the
/// authority on the slots of exactly those rows: every slot in it was written into a binding, so it
/// and the derivation always agree about what is bound, and only it knows where the holes are.
fn current_rows(
    variant: &[BindingSpec],
    declared: &[ActionMapping],
    accepted: &[Option<Vec<Option<BoundSlot>>>],
    context: &'static str,
) -> Vec<ActionMapping> {
    let derived = crate::mapping::mappings_of(variant, context);
    declared
        .iter()
        .enumerate()
        .map(|(index, row)| {
            derived
                .iter()
                .find(|current| {
                    current.key == row.key
                        && current.family == row.family
                        && current.action == row.action
                })
                .cloned()
                .map(
                    |current| match accepted.get(index).and_then(Option::as_ref) {
                        Some(wanted) => ActionMapping {
                            slots: wanted.clone(),
                            ..current
                        },
                        None => current,
                    },
                )
                .unwrap_or_else(|| ActionMapping {
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

    use crate::action::{ActionPhase, InputAction as _};
    use crate::binding::DirectionalButtons;
    use crate::context::{ActionMapAppExt, InputContextState};
    use crate::mapping::{RebindPolicy, declared_mappings, mappings};
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

    /// `Move` on WASD (four rows), `Jump` on Space with a `Lunge` riding it, `Look` on the mouse
    /// (listed, unchangeable), and a reserved settings key.
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Playing>(|controls| {
            controls.bind::<Move>(DirectionalButtons::wasd()).mappable();
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.follow::<Lunge, Jump>(|binding| binding.hold(0.4));
            controls.bind::<Look>(crate::binding::MouseMove);
            controls.bind::<OpenSettings>(KeyCode::F1).reserved();
        });
        app
    }

    fn row(app: &App, name: &str) -> ActionMapping {
        mappings(app.world())
            .into_iter()
            .find(|mapping| mapping.key.to_string() == name)
            .unwrap_or_else(|| panic!("no mapping named {name}"))
    }

    fn slots(app: &App, name: &str) -> Vec<Option<Control>> {
        row(app, name).controls()
    }

    /// What one slot of a row requires held alongside its control.
    fn chord(app: &App, name: &str, slot: usize) -> Vec<crate::present::ControlOrigin> {
        row(app, name).slots[slot]
            .as_ref()
            .unwrap_or_else(|| panic!("slot {slot} of {name} is empty"))
            .with
            .clone()
    }

    /// A row with every slot filled, which is what most of these expect. A row with a gap is
    /// written out with the `None` in place, so the two cannot be confused for one another.
    fn filled<const N: usize>(controls: [Control; N]) -> Vec<Option<Control>> {
        controls.into_iter().map(Some).collect()
    }

    /// Presses each key and runs a frame with them held.
    fn hold(app: &mut App, keys: &[KeyCode]) {
        use bevy_input::{ButtonState, keyboard::Key, keyboard::KeyboardInput};
        for &key_code in keys {
            app.world_mut().write_message(KeyboardInput {
                key_code,
                logical_key: Key::Unidentified(bevy_input::keyboard::NativeKey::Unidentified),
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
    }

    fn bind(app: &App, name: &str, controls: &[Control]) -> Overrides {
        let target = row(app, name);
        let mut overrides = Overrides::new();
        overrides.bind(target.family, target.key, controls.iter().copied());
        overrides
    }

    /// A row the player changed reads back changed.
    #[test]
    fn applying_an_override_moves_a_row() {
        let mut app = app();
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            filled([Control::PhysicalKey(KeyCode::KeyW)])
        );

        let overrides = bind(
            &app,
            "override_tests.move.up",
            &[Control::PhysicalKey(KeyCode::KeyI)],
        );
        let problems = apply_overrides(app.world_mut(), &overrides);

        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            filled([Control::PhysicalKey(KeyCode::KeyI)])
        );
        // And only that part of the composite: the other three keys are where they were.
        assert_eq!(
            slots(&app, "override_tests.move.left"),
            filled([Control::PhysicalKey(KeyCode::KeyA)])
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
            &[Control::PhysicalKey(KeyCode::KeyI)],
        );
        apply_overrides(app.world_mut(), &overrides);

        let declared = declared_mappings(app.world())
            .into_iter()
            .find(|mapping| mapping.key.to_string() == "override_tests.move.up")
            .expect("the row is still declared");
        assert_eq!(
            declared.controls(),
            filled([Control::PhysicalKey(KeyCode::KeyW)]),
            "still W"
        );

        // And applying a second time is a diff against the same defaults, not against the first
        // apply — so going back to a row nobody overrode restores the shipped control.
        apply_overrides(app.world_mut(), &Overrides::new());
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            filled([Control::PhysicalKey(KeyCode::KeyW)])
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
        let overrides = bind(
            &app,
            "override_tests.jump",
            &[Control::PhysicalKey(KeyCode::KeyK)],
        );
        apply_overrides(app.world_mut(), &overrides);

        let jump = row(&app, "override_tests.jump");
        assert_eq!(
            jump.controls(),
            filled([Control::PhysicalKey(KeyCode::KeyK)])
        );
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
            Some(Control::PhysicalKey(KeyCode::KeyK))
        );
    }

    /// A slot says everything about what is bound there. Editing its control keeps what it is held
    /// with, a bare control drops it, and a slot can ask for a chord the game never declared.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_slot_binds_the_chord_it_holds() {
        use crate::binding::ModifierKey;
        use crate::present::ControlOrigin;

        #[derive(InputContext)]
        #[context(path = "override_tests.editor", tick = Render)]
        struct Editor;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Editor>(|controls| {
            controls
                .bind::<Jump>(KeyCode::KeyS)
                .with(ModifierKey::Ctrl)
                .mappable();
        });
        let entity = app.world_mut().spawn(Editor).id();
        let ctrl = [ControlOrigin::Modifier(ModifierKey::Ctrl)];
        assert_eq!(chord(&app, "override_tests.jump", 0), ctrl);

        // What a screen does when the player captures Y into the first cell, keeping the Ctrl, and
        // fills the second with U held with Shift.
        let jump = row(&app, "override_tests.jump");
        let mut overrides = Overrides::new();
        let mut edited = overrides.slots_of(&jump);
        edited[0].as_mut().expect("the declared primary").control =
            Control::PhysicalKey(KeyCode::KeyY);
        edited.push(Some(BoundSlot {
            control: Control::PhysicalKey(KeyCode::KeyU),
            with: alloc::vec![ControlOrigin::Modifier(ModifierKey::Shift)],
        }));
        overrides.bind(jump.family, jump.key, edited);
        assert!(apply_overrides(app.world_mut(), &overrides).is_empty());

        assert_eq!(
            slots(&app, "override_tests.jump"),
            filled([
                Control::PhysicalKey(KeyCode::KeyY),
                Control::PhysicalKey(KeyCode::KeyU),
            ])
        );
        assert_eq!(chord(&app, "override_tests.jump", 0), ctrl);
        assert_eq!(
            chord(&app, "override_tests.jump", 1),
            [ControlOrigin::Modifier(ModifierKey::Shift)],
            "the grown slot's own chord, not the primary's"
        );

        // The plan agrees with the row: the new key alone does nothing, and with Ctrl it fires.
        hold(&mut app, &[KeyCode::KeyY]);
        let state = app
            .world()
            .get::<InputContextState<Editor>>(entity)
            .unwrap();
        assert!(!state.value::<Jump>());
        hold(&mut app, &[KeyCode::ControlRight]);
        let state = app
            .world()
            .get::<InputContextState<Editor>>(entity)
            .unwrap();
        assert!(state.value::<Jump>());

        // A bare control is bound on its own.
        let overrides = bind(
            &app,
            "override_tests.jump",
            &[Control::PhysicalKey(KeyCode::KeyY)],
        );
        assert!(apply_overrides(app.world_mut(), &overrides).is_empty());
        assert!(chord(&app, "override_tests.jump", 0).is_empty());
    }

    /// A row whose two bindings require different things held, with the first emptied through
    /// `unbind`, as a screen's clear button does: the survivor keeps its own chord rather than
    /// taking the one from the slot the player cleared.
    #[cfg(feature = "keyboard")]
    #[test]
    fn a_gap_leaves_each_chord_on_its_own_control() {
        #[derive(InputContext)]
        #[context(path = "override_tests.shell", tick = Render)]
        struct ShellKeys;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        // The chord is on the *secondary*, which is what makes this worth asserting: once the
        // primary is gone the row derives one binding, and a chord read off the column it now
        // occupies would come back empty.
        app.add_context::<ShellKeys>(|controls| {
            controls.bind::<Jump>(KeyCode::F2).mappable();
            controls
                .bind::<Jump>(KeyCode::KeyS)
                .with(crate::binding::ModifierKey::Ctrl)
                .mappable();
        });

        // The primary emptied, the secondary left in the column the player can see it in.
        let target = row(&app, "override_tests.jump");
        let mut overrides = Overrides::new();
        overrides.unbind(&target, 0);
        assert!(apply_overrides(app.world_mut(), &overrides).is_empty());

        assert_eq!(
            slots(&app, "override_tests.jump"),
            [None, Some(Control::PhysicalKey(KeyCode::KeyS))]
        );
        assert_eq!(
            chord(&app, "override_tests.jump", 1),
            [crate::present::ControlOrigin::Modifier(
                crate::binding::ModifierKey::Ctrl
            )]
        );
    }

    /// Clearing leaves the row on screen and the action still bound, which is what distinguishes it
    /// from an action nothing binds at all.
    #[test]
    fn a_cleared_row_leaves_the_action_bound_but_silent() {
        let mut app = app();
        let target = row(&app, "override_tests.jump");
        let mut overrides = Overrides::new();
        overrides.set(target.family, target.key, Override::Cleared);
        apply_overrides(app.world_mut(), &overrides);

        // The row is still on the screen, holding nothing — or there would be nowhere to bind it
        // back from.
        let jump = row(&app, "override_tests.jump");
        assert!(jump.slots.is_empty());
        assert_eq!(jump.rebind_policy, RebindPolicy::Here);

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
            &[
                Control::PhysicalKey(KeyCode::Space),
                Control::PhysicalKey(KeyCode::KeyK),
            ],
        );
        apply_overrides(app.world_mut(), &overrides);

        assert_eq!(
            slots(&app, "override_tests.jump"),
            filled([
                Control::PhysicalKey(KeyCode::Space),
                Control::PhysicalKey(KeyCode::KeyK)
            ])
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
                Some(Control::PhysicalKey(KeyCode::Space)),
                Some(Control::PhysicalKey(KeyCode::KeyK))
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
            &[
                Control::PhysicalKey(KeyCode::Space),
                Control::PhysicalKey(KeyCode::KeyK),
            ],
        );
        apply_overrides(app.world_mut(), &grown);

        let shrunk = bind(
            &app,
            "override_tests.jump",
            &[Control::PhysicalKey(KeyCode::KeyK)],
        );
        apply_overrides(app.world_mut(), &shrunk);

        assert_eq!(
            slots(&app, "override_tests.jump"),
            filled([Control::PhysicalKey(KeyCode::KeyK)])
        );
        let prompts = BindingTable::new(app.world()).prompts(Lunge::id(), PromptScope::ANY);
        assert_eq!(
            prompts
                .iter()
                .map(|prompt| prompt.origin.control())
                .collect::<Vec<_>>(),
            [Some(Control::PhysicalKey(KeyCode::KeyK))],
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
            ActionPhase::Fired
        );

        let overrides = bind(
            &app,
            "override_tests.jump",
            &[Control::PhysicalKey(KeyCode::KeyK)],
        );
        apply_overrides(app.world_mut(), &overrides);

        let state = app
            .world()
            .get::<InputContextState<Playing>>(entity)
            .unwrap();
        assert_eq!(state.phase::<Jump>(), ActionPhase::Canceled);
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
        let overrides = bind(
            &app,
            "override_tests.jump",
            &[Control::PhysicalKey(KeyCode::KeyK)],
        );
        apply_overrides(app.world_mut(), &overrides);

        app.world_mut().spawn(Playing);
        app.update();

        let prompts = BindingTable::new(app.world()).prompts(Jump::id(), PromptScope::ANY);
        assert_eq!(prompts.len(), 1);
        assert_eq!(
            prompts[0].origin.control(),
            Some(Control::PhysicalKey(KeyCode::KeyK))
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

        let overrides = bind(
            &app,
            "override_tests.jump",
            &[Control::PhysicalKey(KeyCode::KeyK)],
        );
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
            ActionPhase::Fired,
            "the named entity was remapped to K"
        );
        assert_eq!(
            app.world()
                .get::<InputContextState<Playing>>(player_b)
                .unwrap()
                .phase::<Jump>(),
            ActionPhase::Idle,
            "a sibling instance never asked for K and is still listening on Space"
        );
        assert_eq!(
            app.world()
                .get::<InputContextState<Playing>>(player_c)
                .unwrap()
                .phase::<Jump>(),
            ActionPhase::Idle,
            "spawned after the apply, and still the world's unmodified default"
        );

        // The world-wide declared/current tables read back unchanged: no other entity, and no
        // future one, ever sees Jump listed on anything but Space.
        assert_eq!(
            slots(&app, "override_tests.jump"),
            filled([Control::PhysicalKey(KeyCode::Space)])
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

        let overrides = bind(
            &app,
            "override_tests.jump",
            &[Control::PhysicalKey(KeyCode::KeyK)],
        );
        apply_overrides(app.world_mut(), &overrides);

        let after = app
            .world()
            .get_resource::<crate::present::PromptGeneration>()
            .map_or(0, |generation| generation.0);
        assert!(after > before, "a rebind said nothing about prompts");
    }

    /// Four ways a row can fail, in one apply: each reported, and each refused whole rather than
    /// half-applied.
    #[test]
    fn every_unusable_row_is_reported_rather_than_dropped() {
        let mut app = app();
        let jump = row(&app, "override_tests.jump");
        let look = row(&app, "override_tests.look");
        let gone = MappingKey::new(
            "override_tests.no_such_action",
            crate::binding::BindingPart::Whole,
        );

        let mut overrides = Overrides::new();
        overrides.bind(
            DeviceFamily::KeyboardMouse,
            gone,
            [Control::PhysicalKey(KeyCode::KeyZ)],
        );
        overrides.bind(look.family, look.key, [Control::MouseMotion]);
        overrides.bind(jump.family, jump.key, [Control::PhysicalKey(KeyCode::F1)]);

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
            control: Control::PhysicalKey(KeyCode::F1)
        }));

        // Refused whole, never half: the row still holds what it shipped with.
        assert_eq!(
            slots(&app, "override_tests.jump"),
            filled([Control::PhysicalKey(KeyCode::Space)])
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

    /// A chord waits on something being held, so an entry with no pressed state has nothing to wait
    /// on — and a control only a backend knows cannot be read here at all.
    #[test]
    fn a_chord_entry_a_player_cannot_hold_is_refused() {
        use crate::present::ControlOrigin;

        let mut app = app();
        let jump = row(&app, "override_tests.jump");
        let foreign = ControlOrigin::Foreign {
            name: "steam/left_grip".to_string(),
            label: "Left Grip".to_string(),
            family: None,
            class: None,
            glyph: None,
        };

        for entry in [ControlOrigin::Ours(Control::MouseMotion), foreign] {
            let mut overrides = Overrides::new();
            overrides.bind(
                jump.family,
                jump.key,
                [BoundSlot {
                    control: Control::PhysicalKey(KeyCode::KeyK),
                    with: alloc::vec![entry.clone()],
                }],
            );
            let problems = apply_overrides(app.world_mut(), &overrides);

            assert_eq!(
                problems
                    .iter()
                    .map(|problem| problem.kind.clone())
                    .collect::<Vec<_>>(),
                [OverrideProblemKind::NotChordable { entry }]
            );
            assert_eq!(
                slots(&app, "override_tests.jump"),
                filled([Control::PhysicalKey(KeyCode::Space)]),
                "refused whole"
            );
        }
    }

    /// A rider reads what its leader reads, and that now includes what the leader is held with:
    /// otherwise `Lunge` would fire on a bare K while `Jump` waited for Ctrl.
    #[test]
    fn a_follower_takes_the_chord_its_row_was_given() {
        use crate::binding::ModifierKey;
        use crate::present::ControlOrigin;

        let mut app = app();
        app.world_mut().spawn(Playing);
        let jump = row(&app, "override_tests.jump");
        let ctrl_k = BoundSlot {
            control: Control::PhysicalKey(KeyCode::KeyK),
            with: alloc::vec![ControlOrigin::Modifier(ModifierKey::Ctrl)],
        };
        let mut overrides = Overrides::new();
        overrides.bind(jump.family, jump.key, [ctrl_k.clone()]);
        assert!(apply_overrides(app.world_mut(), &overrides).is_empty());

        let prompts = BindingTable::new(app.world()).prompts(Lunge::id(), PromptScope::ANY);
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].origin, ControlOrigin::Ours(ctrl_k.control));
        assert_eq!(prompts[0].with, ctrl_k.with);
    }

    /// A southpaw preset swaps the two sticks, the canonical thing a preset is for.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_southpaw_preset_swaps_the_sticks() {
        use crate::binding::Stick;

        #[derive(InputAction)]
        #[action(path = "override_tests.stick.move", output = bevy_math::Vec2, intent = Directional2)]
        struct StickMove;

        #[derive(InputAction)]
        #[action(path = "override_tests.stick.look", output = bevy_math::Vec2, intent = Directional2)]
        struct StickLook;

        #[derive(InputContext)]
        #[context(path = "override_tests.sticks", tick = Render)]
        struct WithSticks;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<WithSticks>(|controls| {
            controls.bind::<StickMove>(Stick::Left).mappable();
            controls.bind::<StickLook>(Stick::Right).mappable();
        });

        let mut southpaw = Overrides::new();
        let move_row = row(&app, "override_tests.stick.move");
        let look_row = row(&app, "override_tests.stick.look");
        southpaw.bind(
            move_row.family,
            move_row.key,
            [Control::GamepadStick(Stick::Right)],
        );
        southpaw.bind(
            look_row.family,
            look_row.key,
            [Control::GamepadStick(Stick::Left)],
        );

        let problems = apply_overrides(app.world_mut(), &southpaw);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&app, "override_tests.stick.move"),
            filled([Control::GamepadStick(Stick::Right)])
        );
        assert_eq!(
            slots(&app, "override_tests.stick.look"),
            filled([Control::GamepadStick(Stick::Left)])
        );
    }

    /// Resetting works at each of the four granularities: one row, one action, one context, or
    /// everything.
    #[test]
    fn resetting_puts_a_row_back_to_what_the_game_declared() {
        let mut app = app();
        let rows = mappings(app.world());
        let mut overrides = Overrides::new();
        for target in &rows {
            if target.rebind_policy.is_rebindable() {
                overrides.bind(
                    target.family,
                    target.key,
                    [Control::PhysicalKey(KeyCode::KeyZ)],
                );
            }
        }

        // One row.
        let up = row(&app, "override_tests.move.up");
        overrides.reset(up.family, up.key);
        assert!(overrides.get(up.family, up.key).is_none());

        // Every row of one action, which for a composite is all four directions.
        overrides.reset_action(&rows, Move::id());
        assert!(
            !rows
                .iter()
                .filter(|r| r.action == Move::id())
                .any(|r| { overrides.get(r.family, r.key).is_some() })
        );

        // Every row of one context, and then the lot.
        overrides.reset_context(&rows, "override_tests.playing");
        assert!(overrides.is_empty());

        overrides.bind(up.family, up.key, [Control::PhysicalKey(KeyCode::KeyZ)]);
        overrides.reset_all();
        assert!(overrides.is_empty());

        apply_overrides(app.world_mut(), &overrides);
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            filled([Control::PhysicalKey(KeyCode::KeyW)])
        );
    }

    /// A preset moves a `Fixed` row a capture cannot, exempting exactly the rows it names from
    /// `NotRebindable` and nothing else.
    #[test]
    fn a_preset_moves_a_fixed_row_a_capture_cannot() {
        let mut app = app();
        let target = row(&app, "override_tests.settings");
        assert_eq!(target.rebind_policy, RebindPolicy::Fixed);

        let mut preset = Overrides::new();
        preset.bind(
            target.family,
            target.key,
            [Control::PhysicalKey(KeyCode::F2)],
        );

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
            filled([Control::PhysicalKey(KeyCode::F1)])
        );

        // The same row moves once the same rows are named as the preset authorizing it.
        let problems = apply_overrides_with_preset(app.world_mut(), &preset, &preset);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&app, "override_tests.settings"),
            filled([Control::PhysicalKey(KeyCode::F2)])
        );
    }

    /// A row an authority owns takes nothing from this crate, whether it arrives from a screen, a
    /// preset or a file, and the answer says why rather than calling it fixed.
    #[test]
    fn a_delegated_row_refuses_every_write() {
        #[derive(InputContext)]
        #[context(path = "override_tests.split", tick = Render)]
        struct Split;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Split>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(crate::backend::Authority(DeviceFamily::Gamepad));
        });
        let pad = mappings(app.world())
            .into_iter()
            .find(|mapping| mapping.family == DeviceFamily::Gamepad)
            .unwrap();
        assert_eq!(pad.rebind_policy, RebindPolicy::Delegated);
        // The family is checked after delegation, so a key is refused for the same reason a pad
        // button would be.
        let key = || alloc::vec![Some(BoundSlot::from(Control::PhysicalKey(KeyCode::KeyJ)))];

        let refused = Rebind::checked(app.world(), &pad, key());
        assert_eq!(refused.unwrap_err(), OverrideProblemKind::Delegated);

        // A preset does not exempt it, as it would a fixed row.
        let mut preset = Overrides::new();
        preset.bind(pad.family, pad.key, [Control::PhysicalKey(KeyCode::KeyJ)]);
        let refused = Rebind::checked_with_preset(app.world(), &preset, &pad, key());
        assert_eq!(refused.unwrap_err(), OverrideProblemKind::Delegated);

        // Clearing is a write too.
        let mut overrides = Overrides::new();
        overrides.set(pad.family, pad.key, Override::Cleared);
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert_eq!(
            problems
                .iter()
                .map(|problem| problem.kind.clone())
                .collect::<Vec<_>>(),
            [OverrideProblemKind::Delegated]
        );
        assert!(mappings(app.world()).contains(&pad));

        // Holding nothing, it is never what a capture clashes with, whichever button the authority
        // happens to have put it on.
        #[cfg(feature = "gamepad")]
        {
            let south = BoundSlot::from(Control::GamepadButton(
                bevy_input::gamepad::GamepadButton::South,
            ));
            assert!(crate::capture::conflicts(app.world(), &south, None).is_empty());
        }
    }

    /// The point of the whole container: emptying the primary of a two-control row leaves the gap
    /// where it was. Position is what primary and secondary mean, so a secondary that slid up into
    /// the column the player just cleared would be a different binding than the one they asked for.
    #[test]
    fn emptying_a_slot_leaves_a_gap_rather_than_promoting_what_follows() {
        let mut app = app();
        let jump = row(&app, "override_tests.jump");

        let mut overrides = Overrides::new();
        overrides.bind(
            jump.family,
            jump.key,
            [
                Control::PhysicalKey(KeyCode::Space),
                Control::PhysicalKey(KeyCode::KeyJ),
            ],
        );
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty(), "{problems:?}");

        // What a screen writes when the player clears the first cell of that row.
        overrides.bind(
            jump.family,
            jump.key,
            [
                None,
                Some(BoundSlot::from(Control::PhysicalKey(KeyCode::KeyJ))),
            ],
        );
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty(), "{problems:?}");

        assert_eq!(
            slots(&app, "override_tests.jump"),
            [None, Some(Control::PhysicalKey(KeyCode::KeyJ))],
            "the secondary is still the secondary"
        );

        // And the plan agrees with the row, which is the half taking the shape from the override
        // rather than from the rewritten bindings could get wrong: the emptied primary's binding is
        // gone, and the secondary's is the one still reading.
        use bevy_input::{ButtonState, keyboard::Key, keyboard::KeyboardInput};

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
        assert!(
            !app.world()
                .get::<InputContextState<Playing>>(entity)
                .unwrap()
                .value::<Jump>(),
            "the control the player cleared fires nothing"
        );

        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyJ,
            logical_key: Key::Character("j".into()),
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        assert!(
            app.world()
                .get::<InputContextState<Playing>>(entity)
                .unwrap()
                .value::<Jump>(),
            "and the one in the second column still does"
        );
    }

    /// What a "clear this cell" button does, across the three shapes a row can normalize to: a hole
    /// where something still follows it, a shorter row where nothing does, and `Cleared` where the
    /// row held one thing.
    #[test]
    fn unbinding_one_slot_empties_it_in_place() {
        let mut app = app();
        let jump = row(&app, "override_tests.jump");
        let space = Control::PhysicalKey(KeyCode::Space);
        let j = Control::PhysicalKey(KeyCode::KeyJ);

        let mut overrides = Overrides::new();
        overrides.bind(jump.family, jump.key, [space, j]);

        // The primary, with the secondary still there: a hole, and the secondary stays second.
        overrides.unbind(&jump, 0);
        assert_eq!(
            overrides.get(jump.family, jump.key),
            Some(&Override::Slots(alloc::vec![None, Some(j.into())]))
        );

        // And the secondary of that same row, which leaves nothing at all.
        overrides.unbind(&jump, 1);
        assert_eq!(
            overrides.get(jump.family, jump.key),
            Some(&Override::Cleared),
            "a row with nothing left is the state that already means that"
        );

        // Reading through to the declared row rather than only what this set holds: nothing has
        // been said about `move.up`, so clearing its only slot clears what the game shipped.
        let up = row(&app, "override_tests.move.up");
        assert_eq!(up.slots.len(), 1);
        overrides.unbind(&up, 0);
        assert_eq!(overrides.get(up.family, up.key), Some(&Override::Cleared));

        // A slot the row does not reach is already empty.
        overrides.unbind(&jump, 9);
        assert_eq!(
            overrides.get(jump.family, jump.key),
            Some(&Override::Cleared),
            "clearing past the end changes nothing"
        );

        // And the whole thing applies, one direction of the movement keys included.
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(slots(&app, "override_tests.jump").is_empty());
        assert!(slots(&app, "override_tests.move.up").is_empty());
    }

    /// A slot is addressed, not appended: writing to the third cell of a one-control row gives a
    /// row of three with a blank in the middle, rather than two controls side by side.
    #[test]
    fn a_sparse_row_applies_and_keeps_its_hole() {
        let mut app = app();
        let jump = row(&app, "override_tests.jump");

        // What a screen writes when the player fills the third cell of a row holding one control:
        // the cell they pressed, not the next one free.
        let mut overrides = Overrides::new();
        overrides.bind(
            jump.family,
            jump.key,
            [
                Some(BoundSlot::from(Control::PhysicalKey(KeyCode::Space))),
                None,
                Some(BoundSlot::from(Control::PhysicalKey(KeyCode::KeyL))),
            ],
        );
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty(), "{problems:?}");

        assert_eq!(
            slots(&app, "override_tests.jump"),
            [
                Some(Control::PhysicalKey(KeyCode::Space)),
                None,
                Some(Control::PhysicalKey(KeyCode::KeyL))
            ],
            "three slots with the middle one empty, not two controls side by side"
        );
    }

    /// The two ways a screen writes a row back, which is the whole of what an editor-style growable
    /// list needs over a fixed-column table: one keeps the holes, the other closes them.
    #[test]
    fn a_row_can_be_written_back_with_its_gaps_closed() {
        let app = app();
        let jump = row(&app, "override_tests.jump");
        let row_with_a_hole = alloc::vec![
            None,
            Some(BoundSlot::from(Control::PhysicalKey(KeyCode::KeyJ)))
        ];

        let mut overrides = Overrides::new();
        overrides.bind(jump.family, jump.key, row_with_a_hole.clone());
        assert_eq!(
            overrides.get(jump.family, jump.key),
            Some(&Override::Slots(row_with_a_hole.clone())),
            "a table means something by which column a control is in"
        );

        overrides.bind(jump.family, jump.key, row_with_a_hole.into_iter().flatten());
        assert_eq!(
            overrides.get(jump.family, jump.key),
            Some(&Override::Slots(alloc::vec![Some(
                Control::PhysicalKey(KeyCode::KeyJ).into()
            )])),
            "a list does not, and closes the gap on the way in"
        );
    }

    /// Trailing empties are not a row's business: a row is as long as its last filled slot, and how
    /// many cells to draw past that is the screen's decision rather than something a save carries.
    #[test]
    fn a_row_normalizes_to_its_last_filled_slot() {
        let app = app();
        let jump = row(&app, "override_tests.jump");
        let space = BoundSlot::from(Control::PhysicalKey(KeyCode::Space));

        let mut overrides = Overrides::new();
        overrides.bind(jump.family, jump.key, [Some(space.clone()), None]);
        assert_eq!(
            overrides.get(jump.family, jump.key),
            Some(&Override::Slots(alloc::vec![Some(space)])),
            "a blank second cell is not something to write down"
        );

        // And a row with nothing left in it is the state that already means that.
        overrides.bind(jump.family, jump.key, [None, None]);
        assert_eq!(
            overrides.get(jump.family, jump.key),
            Some(&Override::Cleared)
        );
    }

    /// One end of a turn axis empties on its own. Each end is a binding of its own, so the other
    /// end, and the other composite's same end, are untouched.
    #[test]
    fn one_end_of_an_axis_empties_on_its_own() {
        #[derive(InputAction)]
        #[action(path = "override_tests.turn", output = f32, intent = Analog1)]
        struct Turn;

        #[derive(InputContext)]
        #[context(path = "override_tests.flying", tick = Render)]
        struct Flying;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Flying>(|controls| {
            controls
                .bind::<Turn>(crate::binding::AxisButtons::ad())
                .mappable();
            controls
                .bind::<Turn>(crate::binding::AxisButtons::left_right())
                .mappable();
        });

        let left = row(&app, "override_tests.turn.negative");
        assert_eq!(
            left.controls(),
            filled([
                Control::PhysicalKey(KeyCode::KeyA),
                Control::PhysicalKey(KeyCode::ArrowLeft)
            ]),
            "one row with both composites' negative ends in it"
        );

        // Clearing the primary, which is what the screen's clear gesture writes.
        let mut overrides = Overrides::new();
        overrides.unbind(&left, 0);
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&app, "override_tests.turn.negative"),
            [None, Some(Control::PhysicalKey(KeyCode::ArrowLeft))]
        );
        assert_eq!(
            slots(&app, "override_tests.turn.positive"),
            filled([
                Control::PhysicalKey(KeyCode::KeyD),
                Control::PhysicalKey(KeyCode::ArrowRight)
            ]),
            "the other end of the axis is untouched"
        );

        let entity = app.world_mut().spawn(Flying).id();
        hold(&mut app, &[KeyCode::KeyA]);
        let state = app
            .world()
            .get::<InputContextState<Flying>>(entity)
            .unwrap();
        assert_eq!(state.value::<Turn>(), 0.0, "A turns nothing");
        hold(&mut app, &[KeyCode::KeyD]);
        let state = app
            .world()
            .get::<InputContextState<Flying>>(entity)
            .unwrap();
        assert_eq!(
            state.value::<Turn>(),
            1.0,
            "and D, its other end, still turns"
        );
    }

    /// One direction of a composite empties on its own, and stays on the screen holding nothing so
    /// there is somewhere to bind it back.
    #[test]
    fn one_direction_of_a_composite_empties_on_its_own() {
        let mut app = app();
        let up = row(&app, "override_tests.move.up");

        let mut overrides = Overrides::new();
        overrides.set(up.family, up.key, Override::Cleared);
        let problems = apply_overrides(app.world_mut(), &overrides);

        assert!(problems.is_empty(), "{problems:?}");
        assert!(slots(&app, "override_tests.move.up").is_empty());
        assert_eq!(
            slots(&app, "override_tests.move.down"),
            filled([Control::PhysicalKey(KeyCode::KeyS)]),
            "and the other three did not empty with it"
        );

        let entity = app.world_mut().spawn(Playing).id();
        hold(&mut app, &[KeyCode::KeyW, KeyCode::KeyD]);
        let state = app
            .world()
            .get::<InputContextState<Playing>>(entity)
            .unwrap();
        assert_eq!(
            state.value::<Move>(),
            bevy_math::Vec2::X,
            "W moves nothing, and D still moves right"
        );
    }

    /// One direction of a composite takes a second control on its own, and the new control drives
    /// that direction.
    #[test]
    fn one_direction_of_a_composite_grows_a_slot_on_its_own() {
        let mut app = app();
        let up = row(&app, "override_tests.move.up");
        let mut overrides = Overrides::new();
        overrides.bind(
            up.family,
            up.key,
            [
                Control::PhysicalKey(KeyCode::KeyW),
                Control::PhysicalKey(KeyCode::KeyI),
            ],
        );
        let problems = apply_overrides(app.world_mut(), &overrides);

        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            filled([
                Control::PhysicalKey(KeyCode::KeyW),
                Control::PhysicalKey(KeyCode::KeyI)
            ])
        );
        assert_eq!(
            slots(&app, "override_tests.move.down"),
            filled([Control::PhysicalKey(KeyCode::KeyS)]),
            "and the other three directions did not grow with it"
        );

        let entity = app.world_mut().spawn(Playing).id();
        hold(&mut app, &[KeyCode::KeyI]);
        let state = app
            .world()
            .get::<InputContextState<Playing>>(entity)
            .unwrap();
        assert_eq!(state.value::<Move>(), bevy_math::Vec2::Y);
    }

    /// Mapping keys name a part, as they did when a composite was one binding, so a save written
    /// then lands on the same directions now.
    #[test]
    fn a_save_naming_composite_parts_applies_to_their_bindings() {
        let mut app = app();
        let up = row(&app, "override_tests.move.up");
        let left = row(&app, "override_tests.move.left");
        let mut overrides = Overrides::new();
        overrides.bind(up.family, up.key, [Control::PhysicalKey(KeyCode::KeyI)]);
        overrides.bind(left.family, left.key, [Control::PhysicalKey(KeyCode::KeyJ)]);
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty(), "{problems:?}");

        let entity = app.world_mut().spawn(Playing).id();
        hold(&mut app, &[KeyCode::KeyI, KeyCode::KeyJ]);
        let state = app
            .world()
            .get::<InputContextState<Playing>>(entity)
            .unwrap();
        assert_eq!(state.value::<Move>(), bevy_math::Vec2::new(-1.0, 1.0));
    }

    /// A second composite is how a game ships a two-column movement table, and each direction then
    /// rebinds its own secondary independently.
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
            filled([
                Control::PhysicalKey(KeyCode::KeyW),
                Control::PhysicalKey(KeyCode::ArrowUp)
            ])
        );

        let up = row(&app, "override_tests.move.up");
        let mut overrides = Overrides::new();
        overrides.bind(
            up.family,
            up.key,
            [
                Control::PhysicalKey(KeyCode::KeyW),
                Control::PhysicalKey(KeyCode::KeyI),
            ],
        );
        let problems = apply_overrides(app.world_mut(), &overrides);

        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&app, "override_tests.move.up"),
            filled([
                Control::PhysicalKey(KeyCode::KeyW),
                Control::PhysicalKey(KeyCode::KeyI)
            ]),
            "the secondary moved and the primary did not"
        );
        assert_eq!(
            slots(&app, "override_tests.move.down"),
            filled([
                Control::PhysicalKey(KeyCode::KeyS),
                Control::PhysicalKey(KeyCode::ArrowDown)
            ]),
            "and the other rows kept both of theirs"
        );
    }

    /// A row is as long as the file says unless the game set a ceiling, and the ceiling is the only
    /// thing that refuses one for its length.
    #[test]
    fn a_row_is_refused_for_its_length_only_against_a_ceiling() {
        let long = [
            Control::PhysicalKey(KeyCode::Space),
            Control::PhysicalKey(KeyCode::KeyK),
            Control::PhysicalKey(KeyCode::KeyL),
        ];

        let mut unbounded = app();
        let overrides = bind(&unbounded, "override_tests.jump", &long);
        let problems = apply_overrides(unbounded.world_mut(), &overrides);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            slots(&unbounded, "override_tests.jump"),
            filled(long),
            "a game that set no ceiling reads whatever its own file says"
        );

        let mut bounded = app();
        bounded.insert_resource(MaxSlots(2));
        let problems = apply_overrides(bounded.world_mut(), &overrides);
        assert_eq!(
            problems
                .iter()
                .map(|problem| problem.kind.clone())
                .collect::<Vec<_>>(),
            [OverrideProblemKind::TooManyControls { limit: 2, given: 3 }]
        );
        assert_eq!(
            slots(&bounded, "override_tests.jump"),
            filled([Control::PhysicalKey(KeyCode::Space)]),
            "refused whole, so the row still holds what the game shipped"
        );
    }

    /// A screen can draw a cell past the ceiling its own game set, so the row that cell would make
    /// has to be refused. `Rebind::checked` turns it down where the screen can say so, and applying
    /// turns down the same row loaded from a file.
    #[test]
    fn a_row_past_the_ceiling_is_refused_where_it_lands() {
        let mut app = app();
        app.insert_resource(MaxSlots(2));
        let jump = row(&app, "override_tests.jump");

        // Filling slot 2 makes a row of three, which is what the ceiling refuses.
        let mut overrides = Overrides::new();
        let wanted = overrides.with_cell(&jump, 2, Control::PhysicalKey(KeyCode::KeyL));
        assert_eq!(
            Rebind::checked(app.world(), &jump, wanted.clone()).err(),
            Some(OverrideProblemKind::TooManyControls { limit: 2, given: 3 }),
            "the screen hears it before the player does"
        );

        overrides.bind(jump.family, jump.key, wanted);
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert_eq!(
            problems
                .iter()
                .map(|problem| problem.kind.clone())
                .collect::<Vec<_>>(),
            [OverrideProblemKind::TooManyControls { limit: 2, given: 3 }],
            "a hole counts toward the length, since it is a column the row reaches"
        );
    }

    /// The claim the whole arrangement rests on: a screen asking before it writes and a save file
    /// judged as it is applied get the same answer for the same row. Driven over the refusals
    /// rather than asserted on one, because "they agree" is the property, not one case of it.
    #[test]
    fn what_a_screen_is_told_is_what_applying_would_say() {
        let mut app = app();
        app.insert_resource(MaxSlots(2));
        let jump = row(&app, "override_tests.jump");
        let settings = row(&app, "override_tests.settings");

        // The expected answer is spelled out as well as compared: a case both sides happen to
        // accept would otherwise agree with itself and pass while proving neither.
        let cases: [(&str, &ActionMapping, Vec<Option<BoundSlot>>, _); 5] = [
            (
                "a control reserved for the settings key",
                &jump,
                alloc::vec![Some(BoundSlot::from(Control::PhysicalKey(KeyCode::F1)))],
                Some(OverrideProblemKind::Reserved {
                    control: Control::PhysicalKey(KeyCode::F1),
                }),
            ),
            (
                "a row the game marked fixed",
                &settings,
                alloc::vec![Some(BoundSlot::from(Control::PhysicalKey(KeyCode::KeyG)))],
                Some(OverrideProblemKind::NotRebindable),
            ),
            (
                "mouse motion where the row drives a button",
                &jump,
                alloc::vec![Some(BoundSlot::from(Control::MouseMotion))],
                Some(OverrideProblemKind::WrongShape {
                    control: Control::MouseMotion,
                    accepts: ChannelShape::Button,
                }),
            ),
            (
                "a row grown past the ceiling",
                &jump,
                jump_row_of_three(),
                Some(OverrideProblemKind::TooManyControls { limit: 2, given: 3 }),
            ),
            (
                "and one that is simply fine",
                &jump,
                alloc::vec![Some(BoundSlot::from(Control::PhysicalKey(KeyCode::KeyG)))],
                None,
            ),
        ];

        for (what, target, wanted, expected) in cases {
            let told = Rebind::checked(app.world(), target, wanted.clone()).err();

            let mut overrides = Overrides::new();
            overrides.bind(target.family, target.key, wanted);
            let applied = apply_overrides(app.world_mut(), &overrides)
                .into_iter()
                .next()
                .map(|problem| problem.kind);

            assert_eq!(told, expected, "{what}: what the screen is told");
            assert_eq!(applied, expected, "{what}: what applying says");
            // Put the defaults back, so the next case is judged against the same world.
            apply_overrides(app.world_mut(), &Overrides::new());
        }
    }

    /// A three-column row for the ceiling case, with the hole that makes it three.
    fn jump_row_of_three() -> Vec<Option<BoundSlot>> {
        alloc::vec![
            Some(BoundSlot::from(Control::PhysicalKey(KeyCode::Space))),
            None,
            Some(BoundSlot::from(Control::PhysicalKey(KeyCode::KeyL))),
        ]
    }

    /// A preset moves rows a rebinding screen never offers a button for, so a screen that offers
    /// presets must not be told a preset's own row is unchangeable. Only the rows it names, though.
    #[test]
    fn a_preset_exempts_the_rows_it_names_and_no_others() {
        let app = app();
        let settings = row(&app, "override_tests.settings");
        assert_eq!(settings.rebind_policy, RebindPolicy::Fixed);

        let moved = alloc::vec![Some(BoundSlot::from(Control::PhysicalKey(KeyCode::F2)))];
        let mut preset = Overrides::new();
        preset.bind(settings.family, settings.key, moved.clone());

        assert!(
            Rebind::checked_with_preset(app.world(), &preset, &settings, moved.clone()).is_ok(),
            "the preset names this row, so it may move"
        );
        assert_eq!(
            Rebind::checked_with_preset(app.world(), &Overrides::new(), &settings, moved.clone())
                .err(),
            Some(OverrideProblemKind::NotRebindable),
            "no preset selected, so the fixed row stands"
        );
        assert_eq!(
            Rebind::checked(app.world(), &settings, moved).err(),
            Some(OverrideProblemKind::NotRebindable),
            "and a manual capture never moves one"
        );
    }

    /// The cell the player pressed is the cell that gets the control, so a row grows to reach it
    /// and the columns skipped on the way stay empty — a screen offers whatever cells it draws
    /// without first asking how long the row is. A filled cell keeps what it was held with.
    #[test]
    fn a_cell_is_addressed_and_the_row_grows_to_reach_it() {
        let app = app();
        let jump = row(&app, "override_tests.jump");
        assert_eq!(jump.slots.len(), 1, "one default");

        let overrides = Overrides::new();
        let grown = overrides.with_cell(&jump, 2, Control::PhysicalKey(KeyCode::KeyL));
        assert_eq!(
            grown
                .iter()
                .map(|slot| slot.as_ref().map(|slot| slot.control))
                .collect::<Vec<_>>(),
            [
                Some(Control::PhysicalKey(KeyCode::Space)),
                None,
                Some(Control::PhysicalKey(KeyCode::KeyL)),
            ],
            "a row of three with a blank in the middle, not a row of two"
        );

        // A chord is the cell's, not the control's: retyping the control of a `Ctrl+S` cell leaves
        // the `Ctrl` where it was.
        let mut held = Overrides::new();
        held.bind(
            jump.family,
            jump.key,
            [Some(BoundSlot {
                control: Control::PhysicalKey(KeyCode::KeyS),
                with: alloc::vec![ControlOrigin::Ours(Control::PhysicalKey(
                    KeyCode::ControlLeft
                ))],
            })],
        );
        let retyped = held.with_cell(&jump, 0, Control::PhysicalKey(KeyCode::KeyD));
        assert_eq!(
            retyped[0]
                .as_ref()
                .map(|slot| (slot.control, slot.with.len())),
            Some((Control::PhysicalKey(KeyCode::KeyD), 1))
        );
    }

    /// A key match alone must not move an override across families: `hold_or_toggle` reaching both
    /// a keyboard and a gamepad binding under one name declares two independent tunables, one per
    /// family's own table, not one shared across devices.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_tunable_override_does_not_cross_families() {
        use bevy_input::gamepad::GamepadButton;

        #[derive(InputAction)]
        #[action(path = "override_tests.thrust", output = bool, intent = Button)]
        struct Thrust;

        #[derive(InputContext)]
        #[context(path = "override_tests.cross_family", tick = Render)]
        struct CrossFamily;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<CrossFamily>(|controls| {
            controls.bind::<Thrust>(KeyCode::Space);
            controls.bind::<Thrust>(GamepadButton::South);
            controls.hold_or_toggle::<Thrust>("override_tests.thrust.hold_or_toggle");
        });

        let mut overrides = Overrides::new();
        overrides.tune(
            DeviceFamily::KeyboardMouse,
            "override_tests.thrust.hold_or_toggle",
            TunableValue::Bool(true),
        );
        let problems = apply_overrides(app.world_mut(), &overrides);
        assert!(problems.is_empty(), "{problems:?}");

        let tunables = crate::mapping::tunables(app.world());
        let keyboard = tunables
            .iter()
            .find(|t| t.family == DeviceFamily::KeyboardMouse)
            .expect("a keyboard row");
        let gamepad = tunables
            .iter()
            .find(|t| t.family == DeviceFamily::Gamepad)
            .expect("a gamepad row");
        assert_eq!(keyboard.value, TunableValue::Bool(true));
        assert_eq!(
            gamepad.value,
            TunableValue::Bool(false),
            "the gamepad row must not have moved"
        );
    }

    /// `SavedOverrides`'s reflect-driven wire shape, pinned by a golden document rather than by an
    /// intention nobody rechecks, and what `resolve_saved`/`save_overrides` do with it once loaded.
    #[cfg(all(feature = "gamepad", feature = "serialize"))]
    mod persistence {
        use super::*;

        use bevy_input::gamepad::GamepadButton;
        use bevy_reflect::FromReflect;
        use bevy_reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
        use serde::de::DeserializeSeed;

        #[derive(InputAction)]
        #[action(path = "persist_tests.move", output = bevy_math::Vec2, intent = Directional2)]
        struct Move;

        #[derive(InputAction)]
        #[action(path = "persist_tests.jump", output = bool, intent = Button)]
        struct Jump;

        #[derive(InputContext)]
        #[context(path = "persist_tests.playing", tick = Render)]
        struct Playing;

        /// `Move` on WASD (four keyboard rows, none of them overridden below), `Jump` on Space and
        /// on the pad's South button.
        fn declared() -> Vec<ActionMapping> {
            let mut app = App::new();
            app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
            app.add_context::<Playing>(|controls| {
                controls.bind::<Move>(DirectionalButtons::wasd()).mappable();
                controls.bind::<Jump>(KeyCode::Space).mappable();
                controls.bind::<Jump>(GamepadButton::South).mappable();
            });
            declared_mappings(app.world())
        }

        fn mapping_key(declared: &[ActionMapping], family: DeviceFamily, name: &str) -> MappingKey {
            declared
                .iter()
                .find(|mapping| mapping.family == family && mapping.key.to_string() == name)
                .unwrap_or_else(|| panic!("no mapping named {name} in {family:?}"))
                .key
        }

        /// The registry a running app actually has, built by `ActionMapPlugin` rather than by
        /// naming types here.
        ///
        /// Hand-registering them is what hid a real bug: the fixture named eight types by hand,
        /// two of which no running app had, and every saved binding was dropped on load with no
        /// diagnostic anywhere while these tests passed. A fixture that can be more complete than
        /// the plugin is a fixture that can pass while the crate is broken.
        fn types() -> bevy_reflect::TypeRegistryArc {
            let mut app = App::new();
            app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
            app.world()
                .resource::<bevy_ecs::reflect::AppTypeRegistry>()
                .0
                .clone()
        }

        const GOLDEN: &str = "action_map_version = 1\n\
            \n\
            [bindings.gamepad]\n\
            \"persist_tests.jump\" = \"cleared\"\n\
            \n\
            [bindings.keyboard_mouse]\n\
            \"persist_tests.jump\" = [\"key/Space\", \"key/KeyJ\"]\n\
            \"persist_tests.move.up\" = \"key/KeyI\"\n\
            \n\
            [tunables]\n";

        /// What [`save_overrides`] writes is a document a person would be willing to write by hand,
        /// and reading it back through `bevy_reflect` — the way a `Reflect`-based settings layer
        /// actually would — produces the identical value: a scalar for the row that holds one
        /// control, a list for the row that holds two, and a word for an emptied row that could
        /// never be mistaken for a control name.
        #[test]
        fn a_saved_override_set_round_trips_through_reflect() {
            let declared = declared();
            let registry = types();
            let types = registry.read();
            let mut overrides = Overrides::new();
            overrides.bind(
                DeviceFamily::KeyboardMouse,
                mapping_key(
                    &declared,
                    DeviceFamily::KeyboardMouse,
                    "persist_tests.move.up",
                ),
                [Control::PhysicalKey(KeyCode::KeyI)],
            );
            overrides.bind(
                DeviceFamily::KeyboardMouse,
                mapping_key(&declared, DeviceFamily::KeyboardMouse, "persist_tests.jump"),
                [
                    Control::PhysicalKey(KeyCode::Space),
                    Control::PhysicalKey(KeyCode::KeyJ),
                ],
            );
            overrides.set(
                DeviceFamily::Gamepad,
                mapping_key(&declared, DeviceFamily::Gamepad, "persist_tests.jump"),
                Override::Cleared,
            );

            let saved = save_overrides(&overrides);
            let serializer = TypedReflectSerializer::new(&saved, &types);
            let text = toml::to_string(&serializer).expect("serializes");
            assert_eq!(text, GOLDEN);

            let registration = types
                .get(core::any::TypeId::of::<SavedOverrides>())
                .unwrap();
            let value: toml::Value = toml::from_str(&text).expect("parses");
            let reflected = TypedReflectDeserializer::new(registration, &types)
                .deserialize(value)
                .expect("deserializes");
            let loaded_saved =
                <SavedOverrides as FromReflect>::from_reflect(&*reflected).expect("round-trips");
            assert_eq!(loaded_saved, saved);

            let (loaded, problems, unresolved) =
                resolve_saved(&loaded_saved, &declared, &[]).expect("a version this build wrote");
            assert!(problems.is_empty(), "{problems:?}");
            assert!(unresolved.is_empty(), "{unresolved:?}");
            assert_eq!(loaded, overrides);
        }

        /// An emptied slot inside a row is the same bare word an emptied row is, and it survives
        /// the trip out to text and back. One word at both levels is the whole of the format
        /// change: no `null`, nothing a person opening the file has to be taught.
        #[test]
        fn an_emptied_slot_round_trips_as_the_word_inside_the_list() {
            const GOLDEN_WITH_A_GAP: &str = "action_map_version = 1\n\
                \n\
                [bindings.keyboard_mouse]\n\
                \"persist_tests.jump\" = [\"cleared\", \"key/KeyJ\"]\n\
                \n\
                [tunables]\n";

            let declared = declared();
            let registry = types();
            let types = registry.read();
            let jump = mapping_key(&declared, DeviceFamily::KeyboardMouse, "persist_tests.jump");

            let mut overrides = Overrides::new();
            overrides.bind(
                DeviceFamily::KeyboardMouse,
                jump,
                [
                    None,
                    Some(BoundSlot::from(Control::PhysicalKey(KeyCode::KeyJ))),
                ],
            );

            let saved = save_overrides(&overrides);
            let serializer = TypedReflectSerializer::new(&saved, &types);
            let text = toml::to_string(&serializer).expect("serializes");
            assert_eq!(text, GOLDEN_WITH_A_GAP);

            let registration = types
                .get(core::any::TypeId::of::<SavedOverrides>())
                .unwrap();
            let value: toml::Value = toml::from_str(&text).expect("parses");
            let reflected = TypedReflectDeserializer::new(registration, &types)
                .deserialize(value)
                .expect("deserializes");
            let loaded_saved =
                <SavedOverrides as FromReflect>::from_reflect(&*reflected).expect("round-trips");

            let (loaded, problems, unresolved) =
                resolve_saved(&loaded_saved, &declared, &[]).expect("a version this build wrote");
            assert!(problems.is_empty(), "{problems:?}");
            assert!(unresolved.is_empty(), "{unresolved:?}");
            assert_eq!(loaded, overrides, "the gap came back where it was");
        }

        /// A chord rides the string its slot already has, under the names a catalogue uses, and
        /// comes back as the same slot — a modifier, a button held as part of the chord, and a
        /// logical key whose character is the separator itself.
        #[test]
        fn a_chord_round_trips_inside_its_slot() {
            use crate::binding::ModifierKey;
            use crate::present::ControlOrigin;

            const GOLDEN_WITH_CHORDS: &str = "action_map_version = 1\n\
                \n\
                [bindings.gamepad]\n\
                \"persist_tests.jump\" = \"pad/LeftTrigger+pad/South\"\n\
                \n\
                [bindings.keyboard_mouse]\n\
                \"persist_tests.jump\" = [\"mod/ctrl+key/KeyS\", \"mod/shift+char/+\"]\n\
                \n\
                [tunables]\n";

            let declared = declared();
            let registry = types();
            let types = registry.read();

            let mut overrides = Overrides::new();
            overrides.bind(
                DeviceFamily::KeyboardMouse,
                mapping_key(&declared, DeviceFamily::KeyboardMouse, "persist_tests.jump"),
                [
                    BoundSlot {
                        control: Control::PhysicalKey(KeyCode::KeyS),
                        with: alloc::vec![ControlOrigin::Modifier(ModifierKey::Ctrl)],
                    },
                    BoundSlot {
                        control: Control::LogicalKey('+'),
                        with: alloc::vec![ControlOrigin::Modifier(ModifierKey::Shift)],
                    },
                ],
            );
            overrides.bind(
                DeviceFamily::Gamepad,
                mapping_key(&declared, DeviceFamily::Gamepad, "persist_tests.jump"),
                [BoundSlot {
                    control: Control::GamepadButton(GamepadButton::South),
                    with: alloc::vec![ControlOrigin::Ours(Control::GamepadButton(
                        GamepadButton::LeftTrigger
                    ))],
                }],
            );

            let saved = save_overrides(&overrides);
            let serializer = TypedReflectSerializer::new(&saved, &types);
            let text = toml::to_string(&serializer).expect("serializes");
            assert_eq!(text, GOLDEN_WITH_CHORDS);

            let registration = types
                .get(core::any::TypeId::of::<SavedOverrides>())
                .unwrap();
            let value: toml::Value = toml::from_str(&text).expect("parses");
            let reflected = TypedReflectDeserializer::new(registration, &types)
                .deserialize(value)
                .expect("deserializes");
            let loaded_saved =
                <SavedOverrides as FromReflect>::from_reflect(&*reflected).expect("round-trips");

            let (loaded, problems, unresolved) =
                resolve_saved(&loaded_saved, &declared, &[]).expect("a version this build wrote");
            assert!(problems.is_empty(), "{problems:?}");
            assert!(unresolved.is_empty(), "{unresolved:?}");
            assert_eq!(loaded, overrides);
        }

        /// Only `char/` can hold a `+`, and only as its one character, so a slot splits the same
        /// way wherever that key sits in it. Anything that does not come apart into known names is
        /// no slot at all.
        #[test]
        fn a_slot_reads_back_only_when_every_name_in_it_does() {
            use crate::binding::ModifierKey;
            use crate::present::ControlOrigin;

            assert_eq!(
                slot_from_name("char/++key/KeyS"),
                Some(BoundSlot {
                    control: Control::PhysicalKey(KeyCode::KeyS),
                    with: alloc::vec![ControlOrigin::Ours(Control::LogicalKey('+'))],
                }),
                "the separator as a held key"
            );
            assert_eq!(
                slot_from_name("mod/alt+mod/super+key/KeyQ"),
                Some(BoundSlot {
                    control: Control::PhysicalKey(KeyCode::KeyQ),
                    with: alloc::vec![
                        ControlOrigin::Modifier(ModifierKey::Alt),
                        ControlOrigin::Modifier(ModifierKey::Super),
                    ],
                })
            );
            assert_eq!(slot_from_name("mod/ctrl+"), None, "a chord with no control");
            assert_eq!(slot_from_name("mod/ctrl"), None, "a modifier fires nothing");
            assert_eq!(slot_from_name("mod/hyper+key/KeyS"), None);
            assert_eq!(slot_from_name("char/ab"), None);
            assert_eq!(slot_from_name("+key/KeyS"), None);
        }

        /// A hand-written row of nothing but the word is the state that already means that, so a
        /// person editing the file cannot produce a row of empties the crate would have to explain.
        #[test]
        fn a_row_of_nothing_but_empties_reads_as_cleared() {
            let declared = declared();
            let saved = SavedOverrides {
                action_map_version: 1,
                bindings: BTreeMap::from([(
                    "keyboard_mouse".to_string(),
                    BTreeMap::from([(
                        "persist_tests.jump".to_string(),
                        SavedRow::Slots(alloc::vec!["cleared".to_string(), "cleared".to_string()]),
                    )]),
                )]),
                tunables: BTreeMap::new(),
            };

            let (loaded, problems, unresolved) =
                resolve_saved(&saved, &declared, &[]).expect("a version this build wrote");
            assert!(problems.is_empty(), "{problems:?}");
            assert!(unresolved.is_empty(), "{unresolved:?}");
            assert_eq!(
                loaded.get(
                    DeviceFamily::KeyboardMouse,
                    mapping_key(&declared, DeviceFamily::KeyboardMouse, "persist_tests.jump")
                ),
                Some(&Override::Cleared)
            );
        }

        /// A version this build never shipped refuses the whole set at once (D58).
        #[test]
        fn an_unrecognized_version_refuses_the_whole_set() {
            let declared = declared();
            let saved = SavedOverrides {
                action_map_version: 99,
                bindings: BTreeMap::from([(
                    "keyboard_mouse".to_string(),
                    BTreeMap::from([(
                        "persist_tests.jump".to_string(),
                        SavedRow::Slots(alloc::vec!["key/KeyZ".to_string()]),
                    )]),
                )]),
                tunables: BTreeMap::new(),
            };

            let err = resolve_saved(&saved, &declared, &[])
                .expect_err("a version this build never shipped must not resolve as one it did");

            assert_eq!(
                err,
                UnsupportedVersion {
                    found: 99,
                    supported: 1
                }
            );
        }

        /// A name this build cannot turn into a `Control` is reported rather than dropped in
        /// silence, and the row after it in the same set still resolves.
        #[test]
        fn an_unknown_control_is_reported_and_the_rest_still_resolves() {
            let declared = declared();
            let saved = SavedOverrides {
                action_map_version: 1,
                bindings: BTreeMap::from([(
                    "keyboard_mouse".to_string(),
                    BTreeMap::from([
                        (
                            "persist_tests.move.up".to_string(),
                            SavedRow::Slots(alloc::vec!["key/DoesNotExist".to_string()]),
                        ),
                        (
                            "persist_tests.jump".to_string(),
                            SavedRow::Slots(alloc::vec!["key/Space".to_string()]),
                        ),
                    ]),
                )]),
                tunables: BTreeMap::new(),
            };

            let (loaded, problems, unresolved) =
                resolve_saved(&saved, &declared, &[]).expect("a version this build wrote");

            assert!(unresolved.is_empty(), "{unresolved:?}");
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
                    DeviceFamily::KeyboardMouse,
                    mapping_key(
                        &declared,
                        DeviceFamily::KeyboardMouse,
                        "persist_tests.move.up"
                    )
                ),
                None
            );
            // And the row after it in the set still resolved.
            assert_eq!(
                loaded.get(
                    DeviceFamily::KeyboardMouse,
                    mapping_key(&declared, DeviceFamily::KeyboardMouse, "persist_tests.jump")
                ),
                Some(&Override::Slots(alloc::vec![Some(
                    Control::PhysicalKey(KeyCode::Space).into()
                )]))
            );
        }

        /// A renamed or removed action's row comes back named rather than vanishing without a
        /// trace.
        #[test]
        fn an_unresolved_mapping_name_is_reported_by_name() {
            let declared = declared();
            let saved = SavedOverrides {
                action_map_version: 1,
                bindings: BTreeMap::from([(
                    "keyboard_mouse".to_string(),
                    BTreeMap::from([(
                        "persist_tests.no_such_action".to_string(),
                        SavedRow::Slots(alloc::vec!["key/KeyZ".to_string()]),
                    )]),
                )]),
                tunables: BTreeMap::new(),
            };

            let (loaded, problems, unresolved) =
                resolve_saved(&saved, &declared, &[]).expect("a version this build wrote");

            assert!(problems.is_empty(), "{problems:?}");
            assert!(loaded.is_empty());
            assert_eq!(
                unresolved,
                [Unresolved {
                    family: DeviceFamily::KeyboardMouse,
                    name: "persist_tests.no_such_action".into(),
                    kind: UnresolvedKind::Mapping,
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

        /// A tunable round-trips through [`save_overrides`]/[`resolve_saved`] the same way a
        /// mapping does.
        #[test]
        fn a_tunable_round_trips_through_save_and_resolve() {
            let declared = declared();
            let tunables = declared_hold_or_toggle();

            let mut overrides = Overrides::new();
            overrides.tune(
                DeviceFamily::KeyboardMouse,
                "persist_tests.jump.hold_or_toggle",
                TunableValue::Bool(true),
            );
            let saved = save_overrides(&overrides);

            let (loaded, problems, unresolved) =
                resolve_saved(&saved, &declared, &tunables).expect("a version this build wrote");

            assert!(problems.is_empty(), "{problems:?}");
            assert!(unresolved.is_empty(), "{unresolved:?}");
            assert_eq!(loaded, overrides);
        }

        /// A name this build has no tunable for is reported by name rather than dropped, the same
        /// as an unresolved mapping.
        #[test]
        fn an_unresolved_tunable_name_is_reported_by_name() {
            let declared = declared();
            let saved = SavedOverrides {
                action_map_version: 1,
                bindings: BTreeMap::new(),
                tunables: BTreeMap::from([(
                    "keyboard_mouse".to_string(),
                    BTreeMap::from([(
                        "persist_tests.no_such_tunable".to_string(),
                        SavedTunableValue::Bool(true),
                    )]),
                )]),
            };

            let (loaded, problems, unresolved) =
                resolve_saved(&saved, &declared, &[]).expect("a version this build wrote");

            assert!(problems.is_empty(), "{problems:?}");
            assert!(loaded.is_empty());
            assert_eq!(
                unresolved,
                [Unresolved {
                    family: DeviceFamily::KeyboardMouse,
                    name: "persist_tests.no_such_tunable".into(),
                    kind: UnresolvedKind::Tunable,
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

            let saved = SavedOverrides {
                action_map_version: 1,
                bindings: BTreeMap::new(),
                tunables: BTreeMap::from([(
                    "gamepad".to_string(),
                    BTreeMap::from([(
                        "persist_tests.move.stick_deadzone".to_string(),
                        SavedTunableValue::Number(5.0),
                    )]),
                )]),
            };

            let (loaded, problems, unresolved) =
                resolve_saved(&saved, &[], &tunables).expect("a version this build wrote");

            assert!(problems.is_empty(), "{problems:?}");
            assert!(unresolved.is_empty(), "{unresolved:?}");
            assert_eq!(
                loaded.get_tunable(DeviceFamily::Gamepad, "persist_tests.move.stick_deadzone"),
                Some(TunableValue::Range {
                    value: 0.5,
                    min: 0.0,
                    max: 0.5,
                })
            );
        }

        /// The arrangement R17.10 and D59 anticipate: a game's own settings struct holding a
        /// `SavedOverrides` beside fields of its own, which is what `bevy_settings` writes a group
        /// from. TOML puts every plain value ahead of every table within a section, so the sibling
        /// field lands above `[overrides]` and not inside it — pinned here because a document that
        /// came out the other order would parse back as something else entirely.
        #[derive(Reflect, Clone, Debug, Default, PartialEq)]
        struct GroupWithOverrides {
            preset: String,
            overrides: SavedOverrides,
        }

        #[test]
        fn a_saved_set_round_trips_as_one_field_of_a_settings_group() {
            let registry = types();
            registry.write().register::<GroupWithOverrides>();
            let types = registry.read();

            let mut overrides = Overrides::new();
            overrides.tune(
                DeviceFamily::Gamepad,
                "persist_tests.move.stick_deadzone",
                TunableValue::Range {
                    value: 0.25,
                    min: 0.0,
                    max: 0.5,
                },
            );
            let group = GroupWithOverrides {
                preset: "persist_tests.southpaw".to_string(),
                overrides: save_overrides(&overrides),
            };

            let serializer = TypedReflectSerializer::new(&group, &types);
            let text = toml::to_string(&serializer).expect("serializes");
            assert_eq!(
                text,
                "preset = \"persist_tests.southpaw\"\n\
                 \n\
                 [overrides]\n\
                 action_map_version = 1\n\
                 \n\
                 [overrides.bindings]\n\
                 \n\
                 [overrides.tunables.gamepad]\n\
                 \"persist_tests.move.stick_deadzone\" = 0.25\n"
            );

            let registration = types
                .get(core::any::TypeId::of::<GroupWithOverrides>())
                .unwrap();
            let value: toml::Value = toml::from_str(&text).expect("parses");
            let reflected = TypedReflectDeserializer::new(registration, &types)
                .deserialize(value)
                .expect("deserializes");
            assert_eq!(
                <GroupWithOverrides as FromReflect>::from_reflect(&*reflected)
                    .expect("round-trips"),
                group
            );
        }

        /// What a settings layer hands over on a first launch, before any file exists — a
        /// `Reflect`-driven one builds the group's default and applies whatever the file held onto
        /// it, so a default stamped with no version at all would refuse every fresh install.
        #[test]
        fn a_default_saved_set_resolves_to_nothing_rather_than_a_refused_version() {
            let (loaded, problems, unresolved) =
                resolve_saved(&SavedOverrides::default(), &[], &[])
                    .expect("a version this build wrote");

            assert!(loaded.is_empty());
            assert!(problems.is_empty(), "{problems:?}");
            assert!(unresolved.is_empty(), "{unresolved:?}");
        }
    }
}
