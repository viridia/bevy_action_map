//! Rebinding by hand: walk a game's mappings and press something for each one.
//!
//! Run it and read the console: `cargo run --example capture`.
//!
//! It takes each mappable slot in turn, listens for a control, and reports what it heard —
//! including what that control already does elsewhere. Nothing is rebound: capture reports a
//! choice, and acting on one is a separate matter.
//!
//! A *slot* rather than a mapping, because a mapping holds an ordered list of them: `Jump` ships
//! two keyboard defaults and `Fire` ships one with room for a second, so the walk visits Jump twice
//! and stops at Fire's empty second slot. That is the "primary and secondary" table every shipped
//! game has, before anything draws it.
//!
//! Worth trying, because each is a case the crate has an opinion about:
//!
//! - press `Escape` — it skips the row instead of being captured, because it is *excluded*, and an
//!   excluded control goes on doing its normal job while a capture listens;
//! - press `F1` — refused out loud, because it opens the settings screen and is *reserved*;
//! - press a key that is already bound — captured, with the clash reported;
//! - press a **gamepad** button on a keyboard row, or a key on a gamepad row — refused, because a
//!   mapping is rebound within its own control scheme.
//!
//! No context is ever spawned here, and nothing evaluates: this is a settings screen with no game
//! behind it, which is the case R19.5 is about.
//!
//! The window has nothing in it. It exists because that is where keyboard input comes from;
//! everything the example has to say, it says on stdout.

#![allow(missing_docs)]

use bevy::input::gamepad::GamepadButton;
use bevy::prelude::*;
use bevy_action_map::mapping;
use bevy_action_map::prelude::*;

#[derive(InputAction)]
#[action(path = "capture_demo.move", output = Vec2, intent = Directional2, category = "capture_demo.movement")]
struct Move;

#[derive(InputAction)]
#[action(path = "capture_demo.jump", output = bool, intent = Button, category = "capture_demo.actions")]
struct Jump;

#[derive(InputAction)]
#[action(path = "capture_demo.fire", output = bool, intent = Button, category = "capture_demo.actions")]
struct Fire;

#[derive(InputAction)]
#[action(path = "capture_demo.open_settings", output = bool, intent = Button)]
struct OpenSettings;

#[derive(InputContext)]
#[context(path = "capture_demo.playing", tick = Render)]
struct Playing;

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "capture — watch the console".into(),
                resolution: (520, 160).into(),
                ..default()
            }),
            ..default()
        }),
        ActionMapPlugin,
    ));

    app.add_context::<Playing>(|controls| {
        controls.bind::<Move>(DirectionalButtons::wasd()).mappable();

        // Two mappable bindings of one action in one scheme are a default primary *and* secondary.
        // They derive the same mapping name on purpose: that is one row holding two controls,
        // not two rows both called Jump, and its capacity grows to fit them without being asked.
        controls.bind::<Jump>(KeyCode::Space).mappable();
        controls.bind::<Jump>(KeyCode::KeyJ).mappable();

        // The other half of the same idea: room for two, only one shipped. The walk below stops at
        // the empty second slot, which is the cell a settings screen would draw blank.
        controls.bind::<Fire>(KeyCode::ControlLeft).mappable_upto(2);

        // The same two actions on the pad, mappable again. Both derive the same mapping name a
        // second time on purpose: `capture_demo.jump` means one thing on the keyboard and another
        // on the gamepad, the two are rebound independently (R19.7), and they are stored in
        // separate tables. Only a repeat *within* one scheme is a collision.
        controls.bind::<Move>(DirectionalButtons::dpad()).mappable();
        controls.bind::<Jump>(GamepadButton::South).mappable();

        // The controls that open this very screen. They get no mapping, and capture will not take
        // them for any other mapping either — which is the half that matters, since a screen you
        // can open but whose key now also fires the gun is no better off.
        controls.bind::<OpenSettings>(KeyCode::F1).reserved();
        controls
            .bind::<OpenSettings>(GamepadButton::Select)
            .reserved();
    });

    app.add_systems(Startup, begin)
        .add_observer(took)
        .add_observer(would_not_take)
        .add_systems(Update, skip)
        .run();
}

/// Which slots are left to walk, and what is listening for the current one.
///
/// A slot rather than a mapping: a mapping holds an ordered *list* of slots, and a "primary and
/// secondary" table is that list drawn as columns. `Fire` below declares room for two and ships one,
/// so the walk stops at its empty second slot like any other.
#[derive(Resource)]
struct Walk {
    remaining: Vec<(mapping::Mapping, usize)>,
    listening: Option<Entity>,
}

