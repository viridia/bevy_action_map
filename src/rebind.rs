//! The player-facing model: mappable slots, tunables, and presets.
//!
//! The binding model is a developer's model. Negate, swizzle and response curves are adapters for
//! fitting a control to an action, and nobody rebinding "move forward" should meet one. What a
//! player gets is deliberately smaller: a list of **slots**, each showing one control they can
//! change.
//!
//! A slot is not a binding. For anything composite the binding has no single control to show — a
//! movement binding is four keys — so each *part* of it is its own slot, which is how every shipped
//! game presents movement and is why the composite never reaches the player.
//!
//! Slots are opt-in. Bindings are developer data until [`mappable`](crate::binding::BindingHandle::mappable)
//! says otherwise, so a game that offers no rebinding screen declares none and nothing changes.
//!
//! ```ignore
//! app.add_context::<OnFoot>(|controls| {
//!     controls.bind::<Move>(DirectionalButtons::wasd()).mappable();  // four slots
//!     controls.bind::<Jump>(KeyCode::Space).mappable();              // one slot
//!     controls.bind::<Look>(MouseMove);                              // none: not rebindable
//! });
//! ```
//!
//! Then a screen walks them with [`slots`], and needs to know nothing else about this crate.

use alloc::string::String;
use alloc::vec::Vec;

use bevy_ecs::world::World;

use crate::action::{ActionId, ChannelShape};
use crate::binding::{Control, Part};

/// The set of devices a player is using, and the scope a rebinding is made in.
///
/// Keyboard bindings and gamepad bindings are alternatives rather than competitors: a player is
/// using one or the other at any moment, so the two never conflict with each other and are remapped
/// independently. A rebinding screen shows one scheme at a time for the same reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scheme {
    /// Keyboard and mouse.
    KeyboardMouse,
    /// A gamepad.
    Gamepad,
}

/// What a slot is called, in a form a translation catalogue can look up.
///
/// This is a **key, not display text**. A rebinding row has two halves — the name of the thing and
/// the control bound to it — and a crate that returned "Move Forward" for the first would make one
/// half of every row untranslatable. So the key is what is carried, and rendering it is the app's
/// business.
///
/// It is derived rather than declared: the action's path plus the part's name, both of which
/// already exist and are already stable. `gameplay.move` plus `up` is `gameplay.move.up`, and
/// nothing has to be kept in step with anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlotKey {
    prefix: &'static str,
    part: Part,
}

impl SlotKey {
    pub(crate) const fn new(prefix: &'static str, part: Part) -> Self {
        Self { prefix, part }
    }

    /// Which part of its binding this slot addresses.
    pub const fn part(self) -> Part {
        self.part
    }

    /// Readable text for a game with no translation catalogue.
    ///
    /// Turns the key into something presentable — `gameplay.move.up` reads as "Move Up" — so that
    /// shipping translations is never the price of a legible rebinding screen. Use it as the
    /// fallback when a lookup misses, not in place of one.
    pub fn fallback_label(self) -> String {
        let mut label = String::new();
        // The namespace is for keeping keys apart, not for reading, so only the last segment of the
        // prefix survives: `dead_zone.toggle_overlay` is "Toggle Overlay" rather than "Dead Zone …".
        let name = self.prefix.rsplit('.').next().unwrap_or(self.prefix);
        for word in name.split('_').chain(self.part.name()) {
            if !label.is_empty() {
                label.push(' ');
            }
            let mut characters = word.chars();
            if let Some(first) = characters.next() {
                label.extend(first.to_uppercase());
                label.push_str(characters.as_str());
            }
        }
        label
    }
}

impl core::fmt::Display for SlotKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.prefix)?;
        if let Some(part) = self.part.name() {
            f.write_str(".")?;
            f.write_str(part)?;
        }
        Ok(())
    }
}

