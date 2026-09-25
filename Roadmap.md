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

[docs/design.md](./docs/design.md) is what the crate does today, and
[docs/issues.md](./docs/issues.md) is what is known to be wrong and not yet routed. What follows is
the rest of the delta: what was never built, and what is left to do.

The target the remaining sequence aims at is **Disasteroids** — an asteroids-like game playable on
keyboard or gamepad, with a rebinding screen built on `bevy_ui_widgets` and operable from the
controller. It is not a phase of its own; it arrives early, badly, and grows a capability per chunk,
because ground rule 3 wants something runnable at every step and a real game is a better acceptance
test than a synthetic one.

### Never built

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
| 113 | A device's brand is a component, not a lookup at every read |
| 71  | Per-player presets, behind a pause popup              |
| 103 | A disconnect signal and a reconnect prompt            |
| 72  | Device identity, and a pad that returns to the pane that lost it |
| 72d | A pairing that survives a restart, through `bevy_settings`       |
| 92  | Bindings that survive a restart, through `bevy_settings`         |
| 92b | A pane's chosen preset survives a restart                        |
| 116 | A backend-neutral gamepad pool, and a join listener per device   |
| 117a | The comments that were wrong, and the focus placeholder          |
| 117c | The evaluator's duplication clusters, merged                      |
| 117d | Shadow, require-reset, and the test docs that narrated bugs       |
| 117e | The serialization docs, and the last public-item citations       |
| 117f | The device and frame group, and `Brand`/`Identity` told apart     |
| 117g | The binding group's listing, capacity, rescale and toggle clusters |
| 117h | The presentation group's naming, follower and prompt clusters     |
| 117i | Disasteroids' working-copy refrain, and the capture examples      |
| 117j | The other examples and `tests/`, and the last chunk references    |
| 117k | The public docs that contradicted the code                        |
| 117l | `devfmt` holds YAML frontmatter verbatim                          |
| 117m | A rewrap stops changing what the text is                          |
| 117o | A preview that shows the change, not the filename                 |
| 117n | Repacking is what `devfmt` does, and a bare run is refused        |
| 117q | The preview stops showing unchanged code                          |
| 119  | Bevy 0.20.0-rc.1, from crates.io rather than git                  |
| 118a | Capacity is the app's business, and a resource-set ceiling replaces it |
| 118b | A slot may be empty                                                    |
| 118c | A slot is addressed, not appended                                     |
| 120  | A cell the player can empty, and a refusal that says so               |
| 124  | Disasteroids reserves the way back to its own settings screen         |
| 125  | A stage after the fold, declared once per action                      |
| 126  | Opposite contributions cancel                                         |
| 127  | A composite expands into one binding per part                         |
| 35   | Disabling an action                                                   |
| 94b  | Either modifier                                                       |
| 128  | A rebinding row that shows its chord                                  |
| 129  | An override that can name a chord                                     |
| 136  | A gallery of prompts                                                  |
| 133  | An icon prompt for a chord                                            |
| 134  | A prompt names what an action is bound to                             |
| 132  | Consumption that reaches a gamepad                                    |
| 110  | A glyph for a control, and an icon prompt                             |
| 123  | Advice on a mapping set a player cannot break                         |
| 109  | Reflect where a scene, a tool or a save file reaches the type         |
| 137  | Arbitration that names whose input it is                              |
| 130  | Capture as a sensor, answering on the release                         |
| 145  | A remote driver: requirements and design                              |
| 146  | The driver's plugin                                                   |
| 147  | The driver's client and runner                                        |
| 148  | Ids in the examples                                                   |
| 150  | A virtual gamepad                                                     |
| 131  | A plan slot as one struct                                             |
| 138  | A plan compiled once, not once per context                            |
| 139  | The action registry, and what an `ActionId` can reach                 |
| 140  | A mapping key derived in one place                                    |
| 141  | A binding input has one part                                          |
| 151a | An authority is a binding for one device family                       |
| 152  | A follower rides its leader's authority                               |
| 153  | A preset skips a family an authority owns                             |
| 154  | An authority held at spawn is held over                               |
| 151b | Disasteroids on Steam                                                 |
| 155  | An authority that stops supplying a held action cancels it            |
| 151d | A delegated row                                                       |
| 157  | The controls screen as a template with a slot                         |
| 151f | The delegated row on the Steam controls screen                        |

---

What is left, in semantic groups ordered roughly by priority. The order is a guide rather than a
schedule: any chunk may be reordered once the one before it has been read, and a chunk's number is
its identity rather than its position.

## Next

* 115: A timing declared as a tunable
* 122: The wheel as a binding source
* 143: One apply, for the world or for an entity
* 144: One rule for what counts as a character
* 28: Docs that run
* 33: Conditions that read other actions

No chunk currently carries a defect. The register of what is known to be wrong is
[docs/issues.md](./docs/issues.md), and an entry there that acquires a chunk gets a section here.

## Devices and players

Disasteroids is one player reading one set of bindings, so everything about device pairing is
invisible to it. Split Friction is the example that has to answer *which* device drove an
action, and most of this section is what it needs.

### 72b. Calibration that survives a restart, and a screen that measures one

R11.7 and R14.11: stage-1 calibration stored per persistent device identity, and offered as an
explicit player-facing step. `DeviceId` is the identity to key it by, and it exists.

- **Chunk 22 built the measuring and the applying keyed to the runtime handle**, which is exactly
  what a persistent identity keys instead. `GamepadCalibration` and `CalibrationSampling` are keyed
  by `Entity` today; this chunk re-keys the stored half to `DeviceId` while the live half stays on
  the handle, or says why the two want different storage.
- **The calibration step has no in-tree caller.** `CalibrationSampling` is driven end to end by
  tests and by no screen. A calibration a player performs and then loses on quit is worth little, so
  the screen and the persistence are one feature.
- **Sampling learns the deflection as rest.** `CalibrationSampling`'s doc tells the player to move
  the sticks and let go, and `observe` takes every reading while the resource exists, so a step that
  follows the instruction sees values near ±1.0 and finishes with a rest envelope wider than the
  stick's range: the axis goes dead. The tests feed only rest values, and no screen has run it. The
  options: two game-driven phases, sampling only after the player lets go, which still catches the
  spring-back and needs a settle wait; restarting an axis's min and max whenever a reading jumps
  well outside any drift, around 0.5, which keeps the instruction and the API as they are; or
  recording each settled position and taking the envelope across those. The second is the lean.
