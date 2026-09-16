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
  with no tooling behind it. Four violations have reached that path, every one caught by reading —
  and one of them was later recorded as gone on a reading that missed a rename
  ([docs/issues.md](./docs/issues.md) 1035). Two are still live.

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
- **Verified by:** calibrating a drifting stick, quitting, relaunching, and finding the stick still
  corrected.

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
- **Closed over the four keyboard modifiers, not generic.** A new `with_modifier(ChordModifier,
  Side)` carries this, where `ChordModifier` is `Ctrl | Shift | Alt | Super` and `Side` is
  `Left | Right | Either` — not `with_either(impl Into<ButtonControl>, impl Into<ButtonControl>)`
  over two arbitrary controls. R12.3 sits under §12, "Keyboard specifics", and never asked for an
  either-shaped bumper or any other pairing; a permissive signature would only add combinations
  nobody asked for and nothing downstream can describe. `with` is unchanged for every other chord
  entry.
- **`Side::Either` is `Fixed`, and stays that way.** It is declared at bind time, not observed:
  capture resolves one keypress to one concrete control, and no keypress means "either side" for it
  to answer with. A rebind touching this row could only replace it with a concrete `Left` or
  `Right`, never hand `Either` back, so the row carries no mapping. A game wanting a rebindable
  "either" still writes two `Fixed` rows by hand, one physical key apiece.
- **Presentation gets a chord-level `fallback_label`, and it is total.** A chord entry is either an
  ordinary control (unchanged) or one of the four modifiers under `Either`, which reads as the bare
  modifier name — `Shift`, not `Left Shift, Right Shift`. Every case is enumerated because the type
  admits nothing else, so there is no pair this can fail to name and nothing for a caller to
  decompose.

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

### 118a. Capacity is the app's business, and one ceiling replaces it

`ActionMapping::capacity` goes, with `mappable_upto`, `mappable_any` and `mapping::widest`. A
mapping is an ordered list; how long it ought to be is the settings screen's decision — how many
columns to draw, whether a row may grow, what a blank cell offers.

- **Measured before deciding.** `mappable_any` — the growable-list case D29 was written for — is
  reached once in tree, by a unit test asserting `capacity == None`. No example uses it.
  `mappable_upto` is used twice, both `upto(2)`, both meaning "ship one default, draw a blank second
  cell"; and Disasteroids takes `max` across the table (`settings.rs:922`), so the per-row widths
  capacity exists to express are read by no screen in tree.
- **One ceiling, everywhere, rather than only on load.** A `MAX_SLOTS` constant enforced at three
  points: the derivation in `mappings_of` as a plan-build diagnostic, `refusal` on the apply path,
  and `for_slot`. `TooManyControls` is repurposed rather than deleted — `capacity: Option<usize>`
  becomes a plain `limit: usize` — so a damaged save naming ten thousand controls for one row is
  still refused once capacity has stopped doing it by accident.
- **Chunk 81 is reversed.** A rebound row keeping its declared capacity was the whole of that chunk,
  and `current_rows`' widening exception goes with the field it widened.
- **D29 is half-reversed, and R19.9's capacity paragraph withdrawn.** The ordered list survives;
  "capacity is inferred from the defaults and raisable by the author" does not, and the withdrawal
  keeps enough to stop a per-mapping width being re-proposed.
- **`for_slot` keeps its `Option`**, and its doc says density rather than capacity: the save format
  is a dense list until 118b, so a skipped slot still has nowhere to go.
- **Not doing: holes**, which are 118b. Nor `RebindPolicy`, which answers a different question and
  already carries the "may this cell be captured into" half a screen needs.
- **Verified by:** Disasteroids' settings screen unchanged on screen, with `slots_in` replaced by a
  screen-owned constant and its `exists` test folded into the `changeable` one.

### 118b. A slot may be empty

`Vec<Option<Control>>` through `ActionMapping::slots` and `Override::Controls`, so a player emptying
the primary leaves a gap rather than promoting the secondary into it.

