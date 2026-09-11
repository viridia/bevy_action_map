# Build plan: `bevy_action_map`

The work that is left, and the gaps that are known. It orders that work into chunks small enough to
review individually — a *sequence*, not a schedule, and any chunk may be reordered once the one
before it has been read.

**What this document admits.** Work not done, and gaps. The test is whether an entry names something
that will change; if it describes the present, it belongs in [docs/design.md](./docs/design.md), and
if it explains why the present is as it is, it belongs in
[docs/decisions.md](./docs/decisions.md). What has been built is described in exactly one place, and
this is not it. The landed index below names chunks; it does not describe the crate.

Ground rules, house style and the commit-message convention are in [CLAUDE.md](./CLAUDE.md);
they are about the work rather than about the crate. Ground rule 5 is the one this document is
built around: nothing outstanding may be left without a destination, and a row here with no gate
is an item that will be dropped.

---

## Where this stands

[docs/design.md](./docs/design.md) is what the crate does today. What follows is only the delta:
what is wrong, what was never built, and what is left to do.

The target the remaining sequence aims at is **Disasteroids** — an asteroids-like game playable on
keyboard or gamepad, with a rebinding screen built on `bevy_ui_widgets` and operable from the
controller. It is not a phase of its own; it arrives early, badly, and grows a capability per chunk,
because ground rule 3 wants something runnable at every step and a real game is a better acceptance
test than a synthetic one.

### Known wrong

Defects, as distinct from limitations that were accepted deliberately — those are decisions and live
in `docs/decisions.md`, where each says what reversing it would cost.

- **`InputDispatchPlugin`, left enabled, bypasses consumption.** `bevy_ui_widgets::Button` activates
  on `Space` from a `FocusedInput<KeyboardInput>` that asks the mapper nothing, so a focused button
  answers a control a context has claimed (R8.2a). What is wrong is the *default*, not the
  capability: a context per widget kind answers it, and Disasteroids ships that way, disabling the
  plugin outright. What is wrong is that `DefaultPlugins` brings the collision and nothing tells a
  game to opt out. The generic form is in the deferred table.
- **A refused capture is silent on Disasteroids' screen.** Wrong shape, wrong device family, or
  reserved, and the session simply keeps listening with nothing said about why the press did not
  take.
- **`R23.2` is unenforced.** No allocation and no synchronization on the per-tick path is a rule
  with no tooling behind it. Two violations have reached that path and both were caught by reading.

### Never built

- **Glyphs.** The art is in `assets/input_prompts/` and the identifier scheme holds against it, but
  no type names a glyph and nothing resolves one. R18.4 specifies the identifier; no code declares
  it.
- **Writing a saved override set to a file.** The crate serializes and deserializes one; where the
  bytes go was always the app's decision — chunk 92.
- **A snapshot of a context's state.** The shape is designed and written down; nothing has taken one
  — chunk 83.

### Upstreaming, if it happens

There is a possibility this crate is taken upstream into Bevy. It is **not committed**, and nothing
here is built on the assumption that it will be. The rule is that the possibility may influence the
*shape* and the *order* of what gets built, but no work happens that a third-party crate would not
want anyway. What that changes today:

- **The extensibility mechanism is public API**, cheap to change now and breaking later, upstream
  or not.
- **The presentation-crate row names this as its own gate**, so if it happens, that split is decided
  as part of the plan rather than discovered in the middle of it.

What it does **not** change: there is no upstream repository, no PR sequence, and no
re-implementation. If it goes ahead it would be a fresh implementation staged as reviewable PRs —
the first around 1,000–2,000 lines, keyboard only — written against this crate as a model so it can
skip the blind alleys this one took. The commitments that follow from the possibility are recorded
as decisions rather than here.

---

## What has landed

An index, not a description. Chunk numbers are stable identities cited in commit messages and in
code comments, so the sequence stays recoverable; what each chunk delivered is in git.

