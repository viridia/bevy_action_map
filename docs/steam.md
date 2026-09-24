# Steam Input, measured

What a running Steam client actually does, as distinct from what its documentation says it does.
Every entry here was observed against real hardware; nothing in it is inferred from a docs page.

**What this document admits.** Measurements of an external system. The test is whether the fact
could change without this repository changing — if it could, it belongs here rather than in
`design.md`, which describes what this crate does, or in `decisions.md`, which records what this
crate chose. A finding here may *contradict* a decision, and where it does, the decision says so and
cites the finding rather than the finding arguing with it.

Findings are numbered `S1`, `S2`, … and the numbers are stable identities, cited from
`decisions.md`, `Requirements.md` and `Roadmap.md`. A finding that is later re-measured and found to
have changed keeps its number and gains a second date; it is not deleted, because what changed is
itself worth knowing.

**Everything here is only as current as the bench below.** A finding measured on one platform says
nothing about another, and Steam ships client updates continuously.

---

## The bench

| | |
| --- | --- |
| Measured | 2026-09-09, corrected and extended 2026-09-10 |
| OS | macOS 15.7.4 (24G517), Apple M1 Pro |
| Steam client | stable channel; exact build not recorded |
| `steamworks` crate | 0.13.1 (`steamworks-sys` 0.13.0) |
| Controller | Xbox Series X, wired USB-C |
| App id | 480 (Spacewar), borrowed — see `S4` for what that does and does not allow |
| Instrument | `steam_probe`, a console binary calling `ISteamInput` directly |

---

## Findings

### S1 — The emulated pad does not carry Valve's vendor id

With Steam Input enabled for Xbox controllers, four HID devices are present:

| Device | Vendor | Product | What it is |
| --- | --- | --- | --- |
| `Controller` | `0x045e` | `0x0b12` | the physical Series X pad |
| `GamePad-1` | `0x045e` | `0x028e` | Steam's emulated pad |
| `Keyboard-1` | `0x28de` | `0x1147` | Steam's emulated keyboard |
| `Mouse-1` | `0x28de` | `0x1146` | Steam's emulated mouse |

The emulated *pad* enumerates under Microsoft's vendor id with the Xbox 360 controller's product
id. Valve's own vendor id appears only on the emulated keyboard and mouse.

**Contradicts D22**, which proposed dropping raw events from a Valve-vendor device while a Steam
authority is active, and closed by asking for exactly this measurement before a real backend ships.
Vendor id cannot separate the emulated pad from the hardware underneath it, because they share one.
Product id can here, but `0x028e` is also what a genuine Xbox 360 pad reports, so it identifies a
model rather than an emulation.

### S2 — The double-read is real

Two gamepad HID devices are present simultaneously (`S1`), so a platform layer enumerating devices
sees both the pad and its emulation. R0.6 is a real problem rather than a defensive one.

### S3 — Steam's controller handle cannot be correlated with an OS device

`GetConnectedControllers` returns an `InputHandle_t` (observed: `1230015079772768`). Nothing in the
API relates it to a HID or USB device, and `GetInputTypeForHandle` answers with a product family
(`S7`) rather than an identity. Suppression under R0.6 therefore cannot currently target *the*
device Steam is emulating; it can only silence a whole family, which would also silence a second
pad the backend does not own.

**Narrowed by S14.** No *API* relates them, which is what constrains a backend. The data itself is
not missing.

### S4 — A borrowed app id carries its own action manifest, from inside the bundle

**Corrected 2026-09-10; the original claim was wrong.** A local manifest does override a published
one, from the right directory. What holds:

- `SetInputActionManifestFilePath` returns `false` for every file offered, whatever its contents,
  and loads nothing;
- `controller_config/game_actions_480.vdf` under the outer `~/Library/Application Support/Steam`
  folder is ignored;
- `controller_config/game_actions_480.vdf` **inside the app bundle**, at
  `Steam.AppBundle/Steam/Contents/MacOS/`, is loaded and takes precedence over Spacewar's published
  manifest.

