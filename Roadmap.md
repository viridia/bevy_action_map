# Build plan: `bevy_action_map`

The work that is left, and the gaps that are known. It orders that work into chunks small enough to
review individually — a *sequence*, not a schedule, and any chunk may be reordered once the one
before it has been read.

**What this document admits.** Work not done, and gaps. The test is whether an entry names something
that will change; if it describes the present, it belongs in [docs/design.md](./docs/design.md), and
if it explains why the present is as it is, it belongs in
[docs/decisions.md](./docs/decisions.md). What has been built is described in exactly one place, and
this is not it. The landed index below names chunks; it does not describe the crate.

Ground rules, house style and the commit-message convention are in [CLAUDE.md](./CLAUDE.md); they
are about the work rather than about the crate. Ground rule 5 is the one this document is built
around: nothing outstanding may be left without a destination, which is a chunk here or a gated
entry in [docs/deferred.md](./docs/deferred.md).

---

## Where this stands

[docs/design.md](./docs/design.md) is what the crate does today, and
[docs/issues.md](./docs/issues.md) is what is known to be wrong and not yet routed, and
[docs/deferred.md](./docs/deferred.md) is what was decided against for now, each with the gate that
reopens it. The chunks below are the rest of the delta.

The target the remaining sequence aims at is **Disasteroids** — an asteroids-like game playable on
keyboard or gamepad, with a rebinding screen built on `bevy_ui_widgets` and operable from the
controller. It is not a phase of its own; it arrives early, badly, and grows a capability per chunk,
because ground rule 3 wants something runnable at every step and a real game is a better acceptance
test than a synthetic one.

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
| 151c | Prompts from Steam's origins                                          |
| 158  | Reserving an authority is an error                                    |
| 164  | Child crates under `crates/`                                          |
| 171  | The root crate moves under `crates/`                                  |
| 163  | Local multiplayer's types in the prelude                              |
| 144  | One rule for what counts as a character                               |
| 143  | One apply, for the world or for an entity                             |
| 162  | A warning for a modifier idle on a composite part                     |
| 72b  | Calibration the app sets, and measuring withdrawn                     |
| 174  | A section, printed by its anchor                                      |
| 165  | `gamepad/`, and the two pieces that exist                             |
| 167  | Rumble as a component                                                 |
| 168  | The Steam build fills them                                            |
| 169  | The design proposal                                                   |
| 175  | Every mention of a name, in one call                                  |
| 176  | A dependency's source, at the locked version                          |
| 156  | Navigating from no focus                                              |

---

What is left, in semantic groups ordered roughly by priority. The order is a guide rather than a
schedule: any chunk may be reordered once the one before it has been read, and a chunk's number is
its identity rather than its position.

## Next

* 115: A timing declared as a tunable
* 122: The wheel as a binding source
* 28: Docs that run
* 33: Conditions that read other actions

No chunk currently carries a defect. The register of what is known to be wrong is
[docs/issues.md](./docs/issues.md), and an entry there that acquires a chunk gets a section here.

## Bindings and conditions

What a binding can name, and when it counts as firing. Each of these is a gap a game runs into
rather than a defect in what exists.

### 129b. A screen that sets a modifier · E[3]

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

### 135. A chord across two device families · E[2]

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

### 121. A camera that takes the mouse, and gives it back · E[3]

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

### 122. The wheel as a binding source, and a zoom for the orbit · E[3]

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

### 33. Conditions that read other actions · E[3]

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

### 115. A timing declared as a tunable · E[2]

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
  direction of forgiveness — the half of R20.4 that was coherent — and it is deferred with its own
  gate as X19 rather than riding along here.
- **Verified by:** Disasteroids' settings screen offering one timing beside the dead-zone slider it
  already has, and the changed value still applied after a quit and relaunch.

---

## Presentation and prompts

What the player is shown, once the crate knows what is bound.

### 73. A key rendered through a catalogue · E[2]

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

### 170. Prompt art from an ordered list of providers · E[2]

`prompt_ui.rs` has Kenney's file layout built in (`IconManifest`, `tier_str`, `icon_path`), and a
backend's art reaches it only through `ExternalArt`: one slot, consulted for `Glyph::External`
alone. Kenney is the base and Steam an override that can only fill Kenney's gaps. Both become
providers, and neither is special.

- **What it is:** a resource in `prompt_ui.rs` holding an ordered list of providers, each a function
  from a `Glyph` and whether the prompt is a block one to `Option<AssetPath>`. The first answer
  wins, and the order the plugins are added in is the priority. `ExternalArt` goes.
