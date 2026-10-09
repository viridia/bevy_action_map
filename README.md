# bevy_action_map

[![crates.io](https://img.shields.io/crates/v/bevy_action_map.svg)](https://crates.io/crates/bevy_action_map)
[![docs.rs](https://docs.rs/bevy_action_map/badge.svg)](https://docs.rs/bevy_action_map)
[![Following released Bevy versions](https://img.shields.io/badge/Bevy%20tracking-released%20version-lightblue)](https://bevy.org/learn/quick-start/plugin-development/#main-branch-tracking)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

A comprehensive input action manager for [Bevy](https://bevyengine.org).

Declare what your game reacts to, bind whatever devices should drive it, and let players change their minds later.

You define **actions** (`Jump`, `Move`, `Fire`) and **contexts** (`OnFoot`, `InVehicle`, `MainMenu`)
as ordinary Rust types. You bind a mix of keyboard, mouse and gamepad controls to them, with
modifiers and conditions that decide how a hardware signal becomes a game-shaped one. Your gameplay
code then reads `Move` as a `Vec2` and never again mentions `WASD`, a stick, or a dead zone — and
when a player wants to rebind `Jump` to a different key, the crate already has everything it needs
to show them what is bound, let them change it, and keep every prompt on screen in sync.

## Why

![settings][settings-screenshot]

A shipped game needs more from its input layer than a map from `KeyCode` to an enum:

- **Devices disagree about what a value means.** A mouse delta and a stick deflection are both
  `Vec2`, but one already happened this frame and the other tells you which way to keep moving. The
  crate tracks that distinction as a type (an action's _intent_) and converts between the two
  correctly, instead of leaving you to remember which is which at every call site.
- **Real games have more than one thing listening to the keyboard.** A pause menu, a chat box, and
  a player's ship shouldn't all react to `Escape`. Contexts have priority and consume input, so a
  higher-priority context can claim a control without the lower one ever knowing it happened.
- **Fixed-timestep gameplay drops input if you're not careful.** A press-and-release inside one
  render frame is invisible to `FixedUpdate` unless something remembers it happened. The crate
  queues timestamped events and each context reads on from where it left off, so a fixed tick sees
  every edge transition exactly once, however many (or few) times it runs between renders. An action
  bound to an outside authority such as Steam Input is the exception: the authority reports a level
  once a frame, so two edges inside one frame are lost, though the fixed tick still sees each edge
  that arrives.
- **Players expect to rebind things, and that's usually bolted on later.** The same
  binding declarations that drive gameplay also generate the list a settings screen shows, which
  controls are changeable, and visible prompts ("Press W") that stay correct after a rebind.

## Features

- **Actions and contexts as types.** `#[derive(InputAction)]` and `#[derive(InputContext)]` give you
  compile-time checked reads (`input.value::<Move>()` returns `Vec2`) and
  a declared, stable name for each — the identity that survives a rename or a save file.
- **Keyboard, mouse, and gamepad**, each an optional feature, sharing one pipeline. `no_std` at the
  core (`alloc` only), so the mapping logic itself doesn't require `std`.
- **Multiple bindings per action**, combined — chords (`Ctrl+S`), alternatives (`Space` or
  gamepad South), and composites (WASD as one `Vec2`) all resolve through the same arbitration. A
  binding can also name a whole class of control, which is how "press anything to join" and a text
  field claiming every character key are written.
- **Modifiers**: dead zones, response curves, scale, negate, swizzle, clamping, and rate conversion
  (turning a stick's _position_ into the same per-frame _delta_ a mouse reports).
- **Conditions**: press, release, hold, tap, multi-tap, pulse — composed the way Unreal's triggers
  are, as "any of these" / "all of these" / "none of these must hold."
- **Context activation and priority.** A context can be tied to a game state, a Bevy run condition,
  or driven by hand; a higher-priority context consumes a control before a lower one ever sees it,
  and an exclusive one shuts out everything beneath it for as long as it is up. A single action can
  be switched off without touching its bindings, such as a serve held back during a countdown.
- **Actions driven from code.** An action can be bound to an outside authority in place of one
  device family's controls, so a platform input service, an AI opponent or a scripted sequence
  writes its value and the rest of the game reads it like any other. Under Steam Input the pad comes
  from Steam while the keyboard stays bound as usual.
- **Fixed and render tick domains**, reading one event queue, so fixed-timestep gameplay loses no
  edges and duplicates none, whatever the frame rate is doing.
- **Read actions by polling or by observer** — `ContextActions<C>` in a system, or `On<Fired<Jump>>`
  as an entity event, whichever fits the call site.
- **Rebinding, end to end.** The same declarations gameplay uses generate a settings model — which
  controls are shown, which are changeable, primary and secondary slots, two actions deliberately
  sharing one control (tap to dodge, hold to sprint, rebind the control rather than either action).
  Capture takes a new control interactively, with conflict detection and reserved controls a game
  never hands out, and every on-screen prompt updates itself. Prompts read a pad in its own words:
  "Cross" on a DualSense, "A" on an Xbox pad.
- **Saved overrides** in a documented, human-editable format that resolves against what the game
  currently declares — a binding saved by an older build reports itself rather than disappearing.
  Writing the bytes is left to whatever settings layer you already use.
- **Presets and tunables.** Ship alternate control families (`Southpaw`, `Classic`) as named presets
  rather than making players rebind row by row, and expose the settings that aren't controls at all
  — sensitivity, invert-Y, hold-versus-toggle — through the same screen.
- **Local co-op.** Give each player's entity its own context instance and pair it to their devices,
  and neither player sees the other's input. Players join by pressing anything on an unclaimed
  device, and rebind independently without either becoming a default the next player inherits.
- **Diagnostics that answer "why didn't this fire?"** — inactive context, a higher-priority consumer,
  a longer chord winning, an unmet condition, or a device that isn't this player's.

See [Roadmap.md][roadmap] for what is not built yet.

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

fn print_jump(input: ContextActions<OnFoot>) {
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
mixing them up produces camera code that either drifts on its own or never catches up. A modifier
such as `.per_second()` turns a stick's position into a delta explicitly, at the binding, instead of
leaving the conversion implicit at the read site.

Every action also declares a **path** — `"gameplay.jump"` — which is the name that ends up in a
settings file. It doesn't have to match the Rust type name, and shouldn't be updated when the type is
renamed; that stability is the point of declaring it separately.

### Contexts: what's listening right now

A context groups the bindings that are active together: `OnFoot`, `InVehicle`, `MainMenu`. You
assign one to an entity — the player, or a bare entity for input that isn't tied to anything in
particular — and that entity holds the live state for every action in the context. Local multiplayer
starts here: each player's entity gets its own context instance, so nobody shares state, and pairing
that instance to a set of devices is what stops player two's stick moving player one. A context with
no pairing reads every device, so a single-player game never has to mention any of this.

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
it builds so a UI can show a charge meter. Several bindings can feed one action, and the crate
resolves them by specificity, so `Ctrl+S` beats a plain `S` bound in the same context without either
binding knowing about the other.

A binding's control doesn't have to be a single input: `DirectionalButtons::wasd()` combines four
keys into one **composite**, so `Move` reads a single `Vec2` instead of four buttons the game code
would otherwise have to assemble itself.

A keyboard binding says which of the two things it means. `KeyCode::KeyW` is a position, which is
what movement wants — WASD is a shape under the left hand, and it should stay that shape on an
AZERTY board. `LogicalKey('z')` is the character, which is what an editor-style shortcut wants, so
`Ctrl+Z` lands on the key a French player actually reads as `z`.

### Reading actions: poll or observe

```rust,ignore
fn movement(input: ContextActions<OnFoot>) {
    let dir = input.value::<Move>();     // Vec2, checked at compile time
    if input.fired::<Jump>() { /* ... */ }
}

fn on_jump(_: On<Fired<Jump>>) {
    // an entity event, for code that would rather react than poll
}
```

Every action has a **phase** each tick — `Idle`, `Started`, `Building`, `Fired`, `Firing`,
`Completed`, `Canceled` — so a hold that's building, a hold that just fired, and a hold released too
early are all distinguishable, whether you read it by polling `ContextActions<C>` or by listening
for `Fired<A>` / `Started<A>` / `Completed<A>` / `Canceled<A>` as entity events on the context's own
entity.

### Presentation: mapping and rebinding

The binding API above is a developer's model — dead zones and response curves are implementation
detail nobody rebinding "move forward" should have to think about. Marking a binding `.mappable()`
adds it to a smaller presentation model instead: a named **mapping** with an ordered list of slots
("Primary", "Secondary"), which a settings screen walks without needing to know anything else about
your action or binding declarations. From there the crate can show what's bound, capture a new
control interactively (with conflict detection against everything else in the context), and keep any
on-screen prompt ("Press W") correct across a rebind — see the `mapping` and `present` modules, and
`examples/disasteroids/settings.rs` for a full rebinding screen operable from a gamepad.

## A fuller example

Two device classes, two contexts (one on the fixed tick for gameplay, one on the render tick for
camera look), dead zones, and a rate conversion that lets a stick drive the same look action as the
mouse:

```rust,ignore
use bevy::prelude::*;
use bevy_action_map::prelude::*;

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
        // W and D together read (1, 1); clamped, a diagonal is no faster than a straight line.
        context.combined::<Move>().clamp_magnitude();
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

fn move_player(input: ContextActions<OnFoot>) {
    let dir = input.value::<Move>();
    if input.fired::<Jump>() { /* ... */ }
}

fn look_camera(input: ContextActions<FreeLook>) {
    let delta = input.value::<Look>();
}
```

Run it: `cargo run --example move_and_jump`.

### Disasteroids

`examples/disasteroids` is the crate's proving ground: a small, playable asteroids-like game, driven
entirely through this crate, keyboard or gamepad. Its whole input layer lives in
`examples/disasteroids/actions.rs`: one context for flying, one for the controls that work whatever
the game is doing, and an exclusive one for menus. Nothing else in the game mentions a key or a
button. Its `F2`/pad-Y settings screen is a real rebinding UI: it lists every binding without being
told about any of them, can be navigated end to end from a gamepad, and applies a rebind live.

```sh
cargo run --features serialize --example disasteroids
```

Thrust with `W`/↑ (hold it for the afterburner), turn with `A`/`D` or ←/→, fire with `Space` or the
left mouse button, double-tap `Left Shift` for hyperspace, hold `B` to charge a smart bomb, and
pause with `Escape`. What you rebind is written to a settings file and applied again the next time
you launch, which is what the `serialize` feature is for.

## Other examples

Every example runs from a clean checkout with `cargo run --example <name>`, and between them they
exercise every part of the crate. The three games are where it is worth starting; the rest are
single-concept demos small enough to read in one sitting, two of them built on Pong. The two that
keep something between runs need `--features serialize` as well, marked below.

| Example           | Shows                                                                     |
| ----------------- | ------------------------------------------------------------------------- |
| `disasteroids`    | A full game with a rebinding settings screen, keyboard or gamepad (`serialize`) |
| `split_friction`  | Split-screen co-op: two players, two cameras, a device paired to each (`serialize`) |
| `pong`            | Two players on one machine, the second pad claimed the moment it appears   |
| `pong_countdown`  | Pong with a countdown before each serve: an action switched off, not unbound |
| `pong_robot`      | Pong against a robot paddle whose action is written by code, not a device |
| `minimal`         | The smallest possible setup                                               |
| `move_and_jump`   | Two device classes, two tick domains, dead zones, a rate conversion        |
| `capture`         | Interactive rebind capture on its own, without a game around it            |
| `text_field`      | A focused text field claiming every character key beside live gameplay     |
| `prompt_gallery`  | Every kind of prompt as text and as icons, under each pad brand and preset |
| `diagnostics`     | What a bad binding declaration reports, and when — no window, no `App`     |
| `ime_diagnostic`  | What a keypress really carries while an IME is composing                   |

## Installing

It targets Bevy 0.20.

```toml
[dependencies]
bevy_action_map = "0.1"
```

Default features are `std`, `bevy_reflect`, `keyboard`, `mouse`, `gamepad`, and `state`. `serialize`
adds `serde` support for overrides. `touch` is opt-in and reserved for touch input, which is not
implemented yet. A `no_std` build needs `--no-default-features --features libm` to give `glam` a
math backend. See `[features]` in [Cargo.toml][cargo-toml] for the complete
list.

## Project documents

This crate is being built from a written requirements and design process, kept in the repository
rather than in an issue tracker. Each document answers one question:

| Document | What it is |
| --- | --- |
| [docs/design.md][design] | How the crate works: architecture, the input frame, evaluation, state, the presentation surface, persistence |
| [docs/decisions.md][decisions] | Why it works that way: the decisions expensive to reverse, each with what it rules out and what reversing it would cost |
| [Roadmap.md][roadmap] | What's left and what's broken. **Start here to see current status** |
| [Requirements.md][requirements] | The numbered requirements, with prior art surveyed from LWIM, `bevy_enhanced_input`, Unreal, Unity, Steam Input, and Godot |

One more, for readers who want the comparison rather than the specification:

| Document | What it is |
| --- | --- |
| [docs/comparison.md][comparison] | For choosing an input crate: how this one differs from `bevy_enhanced_input` and `leafwing-input-manager`, claim by claim, checked against both crates' source, and why most projects today should pick `bevy_enhanced_input` |
## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE][license-apache])
- MIT license ([LICENSE-MIT][license-mit])

at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in this crate by you shall be dual-licensed as above, without any additional terms or
conditions.

<!-- Absolute, because crates.io resolves a relative link against the crate's own directory. -->

[settings-screenshot]: https://github.com/viridia/bevy_action_map/raw/main/images/disasteroids_settings.png
[roadmap]: https://github.com/viridia/bevy_action_map/blob/main/Roadmap.md
[requirements]: https://github.com/viridia/bevy_action_map/blob/main/Requirements.md
[design]: https://github.com/viridia/bevy_action_map/blob/main/docs/design.md
[decisions]: https://github.com/viridia/bevy_action_map/blob/main/docs/decisions.md
[comparison]: https://github.com/viridia/bevy_action_map/blob/main/docs/comparison.md
[cargo-toml]: https://github.com/viridia/bevy_action_map/blob/main/crates/bevy_action_map/Cargo.toml
[license-apache]: https://github.com/viridia/bevy_action_map/blob/main/LICENSE-APACHE
[license-mit]: https://github.com/viridia/bevy_action_map/blob/main/LICENSE-MIT