| #   | Chunk                                             |
| --- | ------------------------------------------------- |
| 1   | Workspace and module skeleton                     |
| 2   | Action identity, value, and intent                |
| 3   | Derive macros                                     |
| 4   | Input frame, keyboard only                        |
| 5   | First end-to-end slice                            |
| 6   | Axis sources and composites                       |
| 7   | Modifiers                                         |
| 8   | Gamepad and the design-stage deadzone             |
| 24  | Housekeeping                                      |
| 9   | Tick domains and the windowed drain               |
| 15  | Source channel shape                              |
| 16  | Disasteroids, first playable                      |
| 12  | Transition log and observers                      |
| 13  | Context activation lifecycle                      |
| 11  | Conditions and the scratch table                  |
| 14  | Arbitration and consumption                       |
| 32  | Activation by run condition                       |
| 17a | Runtime failures (R24.4)                          |
| 17b | Plan-build diagnostics                            |
| 36  | Type-erased inspection and the overlay            |
| 18  | Derive completion                                 |
| 19  | Mappings and localization keys                    |
| 37  | Naming a control                                  |
| 20  | Interactive capture, conflicts, reserved controls |
| 39  | A mapping holds a list of slots                   |
| 41  | Mouse buttons                                     |
| 43  | Listed by default                                 |
| 21  | The settings screen, read-only                    |
| 40  | Reverse lookup                                    |
| 47  | A binding as a text span                          |
| 29  | Directional navigation                            |
| 30  | The settings screen, interactive                  |
| 44  | Bindings that travel together                     |
| 50  | What a held control says                          |
| 53  | A context the player never sees                   |
| 38  | Applying a rebind                                 |
| 54  | Conflict policy                                   |
| 25  | Control classes and class bindings                |
| 56  | Split Friction's tileset                          |
| 57  | A generated dungeon                               |
| 58  | `follow` replaces per-binding `follows`           |
| 31  | The settings screen, rebinding                    |
| 61  | Exclusive contexts                                |
| 45  | Presets                                           |
| 23  | Persistence of overrides                          |
| 55  | A file a person can read                          |
| 62  | Release on focus loss and disconnect              |
| 64  | Tunables and hold-vs-toggle                       |
| 67  | Per-entity `apply_overrides`                      |
| 26  | Device routing (core)                             |
| 66  | The join gesture                                  |
| 68  | Split-screen cameras and protagonists             |
| 69  | AABB collision                                    |
| 27  | Split Friction's device selection                 |
| 10  | The compiled plan, and the state layout settled   |
| 22  | The deadzone chain, stages 1 and 3                |
| 52  | What the crate has accreted                       |
| 74  | One admissibility rule, not two                   |
| 75  | Four names for a 2×2, twelve times over           |
| 80  | Same-priority contexts, ordered                   |
| 81  | A rebound row keeps its declared capacity         |
| 84  | A save file from a build that came later          |
| 87  | A stick is a control                              |
| 82  | A text field beside a live context                |
| 85  | A dead zone at full deflection                    |
| 86  | `active` and `is_active`, told apart               |
| 93  | A shared toggle ignored the hold setting          |
| 79  | `ActionPhase` tells building from firing           |
| 88  | The gamepad-settings warning sees the global thresholds |
| 76  | `Unresolved`, once                                |
| 70  | Device brand and class                           |
| 91  | The crate does not do what its own documents say  |
| 17c | The two normalizes                                |
| 89  | `why_not` can see the pairing                     |
| 48  | Names that survive a glob import                  |
| 90  | A context nobody declared says so                 |
| 96  | `BindingModifier`'s blanket `Modifier` impl       |
| 97  | A binding whose only conditions are blocking fires at rest |
| 95  | Pong, shared across the single-concept demos          |
| 106 | A diagnostic overlay for Split Friction                |
| 107 | A diagnostic overlay for Pong                          |
| 99  | The smart bomb, and a charge meter                     |
| 100 | A preset can carry a tunable                          |
| 51  | The constitution, trimmed                             |
| 77  | `context.rs`'s test fixtures, deduplicated and reordered |
| 78a | `DeviceFamily` moves to `device.rs`                   |
| 78b | `binding.rs` is three files, and `mapping.rs` gains a library |
| 78c | `context.rs` is two files, plus a shared test fixture module |
| 94a | A binding can name the logical key, not only the physical one |
| 108 | Focus loss, regated                                   |
| 111 | An outside authority writes an action's value at L2   |
| 114 | Update Bevy, and migrate the BSN syntax               |

---

What is left, in semantic groups ordered roughly by priority. The order is a guide rather than a
schedule: any chunk may be reordered once the one before it has been read, and a chunk's number is
its identity rather than its position.

## Defects

Wrong answers from code that has already shipped. The full register is
[docs/issues.md](./docs/issues.md), which also holds the findings that carry no chunk and say
so per entry.

---

## Devices and players

Disasteroids is one player reading one set of bindings, so everything about device pairing is
invisible to it. Split Friction is the example that has to answer *which* device drove an
action, and most of this section is what it needs.

### 72. Device identity, and a pairing that survives a restart

R11.5: stable persistent device identity, distinct from the runtime handle, and Split Friction
putting each player back on the device they had.

- **Now carries calibration's persistence too.** Chunk 22 built the measuring and the applying keyed
  to the runtime handle, which is exactly what a persistent identity would key instead. This chunk
  carries `GamepadCalibration` across a restart alongside the pairing, or says why the two want
  different storage.
- **And the calibration step itself**, which has no in-tree caller: `CalibrationSampling` is driven
  end to end by tests but by no screen. A calibration a player performs and then loses on quit is
  worth little, so the screen and the persistence are one feature.
- **Verified by:** playing it, quitting, relaunching — the same protagonist on the same device
  without anyone pressing anything — and by unplugging a pad and plugging it back in.

### 103. A disconnect signal and a reconnect prompt

R15.5 (MUST) (`docs/issues.md` 1020): on device loss the owning player must be identifiable
(already true), in-flight actions canceled (already true), **and a signal raised so the app can
pause and show a reconnect prompt** (nothing today).

- **Split Friction.** A pad disconnects mid-game; the app pauses and shows "player 2, reconnect to
  resume" until the pad (or another) reappears.
- **What the signal is** is this chunk's open question: Bevy's own `GamepadConnectionEvent` read
  directly, or something this crate re-raises so a game does not have to know the pairing's own
  bookkeeping to react correctly.
- **A backend must be able to raise it too**, which settles that question by itself. Under a Steam
  authority no `GamepadConnectionEvent` is ever emitted — the crate learns of a lost pad from a raw
  event that suppression has already stopped (`docs/steam.md`, appendix) — so reading Bevy's event
  directly leaves a whole class of device silently un-disconnectable. Whatever this chunk raises
  has to be raisable by something that is not `gilrs`.

### 104. Named device-requirement sets at the join screen

R15.7 (SHOULD) (`docs/issues.md` 1042, split from 1020): named device-requirement sets with required
and optional devices — nothing exists under this name anywhere in `src/`.

- **Split Friction's join screen**, validating "this player needs a gamepad" versus "keyboard is
  fine" before a pane is handed a protagonist, rather than silently accepting any device.

### 105. Auto-switching a player's active scheme

R15.8 (SHOULD) (`docs/issues.md` 1043, split from 1020): auto-switching a player's active scheme on
input, with hysteresis — nothing exists, and R18.6's withdrawal names this as the one thing that
would revive it.

- **Split Friction.** A player on a pad picks up the keyboard instead; control follows without a
  menu trip. R18.6 stays withdrawn unless this chunk's hysteresis turns out not to hold up under
  real play.
