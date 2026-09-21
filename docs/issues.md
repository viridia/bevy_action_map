# Findings awaiting triage

The triage queue: things known to be wrong, missing, or out of proportion that the author has not
yet decided what to do about. An entry leaves when that decision is made — given a chunk, a deferred
row in `Roadmap.md`, a decision in `docs/decisions.md`, or dropped — and whatever it knew goes with
it to that destination. Nothing here describes what the crate does, which is `docs/design.md`'s job.

**How to read an entry.** Each says where the problem is, what someone would actually observe, and
whether it was confirmed by running something or only by reading. Many were found by a model asked
to scan `src/`, and some are rules nobody would violate, so that distinction matters; where the
reachable case is hypothetical, the entry says so.

**Line numbers drift.** Take a `file.rs:NNN` as "roughly here"; the symbol named beside it is the
part that stays good. Re-verify before acting on one.

**Numbering.** Each entry's number is a permanent identity from a single counter, independent of its
tier, and never reused. A gap in the sequence is a retired entry.

**Next: 1074.**

**What the tiers mean.**

|                         | Test                                                                                             |
| ----------------------- | ------------------------------------------------------------------------------------------------ |
| **1. Live**             | An ordinary build, an ordinary API call, and the answer is wrong                                 |
| **2. Latent**           | The code is wrong and the path is not taken in tree — a feature exists that nothing has used yet |
| **3. Absent**           | Not wrong, missing, where a requirement or an example's own job says it should exist            |
| **4. Prose**            | A comment or document contradicts the code. No behaviour at stake                                |
| **5. Cost and surface** | Public items nothing asks for, and machinery out of proportion                                   |

---

## 1. Live — an ordinary build gets a wrong answer

### 1070 A hold charges once per event, not once per tick

`eval.rs`, `apply_frame`'s replay loop · **confirmed by a probe**

`apply_frame` runs one fold per level event and hands each fold the whole tick's `delta`, and `Hold`
adds `delta` on every call (`condition.rs`, `BindingCondition::evaluate`). A tick carrying three
level events advances every hold timer by three ticks. Probed: `hold(0.25)` at a `delta` of 0.1,
Space down on one tick, then a tick carrying three unrelated key events: `Firing` after 0.2 s.

A moving stick sends axis events nearly every frame, so a pad hold charges two to three times faster
while the player steers. `Tap`, `HoldAndRelease` and any modifier that integrates `delta` take the
same path.

_Fix, sketched:_ the tick's `delta` to one fold and zero to the rest, which keeps the per-event
replay R9.3 needs. Which fold gets it decides how a press and release inside one tick are timed.
Riding along: `part_value` carries the same `#[cfg]` twice.

### 1071 Require-reset lets a held key through if the binding has a hold

`eval.rs`, `commit_slot`'s require-reset check · **confirmed by a probe**

`commit_slot` holds a `Button` action back while its value reads pressed, and clears the latch the
first time it reads rest. The value it checks is taken after conditions, and a binding whose hold is
still `Building` contributes rest. So the latch clears on the first tick after activation with the
key still down, and the hold charges and fires. Probed: Space held across `deactivate`/`activate`
with `hold(0.25)` gave `Started`, `Building`, `Fired`.

R7.5 fails for every binding with a time condition, through `activate`, `enable` (R3.7) and
`unshadow` alike. `a_context_activating_ignores_a_control_already_held` binds plainly, which is why
it passes.

### 1072 `Started<A>` is public and never triggered

`eval.rs`, `commit_slot`'s edge filter · **confirmed by a probe**

`dispatch_for` maps `ActionPhase::Started` to `Started<A>`, whose doc promises it for "a hold that
has just been pressed", and TD5.6 lists it. `commit_slot` logs only `Fired`, `Completed` and
`Canceled`, so `Started` never reaches the log. Probed: a tick ending in `Started` leaves the log
empty. Nothing in tree observes `Started<A>`; Disasteroids reads the phase instead (`ship.rs`),
while its comment in `actions.rs` says `Started` fires.

### 1073 A claim lifting reads as a fresh press to the context below

`eval.rs`, `fold`'s `is_pressed` · **confirmed by a probe; needs a ruling before a fix**