The bundle path is where `steamclient.dylib` itself lives, which is the client's real runtime root
on macOS; the outer folder is the data directory and is not it. With nothing installed there,
Spacewar's own names resolve (`ship_controls`, `menu_controls`, `analog_controls`, `fire_lasers`)
and ours do not. With the file installed, ours resolve and Spacewar's stop.

Names are case-sensitive throughout: `ShipControls` resolves to `0` where `ship_controls` resolves.

A real backend cannot rely on this. Writing into another application's bundle is not something a
shipped game may do, so this is a development affordance for an app id you do not own, not a
delivery mechanism.

### S5 — A resolved action handle is not readiness

`GetActionSetHandle` and `GetAnalogActionHandle` return non-zero handles for actions that are bound
to nothing at all: every origin list was empty and `bActive` was false throughout. A backend that
treats handle resolution as a health check reports itself working and delivers nothing. Readiness is
`bActive`, or a non-empty origin list.

### S6 — Steam Input requires a GUI application with a window

Root cause of what `S5` observes. `GetAnalogActionOrigins` and `GetDigitalActionOrigins` returned
empty for every action of an activated set, and `bActive` was false, in **every** run — including
the runs where a controller was present and the manifest was our own.

Eliminated, one at a time:

- the manifest — ours loads and its names resolve (`S4`, `S13`);
- the app id's published config — ours takes precedence over Spacewar's;
- the pad, the transport and the pairing — Steam's own test-input screen reads stick and buttons
  over both USB and Bluetooth;
- Xbox Configuration Support — enabled, and the emulated devices appear (`S1`);
- `-forcecontrollerappid 480`;
- being launched by Steam — packaged as a `.app` and started from the library, still zero
  controllers and still no origins.

**Answered: the window was it.** A `winit` build of the same probe — same app id, same manifest,
same everything else — gets a controller, resolves origins, and streams analog values. Every
failing run was windowless. macOS bounces the dock icon until an application registers with the
window server, and a process that never does is not one Steam will route a controller to.

No app id is needed for any of this. A borrowed one plus a window is enough.

### S15 — The redistributable dylib must sit beside the binary

`cargo run` resolves `libsteam_api.dylib` from the vendored SDK; a binary started any other way does
not, and dies at load with `Library not loaded: @loader_path/libsteam_api.dylib`. It has to be
copied from `steamworks-sys/lib/steam/redistributable_bin/<platform>/` next to the executable. A
shipped backend has to package it, and the failure is invisible until the first launch that is not
through Cargo.

### S7 — Steam's device taxonomy is coarse and carries no vendor id

`GetInputTypeForHandle` reports an Xbox Series X pad as `XBoxOneController`. `InputType` is a flat
list of product families — no vendor id, no product id, no serial. A backend mapping Steam's
answer onto `GamepadBrand` loses information that the OS had.

### S8 — Steam reports a model identity, not the live device's

The client reported the pad as Bluetooth, at 0% battery, while it was wired and drawing 500 mA:
`0x0b13` where the live connection was `0x0b12`.

**First read as staleness, and it is not.** Deleting `config/virtualgamepadinfo.txt` with the client
closed made Steam regenerate it from the live wired pad — and write `0x0b13` again. Re-paired over
Bluetooth, macOS and Steam agree on `0x0b13`, and Steam's handle is byte-identical to the one it
reported over USB. So the product id Steam exposes identifies the *model*, pinned to this pad's
Bluetooth variant, and does not follow the transport. The battery warning is the same profile
applied to a device that has no battery reading to give.

Backend-supplied *device* identity is therefore not merely advisory but a different namespace from
the OS's, whatever R18.8 grants a backend over *origins*.

### S9 — A pad's product id changes with transport; its vendor id does not

The same physical pad is `0x045e`/`0x0b12` wired and `0x045e`/`0x0b13` over Bluetooth.

**Confirms D64.** Resolving brand from vendor id alone survives a player swapping a cable for
Bluetooth mid-session; anything keyed on product id would not.

### S10 — Layer activation is absent; the presentation surface is complete

`steamworks` 0.13.1 exposes `activate_action_set_handle` and nothing for action set layers, which
**confirms D51's** finding. Everything the presentation half needs is present and callable:
`get_digital_action_origins`, `get_analog_action_origins`, `get_string_for_action_origin`,
`get_glyph_for_action_origin`, `show_binding_panel`. Chunks 151c and 151d are built against it.