- **`docs/issues.md` 1044 (R15.9, opaque platform-user identity)** stays unrouted alongside this —
  floated for Split Friction too, but nothing to show without a real platform SDK, and not yet worth
  a faked stub the way chunk 42 fakes a backend.

### 71. Per-player presets

Each protagonist selects its own preset, applied through `apply_overrides_for_with_preset`. The
preset is a **southpaw swap** — the real thing players ask local co-op games for, and small enough
that the point is the per-player selection, not the preset's own content.

- **What it proves.** Chunk 67 built the per-entity apply path ahead of a need and nothing in tree
  has called it since — this is that caller. A preset is the cheapest override to select, so the
  general per-entity case is validated without a second rebinding UI.
- **It trips a deferred row on purpose.** "Per-entity presentation and prompts" is gated on a
  per-player settings display existing, and a per-pane preset selector is one. Expect it to validate
  that row's sketch or falsify it, and say which.
- **Not a settings screen.** Selecting a preset is a join-screen or pause-menu affordance — a
  button per pane, not a rebinding UI. That stays Disasteroids' territory.
- **Verified by:** playing it — each pane selects independently, and the other pane's bindings do
  not move.

---

## Bindings and conditions

What a binding can name, and when it counts as firing. Each of these is a gap a game runs into
rather than a defect in what exists.

### 94b. Either modifier

R12.3: a chord's modifier should be able to say "either Ctrl", as one binding rather than two.
`with` takes a single `ButtonControl`, so a game wanting either `LeftCtrl` or `RightCtrl` to arm a
chord writes both bindings by hand today. R4.10 already assigns this to the chord mechanism by name,
so the requirement has a destination in `Requirements.md` and, until this chunk, none in the plan.

- **Self-contained**, and independent of 94c — a different corner of the same requirements
  section, not a shared mechanism.
- **Not doing: a mappable either.** The either-ness is declared in the chord's modifier at bind
  time; capture reads one physical keypress and can only ever answer with `LeftCtrl` or `RightCtrl`,
  never both. A row built this way is not something a rebind screen can produce, so it stays a
  `Fixed` binding — a game wanting a rebindable "either" still writes two mappable rows by hand.

### 94c. A platform modifier

R12.4: `Cmd` on macOS should be usable as `Ctrl` everywhere else, as a named modifier resolved at
binding time rather than something every cross-platform game re-derives by hand.

- **Resolved at binding time, not read time.** The name a game binds does not change per platform;
  what it expands to does, once, when the plan is built — not on every frame the control is read.
- **Self-contained**, and independent of 94b.

### 98. Pointer position, and a mouse-controlled paddle

R13.1, R13.4, R13.6 (`docs/issues.md` 1015): the frame carries mouse *motion* and no absolute
position, so a binding cannot target where the pointer is at all.

- **Depends on chunk 95.** The demo is a Pong variant, and the simplest one there is: a paddle whose
  position follows the mouse's Y coordinate directly, rather than reading a delta and integrating
  it.
- **Not doing: split-screen viewport mapping (R15.10).** That is Split Friction's problem once this
  mechanism exists, not this chunk's — a second pointer meaning "position within my own camera's
  viewport" is a follow-on, not part of proving position exists at all.
- **Review surface:** whether an absolute position needs the same dead-zone/rescale modifier chain
  a delta does, or is exempt as a different kind of channel entirely.

### 33. Conditions that read other actions

A chord may require another *control* but not another *action*, and `BlockedBy` does not exist. Both
read a neighbouring slot rather than their own value, which needs the operand evaluated first: slots
ordered topologically, and a cycle rejected at plan build with a diagnostic naming the loop.

- **Depends on chunk 95.** The demo is a Pong variant: serve is blocked while the ball is already in
  play, one `BlockedBy` condition on the serve action reading the rally's own in-flight state. No
  new content beyond what the base already has.
- **Inherited from chunk 44: whether this subsumes `follow`.** An afterburner is genuinely "thrust,
  still held", and a game that could say that in a condition would need no link at all. The two
  answer different questions — `follow` says the mapping is shared, a condition says the value is
  derived — but check when this lands whether the overlap is large enough that one should go, since
  carrying both when either would do is what an outside reader notices first.

### 34. Sequences

R6.4's ordered sequences — double-tap-dash, motion inputs, cheat codes — arriving in order within a
time window.

- **Depends on chunk 95.** The demo is a Pong variant: a double-tap on the move control gives that
  paddle a brief speed boost — a sequence read off the same control the paddle already binds, not a
  new one.
- **Fits the scratch record**, so this is a condition, not a redesign.
- **R6.5's forgiveness windows do not carry here** and are withdrawn. The crossing point in both
  directions is app-domain state the crate cannot see, and events plus elapsed time already give an
  app what it needs to compose the pattern itself.

### 35. Disabling an action

R3.7: an action switched off without being unbound, and switched back on without firing for a
control the player was already holding.

- **Depends on chunk 95.** The demo is a Pong variant: paddle movement is disabled for the serve
  countdown and switched back on when play resumes, without manufacturing a fire for a key the
  player was already holding through the countdown — the exact case R3.7 exists for, with no new
  content beyond the countdown itself.
- **The mechanism is probably already there.** `require_reset` is per slot and `StateFlags` has
  room; what is missing is the public verb and what it means for a disabled action's in-flight
  state. Cancel, on the same terms as deactivating a context, is the answer to beat.
- **Why it exists as its own chunk.** A `MUST` whose only record of a destination was in the log is
  exactly what ground rule 5 forbids.

### 102. A global timing scale

R20.4 (`docs/issues.md` 1045, split from 1021): every hold duration, tap window and repeat rate
globally scalable by one user preference, where today only per-mapping tunables exist.

- **Disasteroids' settings screen**, as an accessibility slider — "input timing ×1.5" — applied
  across every declared hold/tap/repeat value at once rather than one at a time.