- **Verified by:** calibrating a drifting stick, quitting, relaunching, and finding the stick still
  corrected. The in-house fixture is a DualSense whose left stick is noisy at rest and settles at a
  different point after each release, varying the drift's speed and direction; one release is not a
  calibration of it, so the step has to widen across several.

---

## Bindings and conditions

What a binding can name, and when it counts as firing. Each of these is a gap a game runs into
rather than a defect in what exists.

### 129b. A screen that sets a modifier

129's caller, and the obligation 129 lands short of. Nothing in tree can edit a chord until
something draws the toggles; what they edit is `BoundSlot::with`, written back through
`Overrides::bind` (TD9.1).

- **Reset or preserve is this screen's call now.** 129 left it to whoever writes the slot, and
  Disasteroids' capture preserves, because on a screen with no toggles a reset loses a chord the
  player can never get back. With the toggles beside the key, Blender's reset becomes affordable;
  this chunk picks one and says why.

- **Blender's arrangement is the one to copy**, and the screenshot is the argument: the row list
  stays a list of composed labels, and only the expanded row grows a `Shift`/`Ctrl`/`Alt`/`Cmd`
  strip. That is what makes it fit — the widgets are per *edit*, not per row, so a one-page table
  stays one page.
- **Where it lands is this chunk's decision.** Disasteroids' capture is in-cell — the cell flips its
  background and listens — with no detail panel to grow, so this is new UI wherever it goes. A
  second example risks being a feature vehicle rather than a game, which is the friction worth
  weighing against complicating the one settings screen in tree.
- **A row the player can change only in part stops being a thing.** 128 shipped `Ctrl+N` as a fixed
  row precisely because no screen could edit it; with an editor, a chorded row can be `mappable` and
  mean it.

### 135. A chord across two device families

`docs/issues.md` 1060. `binding_family` files a row under its primary control's family without
consulting the chord, and conflicts are per family, so a key in a gamepad binding's chord is
invisible to keyboard conflict detection. Nothing in tree declares one, but since 129 a save can
write one: `refusal` asks nothing of a chord entry's family, so `pad/LeftTrigger+key/KeyS` on a
keyboard row applies.

- **Refuse or detect is the decision.** 1060 sketched a refusal: a plan-build diagnostic on a chord
  entry whose family differs from its primary's, with a twin in `refusal` beside 129's
  `NotChordable`. The case against is the player it would stop — a foot pedal that enumerates as a
  gamepad, a one-handed layout split across a pad and a keyboard — for whom a cross-family chord is
  the point. The alternative leaves the row filed where it is and makes conflict detection consult
  each chord entry's family, so the overlap is reported rather than the input forbidden.
- **The save path answers as the declaration does.** Whatever a declaration may not write, a file
  may not either, and the reverse.
- **Verified by** tests on both paths — a declaration mixing families, and a saved row naming a
  gamepad entry on a keyboard row.

### 121. A camera that takes the mouse, and gives it back

R13.4, and the picking half of R22.4: nothing in tree has ever grabbed the cursor, so the one delta
that must not be invented has never had a camera to snap, and the lever an app pulls to keep picking
off a captured mouse has never been pulled.

- **A 3D orbit demo, and the first 3D example in tree.** A lit object and a camera orbiting a fixed
  focus point from mouse deltas. Bi-modal, and that is the point: cursor free, where picking is
  alive, the UI is clickable and the camera holds still; cursor captured, where the mouse is this
  crate's and picking receives none of it. The toggle is the demonstration — D75 drew a boundary
  between two pipelines, and this is what standing on either side of it looks like.
- **The ordering, written down** (R22.4). D75 settled which pipeline owns which signal; no
  user-facing text says so, and an example demonstrates a boundary rather than documenting it. So
  this chunk also states the ordering and which suppression lever applies when — cursor grab, a
  barrier entity covering the screen, deactivating the context.
- **Frame-rate independence becomes visible** (R13.2). `move_and_jump` already binds `MouseMove`
  beside a `per_second` stick and explains in a comment why one is multiplied by a frame time and
  the other is not. With a camera on the end of it, multiplying the delta is a bug you can see
  rather than a comment you can read.
