# Deferred

Work that has been decided against for now, each entry held until a stated gate.
[`issues.md`](./issues.md) holds what has not been decided yet; an entry here has been, and the
decision was "not yet". [`Roadmap.md`](../Roadmap.md) holds the work that is scheduled.

**What this document admits.** Work that will be done when a named event happens, and not before.
The gate is the entry: one with no gate is an item that will be dropped, which is ground rule 5.

**When a gate fires**, the entry becomes a chunk in `Roadmap.md` and is deleted here, and whatever
it knew goes into the chunk's section.

**Numbering.** Each entry's number is a permanent identity from a single counter, independent of its
group, and never reused. A gap in the sequence is an entry that left.

**Next: 61.**

**How it is grouped.** By the kind of gate, so the question "has anything fired?" is asked of one
group at a time: a Bevy version bump is the first group, and nothing else.

---

## 1. Bevy or winit moving past the pin

Each of these waits on an upstream change that 0.20.0-rc.2 does not carry. On every bump, check each
against the new version: a release candidate is cut from the release branch, so a PR merged to main
before it is in it only if it was picked.

### X51 — Clicking in physical pixels, and deleting `driver.locate`

**Gate:** this crate's Bevy pin moving past [bevy#25890][], merged to main on 23 September 2026 and
not in rc.2.

It moves `CursorMoved`'s enrichment from `bevy_winit` to `bevy_window`, and a client writing
`WindowEvent::CursorMoved` then supplies a `physical_position`. The driver's click writes a logical
one (DD5.1), so this is not optional: the click breaks at the bump. `UiGlobalTransform` is already
physical, and `ComputedUiTargetCamera` and `RenderTarget` are reflected, so the client can locate a
node itself and `driver.locate` goes (DD3.3).

### X52 — Deleting `driver.diagnostics`

**Gate:** this crate's Bevy pin moving past [bevy#25824][], merged to main on 17 September 2026 and
not in rc.2.

It adds `diagnostics.get` to BRP, which reads `frame_count` as `driver.diagnostics` does (DD3.4).
The client switches to it and the method goes. `FrameTimeDiagnosticsPlugin` is still the driver's to
add, since it records the count and is not in `DefaultPlugins`.

### X2 — Deleting `acquire_focus_directional`

**Gate:** this crate's Bevy pin moving past [bevy#25675][], merged to main on 24 September 2026 and
not in rc.2.