/// One row of a rebinding screen.
///
/// Everything a screen needs to draw a row and file it under a heading, and nothing about how the
/// binding it came from is put together.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    /// What this slot is called, as a key to look up.
    pub key: SlotKey,
    /// The action it drives.
    pub action: ActionId,
    /// That action's declared path.
    pub action_path: &'static str,
    /// What to file it under, if the action said.
    pub category: Option<&'static str>,
    /// Which set of devices it belongs to. A screen shows one scheme at a time.
    pub scheme: Scheme,
    /// The kind of control it can hold, which is what a capture may accept for it.
    pub accepts: ChannelShape,
    /// The control bound to it now.
    pub current: Control,
    /// The path of the context the binding lives in.
    pub context: &'static str,
}

/// Every mappable slot a game has declared.
///
/// The list a rebinding screen is built from. Nothing here names an action or a context type, so a
/// screen written against it works for a game it was not compiled with.
///
/// Sorting is the caller's: group by [`category`](Slot::category) to draw headings, and filter by
/// [`scheme`](Slot::scheme) to show one device's worth at a time.
pub fn slots(world: &World) -> Vec<Slot> {
    let Some(declared) = world.get_resource::<crate::inspect::DeclaredContexts>() else {
        return Vec::new();
    };

    declared
        .0
        .iter()
        .flat_map(|context| (context.slots)(world))
        .collect()
}

#[cfg(all(test, feature = "keyboard"))]
mod tests {
    use super::*;

    use alloc::string::ToString;
    use bevy_app::App;
    use bevy_input::keyboard::KeyCode;

    use crate::binding::DirectionalButtons;
    use crate::context::ActionMapAppExt;
    use crate::{ActionMapPlugin, InputAction, InputContext};

    #[derive(InputAction)]
    #[action(path = "rebind_tests.move", output = bevy_math::Vec2, intent = Directional2, category = "rebind_tests.movement")]
    struct Move;

    #[derive(InputAction)]
    #[action(path = "rebind_tests.jump", output = bool, intent = Button)]
    struct Jump;

    #[derive(InputAction)]
    #[action(path = "rebind_tests.toggle_overlay", output = bool, intent = Button)]
    struct ToggleOverlay;

    #[derive(InputContext)]
    #[context(path = "rebind_tests.on_foot", tick = Fixed)]
    struct OnFoot;

    /// The heart of it: a composite is four rows rather than one, because a player rebinds "move
    /// forward" and a movement binding has no single control to put beside it.
    #[test]
    fn a_composite_becomes_one_slot_per_part() {
        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<OnFoot>(|controls| {
            controls.bind::<Move>(DirectionalButtons::wasd()).mappable();
            controls.bind::<Jump>(KeyCode::Space).mappable();
            // Declared but not mappable, so the player never sees it.
            controls.bind::<Jump>(KeyCode::Enter);
        });

        let slots = slots(app.world());
        let names: Vec<String> = slots.iter().map(|slot| slot.key.to_string()).collect();
        assert_eq!(
            names,
            [
                "rebind_tests.move.up",
                "rebind_tests.move.down",
                "rebind_tests.move.left",
                "rebind_tests.move.right",
                "rebind_tests.jump",
            ]
        );

        // Each part carries the control it currently holds, which is what a read-only screen shows.
        assert_eq!(slots[0].current, Control::Key(KeyCode::KeyW));
        assert_eq!(slots[4].current, Control::Key(KeyCode::Space));

        // The category comes from the action, so the four movement rows file together.
        assert_eq!(slots[0].category, Some("rebind_tests.movement"));
        assert_eq!(slots[4].category, None);

        // A part of a composite holds a button whatever the composite reports as a whole.
        assert_eq!(slots[0].accepts, ChannelShape::Button);
        assert_eq!(slots[0].scheme, Scheme::KeyboardMouse);
    }

    /// A game that declares no slots has no player-facing surface at all, which is the default and
    /// is what keeps the whole player-facing model additive.
    #[test]
    fn declaring_nothing_mappable_leaves_no_slots() {
        #[derive(InputContext)]
        #[context(path = "rebind_tests.silent", tick = Fixed)]
        struct Silent;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Silent>(|controls| {
            controls.bind::<Jump>(KeyCode::Space);
        });

        assert!(slots(app.world()).is_empty());
    }

