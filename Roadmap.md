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
- **The prelude exports sixteen bare English nouns** that a glob import drops into a template beside
  Bevy's own — chunk 48.
- **`Phase::Ongoing` means two things**, told apart by reading the action's value: still firing, and
  still building toward firing. Every consumer in tree either does that value test or wants both
  halves — chunk 79.
- **A refused capture is silent on Disasteroids' screen.** Wrong shape, wrong scheme, or reserved,
  and the session simply keeps listening with nothing said about why the press did not take.
- **`cancel_in_flight` may leave an action `Started` rather than `Canceled`.** It matches
  `Fired | Ongoing` and omits `Started`, so deactivating on the tick a hold began appears to
  contradict R7.4. Not traced to a reachable case; chunk 79 is where it gets an answer either way.
- **`R23.2` is unenforced.** No allocation and no synchronization on the per-tick path is a rule
  with no tooling behind it. Two violations have reached that path and both were caught by reading.

### Never built

- **Glyphs.** Identifiers are defined; no image is resolved.
- **Writing a saved override set to a file.** The crate serializes and deserializes one; where the
  bytes go was always the app's decision — chunk 92.
- **A snapshot of a context's state.** The shape is designed and written down; nothing has taken one
  — chunk 83.
- **A window on the input frame.** `RawEvent` carries no source window, so nothing can scope a
  binding to one — chunk 63.

### Upstreaming, if it happens

There is a possibility this crate is taken upstream into Bevy. It is **not committed**, and nothing
here is built on the assumption that it will be. The rule is that the possibility may influence the
*shape* and the *order* of what gets built, but no work happens that a third-party crate would not
want anyway. What that changes today:

- **The extensibility mechanism and the prelude names are public API**, cheap now and breaking
  later, upstream or not — chunk 48.
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

---

## Phase VII — wrong answers from an ordinary build

The live tier of [docs/issues.md](./docs/issues.md): no unusual configuration, no feature nobody has
used, and the answer is still wrong. Six more of its entries are behind this one and not yet
routed.

### 88. The gamepad-settings warning misses the global thresholds

R14.9 exists so a game that configures Bevy's `GamepadSettings` is told they reach no binding,
rather than left wondering. `is_customized` tests four of that struct's six fields and misses
`default_button_settings` and `default_button_axis_settings` — the two *global* ones. Setting a
press threshold for every button at once is the ordinary way to do it and produces silence; setting
one per button is the unusual way and is caught.

- **One clause and one comparison, not two clauses.** `ButtonSettings` derives `PartialEq` so the
  first is `!= ButtonSettings::default()`. `ButtonAxisSettings` does not, so the second compares its
  three public `f32`s against `ButtonAxisSettings::default()`.
- **The comment above the predicate is what produced the gap** and is false at the pinned commit: it
  says `AxisSettings` is the only one of the three with `PartialEq`, but `ButtonSettings` derives it
  (`gamepad.rs:821`) and `AxisSettings` does (`985`) — only `ButtonAxisSettings` does not
  (`1413`). Correcting it is the half that stops this being re-derived.
- **The test targets `is_customized`, not the warning.** `bevy_utils::once!` fires once per process,
  so only the first test in a binary to trip the system would ever observe it.
- **Not doing:** comparing map *entries* against defaults. A populated map means a game put
  something there, which is the question being asked.

### 89. `why_not` can see the pairing

R22.1 names five causes an action might not fire, and `Obstacle` answers three. `why_not_id` takes
`&ConsumedControls` and no `Paired`, so it cannot see device pairing at all. A context whose
pairing is dropping every event the player generates answers `Obstacle::NoInput` — "nothing was
pressed" — when something was pressed and was filtered, which is precisely the confusion R22.1
exists to end, arriving as the answer.

- **It bites where it is hardest to debug.** Pairing is used only in local multiplayer, and worst
  during a join flow, when the pairing is the thing under suspicion.
