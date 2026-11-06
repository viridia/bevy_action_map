# Build plan: `bevy_action_map`

The work that is left, and the gaps that are known. It orders that work into chunks small enough to
review individually — a *sequence*, not a schedule, and any chunk may be reordered once the one
before it has been read.

**What this document admits.** Work not done, and gaps. The test is whether an entry names something
that will change; if it describes the present, it belongs in [docs/design.md](./docs/design.md), and
if it explains why the present is as it is, it belongs in [docs/decisions.md](./docs/decisions.md).
What has been built is described in exactly one place, and this is not it. A landed chunk leaves
this document: its commits carry its number, and [CHANGELOG.md](./CHANGELOG.md) says what it changed
for a game using the crate.

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

What is left, in semantic groups ordered roughly by priority. The order is a guide rather than a
schedule: any chunk may be reordered once the one before it has been read, and a chunk's number is
its identity rather than its position.

## Next

* 191: Accept and back, by the pad's maker
* 172a: `bevy_action_map_ui`, starting with prompts
* 172b: The focus bridge joins `bevy_action_map_ui`
* 115: A timing declared as a tunable
* 122: The wheel as a binding source
* 28: Docs that run
* 205: The orbit editor's saved controls, checked across a restart, after the driver's chunk 33

Chunk 179 carries defects. The register of what is known to be wrong is
[docs/issues.md](./docs/issues.md), and an entry there that acquires a chunk gets a section here.

## Bindings and conditions

What a binding can name, and when it counts as firing. Each of these is a gap a game runs into
rather than a defect in what exists.

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

### 122. The wheel as a binding source, and a zoom for the orbit · E[3]

R13.3: `Control` names mouse buttons and mouse motion and nothing else, so a game wanting the wheel
reads Bevy's own message beside the mapper, and loses the rebinding along with it.

- **Depends on chunk 121**, which supplies the customer. Zoom on an orbit camera is what the wheel
  is for, and the deferred row this replaces was gated on precisely that.
- **The wheel zooms `orbit`'s captured camera, and has no row in its editor**, which stays
  keyboard-only; the binding is declared unlisted.
- **A channel, not a button.** `MouseScrollUnit::{Line, Pixel}` normalized through a configurable
  factor, high-resolution trackpad scroll not quantized on the way through, a `ChannelShape` of its
  own, and the capture, prompt and serialization surface that follows from having one.
- **Review surface:** whether the normalization factor belongs to the app or to the binding. R13.3
  says app-configurable, but a trackpad and a notched wheel want different numbers on the same
  machine and only the binding is that specific — so either the requirement is right and the case is
  rarer than it sounds, or it is a clause to revise.

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

### 191. Accept and back, by the pad's maker · E[3]

A binding names a gamepad button by position, and confirm is not in one position: a Nintendo pad
confirms with A, on `East`, where every other brand confirms on `South`. A menu bound to `South` is
backwards in a Nintendo player's hands, and every game with a menu meets this before any widget
crate does. This chunk lets a binding name the role instead, as `LogicalKey` names a character
rather than a key.

- **Its shape is proposed before it is built**: a `Control` variant naming a role, two roles to
  start, accept and back. A role that means nothing across games, such as "the X button", is not
  one.
- **Resolved per event, from the pad's own `Brand`**, not per binding: a context with no `Paired`
  reads every pad, and two brands can share one game. Resolution is recorded at sampling beside the
  event, as `HeldControlState` records a key's character, so a release finds its press.
- **The answer is fixed in code, with no table a game overrides.** Nintendo resolves accept to
  `East` and back to `South`; every other brand, `Generic` included, the reverse.
- **The Nintendo answer is untested on hardware, and the doc comment says so**, asking the first
  developer who meets a Nintendo pad to report what it does; the author supplies where. It is read
  from SDL_GameControllerDB, as X21 records. Two devices were tried through gilrs on macOS and
  neither delivers input: a clone Switch Pro pad streams its report timer as button presses, and a
  genuine Joy-Con pair connects, resolves as Nintendo and reports nothing.
