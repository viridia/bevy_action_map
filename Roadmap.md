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

- **Glyphs.** Identifiers are defined; no image is resolved.
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

---

## Phase VII — wrong answers from an ordinary build

The live tier of [docs/issues.md](./docs/issues.md): no unusual configuration, no feature nobody has
used, and the answer is still wrong. Six more of its entries are behind this one and not yet
routed.

---

## Phase VIII — settling

Nothing here changes what the crate can do.

### 51. The constitution, trimmed

`Requirements.md` accreted argument because the house style used to point it there. This is the pass
that clears what accumulated under the old rule.

- **The target is 20 italic `_(...)_` asides**, not the long requirements. Measurement first,
  because the instinct is wrong: 220 requirements, 962 lines of body, median 3 lines, only 14 over
  twelve — and most of those are long because they carry a table of cases or an enumerated set of
  states, which *is* the requirement. Trimming by length would remove constitutional content.
- **The test, per requirement:** does this sentence say what must be true, or defend it? Defence
  moves to `docs/decisions.md`. It is moved rather than deleted.
- **Withdrawn requirements are exempt.**
- **Why it is a chunk rather than an afternoon.** Twenty judgement calls in the document every
  other document defers to, where dropping a load-bearing clause is invisible in a diff.
- **Review surface:** whether anything moved landed somewhere a reader would find it. Text moved out
  of the constitution and into a section nobody opens has been deleted with extra steps.

### 77. A test fixture the crate shares

8,672 of the crate's 20,124 lines are tests, and they repeat themselves: `struct Jump` is declared
nine times, `Move` six, `OnFoot` five; `capture.rs` and `overrides.rs` build near-identical fixture
contexts independently; `context.rs` alone holds 53 `App::new()`.

- **A `#[cfg(test)] mod test_support`** with the fixture actions, contexts, and press-and-step
  helpers. It shrinks `context.rs` further than 78 does.
- **Watch the action registry.** It is a process-global intern table keyed by declared path, so
  fixture actions shared across modules share one `ActionId` for the whole test binary. Already true
  of any two modules picking the same path; what changes is that it becomes deliberate.
- **Review surface:** whether a test still reads on its own. A fixture that has to be looked up in
  another module to understand a failure costs more than the duplication did.

### 78. Two files doing several jobs each

- **`context.rs` is three:** the live state; declaration and app wiring; and the monomorphization
  seam — the eight `read_*`/`apply_to_*` functions that are the only reason the file depends on
  `overrides`, `present`, `mapping` and `inspect`. After 75 that seam is half the size, which is why
  this follows rather than leads.
- **`binding.rs` is four**, and is the larger by code: the control vocabulary, the declaration
  structs and the queries over them, the modifiers, and the builder API.
- **The measurement chunk 52 corrected.** `context.rs` is 4,318 lines of which 1,559 are code;
  `binding.rs` is 2,361 lines of code. The "quarter of the crate" this chunk used to cite was
  counting a 2,759-line test module, which is 77's problem.
- **Ground rule 3 applies literally:** `examples/` must not change.

---

## Phase IX — the second example

Disasteroids is one player reading one set of bindings, so everything about device pairing is
invisible to it. Split Friction is the example that has to answer *which* device drove an action.
Two smaller examples sit here for the same reason rather than the same subject: each is the first
caller a shipped mechanism has ever had, and neither is Split Friction's.

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
- **What stays deferred:** injection and reconciliation — feeding a remote player's frame, and
  disagreeing with the authority about what happened. Those want a network; rewinding does not.
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

---

## Unscheduled by phase

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

### 42. The authority backend, faked

The backend seam made real against something that is not Steam, because the seam is only proven by a
second implementer and the real one cannot live here.

