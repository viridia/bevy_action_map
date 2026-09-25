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

**Next: 43.**

**How it is grouped.** By the kind of gate, so the question "has anything fired?" is asked of one
group at a time: a Bevy version bump is the first group, and nothing else.

---

## 1. Bevy or winit moving past the pin

Each of these is merged upstream after 0.20.0-rc.1. On every bump, check each against the new
version.

### X1 — Deleting the gamepad message registration

**Gate:** this crate's Bevy pin moving past [bevy#25904][], merged to main on 23 September 2026,
after rc.1.

`RemoteDriverPlugin` registers `ReflectMessage` for `GamepadConnectionEvent` and `RawGamepadEvent`,
because `bevy_input`'s nine gamepad messages carry no `reflect(Message)` where the ten in its other
input modules all do, and `world.write_message` refuses a message without it (DD5.3). Whether it
makes 0.20 final is not known, so the registration ships rather than waits. What to check on a bump
is only whether the pinned version has it, because nothing else will say so: `register_type_data`
over data a type already carries is an overwrite rather than an error, so the redundancy is silent.

### X2 — Deleting `acquire_focus_directional`

**Gate:** this crate's Bevy pin moving past [bevy#25675][], merged to main on 24 September 2026,
after rc.1.

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

### X3 — Deleting `examples/common/font.rs`

**Gate:** this crate's Bevy pin moving past [bevy#25847][], which answers [bevy#25842][] and merged
to main on 22 September 2026, after rc.1.

Until then the plugin overwrites the `default_font` feature's slot at `AssetId::default()` during
plugin build, which depends on that slot's location and on text layout registering a font id once.
The answer is a `DefaultFontSource` resource that `FontSource::Default`, now `TextFont`'s default,
resolves to. Changing it rebuilds the font collection and marks every `TextFont` changed, so it
reacts to a change and the build-time ordering goes with the file.

### X4 — Dropping the pre-scaled inline glyph art

**Gate:** this crate's Bevy pin moving past [bevy#25767][], merged to main after rc.1, which gives
`InlineImage` a fixed `width` and `height`.

At rc.1 an inline image sizes itself from the loaded image's pixel dimensions, so `prompt_ui.rs`
loads inline glyphs from a second, pre-scaled `input_prompts_inline/` tree. With a size on the
component, the inline prompt can load the block art and set its height from the line. Steam's art is
the same case: its smallest glyph is 32 pixels against the in-tree inline art's 25, so the Steam
build's inline prompts stand taller than the line until then.

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

### X6 — A presentation crate (`bevy_action_map_ui`)

**Gate:** Bevy deciding to take this crate upstream, which is when the workspace has to be arranged
properly regardless.

Until then the layer is `examples/common/` — `prompt_ui.rs` and `widget_focus.rs`, both written
against the public API with nothing added to the crate for them. What is deferred is packaging, not
work; the cost of waiting is a `#[path]` import. The crate's docs owe a game the warning
`widget_focus.rs` carries today: `InputDispatchPlugin` in `DefaultPlugins` activates a focused
`Button` on a key a context has consumed, so a game using both disables it. The crate ships no art:
Kenney's set and a backend's such as Steam's are sibling sources, both out of tree, where
`prompt_ui.rs` today treats the in-tree Kenney atlas as the base and Steam's art as an override on
top of it.

### X7 — The generic tier's art

**Gate:** X6, which is when the Kenney art source has to be offered with no holes in it.

Kenney's generic set is blank, unlabeled buttons, so each generic face button needs a short text
stamp authored onto it by hand: content work with a known answer, and no code. Until then an
unrecognized pad's face buttons resolve to text, which is also where the fallback chain shows itself
in tree, on Disasteroids' Cancel and Confirm and in the gallery's Generic brand.

### X8 — Promoting `WidgetKind` and the per-kind context into the crate