- **Crate work, not only an example.** Scaling every timing by hand from the app side is exactly
  what R20.4 says a game should not have to do, so the scale factor's application point is a design
  question for this chunk rather than something the example alone can supply.

---

## Presentation and prompts

What the player is shown, once the crate knows what is bound.

### 73. A key rendered through a catalogue

Every `fallback_label` call in tree is unconditional: nineteen sites across `examples/`, not one
going through a catalogue. The crate's half of R19.14 is done, but the claim that those names are
*keys a localized game looks up* has never been exercised.

- **In an example, not the crate.** Rendering is the app's business.
- **Disasteroids, not Split Friction.** The keys being resolved are the settings screen's own
  binding labels, so the settings screen that already renders them is where a catalogue lookup
  replaces the unconditional `fallback_label` call — not a new UI.
- **What it is:** Disasteroids' renderer reading a catalogue file, a second locale to prove it
  switches, and the fallback kept for the key the catalogue misses.
- **Not fluent.** `bevy_fluent` is pinned to an older Bevy, and fluent's own value — plurals,
  gender, bidi — is orthogonal to whether our keys resolve, since they resolve to nouns. The one
  thing it would genuinely test is whether our key syntax collides with its identifier grammar, and
  that is a reading of the spec rather than a dependency.
- **Review surface:** whether the key is the one an author would actually want to type.

### 101. Semantic control aliases for a console confirm swap

R4.4 (SHOULD) (`docs/issues.md` 1040): semantic aliases (`Submit`, `Cancel`, `MenuLeft`) resolving
per device family, load-bearing for R18.7's console confirm-button swap rather than merely
convenient.

- **Disasteroids' settings screen**, which already does directional navigation (29): the same screen
  reads `Cancel` rather than a hard-coded key, so its on-screen prompt says the right thing on
  keyboard and on a pad without the app hand-rolling the swap itself.
- **Not doing:** a general aliasing mechanism beyond the three names R4.4 asks for — this proves the
  concept the requirement names, not a configurable alias table.

### 110. A glyph for a control, and an icon prompt

