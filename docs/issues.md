# Findings: the implementation scan

What the six-session scan of `src/` turned up, reordered by how much it matters rather than by
which session found it.

**How to read an entry.** Each says where the problem is, what someone would actually observe, and
whether it was confirmed by running something or only by reading. That last distinction is the
important one, and it is stated per entry rather than assumed.

**Line numbers are as of the scan, and several have since drifted** — `eval.rs:247` is now 257, and
the two `cargo doc` sites in 1038 have moved and been fixed. Take a `file.rs:NNN` as "roughly here,
find it by name"; the symbol named beside it is the part that is still good. Re-verify before acting
on one.

**Numbering.** Each entry's number is a flat, permanent identity, assigned once from a single
counter and independent of which tier it sits in — the tier is where it's filed today, not what it
is. A number is never reused: once an entry is fixed and its chunk has landed, the entry is retired
from this document rather than kept as a silent record, and its number simply does not reappear. The
chunk's own commit, and `docs/design.md` or `docs/decisions.md` where anything about it was durable,
are the record. A gap in the sequence below is a retired finding, not an omission. The next
unassigned number is stated here; keep it up to date when numbering new items.

**Next: 1056.**

**The calibration warning, stated up front because it is fair.** Ask a model to find sixty problems
and it will find sixty. Some of what follows is real and some is a rule nobody would ever violate.
The tiers below are the honest attempt to separate them. Where a finding rests on reasoning rather
than on a probe, or where the reachable case is hypothetical, the entry says so in as many words.
Where an entry judges a small cost not worth acting on, that verdict is stated inline rather than
collected into a tier of its own.

**What the tiers mean.**

|                         | Test                                                                                             |
| ----------------------- | ------------------------------------------------------------------------------------------------ |
| **1. Live**             | An ordinary build, an ordinary API call, and the answer is wrong                                 |
| **2. Latent**           | The code is wrong and the path is not taken in tree — a feature exists that nothing has used yet |
| **3. Absent**           | Not wrong, missing, with a requirement saying it must exist                                      |
| **4. Prose**            | A comment or document contradicts the code. No behaviour at stake                                |
| **5. Cost and surface** | Public items nothing asks for, and machinery out of proportion                                   |

This document does not _decide_ routing, and it does not describe what the crate does — routing is
ground rule 5's business and yours, and `docs/design.md` is the description. It does record routing
once it happens: an entry that has been given a chunk says so on its `Fix` line, so what is left
unrouted can be read off the entries that stay silent.

---

## 1. Live — an ordinary build gets a wrong answer

Every entry this scan filed here has landed (chunks 81, 84, 85, 86, 88, 89, 90, 91) and is retired.

---

## 2. Latent — the code is wrong and nothing in tree takes the path

### 1010 Clearing one part of a composite empties the other three

`overrides.rs:941` · **verified** against a headless `App`: four empty rows, no problem reported

`rewrite` drops a whole _binding_ per slot the row no longer has, and a composite's four rows are
four _parts of one binding_. So `Override::Cleared` on `move.up` takes the binding away and
`move.down`, `.left` and `.right` come back holding nothing.

Nothing in tree produces a `Cleared` row — `settings.rs:1218` is the only site that reads one — but
the state is R17.7's and first-class, and `Overrides::bind` with an empty list is the door. A
settings screen with an "unbind" button is the obvious way in.

`CompositeCannotGrow` refuses the mirror case in the same pass, so the refusal list already knows
the shape and covers only the growing half.

_Fix:_ a chunk rather than an edit. Dropping per part instead of per binding and refusing the clear
outright are both defensible, and which is right is a design question about what clearing one arrow
of a movement composite means. Unrouted.

### 1046 A class binding on an analog source has no dead zone

`binding.rs`'s own doc for `bind_class` — "it skips modifiers, conditions and the presentation
mapping list entirely" · reasoned from that doc and from play-testing Split Friction, **not probed
against a false-fire**