- **Why the container rather than a sentinel.** A `Control::Empty` variant would cost every consumer
  of `Control` an arm meaning "not a control" — the frame, prompts, `fallback_label`, `admissible`,
  conflict comparison — and `conflicts` acquires a bug the first time two empty slots compare equal.
  A `BTreeMap<usize, Control>` answers a sparse-at-index-9000 question nobody asked and gives up the
  scalar shorthand §10.3 keeps on purpose.
- **The wire word already exists.** Every real control name carries a `/`, which is what lets
  `"cleared"` and `"external"` be bare words that cannot collide with one (R17.7). An empty slot
  inside a list is `"cleared"` — one word meaning the same thing at both levels, no JSON `null`, and
  a file a player can still edit by hand.
- **Trailing empties are not written.** A row normalizes to its last filled slot before
  serialization, so a two-column table whose secondaries are mostly blank does not fill a settings
  file with the word; a short list read back means the rest are empty. Deliberately less orthogonal
  than writing them out, because the file is a thing people open. `[Some(x), None]` therefore still
  takes the scalar shorthand, and `[None]` still folds to `Cleared`.
- **`rewrite` grows a fourth case, and it is issue 1010's.** `slot` indexes both the override list
  and `contributors`, which is dense by construction — an author cannot declare a gap — so the new
  arm is "the override emptied a slot the defaults fill", which drops one binding mid-row. That is
  the shape that empties the other three rows of a composite. **1010 is routed here**, and this
  chunk owns the design question it was unrouted for: dropping per part rather than per binding, or
  refusing the clear outright.
- **D48's three slot cases become four**, and D29 gains the empty slot.
- **Not doing: a per-slot reset.** `reset` is per row, and whether "restore just this cell" means
  the declared control or an empty one is a question no screen in tree asks.
- **Verified by:** clearing Disasteroids' primary and keeping the secondary, saved and reloaded with
  the gap intact; and a composite's other three directions surviving one of them being emptied.

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
brand → generic → text is required.

**The inline half has landed** — `Glyph`, `GlyphTier`, the fallback chain and `IconPromptSpan` all
exist, and the art resolves against them: every one of the 164 entries in `assets/input_prompts/`
answers to `format!("input_prompts/{tier}/{}.png", control.name())`. What is left is the block
prompt and the generic tier's art, below.

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
- **A letter is ambiguous where an icon is not**, which is the sharpest argument for this chunk and
  is visible in Split Friction's lobby today: "press A or Enter" names a pad button and a key, and
  nothing in the line says which is which. A real game uses the icon for exactly that reason. The
  text path stays the fallback it is, rather than being made cleverer.
- **Not doing: icons in the capture cells.** The settings table has no width to spare, and the two
  surfaces want different things anyway — a prompt is a hint, where a glyph reads fastest, while a
  capture cell is an editor showing the authoritative name of what is being changed.
- **Not doing: the generic tier's stamped art.** Kenney's generic set is blank, unlabeled buttons,
  so a generic-tier icon needs a short text stamp authored onto it by hand. That is a content task
  with a known answer, and until it is done the generic tier resolves to text.

---

## Snapshots

Taking a context's state out and putting it back, in memory between ticks. The other half of this
section was persistence — the same move, to a file between runs — and chunks 92 and 92b landed it.

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
  Those want a network; rewinding does not, and the injection point itself is already chunk 111's.
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

Four chunks no game asks for and no published crate can do without: an extension point nothing
outside has exercised, the reflection the documents promise, documentation that runs, and
documentation that is true.

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
  types. Whether Steam can emit keys for a pad while the game reads Steam Input natively is
  unmeasured, and `docs/steam.md` carries the question. This chunk suppresses what a backend
  actually owns and leaves that one open.
- **Not doing: R0.4's per-action split**, which lives at L2 and is chunk 111's, already landed.

### 42. A backend-owned row, and the rebind it delegates

What is left of the authority work once chunk 111 landed injection and chunk 112 takes suppression:
the half a controls screen asks for on demand rather than the half the evaluator reads every tick.
Nothing here is faked — `examples/pong_robot` is already an authority resolving a real action.