### S11 — The SDK's action-data structs are packed

`InputAnalogActionData_t` and its siblings are `#[repr(packed)]`, so a reference into one is
unaligned and will not compile. Fields must be copied out individually. This shapes a backend's
inner loop.

### S12 — `steamworks` 0.13.1 miscounts connected controllers

`get_connected_controllers` calls `Vec::shrink_to`, a capacity hint, where it means `truncate`. It
always returns 16 handles with the tail zeroed, so a caller iterating the result processes fifteen
phantom controllers. `get_connected_controllers_slice` returns the true count and is the workaround.
Upstream, one line, worth reporting; chunk 151b writes the brief.

### S13 — A dotted action name survives into Steam's namespace

R1.7, measured. A manifest declaring both naming styles was installed by `S4`'s bundle route, and
every name resolved to a distinct handle:

| Name | Handle |
| --- | --- |
| `InGameControls` | 1 |
| `pong.paddle` | 2 |
| `Move` | 1 |
| `pong.move` | 2 |
| `ToggleOverlay` | 1 |
| `pong.toggle_overlay` | 2 |

So this crate's own path format — dot separator, at least two segments (TD3.3) — is spellable as a
Steam action name, and R1.7 needs no path-to-name mapping on the dot's account. Valve's own
convention is snake_case with underscores, but that is a convention rather than a constraint.

**Precision about what this shows.** Steam accepted and interned the name, which is exactly what
R1.7 asserts. It is not evidence that a binding to that action works end to end — `S5` is the
standing reminder that a resolved handle is not readiness.

---

## Not measured yet

Ground rule 5 applies here as everywhere: each row names what would settle it.

| Question | Gate |
| --- | --- |
| **Do action set *layers* stack, where base sets do not?** `S19` settled the base case; layers are D51's intended answer and `steamworks` 0.13 exposes none of the functions | a patched `steamworks`, or a direct FFI call past the safe wrapper |
| **Does the emulated pad carry Valve's vendor id on Windows?** `S1` is a macOS measurement, and D22's original claim may have described Windows | a Windows machine with the same pad |
| **Can a player's configuration emit keyboard and mouse events for a pad while the game reads Steam Input natively?** `S1` found `Keyboard-1` and `Mouse-1` alongside the emulated pad. They exist whether or not they emit, and only a binding that maps a pad control to a key would make them. If that combination is reachable, suppressing the gamepad family does not stop it, and the input arrives as ordinary keyboard events the game has no reason to distrust | chunk 151b: a bound configuration, so `S6` first, then a hand-edited config that maps a pad control to a key |
| **Does a Steam build run at all with the client absent?** D22 assumed it must, and therefore that one binary has to serve both a Steam launch and a direct one; that was never measured, and the decision now stands on replay instead. The documented pattern is `SteamAPI_RestartAppIfNecessary` relaunching through the client, or `SteamAPI_Init` failing outright, either of which would make per-channel builds the ordinary shape | chunk 151b: the demo launched with the client closed |
| **What is an `absolute_mouse` action's delta measured since?** `S18` measured `joystick_move`, a position. `absolute_mouse` reports movement, and whether it is movement since the last `RunFrame` or since the last read of that action decides how a backend has to poll it | chunk 151b: the right stick bound to an `absolute_mouse` test action, logged across frames with and without a second read |
| **Does an `InputHandle_t` survive a restart?** D74 leans on it for a persistent identity, while this document's appendix follows D52 in treating a handle as runtime-only. S14's layout — vendor, product, instance suffix, kept on disk — suggests it survives, from one sample | chunk 151e: the same pad's handle read across two launches, and across a client restart |
| **A wired Xbox pad on macOS is invisible to raw IOHID enumeration, but Steam still reads it.** Plugged in over USB-C, the same pad opens macOS's own Game Center overlay on its Home button — a system-level claim — and `padprobe` (raw gilrs, `IOHIDManager`) sees nothing from it at all; the identical pad over Bluetooth is ordinary and gilrs sees it fine. Steam Input reads the wired pad regardless, so it has some access path an `IOHIDManager` consumer does not | a packet capture or Steam's own logging against the same wired pad, or confirmation from Valve on how Steam Input acquires a macOS-claimed HID device |

