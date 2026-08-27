//! A named arrangement of mappings, applied as a unit.
//!
//! ```ignore
//! let southpaw = Preset::build(world, "southpaw", |preset| {
//!     preset.bind::<Turn>(Scheme::Gamepad, [Control::GamepadAxis(GamepadAxis::RightStickX)]);
//! });
//!
//! apply_overrides_with_preset(world, &southpaw.rows, &southpaw.rows);
//! ```
//!
//! A preset is an [`Overrides`](crate::overrides::Overrides) with a name attached — nothing more.
//! It reuses the diff it already is: applying one is
//! [`apply_overrides_with_preset`](crate::overrides::apply_overrides_with_preset), which exempts a
//! preset's own rows from the "not rebindable here" refusal that would otherwise stop it moving a
//! row a player's own capture screen never offers, such as a gamepad stick.
//!
//! There is no crate-owned registry: a game keeps its own list of presets, exactly as it keeps its
//! own [`Overrides`] working copy. Where a game's own manual rebinds and a selected preset's rows
//! both belong in one applied set — so that picking a preset does not silently discard a player's
//! own capture-driven edits, and vice versa — is the caller's to merge before applying.

use bevy_ecs::world::World;

use crate::action::InputAction;
use crate::binding::Control;
use crate::mapping::{Scheme, mappings};
use crate::overrides::Overrides;

/// A named set of mapping assignments a player selects as a unit.
///
/// "Default", "Southpaw", "Lefty" — for a device class with no per-mapping rebinding, such as a
/// gamepad stick, this is the entire remapping story.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// The preset's name, as a localization key — the same convention as
    /// [`Mapping::category`](crate::mapping::Mapping::category).
    pub name: &'static str,
    /// What this preset assigns, as a diff against the game's declared bindings.
    pub rows: Overrides,
}

impl Preset {
    /// Builds a preset against a game's own declared mappings — [`add_context`]'s ergonomics, for
    /// the same reason: naming an action by type rather than by the [`MappingKey`] it happens to
    /// have, which nothing outside this crate can spell in the first place.
    ///
    /// [`add_context`]: crate::context::ActionMapAppExt::add_context
    /// [`MappingKey`]: crate::mapping::MappingKey
    pub fn build(world: &World, name: &'static str, f: impl FnOnce(&mut PresetBuilder)) -> Self {
        let mut builder = PresetBuilder {
            world,
            rows: Overrides::new(),
        };
        f(&mut builder);
        Preset {
            name,
            rows: builder.rows,
        }
    }
}

/// Resolves actions to the mappings they actually have, so a preset is built by type rather than by
/// hand-deriving a [`MappingKey`](crate::mapping::MappingKey).
pub struct PresetBuilder<'w> {
    world: &'w World,
    rows: Overrides,
}

impl PresetBuilder<'_> {
    /// Puts `controls` in whatever mapping `A` has in `scheme`.
    ///
    /// # Panics
    ///
    /// If `A` has no mapping in `scheme`, or more than one. A composite has one mapping per part —
    /// `Move` has four — and naming the action and scheme alone cannot say which of them a preset
    /// means; a composite needs its own lookup rather than this one. This is app-build code, the
    /// same class of mistake `add_context`'s own diagnostics catch, and just as unreachable in a
    /// shipped game.
    pub fn bind<A: InputAction>(
        &mut self,
        scheme: Scheme,
        controls: impl IntoIterator<Item = Control>,
    ) -> &mut Self {
        let mut found = mappings(self.world)
            .into_iter()
            .filter(|mapping| mapping.action == A::id() && mapping.scheme == scheme);
        let Some(mapping) = found.next() else {
            panic!(
                "preset names `{}` in {scheme:?}, but nothing binds it there",
                A::PATH
            );
        };
        assert!(
            found.next().is_none(),
            "preset names `{}` in {scheme:?}, which has more than one mapping there — bind the \
             part a composite action's row belongs to instead",
            A::PATH
        );
        self.rows.bind(scheme, mapping.key, controls);
        self
    }
}
