# bevy_action_map

An comprehensive input action manager for [Bevy](https://bevyengine.org).

Declare what your game reacts to, bind whatever devices should drive it, and let players change their minds later.

You define **actions** (`Jump`, `Move`, `Fire`) and **contexts** (`OnFoot`, `InVehicle`, `MainMenu`)
as ordinary Rust types. You bind a mix of keyboard, mouse and gamepad controls to them, with
modifiers and conditions that decide how a hardware signal becomes a game-shaped one. Your gameplay code
then reads `Move` as a `Vec2` and never again mentions `WASD`, a stick, or a dead zone — and when a
player wants to rebind `Jump` to a different key, the crate already has everything it needs to show
them what is bound, let them change it, and keep every prompt on screen in sync.

> **Status: early and public for review, not for production.** The core mapping pipeline —
> keyboard, mouse, gamepad, modifiers, conditions, arbitration, fixed/render tick handling — is
> built and exercised by a real game (below). The player-facing rebinding UI works end to end for
> that game but hasn't shipped a persistence format yet. There's no crate-level API documentation
> on docs.rs, and this crate isn't published to crates.io. See [Roadmap.md](./Roadmap.md) for what's
> done and what's left.

## Why

Input management entails more than just "map a `KeyCode` to an enum", because a
shipped game needs more than a mapping:

- **Devices disagree about what a value means.** A mouse delta and a stick deflection are both
  `Vec2`, but one already happened this frame and the other tells you which way to keep moving. In keeping with the Rust philosophy, these distinctions are represented as _types_: the crate tracks that distinction (an action's _intent_) so it can convert between the two correctly
  instead of leaving you to remember which is which at every call site.
- **Real games have more than one thing listening to the keyboard.** A pause menu, a chat box, and
  a player's ship shouldn't all react to `Escape`. Contexts have priority and consume input, so a
  higher-priority context can claim a control without the lower one ever knowing it happened.
- **Fixed-timestep gameplay drops input if you're not careful.** A press-and-release inside one
  render frame is invisible to `FixedUpdate` unless something remembers it happened. The crate
  queues timestamped events and drains them by time window, so a fixed tick sees every edge transition exactly
  once, however many (or few) times it runs between renders.
- **Players expect to rebind things, and that's usually bolted on later.** The same
  binding declarations that drive gameplay also generate the list a settings screen shows, which
  controls are changeable, and visible prompts ("Press W") that stay correct after a rebind.

## Features

- **Actions and contexts as types.** `#[derive(InputAction)]` and `#[derive(InputContext)]` give you
  compile-time checked reads (`input.value::<Move>()` returns `Vec2`) and
  a declared, stable name for each — the identity that survives a rename or a save file.
- **Keyboard, mouse, and gamepad**, each an optional feature, sharing one pipeline. `no_std` at the
  core (`alloc` only), so the mapping logic itself doesn't require `std`.
- **Multiple bindings per action**, folded together — chords (`Ctrl+S`), alternatives (`Space` or
  gamepad South), and composites (WASD as one `Vec2`) all resolve through the same arbitration.
- **Modifiers**: dead zones, response curves, scale, negate, swizzle, clamping, and rate conversion
  (turning a stick's _position_ into the same per-frame _delta_ a mouse reports).
- **Conditions**: press, release, hold, tap, multi-tap, pulse — composed the way Unreal's triggers
  are, as "any of these" / "all of these" / "none of these must hold."
- **Context activation and priority.** A context can be tied to a game state, a Bevy run condition, or
  driven by hand; a higher-priority context consumes a control before a lower one ever sees it.
- **Fixed and render tick domains**, with a windowed event drain so fixed-timestep gameplay loses no
  edges and duplicates none, whatever the frame rate is doing.
- **Read actions by polling or by observer** — `Actions<C>` in a system, or `On<Fired<Jump>>` as an
  entity event, whichever fits the call site.
- **A player-facing mapping model**, derived from the same bindings gameplay uses: which controls
  are shown, which are changeable, primary/secondary slots, two actions sharing one control on
  purpose (tap to dodge, hold to sprint — rebind the control, not either action).
- **Interactive rebinding capture** with conflict detection, reserved controls, and live text
  prompts that update themselves when a binding changes.
- **Diagnostics that answer "why didn't this fire?"** — inactive context, a higher-priority consumer,
  a longer chord winning, an unmet condition, or a device that isn't this player's.

See [Roadmap.md](./Roadmap.md)'s "Where this stands" table for the precise, current line between
built and not-yet.

## Quick start

```rust,ignore
use bevy::prelude::*;
use bevy_action_map::prelude::*;

#[derive(InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
struct Jump;

#[derive(InputContext)]
#[context(path = "gameplay.on_foot", tick = Render)]
struct OnFoot;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ActionMapPlugin))
        .add_context::<OnFoot>(|context| {
            context.bind::<Jump>(KeyCode::Space);
        })
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(OnFoot);
        })
        .add_systems(Update, print_jump)
        .run();
}

fn print_jump(input: Actions<OnFoot>) {
    if input.fired::<Jump>() {
        println!("Jump fired");
    }
}
```

Run it: `cargo run --example minimal`.

## Concepts

### Actions: what, not how

An action is a type, not a value — `Jump`, `Move`, `Look`. `#[derive(InputAction)]` declares its
**output** (the Rust type your gameplay reads: `bool`, `f32`, `Vec2`, `Vec3`) and its **intent**,
which says what that value _means_:

| Intent         | Meaning                                         | Typical source          |
| -------------- | ----------------------------------------------- | ----------------------- |
| `Button`       | digital, on or off                              | a key, a gamepad button |
| `Analog1`      | a single continuous value                       | a trigger               |
| `Directional2` | a position implying a direction to keep moving  | a stick, WASD           |
| `Delta2`       | a displacement that already happened this frame | mouse motion            |

Intent matters because shape alone can't distinguish a stick from a mouse — both are `Vec2` — but
mixing them up produces camera code that either drifts on its own or never catches up. Modifiers like
`.per_second()` convert between the two explicitly, at the binding, instead of leaving it implicit at
the read site.

Every action also declares a **path** — `"gameplay.jump"` — which is the name that ends up in a
settings file. It doesn't have to match the Rust type name, and shouldn't be updated when the type is
renamed; that stability is the point of declaring it separately.

### Contexts: what's listening right now

A context groups the bindings that are active together: `OnFoot`, `InVehicle`, `MainMenu`. You assign
one to an entity — the player, or a bare entity for input that isn't tied to anything in particular —
and that entity holds the live state for every action in the context. Local multiplayer falls out
of this for free: each player's entity has its own context instance, so nobody shares state.

A context can be always-on, tied to a `bevy_state` state, driven by any run condition, or flipped by
hand. Contexts also have a priority: while a settings screen's context is active and consumes the
arrow keys for navigation, a lower-priority gameplay context never sees them move the ship — no
manual "pause gameplay input" bookkeeping required.

### Bindings: modifiers and conditions

A binding pairs a control with the action it drives, and reads left to right as a pipeline:

```rust,ignore
context.bind::<Move>(Stick::Left).dead_zone(DeadZone::radial(0.15));
context.bind::<Look>(Stick::Right).curve(1.8).per_second(180.0);
context.bind::<Charge>(KeyCode::Space).hold(0.4);
```

**Modifiers** reshape the raw value — dead zone, response curve, scale, clamp — before it becomes the
action's value. **Conditions** decide _when_ a binding counts as firing: without one, a binding fires
whenever its control is off rest; `.hold(0.4)` instead waits for half a second, reporting progress as
it builds so a UI can show a charge meter. Several bindings can feed one action, and the plan folds
them by specificity, so `Ctrl+S` beats a plain `S` bound in the same context without either binding
knowing about the other.

### Reading actions: poll or observe

```rust,ignore
fn movement(input: Actions<OnFoot>) {
    let dir = input.value::<Move>();     // Vec2, checked at compile time
    if input.fired::<Jump>() { /* ... */ }
}

fn on_jump(_: On<Fired<Jump>>) {
    // an entity event, for code that would rather react than poll
}
```

Every action has a **phase** each tick — `Idle`, `Started`, `Ongoing`, `Fired`, `Completed`,
`Canceled` — so a hold that's building, a hold that just fired, and a hold released too early are all
distinguishable, whether you read it by polling `Actions<C>` or by listening for `Fired<A>` /
`Started<A>` / `Completed<A>` / `Canceled<A>` as entity events on the context's own entity.

### The player-facing side: mapping and rebinding

The binding API above is a developer's model — dead zones and response curves are implementation
detail nobody rebinding "move forward" should have to think about. Marking a binding `.mappable()`
adds it to a smaller, player-facing model instead: a named **mapping** with an ordered list of slots
("Primary", "Secondary"), which a settings screen walks without needing to know anything else about
your action or binding declarations. From there the crate can show what's bound, capture a new
control interactively (with conflict detection against everything else in the context), and keep any
on-screen prompt ("Press W") correct across a rebind — see the `mapping` and `present` modules, and
`examples/disasteroids/settings.rs` for a full rebinding screen operable from a gamepad.

## A fuller example

Two device classes, two contexts (one on the fixed tick for gameplay, one on the render tick for
camera look), dead zones and a stick-to-mouse-equivalent rate conversion:

```rust,ignore
use bevy::prelude::*;
use bevy_action_map::prelude::*;
use bevy_input::{gamepad::GamepadButton, keyboard::KeyCode};

#[derive(InputAction)]
#[action(path = "gameplay.move", output = Vec2, intent = Directional2)]
struct Move;

#[derive(InputAction)]
#[action(path = "gameplay.look", output = Vec2, intent = Delta2)]
struct Look;

#[derive(InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
struct Jump;

#[derive(InputContext)]
#[context(path = "gameplay.on_foot", tick = Fixed)]
struct OnFoot;

#[derive(InputContext)]
#[context(path = "gameplay.free_look", tick = Render)]
struct FreeLook;

fn main() {
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, ActionMapPlugin));
    app.add_context::<OnFoot>(|context| {
        context.bind::<Move>(DirectionalButtons::wasd());
        context.bind::<Move>(Stick::Left).dead_zone(DeadZone::radial(0.15));
        context.bind::<Jump>(KeyCode::Space);
        context.bind::<Jump>(GamepadButton::South);
    });
    app.add_context::<FreeLook>(|context| {
        context.bind::<Look>(MouseMove);
        context.bind::<Look>(Stick::Right)
            .dead_zone(DeadZone::radial(0.12))
            .curve(1.8)
            .per_second(180.0);
    });
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn(OnFoot);
        commands.spawn(FreeLook);
    });
    app.add_systems(FixedUpdate, move_player);
    app.add_systems(Update, look_camera);
    app.run();
}

fn move_player(input: Actions<OnFoot>) {
    let dir = input.value::<Move>();
    if input.fired::<Jump>() { /* ... */ }
}

fn look_camera(input: Actions<FreeLook>) {
    let delta = input.value::<Look>();
}
```

Run it: `cargo run --example move_and_jump`.

### Disasteroids

`examples/disasteroids` is the crate's proving ground: a small, playable asteroids-like game, driven
entirely through this crate, keyboard or gamepad. Its input layer — seven actions, two gameplay
contexts, and every binding — lives in `examples/disasteroids/actions.rs`. Its `F2`/pad-Y settings screen is a real rebinding UI, with its own
context: it lists every binding without being told about any of them, can be navigated end to end
from a gamepad, and applies a rebind live.

```sh
cargo run --example disasteroids
```

Fly with `W`/↑ and `A`/`D` (or ←/→), fire with `Space`, jump with `Left Shift`, pause with `Escape`.

## Other examples

| Example         | Shows                                                                  |
| --------------- | ---------------------------------------------------------------------- |
| `minimal`       | The smallest possible setup                                            |
| `move_and_jump` | Two device classes, two tick domains, dead zones, a rate conversion    |
| `disasteroids`  | A full game with a rebinding settings screen                           |
| `capture`       | Interactive rebind capture in isolation, without a full game around it |
| `diagnostics`   | What a bad binding declaration reports, and when                       |

## Installing

Not on crates.io yet. Depend on the git repository directly, and pin `Cargo.lock` the way this repo
does — the Bevy dependencies are git dependencies with no `rev`, so an unpinned `cargo update` can
pull a Bevy commit this crate hasn't been built against:

```toml
[dependencies]
bevy_action_map = { git = "https://github.com/viridia/bevy_action_map" }
```

Default features are `std`, `bevy_reflect`, `keyboard`, `mouse`, `gamepad`, and `state`. `touch` and
`focus` are opt-in; `serialize` adds `serde` support for overrides; a `no_std` build needs
`--no-default-features --features libm` to give `glam` a math backend. See `[features]` in
[Cargo.toml](./Cargo.toml) for the complete list.

## Project documents

This crate is being built from a written requirements and design process, kept in the repository
rather than in an issue tracker, in the order a reviewer would actually want to read them:

| Document                                                | What it is                                                                                                                    |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| [Requirements.md](./Requirements.md)                    | ~204 numbered requirements, with prior art surveyed from LWIM, `bevy_enhanced_input`, Unreal, Unity, Steam Input, and Godot   |
| [Design.md](./Design.md)                                | How the requirements are satisfied — architecture, data flow, object model, evaluation pipeline, developer-experience surface |
| [Roadmap.md](./Roadmap.md)                              | What's built, in what order, and what's left — **start here to see current status**                                           |
| [Log.md](./Log.md) / [Log-archive.md](./Log-archive.md) | What each increment of work delivered and learned                                                                             |

## License

Dual-licensed under MIT or Apache-2.0, at your option, as declared in [Cargo.toml](./Cargo.toml).