    /// Shipping a translation catalogue must not be the price of a legible screen.
    #[test]
    fn a_key_reads_sensibly_without_a_catalogue() {
        let label = |key: SlotKey| key.fallback_label();

        assert_eq!(label(SlotKey::new("gameplay.jump", Part::Whole)), "Jump");
        assert_eq!(label(SlotKey::new("gameplay.move", Part::Up)), "Move Up");
        assert_eq!(
            label(SlotKey::new("dead_zone.toggle_overlay", Part::Whole)),
            "Toggle Overlay",
            "the namespace is for keeping keys apart, not for reading"
        );
        assert_eq!(
            label(SlotKey::new("gameplay.lean", Part::Negative)),
            "Lean Negative"
        );
    }

    /// Two slots answering to one name would mean a saved rebinding of one landing on the other.
    #[test]
    #[should_panic(expected = "declares a mappable slot named")]
    fn two_bindings_of_one_action_cannot_share_a_slot_name() {
        #[derive(InputContext)]
        #[context(path = "rebind_tests.colliding", tick = Fixed)]
        struct Colliding;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Colliding>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(KeyCode::Enter).mappable();
        });
    }

    /// The same name in two schemes is not a collision, and this is the ordinary way to write a
    /// game that offers rebinding on both devices: one key and one button, both mappable, both
    /// called `jump`. They land in separate tables (§10.1), so nothing can be confused for anything.
    #[cfg(feature = "gamepad")]
    #[test]
    fn one_name_in_two_schemes_is_two_rows_rather_than_a_collision() {
        use bevy_input::gamepad::GamepadButton;

        #[derive(InputContext)]
        #[context(path = "rebind_tests.both_devices", tick = Fixed)]
        struct BothDevices;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<BothDevices>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(GamepadButton::South).mappable();
        });

        let slots = slots(app.world());
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].key, slots[1].key, "one name…");
        assert_eq!(slots[0].scheme, Scheme::KeyboardMouse);
        assert_eq!(slots[1].scheme, Scheme::Gamepad, "…two schemes");
    }

    /// And the same collision across two contexts, which no single plan can see.
    #[test]
    #[should_panic(expected = "already uses")]
    fn one_action_mappable_in_two_contexts_collides() {
        #[derive(InputContext)]
        #[context(path = "rebind_tests.first", tick = Fixed)]
        struct First;

        #[derive(InputContext)]
        #[context(path = "rebind_tests.second", tick = Fixed)]
        struct Second;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<First>(|controls| {
            controls.bind::<ToggleOverlay>(KeyCode::F1).mappable();
        });
        app.add_context::<Second>(|controls| {
            controls.bind::<ToggleOverlay>(KeyCode::F2).mappable();
        });
    }

    /// The remedy the diagnostic names, and proof it works.
    #[test]
    fn a_name_of_your_own_settles_a_collision() {
        #[derive(InputContext)]
        #[context(path = "rebind_tests.renamed_a", tick = Fixed)]
        struct RenamedA;

        #[derive(InputContext)]
        #[context(path = "rebind_tests.renamed_b", tick = Fixed)]
        struct RenamedB;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<RenamedA>(|controls| {
            controls.bind::<Move>(DirectionalButtons::wasd()).mappable();
        });
        app.add_context::<RenamedB>(|controls| {
            controls
                .bind::<Move>(DirectionalButtons::arrow_keys())
                .mappable_as("rebind_tests.move_alt");
        });

        let names: Vec<String> = slots(app.world())
            .iter()
            .map(|slot| slot.key.to_string())
            .collect();
        assert!(names.contains(&String::from("rebind_tests.move.up")));
        assert!(names.contains(&String::from("rebind_tests.move_alt.up")));
    }
}