R18.4 and R18.9 (`docs/issues.md` retired 1016's sibling gates): glyph resolution returns an
identifier keyed by (brand, control), the app supplies the atlas, and a fallback chain of
brand → generic → text is required. Nothing declares a glyph type today; the only occurrence of
"glyph" in `src/` is a doc comment.

The art landed ahead of this and the key held against it: every one of the 164 entries in
`assets/input_prompts/` resolves as `format!("input_prompts/{tier}/{}.png", control.name())`, which
is what R18.4's key was waiting to be checked against.

- **The identifier is one variant of a sum, not the whole type.** R18.9 requires it: a backend
  answers with an opaque handle, raw bytes, or a filesystem path instead, and Steam actually does
  the last. Building the sum now costs nothing and retrofitting it costs a breaking change.
- **Resolution reads the manifest rather than probing for a file.**
  `assets/input_prompts/manifest.txt` says which (tier, control) pairs have art, generated by
  `scripts/import_input_prompts.py` from what it wrote. A pair absent from it steps to the next
  tier; a missing path is never a load error, and nothing in the chain opens a file to discover it
  is not there.
- **`IconPrompt` and `IconPromptSpan`, not one component for both.** An inline icon rides
  `InlineImage` ([bevy#25710][], now in the pin) and a block one is its own node; each falls back to
  text in its own layout kind rather than one component switching shape underneath its caller. The
  two are not merged for the same reason a span and a block element are never one type generally —
  their layout knobs differ, `InlineImage`'s are not all built yet, and a block prompt has to align
  like any other block element on its screen, which a component built around the inline case cannot
  promise. `IconPromptSpan` has landed; `IconPrompt` has not.
- **`InlineImage` sizes itself from the loaded image's own pixels, with no resize hook** — measured
  against a real window, not read off the PR: a 64px face-button icon inline with 15px text towers
  over the line rather than sitting in it, and nothing on the component overrides that once the
  asset loads. `assets/input_prompts_inline/` is the fix, a second copy of every entry pre-scaled by
  `scripts/import_input_prompts.py` (`INLINE_SIZE`, currently 25px) rather than a size this crate
  computes at runtime. One manifest still covers both directories, since they mirror each other's
  coverage exactly. The resize itself needs care past a plain `-resize`: the source stores white
  under full transparency, which a naive filter bleeds into the scaled edge as a halo, and a palette
  small enough to compress well flattens the antialiasing into visible bands — full RGBA output,
  composited over black and back rather than resized as one straight-alpha image, is what a smooth,
  fringe-free edge at this size actually needs.
- **Verified by Disasteroids' two gamepad prompt sites**, `Back` on `pad/East` and `Confirm` on
  `pad/West` in the settings screen. Face buttons are where brands diverge most visibly, so swapping
  a pad changes both glyphs; and because Generic ships no face-button art, an unrecognized pad falls
  through to text in exactly those two spots. The fallback demonstrates itself.
- **Not doing: icons in the capture cells.** The settings table has no width to spare, and the two
  surfaces want different things anyway — a prompt is a hint, where a glyph reads fastest, while a
  capture cell is an editor showing the authoritative name of what is being changed.
- **Not doing: the generic tier's stamped art.** Kenney's generic set is blank, unlabeled buttons,
  so a generic-tier icon needs a short text stamp authored onto it by hand. That is a content task
  with a known answer, and until it is done the generic tier resolves to text.

---

## Persistence and snapshots

Both take the same state out of a context and put it back: one to a file between runs, one to
memory between ticks.

### 92. Persisting bindings through `bevy_settings`

Chunk 23 wired `Overrides` into Disasteroids' settings screen; nothing has ever written one to disk.
"Writing a saved override set to a file" is still listed as never built, the destination left to the
app on purpose (D53). `bevy_settings` is that destination: register `SavedOverrides` (D59) as a
`SettingsGroup` resource and let it merge into the game's one settings file alongside whatever else
the app declares there.

- **The mechanism is validated, not open.** A scratch test against
  `bevy_settings::resources_to_toml` / `apply_settings_to_world` confirmed the whole path
  round-trips correctly: `SavedOverrides`'s own fields are walked structurally (no stutter, no
  bevy_settings change needed), and its nested `SavedRow`/`SavedTunableValue` fields bridge through
  `#[reflect(Serialize, Deserialize)]` to their own hand-written encoding rather than bevy_reflect's
  generic enum shape. This crate's own
  `overrides::tests::persistence::a_saved_override_set_round_trips_through_reflect` pins the same
  contract without a `bevy_settings` dependency. This chunk is the remaining wiring, not a risk to
  chase.
- **Load at startup, save on Confirm.** `apply_and_close` in `settings.rs` already applies a pending
  `Overrides` to the running game; this chunk adds a startup system calling `resolve_saved` against
  the loaded `SavedOverrides` resource before the first `apply_overrides`, and a `save_overrides`
  call on Confirm feeding back into it.
- **A dev-dependency of the examples, not the crate.** This crate publishes `SavedOverrides` and no
  opinion about where the bytes go (D53); `bevy_settings` is wired into `examples/disasteroids`
  only.
- **Not doing:** per-profile or per-scheme settings groups (R17.4), and anything Split Friction's
  two protagonists need — chunk 71 owns per-player preset selection once a settings group exists to
  read and write, this chunk owns getting one player's set to and from disk at all.
- **Retires the "Writing a saved override set to a file" row** in "Never built".
- **Verified by:** rebinding a control, quitting Disasteroids, relaunching it, and finding the
  binding still applied.

### 83. Rewind, without the network

`InputContextState`'s own comment says a rollback snapshot is the two tables plus the dirty bits,
and `docs/design.md` §6 says the same. Nothing has ever taken one. A ring buffer of snapshots and
the `InputFrame`s that followed each, with a key that rewinds N ticks and re-simulates forward, is
rollback's three requirements — snapshot, restore, deterministic re-simulation — with the
network removed, which was the expensive part.

- **The recorded transition log and the re-simulated one must match**, which is the assertion doing
  the real work. The visible rewind is what makes it a chunk rather than a test.
- **The held-state question is narrower than the deferred row made it sound.**
  `HashSet<MouseButton>` and `HashMap<GamepadButton, ButtonReading>` were called an obstacle to
  snapshotting, but `bevy_platform`'s maps default to `FixedHasher`, so iteration order is
  deterministic across runs and processes rather than randomly seeded. What is left is that order
  depends on insertion history, which bites only a snapshot serialized by iterating — and not one
  restored by value. This chunk says which kind it needs, and `indexmap` is the tool if the answer
  is the former.
- **The per-slot read it needs, `FixedBitSet::contains`, is already there and ungated** — `dirty`
  moved off the hand-rolled `DirtySet` to `fixedbitset`, which carries the method as a stock part of
  the type rather than something built for this chunk's sake.
- **What stays deferred:** injection and reconciliation — feeding a remote player's resolved action
  through the authority-backend seam (D69), and disagreeing with the authority about what happened.
  Those want a network and chunk 42's trait; rewinding does not.
- **Split if it grows.** Making the state snapshot-able with a differential test is separable from
  the example that rewinds, and ground rule 1 says that split happens before the code, not during.
- **Depends on chunk 95.** The visible rewind reuses its Pong base rather than a third vehicle — the
  same paddle-and-ball simulation snapshotted and re-simulated forward, with the recorded and
  re-simulated transition logs compared. Chunk 42 will already have proven the base can host one
  grafted concept without disturbing its own.
- **Check whether `docs/issues.md` 1041 belongs here.** R9.9's pumped sampling mode (stopping
  `InputFramePlugin` from scheduling its own sampling) was floated as a fit for a rewind demo, on
  the theory that re-simulating forward wants control over exactly when a frame is sampled. Confirm
  that before routing it here — if re-simulation does not actually need to suppress live sampling,
  1041 stays unrouted rather than getting a home it does not need.

---

## The library itself

Three chunks no game asks for and no published crate can do without: an extension point nothing
outside has exercised, the reflection the documents promise, and documentation that runs.

### 112. A backend suppresses a device family at L0

R0.6's other half, and the smallest it will ever be: `docs/steam.md` S1 and S3 killed the
per-device policy D22 assumed, so what is left is a family switch.

- **A resource naming the suppressed families**, read by `sample_input` (`frame.rs:304`) as a guard
  around each family's own `MessageReader` loop. Nothing above L1 changes, because a suppressed
  family simply never reaches the frame.
- **Declared by the game, not detected.** A Steam build launched with the client absent must read
  its pads normally, and a suppression that turns itself on and off between runs is the worst kind
  of input bug to diagnose. The backend's own plugin sets it once its init succeeds.
- **Not a Cargo feature**, for the two reasons D22 now records: it is a runtime fact, and a feature
  that removes behaviour is not additive.
- **Per family rather than per device** because per device is not implementable, not because it is
  cheaper. S3: Steam hands out an `InputHandle_t` and nothing relates it to an OS device. If Valve
  ever exposes that mapping the policy narrows and nothing above L0 notices.
- **Verification is headless and needs no Steam.** An `App` with `gamepad` enabled, the resource
  set, a synthetic `RawGamepadEvent` pushed through the sampler, and the frame asserted empty; the
  same test with the resource clear asserts it arrives. The eight-combination matrix covers the
  `cfg` interaction, since suppression has to compile with each family absent.
- **Not doing: keyboard and mouse.** Steam emulates both alongside the pad (S1) and the family
  switch would silence them wholesale, which is wrong — a player using a pad through Steam still
  types. Whether Steam can emit keys for a pad while the game reads Steam Input natively is
  unmeasured, and `docs/steam.md` carries the question. This chunk suppresses what a backend
  actually owns and leaves that one open.
- **Not doing: R0.4's per-action split**, which lives at L2 and is chunk 111's, already landed.

### 113. A device's brand is a component, not a lookup at every read

`GamepadBrand` is resolved from a vendor id at each read site today, and the read site has to reach
into Bevy's `Gamepad` component to find one — `examples/split_friction/split_screen.rs` does
exactly that. Resolution belongs once, where the knowledge is.

- **A `Brand(GamepadBrand)` newtype component** on the device entity. A wrapper rather than
  deriving `Component` on `GamepadBrand` itself, which is a value type used as a function
  parameter throughout `present.rs` and should stay one.
- **A system resolves and inserts it** on `Added<Gamepad>`, from `vendor_id` and the existing
  `GamepadBrands` table. `GamepadBrands` stays what it is — the policy, still `insert`-able by a
  game shipping a pad the defaults do not know — and gains the system that applies it. `resolve`
  stays public.
- **Consumers become `Query<&Brand>`** and stop caring what produced the entity. That is the point:
  an entity spawned by an authority backend carries the same component, so a read path written
  against it works for a device this crate never enumerated (`docs/steam.md`, appendix).
- **Justified without Steam.** The coupling it removes is in a shipped example today, and the
  per-read lookup it replaces runs every frame the split-screen HUD draws.
- **Settle where the component lives.** Inserting onto Bevy's own gamepad entities is the obvious
  reading and the one the examples want; an entity of this crate's own is the alternative, and the
  choice wants making rather than falling out of whichever is easier to write.
- **Verification is headless.** An `App` with a synthetic gamepad entity carrying a known
  `vendor_id`, asserting `Brand` appears with the right value; a second entity with no `vendor_id`
  asserting `Generic`; and one carrying `Brand` inserted directly, asserting the system leaves it
  alone — the case a backend depends on.
- **Not doing: prompts.** `present.rs` already takes a brand as a parameter rather than resolving
  one, so nothing there changes.
- **Not doing: `InputType` mapping.** Turning Steam's device taxonomy into a brand is the backend's
  job and lossy either way (`docs/steam.md` S7); this chunk only makes somewhere for the answer to
  live.

### 42. The authority backend, faked

The backend seam made real against something that is not Steam, because the seam is only proven by a
second implementer and the real one cannot live here.

- **Depends on chunk 111**, which landed the injection itself: `delegate::<A>()` and
  `AuthorityValues` (D71). What is left here is the half a settings screen asks for on demand rather
  than the half the evaluator reads every tick — origins, glyphs, whether an action is bound at all,
  and delegating a rebind (R18.8, R18.9, R19.8). That is where a trait in `src/backend.rs` earns its
  place, and where a source backend would be described if it needed anything beyond
  `InputFrame::record`, which it does not.
- **The mock lives entirely in `examples/`.** The traits are public API and carry a maintenance
  promise; the fake is a test fixture and gets deleted when a real backend exists.
- **It must fake the API, not the concept.** Level-only reads with no timestamps, an "is this bound"
  flag distinct from a zero value, origins as a type deliberately not `Control`, a glyph as a
  filesystem path, and a binding panel that is ugly on purpose. A mock nicer than Steam proves
  nothing.
- **The binding panel is also `docs/issues.md` 1022's demo.** R19.8 wants a backend-owned row to say
  "not rebindable here, delegate to that backend's own UI" rather than reading as an ordinary fixed
  one — this chunk's ugly-on-purpose panel is exactly where that distinction has to show up, so it
  is one more thing the panel renders rather than a separate chunk.
- **Depends on chunk 95.** The vehicle is its Pong base: one paddle's pad handed to the mock
  backend, the other reading normally, so R0.4's per-context split is the game rather than a
  contrived aside. A pause overlay over an in-progress rally supplies the second modal context the
  review surface below needs live on the same pad, and a hold-to-charge serve gives the
  backend-owned action an in-genre reason to carry a `.hold()`. The acceptance criterion is a
  non-diff: the court, the score, and the pause menu run unchanged, and only which context drives
  the backend-owned paddle changes. It is no longer Disasteroids' pad for the same reason it was
  never going to be — that is where presets get taught, and a pad the backend owns has no presets of
  ours to show.
- **R0.6, the half that is not about Steam.** A backend suppresses its devices at L0 so their raw
  events never reach the frame. Without it the demo reads the pad twice.
- **Review surface, and it is the point of the chunk.** One decision is still falsifiable here: an
  input observed twice. A decision this chunk cannot break is a decision that was not made.
- **Two modal contexts on one pad is measured, and the vehicle survives.** Steam runs one action
  set per controller and the last activation wins (`docs/steam.md` S19), so a set per context is
  out — but one control can drive several actions in one set and Steam arbitrates none of them
  (S20). The pause overlay over an in-progress rally works from a single set holding every
  delegated action, with this crate doing its own gating. `ActivateActionSetLayer` is not needed.
- **A backend-owned action accepting a `.hold()`** was the third falsifiable decision, and chunk
  111 made it unrepresentable rather than diagnosable, so the hold on the serve is now the local
  paddle's or nothing.
- **R0.5's queryable half is still owed.** A delegated action's value is indistinguishable from a
  bound one at the call site, which is the requirement's point, but nothing yet names *which*
  authority produced it: chunk 111 left `AuthorityValues` unnamed rather than adding a field with no
  reader. A chunk with two real backends in one build is where a name earns itself.
- **A delegated action has no row on a controls screen**, because it has no binding to derive one
  from, and `RebindPolicy` has no third state to say why. R17's "not ours" (`Requirements.md`
  §17.1042) and R19.8's delegate-instead outcome are both this chunk's, and the panel is what needs
  them.