- **The traits land in `src/backend.rs`**, which has been a doc comment and no code since chunk 1.
  An authority backend supplies an `ActionValue` per owned action, substituted for the fold's output
  inside the evaluator so the state machine still synthesizes the edges. A source backend needs
  nothing new — `InputFrame::record` is already the door.
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
- **Review surface, and it is the point of the chunk.** Three decisions were written to be
  falsifiable here: a backend-owned action that accepts a `.hold()` without a plan-build diagnostic,
  two modal contexts that must be live on one pad at once, or an input observed twice. A decision
  this chunk cannot break is a decision that was not made.

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

### 94a. A binding can name the logical key, not only the physical one

R12.1: a binding must be able to target physical position (`KeyCode`) or logical character (`Key`),
and the choice must be explicit. Only `KeyCode` is bindable today, so `Ctrl+Z` can only be spelled
physically — `Ctrl+KeyCode::KeyZ`, the position that shows `W` on an AZERTY keyboard — which arms
on the wrong key for anyone using one.

- **The frame already carries what this needs.** `RawEvent::Keyboard` records the whole
  `KeyboardInput`, `logical_key` included; nothing downstream reads it. This is a new bindable
  control and its plumbing through capture, admissibility, and naming — not a frame change.
- **Narrows R12.2's gap rather than closing it.** A logical binding's on-screen label is exactly
  the key it was declared with — no per-layout guess needed — so this fixes the mislabeling for
  bindings a game chooses to make logical. A physical binding still shows the US-layout letter;
  that half of R12.2 stays the documented, unfixable-alone limitation in `present.rs` and
  `docs/decisions.md`.
- **Not doing: composition.** `Key::Character` under an active IME composes across several key
  events before it means anything (R12.6), which is the deferred text-input row and stays there.
  This chunk's `Key` binding is for single, already-resolved characters like `z`.
- **`docs/issues.md` 3.3** names this alongside 94b and 94c; only this one is the AZERTY fix people
  keep being told is coming.

### 94b. Either modifier

R12.3: a chord's modifier should be able to say "either Ctrl", as one binding rather than two.
`with` takes a single `ButtonControl`, so a game wanting either `LeftCtrl` or `RightCtrl` to arm a
chord writes both bindings by hand today. R4.10 already assigns this to the chord mechanism by name,
so the requirement has a destination in `Requirements.md` and, until this chunk, none in the plan.

- **Self-contained**, and independent of 94a and 94c — a different corner of the same requirements
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
- **Self-contained**, and independent of 94a and 94b.

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

### 101. Semantic control aliases for a console confirm swap

R4.4 (SHOULD) (`docs/issues.md` 1040): semantic aliases (`Submit`, `Cancel`, `MenuLeft`) resolving
per device family, load-bearing for R18.7's console confirm-button swap rather than merely
convenient.

- **Disasteroids' settings screen**, which already does directional navigation (29): the same screen
  reads `Cancel` rather than a hard-coded key, so its on-screen prompt says the right thing on
  keyboard and on a pad without the app hand-rolling the swap itself.
- **Not doing:** a general aliasing mechanism beyond the three names R4.4 asks for — this proves the
  concept the requirement names, not a configurable alias table.

### 102. A global timing scale

R20.4 (`docs/issues.md` 1045, split from 1021): every hold duration, tap window and repeat rate
globally scalable by one user preference, where today only per-mapping tunables exist.

- **Disasteroids' settings screen**, as an accessibility slider — "input timing ×1.5" — applied
  across every declared hold/tap/repeat value at once rather than one at a time.
- **Crate work, not only an example.** Scaling every timing by hand from the app side is exactly
  what R20.4 says a game should not have to do, so the scale factor's application point is a design
  question for this chunk rather than something the example alone can supply.

### 103. A disconnect signal and a reconnect prompt

R15.5 (MUST) (`docs/issues.md` 1020): on device loss the owning player must be identifiable
(already true), in-flight actions canceled (already true), **and a signal raised so the app can
pause and show a reconnect prompt** (nothing today).