- **`RebindPolicy` needs a third state** — `docs/issues.md` 1022. `Here | Fixed` cannot tell a
  backend-owned row from an ordinary fixed one, so a screen reading `mappings()` has to consult its
  own working copy to say "not rebindable here, delegate to that backend's own UI".
- **Rebinding reports delegation as an outcome, not a failure** (R19.8), and R19.3's conflict
  detection does not run on those actions, because we do not own the rules they would be checked
  against.
- **Partial delegation is the case; total delegation is not.** `pong_robot` already has the shape:
  `Move` delegated in the robot's context and bound in the human's, so the action keeps its row and
  only what the row can offer changes. Steam is that shape one device family over — the keyboard
  rebound here, the pad delegated — so the screen renders both states side by side rather than
  hiding the action. The previous version of this chunk delegated a whole pad, which is the case no
  real game has.
- **The vehicle is Disasteroids, with one action delegated.** It owns the rebinding screen by a
  division already stated in tree: `examples/pong_robot/main.rs` says Pong has no settings screen
  because Disasteroids owns that, and building a second one there would reverse it. Delegating one
  action leaves the presets and the saved controls intact and adds exactly the row this chunk is
  about.
- **Not doing: origins** (R18.8), already closed — `ControlOrigin` carries a control that is not one
  of ours, with the same stored name and fallback label everything else renders from.
- **Not doing: glyphs** (R18.9), which has its own deferred row and its own asset questions.
- **Not doing: suppression at L0** (R0.6), which is chunk 112's whole subject.
- **Not doing: a backend trait.** Earlier drafts wanted one in `src/backend.rs`, from when injection
  was assumed to need it; chunk 111 landed injection through a component instead. Delegating a
  rebind is a call the game makes to a backend it chose — the crate's part is saying the row is not
  ours to capture on. A trait earns its place when a rebinding widget has to delegate without
  knowing its backend, which is the presentation-crate row.