### S14 — Steam holds the handle-to-device mapping on disk, and the handle embeds it

`config/virtualgamepadinfo.txt` in the Steam data directory records, per slot, the device name,
vendor id, product id, controller type, and the `InputHandle_t` Steam will report for it:

```
[slot 0]
name=Xbox Series X Controller
VID=0x045e
PID=0x0b13
handle=0x00045eb133e5f260
type=xboxone
```

The handle matches what `GetConnectedControllers` returned exactly, and its digits are the vendor
id and product id followed by an instance suffix — `000` `45e` `b13` `3e5f260`. One sample, so the
packing is inferred rather than known; a vendor id needing four significant nibbles would not fit
the same layout.

**Consequence for S3 and R0.6.** The correlation an emulation-aware suppression needs is not
missing from Steam, only from its API. Per-device suppression is unavailable rather than
impossible, which is the case to make upstream. Neither route is usable by a shipped game: reading
another application's cache file is no more permissible than writing into its bundle (`S4`), so
chunk 112 still ships per-family suppression.

**And the identity it holds is Steam's, not the OS's.** The record above carried `0x0b13` while the
pad was wired on `0x0b12`, and kept the same handle across a transport swap (`S8`). The join a
suppression policy needs — Steam's handle to an OS device — therefore fails on product id even
with this file in hand. Vendor id still matches, but vendor id alone cannot separate an emulated
pad from a real one (`S1`), which is where this started.

### S16 — The manifest declares actions; a *layout* binds them, and it is a workshop artifact

Two separate things, and only the first is a file the game controls.

The action manifest (`S4`) declares action sets and actions. What maps a physical control onto an
action is a **layout**, and for app id 480 the one in force was `workshop://1761888411`, "Official
Configuration", authored by Valve. Viewed against our manifest it rendered every binding as `--`:
Valve's layout binds Spacewar's actions, our manifest replaced them, and every binding in it was
orphaned.

**A mismatched layout fails silently.** No refusal, no diagnostic — an empty layout, zero origins,
`bActive` false. Indistinguishable at the API from having no controller at all, which is what made
`S6` take so long to isolate. The client's own red "ERROR MESSAGES PRESENT" banner contained no
errors.

Editing the layout forks a personal copy. Binding the left stick to `Move` by hand made everything
work at once:

```
origins for Move: 1
  Left Stick Movement | glyph: …/controller_base/images/api/dark/shared_lstick_md.png
show_binding_panel -> true
Move: bActive -> true
Move: +0.888, +0.459 (active true)
Move: -0.008, -1.000 (active true)
```

**What this means for R19.8.** "Delegate to the backend's own UI" is not a courtesy — the bindings
live in a workshop ecosystem the game has no write access to. A shipped game publishes a default
layout for its own app id and the player may replace it; there is no in-game path to authoring one.

### S17 — Origins, glyphs and the binding panel all work, and D22's glyph path is wrong

Measured together with `S16`:

- `GetAnalogActionOrigins` returns one origin per bound control;
- `GetStringForActionOrigin` gives a readable label — `Left Stick Movement`;
- `GetGlyphForActionOrigin` gives an absolute path, and it is **not** where D22 says. Measured:
  `<bundle>/Contents/MacOS/controller_base/images/api/dark/shared_lstick_md.png`, against D22's
  `tenfoot/resource/images/library/controller/api/`. Note the `dark` component: the glyphs are
  themed, which nothing in this project's documents anticipated;
- `ShowBindingPanel` returns `true` and opens the client's configuration UI over the game.

### S18 — Analog action data arrives as a normalized pair in ±1.0

`InputAnalogActionData_t`'s `x`/`y` span `-1.0..=1.0` with the resting stick near zero and full
deflection reaching `1.000`. It maps onto `ActionValue::Axis2` with no rescaling, which is what
D51 assumed without measuring.

### S19 — Exactly one action set is live per controller, and the last activation wins

