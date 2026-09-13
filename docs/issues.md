# Findings: the implementation scan

What the six-session scan of `src/` turned up, reordered by how much it matters rather than by
which session found it.

**How to read an entry.** Each says where the problem is, what someone would actually observe, and
whether it was confirmed by running something or only by reading. That last distinction is the
important one, and it is stated per entry rather than assumed.

**Numbering.** Each entry's number is a flat, permanent identity, assigned once from a single
counter and independent of which tier it sits in — the tier is where it's filed today, not what it
is. A number is never reused: once an entry is fixed and its chunk has landed, the entry is retired
from this document rather than kept as a silent record, and its number simply does not reappear. The
chunk's own commit, and `docs/design.md` or `docs/decisions.md` where anything about it was durable,
are the record. A gap in the sequence below is a retired finding, not an omission. The next
unassigned number is stated here; keep it up to date when numbering new items.

**Next: 1049.**

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

Found reaching for `ControlClass::AnyStick` to fix a real papercut: Split Friction's join gesture
(`protagonist.rs`, chunk 66) binds `Join` to `ControlClass::AnyButton` only, so a player picking up
a gamepad and wiggling the stick — the natural first move — does not join; a button has to be found
first. `AnyStick` looks like the fix, but a class binding's fold skips the modifier chain (§8, no
dead zone stage), and `class_dispatch`'s own `actuated` check treats any nonzero axis reading as a
match — so a stick whose rest position sits off true zero, which no calibration step catches before
a device is paired, would fire `Join` on its own drift. The failure is silent: a device joins that
nobody touched, and nothing says why.

No requirement currently asks for stick-triggered joining, so nothing in tree takes this path today
— `AnyButton` alone is what chunk 66 shipped and what Split Friction still binds. The risk is
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
`examples/` today via `.custom()` (`binding.rs:1885`). It has to ride an ordinary
`.bind::<Join>(Stick::Left)` rather than the existing `bind_class::<Join>(ControlClass::AnyButton)`,
since class bindings skip the modifier chain — so `pair_on_join` would need to observe both
`On<ClassFired<Join>>` (buttons) and `On<Fired<Join>>` (the wiggle-gated stick), claiming through
the same `is_claimed` check either way. Free parameters — window length, reversal count, how much
movement counts — are a game's own design question, which is the reason this stays a worked example
rather than a `BindingModifier` variant: baking in an intensity or a pattern would be guessing at
what any particular game's grapple-escape or join gesture actually wants. Worth doing once Split
Friction wants the polish; not routed to a chunk, since nothing here is missing from the crate.

---

## 3. Absent — a requirement says it should exist and nothing does

Ordered by what a real game would miss first.

### 1014 An action has no elapsed time and no progress, so a hold-to-confirm meter cannot be drawn

R3.4 (MUST) and R3.5 · `action.rs:354`

`ActionState` is still `{ value, phase }`. R3.4 wants elapsed time in the current state, in the same
simulated seconds the action's own conditions count with; R3.5 wants progress toward firing, which
is the number a charge bar or a hold-to-confirm ring is drawn from.

**Both numbers already exist** — `Scratch::time` is the elapsed time and `BindingCondition::Hold`'s
`duration` is R3.5's denominator — but the scratch is `pub(crate)` inside `InputContextState` with
no read path out. So this is a plumbing job, not a design one.

It is also R22.1's fifth cause: "condition Z at 40% progress" is the same number.

_Fix:_ **chunk 99** — a Disasteroids smart bomb, hold-to-charge, with the progress number drawing
its charge meter.

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

### 1021 Accessibility has no citation anywhere in the project

`Requirements.md` §20, all six requirements, uncited in `src/`, `examples/`, `docs/`, `Roadmap.md`
and `CLAUDE.md`. The section's own preamble calls these "cheap to accommodate now and expensive to
retrofit," which is the argument for looking at it before more is built on top.

R20.2 and R20.5 are built (chunk 64) and R20.1 holds by construction. R20.4 is 1045. **R20.6** (MAY,
sticky modifiers / one-handed support) is **reviewed and left alone**: no in-tree pressure and no
case behind it — not worth a chunk unless one shows up.

R20.3's sequential alternative to chords is 1023 by content and by no other link.

### 1045 No global timing scale

R20.4 (split from 1021) — every hold duration, tap window and repeat rate globally scalable by one
user preference. The crate's only scaling is a per-mapping tunable, so a game wanting "all timings
×1.5" sets every one of them by hand.