- **Split Friction.** A pad disconnects mid-game; the app pauses and shows "player 2, reconnect to
  resume" until the pad (or another) reappears.
- **What the signal is** is this chunk's open question: Bevy's own `GamepadConnectionEvent` read
  directly, or something this crate re-raises so a game does not have to know the pairing's own
  bookkeeping to react correctly.

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

### 108. Focus loss, regated

R16.1 (`docs/issues.md` 1013): `RawEvent::FocusLost` (`frame.rs:93`) exists only under this crate's
own `keyboard` feature, but the signal behind it — `bevy_input`'s `KeyboardFocusLost` — is unified
on by `bevy_window`'s own `Cargo.toml` in any build with a real window, independent of what this
crate requests. A `mouse`-and-`gamepad` build (no `keyboard`) has the event and no code reading it,
so a mouse button held through alt-tab never clears — R16.1's "all held controls" MUST, unmet in
exactly the configuration that can reach it.

- **A regate, not new plumbing.** `RawEvent::FocusLost` and its `control()`/`device()` arms in
  `frame.rs`, the collecting system at `frame.rs:301`, and the three match arms in
  `eval.rs`/`capture.rs` move from `#[cfg(feature = "keyboard")]` to
  `#[cfg(any(feature = "keyboard", feature = "mouse"))]`. The two `.clear()` calls inside stay
  independently gated on their own feature, same as today.
- **Verification:** a headless `App` built `--no-default-features --features mouse,gamepad,std,...`
  that fires `KeyboardFocusLost` and confirms a held mouse button comes back unheld — the probe this
  finding never got. `scripts/verify.sh --full`'s eight-combination sweep only `cargo check`s each
  shape, so it would not have caught this; this chunk's test is what actually runs the `mouse`-
  without-`keyboard` case.
- **Not doing: anything about gamepad.** A gamepad's held state already clears on its own
  `Connection(Disconnected)` event (`eval.rs:422-426`); focus loss carries no gamepad information
  and needs none.

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

---

## Deliberately deferred

Every row states its gate. A row with no gate is an item that will be dropped, which is ground rule
5.