A consumed control reads as untouched, and nothing records that the reader never saw it go down.
When a claim stops with the key still held (the hold completes or is abandoned, its chord breaks,
the higher context deactivates) the lower context's plain binding on that key fires. Probed: Space
claimed on one tick, unclaimed on the next, never released: `Jump` `Fired`. In play: Shift+Space
held for a vehicle boost, Shift released first, and the on-foot context jumps.

R7.5 and R3.7 give require-reset to activation and to `enable`. Nothing gives it to consumption, and
R8.2 says only that lower contexts "do not see" the control. Whether this is the require-reset case
is the author's call.

---

## 2. Latent — the code is wrong and nothing in tree takes the path

### 1046 A class binding on an analog source has no dead zone

`binding.rs`'s own doc for `bind_class` — "it skips modifiers, conditions and the presentation
mapping list entirely" · reasoned from that doc and from play-testing Split Friction, **not probed
against a false-fire**

Found reaching for `ControlClass::AnyStick` to fix a real papercut: a player picking up a gamepad
and wiggling the stick — the natural first move — has no way to join a game whose join gesture is a
button. `AnyStick` looks like the fix, but a class binding's fold skips the modifier chain (TD8, no
dead zone stage), and `actuated` (`eval.rs`) treats any nonzero axis reading as a match — so a stick
whose rest position sits off true zero, which no calibration step catches before a device is paired,
would fire on its own drift. The failure is silent: a device joins that nobody touched, and nothing
says why.

No requirement asks for stick-triggered joining, and nothing in tree takes this path — Split
Friction's `Join` is two concrete controls (`GamepadButton::South`, `KeyCode::Enter`) and no longer
a class binding at all, so the analog case has no in-tree caller from either direction. The risk is
latent: whoever reaches for `AnyStick` on a class binding next, for a join gesture or anything else
wanting "any analog actuation", hits it with no warning.

A dead zone on the _level_ would not actually close this: a dead zone is centered on assumed zero,
and a stick whose true rest sits at 0.15 reads as "outside it" whether or not anyone is touching the
stick. What the join gesture wants is movement, not position — a game where escaping a grapple means
wiggling the stick rapidly is the same shape of control, and it works on hardware whose zero nobody
calibrated for exactly this reason.

_Fix, sketched:_ not a crate change. `Modifier` (`binding.rs:1256`) is a pure function of a value
and its own `Scratch` — `scratch.prev` holds last tick's position, `scratch.count`/`scratch.time`
can track reversals within a window — so a stateful "wiggle" modifier that outputs `Bool(false)`
until enough movement has accumulated, then passes the real value through, is buildable entirely in
`examples/` today via `.custom()` (`binding.rs:1885`). Free parameters — window length, reversal
count, how much movement counts — are a game's own design question, which is the reason this stays a
worked example rather than a `BindingModifier` variant: baking in an intensity or a pattern would be
guessing at what any particular game's grapple-escape or join gesture actually wants. It rides an
ordinary `.bind::<Join>(Stick::Left)`, since class bindings skip the modifier chain, so a game
wanting both gestures observes `Fired<Join>` for the wiggle beside whatever it already does for the
button. Worth doing once Split Friction wants the polish; not routed to a chunk, since nothing here
is missing from the crate.

---

## 3. Absent — something should exist and nothing does

Ordered by what a real game would miss first.

### 1067 A magnitude modifier on a composite part is silently a no-op

`diagnose` (`plan.rs`), `apply_clamp_magnitude` and `apply_dead_zone` (`binding/modifier.rs`),
`part_value` (`eval.rs`) · **confirmed by running** — written by mistake while porting, caught by a
failing test

`context.bind::<Move>(DirectionalButtons::wasd()).clamp_magnitude()` reads as the obvious way to
stop a diagonal outrunning a straight line. It does nothing at all, and nothing says so.

A composite expands to one binding per part, and `part_value` yields exactly rest or exactly unit —
`Axis2(Vec2::Y)`, `Axis1(1.0)`, never anything between or beyond. So every modifier that acts on
magnitude is the identity on a part:

- `ClampMagnitude` acts only above `length() > 1.0`, which a unit part never is.
- `DeadZone` with rescaling divides the surviving remainder by `1.0 - lower`, which for a magnitude
  of exactly 1.0 returns exactly 1.0 — checked for both `Radial` and `PerAxis`.

