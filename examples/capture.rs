//! Rebinding by hand: walk a game's mappings and press something for each one.
//!
//! Run it and read the console: `cargo run --example capture`.
//!
//! It takes each mappable slot in turn, listens for a control, reports what it heard — including
//! what that control already does elsewhere — and then **binds it**, so each answer is visible in
//! the row the next question prints.
//!
//! A *slot* rather than a mapping, because a mapping holds an ordered list of them: `Jump` ships
//! two keyboard defaults and `Fire` ships one, so the walk visits Jump twice and then offers the
//! empty slot beside Fire's. That is the "primary and secondary" table every shipped game has,
//! before anything draws it — and how many cells to draw is the screen's own decision, not
//! something the game declares alongside the binding.
//!
//! Worth trying, because each is a case the crate has an opinion about:
//!
//! - press `Escape` — it skips the row instead of being captured, because it is *excluded*, and an
//!   excluded control goes on doing its normal job while a capture listens;
//! - press `F1` — refused out loud, because it opens the settings screen and is *reserved*;
//! - press a key that is already bound — captured, with the clash reported;
//! - press a **gamepad** button on a keyboard row, or a key on a gamepad row — refused, because a
//!   mapping is rebound within its own device family.
//!
//! Two lines print after every rebind, and both are the point. `capture_demo.wall_jump` rides
//! Jump's row rather than having one of its own, so rebinding Jump moves it too — two actions
//! declared as sharing a control go on sharing one. And the line under it says what the *game*
//! still ships, unchanged, because an override is a diff: a patch that revises a default reaches
//! every player who never touched that row.
//!
//! No context is ever spawned here, and nothing evaluates: this is a settings screen with no game
//! behind it.
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

        // Two mappable bindings of one action in one family are a default primary *and* secondary.
        // They derive the same mapping name on purpose: that is one row holding two controls, not
        // two rows both called Jump.
        controls.bind::<Jump>(KeyCode::Space).mappable();
        controls.bind::<Jump>(KeyCode::KeyJ).mappable();

        // One shipped control, and a row that can still take a second: the empty slot beside it is
        // the cell a settings screen chooses to draw blank.
        controls.bind::<Fire>(KeyCode::ControlLeft).mappable();

        // Jump held rather than tapped: a second action on a control the player is already being
        // shown, riding Jump's row instead of getting one of its own.
        //
        // One call covers both of Jump's keyboard bindings, generated from them rather than
        // retyped. Called here, before Jump's pad binding below is declared, is what keeps
        // WallJump off the pad: `follow` only sees what its leader has declared so far.
        controls.follow::<WallJump, Jump>(|binding| binding.hold(0.4));

        // The same two actions on the pad, mappable again. Both derive the same mapping name a
        // second time on purpose: `capture_demo.jump` means one thing on the keyboard and another
        // on the gamepad, the two are rebound independently (R19.7), and they are stored in
        // separate tables. Only a repeat *within* one family is a collision.
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
#[derive(Resource)]
struct Walk {
    remaining: Vec<(mapping::ActionMapping, usize)>,
    listening: Option<Entity>,
    /// The row the live session is asking about. A settings screen answers this from wherever it
    /// put the session — usually the cell the player activated — rather than keeping it here.
    asking: Option<(mapping::ActionMapping, usize)>,
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
    let mut remaining: Vec<(mapping::ActionMapping, usize)> = mapping::mappings(world)
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

/// Every slot this walk offers for a row: the ones holding something, plus the next empty one where
/// the row can take it.
///
/// One spare and no more is *this walk's* choice, not a rule the crate enforces — `for_slot` takes
/// whatever slot number it is handed and grows the row to reach it, exactly as a settings screen
/// with four columns would want. A console walk has no columns, so one at a time is what reads.
///
/// The exception is a row that is one direction of a composite: a second "forward" key is one part
/// of a second set of four, so those rows grow only when the whole composite does and the walk
/// stops at what they hold.
fn slots(mapping: &mapping::ActionMapping) -> std::ops::Range<usize> {
    let filled = mapping.slots.len();
    0..if mapping.key.part() == BindingPart::Whole {
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
        // capture. TD9.1 gives those a tunable rather than a rebinding row.
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
        mapping.family,
        column(slot),
        bound(&mapping),
    );

    // Escape is excluded so that it can go on meaning "not this one" — see `skip` below.
    let listening = world
        .spawn(session.excluding([Control::PhysicalKey(KeyCode::Escape)]))
        .id();
    let mut walk = world.resource_mut::<Walk>();
    walk.listening = Some(listening);
    walk.asking = Some((mapping, slot));
}

/// The row `stale` has become, or `stale` itself if this build no longer declares it.
///
/// Matched on family as well as name, because one name is a separate row per family.
fn current(world: &World, stale: &mapping::ActionMapping) -> mapping::ActionMapping {
    mapping::mappings(world)
        .into_iter()
        .find(|row| row.key == stale.key && row.family == stale.family)
        .unwrap_or_else(|| stale.clone())
}

/// What the whole row holds, which is more than one thing once a mapping has a secondary.
///
/// Joining is the app's business rather than the crate's: `fallback_label` answers for one control,
/// and how a screen lays several of them out is a layout decision no crate should be making.
fn bound(mapping: &mapping::ActionMapping) -> String {
    if mapping.slots.is_empty() {
        return "nothing".into();
    }
    mapping
        .slots
        .iter()
        .map(|slot| {
            // A slot the player emptied, with something still bound after it. Printed rather than
            // skipped, so "the row holds —, J" says which column the J is in.
            slot.as_ref().map_or_else(|| String::from("—"), held)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// One slot: whatever has to be held alongside the control, then the control.
///
/// A row that printed only the control would call `Ctrl+N` "N", which is worse than terse — the
/// walk would be telling the player a key that does nothing on its own.
fn held(slot: &mapping::BoundSlot) -> String {
    let mut text = String::new();
    for entry in &slot.with {
        text.push_str(&entry.fallback_label());
        text.push('+');
    }
    text.push_str(&slot.control.fallback_label());
    text
}

/// Which column of the row this slot is, in the words a table would put at the top of it.
fn column(slot: usize) -> String {
    match slot {
        0 => "primary".into(),
        1 => "secondary".into(),
        n => format!("slot {}", n + 1),
    }
}

fn took(captured: On<ControlCaptured>, mut commands: Commands) {
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
                ConflictOverlap::SameContext => {
                    "in this same context, so they are certainly in each other's way"
                }
                ConflictOverlap::OtherContext => {
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
    // screen's. Assignment rather than appending: the cell the player pressed is the cell that gets
    // the control, whether or not the row reaches that far yet. A row that does not is grown, and
    // the slots skipped on the way stay empty — writing to the third cell of a one-control row
    // gives a row of three with a blank in the middle, not a row of two.
    let mut controls = row.controls();
    if slot >= controls.len() {
        controls.resize(slot + 1, None);
    }
    controls[slot] = Some(control);

    let mut chosen = world.remove_resource::<Chosen>().unwrap_or_default();
    chosen.0.bind(row.family, row.key, controls);
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
                .flatten()
                .map(|slot| follower.condition.fallback_format(&held(slot)))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    // The declaration itself is untouched: an override is a diff over it.
    let declared = mapping::declared_mappings(world)
        .into_iter()
        .find(|shipped| shipped.key == row.key && shipped.family == row.family);
    if let Some(declared) = declared {
        println!("    the game still ships {}", bound(&declared));
    }
}

fn would_not_take(refused: On<CaptureRefused>) {
    let why = match refused.reason {
        RefusedReason::Reserved => {
            "reserved — it opens this screen, so nothing may be bound over it"
        }
        RefusedReason::Family => "wrong device — a mapping is rebound within its own device family",
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