- **Not doing: the upstream pan-orbit controller.** `bevy_camera_controller`'s `PanOrbitCamera`
  ([bevy#19741][]) ships in 0.20.0-rc.1 and is declined on design grounds rather than availability:
  it is pointer-driven by construction, taking `PointerLocation` and `PointerInteraction`, and its
  pixel-perfect pan is defined as the world point under the cursor staying under it. That is
  picking's pipeline, which D75 assigned away from this crate, so driving it from here would be
  building the thing the decision refused. Its own example calls its input plugin a placeholder for
  "a first-party input-manager-integrated solution", which is worth watching; an orbit from deltas
  around a fixed focus is a dozen lines and is the half a mapper can own.
- **Not doing: zoom on the wheel** (R13.3) — chunk 122, which lands into this example.
- **Review surface:** alt-tab while captured. Focus loss has to drop the grab and must not deliver
  whatever the cursor accumulated while away, which is R13.4's rule arriving through R16 rather than
  through a mode change the player asked for.

### 122. The wheel as a binding source, and a zoom for the orbit

R13.3: `Control` names mouse buttons and mouse motion and nothing else, so a game wanting the wheel
reads Bevy's own message beside the mapper, and loses the rebinding along with it.

- **Depends on chunk 121**, which supplies the customer. Zoom on an orbit camera is what the wheel
  is for, and the deferred row this replaces was gated on precisely that.
- **A channel, not a button.** `MouseScrollUnit::{Line, Pixel}` normalized through a configurable
  factor, high-resolution trackpad scroll not quantized on the way through, a `ChannelShape` of its
  own, and the capture, prompt and serialization surface that follows from having one.
- **Review surface:** whether the normalization factor belongs to the app or to the binding. R13.3
  says app-configurable, but a trackpad and a notched wheel want different numbers on the same
  machine and only the binding is that specific — so either the requirement is right and the case is
  rarer than it sounds, or it is a clause to revise.

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
- **`pong_countdown` already has a serve.** Chunk 35 gates it by disabling `Serve` outside the wait,
  from a system. The likely demo is that gate replaced by a condition, on that variant, rather than
  a second `Serve` in another.

### 115. A timing declared as a tunable

R20.7 (`docs/issues.md` 1045): a hold duration, tap window, multi-tap gap or pulse interval offered
to the player as a named tunable, the way `tunable_dead_zone` already offers a dead zone.

- **Replaces chunk 102**, which would have built R20.4's global scale. That requirement is withdrawn
  for applying one factor to two floors and three ceilings, which cannot move both toward
  forgiveness at once.
- **The evaluator does not change, and confirming that is this chunk's own review surface rather
  than an assumption to carry in.** A `Range` tunable is applied by rewriting the declared value and
  recompiling the plan (`builder.rs`'s `TunableDecl`, "by rewriting `modifiers[modifier_index]` and
  recompiling"), so `BindingCondition::Hold` keeps reading a constant — one that arrived from a
  saved override rather than from the source. Nothing new is read per tick, and no scratch cell is
  needed: a `Bool` tunable takes one because `hold_or_toggle` latches, and a `Range` tunable takes
  none.
- **What does change is what a `TunableDecl` can point at.** It carries a `modifier_index`, and a
  timing lives in a condition rather than a modifier, so it has to say which of the two it
  addresses. That is the whole delta, and it is where this chunk's cost actually sits.
- **Not doing: a game-wide "more forgiving" control.** That needs the crate to hold a per-threshold
  direction of forgiveness — the half of R20.4 that was coherent — and it has its own deferred row
  with a gate rather than riding along here.
- **Verified by:** Disasteroids' settings screen offering one timing beside the dead-zone slider it
  already has, and the changed value still applied after a quit and relaunch.

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

---

## Snapshots

Taking a context's state out and putting it back, in memory between ticks. The other half of this
section was persistence — the same move, to a file between runs — and chunks 92 and 92b landed it.

### 83. Rewind, without the network

`InputContextState`'s own comment says a rollback snapshot is the two tables plus the dirty bits,
and TD6 says the same. Nothing has ever taken one. A ring buffer of snapshots and the `InputFrame`s
that followed each, with a key that rewinds N ticks and re-simulates forward, is rollback's three
requirements — snapshot, restore, deterministic re-simulation — with the network removed, which was
the expensive part.

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
  Those want a network; rewinding does not, and the injection point itself is already chunk 111's.
- **`disabled` is state a restore must bring back**, beside `require_reset`: chunk 35's per-action
  switch, parallel to the action table.
- **Split if it grows.** Making the state snapshot-able with a differential test is separable from
  the example that rewinds, and ground rule 1 says that split happens before the code, not during.
- **Depends on chunk 95.** The visible rewind reuses its Pong base rather than a third vehicle — the
  same paddle-and-ball simulation snapshotted and re-simulated forward, with the recorded and
  re-simulated transition logs compared. `pong_robot` has already shown the base can host one
  grafted concept without disturbing its own.
- **Check whether `docs/issues.md` 1041 belongs here.** R9.9's pumped sampling mode (stopping
  `InputFramePlugin` from scheduling its own sampling) was floated as a fit for a rewind demo, on
  the theory that re-simulating forward wants control over exactly when a frame is sampled. Confirm
  that before routing it here — if re-simulation does not actually need to suppress live sampling,
  1041 stays unrouted rather than getting a home it does not need.

---

## The library itself

Work no game asks for and no published crate can do without: the crate's internals kept consistent,
extension points exercised, and documentation that is true and runs.

### 143. One apply, for the world or for an entity

`overrides.rs`'s `apply_with` and `apply_for_entity_with` are the same function: they differ only in
which applier they collect from `DeclaredContexts`, and then both run the same `NoSuchMapping`
report and the same prompt bump. The per-entity copy's own comment says so. The report is tested
only through `apply_overrides`, so a change made to one copy and not the other fails nothing.

- **One `apply_with(world, target: Option<Entity>, overrides, preset)`**, matching on `target` to
  call `apply` or `apply_for_entity`. Everything after the appliers exists once. The four public
  entry points keep their signatures and pass `None` or `Some(entity)`.
- **A test that `apply_overrides_for` reports `NoSuchMapping`**, the path nothing covers today.
- **Not doing: merging `DeclaredContexts`'s two function pointers.** `apply_to_context` rewrites the
  shared default and every instance, and `apply_to_entity` rewrites one instance. Those are two
  operations rather than a copy of one.
- **Verified by:** the new test, the existing suite unchanged, and no diff in `examples/`.

### 144. One rule for what counts as a character

A logical key is decided from text in two places: `eval.rs`'s `bound_character`, from a key event's
`Key::Character`, and `Control::from_name` (`present.rs`), from a saved `char/` name. Both take
exactly one `char`, then `normalize_character`, written out twice. The two have to agree — a saved
name is only good if a key event can produce the same character — and nothing makes them.

- **One `pub(crate) fn single_character(text: &str) -> Option<char>`** beside `normalize_character`
  in `binding/control.rs`, returning the normalized character when `text` holds exactly one.
  `bound_character` and `from_name` both call it.
- **Not doing: `normalize_character`'s own one-character match.** It asks whether lowercasing is
  one-for-one, which is a different question.
- **Verified by:** a unit test on `single_character` (empty, one, two, uppercase), `from_name`'s
  existing `char/` tests unchanged, and no diff in `examples/`.

### 158. Reserving an authority is an error

`.reserved()` on an `Authority` binding reserves nothing: an authority has no controls, so
`reserved` in `binding/builder.rs` collects none, and the declaration succeeds without a word. R4.8
forbids a build that silently does nothing. Found in 151f, where the Steam build's pad reaches the
controls screen only through authorities.

- **`DiagnosticKind::ReservedAuthority`, an error**, found in `plan.rs` beside `ReservedAndMappable`
  and `DeltaFromAuthority`, and listed in TD4's enum. Its message says the authority's own screen is
  where its bindings are recovered (TD9.3).
- **A clause in R4.8** names the case, since it is the requirement the silence breaks.
- **Not doing: reserving through the backend.** Whether Steam can pin an action to a control is a
  question about Steam, and the crate has nothing to ask it with.
- **Verified by:** a unit test declaring `.reserved()` on an authority and finding the diagnostic,
  and no diff in `examples/`, since no example reserves an authority.

### 112. A backend suppresses a device family at L0

R0.6's other half, and the smallest it will ever be: `docs/steam.md` S1 and S3 killed the
per-device policy D22 assumed, so what is left is a family switch.

- **A resource naming the suppressed families**, read by `sample_input` (`frame.rs:304`) as a guard
  around each family's own `MessageReader` loop. Nothing above L1 changes, because a suppressed
  family simply never reaches the frame.
- **Declared by the game, not detected**, because a suppression that turns itself on and off between
  runs is the worst kind of input bug to diagnose. The backend's own plugin sets it once its init
  succeeds.
- **Replay is what justifies this, not Steam.** A replay backend mutes live hardware in a build that
  compiled the driver in (R0.6, R10.8), which no build configuration expresses. A Steam build can
  instead take `bevy/gamepad` without `bevy_gilrs` and produce no hardware event to suppress; D22
  records why the chunk stands anyway, and which of its old reasons was assumed rather than
  measured.
- **Not a Cargo feature on this crate**, because a feature that removes behaviour is not additive.
- **Per family rather than per device** because per device is not implementable, not because it is
  cheaper. S3: Steam hands out an `InputHandle_t` and nothing relates it to an OS device. If Valve
  ever exposes that mapping the policy narrows and nothing above L0 notices.
- **Verification is headless and needs no Steam.** An `App` with `gamepad` enabled, the resource
  set, a synthetic `RawGamepadEvent` pushed through the sampler, and the frame asserted empty; the
  same test with the resource clear asserts it arrives. The eight-combination matrix covers the
  `cfg` interaction, since suppression has to compile with each family absent.
- **Not doing: keyboard and mouse.** Steam emulates both alongside the pad (S1) and the family
  switch would silence them wholesale, which is wrong — a player using a pad through Steam still
  types. Steam does emit keys and mouse motion for a pad while the game reads Steam Input natively
  (S27), as events nothing can tell from the real devices', so no family switch reaches them. This
  chunk suppresses what a backend actually owns.
- **Steam does not need this for the pad.** A Steam build without `bevy_gilrs` has no hardware event
  to suppress, and it runs cleanly with the client absent (`docs/steam.md` S21), so one binary
  serves both launches without switching anything off. Replay is this chunk's only customer for the
  gamepad family.
- **Not doing: R0.4's per-family split**, which lives at L2 and landed as chunk 151a.

### 28. Docs that run

- **Make the doctests execute anywhere.** `scripts/verify.sh --doc` runs them on macOS, pointing
  dyld at the toolchain's libstd that `dynamic_linking` on the `bevy` dev-dependency leaves the
  merged doctest binary unable to find. That repairs the run, not the crate — a plain
  `cargo test --doc` still dies, and so does the workaround on Linux. The portable fix is making
  `dynamic_linking` opt-in, at the cost of slower example builds, a trade-off to make deliberately
  rather than inherit.
- **The 42 `ignore` fences**, against nine doctests that are live code. The rest are fragments
  written to read mid-prose — `context.bind::<Move>(Stick::Left).dead_zone(…)` — compilable only
  inside an `App` and a context closure, so each needs hidden `#` scaffolding before the compiler
  checks it. Some twenty files, and the bulk of what "docs that run" means. One of the 42 is the
  macros crate's only doctest, which until now no step reached at all.
- **The README rewrite** — a user-facing introduction, feature list and quickstart, with examples
  lifted from a real game rather than invented.
- **`src/lib.rs`'s crate-level docs, alongside it.** The `//!` block largely mirrors the README's
  Concepts section and has drifted the same way — both were drafted early and neither has kept pace
  with what the crate grew into since.
- **Comparison upkeep.** [docs/comparison.md](./docs/comparison.md) is read against BEI 0.26.0 and
  LWIM 0.21.0, which is a claim with a date on it. Both crates move.
- **`cargo doc` is not in the Verification list, and fails outside `--all-features`.** The
  no-devices build reports eighteen unresolved intra-doc links, every one to a feature-gated item
  linked from ungated prose — `KeyCode`, `LogicalKey`, `SavedOverrides`, `active_in_state`, `Stick`,
  `InputFramePlugin` among them. `--all-features` resolves all of them and so hides the lot.
- **Review surface:** read the rendered docs, not the diff. `cargo doc --all-features --open`, and
  look at the module pages the way a stranger would.

---

## Driving an example from outside

Nothing in tree can run an example, act on it and see the result without a person at the keyboard.
Live screenshots and window driving have been unreliable, so every check that needs a running window
has been made by reading instead. What is wanted works the way Playwright does: start the app, find
an element by a path of names, click it, wait until what it opens exists and has loaded, take a
screenshot, quit — all over HTTP and JSON.

It is developed here because that is faster, as a workspace member (`bevy_remote_driver/`) with its
own documents, and is meant to leave as a standalone crate, so what it needs travels with it. Which
of its server methods are proposed upstream to Bevy is a separate question, and DD8's.

### 149. The mapper's `remote` feature

R25 and DD6: `action_map.dump` and `action_map.authority`, and the client's generic `call` step for
a method it does not know, which is how a plan reaches either.

`call` polls while the step carries a `value` and invokes once where it does not: the client's
checks re-run until they pass, which is what a read wants and what a write cannot have. `dump` is
the read, `authority` the write. Matching reuses `expect`'s `subset`, so a result is given as a
skeleton rather than through a pointer language — confirm first that `inspect::dump` does not put
instances in a list, which `subset` cannot index.

Which to assert on: the game's own state where that is the subject and unambiguous, since it is
reflected already and `expect` reads it today; the dump where the mapper is the subject, as a
rebinding is.

- **Not doing:** a step of the mapper's own, or a Python helper over `call`. Deferred below.
- **Not doing:** raw injection, which R25.4 says needs nothing.
- **Verified by:** a headless test of each handler, and 148's plan reaching one through `call`.

## A Steam demo

The authority seam has had one customer, `pong_robot`, whose authority is a robot. Several things
are unbuildable or unverifiable without a real backend: a delegated rebind, glyphs a backend
supplies, and prompt invalidation the backend drives. A Steam build of Disasteroids, then of Split
Friction, is the real backend. It lives in `steam_examples/`, beside `steam_probe/`, with its own
`Cargo.toml` outside the workspace so `steamworks-sys` never builds in `verify.sh`.

151c and 151e are independent of each other.

Every chunk here is an audit rather than a gate. It needs a running client, a pad, and a layout
bound by hand (`docs/steam.md` S16), and no CI can run it.

### 156. Navigating from no focus

Disasteroids' controls screen can lose focus with the screen still open, and a player with only a
pad then has no way back: `navigate` discards `DirectionalNavigationError::NoFocus`, so the stick
does nothing, and only closing and reopening the screen lets `AutoFocus` restore it. Seen in 151b's
flight test, cause unknown; the route found in the code is a pointer press on empty space, which
`click_to_focus` bubbles to the window and `acquire_focus_directional` does not catch.

- **First, reproduce the loss.** It followed a fat-fingered `ToggleSettings`; the stick held while
  pressing it is the other suspect. Reading the code found neither clearing focus: close and reopen
  cannot share a frame, and `Navigate` is evaluated before the transition that spawns the screen.
  Drive base Disasteroids' virtual pad over the remote driver (`pad.py`) with Y pressed twice a
  frame or two apart, and with Y pressed on a held stick, reading `InputFocus` after each. A cause
  found here is fixed here, alongside the fallback below.
- **On `NoFocus`, focus the screen's `AutoFocus` entity**, so the first direction selects instead of
  moving, as a console menu does. Clicking away still clears focus, which is what a mouse user
  expects.
- **Whether `press_focused_button` takes the same fallback**, for a player who presses South before
  a direction, is decided here.
- **Not doing: an always-focused invariant.** It needs the same default, a system or observer to
  hold it, and it would undo a deliberate click-away.
- **Verified by:** a `disasteroids/pad.py` step that clears `InputFocus` and navigates, if the
  driver can clear it; by hand otherwise.

### 151c. Prompts from Steam's origins

R18.8, R18.9 and R18.10 against a real backend.

- **A `Prompts` implementation in `steam_examples/`** answers the gamepad family with
  `ControlOrigin::Foreign`, from the action's origins and `get_string_for_action_origin` (S17).
- **Glyphs load through a custom `AssetSource`.** `get_glyph_for_action_origin` returns an absolute
  path inside the client's bundle (S17), which `AssetPath::from_path_buf` carries without escaping.
  The reader canonicalizes it and refuses anything outside that one root before reading, rather than
  trusting an SDK's return value as a bare path. One scheme and one root: a second scheme is
  warranted only by a second root with its own lifecycle, not by a second SDK call returning a path.
- **The glyphs are themed.** The path carries `dark`, so the light variant is selected rather than
  assumed.
- **A rebind in the overlay invalidates prompts** (R18.10) through `PromptGeneration::invalidate`.
  How the change is detected is open: a configuration-loaded callback if `steamworks` exposes one, a
  poll of the origins otherwise.
- **Verified by:** the Steam build's hint line prompting for the pad with Steam's glyph, and
  changing after a rebind in the overlay.

### 151e. Split Friction on Steam

Two players are two `InputHandle_t`s, not two action sets (`docs/steam.md`'s appendix).

- **One set, activated per controller**, each handle's entity `Paired` to its protagonist; join is a
  declared action prompted by name.
- **The backend writes `RawGamepadEvent::Connection`** when a handle appears or goes, since nothing
  else will (D73), and an action held on a vanished pad releases (R11.4).
- **Chunk 116's pool runs on Steam's entities**, which is the test of "backend-neutral".
- **Brand is `InputType` mapped onto `GamepadBrand`**, or `Generic` where it has no answer (S7).
- **Measured here:** whether a handle survives a relaunch and a client restart, which settles D74's
  claim either way.
- **Measured here:** how Steam orders its connected pads with two awake, whether that order is
  stable, and which pad `show_binding_panel` opens for. 151f's flight test saw Steam hesitate to
  switch between the PS5 and Xbox layouts with both awake, cause unknown.
- **Not doing: R15.9.** Steam has one account per machine, not one per controller.
- **Verified by:** two pads joining, walking, and one unplugged mid-game with its held input
  released.

---

## Deliberately deferred

Every row states its gate. A row with no gate is an item that will be dropped, which is ground rule
5.

| Area | Gated on |
| --- | --- |
| **Focus orchestration: guidance, and a worked example** (D87) | **community feedback, and Bevy's own direction on driving widget state from outside.** D87 keeps the mapper focus-agnostic, so the layer binding actions to whichever widget has focus sits outside the crate — `widget_focus.rs` is one, and names the role. What is deferred is telling someone how to build their own: a requirements-and-design section for an orchestrator, and the worked example of held-down visual state with the latch D87 describes. Guidance is the deliverable whether or not this project ever ships an orchestrator itself. Both gates are real rather than a delay. The first: that a game activating widgets from the keyboard and the pad wants the pressed highlight a mouse gives, and that the one-shot activation `widget_focus.rs` ships today is not already enough, are guesses about other people's UI. The second: open Bevy issues on remote control of widget state will change what an orchestrator has to do, so guidance written now would describe a shape about to move. Nothing is blocked — a game wanting the highlight has D87's rule and no crate change to wait for |
| **Resolving a stored device identity to the connected devices that match it** | a caller asking in that direction. Written for chunk 72 and withdrawn for want of one; chunks 72d and 92 did not need it either. Devices arrive as connection events one at a time, the ones already plugged in at launch included, so every caller has one device in hand and asks the inverse question. Two identical pads never present themselves as a set to choose from |
| **Proposing the driver's server methods upstream** (DD8) | **chunk 148's plan passing, and a second plan against a different example.** Four gaps: selection by name path, where a UI node is drawn, readiness as state, and a reflected `DiagnosticsStore`. The deliverable is a short brief for each, for the author to edit and post. Until two apps have used the methods, their shapes are guesses about what a test needs |
| **Isolating an app under test from what it has saved** | **an app whose state a plan cannot reach through its own controls.** A run shares the developer's settings file: Disasteroids saves a confirmed rebind, so chunk 148's plan found its own last run's rebinding still there and failed on the second run. It now bookends itself with Reset and Confirm (DD9), which is a plan reaching a known state the way a player would — the right answer while an app offers one, and it exercises two more paths besides. What it does not cover is a plan that fails halfway, which leaves the file dirty for the next run to reset. Deleting the file first is the obvious answer and is half of one: it makes a run repeatable without stopping it writing the developer's real settings on the way out, which the bookend does handle. Isolation is the whole answer. At rc.1 it costs a platform branch — `XDG_CONFIG_HOME` on Linux, `LOCALAPPDATA` on Windows, and on macOS `HOME` itself, since `preferences_dir` is `home_dir()/Library/Preferences` with no narrower lever. [bevy#25902][], merged to main on 24 September 2026, after rc.1, makes `preferences_dir` honour an absolute `BEVY_SETTINGS_DIR` on all three, so past it isolation is one variable in `environment()` in `run.py`. Whichever is built wants a check that the throwaway directory was actually written, because a path or variable that is wrong fails silently: nothing is deleted, or the redirect does not take, and the plan passes while reading the real file. `SettingsPlugin`'s `app_name` is the directory component and is a plain `pub` field, so a per-run app id would isolate with no platform code at all — turned down because the example would have to read an env var, and a plan drives an example without the example knowing it is under test |
| **A step of the mapper's own, or a Python helper over `call`** | **a second plan whose raw `call` steps are unreadable**, which is DD8's gate as well. Both extension points exist: BRP registration is the Rust one, and the mapper's `remote` feature adding `action_map.*` is already a plugin adding methods with no dependency either way; a Python plan is a program (DD4.2), so a helper is a module it imports. What is deferred is sugar over those, and a step registry would be a third mechanism where two already reach |
| **Deleting the gamepad message registration** | this crate's Bevy pin moving past [bevy#25904][], merged to main on 23 September 2026, after rc.1. `RemoteDriverPlugin` registers `ReflectMessage` for `GamepadConnectionEvent` and `RawGamepadEvent`, because `bevy_input`'s nine gamepad messages carry no `reflect(Message)` where the ten in its other input modules all do, and `world.write_message` refuses a message without it (DD5.3). Whether it makes 0.20 final is not known, so the registration ships rather than waits. What to check on a bump is only whether the pinned version has it, because nothing else will say so: `register_type_data` over data a type already carries is an overwrite rather than an error, so the redundancy is silent |
| **A virtual pad that says which pad it is** | **a plan whose subject is brand-specific presentation.** The connection event carries `name`, `vendor_id` and `product_id`, and the client fills all three with a pad no vendor table knows, so every machine sees the same fallback rather than whatever is plugged in. Exposing them is three optional arguments and no new mechanism; what is missing is a plan that would read differently for a pad a game recognizes, which is the generic tier's art row from the other side |
| **A second virtual pad, and disconnecting one** | **a plan driving Split Friction, or one whose subject is a pad going away.** The client holds one pad's entity and connects it on first use, so a second is another entity and a `pad` step that says which; a disconnect is one more message on the one it already has. Neither is hard and neither has a caller: Disasteroids is one player who never unplugs anything, so nothing in tree can tell a pad from the pad |
| **A presentation crate** (`bevy_action_map_ui`) | **Bevy deciding to take this crate upstream**, which is when the workspace has to be arranged properly regardless. Until then the layer is `examples/common/` — `prompt_ui.rs` and `widget_focus.rs`, both written against the public API with nothing added to the crate for them. What is deferred is packaging, not work; the cost of waiting is a `#[path]` import. The crate's docs owe a game the warning `widget_focus.rs` carries today: `InputDispatchPlugin` in `DefaultPlugins` activates a focused `Button` on a key a context has consumed, so a game using both disables it |
| **The generic tier's art** | **the presentation crate**, which is what has to ship an atlas with no holes in it. Kenney's generic set is blank, unlabeled buttons, so each generic face button needs a short text stamp authored onto it by hand: content work with a known answer, and no code. Until then an unrecognized pad's face buttons resolve to text, which is also where the fallback chain shows itself in tree, on Disasteroids' Cancel and Confirm and in the gallery's Generic brand |
| **A stepper adjusted by a pad Steam owns** | **a Steam build whose screen has a stepper.** Chunk 151f binds `Activate` to an authority and leaves `Adjust` on the keyboard, because no stepper remains on the Steam controls screen. The base game gives Adjust only the D-pad's left and right, and only while a stepper has focus, so up and down still navigate. A Steam set gives an input to one action, so the faithful counterpart is an action-set layer switched on while a stepper has focus, not the D-pad in joystick mode for the whole menu. `adjust_focused_stepper` also passes an analog value through as the step, so it would need a sign taken first |
| **Netcode injection and reconciliation** | a networked target. The injection point is built: an `Authority` binding and `AuthorityValues` (chunks 111 and 151a), so a peer's resolved action already has somewhere to go (D71). Rollback's local half — snapshot, restore, re-simulate — is chunk 83, which also takes the held-state containers. Injection targets L2 (D69): a network authority backend supplies the already-resolved `ActionValue`, not a raw frame, so no shared `Plan` across peers and no hold timers or tap counts on the wire. What is left here needs a remote player's resolved action to inject and a later correction to reconcile against it |
| **Timestamped authority transitions** | **an authority that has edges to give**: a network peer, or a replay backend. `AuthorityValues` is a level sampled once a tick (D92), so a press and release between two of an authority's writes never reach the action and its resolution is its own poll. A peer or a replay that recorded the transitions would gain, and so might Steam Input: its action event callbacks may deliver ordered edges between polls, which `docs/steam.md` carries as an unmeasured question. Ordered is enough, since D4 has timestamps order events rather than time them. What it needs is somewhere to put them |
| **Consumption-aware `FocusedInput` dispatch** (R8.2a) | **a game wanting `bevy_ui_widgets`' own widgets working generically, unmodified, without a context per widget kind.** A context per kind is the path to reach for first, and Disasteroids ships that way. A design for the filter was built and set aside: a lowest-priority, non-consuming context binding `ControlClass::AnyButton`, feeding dispatch through the existing class-binding pipeline rather than a second raw-message read — keyboard only, since every keyboard-driven widget observer at 0.20 gates on `ButtonState::Pressed` and none reacts to a release |
| **Promoting `WidgetKind` and the per-kind context into the crate** | [bevy#25592][], the author's own upstream proposal for a `bevy_ui_widgets`-native widget-kind id. Promoting a shape this crate invented first, ahead of that conversation, risks committing to the wrong one |
| **Deleting `acquire_focus_directional`** | this crate's Bevy pin moving past [bevy#25675][], merged to main on 24 September 2026, after rc.1. `examples/common/widget_focus.rs` carries a global `AcquireFocus` observer mirroring `acquire_focus_tab_index`, with `AutoDirectionalNavigation` standing in for `TabIndex`: `bevy_input_focus`'s `click_to_focus` bubbles an `AcquireFocus` on every pointer press, a screen navigating by anything but `TabIndex` intercepts it nowhere, so it reaches the window and clears focus — and a widget whose interactive children are separate entities, like a stepper's two chevrons, blinks on every press rather than rarely. The PR separates focusability from navigation policy behind a `Focusable` component and fixes [bevy#25596][], click-to-focus under directional navigation. Read on main, it retires the observer outright, including for a scheme that is neither `TabIndex` nor one of upstream's own: `InputFocusPlugin` installs an `acquire_focus` observer that stops at the first `Focusable` ancestor whatever the scheme, and `AutoDirectionalNavigation` requires `Focusable`. What is left on the bump is the deletion, and the stepper's chevrons as its test |
| **Deleting `examples/common/font.rs`** | this crate's Bevy pin moving past [bevy#25847][], which answers [bevy#25842][] and merged to main on 22 September 2026, after rc.1. Until then the plugin overwrites the `default_font` feature's slot at `AssetId::default()` during plugin build, which depends on that slot's location and on text layout registering a font id once. The answer is a `DefaultFontSource` resource that `FontSource::Default`, now `TextFont`'s default, resolves to. Changing it rebuilds the font collection and marks every `TextFont` changed, so it reacts to a change and the build-time ordering goes with the file |
| **Dropping the pre-scaled inline glyph art** | this crate's Bevy pin moving past [bevy#25767][], merged to main after rc.1, which gives `InlineImage` a fixed `width` and `height`. At rc.1 an inline image sizes itself from the loaded image's pixel dimensions, so `prompt_ui.rs` loads inline glyphs from a second, pre-scaled `input_prompts_inline/` tree. With a size on the component, the inline prompt can load the block art and set its height from the line |
| **Whether an action is live, as something a hint can follow** (R18.2's withdrawal) | reactive UI in Bevy, so a hint's visibility can be bound to a predicate rather than set by a system the game writes. Until then a game hides a hint from its own state, which it knows better than the crate does: which menu is open, whether play is paused. The predicate would be whether a carried, active context binds the action to a control nothing stronger consumes; D84 is why it is not a filter on the prompt lookup |
| **Asking whether a row would be refused, before Confirm** | **a refusal a screen's working copy can reach that capture does not already turn down.** Chunk 127 removed the last one: a wrong family, a wrong shape and a reserved control are refused at capture, and a `Fixed` row has no cell to capture into. The screen writes gestures into `PendingOverrides` and only Confirm asks whether they are legal, so a new refusal of that kind is shown taking and then silently lost. `refusal` is private and wants the declared bindings, so the general answer is a dry run of the apply, which is public API. The cheaper answers are for the row to say what would be refused so a screen can decline the gesture, or for the screen to apply eagerly and keep Confirm for persistence |
| **A context-level exclusion from the mapping list** | a second screen needing the same filter and duplicating it. `ActionMapping::context` already carries the data, and one call site filtering on it costs one line — at two, the crate is the one paying for the repetition |
| **An initial delay distinct from the repeat rate** (R22.5) | **a screen long enough to feel the difference.** `.on_change().pulse(0.25)` gives one number serving as both. Two numbers is a small change; what is missing is a case where equal is wrong, and a two-table settings screen is not it |
| **Free-form mutually-exclusive context sets** (R7.7 remainder) | nothing in tree needs two independently-exclusive contexts to coexist rather than one dominating the other by priority |
| **A game-wide "more forgiving timings" control** (R20.4's withdrawal) | a game with enough timings that setting them one at a time is the complaint. One player-facing control across a whole game needs the crate to know which way forgiveness runs per threshold — down for `Hold` and `HoldAndRelease`'s floors, up for `Tap` and `MultiTap`'s ceilings and `Pulse`'s interval — which is the one part of this a game cannot get right without hand-checking five signs, and the reason the row exists rather than the idea being dropped with the requirement. Chunk 115's per-timing tunables come first regardless: they are what a game would expose the control *through*, and they may turn out to be all anyone wants |
| **Auto-switching which device a player is paired to** (R15.8) | **a single-player game in tree that wants it**, which is where the value is: asked directly, LWIM's maintainer put pad-to-keyboard switching at mattering a bit, and much more in single player or networked multiplayer than in local co-op. One person pressing things makes "which device are they on now" a question with one right answer; two make it the wrong question, which is why Split Friction joins once and a player who wants the keyboard takes it the same way they took the pad. The gate stays untripped for a reason rather than for want of demand: Disasteroids is the single-player game, and it pins `PromptDevice` to the keyboard on purpose, being a desktop game whose prompts name keys with a pad plugged in. Deferred rather than withdrawn alongside R15.7, because unlike R15.7 an app cannot write it: telling a deliberate grab from a drifting stick means reading raw samples under a deadzone floor before any action fires, which an app watching `Fired` never sees. If it lands, a prompt reads the player's paired device rather than tracking one of its own, and R18.6 revives with it |
| **Nintendo's confirm button** (what R18.7's withdrawal left) | **a game that wants confirm to follow the pad in hand**, checked first on a Nintendo pad reporting through gilrs. A Nintendo pad confirms with A, in the East position, where every other brand confirms with South. Only the gilrs path sees this, since a Steam Input backend hands over actions already mapped. Read, not run: gilrs takes SDL's `a`/`b` as `South`/`East`, and SDL_GameControllerDB maps a Nintendo pad by position (its `mapping_guide.png`), so A arrives as `East` and a game confirming on South confirms on a Nintendo player's B. A preset swapping South and East fixes that for a game that knows its player. Following the pad instead is per device rather than per family, since two brands can share one game, and `Brand` is already on the gamepad entity to read |
| **Opaque platform-user identity** (R15.9) | a real platform SDK. Floated for Split Friction, but Steam has one account per machine rather than one per controller, so even 151e has nothing to attach |
| **Naming which authority produced a value** (R0.5's queryable half) | a build with two authorities in it. That a delegated action is indistinguishable from a bound one at the call site is the requirement's point; what has no reader is *which* authority supplied it. Chunk 111 left `AuthorityValues` unnamed rather than adding a field nothing consults, and one authority cannot motivate a name — `pong_robot`'s robot has nothing to be told apart from |
| **An authority backend's actions in rollback** (D22's remainder) | a snapshot to fit them into. `AuthorityValues` is a plain component and clones with the entity, but what a rewind has to reproduce is what the authority *said* on the tick being re-simulated, which is not in the frame. The available answer is recording the backend's output into the frame at sample time, at the cost of a larger frame |
| **Sub-frame event timing** (D4's remainder) | [bevy#9087][] upstream. Gamepad stays frame-quantized regardless until gilrs polling is rewritten, so mixed fidelity across sources is permanent for now rather than an artifact |
| **Schedule enforcement for tick domains** (D9's remainder) | Bevy giving a `SystemParam` a way to know its own schedule. A plugin-time validation pass and a debug assertion stand in |
| **An authority on a `Delta2` action** (Steam's `absolute_mouse`) | **a Steam build of a game with a look action**, most likely chunk 121's camera. Chunk 151a refuses the declaration. The design: a delta must be folded once, where a level may be read by every tick, so a context folds a `Delta2` authority value only when `AuthorityValues` has changed since that context's evaluator last ran, and zero otherwise. That is the frame cursor's rule for real mouse motion, with the component's change as the cursor; an explicit write counter is the alternative if change detection proves too implicit. Steam's delta accumulates since the last read and the read consumes it (S26), so a backend reads it once per frame; that decides how a backend polls, not what the crate does |
| **OS gestures as binding sources** (R13.7) | **a game that wants one, and can say what it should do.** Pinch, rotation, pan and double-tap arrive as `bevy_input::gestures` events — window-level, carrying no pointer and no entity — so they are the wheel's shape rather than picking's, which is why section 13 keeps them where it withdrew the rest of the pointer. What is missing is not a mechanism: the design questions are what a pinch's units are and whether it wants the modifier chain a stick does, and neither can be answered without a customer to ask |
| **Suspend/resume** (R16.3; mobile, console) | a platform target that needs it. Nothing in this crate's supported platforms emits a suspend signal or has a device re-enumeration step to hook |
| **Split Friction's monsters, spawners and missiles** | a mechanic that would exercise input this crate has not already proven. Kept as a row rather than deleted because the sprites, the dungeon's region aspects and a `Fire`-shaped action all exist, so changing our mind is cheap |
| **Guardian migration** | porting it from Bevy 0.16.1 with `bevy_enhanced_input` 0.12 to 0.20 — four versions, and a port plus a rewrite. Doing both at once would confuse "action_map is wrong" with "0.20 moved this" |
| **A devfmt usage log, to catch the misses nobody notices** | **hand reflows still happening now that repacking is canonical.** Measured before deferring: 350 loose breaks in tree against 6 pure rewraps in 60 commits, so the aftermath of a devfmt run lives in working-tree churn and not in history — `git log` cannot be mined for it, and devfmt is the only thing positioned to see it. The shape, if it revives: devfmt appends to a gitignored log from the process already being run, costing no approval and no tokens; per paragraph it records a hash of the word sequence and a hash of the physical lines, so a later run finding the same words under different line breaks has caught a miss and can attribute it to its own earlier decision. Worth building only with a mechanical trigger to read it — one line of output when the count crosses a threshold — since a log nobody opens is cost with no signal |
| **Repacking the prose `devfmt` never reached** | **the residue stopping its own shrink.** Prose wrapped before repacking existed and never repacked since; `--diff` repacks whatever a commit touches, so what is left is the paragraphs in files nothing is working on, and a sweep buys less each month. Measured today, files needing a reflow: `src` 21/25, `docs` 6/6, `examples` 22/38, `macros` 1/1, `tests` 2/12. A `--sweep <dir>` is warranted when that stops falling, or ahead of reading a directory end to end. What rides with it: six backtick spans broken across two lines (`Requirements.md`, `Roadmap.md`, `docs/design.md`, `docs/issues.md`, `src/binding/control.rs`, `examples/split_friction/main.rs`), left by the bug 117m fixed. `tools/devfmt/src/main.rs` is swept by hand or not at all — its fixtures are string literals full of `///`, which is the row below. `archive/` is excluded: nothing in flight reasons from it |
| **`devfmt` reading a comment marker inside a string literal** | a second file in tree acquiring one. A `.rs` line whose trimmed text starts with `//` inside a string literal is reflowed as though it were a comment — the module header's "does not occur in idiomatic Rust" assumption, which `devfmt`'s own test fixtures are the sole counter-example to, and they are also the one file a devfmt chunk edits. So the cost today is a hand check on a file already under review, not a corruption nobody sees. Telling the two apart needs a Rust lexer carrying string state, raw strings and `\`-continued literals, which is a different tool from the line classifier this is built on; a second file acquiring one is what changes that arithmetic |
| **A physical binding's label matching the current layout** (R12.2, R12.7) | winit exposing a physical-to-logical query and a layout-change signal, requested as [winit#4606][] and tracked by the broader [winit#2678][], open since February 2023 and unimplemented. A workaround was scoped and set aside: `run_captures` already sees the logical key at capture time, but keeping it means a new field on `ControlCaptured`, a session table `present.rs` consults ahead of the static fallback, and an honest answer on whether it survives a save — which drags in the still-deferred binding-definition serialization (R17.6, R22.16) for a fix that only covers controls a player has personally rebound. A landed query supersedes it outright, for every physical binding rather than only captured ones, so the workaround is not worth building ahead of it |

---

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
[bevy#19741]: https://github.com/bevyengine/bevy/issues/19741
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