The spelling that works is `combined::<Move>().clamp_magnitude()`, acting on the folded value, which
is the only place the 1.41 diagonal exists. `combined`'s own doc says so; nothing warns the person
who did not read it. `diagnose` already refuses a `combined` naming an action with no bindings
(`CombinedWithoutBindings`), so the machinery to report the mirror-image mistake is present.

This is the class of error the diagnostics exist for: no error, no warning, and movement 41% faster
on the diagonal, discovered by feel.

_Fix, sketched:_ a warning when a `BindingInput::Part` carries `DeadZone` or `ClampMagnitude`,
naming `combined` in the message. Warning rather than error, because it is inert rather than wrong
and a game that chains one harmlessly should not fail to boot. `BindingSpec::continues_declaration`
already makes it report once per `bind` call rather than four times.

### 1052 Naming a device to the player has no requirement and no support

`split_screen.rs`'s `device_name` · R11, R18

R18.3 forbids hard-coding English for a control's display string and the crate supplies
`fallback_label` so a game does not have to. A *device* has no equivalent: nothing returns a
descriptor or a key for "Xbox Controller", and R11.6 covers brand *resolution* without saying
anything about naming the result. So Split Friction's pane label hard-codes four English strings off
`GamepadBrand`, which is exactly the shape R18.3 exists to prevent one step to the left.

The gap is in `Requirements.md` first: no requirement covers it, so the crate is not failing one.
Whether a device name is R18's business (a display string, like a control's) or R11's (a fact about
the device, like its brand) is the question to settle before anything is built.

### 1021 A chord has no sequential alternative

R20.3 (SHOULD) · uncited anywhere in tree

R20.3 wants every chord re-expressible as a sequence. Its destination was the sequence condition,
and R6.4 withdrew that, so nothing is built and nothing is planned. What a player would need is
narrower than R6.4's matching models: press the modifier, release it, then press the key. That is
R20.6's sticky modifier (MAY), which is **reviewed and left alone** for want of a case behind it —
so the two requirements stand or fall together, and neither is decided.

The rest of R20 is accounted for: R20.2 and R20.5 are built, R20.1 holds by construction, R20.4 is
withdrawn, and R20.7 is chunk 115.

### 1041 No way to stop the frame sampling itself

R9.9 — a pumped sampling mode. `sample_input`, `begin_sample` and `record` are all public, so the
pieces exist; what is missing is a way to stop `InputFramePlugin` scheduling sampling at all.
Floated as a companion to chunk 83's rewind; chunk 83 says to confirm the need before routing it
there.

### 1047 Capability queries are absent for devices the crate already models

R11.3 (MUST) · `device.rs`

The module's own doc claims "capability data" (`device.rs:5`); nothing answers a capability question
anywhere in the crate — no rumble, motion/gyro, touchpad, battery or LED query, and no way to ask
what controls a device has beyond matching on `DeviceHandle`'s own closed kind. R18's prompts and
any "can this player play at all" check — R11.3's own two named callers — have nothing to call.

Unlike the device model's closedness (D65), this isn't about admitting an unknown device kind — a
gamepad's rumble motors and battery level are things `bevy_input`'s own `Gamepad` component already
reports. Nothing here reads them.

Chunk 117k removed the module doc's claim rather than leaving it promising a MUST with nothing
behind it. Whatever lands this puts it back: `device.rs`'s summary line and its second paragraph
both list what the module holds, and capabilities belong in both once they exist.

### 1048 Virtual devices have no first-class support

R11.8 (SHOULD) · no citation anywhere in `src/`

"On-screen touch sticks, AI/bot drivers, and test fixtures must be first-class devices, not special
cases" — nothing in `DeviceHandle` or the frame models a device that isn't a real keyboard, mouse or
gamepad. A test fixture or bot driver today has to fake `RawEvent`s attributed to
`DeviceHandle::KeyboardMouse` or a real gamepad `Entity`, which is exactly the special case R11.8
asks not to need.

Unlike R11.2 (withdrawn, D65), this doesn't need third-party extensibility — a virtual device can be
modeled inside the crate's own closed set rather than through an escape hatch for hardware nobody's
written. What's missing is a variant and an identity for "not a real piece of hardware," not a
mechanism for hardware this crate has never seen.