- **X21 closes**, and its hardware check moves to a new X-entry gated on that first report, or on a
  Linux machine reading a Nintendo pad.
- **What a new control reaches**, counted from `LogicalKey`, which appears in eight files under
  `src/`: binding reading and held state, consumption (claiming the button the role resolved to, so
  a lower context bound to `South` sees a Nintendo player's back as taken), prompts (accept draws
  "A" on either brand), and override serialization, as a name of its own beside `char/`.
- **A preset can name a role; a capture never produces one.** Capture records the physical button,
  for the reason it records a physical key. A preset replaces rows rather than transforming them, so
  a game wanting confirm on `East` for every brand writes `East` into the row, and one with
  positional defaults ships a preset writing the role. A role reaches the player's own file only
  through a preset merged into their working copy.
- **Not single Joy-Cons.** A Joy-Con held sideways rotates its face buttons, and a brand does not
  say how a pad is held. Deferred as an X-entry when this lands.
- **Not the Steam path.** An authority backend hands over actions already mapped.
- **Verified by:** a headless `App` test pressing `South` and `East` on a pad carrying
  `Brand(Nintendo)` and on one carrying `Brand(Xbox)`, `prompt_gallery` drawing accept per brand,
  and `scripts/verify.sh --full`.

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

### 193. Prompts name their context and action without generics · E[2]

A scene names a prompt's context and action through three generic templates (`PromptSpan<C, A>`,
`IconPromptSpan<C, A>`, `IconPrompt<C, A>`). A BSN asset, due in Bevy 0.21, can use a generic type
only once each specialization is registered for reflection: one registration per context, action and
form. `bsn!` already converts a bare type name in a field with `.into()`, and both ids convert from
their types, so the component can be written directly:
`ActionPrompt { context: Menu, action: Close, form: PromptForm::BlockIcon }`. This is the spelling
an asset will use, through X62.

- **The three generic templates and their macro go.** The 21 typed uses become `ActionPrompt`
  literals: in `prompt_ui.rs`'s doc comments, Disasteroids (`main.rs`, `settings.rs`, `ship.rs`),
  Split Friction (`split_screen.rs`), `steam_examples/disasteroids/main.rs` and
  `tests/prompt_ui.rs`.
- **To confirm first:** a bare name compiles in a `bsn!` field, which is read from
  `bevy_scene_macros`' codegen and not yet compiled. A qualified name such as `actions::Fire` may
  need braces.
- **G13 and D85 name the removed templates** as the inline and block types. Once those are gone, the
  inline/block split is a `PromptForm` value on one component. Whether that still satisfies G13 is a
  review question, and the chunk rewrites both entries either way.
- **`ActionPrompt`'s doc comment** shows the new spelling, in place of the templates.
- **Scoped against rc.2's `bsn!`, and true of 0.20.0's**: between the two, `bevy_scene_macros` gains
  only array arguments to a scene function, and field values convert as before.
- **Not loading prompts from an asset**, which is X62. **Not naming a context by path**, which is
  X63.
- **Verified by:** `tests/prompt_ui.rs` spawning a prompt from type names;
  `scripts/verify.sh --full --doc`; `scripts/smoke.sh` on `prompt_gallery`, Disasteroids and Split
  Friction; the author running the Steam build.

### 172a. `bevy_action_map_ui`, starting with prompts · E[3]

`prompt_ui.rs` and `widget_focus.rs` are a layer an input crate cannot own: drawing prompts, and a
`bevy_ui_widgets` bridge for a controls screen. Both are written against the public API and reach
their users by `#[path]`, the Steam build included. They become `crates/bevy_action_map_ui/`,
published beside the base crate, a layer at a time. Prompts come first: neither file uses the other,
and prompts are the half with tests.