_Fix:_ **chunk 102**.

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

- **`bevy_picking` has no mention anywhere in the tree** — not in `src/`, `examples/`, `docs/` or
  `Roadmap.md`.
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

### 1030 Public docs that promise a feature

| Where                                   | Says                                                                                                                | Actually                                                                                                                                                                                                                                                                                               |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `lib.rs:174`                            | the `touch` feature is "Touch input as a binding source"                                                            | no `cfg(feature = "touch")` anywhere in `src/`; design §11 says _reserved_                                                                                                                                                                                                                             |
| `device.rs` module doc                  | the module has persistent device identity and capability data                                                       | neither (R11.5, R11.3); the `DeviceHandle` doc eight lines below says the first is not built                                                                                                                                                                                                           |
| `player.rs` module doc                  | the module "describes the named device requirements a game can assign players against"                              | it holds `Paired` and nothing else. That was R15.7, now withdrawn, so this promises something the crate will never grow rather than something it owes — the sentence goes rather than waiting on a fix. Chunk 48 renamed the one `Scheme`-like type to `DeviceFamily`, which answers a different question (which family a control belongs to, not what a player requires) |
| `inspect.rs:76`                         | `ActionDump::state` is "Value, phase, elapsed time and progress"                                                    | `ActionState` is `{ value, phase }`; the two extra numbers are 1014                                                                                                                                                                                                                                    |
| `lib.rs:219`                            | `ActionMapSystems` is "System sets for the two stages of the input pipeline"                                        | four variants; the body names `Sample` and `Evaluate` and says nothing about `Capture` or `Dispatch`, both of which are public ordering targets                                                                                                                                                        |
| `action.rs:497`                         | write `InputContext` by hand "if you need to configure the component differently; it is three associated constants" | the trait is not what makes the type a component — the derive emits `Component`, `Default`, `Clone` and `Copy` alongside it, and a hand-written impl gets none. `macros/src/lib.rs:131` says "four associated consts" for the same trait; four exist and three are required                            |
| `mapping.rs`, `ActionMapping::capacity` | "Meaningful only where `rebind_policy` is `Here`"                                                                   | a preset moving a `Fixed` row is refused `TooManyControls { capacity: Some(1), given: 2 }`, verified — capacity is the second thing `refusal` consults on exactly the rows the sentence excuses it from                                                                                                |

Unrouted.

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
- **Three `§10.1` citations point at the wrong section**, in `context.rs`, `overrides.rs` (twice).
  Two are about the serialized form, now §10.3; one is about the override store being keyed by
  mapping alone, which is §10's preamble. §10.1 is "Applying," and none of the three is about
  applying. **Small — worth a minute alongside the R14.10 mis-citation above, not worth a pass of
  its own.**

Unrouted.

---

## 5. Cost and surface

### 1034 One prompt lookup walks every declared context

`present.rs`, `BindingTable::prompts`, still calling `read_bindings::<C>` fresh per context per call

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
scan found two more, both still present:

- `eval.rs:763` calls `binding.source.controls()`, allocating a `Vec<Control>` once per consuming
  binding per tick it fires or is ongoing. `for_each_control` is the allocation-free form and its
  doc says so in as many words; `eval.rs:763` is `controls`' only caller in the tree. **One line.**
- `eval.rs:247` builds `let mut claims = Vec::new()` per instance per tick and allocates the moment
  anything is claimed. `chord_claims` sits on `InputContextState` and cites R23.2 in its comment for
  exactly this reason, and `dispatch_transitions` takes and hands back its log to keep the
  allocation — so both idioms are established in the same file and `claims` follows neither.

The finding is not really either line. It is that four violations have now been found by reading,
which is what the register calls "a rule with no tooling behind it," and the count is the part that
keeps going stale. Unrouted.

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
- `action.rs`: `ActionValue::from_output` — no caller in `src/`, `examples/` or the tests, and it
  duplicates the four `From` impls twenty lines above it. `into_output` is called only by its own
  tests. Four public names for two conversions.
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

`cargo doc --no-deps --all-features` warns twice — `device.rs:22`, a redundant explicit link target,
and `present.rs:511`, a link to `GamepadBrand::Generic` with nothing importing `GamepadBrand` into
that scope. That command is the only one that reads doc comments, so it belongs in `CLAUDE.md`'s
Verification list. Unrouted.

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
of held state and `is_pressed` refuses a consumed control inside it. R10.2 holds but for 1014. The
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
