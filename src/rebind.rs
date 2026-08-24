//! The player-facing model: mappings, tunables, and presets.
//!
//! The binding model is a developer's model. Negate, swizzle and response curves are adapters for
//! fitting a control to an action, and nobody rebinding "move forward" should meet one. What a
//! player gets is deliberately smaller: a list of **mappings**, each a named thing they can change
//! the controls for.
//!
//! A mapping holds an ordered list of **slots**, each holding one control, because a rebinding row
//! usually has more than one — "Primary" and "Secondary" is the arrangement almost every game ships.
//! Declaring one action mappable twice in one scheme is how you ship both defaults, and the mapping
//! grows to fit them.
//!
//! A mapping is not a binding. For anything composite the binding has no single control to show — a
//! movement binding is four keys — so each *part* of it becomes a mapping of its own, which is how
//! every shipped game presents movement and is why the composite never reaches the player.
//!
//! Mappings are opt-in. Bindings are developer data until
//! [`mappable`](crate::binding::BindingHandle::mappable) says otherwise, so a game that offers no
//! rebinding screen declares none and nothing changes.
//!
//! ```ignore
//! app.add_context::<OnFoot>(|controls| {
//!     controls.bind::<Move>(DirectionalButtons::wasd()).mappable();  // four mappings
//!     controls.bind::<Jump>(KeyCode::Space).mappable();              // one mapping…
//!     controls.bind::<Jump>(KeyCode::KeyJ).mappable();               // …with a second slot
//!     controls.bind::<Fire>(KeyCode::ControlLeft).mappable_upto(2);  // one control, two slots
//!     controls.bind::<Look>(MouseMove);                              // none: not rebindable
//! });
//! ```
//!
//! Then a screen walks them with [`mappings`], and needs to know nothing else about this crate.

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

/// What a mapping is called, in a form a translation catalogue can look up.
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
pub struct MappingKey {
    prefix: &'static str,
    part: Part,
}

impl MappingKey {
    pub(crate) const fn new(prefix: &'static str, part: Part) -> Self {
        Self { prefix, part }
    }

    /// Which part of its binding this mapping addresses.
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

impl core::fmt::Display for MappingKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.prefix)?;
        if let Some(part) = self.part.name() {
            f.write_str(".")?;
            f.write_str(part)?;
        }
        Ok(())
    }
}

/// How many slots a mapping has, and so how many controls a player may put in it.
///
/// A shipped game's rebinding screen is a table with a fixed shape, so the developer says how wide a
/// row is and the screen draws a cell per slot. The common commercial arrangement is two —
/// "primary" and "secondary" — and one is the default because that is what a binding declares
/// without saying anything.
///
/// [`Any`](Capacity::Any) exists for the other kind of program: a tool whose command set is large
/// and open, where the shortcuts cannot be laid out in advance and the screen grows an "add" button
/// instead. Blender and VS Code work this way; games essentially do not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Capacity {
    /// At most this many slots. Never zero.
    UpTo(usize),
    /// Any number of them, added and removed by the player.
    Any,
}

impl Capacity {
    /// Whether a mapping whose first `count` slots are filled has room for another.
    pub const fn has_room_for(self, count: usize) -> bool {
        match self {
            Self::UpTo(limit) => count < limit,
            Self::Any => true,
        }
    }

    /// How many slots there are, or `None` if the mapping grows without limit.
    ///
    /// A fixed-width table draws one cell per slot, so this is its column count.
    pub const fn slots(self) -> Option<usize> {
        match self {
            Self::UpTo(limit) => Some(limit),
            Self::Any => None,
        }
    }
}

/// One row of a rebinding screen.
///
/// Everything a screen needs to draw a row and file it under a heading, and nothing about how the
/// binding it came from is put together.
#[derive(Clone, Debug, PartialEq)]
pub struct Mapping {
    /// What this mapping is called, as a key to look up.
    pub key: MappingKey,
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
    /// The controls bound to it now, one per slot, in the order they were declared.
    ///
    /// Usually one. Two mappable bindings of the same action in the same scheme are the ordinary
    /// way to ship a default primary *and* secondary, and they arrive here as one row with two
    /// slots filled rather than as two rows.
    pub slots: Vec<Control>,
    /// How many slots this mapping has.
    pub capacity: Capacity,
    /// The path of the context the binding lives in.
    pub context: &'static str,
}

