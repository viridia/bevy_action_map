//! A named arrangement of mappings, applied as a unit.
//!
//! ```ignore
//! let southpaw = Preset::build(world, "southpaw", |preset| {
//!     preset.bind::<Turn>(DeviceFamily::Gamepad, [Control::GamepadAxis(GamepadAxis::RightStickX)]);
//! });
//!
//! apply_overrides_with_preset(world, &southpaw.rows, &southpaw.rows);
//! ```
//!
//! A preset is an [`Overrides`] with a name attached, nothing more. Applying one calls
//! [`apply_overrides_with_preset`](crate::overrides::apply_overrides_with_preset), which lets a
//! preset move a row that a player's own capture screen cannot offer, such as a gamepad stick.
//!
//! There is no crate-owned registry. A game keeps its own list of presets, exactly as it keeps its
//! own [`Overrides`] working copy. Merging a game's own manual rebinds with a selected preset's
//! rows into one applied set, so that picking a preset does not discard a player's capture-driven
//! edits and vice versa, is the caller's job.

use bevy_ecs::world::World;

use crate::action::InputAction;
use crate::device::DeviceFamily;
use crate::mapping::{BoundSlot, TunableValue, mappings};
use crate::overrides::Overrides;

/// A named set of mapping assignments a player selects as a unit.
///
/// "Default", "Southpaw", "Lefty". For a device class with no per-mapping rebinding, such as a
/// gamepad stick, this is the entire remapping story.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// The preset's name, as a localization key, using the same convention as
    /// [`ActionMapping::category`](crate::mapping::ActionMapping::category).
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
    /// Puts `slots` in whatever mapping `A` has in `family`: bare controls, or [`BoundSlot`]s for
    /// any that are held with something, bound exactly as [`Overrides::bind`] binds them.
    ///
    /// # Panics
    ///
    /// If `A` has no mapping in `family`, or more than one. A composite has one mapping per part
    /// (`Move` has four), and naming the action and family alone cannot say which of them a
    /// preset means; bind a composite's part directly instead. This runs while building the app,
    /// not during play.
    pub fn bind<A: InputAction>(
        &mut self,
        family: DeviceFamily,
        slots: impl IntoIterator<Item = impl Into<BoundSlot>>,
    ) -> &mut Self {
        let mut found = mappings(self.world)
            .into_iter()
            .filter(|mapping| mapping.action == A::id() && mapping.family == family);
        let Some(mapping) = found.next() else {
            panic!(
                "preset names `{}` in {family:?}, but nothing binds it there",
                A::PATH
            );
        };
        assert!(
            found.next().is_none(),
            "preset names `{}` in {family:?}, which has more than one mapping there — bind the \
             part a composite action's row belongs to instead",
            A::PATH
        );
        self.rows.bind(
            family,
            mapping.key,
            slots.into_iter().map(Into::<BoundSlot>::into),
        );
        self
    }

    /// Sets a tunable to `value`, using the key it was declared with.
    ///
    /// A preset is not only rebound controls: "Southpaw" might also want a tighter dead zone on
    /// the stick it just moved, and this is how it says so. `key` is whatever was passed to
    /// [`tunable_dead_zone`](crate::binding::BindingBuilder::tunable_dead_zone) or
    /// [`hold_or_toggle`](crate::binding::InputContextBuilder::hold_or_toggle) when the binding
    /// was declared.
    pub fn tune(
        &mut self,
        family: DeviceFamily,
        key: &'static str,
        value: TunableValue,
    ) -> &mut Self {
        self.rows.tune(family, key, value);
        self
    }
}

#[cfg(all(test, feature = "keyboard"))]
mod tests {
    use super::*;

    use bevy_app::App;
    use bevy_input::keyboard::KeyCode;

    use crate::context::ActionMapAppExt;
    use crate::{ActionMapPlugin, InputAction, InputContext};

    #[derive(InputAction)]
    #[action(path = "preset_tests.jump", output = bool, intent = Button)]
    struct Jump;

    #[derive(InputContext)]
    #[context(path = "preset_tests.on_foot", tick = Fixed)]
    struct OnFoot;

    const HOLD_OR_TOGGLE_KEY: &str = "preset_tests.jump.hold_or_toggle";

    /// A preset moves a tunable alongside its bindings — the case `bind` alone cannot cover, since
    /// a tunable is not a mapping.
    #[test]
    fn a_preset_can_set_a_tunable() {
        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.hold_or_toggle::<Jump>(HOLD_OR_TOGGLE_KEY);
        });

        let preset = Preset::build(app.world(), "preset_tests.toggle", |preset| {
            preset.tune(
                DeviceFamily::KeyboardMouse,
                HOLD_OR_TOGGLE_KEY,
                TunableValue::Bool(true),
            );
        });

        assert_eq!(
            preset
                .rows
                .get_tunable(DeviceFamily::KeyboardMouse, HOLD_OR_TOGGLE_KEY),
            Some(TunableValue::Bool(true))
        );
    }
}