- **`Obstacle` is `#[non_exhaustive]`**, so the new cause is an addition rather than a break.
- **Not doing: the fifth cause.** "Condition Z at 40% progress" needs a progress number that
  `ActionState` does not carry — [docs/issues.md](./docs/issues.md) 3.1, which is R3.4 and R3.5,
  and has no destination at all. This chunk fixes the answer that is *wrong*; that one adds the
  answer that is *missing*.
- **Reasoned from the signature, not probed** — the one finding in the live tier that was not run.
  Confirming it is the first thing this chunk does, and it may end up smaller than it looks.

### 90. A context nobody declared says so

Spawn a `#[derive(InputContext)]` component for a type `add_context` was never called for and
nothing happens: no `InputContextState` is attached, no diagnostic is logged, and `dump` cannot see
it either, since `DeclaredContexts` is its only source. The tool built to answer "why is nothing
happening" is blind to the likeliest cause of it.

- **The mirror case is already handled**, which is what makes this an asymmetry rather than an
  omission: `ContextDump::instances` documents "declared and nobody has it, which is usually a
  mistake" and shows it.
- **The cheap half is a warning from the component's own `on_add` hook** when no plan resource
  exists for its type.
- **It owes a sentence to `docs/design.md` §7.1.** R22.14's MUST — spawning must be sufficient,
  including from a scene — holds only for a type that was *also* declared imperatively, and no
  document says so today.

### 91. The crate does not do what its own documents say

Two one-line defects that share nothing but a cause: both are invisible to reading, and to
`cargo check`.

- **`ActionMapPlugin` panics on its first update in the no-devices build.** `InputFramePlugin` is
  gated on `any(keyboard, mouse, gamepad)` and is the only caller of `init_resource::<InputFrame>`,
  while `run_captures` and `evaluate_context` take `Res<InputFrame>` unconditionally. All three
  single-feature builds pass; only the zero-feature one fails, and it is on `CLAUDE.md`'s
  Verification list — which runs `check` and `clippy` for it, neither of which can see this.
- **The smoke test is the durable half.** An `App::update` in that configuration, because the whole
  class of defect is invisible to a type check and will recur otherwise.
- **`KeyCode` is not in the prelude**, so `docs/design.md` §7.1's opening line —
  `controls.bind::<Jump>(KeyCode::Space)` — does not compile after a glob import. `lib.rs`'s quick
  start works only because it also globs `bevy::prelude`. It is the first thing anyone types.
- **Re-exporting is the fix, and it is safe against a double glob**: it is the same item as
  `bevy::prelude`'s, so both globs resolve to one thing. What the chunk decides is how far the
  re-export goes — `KeyCode` alone, or the control vocabulary a binding names. Chunk 48 is
  rethinking the prelude and inherits whatever this settles.
- **Chunk 28 owns the general class** — the design's examples actually running — and this does
  not wait for it.

---

## Phase VI — the parts a solo developer trips over

The long tail cannot verify what it does not own, so mistakes have to be caught rather than
discovered in QA that nobody is running.

### 17c. Reflect, and the two normalizes

`Reflect` on modifiers and conditions (R5.6, R17.5), plus R5.9's two `normalize` operations, now
named: `clamp_magnitude` scales a vector down if it exceeds magnitude 1, and `rescale` maps a range
onto 0..1 and therefore falls under the one-rescaling-stage rule. `rescale` is the word the crate
already uses for that stage, so this builds the modifier under a name the plan-build check counts
by.

- **Smaller than when it was written.** R17.5 wanted `Reflect` so third-party modifiers round-trip
  through persistence, but an override stores *controls*, so a custom modifier never reaches a saved
  file. What still wants it is serializing whole binding *definitions* (R17.6, R22.16), which is
  deferred — so this is no longer on the path to anything scheduled.
- **Not doing:** anything that would make `Modifier` or `Condition` require `Reflect`.

### 63. Multi-window

R13.5: an input frame carries no source window, so nothing can scope a binding to one — a MUST with
zero code behind it.