| Area | Gated on |
| --- | --- |
| **Persisting calibration**, keyed to identity (R11.7, R14.11) | R11.5's stable device identity, which chunk 72 builds. Measured calibration lasts as long as the process |
| **Glyph ids** (R18.4) | asset-pipeline questions, sharper than they looked when this row was written. Kenney's input prompt set covers keyboard, mouse, three pad brands and Steam, CC0 — but its generic set ships blank, unlabeled buttons, so a generic-tier icon is not the self-contained image the brand → generic → text chain assumed; it needs a short text stamp. Chunk 70 closed this for the three named brands — `fallback_label_for_brand` gives a short current-generation word ("A", "Cross", "B") for face buttons, bumpers, triggers, Select/Start and Mode — but deliberately left `GamepadBrand::Generic` falling through to `fallback_label`'s sentence-shaped strings ("East Button"), so the unlabeled-icon problem is exactly as open for the generic tier as it was before. The identifier scheme is the other open half — R18.4 wants a key of (brand, control), chunk 37's stored names are already the control half, and Kenney's real file names are still the way to falsify it. Presentation sketch: `PromptIcon`, standalone in `examples/common/` beside `PromptSpan` rather than a mode of it — `PromptSpan` is `TextSpan`-based for inline prose, and Bevy/parley has no inline-image-in-text-run support, so an icon-capable prompt is necessarily block-level — resolving through R18.9's glyph-source sum type rather than assuming our own identifier is the only shape a backend hands back. The stamp text is a defaults question, not a missing feature: the studio's answer is R19.14's catalogue, the long tail's is a heuristic (leading candidate: the compass name's first letter, "E" for East) that has to be *right* rather than merely present, per "Who this is for"'s standard for a default nobody tests |
| **Glyphs from a backend** (R18.9) | the same asset questions from the other side. The *origin* half is closed — `ControlOrigin` already carries a control that is not one of ours, with the same stored name and fallback label everything else renders from — so what is deferred is the image rather than room for it. Checked against `steamworks` 0.13: `get_glyph_for_action_origin` resolves to an absolute filesystem path under the Steam client's own install directory (`tenfoot/resource/images/library/controller/api/`), which a Bevy `AssetPath` can carry natively via `from_path_buf` — no string-escaping the drive letter or backslashes. The path is not to be opened as given: a custom `AssetSource` reader must canonicalize it and reject anything outside a known root before reading, rather than trust an external SDK's return value as a bare filesystem path. One scheme, one hard-coded root is the right size while only this one root is confirmed; a second scheme is warranted only if a second root with its own lifecycle surfaces (e.g. something ephemeral, which cannot share a stable root's caching and hot-reload assumptions) — not one scheme per SDK call that happens to return a path |
| **A presentation crate** (`bevy_action_map_ui`) | **Bevy deciding to take this crate upstream**, which is when the workspace has to be arranged properly regardless. Until then the layer is `examples/common/` — `prompt_ui.rs` and `widget_focus.rs`, both written against the public API with nothing added to the crate for them. What is deferred is packaging, not work; the cost of waiting is a `#[path]` import |
| **Netcode injection and reconciliation** | a networked target. Rollback's local half — snapshot, restore, re-simulate — is chunk 83, which also takes the held-state containers. What is left here needs a remote player to inject a frame for and an authority to disagree with |
| **Consumption-aware `FocusedInput` dispatch** (R8.2a) | **a game wanting `bevy_ui_widgets`' own widgets working generically, unmodified, without a context per widget kind.** A context per kind is the path to reach for first, and Disasteroids ships that way. A design for the filter was built and set aside: a lowest-priority, non-consuming context binding `ControlClass::AnyButton`, feeding dispatch through the existing class-binding pipeline rather than a second raw-message read — keyboard only, since every keyboard-driven widget observer at the pinned commit gates on `ButtonState::Pressed` and none reacts to a release |
| **Promoting `WidgetKind` and the per-kind context into the crate** | [bevy#25592][], the author's own upstream proposal for a `bevy_ui_widgets`-native widget-kind id. Promoting a shape this crate invented first, ahead of that conversation, risks committing to the wrong one |
| **A context-level exclusion from the mapping list** | a second screen needing the same filter and duplicating it. `ActionMapping::context` already carries the data, and one call site filtering on it costs one line — at two, the crate is the one paying for the repetition |
| **An initial delay distinct from the repeat rate** (R22.5) | **a screen long enough to feel the difference.** `.on_change().pulse(0.25)` gives one number serving as both. Two numbers is a small change; what is missing is a case where equal is wrong, and a two-table settings screen is not it |
| **Free-form mutually-exclusive context sets** (R7.7 remainder) | nothing in tree needs two independently-exclusive contexts to coexist rather than one dominating the other by priority |
| **Owner-scoped `ConsumedControls`/exclusion ceiling** (R15.3 remainder, and D13's own remainder) | a real in-tree case with a per-player exclusive context, or a binding consumed across two players' devices. Design if built: a claim visible only if made globally or by the viewer's own paired device; an exclusive context's shadow implicit in its own pairing rather than a separate flag |
| **Per-entity presentation and prompts** (D52's remainder) | an actual per-player settings or prompt display — **chunk 71 is what this was waiting for**, so expect it met or falsified there rather than merely waiting |
| **An authority backend's actions in rollback** (D22's remainder) | chunk 42 having a backend to ask. The available answer is recording the backend's output into the frame at sample time, at the cost of a larger frame |
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
[winit#4606]: https://github.com/rust-windowing/winit/issues/4606
[winit#2678]: https://github.com/rust-windowing/winit/issues/2678