`examples/common/widget_focus.rs` carries a global `AcquireFocus` observer mirroring
`acquire_focus_tab_index`, with `AutoDirectionalNavigation` standing in for `TabIndex`:
`bevy_input_focus`'s `click_to_focus` bubbles an `AcquireFocus` on every pointer press, a screen
navigating by anything but `TabIndex` intercepts it nowhere, so it reaches the window and clears
focus — and a widget whose interactive children are separate entities, like a stepper's two
chevrons, blinks on every press rather than rarely. The PR separates focusability from navigation
policy behind a `Focusable` component and fixes [bevy#25596][], click-to-focus under directional
navigation. Read on main, it retires the observer outright, including for a scheme that is neither
`TabIndex` nor one of upstream's own: `InputFocusPlugin` installs an `acquire_focus` observer that
stops at the first `Focusable` ancestor whatever the scheme, and `AutoDirectionalNavigation`
requires `Focusable`. What is left on the bump is the deletion, and the stepper's chevrons as its
test.

### X53 — Deleting `navigate_from`

**Gate:** this crate's Bevy pin moving past a fix for [bevy#25944][], filed 27 September 2026.

`AutoDirectionalNavigator::navigate` returns `NoFocus` with nothing focused, so a click on empty
space leaves a pad with no way back. `examples/common/widget_focus.rs`'s `navigate_from` answers it
by focusing the screen's `AutoFocus` entity, and both Disasteroids' controls screen and Split
Friction's popup call it. The issue offers that and two other defaults; whichever upstream picks,
the two callers go back to calling the navigator directly, and `pad.py`'s click-on-the-title step is
the test. If upstream picks a default other than `AutoFocus`, the step's expected landing changes
with it.

### X3 — Deleting `examples/common/font.rs`

**Gate:** this crate's Bevy pin moving past [bevy#25847][], which answers [bevy#25842][] and merged
to main on 22 September 2026 and not in rc.2.

Until then the plugin overwrites the `default_font` feature's slot at `AssetId::default()` during
plugin build, which depends on that slot's location and on text layout registering a font id once.
The answer is a `DefaultFontSource` resource that `FontSource::Default`, now `TextFont`'s default,
resolves to. Changing it rebuilds the font collection and marks every `TextFont` changed, so it
reacts to a change and the build-time ordering goes with the file.

### X43 — Publishing to crates.io

**Gate:** Bevy 0.20.0 final. The dependency requirement `^0.20.0-rc.2` already admits it, so the
move is a `cargo update` and a lockfile commit.

crates.io would accept a publish against a release candidate, so the gate is a judgement, not a
limit: a crate published against an rc pins its users to a version about to be superseded. What this
entry holds is the order. A change that breaks the public API is free until the first publish and
costly after it, so those land first: chunk 161's narrowing of public items is one, and so is any
reshaping of the extensibility mechanism. The README's "Not on crates.io yet" section changes with
the publish. The macros crate goes first: until it is on crates.io, `cargo package` resolves the
root only with `--exclude-lockfile`.

Once everything else is done, the release is the publish, then an announcement on the Bevy Discord.
X54's question goes to the users that announcement reaches.

## 2. An upstream decision still open

### X5 — Focus orchestration: guidance, and a worked example (D87)

**Gate:** community feedback, and Bevy's own direction on driving widget state from outside.

D87 keeps the mapper focus-agnostic, so the layer binding actions to whichever widget has focus sits
outside the crate — `widget_focus.rs` is one, and names the role. What is deferred is telling
someone how to build their own: a requirements-and-design section for an orchestrator, and the
worked example of held-down visual state with the latch D87 describes. Guidance is the deliverable
whether or not this project ever ships an orchestrator itself. Both gates are real rather than a
delay. The first: that a game activating widgets from the keyboard and the pad wants the pressed
highlight a mouse gives, and that the one-shot activation `widget_focus.rs` ships today is not
already enough, are guesses about other people's UI. The second: open Bevy issues on remote control
of widget state will change what an orchestrator has to do, so guidance written now would describe a
shape about to move. Nothing is blocked — a game wanting the highlight has D87's rule and no crate
change to wait for.

### X46 — Upstreaming L0, then a minimal mapper

**Gate:** Bevy accepting the gamepad layer proposed in [bevy#25757][], which chunks 165 and 167–169
build, and `docs/proposals/gamepad.md` sets out.

Upstreaming goes in three stages, each a stack of reviewable PRs: the gamepad layer first, then L0
(device families, raw messages and calibration), and a minimal mapper only on top of those. A first
PR that is the mapper itself, keyboard only and in 1,000–2,000 lines, was the earlier plan; the
prototype on the `ported` branch showed a minimal working mapper does not come close to fitting that
budget.

### X50 — Gamepad capabilities and battery (R11.3, R14.7)

**Gate:** a backend with a source for either. For gilrs, `bevy_gilrs` forwarding `power_info()` and
`is_ff_supported()`, checked on each Bevy bump; for Steam, `ISteamInput` gaining a controller
battery or capability query, checked on each `steamworks-sys` bump. Or a prompt in tree needing
motion, touchpad or LED enough to pay for a table keyed on the pad's model.

Was chunk 166, withdrawn before it began, because what either backend supplies is too little to
build on:

- **gilrs 0.11** reports power state and force-feedback support on Linux and Windows only, and
  `Unknown` and `false` on macOS and wasm. It has no motion, touchpad or LED query anywhere, and no
  power-change event, so battery would be polled.
- **`bevy_gilrs` 0.20.0-rc.2** forwards neither, and keeps `Gilrs` and its entity-to-`GamepadId` map
  `pub(crate)`, so a crate outside it cannot read them at all.
- **Steam Input, through SDK 1.65** (`SteamInput007`), has no controller battery and no capability
  query; `ISteamUtils`' battery calls report the host. Vibration, LED, haptics and trigger effects
  are silent on hardware without them. The product family from `GetInputTypeForHandle` is the only
  per-device fact.
- **Under Steam, what gilrs sees depends on how the game reads Steam Input.** S1 found an emulated
  Xbox 360 pad beside the real one, and gilrs's answers about it describe the emulation. A game
  reading Steam Input natively got no emulated pad in S33, and gilrs saw the real pad. Which of
  Steam's settings create the emulated pad is unmeasured.

Motion, touchpad and LED can only come from a table keyed on vendor and product id, or on Steam's
family. That table is the chunk when this returns, and whether capabilities are one component or
markers is decided with it.

### X48 — Upstreaming the presentation crate

**Gate:** X46's minimal mapper accepted into Bevy.

`bevy_action_map_ui`, which chunks 172a and 172b extract, can go upstream only above an input crate
that is already there: Bevy cannot ship a crate depending on a third-party one. Whether it goes is
independent of whether it exists, and on the gate it is ported onto the upstream mapper's API rather
than moved, since X46 is a fresh implementation.

### X8 — Replacing `WidgetKind` with Bevy's own

**Gate:** [bevy#25592][], the author's own upstream proposal for a `bevy_ui_widgets`-native
widget-kind id, landing in a Bevy this crate pins.

Chunk 172b publishes `WidgetKind` in `bevy_action_map_ui`, a newtype over a string, rather than wait
for that conversation. On the gate the ui crate's copy is replaced by Bevy's, a breaking change a
0.x crate can take. The base crate never gains it: R22.9 keeps widget knowledge in a bridging crate.

### X9 — Whether an action is live, as something a hint can follow (R18.2's withdrawal)

**Gate:** reactive UI in Bevy, so a hint's visibility can be bound to a predicate rather than set by
a system the game writes.

Until then a game hides a hint from its own state, which it knows better than the crate does: which
menu is open, whether play is paused. The predicate would be whether a carried, active context binds
the action to a control nothing stronger consumes; D84 is why it is not a filter on the prompt
lookup.

### X10 — Sub-frame event timing (D4's remainder)

**Gate:** [bevy#9087][] upstream.

Gamepad stays frame-quantized regardless until gilrs polling is rewritten, so mixed fidelity across
sources is permanent for now rather than an artifact.

### X12 — A physical binding's label matching the current layout (R12.2, R12.7)

**Gate:** winit exposing a physical-to-logical query and a layout-change signal, requested as
[winit#4606][] and tracked by the broader [winit#2678][], open since February 2023 and
unimplemented.

A workaround was scoped and set aside: `run_captures` already sees the logical key at capture time,
but keeping it means a new field on `ControlCaptured`, a session table `present.rs` consults ahead
of the static fallback, and an honest answer on whether it survives a save — which drags in the
still-deferred binding-definition serialization (R17.6, R22.16) for a fix that only covers controls
a player has personally rebound. A landed query supersedes it outright, for every physical binding
rather than only captured ones, so the workaround is not worth building ahead of it.

## 3. A game or screen in tree that needs it

### X13 — Resolving a stored device identity to the connected devices that match it

**Gate:** a caller asking in that direction.

Written for chunk 72 and withdrawn for want of one; chunks 72d and 92 did not need it either.
Devices arrive as connection events one at a time, the ones already plugged in at launch included,
so every caller has one device in hand and asks the inverse question. Two identical pads never
present themselves as a set to choose from.

### X14 — Consumption-aware `FocusedInput` dispatch (R8.2a)

**Gate:** a game wanting `bevy_ui_widgets`' own widgets working generically, unmodified, without a
context per widget kind.

A context per kind is the path to reach for first, and Disasteroids ships that way. A design for the
filter was built and set aside: a lowest-priority, non-consuming context binding
`ControlClass::AnyButton`, feeding dispatch through the existing class-binding pipeline rather than
a second raw-message read — keyboard only, since every keyboard-driven widget observer at 0.20 gates
on `ButtonState::Pressed` and none reacts to a release.

### X15 — Asking whether a row would be refused, before Confirm

**Gate:** a refusal a screen's working copy can reach that capture does not already turn down.

Chunk 127 removed the last one: a wrong family, a wrong shape and a reserved control are refused at
capture, and a `Fixed` row has no cell to capture into. The screen writes gestures into
`PendingOverrides` and only Confirm asks whether they are legal, so a new refusal of that kind is
shown taking and then silently lost. `refusal` is private and wants the declared bindings, so the
general answer is a dry run of the apply, which is public API. The cheaper answers are for the row
to say what would be refused so a screen can decline the gesture, or for the screen to apply eagerly
and keep Confirm for persistence.

### X16 — A context-level exclusion from the mapping list

**Gate:** a second screen needing the same filter and duplicating it.

`ActionMapping::context` already carries the data, and one call site filtering on it costs one line
— at two, the crate is the one paying for the repetition.

### X17 — An initial delay distinct from the repeat rate (R22.5)

**Gate:** a screen long enough to feel the difference.

`.on_change().pulse(0.25)` gives one number serving as both. Two numbers is a small change; what is
missing is a case where equal is wrong, and a two-table settings screen is not it.

### X18 — Free-form mutually-exclusive context sets (R7.7 remainder)

**Gate:** something in tree needing two independently-exclusive contexts to coexist rather than one
dominating the other by priority.

### X19 — A game-wide "more forgiving timings" control (R20.4's withdrawal)

**Gate:** a game with enough timings that setting them one at a time is the complaint.

One player-facing control across a whole game needs the crate to know which way forgiveness runs per
threshold — down for `Hold` and `HoldAndRelease`'s floors, up for `Tap` and `MultiTap`'s ceilings
and `Pulse`'s interval — which is the one part of this a game cannot get right without hand-checking
five signs, and the reason this entry exists rather than the idea being dropped with the
requirement. Chunk 115's per-timing tunables come first regardless: they are what a game would
expose the control *through*, and they may turn out to be all anyone wants.

### X20 — Auto-switching which device a player is paired to (R15.8)

**Gate:** a single-player game in tree that wants it, which is where the value is.

Asked directly, LWIM's maintainer put pad-to-keyboard switching at mattering a bit, and much more in
single player or networked multiplayer than in local co-op. One person pressing things makes "which
device are they on now" a question with one right answer; two make it the wrong question, which is
why Split Friction joins once and a player who wants the keyboard takes it the same way they took
the pad. The gate stays untripped for a reason rather than for want of demand: Disasteroids is the
single-player game, and it pins `PromptDevice` to the keyboard on purpose, being a desktop game
whose prompts name keys with a pad plugged in. Deferred rather than withdrawn alongside R15.7,
because unlike R15.7 an app cannot write it: telling a deliberate grab from a drifting stick means
reading raw samples under a deadzone floor before any action fires, which an app watching `Fired`
never sees. If it lands, a prompt reads the player's paired device rather than tracking one of its
own, and R18.6 revives with it.

### X21 — Nintendo's confirm button (what R18.7's withdrawal left)

**Gate:** a game that wants confirm to follow the pad in hand, checked first on a Nintendo pad
reporting through gilrs.

A Nintendo pad confirms with A, in the East position, where every other brand confirms with South.
Only the gilrs path sees this, since a Steam Input backend hands over actions already mapped. Read,
not run: gilrs takes SDL's `a`/`b` as `South`/`East`, and SDL_GameControllerDB maps a Nintendo pad
by position (its `mapping_guide.png`), so A arrives as `East` and a game confirming on South
confirms on a Nintendo player's B. A preset swapping South and East fixes that for a game that knows
its player. Following the pad instead is per device rather than per family, since two brands can
share one game, and `Brand` is already on the gamepad entity to read.

### X22 — Naming which authority produced a value (R0.5's queryable half)

**Gate:** a build with two authorities in it.

That a delegated action is indistinguishable from a bound one at the call site is the requirement's
point; what has no reader is *which* authority supplied it. Chunk 111 left `AuthorityValues` unnamed
rather than adding a field nothing consults, and one authority cannot motivate a name —
`pong_robot`'s robot has nothing to be told apart from.

### X23 — An authority backend's actions in rollback (D22's remainder)

**Gate:** a snapshot to fit them into.

`AuthorityValues` is a plain component and clones with the entity, but what a rewind has to
reproduce is what the authority *said* on the tick being re-simulated, which is not in the frame.
The available answer is recording the backend's output into the frame at sample time, at the cost of
a larger frame.

### X24 — An authority on a `Delta2` action (Steam's `absolute_mouse`)

**Gate:** a Steam build of a game with a look action, most likely chunk 121's camera.

Chunk 151a refuses the declaration. The design: a delta must be folded once, where a level may be
read by every tick, so a context folds a `Delta2` authority value only when `AuthorityValues` has
changed since that context's evaluator last ran, and zero otherwise. That is the frame cursor's rule
for real mouse motion, with the component's change as the cursor; an explicit write counter is the
alternative if change detection proves too implicit. Steam's delta accumulates since the last read
and the read consumes it (S26), so a backend reads it once per frame; that decides how a backend
polls, not what the crate does.

### X25 — OS gestures as binding sources (R13.7)

**Gate:** a game that wants one, and can say what it should do.

Pinch, rotation, pan and double-tap arrive as `bevy_input::gestures` events — window-level, carrying
no pointer and no entity — so they are the wheel's shape rather than picking's, which is why section
13 keeps them where it withdrew the rest of the pointer. What is missing is not a mechanism: the
design questions are what a pinch's units are and whether it wants the modifier chain a stick does,
and neither can be answered without a customer to ask.

### X26 — Split Friction's monsters, spawners and missiles

**Gate:** a mechanic that would exercise input this crate has not already proven.

Kept rather than deleted because the sprites, the dungeon's region aspects and a `Fire`-shaped
action all exist, so changing our mind is cheap.

### X49 — Reading back one entity's rebound rows

**Gate:** a game or example in tree building a per-player controls screen.

`apply_to_entity` computes the rewritten mappings and tunables and drops them: `AppliedPlan<C>` is
the default a new instance inherits, so a per-entity apply cannot write there, and there is no
per-entity store for them. `mappings` and `tunables` therefore read the world-wide rows only, and a
screen showing one player's bindings after `apply_overrides_for` shows the shared ones. The fix is a
per-entity reader, with the rows kept on the instance or recomputed from its plan. Until then, a
game holds each player's `Overrides` itself and can layer them over `declared_mappings`, though that
skips refusal.

## 4. The Steam build

### X27 — A cheaper watch on Steam's layout

**Gate:** a profile showing the Steam build's origin poll costs a frame something, or a game with
enough actions that it plainly would.

`poll` asks every action's origins every frame and compares, because `steamworks` 0.13 has no safe
layout-change callback (`docs/steam.md` S30). Prompts can afford to lag, so the options are,
cheapest first: ask on the events that can change the answer (the window gaining focus, which S30
found is when a rebind shows, and a pad connecting or going), for a short while after each rather
than once, since how many frames Steam takes to answer with the new layout after focus arrives is
unmeasured, and the frame count logged from focus to the first changed answer sizes that window;
check one action per frame in rotation; ask on a timer; skip frames with no prompt on screen; or
receive `SteamInputConfigurationLoaded_t` through an `unsafe impl Callback`, which first needs
measuring whether it fires for a rebind in the panel. Events with a slow rotation behind them
combine well. The comment in `poll` points a reader at these.

### X28 — A stepper adjusted by a pad Steam owns

**Gate:** a Steam build whose screen has a stepper.

Chunk 151f binds `Activate` to an authority and leaves `Adjust` on the keyboard, because no stepper
remains on the Steam controls screen. The base game gives Adjust only the D-pad's left and right,
and only while a stepper has focus, so up and down still navigate. A Steam set gives an input to one
action, so the faithful counterpart is an action-set layer switched on while a stepper has focus,
not the D-pad in joystick mode for the whole menu. `adjust_focused_stepper` also passes an analog
value through as the step, so it would need a sign taken first.

### X29 — Screenshots of the Steam build, driven by the keyboard

**Gate:** the next change to what Steam Disasteroids shows on screen, where reading the code is the
only check today.

The build adds `RemoteDriverPlugin` and the umbrella's `bevy_remote`; `run.py` learns a plan naming
a manifest and a `--bin`, and extends `DYLD_FALLBACK_LIBRARY_PATH` from the build scripts'
`linked_paths` in Cargo's JSON, which is how `cargo run` finds `libsteam_api.dylib` (S15). No chunk
149: a keyboard binding is not an authority binding, so `poll` overwriting `AuthorityValues` never
touches it. Run once with Steam absent and once with Steam, a pad and a bound layout, which is a
manual precondition the plan asserts on first. The plan never presses **Change in Steam**, which
takes focus. Measured here: whether Steam's overlay toast appears in a game Steam did not launch.

### X57 — The Steam build playing the pad without Steam

**Gate:** a game asks to ship one binary with Steam optional.

Chunk 112a made the bindings declarable: each pad action can bind the base game's control beside its
authority. What is left is the example. It builds with `bevy_gilrs`, starts the Steam client before
adding `DefaultPlugins` and disables `GilrsPlugin` if Steam Input starts, links `pad_presets.rs` so
Southpaw applies in a direct launch, and under Steam draws the pad column as "Change in Steam" alone
rather than the controls' rows. No filter: disabling the plugin keeps the pad out of Bevy's own
entities too (D93).

## 5. The remote driver

These leave with the driver, into its own documents, if it moves out of this repository.

### X45 — What the driver becomes

**Gate:** the driver proposal answered, which says which of the driver's methods Bevy takes. It is
`docs/proposals/remote-driver.md`, posted by the author as a gist on 26 September 2026.

The driver is `publish = false` (chunk 164), and nothing in `bevy_action_map` needs it published.
What Bevy declines is the candidate for a standalone crate, and the Python client goes with it, or
is rewritten, since a script does not ship well inside a published crate.

### X31 — Isolating an app under test from what it has saved

**Gate:** an app whose state a plan cannot reach through its own controls.

A run shares the developer's settings file: Disasteroids saves a confirmed rebind, so chunk 148's
plan found its own last run's rebinding still there and failed on the second run. It now bookends
itself with Reset and Confirm (DD9), which is a plan reaching a known state the way a player would —
the right answer while an app offers one, and it exercises two more paths besides. What it does not
cover is a plan that fails halfway, which leaves the file dirty for the next run to reset. Deleting
the file first is the obvious answer and is half of one: it makes a run repeatable without stopping
it writing the developer's real settings on the way out, which the bookend does handle. Isolation is
the whole answer. At rc.2 it costs a platform branch — `XDG_CONFIG_HOME` on Linux, `LOCALAPPDATA` on
Windows, and on macOS `HOME` itself, since `preferences_dir` is `home_dir()/Library/Preferences`
with no narrower lever. [bevy#25902][], merged to main on 24 September 2026, not in rc.2, and
milestoned for 0.20, makes `preferences_dir` honour an absolute `BEVY_SETTINGS_DIR` on all three, so
past it isolation is one variable in `environment()` in `run.py`. Whichever is built wants a check
that the throwaway directory was actually written, because a path or variable that is wrong fails
silently: nothing is deleted, or the redirect does not take, and the plan passes while reading the
real file. `SettingsPlugin`'s `app_name` is the directory component and is a plain `pub` field, so a
per-run app id would isolate with no platform code at all — turned down because the example would
have to read an env var, and a plan drives an example without the example knowing it is under test.

### X32 — A step of the mapper's own, or a Python helper over `call`

**Gate:** a second plan whose raw `call` steps are unreadable.

Both extension points exist: BRP registration is the Rust one, and the mapper's `remote` feature
adding `action_map.*` is already a plugin adding methods with no dependency either way; a Python
plan is a program (DD4.2), so a helper is a module it imports. What is deferred is sugar over those,
and a step registry would be a third mechanism where two already reach.

### X33 — A virtual pad that says which pad it is

**Gate:** a plan whose subject is brand-specific presentation.

The connection event carries `name`, `vendor_id` and `product_id`, and the client fills all three
with a pad no vendor table knows, so every machine sees the same fallback rather than whatever is
plugged in. Exposing them is three optional arguments and no new mechanism; what is missing is a
plan that would read differently for a pad a game recognizes, which is X7 from the other side.

### X34 — A second virtual pad, and disconnecting one

**Gate:** a plan driving Split Friction, or one whose subject is a pad going away.

The client holds one pad's entity and connects it on first use, so a second is another entity and a
`pad` step that says which; a disconnect is one more message on the one it already has. Neither is
hard and neither has a caller: Disasteroids is one player who never unplugs anything, so nothing in
tree can tell a pad from the pad.

## 6. An outside target

### X35 — Netcode injection and reconciliation

**Gate:** a networked target.

The injection point is built: an `Authority` binding and `AuthorityValues` (chunks 111 and 151a), so
a peer's resolved action already has somewhere to go (D71). Rollback's local half — snapshot,
restore, re-simulate — is chunk 83, which also takes the held-state containers. Injection targets L2
(D69): a network authority backend supplies the already-resolved `ActionValue`, not a raw frame, so
no shared `Plan` across peers and no hold timers or tap counts on the wire. What is left here needs
a remote player's resolved action to inject and a later correction to reconcile against it.

### X58 — Pumped sampling (R9.9, withdrawn)

**Gate:** a game that needs to choose when input is consumed, lockstep netcode the likeliest.

Two ways exist without new API, neither tried. A run condition on `ActionMapSystems::Sample` stops
sampling, but Bevy's messages expire after about two updates, so a pause longer than that loses a
release and leaves the control held. A filter can hold back instead: it copies what `retain_sampled`
rejects into its own buffer and `record`s it into a later frame. Nothing expires, calibration is
already applied, and `FocusLost` still passes, but the events take the release frame's timestamps
and the `Filter` docs describe dropping, not re-recording.

- **What to ask:** which of the two fits their netcode, or what they would build instead. The answer
  decides whether this is a doc recipe, a helper, or a mode.

### X54 — Conditions that read other actions

**Gate:** the crate public (X43), and its users asked on the Bevy Discord whether a game wants a
condition that reads another action rather than a control.

The evaluator removed the main obstacle (D98): an action's state changes only when it commits, so a
commit can mark its dependents affected the way a control change does. Left are the condition that
reads another action, its builder method, commits ordered by dependency, and a cycle diagnostic at
plan build: a new mechanism with public API and a `TD` section, E[3], where it was E[4] before.

A chord may require another *control* but not another *action*, and `BlockedBy` does not exist. Both
read a neighbouring slot rather than their own value, which needs the operand evaluated first: slots
ordered topologically, and a cycle rejected at plan build with a diagnostic naming the loop. This
was chunk 33, inherited from the prior-art survey (R6.1's chord and blocked-by), and it has no
customer in tree: its planned demo blocked a Pong serve while the ball is in play, which is game
state, and `pong_countdown` already gates that by disabling `Serve` from a system (chunk 35).

- **Whether it subsumes `follow`**, inherited from chunk 44. An afterburner is genuinely "thrust,
  still held", and a game that could say that in a condition would need no link at all. `follow`
  says the mapping is shared, a condition says the value is derived; if both ship, check whether one
  should go.
- **What to ask:** whether a game gates one action on another's state in the mapper, or in a system
  as `pong_countdown` does, and what the case was. An answer naming game state is R6.5's argument
  and argues for withdrawing R6.1's two clauses.
- **One answer so far** (September 2026, asked before X43): `leafwing-input-manager`'s maintainer
  prefers gating one action on another at the simulation level, not in the mapper.

### X47 — Measuring a stick's rest envelope

**Gate:** a game asking the crate to measure calibration rather than set it, or someone with the
domain expertise offering to own it.

Chunk 72b withdrew `CalibrationSampling`, which recorded every reading while it existed and told the
player to move the sticks and let go, so a step that followed the instruction learned the deflection
as rest and killed the axis. The instruction was right: a pad reports an axis only when it changes,
so a stick that settled before the step reports nothing. The fixes considered were two game-driven
phases that sample only after release, with a settle wait; restarting an axis's min and max when a
reading jumps past about 0.5; and taking the envelope across each settled position. The second was
the lean. The test fixture is a DualSense whose left stick settles at a different point after each
release, so one release is not a calibration of it.

### X36 — Timestamped authority transitions

**Gate:** an authority that has edges to give: a network peer, or a replay backend.

`AuthorityValues` is a level sampled once a tick (D92), so a press and release between two of an
authority's writes never reach the action and its resolution is its own poll. A peer or a replay
that recorded the transitions would gain, and so might Steam Input: its action event callbacks may
deliver ordered edges between polls, which `docs/steam.md` carries as an unmeasured question.
Ordered is enough, since D4 has timestamps order events rather than time them. What it needs is
somewhere to put them.

### X37 — Opaque platform-user identity (R15.9)

**Gate:** a real platform SDK.

Floated for Split Friction, but Steam has one account per machine rather than one per controller, so
even 151e has nothing to attach.

### X38 — Suspend/resume (R16.3; mobile, console)

**Gate:** a platform target that needs it.

Nothing in this crate's supported platforms emits a suspend signal or has a device re-enumeration
step to hook.

### X7 — The generic tier's art

**Gate:** someone outside this repository wanting the Kenney provider, which is when it leaves
`examples/common/` and has to be offered with no holes in it.

Kenney's generic set is blank, unlabeled buttons, so each generic face button needs a short text
stamp authored onto it by hand: content work with a known answer, and no code. Until then an
unrecognized pad's face buttons resolve to text, which is also where the fallback chain shows itself
in tree, on Disasteroids' Cancel and Confirm and in the gallery's Generic brand.

### X55 — Skipping the closing run for a binding that did not change

**Gate:** a game's profile, or the `eval` benchmark on a plan larger than its own, showing the
closing step as the evaluator's cost.

The closing step runs every enabled binding's pipeline, changed or not (D98), because a
time-dependent stage (a hold, a pulse, a tap window) has to see the tick's `delta`. A binding whose
reading did not change and whose stages hold no time could skip it and keep its last output. After
chunk 181e an idle tick is level with 181c's, and the remaining cost is small plans with many
instances, about 3 to 5 percent over 181c. The work is a per-binding flag at plan build for "no
time-dependent stage", and the closing step honouring it: E[2].

### X56 — A player-facing "ignore this controller"

**Gate:** someone asks for it.

This was chunk 178: a setting keyed by `Identity`, as a game's stored calibration is, so under gilrs
ignoring one pad ignores its model, shown on the Disasteroids controls screen and written as an L0
filter (`ActionMapSystems::Filter`, built by chunk 112b) that unpairs the pad when the list changes.
An ignored pad stays listed so it can be un-ignored; telling two identical pads apart is out, since
Bevy's gamepad backend has no identity per unit. Deferred on Discord feedback that a player can
unplug a bad pad, which answers the measured case, the Switch-protocol clone's phantom presses (the
R14 notes). It does not answer a device that stays plugged in: a virtual duplicate from DS4Windows,
a wheel or stick kept for another game, a built-in HID device that reports itself as a joystick.
Those reach players only if the setting ships without a developer building it, so it waits for a
request rather than being written into an example.

### X59 — An alternative to a chord (R20.3, R20.6)

**Gate:** a game asks for a way to perform a chord without pressing its controls together.

R20.3 was a SHOULD inherited from the accessibility guidelines R20 cites, not from shipped games,
which mostly leave simultaneity to platform aids: adaptive controllers, Xbox Copilot, the OS's
sticky keys. It is a MAY now. What a player needs is narrower than a sequence: that a chord can be
replaced by a single control. Capture rebinds only a chord's base key, so remapping does not do that
today.

- **Keyboard:** the planned interactive editing of modifier keys covers it if *no modifier* is one
  of the choices.
- **Gamepad:** a single-press alternative the game ships, as a second binding or a preset, may be
  enough with no new mechanism. Whether a preset can replace a chord with one control is unchecked.
- **Sequence or sticky modifier** (R20.6): only for a chord the game reserves. The general sequence
  condition was withdrawn with R6.4; the sticky modifier (press the modifier, release it, press the
  key) is the narrower of the two.

## 7. Tooling, and other projects

### X39 — A devfmt usage log, to catch the misses nobody notices

**Gate:** hand reflows still happening now that repacking is canonical.

Measured before deferring: 350 loose breaks in tree against 6 pure rewraps in 60 commits, so the
aftermath of a devfmt run lives in working-tree churn and not in history — `git log` cannot be mined
for it, and devfmt is the only thing positioned to see it. The shape, if it revives: devfmt appends
to a gitignored log from the process already being run, costing no approval and no tokens; per
paragraph it records a hash of the word sequence and a hash of the physical lines, so a later run
finding the same words under different line breaks has caught a miss and can attribute it to its own
earlier decision. Worth building only with a mechanical trigger to read it — one line of output when
the count crosses a threshold — since a log nobody opens is cost with no signal.

### X40 — Repacking the prose `devfmt` never reached

**Gate:** the residue stopping its own shrink.

Prose wrapped before repacking existed and never repacked since; `--diff` repacks whatever a commit
touches, so what is left is the paragraphs in files nothing is working on, and a sweep buys less
each month. Measured today, files needing a reflow: `src` 21/25, `docs` 6/6, `examples` 22/38,
`macros` 1/1, `tests` 2/12. A `--sweep <dir>` is warranted when that stops falling, or ahead of
reading a directory end to end. What rides with it: six backtick spans broken across two lines
(`Requirements.md`, `Roadmap.md`, `docs/design.md`, `docs/issues.md`,
`crates/bevy_action_map/src/binding/control.rs`, `examples/split_friction/main.rs`), left by the bug
117m fixed. `tools/devfmt/src/main.rs` is swept by hand or not at all — its fixtures are string
literals full of `///`, which is X41. `archive/` is excluded: nothing in flight reasons from it.

### X41 — `devfmt` reading a comment marker inside a string literal

**Gate:** a second file in tree acquiring one.

A `.rs` line whose trimmed text starts with `//` inside a string literal is reflowed as though it
were a comment — the module header's "does not occur in idiomatic Rust" assumption, which `devfmt`'s
own test fixtures are the sole counter-example to, and they are also the one file a devfmt chunk
edits. So the cost today is a hand check on a file already under review, not a corruption nobody
sees. Telling the two apart needs a Rust lexer carrying string state, raw strings and `\`-continued
literals, which is a different tool from the line classifier this is built on; a second file
acquiring one is what changes that arithmetic.

### X44 — Refreshing `docs/architecture.md`

**Gate:** the author calling a refresh.

The tour is a snapshot, frozen so that a chunk does not pay to keep its diagrams current. A refresh
is one batch: `git log` from the file's own last commit, over `docs/design.md` and
`crates/bevy_action_map/src/`, lists what has moved since. Validate every Mermaid block by rendering
it (`mmdc` through `npx`), since GitHub shows a broken one as its source text with no error.

### X42 — Guardian migration

**Gate:** Guardian running on Bevy 0.20, still on `bevy_enhanced_input`.

It is on Bevy 0.16.1 with `bevy_enhanced_input` 0.12, four versions back, so moving it to this crate
is a port plus a rewrite. Porting first keeps the two apart: doing both at once would confuse
"action_map is wrong" with "0.20 moved this".

### X60 — A letter prefix for issue numbers

**Gate:** a chunk number reaching 900.

Issues are numbered from 1000 so that `show.py` can tell a bare four-digit number from a chunk with
no prefix (chunk 185), and `xref.py` relies on the same split (chunk 186). Once chunk numbers come
near that range, issues take a letter prefix like every other anchor, and both scripts follow.

---

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
[bevy#25592]: https://github.com/bevyengine/bevy/issues/25592
[bevy#25596]: https://github.com/bevyengine/bevy/issues/25596
[bevy#25675]: https://github.com/bevyengine/bevy/pull/25675
[bevy#25842]: https://github.com/bevyengine/bevy/issues/25842
[bevy#25757]: https://github.com/bevyengine/bevy/discussions/25757
[bevy#25847]: https://github.com/bevyengine/bevy/pull/25847
[bevy#25902]: https://github.com/bevyengine/bevy/pull/25902
[bevy#25824]: https://github.com/bevyengine/bevy/pull/25824
[bevy#25890]: https://github.com/bevyengine/bevy/pull/25890
[bevy#25944]: https://github.com/bevyengine/bevy/issues/25944
[winit#4606]: https://github.com/rust-windowing/winit/issues/4606
[winit#2678]: https://github.com/rust-windowing/winit/issues/2678
