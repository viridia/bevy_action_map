# Backend-neutral gamepads for `bevy_input`

A concrete design for the gamepad layer discussed in
[bevy#25757](https://github.com/bevyengine/bevy/discussions/25757), with a working prototype. The
prototype runs on both `bevy_gilrs` and Steam Input and has been tested on macOS hardware, except
rumble under gilrs, which gilrs does not support on macOS and which is covered by tests only.

## The problem

`Gamepad` answers two questions: *is there a pad, and what is it?* and *what are its buttons and
sticks doing right now?* Only a backend that reports raw device events can answer the second. Steam
Input hands the game already-mapped actions instead, so a Steam pad either carries an empty
`Gamepad` that reads as untouched, or has no `Gamepad` and is missing from every query that lists
pads.

This also ties a game to one backend's coverage. gilrs has no force feedback on macOS, and reports
power and force-feedback support only on Linux and Windows. A game that wants a different backend on
some platform (Steam, SDL, a future pure-Rust library) has to rewrite every query that names
`Gamepad`.

## The proposal

`Gamepad` keeps its name and becomes a marker that every backend supplies. What it holds today moves
into two components that only a raw-event backend can supply. Two new components, `Brand` and
`Rumble`, work the same on every backend.

```rust
/// A connected gamepad, from any backend. `Add`/`Remove` on it are arrival and departure.
#[derive(Component)]
pub struct Gamepad;

/// Which conventions the pad follows, for prompts and glyphs.
#[derive(Component)]
pub struct Brand(pub GamepadBrand);
pub enum GamepadBrand { Xbox, PlayStation, Nintendo, Generic }

/// How hard the pad rumbles, for as long as this is set.
#[derive(Component)]
pub struct Rumble(pub GamepadRumbleIntensity);

// From a backend that reports raw device events:

/// The USB ids the device reported. Absent unless it reported both.
#[derive(Component)]
#[require(Gamepad)]
pub struct ModelId(pub GamepadModelId);

/// A model, not a unit: two identical pads report the same ids.
pub struct GamepadModelId { pub vendor: u16, pub product: u16 }

/// Live button and axis readings: today's `Gamepad`, renamed.
#[derive(Component)]
#[require(Gamepad)]
pub struct GamepadReadings { digital: ButtonInput<GamepadButton>, analog: Axis<GamepadInput> }
```

- **`Gamepad`** carries no data: the entity is the pad, and the other components describe it. On
  disconnect it is removed along with `ModelId` and `GamepadReadings`, and `GamepadSettings` stays,
  as it does today.
- **`Brand`** is resolved once, when the pad connects. `Generic` is the ordinary answer for a pad
  the backend cannot identify. A game that knows better inserts its own first, and the backend
  leaves it alone.
- **`Rumble`** is a level, not an effect: the pad holds it until it changes, and zero or removal
  stops it. There is one value per pad and the last write wins, so a game with several reasons to
  rumble combines them itself. `GamepadRumbleRequest` is timed and additive, so every game that
  rumbles "while X is true" currently rebuilds the same stop-then-restart bookkeeping. The request
  stays, for one-off buzzes.

## How each backend fills them

| | Raw events (`bevy_gilrs`) | Steam Input |
| --- | --- | --- |
| `Gamepad` | `bevy_input`'s connection system, as today | inserted on the entity the backend spawns per pad handle |
| `ModelId`, `GamepadReadings` | inserted beside it | absent |
| `Brand` | from the vendor id, through a `GamepadBrands` table the app can extend | from Steam's product family (`GetInputTypeForHandle`), which carries no vendor id |
| `Rumble` | read by `bevy_gilrs` | sent through `TriggerVibration`, which holds a level until it is changed |

The prototype cannot change `bevy_gilrs`, so its gilrs driver turns each change to `Rumble` into
`GamepadRumbleRequest::Stop` and `Add`.

## Connection and the entity

The entity outlives a connection. On disconnect a backend removes `Gamepad` and keeps the entity, so
whatever points at it survives: the game's record of which player holds the pad, or its
`GamepadSettings`. When the pad comes back, a backend that recognises it adds `Gamepad` to the same
entity, and a game holding a player's slot open resumes without asking them to rejoin.

How reliably a pad is recognised is the backend's to say:

- **gilrs** does not promise a returning pad its old id. No per-unit id survives an unplug
  ([gilrs#154](https://gitlab.com/gilrs-project/gilrs/-/work_items/154)), and on macOS a Bluetooth
  reconnect always gets a new one
  ([gilrs#207](https://gitlab.com/gilrs-project/gilrs/-/work_items/207)). An unrecognised pad gets a
  new entity, and the old one is left without `Gamepad`.
- **Steam Input** gave a returning pad the same handle, even after a switch from USB to Bluetooth,
  so a backend keyed on the handle recognises it. That is one pad measured. The handle also survived
  the game relaunching; if it survives a restart of Steam too, it is an identity a game could save.

`ModelId` cannot recognise a pad: it names a model, so two identical pads share one.

## Migrating

- **Listing pads, or observing them connect:** no change. `With<Gamepad>` and `On<Add<Gamepad>>` now
  also see other backends' pads.
- **Reading buttons and sticks:** `&Gamepad` becomes `&GamepadReadings`, with the same methods. List
  pads with `Gamepad` and read them with `GamepadReadings`: a rename alone compiles, but drops the
  pads of a backend without readings.
- **`vendor_id()` and `product_id()`** become `ModelId`, present only when both were reported. A
  call site working out a brand from them wants `Brand`.

## Not proposed

Capabilities (motion, touchpad, LED) and battery. Neither backend supplies enough to build on: gilrs
reports power and force-feedback support on Linux and Windows only, and Steam Input has no query for
either. One small step would help where gilrs can answer: `bevy_gilrs` forwarding gilrs's
`power_info()` and `is_ff_supported()`.

## The prototype

[`bevy_action_map`](https://github.com/viridia/bevy_action_map), an input-mapping crate. Its gamepad
module depends on Bevy alone and is written as it would sit upstream, except that it cannot rename
`Gamepad`, so it adds a `ConnectedGamepad` marker beside it.

| Prototype | Upstream |
| --- | --- |
| [`gamepad/mod.rs`](https://github.com/viridia/bevy_action_map/blob/main/crates/bevy_action_map/src/gamepad/mod.rs) | `bevy_input`: `Brand`, `Rumble`, and `ConnectedGamepad` as `Gamepad` |
| [`gamepad/gilrs.rs`](https://github.com/viridia/bevy_action_map/blob/main/crates/bevy_action_map/src/gamepad/gilrs.rs) | `GamepadModelId` and the brand table to `bevy_input`, the rumble driver to `bevy_gilrs`; its connection observers are replaced by the connection system inserting `Gamepad` |
| [`steam.rs`](https://github.com/viridia/bevy_action_map/blob/main/steam_examples/disasteroids/steam.rs) | not upstream: a third-party Steam backend filling the same components |

In the Disasteroids example, the ship rumbles the pad while it scrapes a rock, from one system that
sets `Rumble` on every `ConnectedGamepad` without naming a backend.
