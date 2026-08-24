# bevy_action_map

Input action mapping for Bevy: bindings, contexts, devices, and binding presentation.

Greenfield. Implementation has started: the module tree exists and is documented, the code that
fills it mostly does not. See the roadmap for what lands in what order.

## The documents

Read them in this order:

| Document                             | What it is                                                                                                                                                                                                      |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Requirements.md](./Requirements.md) | 204 numbered requirements across 24 areas, with prior art from LWIM, bevy_enhanced_input, Unreal, Unity, Steam Input, and Godot. Settled decisions are tagged `(D1)`…`(D7)`; requirements are `R<section>.<n>`. |
| [Design.md](./Design.md)             | How the requirements are to be satisfied: architecture, data flow, object model, evaluation pipeline, and the developer-experience surface. Commits to positions on the open questions.                         |
| [Roadmap.md](./Roadmap.md)           | What is left to build, in order. **Start here to write code.**                                                                                                                                                  |
| [Log.md](./Log.md)                   | What has been built and what building it taught us, from Phase VII on. Optional: the three above are self-contained, and this exists so they do not have to carry their own history. Read it when a decision looks arbitrary. Phases I–VI are in [Log-archive.md](./Log-archive.md). |

## Layout

A two-crate workspace. `bevy_action_map` is the root package; `macros/` is
`bevy_action_map_macros`, which exists only because Rust requires proc macros to live in their own
crate and is re-exported so users never name it. `tools/padprobe` declares its own `[workspace]` and
so stays detached from both.

## Building

The Bevy dependencies are git dependencies with no `rev` pin, so a plain `cargo check` will try to
fetch the whole Bevy repository and can look like it has hung for several minutes.

`Cargo.lock` pins Bevy at `17e28cd` (0.20-dev). Use it:

```sh
cargo check --offline     # instant; resolves from the lock
cargo check --locked      # allows fetching, but will not change the lock
```

Run `cargo update -p bevy` deliberately when you actually want to move to a newer Bevy, and expect
to re-verify §14's "Bevy's current behavior" notes in Requirements.md when you do — several of them
cite specific line numbers at `17e28cd`.

### The `no_std` build needs a math backend

The core is `no_std`, but `cargo check --no-default-features` on its own **fails** — glam takes its
math backend from a feature and gets it from `std` by default, so turning `std` off leaves it with
none and it refuses to compile. Name the replacement:

```sh
cargo check --offline --workspace --all-features
cargo check --offline --workspace --no-default-features --features libm
```

Those two are the configurations to keep green; the `libm` feature exists for the second and has no
other purpose.

## Hardware for testing

Gamepad support runs through [gilrs](https://docs.rs/gilrs/), which is what `bevy_gilrs` wraps. Not
every controller works, and the failure modes are not obvious. Verified on this machine (macOS):

| Controller                 | Result                                                                                                                                                                                                                                  |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Xbox Series, **Bluetooth** | ✅ Works fully. Use this.                                                                                                                                                                                                               |
| Xbox Series, **USB**       | ❌ Enumerates, but gilrs receives no values — macOS binds its own `com.apple.gamecontroller.driver.XboxGamepad` DriverKit dext and the GameController framework takes the input.                                                        |
| Switch-protocol clone, USB | ❌ Advertises a HID descriptor that does not match the report it sends. gilrs decodes the report's timer byte as buttons: ~500 phantom presses/sec, no stick data. SDL handles these with a dedicated handshake driver; gilrs has none. |

So: **pair over Bluetooth, don't plug in.** A second device is still needed before the device-pairing
and local-multiplayer work (Requirements §15) can be tested at all.

## tools/padprobe

A standalone probe that answers "what does this controller actually report?" without building a Bevy
app. It is its own workspace root, so it stays detached from the main crate.

```sh
cd tools/padprobe
cargo run -- 30 --bevy
```

Arguments: `[seconds] [--bevy | --unfiltered]`. The filter mode is the point of the tool:

- **default** — gilrs's own default filters: `axis_dpad_to_button`, `Jitter`, and a **radial 0.1
  deadzone with rescaling**. Stick values are deadzoned, and a resting stick reads exactly `0.0000`.
  This is _not_ what Bevy sees.
- **`--bevy`** — replicates `bevy_gilrs` exactly: default filters off, `axis_dpad_to_button`
  re-applied. Stick values are raw. **This is what `RawGamepadEvent` carries**, so it is the mode to
  use when measuring anything that feeds the deadzone design (D6, Requirements §14).
- **`--unfiltered`** — no filters. More raw than Bevy: a hat D-pad stays as `DPadX`/`DPadY` axes
  instead of becoming four buttons.

Use `--bevy` to measure a pad's **resting drift**, which is the calibration number that this mode
is meant to reveal and the default mode cannot read.