- **Why it waits:** nothing in tree has a second window. Of everything the grooming sweeps found,
  this is the one with no in-tree pressure behind it at all.

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

### 76. `Unresolved`, once

`UnresolvedMapping` and `UnresolvedTunable` are the same struct — `{ scheme, name }` — each
documented as being for the same reason as the other. One type with a field saying which kind of row
it was, and `resolve_saved`'s `ResolvedOverrides` four-tuple becomes a struct with three fields.

- **Not doing:** anything to `OverrideProblem`. That is a different report at a different time.

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

### 79. `Phase` tells building from firing

Split `Phase::Ongoing` into `Firing` and `Building`.

- **Nobody wants the union.** Every consumer either re-reads the value immediately or deliberately
  wants both halves. `update_action_state`'s two guards stop inspecting the value at all.
- **It completes the rule R3.1 already states.** A gerund or adjective is a level (`Idle`,
  `Building`, `Firing`); a past participle is an edge (`Started`, `Fired`, `Completed`, `Canceled`).
  `Ongoing` is the one variant carrying both.
- **`Phase` is not `#[non_exhaustive]`**, so this breaks any exhaustive match — the same shelf life
  as 48, which is why the two are last in this phase.
- **It owns the `cancel_in_flight` defect** in "Known wrong" above.
- **Not doing:** `Active` for the firing half. It collides with context activation, and "an `Active`
  action in an inactive context" is a sentence this crate can produce.

### 48. Names that survive a glob import

The prelude exports sixteen bare English nouns — `Scheme`, `Mapping`, `Capacity`, `Conflict`,
`Overlap`, `Captured`, `Refused`, `Condition`, `Verdict`, `Part`, `Intent`, `Phase`, `Obstacle`,
`Timestamp`, `Rebinding`, `Actions` — plus the four transition events, and a game glob-imports it
beside Bevy's own.

- **The criterion, so the pass is not taste.** A name earns its bareness if a reader who knows Bevy
  but not this crate would guess right. `Control` and `Prompt` pass. `Phase` and `Part` do not.
  `CompassPoints` passes, being named after `bevy_math`'s own `CompassOctant`.
- **`Verdict` needs it most**, and becomes `ConditionState`, joining the family already there. Its
  variants go with it: `Idle`/`Ongoing`/`Fired` becomes `Idle`/`Building`/`Satisfied`, which is
  level-shaped throughout — a `Down` condition answers on every tick the control is held, so a past
  participle read as an edge. Depends on 79, which claims `Firing` for `Phase`. The objection,
  recorded rather than resolved: `ActionState` and `InputContextState` are storage, and a
  condition's storage is `Scratch`.
- **Why it is not cosmetic.** BSN templates are where it bites: a scene lists components from
  several preludes with nothing saying which crate each came from.
- **A prelude that grows before this lands needs checking as it grows**, rather than re-enumerating
  at the end.
- **Not doing:** deprecation shims. Nothing outside this repository depends on the crate yet, and
  the moment that stops being true this chunk gets more expensive than it is worth — a shelf life
  rather than a chunk that can wait indefinitely.
- **Review surface:** whether the renames read as prefixes bolted on. `PromptScope` reads as one
  thing; `InputActionPhase` would not, and where that happens the answer is a better word rather
  than a longer one.

---

## Phase IX — the second example

Disasteroids is one player reading one set of bindings, so everything about device pairing is
invisible to it. Split Friction is the example that has to answer *which* device drove an action.
Two smaller examples sit here for the same reason rather than the same subject: each is the first
caller a shipped mechanism has ever had, and neither is Split Friction's.

### 70. Device brand and class

R11.6: brand and class resolution (Xbox / PlayStation / Nintendo / generic), app-overridable, seeded
from a database such as SDL_GameControllerDB.

- **Unknown is an ordinary answer.** `vendor_id`/`product_id` are `Option` and often absent — wasm,
  some Linux setups — so the generic fallback is a common path, not an error.