- **Coverage is the same question.** The `has_art` closure given to `resolve_glyph` becomes "does
  any provider answer for this `Glyph::Own`", so the brand-to-generic fallback follows the art that
  is installed rather than Kenney's manifest.
- **The Kenney provider** is a module of its own in `examples/common/`, taking the manifest, the
  tier directories and the `macos/` preference with it. `prompt_ui.rs` ends with no path into
  `assets/`.
- **The Steam provider** is `steam_examples/disasteroids/glyphs.rs`, registered ahead of Kenney:
  Steam's art for the pad, Kenney's for the keys.
- **With no provider**, every prompt is text, as a control without art is today.
- **Not the crate.** `crates/bevy_action_map/src/` is untouched: `resolve_glyph` already takes
  coverage as a closure.
- **Not `PromptSource`**, which decides which controls a prompt names. This decides how they are
  drawn.
- **Not the packaging**, which is chunk 172a.
- **Verified by:** `crates/bevy_action_map/tests/prompt_ui.rs`, plus a test that the first provider
  to answer wins; `prompt_gallery` and Disasteroids drawing the art they drew before;
  `scripts/verify.sh --full` for the Steam build, and the author running it to see Steam's pad
  glyphs beside Kenney's keys.
- **On landing:** that the presentation layer ships no art, and takes it from sibling providers, is
  an entry in `docs/decisions.md`.

### 172a. `bevy_action_map_ui`, starting with prompts · E[3]

`prompt_ui.rs` and `widget_focus.rs` are the layer door 3 of `docs/one-way-doors.md` says an input
crate cannot own: drawing prompts, and a `bevy_ui_widgets` bridge for a controls screen. Both are
written against the public API and reach their users by `#[path]`, the Steam build included. They
become `crates/bevy_action_map_ui/`, published beside the base crate, a layer at a time. Prompts
come first: neither file uses the other, and prompts are the half with tests.

- **Depends on chunks 170 and 171**: 170 leaves `prompt_ui.rs` with no path into `assets/`, and 171
  leaves a workspace for the crate to join.
- **The crate is created here**, with prompts as its first module. Its public shape is proposed
  before it is built: what is `pub`, which plugins there are, and which of the prompt components and
  resources keep their names.
- **Ships no art.** The Kenney provider stays in `examples/common/` beside `assets/`, and the Steam
  provider in `steam_examples/`.
- **`crates/bevy_action_map/tests/prompt_ui.rs` moves into the ui crate**, and its `#[path]` goes.
  Until it does, the base crate's package ships a test that cannot build from the package, since the
  file it includes is outside it.
- **Both published crates carry the license files.** The base stopped shipping `LICENSE-MIT` and
  `LICENSE-APACHE` when it moved under `crates/`, and the ui crate starts without them; whether each
  crate gets copies or links is settled here.
- **Not the focus bridge**, which is 172b, and with it door 3's path: the layer is half moved until
  then.
- **Not upstreaming it**, which is X48.
- **Verified by:** `scripts/verify.sh --full --doc`, `prompt_gallery`, Disasteroids and Split
  Friction drawing the prompts they drew before, the author running the Steam build, and
  `cargo package --list` for both crates showing the license files and no test reaching outside its
  crate.

### 172b. The focus bridge joins `bevy_action_map_ui` · E[3]

`widget_focus.rs`, the `bevy_ui_widgets` bridge a controls screen is built on, becomes the ui
crate's second module, completing the layer 172a started.

- **Depends on chunk 172a**, for the crate and the shape its public API took.
- **The public shape is proposed before it is built**, as for prompts: what is `pub`, and which
  plugin a game adds.