fn begin(world: &mut World) {
    println!("Walking every mappable slot this game declares.");
    let mut remaining: Vec<(mapping::Mapping, usize)> = mapping::mappings(world)
        .into_iter()
        .flat_map(|mapping| slots(&mapping).map(move |slot| (mapping.clone(), slot)))
        .collect();
    // Reversed so that popping from the end walks them in declaration order.
    remaining.reverse();
    world.insert_resource(Walk {
        remaining,
        listening: None,
    });
    next(world);
}

/// Every slot of a row a capture could fill: the ones holding something, plus the next empty one
/// if the row has room for it.
///
/// The same rule `CaptureSession::for_slot` enforces — it refuses anything else — so a screen that
/// asks this first never offers a slot that would be turned down.
fn slots(mapping: &mapping::Mapping) -> std::ops::Range<usize> {
    let filled = mapping.slots.len();
    0..if mapping.capacity.has_room_for(filled) {
        filled + 1
    } else {
        filled
    }
}

/// Starts a capture for the next slot, or reports that the walk is over.
fn next(world: &mut World) {
    if let Some(listening) = world.resource_mut::<Walk>().listening.take() {
        world.despawn(listening);
    }

    let Some((mapping, slot)) = world.resource_mut::<Walk>().remaining.pop() else {
        println!("\nThat is every slot. Close the window.");
        return;
    };

    let Some(session) = CaptureSession::for_slot(&mapping, slot) else {
        // A stick or a mouse bound whole: no single control can fill it, so there is nothing to
        // capture. Design §9.7 gives those a tunable rather than a rebinding row.
        println!(
            "\n{} — no single control can fill this; skipping",
            mapping.key
        );
        next(world);
        return;
    };

    println!(
        "\n{} [{:?}] {} — the row holds {}. Press a control, or Escape to skip.",
        mapping.key.fallback_label(),
        mapping.scheme,
        column(slot, &mapping),
        bound(&mapping),
    );

    // Escape is kept out of it so that it can go on meaning "not this one". A control capture
    // ignores is a control that still works, which is the whole purpose of an exclusion list.
    let listening = world
        .spawn(session.excluding([Control::Key(KeyCode::Escape)]))
        .id();
    world.resource_mut::<Walk>().listening = Some(listening);
}

/// What the whole row holds, which is more than one thing once a mapping has a secondary.
///
/// Joining is the app's business rather than the crate's: `fallback_label` answers for one control,
/// and how a screen lays several of them out is a layout decision no crate should be making.
fn bound(mapping: &mapping::Mapping) -> String {
    if mapping.slots.is_empty() {
        return "nothing".into();
    }
    mapping
        .slots
        .iter()
        .map(|control| control.fallback_label().into_owned())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Which column of the row this slot is, in the words a table would put at the top of it.
fn column(slot: usize, mapping: &mapping::Mapping) -> String {
    match (slot, mapping.capacity.slots()) {
        (_, Some(1)) => "the only slot".into(),
        (0, _) => "primary".into(),
        (1, _) => "secondary".into(),
        (n, _) => format!("slot {}", n + 1),
    }
}

fn took(captured: On<Captured>, mut commands: Commands) {
    let control = captured.control;
    let mapping = captured.mapping;
    println!(
        "  captured {} into slot {} — stored as `{}`",
        control.fallback_label(),
        captured.slot + 1,
        control.name(),
    );

    commands.queue(move |world: &mut World| {
        // Asked afterwards rather than carried on the event. Answering it means reading every
        // declared context, which capture cannot do from the middle of the input pipeline — and it
        // is the caller's question anyway, since what to *do* about a clash is a policy.
        for clash in conflicts(world, control, mapping) {
            let certainty = match clash.overlap {
                Overlap::SameContext => {
                    "in this same context, so they are certainly in each other's way"
                }
                Overlap::OtherContext => {
                    "in another context, which may never be live at the same time"
                }
            };
            println!("  ! `{}` already holds it — {certainty}", clash.mapping);
        }
        next(world);
    });
}

fn would_not_take(refused: On<Refused>) {
    let why = match refused.reason {
        RefusedReason::Reserved => {
            "reserved — it opens this screen, so nothing may be bound over it"
        }
        RefusedReason::Scheme => {
            "wrong device — a mapping is rebound within its own control scheme"
        }
        RefusedReason::Shape => "wrong kind of control for what this mapping drives",
    };
    println!("  x {} — {why}", refused.control.fallback_label());
}

/// Escape reaches this because capture was told to leave it alone.
///
/// Read from Bevy's own button state rather than through an action, to make the point: there is no
/// context spawned in this example at all, and the exclusion still works, because capture neither
/// takes an excluded control nor swallows it.
fn skip(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands, walk: Option<Res<Walk>>) {
    if walk.is_some_and(|walk| walk.listening.is_some()) && keys.just_pressed(KeyCode::Escape) {
        commands.queue(|world: &mut World| {
            println!("  skipped");
            next(world);
        });
    }
}