- **Not doing: naming which authority produced a value** (R0.5's queryable half). One authority in a
  build cannot motivate a name; the deferred table carries it.

### 109. Derive `Reflect`, and turn on auto-registration instead of hand-writing it

R24.3 (`docs/issues.md` 1019): `Control`, `DeviceFamily`, `ActionMapping`, `RebindPolicy`, `Prompt`,
`ControlOrigin`, `DeviceHandle`, `ActionObstacle` and `Paired` carry no `Reflect`, and nothing calls
`register_type` for the two types that do (`action.rs`, `frame.rs`). The second half looked like it
needed a call per type, written once and kept in sync forever after — but [bevyengine/bevy#15030][]
means it doesn't: a non-generic `#[derive(Reflect)]` type registers itself at startup once
`bevy_reflect`'s `auto_register_inventory` feature is on, which it isn't here — this crate's
`bevy_reflect` dependency is `default-features = false` (`Cargo.toml:43`) and its own forwarded
feature (`Cargo.toml:116-123`) never re-adds it.

- **Derive `Reflect`** on the nine types named in 1019, same as `action.rs` and `frame.rs` already
  do it. Add `Identity` to that list: chunk 72 gave it to a device's entity and left it, like
  `Paired` beside it, without `Reflect`.
- **`register_device_identity` is the one `register_type` call in tree now**, added by chunk 72
  because a stored identity's domain has to resolve back to a concrete type at load. It stays
  either way — it registers type data, not just the type — but it is no longer true that nothing
  calls `register_type`.
- **Add `bevy_reflect/auto_register_inventory`** to this crate's own `bevy_reflect` feature, so
  every type above — and every one derived after this chunk — registers itself, rather than adding a
  `register_type` call this chunk would immediately have to write nine of and the next new type
  would silently skip.
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
  `InputFramePlugin` among them. `--all-features` resolves all of them and so hides the lot. Extends
  `docs/issues.md` 1038, which named only the all-features build.
- **Review surface:** read the rendered docs, not the diff. `cargo doc --all-features --open`, and
  look at the module pages the way a stranger would.

---

## Deliberately deferred

Every row states its gate. A row with no gate is an item that will be dropped, which is ground rule
5.

| Area | Gated on |
| --- | --- |
| **A real Steam backend, validated out of tree** | less than it looked. `docs/steam.md` S4 now says a borrowed app id does carry its own manifest from inside the client's bundle, and S13 answered R1.7 that way, so what is left needing an app id is only what S6 blocks: a configuration Steam actually applies. Whether that needs an app id at all, or only a windowed app Steam launches, is itself unmeasured. The shape is settled: a probe rather than a game, in its own repository, pinning `bevy_action_map` by git rev so it breaks only on a deliberate bump — and it is an audit rather than a gate, since it needs a client and a pad and cannot run in CI. `steam_probe/` is its gitignored seed and moves out when the repository exists. Nothing in this crate is blocked on it: per-family suppression is chunk 112, and the presentation half can be built against API signatures already verified to exist |
| **Named override profiles per user** (R17.4's first clause) | a game wanting more than one set of bindings under one player. The per-scheme half of R17.4 is already met — `Overrides` is keyed by family, so a keyboard remap cannot reach the gamepad layout — and chunk 92 stores one set per game, in one settings group. A second is another group, or a group holding several; which of those is right depends on whether profiles are switched at runtime, and nothing in tree switches one |
| **Resolving a stored device identity to the connected devices that match it** | a caller asking in that direction. Written for chunk 72 and withdrawn for want of one; chunks 72d and 92 did not need it either. Devices arrive as connection events one at a time, the ones already plugged in at launch included, so every caller has one device in hand and asks the inverse question. Two identical pads never present themselves as a set to choose from |
| **Glyphs from a backend** (R18.9) | the same asset questions from the other side. The *origin* half is closed — `ControlOrigin` already carries a control that is not one of ours, with the same stored name and fallback label everything else renders from — so what is deferred is the image rather than room for it. Measured (`docs/steam.md` S17): `get_glyph_for_action_origin` resolves to an absolute filesystem path inside the client's own app bundle — `Contents/MacOS/controller_base/images/api/dark/shared_lstick_md.png` on macOS, not the `tenfoot/resource/...` path this row previously guessed — and the `dark` component says the glyphs are themed, so a light variant has to be selected rather than assumed. A Bevy `AssetPath` can carry it natively via `from_path_buf` — no string-escaping the drive letter or backslashes. The path is not to be opened as given: a custom `AssetSource` reader must canonicalize it and reject anything outside a known root before reading, rather than trust an external SDK's return value as a bare filesystem path. One scheme, one hard-coded root is the right size while only this one root is confirmed; a second scheme is warranted only if a second root with its own lifecycle surfaces (e.g. something ephemeral, which cannot share a stable root's caching and hot-reload assumptions) — not one scheme per SDK call that happens to return a path |
| **A presentation crate** (`bevy_action_map_ui`) | **Bevy deciding to take this crate upstream**, which is when the workspace has to be arranged properly regardless. Until then the layer is `examples/common/` — `prompt_ui.rs` and `widget_focus.rs`, both written against the public API with nothing added to the crate for them. What is deferred is packaging, not work; the cost of waiting is a `#[path]` import |
| **Netcode injection and reconciliation** | a networked target. The injection point is built: chunk 111 landed `delegate` and `AuthorityValues`, so a peer's resolved action already has somewhere to go (D71). Rollback's local half — snapshot, restore, re-simulate — is chunk 83, which also takes the held-state containers. Injection targets L2 (D69): a network authority backend supplies the already-resolved `ActionValue`, not a raw frame, so no shared `Plan` across peers and no hold timers or tap counts on the wire. What is left here needs a remote player's resolved action to inject and a later correction to reconcile against it |
| **Consumption-aware `FocusedInput` dispatch** (R8.2a) | **a game wanting `bevy_ui_widgets`' own widgets working generically, unmodified, without a context per widget kind.** A context per kind is the path to reach for first, and Disasteroids ships that way. A design for the filter was built and set aside: a lowest-priority, non-consuming context binding `ControlClass::AnyButton`, feeding dispatch through the existing class-binding pipeline rather than a second raw-message read — keyboard only, since every keyboard-driven widget observer at 0.20 gates on `ButtonState::Pressed` and none reacts to a release |
| **Promoting `WidgetKind` and the per-kind context into the crate** | [bevy#25592][], the author's own upstream proposal for a `bevy_ui_widgets`-native widget-kind id. Promoting a shape this crate invented first, ahead of that conversation, risks committing to the wrong one |
| **A context-level exclusion from the mapping list** | a second screen needing the same filter and duplicating it. `ActionMapping::context` already carries the data, and one call site filtering on it costs one line — at two, the crate is the one paying for the repetition |
| **An initial delay distinct from the repeat rate** (R22.5) | **a screen long enough to feel the difference.** `.on_change().pulse(0.25)` gives one number serving as both. Two numbers is a small change; what is missing is a case where equal is wrong, and a two-table settings screen is not it |
| **Free-form mutually-exclusive context sets** (R7.7 remainder) | nothing in tree needs two independently-exclusive contexts to coexist rather than one dominating the other by priority |
| **Owner-scoped `ConsumedControls`/exclusion ceiling** (R15.3 remainder, and D13's own remainder) | a real in-tree case with a per-player exclusive context, or a binding consumed across two players' devices. Design if built: a claim visible only if made globally or by the viewer's own paired device; an exclusive context's shadow implicit in its own pairing rather than a separate flag |
| **A game-wide "more forgiving timings" control** (R20.4's withdrawal) | a game with enough timings that setting them one at a time is the complaint. One player-facing control across a whole game needs the crate to know which way forgiveness runs per threshold — down for `Hold` and `HoldAndRelease`'s floors, up for `Tap` and `MultiTap`'s ceilings and `Pulse`'s interval — which is the one part of this a game cannot get right without hand-checking five signs, and the reason the row exists rather than the idea being dropped with the requirement. Chunk 115's per-timing tunables come first regardless: they are what a game would expose the control *through*, and they may turn out to be all anyone wants |
| **Auto-switching which device a player is paired to** (R15.8) | a game where picking up the other device happens often enough that re-joining is a real cost. Split Friction joins once and a player who wants the keyboard instead can take it the same way they took the pad. Deferred rather than withdrawn alongside R15.7, because unlike R15.7 this is not something an app can write for itself: telling a deliberate grab from a drifting stick means reading the raw samples under a deadzone floor before any action fires, which an app watching `Fired` never sees. R18.6 stays withdrawn on it — if this lands, a prompt reads the player's paired device rather than tracking one of its own |
| **Opaque platform-user identity** (R15.9) | a real platform SDK. Floated for Split Friction, but there is nothing to show without one, and not worth a faked stub the way chunk 42 fakes a backend |
| **Naming which authority produced a value** (R0.5's queryable half) | a build with two authorities in it. That a delegated action is indistinguishable from a bound one at the call site is the requirement's point; what has no reader is *which* authority supplied it. Chunk 111 left `AuthorityValues` unnamed rather than adding a field nothing consults, and one authority cannot motivate a name — `pong_robot`'s robot has nothing to be told apart from |
| **An authority backend's actions in rollback** (D22's remainder) | a snapshot to fit them into. `AuthorityValues` is a plain component and clones with the entity, but what a rewind has to reproduce is what the authority *said* on the tick being re-simulated, which is not in the frame. The available answer is recording the backend's output into the frame at sample time, at the cost of a larger frame |
| **Sub-frame event timing** (D4's remainder) | [bevy#9087][] upstream. Gamepad stays frame-quantized regardless until gilrs polling is rewritten, so mixed fidelity across sources is permanent for now rather than an artifact |
| **Schedule enforcement for tick domains** (D9's remainder) | Bevy giving a `SystemParam` a way to know its own schedule. A plugin-time validation pass and a debug assertion stand in |
| **Mouse wheel as a binding source** (R13.3) | nothing in tree wants it. The wheel is a delta on its own channel, needs `Line`/`Pixel` normalization, and shares nothing with a button but the device |
| **Suspend/resume** (R16.3; mobile, console) | a platform target that needs it. Nothing in this crate's supported platforms emits a suspend signal or has a device re-enumeration step to hook |
| **Split Friction's monsters, spawners and missiles** | a mechanic that would exercise input this crate has not already proven. Kept as a row rather than deleted because the sprites, the dungeon's region aspects and a `Fire`-shaped action all exist, so changing our mind is cheap |
| **Guardian migration** | porting it from Bevy 0.16.1 with `bevy_enhanced_input` 0.12 to 0.20 — four versions, and a port plus a rewrite. Doing both at once would confuse "action_map is wrong" with "0.20 moved this" |
| **A devfmt usage log, to catch the misses nobody notices** | **hand reflows still happening now that repacking is canonical.** Measured before deferring: 350 loose breaks in tree against 6 pure rewraps in 60 commits, so the aftermath of a devfmt run lives in working-tree churn and not in history — `git log` cannot be mined for it, and devfmt is the only thing positioned to see it. The shape, if it revives: devfmt appends to a gitignored log from the process already being run, costing no approval and no tokens; per paragraph it records a hash of the word sequence and a hash of the physical lines, so a later run finding the same words under different line breaks has caught a miss and can attribute it to its own earlier decision. Worth building only with a mechanical trigger to read it — one line of output when the count crosses a threshold — since a log nobody opens is cost with no signal |
| **Repacking the prose `devfmt` never reached** | **the residue stopping its own shrink.** Prose wrapped before repacking existed and never repacked since; `--diff` repacks whatever a commit touches, so what is left is the paragraphs in files nothing is working on, and a sweep buys less each month. Measured today, files needing a reflow: `src` 21/25, `docs` 6/6, `examples` 22/38, `macros` 1/1, `tests` 2/12. A `--sweep <dir>` is warranted when that stops falling, or ahead of reading a directory end to end. What rides with it: six backtick spans broken across two lines (`Requirements.md`, `Roadmap.md`, `docs/design.md`, `docs/issues.md`, `src/binding/control.rs`, `examples/split_friction/main.rs`), left by the bug 117m fixed. `tools/devfmt/src/main.rs` is swept by hand or not at all — its fixtures are string literals full of `///`, which is the row below. `archive/` is excluded: nothing in flight reasons from it |
| **`devfmt` reading a comment marker inside a string literal** | a second file in tree acquiring one. A `.rs` line whose trimmed text starts with `//` inside a string literal is reflowed as though it were a comment — the module header's "does not occur in idiomatic Rust" assumption, which `devfmt`'s own test fixtures are the sole counter-example to, and they are also the one file a devfmt chunk edits. So the cost today is a hand check on a file already under review, not a corruption nobody sees. Telling the two apart needs a Rust lexer carrying string state, raw strings and `\`-continued literals, which is a different tool from the line classifier this is built on; a second file acquiring one is what changes that arithmetic |
| **A physical binding's label matching the current layout** (R12.2, R12.7) | winit exposing a physical-to-logical query and a layout-change signal, requested as [winit#4606][] and tracked by the broader [winit#2678][], open since February 2023 and unimplemented. A workaround was scoped and set aside: `run_captures` already sees the logical key at capture time, but keeping it means a new field on `ControlCaptured`, a session table `present.rs` consults ahead of the static fallback, and an honest answer on whether it survives a save — which drags in the still-deferred binding-definition serialization (R17.6, R22.16) for a fix that only covers controls a player has personally rebound. A landed query supersedes it outright, for every physical binding rather than only captured ones, so the workaround is not worth building ahead of it |

---

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
[bevy#25592]: https://github.com/bevyengine/bevy/issues/25592
[bevy#25710]: https://github.com/bevyengine/bevy/pull/25710
[winit#4606]: https://github.com/rust-windowing/winit/issues/4606
[winit#2678]: https://github.com/rust-windowing/winit/issues/2678