- **It has to decide what "generic" contains**, which has never been designed. Xbox and
  PlayStation share close enough conventions for one tier to serve both; Nintendo is not a peer,
  since its face buttons sit mirrored, so a generic tier built on the other two names the wrong
  button rather than a merely-generic one for an unidentified Switch-family pad. Whether
  `fallback_label` gains the same brand → generic → text tiering, or stays positional and leaves
  brand-specific text to an app's catalogue, is open and this chunk closes it.
- **Not shipping SDL's database.** A small table of known ids plus the app-override hook proves the
  seam; the full database is data an app supplies.
- **It is the missing half of the glyph row**, not a separate errand. R18.4 keys a glyph
  identifier on (brand, control) and chunk 37 already stores the control half.
- **Verified by:** Split Friction's device label naming the pad actually being held rather than
  "Gamepad".

### 71. Per-player presets

Each protagonist selects its own preset, applied through `apply_overrides_for_with_preset`.

- **What it proves.** Chunk 67 built the per-entity apply path ahead of a need and nothing in tree
  has called it since — this is that caller. A preset is the cheapest override to select, so the
  general per-entity case is validated without a second rebinding UI.
- **It trips a deferred row on purpose.** "Per-entity presentation and prompts" is gated on a
  per-player settings display existing, and a per-pane preset selector is one. Expect it to validate
  that row's sketch or falsify it, and say which.
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
- **What it is:** one example's renderer reading a catalogue file, a second locale to prove it
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

### 92. Persisting bindings through `bevy_settings`

Chunk 23 wired `Overrides` into Disasteroids' settings screen; nothing has ever written one to disk.
"Writing a saved override set to a file" is still listed as never built, the destination left to the
app on purpose (D53). `bevy_settings` is that destination: register `SavedOverrides` (D59) as a
`SettingsGroup` resource and let it merge into the game's one settings file alongside whatever else
the app declares there.

- **The mechanism is validated, not open.** A scratch test against `bevy_settings::resources_to_toml`
  / `apply_settings_to_world` confirmed the whole path round-trips correctly: `SavedOverrides`'s own
  fields are walked structurally (no stutter, no bevy_settings change needed), and its nested
  `SavedRow`/`SavedTunableValue` fields bridge through `#[reflect(Serialize, Deserialize)]` to their
  own hand-written encoding rather than bevy_reflect's generic enum shape. This crate's own
  `overrides::tests::persistence::a_saved_override_set_round_trips_through_reflect` pins the same
  contract without a `bevy_settings` dependency. This chunk is the remaining wiring, not a risk to
  chase.
- **Load at startup, save on Confirm.** `apply_and_close` in `settings.rs` already applies a pending
  `Overrides` to the running game; this chunk adds a startup system calling `resolve_saved` against
  the loaded `SavedOverrides` resource before the first `apply_overrides`, and a `save_overrides` call
  on Confirm feeding back into it.
- **A dev-dependency of the examples, not the crate.** This crate publishes `SavedOverrides` and no
  opinion about where the bytes go (D53); `bevy_settings` is wired into `examples/disasteroids` only.
- **Not doing:** per-profile or per-scheme settings groups (R17.4), and anything Split Friction's two
  protagonists need — chunk 71 owns per-player preset selection once a settings group exists to read
  and write, this chunk owns getting one player's set to and from disk at all.
- **Retires the "Writing a saved override set to a file" row** in "Never built".
- **Verified by:** rebinding a control, quitting Disasteroids, relaunching it, and finding the
  binding still applied.

---

## Unscheduled by phase

### 33. Conditions that read other actions

A chord may require another *control* but not another *action*, and `BlockedBy` does not exist. Both
read a neighbouring slot rather than their own value, which needs the operand evaluated first: slots
ordered topologically, and a cycle rejected at plan build with a diagnostic naming the loop.