- **`WidgetKind` is published as the crate's own**, a newtype over a string that Bevy's own id
  replaces once [bevy#25592][] gives one, which is X8. The crate is the bridging crate R22.9 names
  as the one place allowed to know both widgets and mapping.
- **Its docs owe a game the warning** `widget_focus.rs` carries today: `InputDispatchPlugin` in
  `DefaultPlugins` activates a focused `Button` on a key a context has consumed, so a game using
  both disables it.
- **Paths that follow it:** X2 and X5 name `widget_focus.rs`, and door 3 names `examples/common/` as
  where the layer sits. R22.7 and R22.8's annotations name the file too, and R22.8's calls the
  focus-kind contexts example-side for want of X8, which this makes crate API of the ui crate.
- **Not upstreaming it**, which is X48.
- **Verified by:** `scripts/verify.sh --full --doc`, the controls screens and menus of Disasteroids
  and Split Friction navigating as before, `disasteroids/rebind.py` and `disasteroids/pad.py`
  passing, and the author running the Steam build.

---

## Snapshots

Taking a context's state out and putting it back, in memory between ticks. The other half of this
section was persistence — the same move, to a file between runs — and chunks 92 and 92b landed it.

### 83. Rewind, without the network · E[4]

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

### 112. A backend suppresses a device family at L0 · E[2]

R0.6's other half, and the smallest it will ever be: `docs/steam.md` S1 and S3 rule out suppressing
one device, so what is left is a family switch (D93).

- **A resource naming the suppressed families**, read by `sample_input` (`frame.rs:304`) as a guard
  around each family's own `MessageReader` loop. Nothing above L1 changes, because a suppressed
  family simply never reaches the frame.
- **Declared by the game, not detected**, because a suppression that turns itself on and off between
  runs is the worst kind of input bug to diagnose. The backend's own plugin sets it once its init
  succeeds.
- **Replay is what justifies this, not Steam.** A replay backend mutes live hardware in a build that
  compiled the driver in (R0.6, R10.8), which no build configuration expresses. A Steam build can
  instead take `bevy/gamepad` without `bevy_gilrs` and produce no hardware event to suppress, which
  is why D93 rests on replay.
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

### 28. Docs that run · E[4]

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
- **`crates/bevy_action_map/src/lib.rs`'s crate-level docs, alongside it.** The `//!` block largely
  mirrors the README's Concepts section and has drifted the same way — both were drafted early and
  neither has kept pace with what the crate grew into since.
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

It is developed here because that is faster, as a workspace member (`crates/bevy_remote_driver/`)
with its own documents, so what it needs travels with it if it leaves; whether it does is X45's.
Which of its server methods are proposed upstream to Bevy is a separate question, and DD8's.

### 149. The mapper's `remote` feature · E[2]

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

- **Not doing:** a step of the mapper's own, or a Python helper over `call`. Deferred as X32.
- **Not doing:** raw injection, which R25.4 says needs nothing.
- **Verified by:** a headless test of each handler, and 148's plan reaching one through `call`.

## A Steam demo

The authority seam has had one customer, `pong_robot`, whose authority is a robot. Several things
are unbuildable or unverifiable without a real backend: a delegated rebind, glyphs a backend
supplies, and prompt invalidation the backend drives. A Steam build of Disasteroids, then of Split
Friction, is the real backend. It lives in `steam_examples/`, beside `steam_probe/`, with its own
`Cargo.toml` outside the workspace so `steamworks-sys` never builds in `verify.sh`.

Every chunk here is an audit rather than a gate. It needs a running client, a pad, and a layout
bound by hand (`docs/steam.md` S16), and no CI can run it.

### 151e. Split Friction on Steam · E[3]

Two players are two `InputHandle_t`s, not two action sets (`docs/steam.md`'s appendix).

- **One set, activated per controller**, each handle's entity `Paired` to its protagonist; join is a
  declared action prompted by name.
- **The backend writes `RawGamepadEvent::Connection`** when a handle appears or goes, since nothing
  else will (D73), and an action held on a vanished pad releases (R11.4).
- **Chunk 116's pool runs on Steam's entities**, which is the test of "backend-neutral".
- **Brand is Disasteroids' `brand`**, which maps `InputType` onto `GamepadBrand`, shared rather than
  copied.
- **Measured here:** whether a handle survives a client restart, which settles D74's claim either
  way. A relaunch of the game it survives (S32).
- **Measured here:** how Steam orders its connected pads with two awake, whether that order is
  stable, and which pad `show_binding_panel` opens for. 151f's flight test saw Steam hesitate to
  switch between the PS5 and Xbox layouts with both awake, cause unknown.
- **Not doing: R15.9.** Steam has one account per machine, not one per controller.
- **Verified by:** two pads joining, walking, and one unplugged mid-game with its held input
  released.

## Sweeps

Each of these passes over a whole document or module. A sweep lands a unit at a time, as a lettered
chunk (159a, 159b, …) small enough to finish in one session, and a unit is never left half done. A
unit starts and ends with `scripts/growth.py`, so its effect is measured rather than asserted, and
its section's **Done** line records it.

### 159. Compressing `docs/decisions.md` · E[1]

The preamble's "What an entry keeps", applied to every entry: each keeps what going back would cost
and the facts that make a rejected alternative worse, and loses the argument for a choice between
equals and the story of how a revised entry got where it is.

- **A unit is one topical section**, or ten entries where a section is longer.
- **An entry holding two decisions is split**, and every citation of it is re-pointed in the same
  unit. A fact about an external system moves to `docs/steam.md` if it is not already there.
- **Not doing:** changing a decision. An entry whose reasons no longer hold goes to
  `docs/issues.md`, not into a rewrite.
- **Verified by:** `scripts/xref.py`, and the unit's `growth.py` numbers.
- **Done:** D22, the sample, which halved and became D22 and D93.

### 160. An editorial pass on `docs/design.md` · E[1]

For order and clarity, not length: TD9.1, the sample, lost a tenth of its words. Most of the
document was written by an earlier model and has not been edited as a whole.

- **A unit is one section, and makes three checks:** the order follows a reader; every claim the
  pass rewords is checked against the source; and every name and term is one this reader has already
  met, with the layer named where two could answer. TD9.1 needed all three: its "binding list" meant
  the bindings `rewrite` produces, beside prose about the override's list.
- **Largest sections first**, from `growth.py`.
- **Not doing:** changing the mechanism. A claim the source contradicts goes to `docs/issues.md`.
- **Verified by:** `scripts/xref.py`, and the unit's `growth.py` numbers.
- **Done:** TD9.1.

### 161. Public items nothing outside `crates/bevy_action_map/src/` names · E[1]

`growth.py --api` lists public items that no example or integration test names; the ones outside the
prelude are the candidates, 48 at the first run.

- **Each gets one of three answers:** made `pub(crate)`, where it is public only because its module
  is (`Scratch`, `run_captures` and `sample_input` look like that); kept, with an example or test
  made to use it, where it is API nothing has exercised; or left alone, where it is reached without
  being named, as a builder a closure receives or an extension trait the prelude brings in.
- **A unit is one module**, and folds in any tier-5 finding in `docs/issues.md` about the same
  items.
- **Narrowing visibility breaks the public API**, which costs nothing until the first publish.
- **Verified by:** `scripts/verify.sh`, since this one changes code, and the examples do not change.

## Tooling

Tools the project runs on itself, under `tools/` and `scripts/`, rather than anything a game uses.

### 173. A reflow tool that parses what it reflows · E[3]

devfmt classifies lines by guessing: a comment is any line starting with `//`, and a block's kind is
read off its first characters. A new tool beside it finds comments with a Rust lexer and block
boundaries with `pulldown-cmark`, and keeps devfmt's rewrap and `--diff` scoping unchanged.

- **devfmt stays**, and is the oracle: both tools run `--sweep` over the tree and every divergence
  is reviewed as a bug in one or the other. devfmt's tests are the new tool's starting suite.
- **The cutover** is `CLAUDE.md` naming the new tool as the authority on width, once the divergences
  are all devfmt's.
- **Verified by:** the divergence list, reviewed to empty or to devfmt's bugs alone, and a test for
  each block type devfmt misreads: an indented code block, an HTML block, a fence inside a list
  item, and a string literal starting with `///`.
- **Not done:** retiring devfmt, or rendering Markdown through the parser, which would restyle every
  document.

### 177. An example, launched and checked · E[1]

`scripts/smoke.sh <example>` is the launch `CLAUDE.md` asks of every example a chunk touches: run it
for a minute and fail if the log has `panicked` or `ERROR`. It knows each example's features
(Disasteroids needs `serialize`) and the warnings known to be harmless (Split Friction's
`mesh2d::bindings` import, bevy#25936), so a clean run prints one line.

- **It bounds the run itself**, since macOS has no `timeout`: one missing exits 127 before the
  example starts, and an empty log reads as a clean one.
- **`CLAUDE.md`'s launch paragraph** names the script, and the known warnings move into it, where a
  Bevy bump that fixes one shows up as a stale entry.
- **Verified by:** a clean run of each example, and a run with a deliberate panic in a scratch
  example reported as failed.
- **Not done:** driving the example, which is the remote driver's job, or catching a wrong answer
  that neither panics nor logs.

---

[bevy#19741]: https://github.com/bevyengine/bevy/issues/19741
[bevy#25592]: https://github.com/bevyengine/bevy/issues/25592