**Gate:** [bevy#25592][], the author's own upstream proposal for a `bevy_ui_widgets`-native
widget-kind id.

Promoting a shape this crate invented first, ahead of that conversation, risks committing to the
wrong one.

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

### X11 — Schedule enforcement for tick domains (D9's remainder)

**Gate:** Bevy giving a `SystemParam` a way to know its own schedule.

A plugin-time validation pass and a debug assertion stand in.

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

## 5. The remote driver

### X30 — Proposing the driver's server methods upstream (DD8)

**Gate:** chunk 148's plan passing, and a second plan against a different example.

Four gaps: selection by name path, where a UI node is drawn, readiness as state, and a reflected
`DiagnosticsStore`. The deliverable is a short brief for each, for the author to edit and post.
Until two apps have used the methods, their shapes are guesses about what a test needs.

### X31 — Isolating an app under test from what it has saved

**Gate:** an app whose state a plan cannot reach through its own controls.

A run shares the developer's settings file: Disasteroids saves a confirmed rebind, so chunk 148's
plan found its own last run's rebinding still there and failed on the second run. It now bookends
itself with Reset and Confirm (DD9), which is a plan reaching a known state the way a player would —
the right answer while an app offers one, and it exercises two more paths besides. What it does not
cover is a plan that fails halfway, which leaves the file dirty for the next run to reset. Deleting
the file first is the obvious answer and is half of one: it makes a run repeatable without stopping
it writing the developer's real settings on the way out, which the bookend does handle. Isolation is
the whole answer. At rc.1 it costs a platform branch — `XDG_CONFIG_HOME` on Linux, `LOCALAPPDATA` on
Windows, and on macOS `HOME` itself, since `preferences_dir` is `home_dir()/Library/Preferences`
with no narrower lever. [bevy#25902][], merged to main on 24 September 2026, after rc.1, makes
`preferences_dir` honour an absolute `BEVY_SETTINGS_DIR` on all three, so past it isolation is one
variable in `environment()` in `run.py`. Whichever is built wants a check that the throwaway
directory was actually written, because a path or variable that is wrong fails silently: nothing is
deleted, or the redirect does not take, and the plan passes while reading the real file.
`SettingsPlugin`'s `app_name` is the directory component and is a plain `pub` field, so a per-run
app id would isolate with no platform code at all — turned down because the example would have to
read an env var, and a plan drives an example without the example knowing it is under test.

### X32 — A step of the mapper's own, or a Python helper over `call`

**Gate:** a second plan whose raw `call` steps are unreadable, which is X30's gate as well.

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
(`Requirements.md`, `Roadmap.md`, `docs/design.md`, `docs/issues.md`, `src/binding/control.rs`,
`examples/split_friction/main.rs`), left by the bug 117m fixed. `tools/devfmt/src/main.rs` is swept
by hand or not at all — its fixtures are string literals full of `///`, which is X41. `archive/` is
excluded: nothing in flight reasons from it.

### X41 — `devfmt` reading a comment marker inside a string literal

**Gate:** a second file in tree acquiring one.

A `.rs` line whose trimmed text starts with `//` inside a string literal is reflowed as though it
were a comment — the module header's "does not occur in idiomatic Rust" assumption, which `devfmt`'s
own test fixtures are the sole counter-example to, and they are also the one file a devfmt chunk
edits. So the cost today is a hand check on a file already under review, not a corruption nobody
sees. Telling the two apart needs a Rust lexer carrying string state, raw strings and `\`-continued
literals, which is a different tool from the line classifier this is built on; a second file
acquiring one is what changes that arithmetic.

### X42 — Guardian migration

**Gate:** Guardian running on Bevy 0.20, still on `bevy_enhanced_input`.

It is on Bevy 0.16.1 with `bevy_enhanced_input` 0.12, four versions back, so moving it to this crate
is a port plus a rewrite. Porting first keeps the two apart: doing both at once would confuse
"action_map is wrong" with "0.20 moved this".

---

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
[bevy#25592]: https://github.com/bevyengine/bevy/issues/25592
[bevy#25596]: https://github.com/bevyengine/bevy/issues/25596
[bevy#25675]: https://github.com/bevyengine/bevy/pull/25675
[bevy#25767]: https://github.com/bevyengine/bevy/pull/25767
[bevy#25842]: https://github.com/bevyengine/bevy/issues/25842
[bevy#25847]: https://github.com/bevyengine/bevy/pull/25847
[bevy#25902]: https://github.com/bevyengine/bevy/pull/25902
[bevy#25904]: https://github.com/bevyengine/bevy/pull/25904
[winit#4606]: https://github.com/rust-windowing/winit/issues/4606
[winit#2678]: https://github.com/rust-windowing/winit/issues/2678