Found reaching for `ControlClass::AnyStick` to fix a real papercut: a player picking up a gamepad
and wiggling the stick — the natural first move — has no way to join a game whose join gesture is a
button. `AnyStick` looks like the fix, but a class binding's fold skips the modifier chain (§8, no
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

### 1049 The documented join recipe races two players for one slot

`join.rs`'s module doc · **verified by reading**, and by the workaround Split Friction already
carries

The sketch in `join.rs` teaches: check `is_claimed`, return if the device is taken, otherwise "pick
a slot and insert `Paired::to(device)` on it". Two devices pressing join on the same tick both pass
`is_claimed` — correctly, they are different devices — and then both pick the same slot, because
the first one's `Paired` insert is a deferred command that has not applied when the second observer
runs. A game following the documented path hands both players protagonist 0.

`is_claimed` is not wrong; it answers the question it is named for. What is missing is that the
sketch's second half needs state the query cannot see yet, and nothing says so.

Split Friction hit this and worked around it: `protagonist.rs`'s `ClaimedDevices` is a resource
updated synchronously inside the observer, and its doc comment explains the race in full. So the
crate's flagship multiplayer example does not use the crate's documented join recipe, for a reason
the crate's documentation does not mention — in a crate whose stated purpose includes device
routing for local multiplayer.

_Fix:_ **chunk 116**, which rewrites `join.rs`'s recipe wholesale and takes this paragraph with it.
A claim helper that sees queued claims would let the sketch stay as short as it reads, and is the
better fix if the rewritten recipe still reads long.

---

## 3. Absent — a requirement says it should exist and nothing does

Ordered by what a real game would miss first.

### 1015 Nothing can bind to where the pointer is

R13.1, R13.4, R13.6 · `frame.rs`

The input frame carries mouse _motion_ and no absolute position at all, so position cannot be
distinguished from motion because only one of the two is there. R13.1 wants both. R13.3's mouse
wheel is deferred with a gate; these are not, and R15.10's split-screen pointer-to-viewport mapping
is blocked behind them.

_Fix:_ **chunk 98** — a Pong variant, a mouse-controlled paddle. R15.10 stays a Split Friction
follow-on once the mechanism lands here.

### 1017 Either modifier

R12.3: a chord's modifier should be able to say "either Ctrl", as one binding rather than two.
`with` takes a single `ButtonControl`, so a game wanting either `LeftCtrl` or `RightCtrl` to arm a
chord writes both bindings by hand today. R4.10 already assigns this to the chord mechanism by name.

Self-contained, independent of 1018.

_Fix:_ **chunk 94b**.

### 1018 A platform modifier

R12.4: `Cmd` on macOS should be usable as `Ctrl` everywhere else, as a named modifier resolved at
binding time rather than something every cross-platform game re-derives by hand.

Self-contained, independent of 1017.