- **Why it waits:** self-contained, and nothing in tree wants it. It carried two motivating cases
  and the settings screen claimed the first — a modal that blocks an action while it is open turned
  out to be exclusive contexts, because the block is per-context rather than per-action. What is
  left is a chord on an action rather than a key.
- **Inherited from chunk 44: whether this subsumes `follow`.** An afterburner is genuinely "thrust,
  still held", and a game that could say that in a condition would need no link at all. The two
  answer different questions — `follow` says the mapping is shared, a condition says the value is
  derived — but check when this lands whether the overlap is large enough that one should go, since
  carrying both when either would do is what an outside reader notices first.

### 34. Sequences

R6.4's ordered sequences — double-tap-dash, motion inputs, cheat codes — arriving in order within a
time window.

- **Fits the scratch record**, so this is a condition, not a redesign.
- **R6.5's forgiveness windows do not carry here** and are withdrawn. The crossing point in both
  directions is app-domain state the crate cannot see, and events plus elapsed time already give an
  app what it needs to compose the pattern itself.

### 35. Disabling an action

R3.7: an action switched off without being unbound, and switched back on without firing for a
control the player was already holding.

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
- **The acceptance criterion is a non-diff**, and it is no longer Disasteroids' pad: that is where
  presets get taught, and a pad the backend owns has no presets of ours to show. The proof still has
  to be screen code running unchanged against a backend-owned context, but it needs a vehicle that
  is not already spoken for. Choosing one is part of this chunk.
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
| **Persisting calibration**, keyed to identity (R11.7, R14.11) | R11.5's stable device identity, which chunk 72 builds. Measured calibration lasts as long as the process |
| **Glyph ids** (R18.4) | asset-pipeline questions, though *the art is not one of them*: Kenney's input prompt set covers keyboard, mouse and three pad brands and is CC0. What stays open is the identifier scheme, and Kenney is the way to falsify it — R18.4 wants a key of (brand, control), and chunk 37's stored names are already the control half. If that does not survive contact with a real atlas's file names, R18.4 is wrong rather than merely unbuilt |
| **Glyphs from a backend** (R18.9) | the same asset questions from the other side. The *origin* half is closed — `ControlOrigin` already carries a control that is not one of ours, with the same stored name and fallback label everything else renders from — so what is deferred is the image rather than room for it |
| **A presentation crate** (`bevy_action_map_ui`) | **Bevy deciding to take this crate upstream**, which is when the workspace has to be arranged properly regardless. Until then the layer is `examples/common/` — `prompt_ui.rs` and `widget_focus.rs`, both written against the public API with nothing added to the crate for them. What is deferred is packaging, not work; the cost of waiting is a `#[path]` import |
| **Netcode injection and reconciliation** | a networked target. Rollback's local half — snapshot, restore, re-simulate — is chunk 83, which also takes the held-state containers. What is left here needs a remote player to inject a frame for and an authority to disagree with |
| **Consumption-aware `FocusedInput` dispatch** (R8.2a) | **a game wanting `bevy_ui_widgets`' own widgets working generically, unmodified, without a context per widget kind.** A context per kind is the path to reach for first, and Disasteroids ships that way. A design for the filter was built and set aside: a lowest-priority, non-consuming context binding `ControlClass::AnyButton`, feeding dispatch through the existing class-binding pipeline rather than a second raw-message read — keyboard only, since every keyboard-driven widget observer at the pinned commit gates on `ButtonState::Pressed` and none reacts to a release |
| **Promoting `WidgetKind` and the per-kind context into the crate** | [bevy#25592][], the author's own upstream proposal for a `bevy_ui_widgets`-native widget-kind id. Promoting a shape this crate invented first, ahead of that conversation, risks committing to the wrong one |
| **A context-level exclusion from the mapping list** | a second screen needing the same filter and duplicating it. `Mapping::context` already carries the data, and one call site filtering on it costs one line — at two, the crate is the one paying for the repetition |
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

---

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
[bevy#25592]: https://github.com/bevyengine/bevy/issues/25592
