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
//! A preset is an [`Overrides`] with a name attached, nothing more.
//! Applying one calls
//! [`apply_overrides_with_preset`](crate::overrides::apply_overrides_with_preset), which lets a
//! preset move a row that a player's own capture screen cannot offer, such as a gamepad stick.
//!
//! There is no crate-owned registry. A game keeps its own list of presets, exactly as it keeps its
//! own [`Overrides`] working copy. Merging a game's own manual rebinds with a selected preset's
//! rows into one applied set, so that picking a preset does not discard a player's capture-driven
//! edits and vice versa, is the caller's job.

use bevy_ecs::world::World;

use crate::action::InputAction;
use crate::binding::Control;
use crate::mapping::{Scheme, mappings};
use crate::overrides::Overrides;

/// A named set of mapping assignments a player selects as a unit.
///
/// "Default", "Southpaw", "Lefty". For a device class with no per-mapping rebinding, such as a
/// gamepad stick, this is the entire remapping story.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// The preset's name, as a localization key, using the same convention as
    /// [`Mapping::category`](crate::mapping::Mapping::category).
    pub name: &'static str,
    /// What this preset assigns, as a diff against the game's declared bindings.
    pub rows: Overrides,
}

impl Preset {
    /// Builds a preset against a game's own declared mappings, matching [`add_context`]'s
    /// ergonomics: actions are named by type rather than by the [`MappingKey`] they happen to
    /// have.
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
    /// If `A` has no mapping in `scheme`, or more than one. A composite has one mapping per part
    /// (`Move` has four), and naming the action and scheme alone cannot say which of them a
    /// preset means; bind a composite's part directly instead. This runs while building the app,
    /// not during play.
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