_Fix:_ **chunk 94c**. (R12.2 and R12.7, the physical-binding layout-label gap, wait on upstream
winit — [winit#4606][] and [winit#2678][] — and are the deferred table's row, not a chunk.)

[winit#4606]: https://github.com/rust-windowing/winit/issues/4606
[winit#2678]: https://github.com/rust-windowing/winit/issues/2678

### 1019 `Reflect` reaches two modules, and nothing anywhere registers a type

R24.3 (MUST) · `action.rs`, `frame.rs`, `Cargo.toml:38,94-101` · reasoned from the pinned Bevy
commit's own `Cargo.toml` feature graph, **not probed**

`#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]` appears in `action.rs` and `frame.rs` and
nowhere else, and **nothing in the crate or the examples calls `register_type`**. That reads like a
gap only manual calls can close, but since [bevyengine/bevy#15030][], landed well before the pinned
commit, a non-generic `#[derive(Reflect)]` type is registered automatically at app startup — no
`register_type` needed — unless it opts out with `#[reflect(no_auto_register)]`. A generic type is
the one case that still needs a manual call, per type parameter, because there is no single
`TypeId` to register on its behalf.

That mechanism is inert here, though, which is why the finding still holds. It lives behind
`bevy_reflect`'s own `auto_register_inventory` (or, on platforms `inventory` doesn't support,
`auto_register_static`) feature, bundled into `bevy_reflect`'s `default` set upstream — but this
crate's `bevy_reflect` dependency is declared `default-features = false` (`Cargo.toml:38`), and its
own forwarded `bevy_reflect` feature (`Cargo.toml:94-101`) never re-adds either. So today, deriving
`Reflect` on `Prompt` or `ActionMapping` would still leave it out of the registry — not because the
crate has to hand-register everything, but because it never turned on the feature that would do it
for free.

`Control`, `DeviceFamily`, `ActionMapping`, `RebindPolicy`, `Prompt`, `ControlOrigin`,
`DeviceHandle`, `ActionObstacle` and `Paired` still carry no `Reflect` at all, which
auto-registration doesn't touch — deriving it is a separate step from registering what's derived.
`Paired` is a component a scene would author, and the five resources `ActionMapPlugin` initializes
are unregistered.

Chunk 17c owns R5.6 and R17.5 — `Modifier` and `Condition` — and `docs/decisions.md:430`
deliberately keeps those two bound-free. Neither is R24.3.

_Fix:_ **chunk 109** — derive `Reflect` on the missing types and turn on auto-registration, checking
the `no_std` interaction first rather than assuming it.

[bevyengine/bevy#15030]: https://github.com/bevyengine/bevy/pull/15030

### 1043 No auto-switching which device a player is paired to

R15.8 (SHOULD, split from 1020) — auto-switching on input from another device, with hysteresis.
Nothing — and R18.6's _withdrawal_ names this as the one thing that would revive it, so an unbuilt
SHOULD is load-bearing for a withdrawn requirement staying withdrawn.

_Fix:_ **deferred**, with the gate stated in Roadmap's deferred table.

### 1044 No opaque platform-user identity

R15.9 (SHOULD, split from 1020) — opaque platform-user identity attached to a player. Nothing to
show without a real platform SDK behind it, unlike the rest of this group.

_Fix:_ **deferred**, with the gate stated in Roadmap's deferred table.

### 1050 The one answer to "which device fired this" is unused by the example that needs it

`protagonist.rs`'s `pair_on_join` · **verified**: its own doc says "Not backend-safe"

An ordinary action's value is device-agnostic by design, so `Fired<Join>` cannot say which device
pressed. The crate's answer is a class binding: `ClassFired` carries the untouched raw event, and
`event.device()` names the device — which is what `join.rs` documents and what chunk 66 originally
shipped.

Split Friction no longer uses it. `Join` is bound to two concrete controls, and `pair_on_join` reads
`ButtonInput<KeyCode>` and `Query<&Gamepad>` directly to find the presser — queries that do not
exist under a Steam authority, which its own comment says in as many words. So the example
demonstrates a device-routing crate failing to answer, on its own flagship screen, the question its
device routing exists for, and it does so by reaching around the crate to Bevy.

**Why** `Join` moved off the class binding is recorded, in chunk 110's commit message rather than in
any document: a class binding has no Steam expression, and the join caption needed a concrete
control to name instead of a hardcoded "press any button". Neither reason expires, so going back to
the class binding is not the fix — 1046 adds a third against it.

_Fix:_ **chunk 116**, which answers the question by pairing the context that hears the press rather
than by carrying an origin on the action. The deferred row ("a backend-safe way to ask which device
drove an ordinary action's current activation") is withdrawn there rather than met.

### 1052 Naming a device to the player has no requirement and no support

`split_screen.rs`'s `device_name` · `Requirements.md` §11, §18

R18.3 forbids hard-coding English for a control's display string and the crate supplies
`fallback_label` so a game does not have to. A *device* has no equivalent: nothing returns a
descriptor or a key for "Xbox Controller", and R11.6 covers brand *resolution* without saying
anything about naming the result. So Split Friction's pane label hard-codes four English strings off
`GamepadBrand`, which is exactly the shape R18.3 exists to prevent one step to the left.

The gap is in `Requirements.md` first: no requirement covers it, so the crate is not failing one.
Whether a device name is §18's business (a display string, like a control's) or §11's (a fact about
the device, like its brand) is the question to settle before anything is built.

Unrouted.

### 1021 Accessibility has no citation anywhere in the project

`Requirements.md` §20, all six requirements, uncited in `src/`, `examples/`, `docs/`, `Roadmap.md`
and `CLAUDE.md`. The section's own preamble calls these "cheap to accommodate now and expensive to
retrofit," which is the argument for looking at it before more is built on top.

R20.2 and R20.5 are built (chunk 64) and R20.1 holds by construction. R20.4 is withdrawn, and R20.7,
the narrower requirement that replaced it, is 1045. **R20.6** (MAY, sticky modifiers / one-handed
support) is **reviewed and left alone**: no in-tree pressure and no case behind it — not worth a
chunk unless one shows up.

R20.3's sequential alternative to chords is 1023 by content and by no other link.

### 1045 No timing threshold can be offered to the player at all

R20.7 (was R20.4, withdrawn; split from 1021) · `binding/builder.rs`

`tunable_dead_zone` lets a game expose a dead zone as a named, bounded, persisted value. Nothing
does the same for a duration: `hold`, `tap`, `multi_tap` and `pulse` each take a compile-time
constant, and `TunableValue`'s own doc says its two shapes cover "both tunables this crate declares
anywhere in-tree". So a game wanting to offer a longer double-tap window cannot, at any granularity.

The finding used to be R20.4's global scale. That requirement is withdrawn — its thresholds do not
share a sign, so one factor cannot move them all toward forgiveness — and what is absent survives
the withdrawal in a narrower form: not "a game cannot move all the timings at once" but "a game
cannot move one".

_Fix:_ **chunk 115**.

### 1022 A backend-owned row cannot be told apart from an ordinary fixed one

R19.8 (MUST) — a row a backend owns should say "not rebindable here, delegate to that backend's own
UI". `RebindPolicy` is `Here | Fixed` and `Override::NotOurs` is a row in the _player's_ diff, so a
screen reading `mappings()` cannot tell a backend-owned row from an ordinary fixed one without
consulting its own working copy.

_Fix:_ **chunk 42** — its binding panel is where this distinction has to render anyway.

### 1040 No semantic control aliases

R4.4 (SHOULD) — semantic control aliases (`Submit`, `Cancel`, `MenuLeft`) resolving per device
class. Nothing in tree. It is load-bearing rather than convenient: R4.4 names it as what makes
R18.7's console confirm-button swap tractable.

_Fix:_ **chunk 101**.

### 1041 No way to stop the frame sampling itself

R9.9 — a pumped sampling mode. `sample_input`, `begin_sample` and `record` are all public, so the
pieces exist; what is missing is a way to stop `InputFramePlugin` scheduling sampling at all.
Floated as a companion to chunk 83's rewind; chunk 83 says to confirm the need before routing it
there. Unrouted.

### 1047 Capability queries are absent for devices the crate already models

R11.3 (MUST) · `device.rs`

The module's own doc claims "capability data" (`device.rs:5`); nothing answers a capability
question anywhere in the crate — no rumble, motion/gyro, touchpad, battery or LED query, and no way
to ask what controls a device has beyond matching on `DeviceHandle`'s own closed kind. §18's
prompts and any "can this player play at all" check — R11.3's own two named callers — have nothing
to call.

Unlike the device model's closedness (D65), this isn't about admitting an unknown device kind — a
gamepad's rumble motors and battery level are things `bevy_input`'s own `Gamepad` component already
reports. Nothing here reads them.

Chunk 117k removed the module doc's claim rather than leaving it promising a MUST with nothing
behind it. Whatever lands this puts it back: `device.rs`'s summary line and its second paragraph
both list what the module holds, and capabilities belong in both once they exist.

Unrouted.

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

Unrouted.

### 1025 A context instance cannot be driven from outside the crate

R23.6 · `InputContextState::new` and `apply_frame` are both `pub(crate)`

`docs/design.md` §6 says "a test or replay harness can drive one directly." From outside, the only
way to get an instance is to spawn an entity and the only way to advance one is `App::update`. The
struct's freedom from ECS references is real and unreachable, and R23.6's standalone half has no
citation anywhere. The netcode deferred row is where this plausibly already belongs. Unrouted.

### 1026 Focus and picking ordering is neither documented nor enforced

R22.4 (MUST) wants documented ordering and integration with `bevy_input::InputSystems`,
`bevy_input_focus` and `bevy_picking`. The `InputSystems` third is met and documented
(`frame.rs:374`, design §1). The other two:

- **`bevy_picking` is named once, about something else.** `docs/decisions.md` mentions it flattening
  its generic `Pointer<E>`, which is a reversal note rather than an ordering. Nothing in `src/`,
  `examples/` or `Roadmap.md` mentions it at all, and no ordering constraint anywhere relates the
  two — which is the third R22.4 asks for.
- **R22.11** (MUST) — focus changes must resolve before the same frame's actions are evaluated.
  `active_if` schedules `condition.pipe(apply_active::<C>)` in `PreUpdate` `.before(Evaluate)` with
  no constraint against whatever writes `InputFocus`, and `examples/common/widget_focus.rs`'s
  `focus_is` adds none. Disasteroids is not bitten because it disables `InputDispatchPlugin` and
  moves focus from an observer in `Dispatch`, so the write lands after the read by construction
  rather than by an ordering — which is exactly the arrangement that stops holding for a game that
  keeps the plugin. **Reasoned, not probed.**

Unrouted.

### 1027 Two documentation requirements with no document

- **R16.4** (SHOULD) — the web caveats: pointer lock and gamepad access needing a user gesture,
  gamepad events being polled, key codes and `vendor_id` being less reliable. There is no occurrence
  of "wasm", "web" or "pointer lock" in `README.md`, `docs/` or `src/`.
- **R16.5** (SHOULD) — name the OS-reserved combinations that are unavailable. Nothing.

Unrouted.

### 1028 `tracing` is contradicted by a dependency choice recorded only in `Cargo.toml`

R22.3 (SHOULD) wants spans and events at the sampling and firing boundaries. What exists is four
`log::warn!` sites, all app-build or misconfiguration.

The crate depends on `log` rather than `bevy_log`, and the comment in `Cargo.toml` gives the reason:
`bevy_log` installs a `tracing-subscriber` and is `std`-only. That is a decision by
`docs/decisions.md`'s own admission test — name what breaks if reversed, and the answer is R22.3 or
the `no_std` build — and that document does not carry it. Unrouted.

### 1029 Two routing gaps rather than findings

- `Roadmap.md`'s chunk 17c calls serializing whole binding definitions (R17.6, R22.16) "deferred"
  inside a bullet. There is no row and no gate, which is what ground rule 5 forbids in as many
  words. Both are MAYs, so the stakes are small and the omission is not.
- **R7.5's opt-out is exercised by a test and nothing else.** `activate_including_held` now has a
  caller — a unit test in `src/eval.rs` — where at the time of the scan it had none. What is still
  missing is any example or production caller: the MUST's "unless explicitly opted in" clause is
  proven correct in isolation but has never been asked for by a game in tree.

Unrouted.

---

## 4. Prose — a comment or document contradicts the code

No behaviour at stake. All are small, and the reason to do them together is that a reader trusting
any one of them is misled about a mechanism.

### 1031 Examples and sketches that do not compile

- `docs/design.md` §3's trait sketch says `// plus CATEGORY and CONSUME, with defaults`. The
  constant is `CONSUMES`. Copying the sketch into a hand-written impl does not compile.

Unrouted.

### 1032 Internal comments whose stated reason is false

- `binding.rs:205`, `BindingSpec` justifies its copies with "the plan keys state by `ActionId`,
  which does not reach back to the type." `ActionId::info` reaches back to exactly the three fields
  the comment is justifying. The copies are still right — `info` takes the registry lock and linear
  scans — but the stated reason is the one a reader would use to decide whether the duplication may
  go.
- `frame.rs:340` credits calibration's placement with meeting R14.10. R14.10 governs an authority
  backend, which per D51 enters at the button state machine and never touches the frame. The
  placement is right and the `R`-number is wrong; it is the crate's only claim on R14.10. **Small —
  worth a minute if something else is open in this file, not worth a pass of its own.**
- `action.rs:335`, `Scratch::flags` is documented as "Condition-defined bits" and a modifier defines
  one too (`TOGGLE_LATCH`, `binding.rs:2311`). Related, and worth carrying with it: `condition.rs`'s
  own constant does not carry the note `binding.rs`'s does, explaining why two constants in two
  files can both be `1 << 0` — `plan.rs:691` gives every modifier and every condition its own cell.

Unrouted.

### 1033 Design sentences that are a clause short

- **§4 says "only the scratch is rebuilt."** `Plan::compile` rebuilds `indexed_controls` and
  `has_chords` as well, and `plan.rs`'s own comment says the first is required rather than
  incidental: an override rewrites which controls a binding reads, so one has to move between
  indexed and not. The code is right and the sentence is short.
- **§5.3 says the exclusion ceiling "is set by the `PreUpdate` pass and read — never rewritten — by
  every `FixedPreUpdate` run."** `evaluate_context` raises it for any `C::EXCLUSIVE` with an active
  instance in whichever schedule it runs, which is what makes a `Fixed` exclusive context work at
  all. The consequence the sentence hides, verified: a fixed exclusive context shadows lower-
  priority _fixed_ contexts and never a render one, in that frame or the next, because the reset
  runs at the top of `PreUpdate`. §5.2 states this for consumption and nothing states it for
  exclusion.
- **§6's "a test or replay harness can drive one directly"** — see 1025.
- **§7 does not state R7.3's cost.** Two simultaneously-active layers hold separate action state, so
  a game reading `ContextActions<Base>` does not see the layer's answer and has to read both. R7.3
  is a MUST that is met and claimed nowhere.
- **A `§10.1` citation points at the wrong section**, at `context/declare.rs:761`. It is about the
  override store being keyed by mapping alone, which is §10's preamble; §10.1 is "Applying," and
  this is not about applying. Two others, both in `overrides.rs` and both about the serialized form
  (now §10.3), went with 117e. **Small — worth a minute alongside the R14.10 mis-citation above, not
  worth a pass of its own.**

Unrouted.

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

Unrouted.

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
per-tick path, so R23.2 does not apply — but see 1003 (fixed) which made "how often" every frame
while it lasted. Unrouted.

### 1035 R23.2 is unenforced, and the register's count of violations is stale

`Roadmap.md` says two violations have reached the per-tick path and both were caught by reading. The
scan found two more. One is gone: the `binding.source.controls()` call that allocated a
`Vec<Control>` per consuming binding per tick no longer exists — `controls` has no caller in
`eval.rs` at all now, and nothing recorded which chunk removed it. The other is still present:

- `evaluate_context` builds `let mut claims = Vec::new()` per instance per tick (`eval.rs`, in the
  loop that calls `apply_frame`) and allocates the moment anything is claimed. `chord_claims` sits
  on `InputContextState` and cites R23.2 in its comment for exactly this reason, and
  `dispatch_transitions` takes and hands back its log to keep the allocation — so both idioms are
  established in the same file and `claims` follows neither.

The finding is not the line. It is that violations keep being found by reading and then quietly
stop being true, which is what the register calls "a rule with no tooling behind it." This entry has
now gone stale in exactly the way it complains about: it has been rewritten once for a count that
moved, and it will need it again. Unrouted.

### 1036 Public items with no caller and no document

Splitting by why they are here, because the answer differs:

**Should probably be `pub(crate)`** — used only inside the crate, while a sibling doing the same job
is already crate-private:

- `eval.rs`: `release_consumed_controls`, `release_consumed_in`, `dispatch_transitions`,
  `dispatch_class_fires` — `pub` with `ActionMapPlugin` and `declare_context` as their only
  registrars, beside `reset_exclusion_ceiling` and `evaluate_context`, which are `pub(crate)` and do
  the same job in the same file.
- `frame.rs`: `warn_on_unread_gamepad_settings`, `retire_read_events` — same shape, only
  `InputFramePlugin` names them.
- `capture.rs`: `ReservedControls::claimant` and `iter` — every caller is inside the crate.
  `claimant` is the one with a plausible unwritten caller, since a screen refusing a reserved
  control wants to say what reserved it.

**No caller anywhere, and no document asks for them:**

- `capture.rs`: `CaptureSession::mapping`, `slot`, `accepts`, `family`, `excluded`, `is_listening` —
  six readers, named by no document. `is_listening`'s own doc says it "exists for tests."
  `ControlCaptured` carries the mapping and the slot back, which is the path a screen actually
  takes.
- `device.rs`: `GamepadCalibration::clear_device`, `is_empty`.
- `mapping.rs`: `MappingKey::part` — plausible for a screen grouping a composite's four rows, and
  nothing does.
- `overrides.rs`: `Overrides::is_empty` — called only by its own test; design §10 enumerates twelve
  `Overrides` methods and this is not one.
- `action.rs`: `ActionValue::from_output` — **now has a caller**, `backend.rs`'s
  `AuthorityValues::set`, added by chunk 111 after the scan. It still duplicates the four `From`
  impls twenty lines above it, and `into_output` is still called only by its own tests, so "four
  public names for two conversions" holds; "no caller" no longer does.
- `action.rs`: `ActionIntent::supports_output` — a public wrapper over `is_one_of`, which is the one
  the derive calls. Nothing else calls either.
- `action.rs`: `ActionState::new` — a `const fn` constructor for a two-field struct with both fields
  public and a `Default` impl.
- `plan.rs`: `Plan` is `pub` with **nothing public on it** — no field, no method, no constructor,
  and it appears in no public signature, every wrapper holding one being `pub(crate)`. It is on
  docs.rs as a struct a reader can name and do nothing with. Design §4 names `Plan<C>` in prose,
  which is architecture rather than a request for it to be public.

**Reviewed and left alone**: `GamepadCalibration::clear_device`/`is_empty`, `MappingKey::part`,
`Overrides::is_empty` and `ActionState::new` are ordinary API completeness on small types. "No
caller in tree" is not a defect for a library; it is only worth acting on for items that are _also_
misleading, and none of these is.

Worth stating for calibration: `docs/design.md` §7.3, §8.2, §9.1 and §10 _enumerate_ their public
surface rather than describing it, so the sweep over those was a diff and came back nearly empty —
eleven conditions, ten modifiers, six presentation methods, eleven `ActionMapping` fields, eight
problem kinds, all matching one for one. The list above is concentrated where no document
enumerates.

Unrouted.

### 1053 The release itself has no destination

`Roadmap.md` · `Cargo.toml`

Every Bevy dependency is a git dependency, resolved in `Cargo.lock` to a `0.20.0-dev` commit.
crates.io rejects git dependencies, so the crate cannot be published until Bevy 0.20 ships — an
external date, not a chunk.

Nothing in `Roadmap.md` says so. There is no release chunk, no deferred row gated on the 0.20
release, and no checklist of what must be true before the first publish, which by ground rule 5 is
an item that will be dropped. The work that is release-shaped is scattered through chunks whose
descriptions do not mention it: 110's sum type and 42's backend trait are both "cheap now, breaking
later" and neither says that is a publishing deadline rather than a preference.

_Fix:_ a deferred row gated on the Bevy 0.20 release would be the smallest thing that stops this
being forgotten. What belongs in it is the ordering question rather than the date. Unrouted.

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

Unrouted.

### 1037 Machinery out of proportion

None of these costs anything measurable at run time. What each costs is an invariant a reader has to
confirm is still true.

- **The action registry holds one fact three ways.** `next_id` is always `entries.len()`; each
  entry's stored `ActionId` is always its own index; and `ActionId::info` linear-scans the vector
  that index would subscript. Written once per process, holding tens of rows.
- **`Plan<C>`'s type parameter is phantom.** No field and no method reads `C`. It buys that handing
  context A's plan to context B's state does not compile; it costs `compile`, 130 lines of it,
  monomorphized once per context type — and the three wrappers that hold a `Plan` carry `C`
  themselves already. **Reviewed and left alone**: the compile-time cost is real and the type safety
  it buys is also real. Nothing has measured either, and this is not worth changing without a
  measurement.
- **`ConsumedControls` is a `HashMap<TypeId, HashMap<Control, &'static str>>` over two schedules.**
  `claim::<S>` is instantiated at `PreUpdate` and `FixedPreUpdate` and nowhere else, so the outer
  map holds at most two entries for the life of the process and `claimant` walks both per lookup.
  Two fields would say the same thing, and the `ScheduleLabel` bound would stop being propagated
  through `evaluate_context`, `release_consumed_in` and `declare_context`'s two arms to key a map of
  two.
- **`MappedPart` caches two facts it could derive.** `family` is always `control.family()` and `key`
  is always `MappingKey::new(prefix_of(binding), part)` — which `mappings_of`'s follower pass
  recomputes from the same inputs twenty lines later rather than reading.

Unrouted.

### 1038 One command the Verification list does not run

`cargo doc --no-deps --all-features` warned twice when the scan ran — a redundant explicit link
target in `device.rs`, and a link to `GamepadBrand::Generic` in `present.rs` with nothing importing
`GamepadBrand` into that scope. **Both are fixed**: rebuilt from a touched `lib.rs`, the command is
now warning-free.

The finding survives the fix, because it was never really about the two warnings. That command is
the only one in the project that reads doc comments at all, and it is not in `CLAUDE.md`'s
Verification list — so nothing would have caught either warning, and nothing will catch the next.
Unrouted.

---

## Checked and correct

Recorded so nobody re-derives it. Every one of these was read against the code and holds.

**Values and shapes.** R2.2's conversion table matches `to_bool`/`to_axis1`/`to_axis2`/`to_axis3`
cell for cell, including the two rows the requirement expects an argument about. R2.10's two
hardware cases hold in `ActionIntent::accepts`. R1.1's declared path is required by the derive with
no default. Design §4's fourteen `DiagnosticKind` variants and both `Severity` variants match
exactly.

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
tunable pass runs after the control rewrite and matches family as well as key. R18.2's consumption
filter reads only earlier contexts' claims; the sort is stable, so declaration order survives as the
last tiebreak. R18.5's invalidation covers every clause but the layout one its own aside withdraws.
R18.8 and R18.9's origin half hold. The four control tables round-trip exhaustively, unnamed
variants included. R22.6's migration path exists in `docs/comparison.md`. R21.1–R21.3 are met by the
test suite's shape. Capture's arming skips the press that opened the session, and a refused press is
claimed so it does not also play the game. `admissible` asks family before reserved before shape.
R15.1's many-to-many holds — neither `Paired` nor `is_claimed` enforces exclusivity, which is what
lets two players share one keyboard.

**Excluded rather than missed**, both already recorded: `apply_overrides_for` discards the rewritten
rows, which is the per-entity presentation deferred row; and `Override::NotOurs` leaves the crate's
binding live rather than silencing it, which is R0.6 and chunk 42's review surface.