- **Depends on chunks 170, 171, 189 and 193**: 170 leaves `prompt_ui.rs` with no path into `assets/`
  and the provider list already in the crate, 171 leaves a workspace for the crate to join, 189
  leaves one prompt component to publish, and 193 leaves it without generic templates, so the shape
  published is the one assets will use.
- **The crate exists**, as scaffolding landed ahead of 170: a manifest, a README, and the license
  files linked from the workspace root into both published crates. Prompts join the provider list
  there. Their public shape is proposed before it is built: what is `pub`, which plugins there are,
  and which of the prompt components and resources keep their names.
- **Ships no art.** The Kenney provider stays in `examples/common/` beside `assets/`, and the Steam
  provider in `steam_examples/`.
- **`tests/prompt_ui.rs` moves into the ui crate** with the file it tests, from the examples'
  package where 170 leaves it, and its `#[path]` goes.
- **Not the focus bridge**, which is 172b: the layer is half moved until then.
- **Not upstreaming it**, which is X48.
- **Verified by:** `scripts/verify.sh --full --doc`, `prompt_gallery`, Disasteroids and Split
  Friction drawing the prompts they drew before, the author running the Steam build, and
  `cargo package --list` for both crates showing the license files and no test reaching outside its
  crate.

### 172b. The focus bridge joins `bevy_action_map_ui` · E[3]

`widget_focus.rs`, the `bevy_ui_widgets` bridge a controls screen is built on, becomes the ui
crate's second module, completing the layer 172a started.

- **Depends on chunk 172a**, for the crate and the shape its public API took, and on chunk 191, so
  the binding recipe it documents names accept rather than `South`.
- **The public shape is proposed before it is built**, as for prompts: what is `pub`, and which
  plugin a game adds.