### 109. Derive `Reflect`, and turn on auto-registration instead of hand-writing it

R24.3 (`docs/issues.md` 1019): `Control`, `DeviceFamily`, `ActionMapping`, `RebindPolicy`, `Prompt`,
`ControlOrigin`, `DeviceHandle`, `ActionObstacle` and `Paired` carry no `Reflect`, and nothing calls
`register_type` for the two types that do (`action.rs`, `frame.rs`). The second half looked like it
needed a call per type, written once and kept in sync forever after — but
[bevyengine/bevy#15030][] means it doesn't: a non-generic `#[derive(Reflect)]` type registers itself
at startup once `bevy_reflect`'s `auto_register_inventory` feature is on, which it isn't here — this
crate's `bevy_reflect` dependency is `default-features = false` and its own forwarded feature never
re-adds it.

- **Derive `Reflect`** on the nine types named in 1019, same as `action.rs` and `frame.rs` already
  do it.
- **Add `bevy_reflect/auto_register_inventory`** to this crate's own `bevy_reflect` feature
  (`Cargo.toml:94-101`), so every type above — and every one derived after this chunk — registers
  itself, rather than adding a `register_type` call this chunk would immediately have to write nine
  of and the next new type would silently skip.
- **Check the `no_std` interaction before assuming it holds.** `inventory` lists Linux, macOS, iOS,
  FreeBSD, Android, Windows and WebAssembly as supported, which says nothing about a `no_std` +
  `libm` build. Run the `libm` shape through `scripts/verify.sh --full`'s combination sweep with
  the feature on; if it fails there, `auto_register_static` is the documented fallback, but it
  "requires additional setup" per `bevy_reflect`'s own docs — scope that setup before promising it
  rather than after.
- **Verification:** a headless `App` that builds `ActionMapPlugin` and asserts `AppTypeRegistry`
  contains `Control`, `ActionMapping` and `Paired` with no `register_type` call anywhere in the
  test — the probe 1019 never got.
- **Not doing: `Modifier` or `Condition`.** Chunk 17c owns R5.6 and R17.5, and
  `docs/decisions.md:430` keeps those two deliberately Reflect-free; this chunk doesn't reopen that.

[bevyengine/bevy#15030]: https://github.com/bevyengine/bevy/pull/15030

### 28. Docs that run

- **Make the doctests execute.** `dynamic_linking` on the `bevy` dev-dependency breaks the merged
  doctest binary, so every `///` example compiles but none runs. Fixing it means making
  `dynamic_linking` opt-in, at the cost of slower example builds — a trade-off to make deliberately
  rather than inherit.
- **The README rewrite** — a user-facing introduction, feature list and quickstart, with examples
  lifted from a real game rather than invented.
- **`src/lib.rs`'s crate-level docs, alongside it.** The `//!` block largely mirrors the README's
  Concepts section and has drifted the same way — both were drafted early and neither has kept pace
  with what the crate grew into since.
- **Comparison upkeep.** [docs/comparison.md](./docs/comparison.md) is read against BEI 0.26.0 and
  LWIM 0.21.0, which is a claim with a date on it. Both crates move.
- **Review surface:** read the rendered docs, not the diff. `cargo doc --all-features --open`, and
  look at the module pages the way a stranger would.

---

## Deliberately deferred

Every row states its gate. A row with no gate is an item that will be dropped, which is ground rule
5.

| Area | Gated on |
| --- | --- |
| **A real Steam backend, validated out of tree** | less than it looked. `docs/steam.md` S4 now says a borrowed app id does carry its own manifest from inside the client's bundle, and S13 answered R1.7 that way, so what is left needing an app id is only what S6 blocks: a configuration Steam actually applies. Whether that needs an app id at all, or only a windowed app Steam launches, is itself unmeasured. The shape is settled: a probe rather than a game, in its own repository, pinning `bevy_action_map` by git rev so it breaks only on a deliberate bump — and it is an audit rather than a gate, since it needs a client and a pad and cannot run in CI. `steam_probe/` is its gitignored seed and moves out when the repository exists. Nothing in this crate is blocked on it: per-family suppression is chunk 112, and the presentation half can be built against API signatures already verified to exist |
| **Persisting calibration**, keyed to identity (R11.7, R14.11) | R11.5's stable device identity, which chunk 72 builds. Measured calibration lasts as long as the process |
| **A backend-safe way to ask which device drove an ordinary action's current activation** | a real need, not just Split Friction's. `Fired`'s value is device-agnostic by design — the same reason `Move` never says which stick moved it — so `protagonist.rs`'s `pair_on_join` (chunk 66, restated when `Join` moved off a class binding) reads Bevy's `Gamepad` component and `ButtonInput<KeyCode>` directly to find out who pressed it. That query does not exist under a Steam authority (D22, suppression), so the example breaks under the one backend this crate means to support. Whatever answers this generalizes past one example: some notion of "which device is this activation's origin" carried alongside an ordinary action's value, not only a class binding's raw event |
| **Glyphs from a backend** (R18.9) | the same asset questions from the other side. The *origin* half is closed — `ControlOrigin` already carries a control that is not one of ours, with the same stored name and fallback label everything else renders from — so what is deferred is the image rather than room for it. Measured (`docs/steam.md` S17): `get_glyph_for_action_origin` resolves to an absolute filesystem path inside the client's own app bundle — `Contents/MacOS/controller_base/images/api/dark/shared_lstick_md.png` on macOS, not the `tenfoot/resource/...` path this row previously guessed — and the `dark` component says the glyphs are themed, so a light variant has to be selected rather than assumed. A Bevy `AssetPath` can carry it natively via `from_path_buf` — no string-escaping the drive letter or backslashes. The path is not to be opened as given: a custom `AssetSource` reader must canonicalize it and reject anything outside a known root before reading, rather than trust an external SDK's return value as a bare filesystem path. One scheme, one hard-coded root is the right size while only this one root is confirmed; a second scheme is warranted only if a second root with its own lifecycle surfaces (e.g. something ephemeral, which cannot share a stable root's caching and hot-reload assumptions) — not one scheme per SDK call that happens to return a path |
| **A presentation crate** (`bevy_action_map_ui`) | **Bevy deciding to take this crate upstream**, which is when the workspace has to be arranged properly regardless. Until then the layer is `examples/common/` — `prompt_ui.rs` and `widget_focus.rs`, both written against the public API with nothing added to the crate for them. What is deferred is packaging, not work; the cost of waiting is a `#[path]` import |
| **Netcode injection and reconciliation** | a networked target. The injection point is built: chunk 111 landed `delegate` and `AuthorityValues`, so a peer's resolved action already has somewhere to go (D71). Rollback's local half — snapshot, restore, re-simulate — is chunk 83, which also takes the held-state containers. Injection targets L2 (D69): a network authority backend supplies the already-resolved `ActionValue`, not a raw frame, so no shared `Plan` across peers and no hold timers or tap counts on the wire. What is left here needs a remote player's resolved action to inject and a later correction to reconcile against it |
| **Consumption-aware `FocusedInput` dispatch** (R8.2a) | **a game wanting `bevy_ui_widgets`' own widgets working generically, unmodified, without a context per widget kind.** A context per kind is the path to reach for first, and Disasteroids ships that way. A design for the filter was built and set aside: a lowest-priority, non-consuming context binding `ControlClass::AnyButton`, feeding dispatch through the existing class-binding pipeline rather than a second raw-message read — keyboard only, since every keyboard-driven widget observer at the pinned commit gates on `ButtonState::Pressed` and none reacts to a release |
| **Promoting `WidgetKind` and the per-kind context into the crate** | [bevy#25592][], the author's own upstream proposal for a `bevy_ui_widgets`-native widget-kind id. Promoting a shape this crate invented first, ahead of that conversation, risks committing to the wrong one |
| **A context-level exclusion from the mapping list** | a second screen needing the same filter and duplicating it. `ActionMapping::context` already carries the data, and one call site filtering on it costs one line — at two, the crate is the one paying for the repetition |
| **An initial delay distinct from the repeat rate** (R22.5) | **a screen long enough to feel the difference.** `.on_change().pulse(0.25)` gives one number serving as both. Two numbers is a small change; what is missing is a case where equal is wrong, and a two-table settings screen is not it |
| **Free-form mutually-exclusive context sets** (R7.7 remainder) | nothing in tree needs two independently-exclusive contexts to coexist rather than one dominating the other by priority |
| **Owner-scoped `ConsumedControls`/exclusion ceiling** (R15.3 remainder, and D13's own remainder) | a real in-tree case with a per-player exclusive context, or a binding consumed across two players' devices. Design if built: a claim visible only if made globally or by the viewer's own paired device; an exclusive context's shadow implicit in its own pairing rather than a separate flag |
| **Per-entity presentation and prompts** (D52's remainder) | an actual per-player settings or prompt display — **chunk 71 is what this was waiting for**, so expect it met or falsified there rather than merely waiting |
| **An authority backend's actions in rollback** (D22's remainder) | a snapshot to fit them into. `AuthorityValues` is a plain component and clones with the entity, but what a rewind has to reproduce is what the authority *said* on the tick being re-simulated, which is not in the frame. The available answer is recording the backend's output into the frame at sample time, at the cost of a larger frame |
| **Sub-frame event timing** (D4's remainder) | [bevy#9087][] upstream. Gamepad stays frame-quantized regardless until gilrs polling is rewritten, so mixed fidelity across sources is permanent for now rather than an artifact |
| **Schedule enforcement for tick domains** (D9's remainder) | Bevy giving a `SystemParam` a way to know its own schedule. A plugin-time validation pass and a debug assertion stand in |
| **Mouse wheel as a binding source** (R13.3) | nothing in tree wants it. The wheel is a delta on its own channel, needs `Line`/`Pixel` normalization, and shares nothing with a button but the device |
| **R16.3's suspend/resume** (mobile, console) | a platform target that needs it. Nothing in this crate's supported platforms emits a suspend signal or has a device re-enumeration step to hook |
| **Split Friction's monsters, spawners and missiles** | a mechanic that would exercise input this crate has not already proven. Kept as a row rather than deleted because the sprites, the dungeon's region aspects and a `Fire`-shaped action all exist, so changing our mind is cheap |
| **Guardian migration** | porting it from Bevy 0.16.1 with `bevy_enhanced_input` 0.12 to 0.20-dev — four versions, and a port plus a rewrite. Doing both at once would confuse "action_map is wrong" with "0.20 moved this" |
| **A physical binding's label matching the current layout** (R12.2, R12.7) | winit exposing a physical-to-logical query and a layout-change signal, requested as [winit#4606][] and tracked by the broader [winit#2678][], open since February 2023 and unimplemented. A workaround was scoped and set aside: `run_captures` already sees the logical key at capture time, but keeping it means a new field on `ControlCaptured`, a session table `present.rs` consults ahead of the static fallback, and an honest answer on whether it survives a save — which drags in the still-deferred binding-definition serialization (R17.6, R22.16) for a fix that only covers controls a player has personally rebound. A landed query supersedes it outright, for every physical binding rather than only captured ones, so the workaround is not worth building ahead of it |

---

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
[bevy#25592]: https://github.com/bevyengine/bevy/issues/25592
[bevy#25710]: https://github.com/bevyengine/bevy/pull/25710
[winit#4606]: https://github.com/rust-windowing/winit/issues/4606
[winit#2678]: https://github.com/rust-windowing/winit/issues/2678