D51 measured. A manifest declaring two action sets, an analog action bound in each, and four
phases cycling how they are activated:

| Phase | Activation | `Move` (set 1) | `pong.move` (set 2) |
| --- | --- | --- | --- |
| 0 | set 1 alone | active | — |
| 1 | set 2 alone | — | active |
| 2 | set 1 **then** set 2, same frame | — | active |
| 3 | set 2 **then** set 1, same frame | active | — |

`ActivateActionSet` is exclusive and replaces rather than stacks. Two sets cannot be live at once
through the base API, and `steamworks` 0.13 exposes nothing else (`S10`).

**Confirms D51**, which reached the same conclusion from Valve's documentation and the binding's
API surface. The value of measuring it is that the alternative was live: had Steam left both sets
reporting, this crate's any-number-of-contexts model would have mapped onto Steam directly.

**What it costs.** A Steam authority backend can drive **one** of this crate's contexts at a time. A
game running a gameplay context and a modal overlay context simultaneously — which this crate does
routinely — cannot have both fed by Steam. `ActivateActionSetLayer` exists in the Steamworks SDK and
is the intended answer, but the Rust binding does not expose it, so reaching it needs a patch
upstream or a direct FFI call past the safe wrapper.

### S20 — One control can drive several actions in one set, and Steam does not arbitrate

Two digital actions, `alpha` and `beta`, declared in the same action set and both bound to the A
button. Both resolve an origin reading `A Button`, and both report pressed on the same frame:

```
origins for alpha: 1
  A Button
origins for beta: 1
  A Button
alpha: active=true pressed=true
beta: active=true pressed=true
```

Steam fans a control out to every action bound to it and picks no winner. Arbitration is the
game's.

**This largely retires S19's constraint.** S19 rules out a Steam action set per context wherever two
contexts can be live together — but a set per context was never required. One set holding every
action bound to the authority works: activate it once, poll every action every tick, and let this
crate's own contexts, priorities and consumption decide what a value means. The evaluator already
does the gating, since an inactive or shadowed context does not sample its authority.

**The correspondence is therefore not one-to-one.** What Steam's sets must not do is split two
contexts that can be active simultaneously. Contexts that are mutually exclusive may share Steam's
exclusivity freely — and the crate already names those boundaries, in `EXCLUSIVE` contexts and the
exclusion ceiling.

**What one set costs** is presentation, not function. The player sees a flat list with one button
bound to several actions, and no indication that context decides which applies. A set per
mutually-exclusive group keeps Steam's own tabbed binding UI meaningful, so it remains the better
shape wherever the exclusivity is real.

---

## Appendix: what the two examples would encode

Worked from the contexts `examples/disasteroids` and `examples/split_friction` actually declare.
The rule from `S19` and `S20` is that a Steam action set may not split two contexts that can be
live at the same time; beyond that the mapping is free.

One thing to know first: **action handles are global, not per set.** `GetDigitalActionHandle` takes
a name and no set, and the same handle comes back whatever is active. A set scopes which actions
are *live* and what they are bound to, so declaring one action in several sets is natural rather
than a duplication problem.

### Disasteroids — three contexts, two sets

| Context | Tick | Active when |
| --- | --- | --- |
| `Flying` | Fixed | `Game::Playing` |
| `Shell` | Render | always — it owns Pause, so something must hear the unpause |
| `Menu` | Render, priority 10, **exclusive** | a screen entity exists |

Which can be live together:

- playing: `Flying` **and** `Shell`;
- paused: `Shell` alone;
- a screen up: `Menu` alone, because being exclusive at priority 10 shadows both others.

So `Flying` and `Shell` must share a set, and `Menu` may have its own:

```
gameplay   Thrust, Turn, Fire, …   (Flying)   +   Pause, ToggleOverlay, ToggleSettings   (Shell)
menu       Navigate, Confirm, Back (Menu)
```

Three contexts, two sets, and the split falls exactly where `exclusive` already put it. Note what
follows: while `menu` is active nothing in `gameplay` reports, so `Pause` goes quiet under a
settings screen — correct here, since `Menu` has its own way out, but a game wanting a
still-live global action under a modal would declare it in both sets rather than reaching for
layers.

