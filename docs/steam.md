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
| Measured | 2026-09-09 |
| OS | macOS 15.7.4 (24G517), Apple M1 Pro |
| Steam client | stable channel; exact build not recorded |
| `steamworks` crate | 0.13.1 (`steamworks-sys` 0.13.0) |
| Controller | Xbox Series X, wired USB-C |
| App id | 480 (Spacewar), borrowed — see `S4` |
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

### S4 — A borrowed app id cannot carry its own action manifest

Under app id 480, Spacewar's published action manifest is what Steam serves. Both delivery routes
were refused:

- `SetInputActionManifestFilePath` returned `false` for every file offered, whatever its contents;
- a `game_actions_480.vdf` placed in the client's own `controller_config` directory was ignored.

Spacewar's own names resolve (`ship_controls`, `menu_controls`, `analog_controls`, `fire_lasers`),
so the manifest being served is Valve's. Names are case-sensitive: `ShipControls` resolves to `0`
where `ship_controls` resolves.

### S5 — A resolved action handle is not readiness

`GetActionSetHandle` and `GetAnalogActionHandle` return non-zero handles for actions that are bound
to nothing at all: every origin list was empty and `bActive` was false throughout. A backend that
treats handle resolution as a health check reports itself working and delivers nothing. Readiness is
`bActive`, or a non-empty origin list.

### S6 — Steam applies no controller configuration to a process it did not launch

Root cause of what `S5` observes. A terminal binary that borrows an app id gets handle resolution
and controller enumeration, and no bindings: `GetAnalogActionOrigins` and `GetDigitalActionOrigins`
returned empty for every action of an activated set. Whether a windowed application launched *by*
Steam behaves differently is not yet measured.

### S7 — Steam's device taxonomy is coarse and carries no vendor id

`GetInputTypeForHandle` reports an Xbox Series X pad as `XBoxOneController`. `InputType` is a flat
list of product families — no vendor id, no product id, no serial. A backend mapping Steam's
answer onto `GamepadBrand` loses information that the OS had.

### S8 — Steam's device identity can be stale

The client reported the pad as Bluetooth, at 0% battery, while it was wired and drawing 500 mA; its
device-support string carried `0x0b13`, the Bluetooth product id, where the live connection was
`0x0b12`. A record from an earlier pairing survived both a client restart and an OS-level unpair.
Backend-supplied *device* identity is advisory, whatever R18.8 grants a backend over *origins*.

### S9 — A pad's product id changes with transport; its vendor id does not

The same physical pad is `0x045e`/`0x0b12` wired and `0x045e`/`0x0b13` over Bluetooth.

**Confirms D64.** Resolving brand from vendor id alone survives a player swapping a cable for
Bluetooth mid-session; anything keyed on product id would not.

### S10 — Layer activation is absent; the presentation surface is complete

`steamworks` 0.13.1 exposes `activate_action_set_handle` and nothing for action set layers, which
**confirms D51's** finding. Everything the presentation half needs is present and callable:
`get_digital_action_origins`, `get_analog_action_origins`, `get_string_for_action_origin`,
`get_glyph_for_action_origin`, `show_binding_panel`. Chunk 42 planned to mock that surface; it can
be developed against the real one.

### S11 — The SDK's action-data structs are packed

`InputAnalogActionData_t` and its siblings are `#[repr(packed)]`, so a reference into one is
unaligned and will not compile. Fields must be copied out individually. This shapes a backend's
inner loop.

### S12 — `steamworks` 0.13.1 miscounts connected controllers

`get_connected_controllers` calls `Vec::shrink_to`, a capacity hint, where it means `truncate`. It
always returns 16 handles with the tail zeroed, so a caller iterating the result processes fifteen
phantom controllers. `get_connected_controllers_slice` returns the true count and is the workaround.
Upstream, one line, worth reporting.

---

## Not measured yet

Ground rule 5 applies here as everywhere: each row names what would settle it.

| Question | Gate |
| --- | --- |
| **R1.7: does a dotted action name survive into Steam's namespace?** Valve's own convention is snake_case with underscores. `design.md` §3.3 mandates a dot separator and at least two segments, so if Steam refuses a dot the two collide and a backend needs a path-to-name mapping rather than using the path verbatim | an app id we own, since `S4` makes a borrowed one unable to carry our manifest |
| **Does a windowed app launched by Steam get a controller configuration?** `S6` measured only a terminal binary that Steam did not launch | the same app id, plus a windowed build |
| **Do glyph paths resolve as D22 describes** — absolute, under the client's own install directory | bound actions, so `S6` first |
| **Can two action sets be live at once?** D51's layer question, and the one measurement that could falsify a decision rather than reveal a gap | bound actions, so `S6` first |
| **Does the emulated pad carry Valve's vendor id on Windows?** `S1` is a macOS measurement, and D22's original claim may have described Windows | a Windows machine with the same pad |
| **Can a player's configuration emit keyboard and mouse events for a pad while the game reads Steam Input natively?** `S1` found `Keyboard-1` and `Mouse-1` alongside the emulated pad. They exist whether or not they emit, and only a binding that maps a pad control to a key would make them. If that combination is reachable, suppressing the gamepad family does not stop it, and the input arrives as ordinary keyboard events the game has no reason to distrust | a bound configuration, so `S6` first, then a hand-edited config that maps a pad control to a key |