### 1025 A context instance cannot be driven from outside the crate

R23.6 · `InputContextState::new` and `apply_frame` are both `pub(crate)`

TD6 says "a test or replay harness can drive one directly." From outside, the only way to get an
instance is to spawn an entity and the only way to advance one is `App::update`. The struct's
freedom from ECS references is real and unreachable, and R23.6's standalone half has no citation
anywhere. The netcode deferred row is where this plausibly already belongs.

### 1027 Two documentation requirements with no document

- R16.4 (SHOULD) — the web caveats: pointer lock and gamepad access needing a user gesture, gamepad
  events being polled, key codes and `vendor_id` being less reliable. What the crate says about the
  web is three comments in `device.rs` noting that `vendor_id` is often absent there, which is one
  clause of one caveat, written where it happened to matter rather than anywhere a reader would look
  for the list. Nothing mentions pointer lock or the user gesture.
- R16.5 (SHOULD) — name the OS-reserved combinations that are unavailable. Nothing.

### 1028 `tracing` is contradicted by a dependency choice recorded only in `Cargo.toml`

R22.3 (SHOULD) wants spans and events at the sampling and firing boundaries. What exists is six
`log::warn!` sites, all app-build or misconfiguration.

The crate depends on `log` rather than `bevy_log`, and the comment in `Cargo.toml` gives the reason:
`bevy_log` installs a `tracing-subscriber` and is `std`-only. That is a decision by
`docs/decisions.md`'s own admission test — name what breaks if reversed, and the answer is R22.3 or
the `no_std` build — and that document does not carry it.

### 1068 The prelude omits both types local multiplayer needs

`prelude` (`lib.rs:371`), `Paired` (`player.rs:33`), `DeviceHandle` (`device.rs:36`) · read, and hit
while writing chunk 137's probes

`use bevy_action_map::prelude::*` gives you neither `Paired` nor `DeviceHandle`, so the minimal
two-player setup needs two further imports reaching into `player` and `device` by hand. The prelude
does export `DeviceFamily` and `ConnectedGamepad`, which are the types a prompt needs, so the
omission reads as an oversight rather than a line drawn somewhere.

Per-player input is one of the crate's headline distinctions in `docs/comparison.md`, and spawning
`(Player, OnFoot, Paired::to(device))` is its whole surface.

_Fix, sketched:_ add both. `Paired` is ungated, and `DeviceHandle` exists in every configuration
since `KeyboardMouse` is unconditional.

### 1069 Landing `touch` breaks every exhaustive match on the device enums

`Control` (`binding/control.rs:434`), `ButtonControl` (`binding/control.rs:26`), `RawEvent`
(`frame.rs:87`), `DeviceFamily` (`device.rs:19`), `DeviceHandle` (`device.rs:36`) · read only

Four public enums in the crate are `#[non_exhaustive]`: `DiagnosticKind`, `OverrideProblemKind`,
`Glyph` and `ActionObstacle`. The other thirty-six are not, and among them are the ones the reserved
`touch` feature exists to grow.

The five named above each gain a variant the day touch lands. Every downstream `match` on any of
them — a glyph catalogue, a rebinding row, a backend deciding what a raw event was — stops compiling
at that point. For a crate meant to become the engine's answer, that is every game that ever named
the vocabulary.

Marking them costs nothing now and is not itself a break; marking them after touch ships means
shipping the break twice. The feature gating already half-forces the discipline, since a `match` on
`Control` cannot be written portably across device-feature configurations today — so the callers who
would be broken are the ones who fixed their feature set and matched exhaustively anyway.

Not blanket advice. `ActionValue`, `ActionPhase` and `ChannelShape` are closed sets the design
argues are complete, and `ActionIntent`'s four are load-bearing in `accepts`. Those should stay
exhaustive.

_Fix, sketched:_ `#[non_exhaustive]` on the five named above, and an entry in `docs/decisions.md`
recording which enums are deliberately closed and on what grounds, so the next person adding one has
a rule rather than a coin flip.

### 1029 Two routing gaps rather than findings

