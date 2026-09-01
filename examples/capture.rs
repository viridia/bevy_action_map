//! Rebinding by hand: walk a game's mappings and press something for each one.
//!
//! Run it and read the console: `cargo run --example capture`.
//!
//! It takes each mappable slot in turn, listens for a control, reports what it heard — including
//! what that control already does elsewhere — and then **binds it**, so each answer is visible in
//! the row the next question prints.
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
//! Two lines print after every rebind, and both are the point. `capture_demo.wall_jump` rides
//! Jump's row rather than having one of its own, so rebinding Jump moves it too — two actions
//! declared as sharing a control go on sharing one. And the line under it says what the *game*
//! still ships, unchanged, because an override is a diff: a patch that revises a default reaches
//! every player who never touched that row.
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
use bevy_action_map::overrides::{Overrides, apply_overrides};
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

#[derive(InputAction)]
#[action(path = "capture_demo.wall_jump", output = bool, intent = Button, category = "capture_demo.actions")]
struct WallJump;

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

        // Jump held rather than tapped, which is a second action on a control the player is already
        // being shown. It rides Jump's row instead of getting one of its own — and when Jump is
        // rebound below, watch it move too. That is the whole reason `follow` exists: two actions
        // declared as sharing a control have to go on sharing one.
        //
        // One call covers both of Jump's keyboard bindings, generated from them rather than
        // retyped. Called here, before Jump's pad binding below is declared, is what keeps
        // WallJump off the pad: `follow` only sees what its leader has declared so far.
        controls.follow::<WallJump, Jump>(|binding| binding.hold(0.4));

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
    /// The row the live session is asking about. A settings screen answers this from wherever it
    /// put the session — usually the cell the player activated — rather than keeping it here.
    asking: Option<(mapping::Mapping, usize)>,
}

/// Everything the player has changed so far.
///
/// A plain value the crate hands back rather than something it owns, so a game keeps it wherever it
/// keeps the rest of its settings. This example keeps it in a resource of its own and never writes
/// it anywhere; a shipped game would serialize exactly this.
#[derive(Resource, Default)]
struct Chosen(Overrides);

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
        asking: None,
    });
    world.init_resource::<Chosen>();
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

    let Some((stale, slot)) = world.resource_mut::<Walk>().remaining.pop() else {
        println!("\nThat is every slot. Close the window.");
        return;
    };
    // Re-read the row rather than trusting the copy taken when the walk was planned: `mappings`
    // answers with what is bound *now*, and by this point the player may have changed it.
    let mapping = current(world, &stale);

    let Some(session) = CaptureSession::for_slot(&mapping, slot) else {
        // A stick or a mouse bound whole: no single control can fill it, so there is nothing to
        // capture. docs/design.md §9.1 gives those a tunable rather than a rebinding row.
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
    let mut walk = world.resource_mut::<Walk>();
    walk.listening = Some(listening);
    walk.asking = Some((mapping, slot));
}

/// The row `stale` has become, or `stale` itself if this build no longer declares it.
///
/// Matched on scheme as well as name, because one name means one thing on the keyboard and another
/// on the pad — `capture_demo.jump` is two rows, rebound independently.
fn current(world: &World, stale: &mapping::Mapping) -> mapping::Mapping {
    mapping::mappings(world)
        .into_iter()
        .find(|row| row.key == stale.key && row.scheme == stale.scheme)
        .unwrap_or_else(|| stale.clone())
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
        rebind(world, control);
        next(world);
    });
}

/// Writes the captured control into the player's set and makes the game agree with it.
///
/// The two halves of a rebind, and they are separate on purpose: the set is the app's to keep and
/// [`apply_overrides`] is what a running game hears about it. A settings screen with a Confirm
/// button edits the first for as long as it likes and calls the second once.
fn rebind(world: &mut World, control: Control) {
    let Some((row, slot)) = world.resource_mut::<Walk>().asking.take() else {
        return;
    };

    // A row is written whole, so the slot-level edit — "put this in the secondary" — happens here,
    // against the list the row currently holds. The crate's unit is the row; the cell is the
    // screen's.
    let mut controls = row.slots.clone();
    if slot < controls.len() {
        controls[slot] = control;
    } else {
        controls.push(control);
    }

    let mut chosen = world.remove_resource::<Chosen>().unwrap_or_default();
    chosen.0.bind(row.scheme, row.key, controls);
    let problems = apply_overrides(world, &chosen.0);
    world.insert_resource(chosen);

    for problem in &problems {
        println!(
            "  ! `{}` was not applied: {:?}",
            problem.mapping, problem.kind
        );
    }

    let now = current(world, &row);
    println!("    the row now holds {}", bound(&now));
    // The rider moved with it, which is the difference between rebinding a control and rebinding
    // one of the two actions that read it.
    for follower in &now.followers {
        println!(
            "    …and `{}` rides it: {}",
            follower.action_path,
            now.slots
                .iter()
                .map(|held| follower.condition.fallback_format(&held.fallback_label()))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    // The declaration is untouched, which is what lets the next patch ship a revised default to
    // every player who never touched this row.
    let declared = mapping::declared_mappings(world)
        .into_iter()
        .find(|shipped| shipped.key == row.key && shipped.scheme == row.scheme);
    if let Some(declared) = declared {
        println!("    the game still ships {}", bound(&declared));
    }
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