### Split Friction — three contexts, one set, two controllers

| Context | Tick | Instances |
| --- | --- | --- |
| `Debug` | Render | one, unpaired |
| `Inviting` | Render | one per available device, each `Paired` to its own; its only binding is the join gesture |
| `OnFoot` | Fixed | one per protagonist, each `Paired` to a device |

All three are live together — an `Inviting` instance keeps waiting for player two while player one
is already walking around under `OnFoot`. So they share a single set, and the interesting dimension
is not sets at all:

**Two players are two controllers, not two action sets.** `ActivateActionSet` and every data read
take an `InputHandle_t`, so one set is activated separately per controller and each player's values
are read from their own handle. That is the same axis `Paired` already models, which is why the
per-player case needs nothing from Steam's set mechanism.

Two problems surface here that Disasteroids never reaches:

**A class-bound join gesture has no Steam counterpart, and that is survivable.** Steam reports
actions you declared and nothing else, so there is no way to ask it about an unbound press, and
`bind_class::<Join>(ControlClass::AnyButton)` — "any button, on any device" — has no expression.
A build declares a `join` action instead and prompts for it by name, which is what shipped console
games do: Split Fiction asks for A on Switch. Split Friction moved to exactly that in chunk 110 and
no longer depends on the class path here, so what is left is general: class bindings remain the one
part of this crate's binding vocabulary with no Steam expression.

**Which device pressed join is answered by pairing, not by the action.** `Inviting` is one instance
per device, so a Steam backend writes `AuthorityValues` on the instance belonging to the
`InputHandle_t` it polled, and the observer reads the presser off that entity's `Paired`. Nothing in
the path asks for a raw button state, which is what makes it work here at all.

**Pairing lives in Steam's namespace, and `DeviceHandle` extends to say so.** `Paired` holds a
`DeviceHandle` naming a Bevy gamepad, where Steam offers an `InputHandle_t`, and `S3` and `S14`
say the two cannot be joined. That would be a problem only if both device sources were live at
once — and they are not: suppression is per family and wholesale (D22), so gamepads are Steam's or
`gilrs`'s and never both. With no gilrs gamepad to reconcile against, a third `DeviceHandle`
variant carrying the backend's own handle is the whole of it, and D52 already frames a handle as a
runtime value no save file may compare across a restart, which is exactly what an `InputHandle_t`
is.

**And no new variant is needed.** `DeviceHandle::Gamepad` already holds a bare `Entity` and the
crate never looks inside it: nothing in `src/` queries Bevy's `Gamepad` component, and the crate's
own tests build handles from `Entity::from_bits` — synthetic entities carrying no components at
all — with everything downstream working. The entity is an opaque key. The only place a real Bevy
gamepad entity enters is `frame.rs`, converting a raw gamepad event, and that is the path
suppression turns off.

So a Steam backend spawns one entity per `InputHandle_t`, keeps the handle in its own component in
its own crate, and hands the game `Paired::to(DeviceHandle::Gamepad(entity))`. `Entity` is already
a dependency of this crate and `steamworks` never enters its graph — the same indirection Bevy
uses for `gilrs`, reused rather than reinvented. D52's own wording, "identified by the backend's
own entity for it", already covers this; "backend" was doing double duty and now means both.

That also answers R0.5 more cheaply than widening anything: given the entity, query it for the
backend's marker component. Identity that is queryable and never required at the call site.

**Two things the entity does not carry.**

*Brand.* `GamepadBrand::resolve` takes an `Option<u16>` vendor id and is called by the game rather
than by this crate. A Steam-spawned entity has no vendor id to give (`S7`), so a backend either
reports `Generic` or maps `InputType` onto `GamepadBrand` itself, which is lossy one way and empty
the other.

*Disconnect.* The crate learns a pad is gone from `RawGamepadEvent::Connection` reaching the frame,
and under Steam no such event exists. A backend has to synthesise it, either by pushing a raw event
or through a path that does not exist yet. This is the one place where "the entity is just a key"
stops being sufficient, and it touches R11.4 and chunk 103.