- **Serializing whole binding definitions (R17.6, R22.16) has no destination at all.** The scan
  found it called "deferred" inside a bullet of chunk 17c — no row, no gate, which is what ground
  rule 5 forbids in as many words. 17c has since landed, so that bullet went with its section, and
  what is left is a parenthetical inside the physical-binding-label row saying the serialization is
  "still deferred" while gating something else. Both are MAYs, so the stakes are small and the
  omission is not.
- **The R7.5 opt-out is exercised by a test and nothing else.** `activate_including_held` now has a
  caller — a unit test in `src/eval.rs` — where at the time of the scan it had none. What is still
  missing is any example or production caller: the MUST's "unless explicitly opted in" clause is
  proven correct in isolation but has never been asked for by a game in tree.

---

## 4. Prose — a comment or document contradicts the code

No behaviour at stake. All are small, and the reason to do them together is that a reader trusting
any one of them is misled about a mechanism.

### 1031 Examples and sketches that do not compile

- TD3's trait sketch says `// plus CATEGORY and CONSUME, with defaults`. The constant is `CONSUMES`.
  Copying the sketch into a hand-written impl does not compile.

### 1032 Internal comments whose stated reason is false

- `frame.rs:340` credits calibration's placement with meeting R14.10. R14.10 governs an authority
  backend, which per D51 enters at the button state machine and never touches the frame. The
  placement is right and the `R`-number is wrong; it is the crate's only claim on R14.10. **Small —
  worth a minute if something else is open in this file, not worth a pass of its own.**
- `action.rs:335`, `Scratch::flags` is documented as "Condition-defined bits" and a modifier defines
  one too (`TOGGLE_LATCH`, `binding.rs:2311`). Related, and worth carrying with it: `condition.rs`'s
  own constant does not carry the note `binding.rs`'s does, explaining why two constants in two
  files can both be `1 << 0` — `plan.rs:691` gives every modifier and every condition its own cell.

### 1033 Design sentences that are a clause short

- **TD4 says "only the scratch is rebuilt."** `Plan::compile` rebuilds `indexed_controls` and
  `has_chords` as well, and `plan.rs`'s own comment says the first is required rather than
  incidental: an override rewrites which controls a binding reads, so one has to move between
  indexed and not. The code is right and the sentence is short.
- **TD5.3 says the exclusion ceiling "is set by the `PreUpdate` pass and read — never rewritten — by
  every `FixedPreUpdate` run."** `evaluate_context` raises it for any `C::EXCLUSIVE` with an active
  instance in whichever schedule it runs, which is what makes a `Fixed` exclusive context work at
  all. The consequence the sentence hides, verified: a fixed exclusive context shadows lower-
  priority _fixed_ contexts and never a render one, in that frame or the next, because the reset
  runs at the top of `PreUpdate`. TD5.2 states this for consumption and nothing states it for
  exclusion.
- **TD2 says "Order is preserved."** Within one device stream it is. Across streams it is not:
  `sample_input` records every key event, then focus loss, mouse buttons, motion and gamepad, so a
  click and a key press in one frame always arrive key first. It matters to anything comparing
  arrival order across families, chunk 135's chord among them.
- **TD7 does not state R7.3's cost.** Two simultaneously-active layers hold separate action state,
  so a game reading `ContextActions<Base>` does not see the layer's answer and has to read both.
  R7.3 is a MUST that is met and claimed nowhere.
- **A `TD10.1` citation points at the wrong section**, at `context/declare.rs:761`. It is about the
  override store being keyed by mapping alone, which is TD10's preamble; TD10.1 is "Applying," and
  this is not about applying. Two others, both in `overrides.rs` and both about the serialized form
  (now TD10.3), went with 117e. **Small — worth a minute alongside the R14.10 mis-citation above,
  not worth a pass of its own.**

### 1055 Split Friction teaches a disputed claim about somebody else's crate