- **`WidgetKind` is published as the crate's own**, a newtype over a string that Bevy's own id
  replaces once [bevy#25592][] gives one, which is X8. The crate is the bridging crate R22.9 names
  as the one place allowed to know both widgets and mapping.
- **Its docs owe a game the warning** `widget_focus.rs` carries today: `InputDispatchPlugin` in
  `DefaultPlugins` activates a focused `Button` on a key a context has consumed, so a game using
  both disables it.
- **Paths that follow it:** Chunk 200 and X5 name `widget_focus.rs`. R22.7 and R22.8's annotations
  name the file too, and R22.8's calls the focus-kind contexts example-side for want of X8, which
  this makes crate API of the ui crate.
- **Not upstreaming it**, which is X48.
- **Verified by:** `scripts/verify.sh --full --doc`, the controls screens and menus of Disasteroids
  and Split Friction navigating as before, `disasteroids/rebind.py` and `disasteroids/pad.py`
  passing, and the author running the Steam build.

---

### 200. Deleting `acquire_focus_directional` · E[1]

`examples/common/widget_focus.rs` carries a global `AcquireFocus` observer mirroring
`acquire_focus_tab_index`, with `AutoDirectionalNavigation` standing in for `TabIndex`, because
`click_to_focus` cleared focus under directional navigation (bevy#25596). Past bevy#25675,
`InputFocusPlugin` installs an `acquire_focus` observer that stops at the first `Focusable` ancestor
whatever the scheme, and `AutoDirectionalNavigation` requires `Focusable`, so the observer goes.

- **Verified by** the stepper's chevrons, which blinked on every press before the observer and must
  not after it goes, and `disasteroids/pad.py` end to end.

### 201. Deleting `examples/common/font.rs` · E[1]

The plugin overwrites the `default_font` feature's slot at `AssetId::default()` during plugin build,
which depends on that slot's location and on text layout registering a font id once. Past
bevy#25847, a `DefaultFontSource` resource is what `FontSource::Default`, now `TextFont`'s default,
resolves to, and changing it rebuilds the font collection, so the file goes.

- **Verified by** `scripts/smoke.sh` on every example, and a screenshot from a driver plan with the
  examples' font in place.

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
- **Re-simulation leaves live sampling alone.** It replays recorded frames through `FixedMain`, and
  sampling runs in `PreUpdate`, so no new API is needed. That holds only if the Pong base evaluates
  on the fixed tick; if it evaluates on the render tick, gate the `Sample` set with a run condition.

---

## The library itself

Work no game asks for and no published crate can do without: the crate's internals kept consistent,
extension points exercised, and documentation that is true and runs.

### 182. A glossary as the preface to `docs/design.md` · E[1]

The project's own terms, defined once, ahead of the sections that use them: `## 0. Glossary`,
numbered zero so that it moves no other section's anchor.

- **The first entries are the evaluator's vocabulary**, settled in 181e's review: a *reading* is
  *taken* from held state and *recorded* by the pipeline, a *run* is one pass recording one reading,
  a claim *grabs* a control, a control *taken away* while down is *withdrawn*, and "latch" alone is
  the toggle latch, the other being the *require-reset latch*. With them: pending, superseded, the
  closing step, commit, output, stage.
- **Then the rest of the crate's terms**, among them `slot`'s two meanings. The jargon-frequency
  pass, once the author routes it, supplies the candidates.
- **An entry is one line**: the term, what it means, and the `TD` section that owns the detail. A
  word with two senses says so and names both.
- **Not doing:** `docs/architecture.md`'s Vocabulary, which defines the tour's own terms for its
  reader; renaming anything; doc comments.
- **After 181**, whose TD5 rewrite introduces most of the first entries.
- **Verified by** `python3 scripts/xref.py`, and `scripts/show.py` printing the new section.

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

Plans drive the examples with [`bevy-remote-driver`](https://github.com/viridia/bevy_remote_driver),
which began here and is its own project now; chunks 197, 198, 199 and 202 landed there. What is left
here is the mapper's side.

### 149. The mapper's `remote` feature · E[2]

R25: `action_map.dump` and `action_map.authority`, server methods behind a `remote` feature, and a
plan reaching them. `bevy-remote-driver` has no step for an app's own methods yet: its generic
`call` step is deferred in its ledger, gated on this chunk, which ports it.

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
- **Verified by:** a headless test of each handler, and `disasteroids/rebind.py` reaching one
  through `call`.

### 205. The orbit editor's saved controls, checked across a restart · E[1]

`plans/orbit/saved.py` reads back what `editor.py` saved, in a second launch, and fails since each
plan's settings start empty. Once `bevy-remote-driver` can restart the app inside a plan (its chunk
33), `editor.py` restarts after its edits and checks Zoom In's keys and modifier itself, ending on
Reset, and `saved.py` goes. On `main`.

- **Verified by** `orbit/editor.py` end to end.

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
chunk (160a, 160b, …) small enough to finish in one session, and a unit is never left half done. A
unit starts and ends with `scripts/growth.py`, so its effect is measured rather than asserted, and
its section's **Done** line records it.

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

### 186. `xref.py` checks references to issues · E[1]

`xref.py` validates every numbered anchor except an issue's, so a reference to one that has been
routed or dropped survives until someone greps for it. The signature is a four-digit number on a
line naming `issues.md`, or one directly after "issue" or "issues". References read
"`docs/issues.md`, 1074", "`docs/issues.md` 1041" and "issues.md's 1041", and a chunk section's own
"(issue 1034, retired here)", the form five commits have added.

- **Validate the scan first** against a reference known to be good and one planted as dead in each
  form, since a clean result from an unvalidated scan says nothing.
- **Verified by:** that pair, and `scripts/verify.sh` clean on the tree.
- **Not done:** a bare issue number with no `issues.md` beside it, which cannot be told from a year
  or a count.

---

[bevy#25592]: https://github.com/bevyengine/bevy/issues/25592