/// Every mapping a game has declared.
///
/// The list a rebinding screen is built from. Nothing here names an action or a context type, so a
/// screen written against it works for a game it was not compiled with.
///
/// Sorting is the caller's: group by [`category`](Mapping::category) to draw headings, and filter by
/// [`scheme`](Mapping::scheme) to show one device's worth at a time.
pub fn mappings(world: &World) -> Vec<Mapping> {
    let Some(declared) = world.get_resource::<crate::inspect::DeclaredContexts>() else {
        return Vec::new();
    };

    declared
        .0
        .iter()
        .flat_map(|context| (context.mappings)(world))
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

        let mappings = mappings(app.world());
        let names: Vec<String> = mappings
            .iter()
            .map(|mapping| mapping.key.to_string())
            .collect();
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

        // Each part carries the controls it currently holds, which is what a read-only screen
        // shows. One apiece here: nothing declared a second mappable binding.
        assert_eq!(mappings[0].slots, [Control::Key(KeyCode::KeyW)]);
        assert_eq!(mappings[4].slots, [Control::Key(KeyCode::Space)]);
        assert_eq!(
            mappings[4].capacity,
            Capacity::UpTo(1),
            "one default, one slot"
        );

        // The category comes from the action, so the four movement rows file together.
        assert_eq!(mappings[0].category, Some("rebind_tests.movement"));
        assert_eq!(mappings[4].category, None);

        // A part of a composite holds a button whatever the composite reports as a whole.
        assert_eq!(mappings[0].accepts, ChannelShape::Button);
        assert_eq!(mappings[0].scheme, Scheme::KeyboardMouse);
    }

    /// A game that declares no mappings has no player-facing surface at all, which is the default
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

        assert!(mappings(app.world()).is_empty());
    }

    /// Shipping a translation catalogue must not be the price of a legible screen.
    #[test]
    fn a_key_reads_sensibly_without_a_catalogue() {
        let label = |key: MappingKey| key.fallback_label();

        assert_eq!(label(MappingKey::new("gameplay.jump", Part::Whole)), "Jump");
        assert_eq!(label(MappingKey::new("gameplay.move", Part::Up)), "Move Up");
        assert_eq!(
            label(MappingKey::new("dead_zone.toggle_overlay", Part::Whole)),
            "Toggle Overlay",
            "the namespace is for keeping keys apart, not for reading"
        );
        assert_eq!(
            label(MappingKey::new("gameplay.lean", Part::Negative)),
            "Lean Negative"
        );
    }

    /// Two mappable bindings of one action in one scheme are a default primary and secondary, which
    /// is how a shipped game writes that — so they merge into one row holding two controls rather
    /// than becoming two rows both called Jump.
    #[test]
    fn two_bindings_of_one_action_share_one_slot() {
        #[derive(InputContext)]
        #[context(path = "rebind_tests.two_defaults", tick = Fixed)]
        struct TwoDefaults;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<TwoDefaults>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable();
            controls.bind::<Jump>(KeyCode::Enter).mappable();
        });

        let mappings = mappings(app.world());
        assert_eq!(mappings.len(), 1, "one row, not two");
        assert_eq!(
            mappings[0].slots,
            [Control::Key(KeyCode::Space), Control::Key(KeyCode::Enter)],
            "in the order they were declared, which is what makes the first one primary"
        );
        // Nobody said "2". A mapping is never narrower than the defaults it already holds, so
        // declaring two of them is enough on its own to make a two-slot row.
        assert_eq!(mappings[0].capacity, Capacity::UpTo(2));
    }

    /// The collision that survives the merge above: *different* actions answering to one name, where
    /// a saved rebinding of one would land on the other.
    #[test]
    #[should_panic(expected = "declares a mapping named")]
    fn two_actions_cannot_share_a_slot_name() {
        #[derive(InputContext)]
        #[context(path = "rebind_tests.colliding", tick = Fixed)]
        struct Colliding;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Colliding>(|controls| {
            controls
                .bind::<Jump>(KeyCode::Space)
                .mappable_as("rebind_tests.go");
            controls
                .bind::<ToggleOverlay>(KeyCode::Enter)
                .mappable_as("rebind_tests.go");
        });
    }

    /// Capacity is a ceiling the author raises, not a count of what is bound: a game ships one
    /// default and leaves the second slot for the player.
    #[test]
    fn a_slot_can_be_given_more_room_than_its_defaults_need() {
        #[derive(InputContext)]
        #[context(path = "rebind_tests.roomy", tick = Fixed)]
        struct Roomy;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Roomy>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable_upto(2);
            controls.bind::<ToggleOverlay>(KeyCode::F1).mappable_any();
        });

        let mappings = mappings(app.world());
        assert_eq!(mappings[0].slots, [Control::Key(KeyCode::Space)]);
        assert_eq!(
            mappings[0].capacity,
            Capacity::UpTo(2),
            "one default, two slots"
        );
        assert_eq!(mappings[1].capacity, Capacity::Any);

        // What a table lays out, and what an "add" button asks before offering itself.
        assert_eq!(mappings[0].capacity.slots(), Some(2));
        assert_eq!(mappings[1].capacity.slots(), None);
        assert!(mappings[0].capacity.has_room_for(1));
        assert!(!mappings[0].capacity.has_room_for(2));
        assert!(mappings[1].capacity.has_room_for(2));
    }

    /// The widest declaration wins, and the defaults widen it further — because a narrower word
    /// elsewhere is a statement about *that* binding, not a demand that the row be narrow.
    #[test]
    fn capacity_is_the_widest_anything_asked_for() {
        #[derive(InputContext)]
        #[context(path = "rebind_tests.widening", tick = Fixed)]
        struct Widening;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Widening>(|controls| {
            // A plain `mappable` says `UpTo(1)`, and does not narrow the row it lands in.
            controls.bind::<Jump>(KeyCode::Space).mappable_upto(3);
            controls.bind::<Jump>(KeyCode::Enter).mappable();
        });

        let mappings = mappings(app.world());
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].capacity, Capacity::UpTo(3));
    }

    /// A mapping with no room is a binding that is not mappable, which is what leaving `mappable`
    /// already says.
    #[test]
    #[should_panic(expected = "room for at least one control")]
    fn a_slot_cannot_be_declared_empty() {
        #[derive(InputContext)]
        #[context(path = "rebind_tests.empty", tick = Fixed)]
        struct Empty;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<Empty>(|controls| {
            controls.bind::<Jump>(KeyCode::Space).mappable_upto(0);
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

        let mappings = mappings(app.world());
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].key, mappings[1].key, "one name…");
        assert_eq!(mappings[0].scheme, Scheme::KeyboardMouse);
        assert_eq!(mappings[1].scheme, Scheme::Gamepad, "…two schemes");
    }

    /// A name and a capacity are separate things to say, so saying both must work in either order
    /// — neither call may quietly discard what the other declared.
    #[test]
    fn naming_a_slot_and_widening_it_are_independent() {
        #[derive(InputContext)]
        #[context(path = "rebind_tests.named_and_wide", tick = Fixed)]
        struct NamedAndWide;

        let mut app = App::new();
        app.add_plugins((bevy_input::InputPlugin, ActionMapPlugin));
        app.add_context::<NamedAndWide>(|controls| {
            controls
                .bind::<Jump>(KeyCode::Space)
                .mappable_as("rebind_tests.leap")
                .mappable_upto(2);
            controls
                .bind::<ToggleOverlay>(KeyCode::F1)
                .mappable_upto(3)
                .mappable_as("rebind_tests.peek");
        });

        let mappings = mappings(app.world());
        assert_eq!(mappings[0].key.to_string(), "rebind_tests.leap");
        assert_eq!(mappings[0].capacity, Capacity::UpTo(2));
        assert_eq!(mappings[1].key.to_string(), "rebind_tests.peek");
        assert_eq!(mappings[1].capacity, Capacity::UpTo(3));
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

        let names: Vec<String> = mappings(app.world())
            .iter()
            .map(|mapping| mapping.key.to_string())
            .collect();
        assert!(names.contains(&String::from("rebind_tests.move.up")));
        assert!(names.contains(&String::from("rebind_tests.move_alt.up")));
    }
}