`split_screen.rs`'s module doc tells a reader that three `Camera2d`s sharing one window hit "what
looks like a Bevy rendering bug", where only the last camera's `ClearColorConfig` is honored and it
clears the entire window rather than its own `Viewport`
([bevyengine/bevy#25608](https://github.com/bevyengine/bevy/issues/25608)). Whether it is a bug is
arguable: it follows from how a wgpu render pass clears. So an example is teaching a contested claim
about another crate in order to explain an arrangement it abandoned.

Two ways out, and they exclude each other. Either the example learns to live with the clear
behaviour, so a third camera works and the paragraph has no subject left; or the arrangement stands
and the explanation comes out of the comment, since what a reader needs is that each pane's UI is
`UiTargetCamera`'d at its own pane's camera, not why the alternative was dropped. The second is
cheap and the first is the one that would make the example teach more.

---

## 5. Cost and surface

### 1034 One prompt lookup walks every declared context

`present.rs`, `BindingTable::prompts`, calling each declared context's own `bindings` fn fresh per
context per call — the scan cited this as `read_bindings::<C>`, which the call now reaches
indirectly through `DeclaredContexts`

`prompts` rebuilds every declared context's binding list on every call — two fresh vectors per
context, every binding and every part — then does an O(n²) scan for earlier claims and an O(n²)
dedup. The cost does not depend on which action was asked for, so it is paid in full per call:
`examples/common/prompt_ui.rs` calls it once per span, so twenty spans over six contexts rebuild a
hundred and twenty context binding lists.

`PromptGeneration` bounds how _often_ this runs and nothing bounds what one pass costs.
`BindingTable` holds the world for exactly the lifetime an amortization would want. Not on the
per-tick path, so R23.2 does not apply.

### 1035 R23.2 is unenforced, and reading is the only thing enforcing it

No allocation and no synchronization on the per-tick path is a rule with no tooling behind it. Four
violations have reached that path and every one was caught by reading. Two were fixed; two are still
there:

- `evaluate_context` builds `let mut claims = Vec::new()` per instance per tick (`eval.rs`, in the
  loop that calls `apply_frame`) and allocates the moment anything is claimed. `chord_claims` sits
  on `InputContextState` and cites R23.2 in its comment for exactly this reason, and
  `dispatch_transitions` takes and hands back its log to keep the allocation — so both idioms are
  established in the same file and `claims` follows neither.
- `controls()` allocates a fresh `Vec<Control>` and is called per consuming binding per tick, in the
  same loop, to fill that vector.

This entry once recorded the second as gone, on a grep for `binding.source.controls()` that missed
it: the field was renamed to `input`, not removed. Which is the finding. Violations keep being found
by reading, and then a reading finds them absent with equal confidence — "a rule with no tooling
behind it," as the register puts it, does not only fail to prevent them.

### 1053 The release itself has no destination

`Roadmap.md` · `Cargo.toml`

Chunk 119 moved every Bevy dependency to crates.io at `0.20.0-rc.1`, which removes the mechanical
barrier this finding was filed against: crates.io rejected the git dependencies outright, and it
accepts an rc. What stops a publish now is a judgement — that nothing ships while the dependency is
a release candidate — and 0.20.0 has no stable release yet.

That makes the finding sharper rather than smaller. The thing that was guaranteeing nobody published
by accident is gone, and `Roadmap.md` still has no release chunk, no deferred row gated on the 0.20
release, and no checklist of what must be true before the first publish — which by ground rule 5 is
an item that will be dropped. The work that is release-shaped is scattered through chunks whose
descriptions do not mention it: 42's backend trait is "cheap now, breaking later" and does not say
that is a publishing deadline rather than a preference.

_Fix:_ a deferred row gated on the Bevy 0.20 release would be the smallest thing that stops this
being forgotten. What belongs in it is the ordering question rather than the date.

### 1054 Deriving a pane's persistent identity is boilerplate every game rewrites

`reconnect.rs`'s `remember_identity` and `KnownDevice`

A game that saves pairings needs "which persistent `DeviceId` is this entity paired to". The crate
supplies both halves — `Paired` names the runtime handles, `Identity` sits on the device entity —
and joins neither, so the app writes the join: an `Insert<Paired>` observer, an `owner_for` call per
family, a special case for the keyboard's constant identity, and a component to put the answer in.
About twenty lines with exactly one correct implementation, which is the signal.

`saved_pairings.rs` then watches `KnownDevice` rather than `Paired` precisely because it is the
component that exists only once the identity is actually known — so the derived component is not
incidental, it is what the persistence layer is built on.

**Reviewed and left alone, in the same area**: the examples coordinate context priorities by hand
and by comment — `PopupMenu` at 10 "matching Disasteroids' `Menu`", `ButtonFocused` at 20 so a
focused button outranks both. Named bands would be cheap, but two examples agreeing is not yet
evidence of a convention worth fixing in the crate, and a game with a different layering would want
different numbers.

---

## Checked and correct

Recorded so nobody re-derives it. Every one of these was read against the code and holds.

**Values and shapes.** R2.2's conversion table matches `to_bool`/`to_axis1`/`to_axis2`/`to_axis3`
cell for cell, including the two rows the requirement expects an argument about. R2.10's two
hardware cases hold in `ActionIntent::accepts`. R1.1's declared path is required by the derive with
no default. TD4's fourteen `DiagnosticKind` variants and both `Severity` variants match exactly.

**The frame.** R9.1–R9.5 and R9.7, including the two worth doubting: deltas are summed rather than
replaced (`eval.rs:347`, asserted at `eval.rs:1131`), and events are replayed singly rather than
folded once, which is what keeps a press-and-release inside one window from cancelling. The queue's
append-monotonic invariant survives `clear()`. R11.4's hot-plug policy holds at `eval.rs:422`.
`retire_read_events`, the per-instance cursor and the `level_changes == 0` top-up together give R9.3
and R9.4, confirmed from both the frame's side and the evaluator's.

**Bindings.** R4.1a's three placements for a mouse button all hold. R4.2's five composites exist.
R4.7's two opposite defaults are both right. R5.1, R5.2, R5.3, R5.5, R5.7 and R5.8 hold. R5.4's
remainder is the netcode row's. D20's stage-1 no-rescale rule holds, clamp included. R12.5 holds by
construction rather than by a filter: keyboard actuation is a set insert/remove keyed on `state`, so
a `repeat: true` event moves nothing. R6.7 holds by construction — a condition's only clock is the
`delta` it is handed. The three enums over one control space were read for proportion and judged
proportionate.

**Evaluation and storage.** R23.5's O(1) holds with no hash on that path. R23.3 holds: activation
sets a flag and fills a vector. R23.7 holds by construction. R7.4's cancellation matches `Fired |
Ongoing`, with the `Started` gap already in `Roadmap.md`. R8.1's chord pre-pass is a pure function
of held state and `is_pressed` refuses a consumed control inside it. R10.2 holds outright, now that
chunk 99 has landed the read path its exception named. The
three `Fold` kinds partition correctly. R7.5's default half holds for a newly spawned context by the
empty held-state map rather than by the require-reset latch. R24.4's app-build / runtime split is
honoured at both panics.

**Overrides and presentation.** R17.1 holds by construction, which is also D47. R17.7's three states
round-trip and the two bare words cannot collide with a control name. R17.8 holds by construction.
R17.2's tolerance holds on both axes. R19.4's four resets exist. R19.16 holds in both directions.
R19.9 holds at declaration — it is only the rewrite that lowers it, and that finding has landed. The
tunable pass runs after the control rewrite and matches family as well as key. R18.1's context sort
is stable, so declaration order survives as the last tiebreak. R18.5's invalidation covers every
clause but the layout one its own aside withdraws. R18.8 and R18.9's origin half hold. The four
control tables round-trip exhaustively, unnamed variants included. R22.6's migration path exists in
`docs/comparison.md`. R21.1–R21.3 are met by the test suite's shape. Capture's arming skips the
press that opened the session, and a refused press is claimed so it does not also play the game.
`admissible` asks family before reserved before shape. R15.1's many-to-many holds — neither `Paired`
nor `is_claimed` enforces exclusivity, which is what lets two players share one keyboard.

**Excluded rather than missed**, both already recorded: `apply_overrides_for` discards the rewritten
rows, which is the per-entity presentation deferred row; and `Override::NotOurs` leaves the crate's
binding live rather than silencing it, which is R0.6 and chunk 112's.

**Plausible and wrong**, from a later scan that read these as defects. `chord_claims` on
`InputContextState` is written every fold: `fold` destructures `self`, so the name there is the
field, and `why_not_id` reads what the last fold left. `rewrite`'s `preset_authorized` is the only
thing that lets a preset move a `Fixed` row; no enclosing `is_rebindable` check makes it dead. The
per-entity `NoSuchMapping` report can fire, since `MappingKey::new` names any path. And
`ControlOrigin::Foreign` has no constructor in the crate because an outside `Prompts` implementation
is what builds it.
