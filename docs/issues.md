# Findings: the implementation scan

Everything the six-session scan of `src/` turned up, reordered by how much it matters rather than by
which session found it. Nothing here has been acted on: no code and no `Roadmap.md` entry was
changed while it was being written.

**How to read an entry.** Each says where the problem is, what someone would actually observe, and
whether it was confirmed by running something or only by reading. That last distinction is the
important one, and it is stated per entry rather than assumed.

**The calibration warning, stated up front because it is fair.** Ask a model to find sixty problems
and it will find sixty. Some of what follows is real and some is a rule nobody would ever violate.
The tiers below are the honest attempt to separate them, and §6 names the ones I would drop outright
rather than leaving them to look like work. Where a finding rests on reasoning rather than on a
probe, or where the reachable case is hypothetical, the entry says so in as many words.

**What the tiers mean.**

| | Test |
| --- | --- |
| **1. Live** | An ordinary build, an ordinary API call, and the answer is wrong |
| **2. Latent** | The code is wrong and the path is not taken in tree — a feature exists that nothing has used yet |
| **3. Absent** | Not wrong, missing, with a requirement saying it must exist |
| **4. Prose** | A comment or document contradicts the code. No behaviour at stake |
| **5. Cost and surface** | Public items nothing asks for, and machinery out of proportion |
| **6. Drop** | Named so nobody re-finds them |

This document does not *decide* routing, and it does not describe what the crate does — routing is
ground rule 5's business and yours, and `docs/design.md` is the description. It does record routing
once it happens: an entry that has been given a chunk says so on its `Fix` line, so what is left
unrouted can be read off the entries that stay silent.

---

## 1. Live — an ordinary build gets a wrong answer

### 1.2 A player who rebinds a two-slot row down to one control loses the second slot for good

`overrides.rs`, `current_rows` and `mappings_of` · **verified** against a headless `App`

A mapping declared with two default controls (say `Space` and `KeyJ`) has `capacity: UpTo(2)`, which
is what draws the settings screen's "Primary" and "Secondary" columns. Rebind it to hold one control
and `current_rows` takes the derived row whole, `mappings_of` re-infers capacity from the bindings
that survived, and the row comes back `UpTo(1)`.

What the player sees: the Secondary column disappears from that row and there is no way to get it
back. `refusal` still reads the *declared* row, so the model would accept the secondary — what stops
them is the screen, which sizes its columns from `capacity.slots()` (`settings.rs:942`,
`overlay.rs:120`) or asks `has_room_for` (`examples/capture.rs:173`).

`docs/design.md` §9.1 and R19.9 both say capacity is raised and never lowered.

The fully-cleared case is right by accident: no derived row is found at all, so `current_rows` falls
back to the declared one, capacity included.

*Fix:* **chunk 81** — one line and a test, `current_rows` carrying the declared capacity through
`widest`.

### 1.3 A save file with a version number this build does not know loads as if it were version 1

`overrides.rs:558` · **verified**

`resolve` binds the version field to `_version` and never branches on it, with a comment saying
there is only one version so far. A file saying `version = 99` is therefore read row by row as a
version 1 file.

The case: you ship a v2 format, a player runs a v1 build (a rollback, a second machine, a Steam beta
branch), and their v2 settings file is silently reinterpreted rather than refused. Half of R17.3's
MUST is done — a file *missing* `version` fails the whole load — and the other half is that nothing
says what a loader does with a number it does not recognise. `docs/design.md` §10.3 shows
`version = 1` in its sample without saying.

*Fix:* **chunk 84**. Refusing an unknown version is three lines; deciding what a migration actually
looks like is not, and is the part worth thinking about before the format ships.

### 1.4 A dead zone at or above full deflection multiplies by sixteen million instead of zeroing

`binding.rs:2164`, `dead_zone_remainder` · **verified** by probe

```rust
remainder / (1.0 - dead_zone.lower).max(f32::EPSILON)
```

The `.max(f32::EPSILON)` turns a divide-by-zero into a divide-by-epsilon. `DeadZone::radial(1.0)`
applied to `(3.0, 0.0)` returns `(16777216.0, 0.0)`. The comment directly above the guard says the
case "leaves nothing to stretch", so the code and the comment disagree about the one case the
comment exists for.

Reachable wherever the magnitude is not normalized to 1: a diagonal `DirectionalButtons` reaches
1.414, and `MouseMove` carries an unbounded pixel delta, where `radial(0.9)` already turns 3.0 into
21.0. So it does not take `radial(1.0)` to see it — an ordinary large dead zone on mouse motion is
enough.

*Fix:* **chunk 85**, which needs both — `tunable_dead_zone` lets a player drive `lower` to full
deflection from a slider, where a plan-build diagnostic cannot reach.

### 1.5 While a modal is up, every prompt on screen is redrawn every frame

`context.rs:856` · **verified** against a headless `App`, 5 of 5 quiet frames against 0 of 5 without
the exclusive context

`apply_active` reads `if context.is_active() == live { continue; }` and the two branches below set
`active` alone. `is_active()` folds in shadowing, so under an exclusive context the comparison is
against a number the assignment cannot move. `activate` returns early, but the `Mut` deref that
reaches it does not, so `changed` is set regardless — and that bumps `PromptGeneration`.

What that costs: `PromptGeneration` is the subscription a prompt layer runs off, promised by R23.4
and by `InputContextState`'s own change-detection doc. For as long as a modal (any `EXCLUSIVE`
context) is up, that subscription fires every frame and every prompt in the game is recomputed with
it — and §5.1 of this document says one prompt lookup walks every declared context.

**The same line has a second consequence**, also verified: a context whose condition goes false
*while it is shadowed* evaluates as active for one frame after the shadow lifts. `shadowed` is
written by `evaluate_context` and read one system earlier by `apply_active`, so on the frame the
modal closes the comparison still sees the old shadow, matches `live == false`, and skips the
`deactivate`. Set a modal's and the game's conditions false together and the game context reads
`is_active() == true` that frame and false the next. `active_in_state` reads the same stale field
from `StateTransition`.

*Fix:* **chunk 86**. The one-line comparison is `context.active == live`; what has to go with it is
a reading of which of `active` and `is_active` every other caller means, which is why this wants a
chunk rather than an edit.

### 1.8 A game that sets a gamepad button threshold is ignored, and told nothing

`device.rs:296`, `is_customized` · **verified** by reading the field list against
`GamepadSettings`

The crate applies its own thresholds rather than Bevy's, and R14.9 exists so that a game which
configured Bevy's is warned rather than left wondering. The warning's trigger reads four of
`GamepadSettings`' six fields — `default_axis_settings`, `axis_settings`, `button_settings`,
`button_axis_settings` — and misses `default_button_settings` and `default_button_axis_settings`.

So a game that sets a *global* button press/release threshold (the common way to do it, rather than
per-button) gets exactly the silence R14.9 was written to prevent.

The comment's stated reason is also wrong at the pinned commit: it says `AxisSettings` is the only
one of the three with `PartialEq`, but `ButtonSettings` derives it (`gamepad.rs:821`) and only
`ButtonAxisSettings` does not (`gamepad.rs:1413`).

*Fix:* **chunk 88** — one clause and one field-by-field comparison, since `ButtonAxisSettings` is
the type without `PartialEq`, plus the correction to the comment that produced the gap.

### 1.9 `why_not` blames `NoInput` when the real reason is that the device is not this player's

`eval.rs`, `why_not_id` · reasoned from the signature, **not probed**

R22.1 names five causes an action might not fire and `Obstacle` answers three. The one that produces
a *wrong* answer rather than a missing one is device ownership: `why_not_id` takes
`&ConsumedControls` and no `Paired`, so it cannot see the pairing at all. A context whose `Paired`
is dropping every event the player generates reports `Obstacle::NoInput` — "nothing was pressed" —
when something was pressed and was filtered.

That is precisely the confusion R22.1 exists to end, arriving as the answer. It bites in local
multiplayer, which is the only place pairing is used, and it bites hardest during a join flow when
the pairing is the thing being debugged.

The fifth cause, "condition Z at 40% progress", is missing rather than wrong, and is 3.1 below.
`Obstacle` is `#[non_exhaustive]`, so both are additions rather than breaking changes.

*Fix:* **chunk 89**, which confirms this before fixing it — it is the one entry in this tier that
was reasoned rather than run. 3.1 keeps the fifth cause.

### 1.10 A context spawned but never declared does nothing and reports nothing

`context.rs`, `inspect.rs` · **verified**

Spawn a `#[derive(InputContext)]` component for a type that `add_context` was never called for and:
no `InputContextState` appears on the entity, no diagnostic is logged, and `dump` does not list it —
`DeclaredContexts` is the dump's only source, so the tool built to answer "why is nothing happening"
cannot see this case at all.

`ContextDump::instances` documents the *mirror* case ("declared and nobody has it, which is usually
a mistake") and makes it visible, which sharpens the asymmetry.

Forgetting one `add_context` call while adding a context is an ordinary mistake, and it is the
likeliest way to spend an afternoon on nothing. It also bears on R22.14's MUST — "spawning must be
sufficient", including from a scene — which holds only for a type that was *also* declared
imperatively, and no document says so.

*Fix:* **chunk 90**. The cheap half is a warning from the context component's own `on_add` hook
when no plan resource exists for its type.

### 1.11 `ActionMapPlugin` panics on its first update in the no-devices build

`lib.rs:255`, `capture.rs:607` · **verified** by running an `App` under
`--no-default-features --features libm`

`InputFramePlugin` is gated on `any(keyboard, mouse, gamepad)` and is the only caller of
`init_resource::<InputFrame>()`. `run_captures` takes `Res<InputFrame>` and is registered
unconditionally, as does `evaluate_context`. The result is
`Parameter … failed validation: Resource does not exist`.

All three single-feature builds pass; it is only the zero-feature one. That configuration is in
`CLAUDE.md`'s own Verification list, and **`cargo check` and clippy cannot see this** — which is all
that list runs for it.

*Fix:* **chunk 91**, with 1.12. The smoke test that calls `App::update` in that configuration is
the durable half, since the whole class is invisible to a type check.

### 1.12 `KeyCode` is not in the prelude, so the design's own first example does not compile

`lib.rs:319` · **verified**

`docs/design.md` §7.1 opens with `controls.bind::<Jump>(KeyCode::Space)`, and after a glob import of
`bevy_action_map::prelude` that does not compile. It works in `lib.rs`'s own quick start only
because that example also globs `bevy::prelude`.

Low severity and high frequency: it is the first thing anyone types.

*Fix:* **chunk 91**, with 1.11.

---

## 2. Latent — the code is wrong and nothing in tree takes the path

### 2.1 Wrapping a rescaling dead zone defeats the one-rescaling-stage rule

`binding.rs:2355` · **verified**: `PerSecond(100.0)` at `delta = 0.5` returns `Axis1(0.0)` through
the trait against `Axis1(50.0)` inherent; `Toggle { active: true }` never latches; `rescales()`
reports `false` for a rescaling dead zone

`impl Modifier for BindingModifier` forwards to the inherent `apply` with `&mut Scratch::default()`
and `0.0`, and does not forward `rescales` at all.

The reachable consequence is the third: `.custom(BindingModifier::DeadZone(..))` wraps the enum in
`Custom(Arc<dyn Modifier>)`, whose `rescales` asks the trait. So **two stacked rescaling dead zones
are refused as `ChainedRescaling` when both are declared with `.dead_zone`, and accepted when the
second is wrapped** — the same chain, one spelling caught and the other not. That rule is R5.3 and
D20.

Nothing in `src/`, `examples/` or the tests calls the impl, so nobody has hit it. It is public API,
and it is one of the two shapes `docs/design.md` §8.2 asks for.

*Fix:* delete the impl. That removes the finding outright and costs a public impl, which wants
naming in the commit; add a test that a wrapped rescaling modifier is still counted.

### 2.2 Clearing one part of a composite empties the other three

`overrides.rs:941` · **verified** against a headless `App`: four empty rows, no problem reported

`rewrite` drops a whole *binding* per slot the row no longer has, and a composite's four rows are
four *parts of one binding*. So `Override::Cleared` on `move.up` takes the binding away and
`move.down`, `.left` and `.right` come back holding nothing.

Nothing in tree produces a `Cleared` row — `settings.rs:1218` is the only site that reads one — but
the state is R17.7's and first-class, and `Overrides::bind` with an empty list is the door. A
settings screen with an "unbind" button is the obvious way in.

`CompositeCannotGrow` refuses the mirror case in the same pass, so the refusal list already knows
the shape and covers only the growing half.

*Fix:* a chunk rather than an edit. Dropping per part instead of per binding and refusing the clear
outright are both defensible, and which is right is a design question about what clearing one arrow
of a movement composite means.

### 2.3 A capture accepting `CharacterProducing` refuses every key, loudly, and never ends

`capture.rs`, `run_captures` → `admissible` · **verified**: `KeyA` answers
`Refused { reason: Shape }` and the session stays

`admissible` asks `class.contains(control)`, and that method's own doc says it is always `false` for
`CharacterProducing`, because membership is a property of the *event* rather than of the control.
`contains_event` is the one test that works, and `eval.rs:485` is its only caller.

`PromptScope::of` is the same root from the other side: `prompts(Jump, ANY)` returns 1 and
`prompts(Jump, ANY.of(CharacterProducing))` returns 0 where `ANY.of(AnyButton)` returns 1 — so
narrowing a prompt scope to that class silently empties it.

*Fix:* **chunk 82**, which takes `CharacterProducing` out of `ControlClass` — text input is its
only use case, so the class-binding builder gets its own door for it and both entry points here
become total. A second door rather than a second type: one variant gone, one method added.

### 2.4 A binding whose only conditions are blocking fires every tick at rest

`condition.rs:361`, `combine` · **verified** by driving `combine` at `ActionValue::Bool(false)`:
`Fired`

`combine` tests actuation in the no-conditions case only. Once a binding has any condition, the
"control is off rest" test is gone and nothing replaces it for a set with no explicit condition that
reads the value. One blocking condition that is not vetoing leaves `explicit == 0` and
`implicit_all` vacuously true, so the binding fires with the control at rest.

Unreachable through the built-ins — none of them returns `ConditionKind::Blocking` — but the kind is
public API through `Condition`, and a `BlockedBy` built-in would be the first thing to land on it.

### 2.5 In a build without `keyboard`, a held mouse button survives alt-tab

`eval.rs:430` · reasoned from the `cfg` structure, **not probed**

`KeyboardFocusLost` is behind `bevy_input`'s own `keyboard` feature, so a `mouse`-only build samples
no focus loss, and `held_mouse_buttons.clear()` compiles out with it. R16.1's MUST — the alt-tab
stuck-key bug must be impossible — is therefore unimplemented in that configuration.

Not fixable at our layer; it wants a stated price rather than silence.

---

## 3. Absent — a requirement says it should exist and nothing does

Ordered by what a real game would miss first.

### 3.1 An action has no elapsed time and no progress, so a hold-to-confirm meter cannot be drawn

R3.4 (MUST) and R3.5 · `action.rs:349`

`ActionState` is `{ value, phase }`. R3.4 wants elapsed time in the current state, in the same
simulated seconds the action's own conditions count with; R3.5 wants progress toward firing, which
is the number a charge bar or a hold-to-confirm ring is drawn from.

**Both numbers already exist** — `Scratch::time` is the elapsed time and `BindingCondition::Hold`'s
`duration` is R3.5's denominator — but the scratch is `pub(crate)` inside `InputContextState` with
no read path out. So this is a plumbing job, not a design one.

It is also the missing fifth cause in 1.9: R22.1's "condition Z at 40% progress" is the same number.

Neither requirement appears in `Roadmap.md`'s register, the deferred table, or any `Still open`
remainder.

### 3.2 Nothing can bind to where the pointer is

R13.1, R13.4, R13.6 · `frame.rs`

The input frame carries mouse *motion* and no absolute position at all, so position cannot be
distinguished from motion because only one of the two is there. R13.1 wants both. R13.3's mouse
wheel is deferred with a gate; these are not, and R15.10's split-screen pointer-to-viewport mapping
is blocked behind them.

### 3.3 Keyboard: no logical keys, no "either Ctrl", and no `Cmd` ≡ `Ctrl`

`Requirements.md` §12 has no citation anywhere in the project — not in `Roadmap.md`,
`docs/decisions.md`, `docs/design.md`, `src/` or `examples/`. Four MUSTs bear on bindings and three
are unbuilt:

- **R12.1** — physical or logical key binding, explicitly chosen. Only `KeyCode` is bindable.
  `RawEvent::Keyboard` carries the whole `KeyboardInput`, so `logical_key` already reaches the frame
  and nothing can bind it. The requirement's own example is `Ctrl+Z`, which this crate can spell
  only physically — so on AZERTY it is the wrong key.
- **R12.3** — left/right variants plus "either" as a first-class concept. `with` takes one
  `ButtonControl`, so "either Ctrl" is two bindings a game writes out by hand. R4.10 assigns this to
  the chord mechanism *by name*, so it has a destination in the requirements and none in the plan.
- **R12.4** — `Cmd` on macOS ≡ `Ctrl` elsewhere, as a named modifier resolved at binding time.
  Nothing. Every cross-platform game needs this and writes it itself.

R12.2 and R12.7 are presentation; R12.6 is the deferred text-input row; R12.5 is met.

### 3.4 `Reflect` reaches two modules, and nothing anywhere registers a type

R24.3 (MUST) · `action.rs`, `frame.rs`

`#[cfg_attr(feature = "bevy_reflect", derive(Reflect))]` appears in `action.rs` and `frame.rs` and
nowhere else, and **nothing in the crate or the examples calls `register_type`** — so even the types
that do derive it are absent from the type registry.

`Control`, `Scheme`, `Mapping`, `Capacity`, `Rebinding`, `Prompt`, `ControlOrigin`, `DeviceHandle`,
`Obstacle` and `Paired` carry no `Reflect`. `Paired` is a component a scene would author, and the
five resources `ActionMapPlugin` initializes are unregistered.

`bevy_reflect` is a **default** feature, so this is the shipped configuration: an inspector, an
editor or a scene serializer sees nothing of this crate.

Chunk 17c owns R5.6 and R17.5 — `Modifier` and `Condition` — and `docs/decisions.md:430`
deliberately keeps those two bound-free. Neither is R24.3.

### 3.5 Local multiplayer: no disconnect signal, no control schemes, no auto-switching

`Requirements.md` §15 has one cited requirement (R15.3) and four real gaps:

- **R15.5** (MUST) — on device loss the owning player must be identifiable, in-flight actions
  canceled, **and a signal raised so the app can pause and show a reconnect prompt**. The first two
  hold: a disconnect clears held state at `eval.rs:422` and the actions fall out of flight; the
  owner is identifiable by querying `Paired`. Nothing is raised. An app can read Bevy's own
  `GamepadConnectionEvent`, but no document says that is the intended answer, and a pause-on-
  disconnect prompt is a console certification item.
- **R15.7** (SHOULD) — named device-requirement sets with required and optional devices. Nothing.
  `Scheme` is a two-variant enum and is not this. See 4.3: `player.rs`'s own module doc claims it.
- **R15.8** (SHOULD) — auto-switching a player's active scheme on input, with hysteresis. Nothing —
  and R18.6's *withdrawal* names this as the one thing that would revive it, so an unbuilt SHOULD is
  load-bearing for a withdrawn requirement staying withdrawn.
- **R15.9** (SHOULD) — opaque platform-user identity attached to a player. Nothing.

R15.6 reaches chunk 72 through R11.5; R15.10 waits on 3.2.

### 3.6 Accessibility has no citation anywhere in the project

`Requirements.md` §20, all six requirements, uncited in `src/`, `examples/`, `docs/`, `Roadmap.md`
and `CLAUDE.md`. The section's own preamble calls these "cheap to accommodate now and expensive to
retrofit", which is the argument for looking at it before more is built on top.

R20.2 and R20.5 are built (chunk 64) and R20.1 holds by construction. Two have nothing:

- **R20.4** — every hold duration, tap window and repeat rate globally scalable by one user
  preference. The crate's only scaling is a per-mapping tunable, so a game wanting "all timings
  ×1.5" sets every one of them by hand.
- **R20.6** (MAY) — sticky modifiers / one-handed support.

R20.3's sequential alternative to chords is chunk 34 by content and by no other link.

### 3.7 Four smaller absences, each with a requirement and no destination

- **R19.8** (MUST) — a row a backend owns should say "not rebindable here, delegate to that
  backend's own UI". `Rebinding` is `Here | Fixed` and `Override::NotOurs` is a row in the
  *player's* diff, so a screen reading `mappings()` cannot tell a backend-owned row from an ordinary
  fixed one without consulting its own working copy.
- **R19.12**, the tunable half — "named alternative arrangements of mappings **and tunables**".
  `PresetBuilder` has `bind` and no `tune`. Reachable by hand (`Preset::rows` is public and
  `Overrides::tune` applies through the same path), offered by nothing, tested by nothing. The
  smallest item in this document: one builder method.
- **R4.4** (SHOULD) — semantic control aliases (`Submit`, `Cancel`, `MenuLeft`) resolving per device
  class. Nothing in tree. It is load-bearing rather than convenient: R4.4 names it as what makes
  R18.7's console confirm-button swap tractable.
- **R9.9** — a pumped sampling mode. `sample_input`, `begin_sample` and `record` are all public, so
  the pieces exist; what is missing is a way to stop `InputFramePlugin` scheduling sampling at all.

### 3.8 The device model is closed, which is a decision nobody wrote down

R11.1, R11.2, R11.3, R11.8, R11.9 — three of them MUST. `DeviceHandle` has no `Custom` and no
`#[non_exhaustive]`, and its doc argues for exhaustive matchability — the opposite of D19's choice
for modifiers and conditions.

R11.5, R11.6 and R11.7 have destinations, so what is missing is the model's *openness*, not its
identity half. The closure is a decision by `docs/decisions.md`'s own admission test and that
document does not carry it.

### 3.9 A context instance cannot be driven from outside the crate

R23.6 · `InputContextState::new` and `apply_frame` are both `pub(crate)`

`docs/design.md` §6 says "a test or replay harness can drive one directly". From outside, the only
way to get an instance is to spawn an entity and the only way to advance one is `App::update`. The
struct's freedom from ECS references is real and unreachable, and R23.6's standalone half has no
citation anywhere. The netcode deferred row is where this plausibly already belongs.

### 3.10 Focus and picking ordering is neither documented nor enforced

R22.4 (MUST) wants documented ordering and integration with `bevy_input::InputSystems`,
`bevy_input_focus` and `bevy_picking`. The `InputSystems` third is met and documented
(`frame.rs:374`, design §1). The other two:

- **`bevy_picking` has no mention anywhere in the tree** — not in `src/`, `examples/`, `docs/` or
  `Roadmap.md`.
- **R22.11** (MUST) — focus changes must resolve before the same frame's actions are evaluated.
  `active_if` schedules `condition.pipe(apply_active::<C>)` in `PreUpdate` `.before(Evaluate)`
  (`context.rs:924`) with no constraint against whatever writes `InputFocus`, and
  `examples/common/widget_focus.rs`'s `focus_is` adds none. Disasteroids is not bitten because it
  disables `InputDispatchPlugin` and moves focus from an observer in `Dispatch`, so the write lands
  after the read by construction rather than by an ordering — which is exactly the arrangement that
  stops holding for a game that keeps the plugin. **Reasoned, not probed.**

### 3.11 Two documentation requirements with no document

- **R16.4** (SHOULD) — the web caveats: pointer lock and gamepad access needing a user gesture,
  gamepad events being polled, key codes and `vendor_id` being less reliable. There is no occurrence
  of "wasm", "web" or "pointer lock" in `README.md`, `docs/` or `src/`.
- **R16.5** (SHOULD) — name the OS-reserved combinations that are unavailable. Nothing.

### 3.12 `tracing` is contradicted by a dependency choice recorded only in `Cargo.toml`

R22.3 (SHOULD) wants spans and events at the sampling and firing boundaries. What exists is four
`log::warn!` sites, all app-build or misconfiguration.

The crate depends on `log` rather than `bevy_log`, and the comment in `Cargo.toml` gives the reason:
`bevy_log` installs a `tracing-subscriber` and is `std`-only. That is a decision by
`docs/decisions.md`'s own admission test — name what breaks if reversed, and the answer is R22.3 or
the `no_std` build — and that document does not carry it.

### 3.13 Two routing gaps rather than findings

- `Roadmap.md`'s chunk 17c calls serializing whole binding definitions (R17.6, R22.16) "deferred"
  inside a bullet. There is no row and no gate, which is what ground rule 5 forbids in as many
  words. Both are MAYs, so the stakes are small and the omission is not.
- **R7.5's opt-out has no exercise.** `activate_including_held` has no caller and no test in `src/`,
  `examples/` or `tests/`. It is named by design §7.2, so it is asked for; what is missing is that
  the MUST's "unless explicitly opted in" clause is the only half of R7.5 nothing runs.

---

## 4. Prose — a comment or document contradicts the code

No behaviour at stake. All are small, and the reason to do them together is that a reader trusting
any one of them is misled about a mechanism.

### 4.1 Public docs that promise a feature

| Where | Says | Actually |
| --- | --- | --- |
| `lib.rs:174` | the `touch` feature is "Touch input as a binding source" | no `cfg(feature = "touch")` anywhere in `src/`; design §11 says *reserved* |
| `device.rs` module doc | the module has persistent device identity and capability data | neither (R11.5, R11.3); the `DeviceHandle` doc eight lines below says the first is not built |
| `player.rs` module doc | the module "describes the named device requirements a game can assign players against" | it holds `Paired` and nothing else; that is R15.7, which is 3.5 |
| `inspect.rs:76` | `ActionDump::state` is "Value, phase, elapsed time and progress" | `ActionState` is `{ value, phase }`; the two extra numbers are 3.1 |
| `lib.rs:218` | `ActionMapSystems` is "the two stages of the input pipeline" | four variants; the body names `Sample` and `Evaluate` and says nothing about `Capture` or `Dispatch`, both of which are public ordering targets |
| `action.rs:497` | write `InputContext` by hand "if you need to configure the component differently; it is three associated constants" | the trait is not what makes the type a component — the derive emits `Component`, `Default`, `Clone` and `Copy` alongside it, and a hand-written impl gets none. `macros/src/lib.rs:131` says "four associated consts" for the same trait; four exist and three are required |
| `mapping.rs`, `Mapping::capacity` | "Meaningful only where `rebinding` is `Here`" | a preset moving a `Fixed` row is refused `TooManyControls { capacity: UpTo(1), given: 2 }`, verified — capacity is the second thing `refusal` consults on exactly the rows the sentence excuses it from |

### 4.2 Examples and sketches that do not compile

- `docs/design.md` §3's trait sketch says `// plus CATEGORY and CONSUME, with defaults`. The
  constant is `CONSUMES`. Copying the sketch into a hand-written impl does not compile.

### 4.3 Internal comments whose stated reason is false

- `binding.rs:205`, `BindingSpec` justifies its copies with "the plan keys state by `ActionId`,
  which does not reach back to the type". `ActionId::info` reaches back to exactly the three fields
  the comment is justifying. The copies are still right — `info` takes the registry lock and linear
  scans — but the stated reason is the one a reader would use to decide whether the duplication may
  go.
- `frame.rs:338` credits calibration's placement with meeting R14.10. R14.10 governs an authority
  backend, which per D51 enters at the button state machine and never touches the frame. The
  placement is right and the `R`-number is wrong; it is the crate's only claim on R14.10.
- `action.rs:330`, `Scratch::flags` is documented as "Condition-defined bits" and a modifier defines
  one too (`TOGGLE_LATCH`, `binding.rs:2229`). Related, and worth carrying with it:
  `condition.rs:206` does not carry the note `binding.rs:2226` does, explaining why two constants in
  two files can both be `1 << 0` — `plan.rs:691` gives every modifier and every condition its own
  cell.

### 4.4 Design sentences that are a clause short

- **§4 says a variant rebuilds only the scratch.** `Plan::compile` rebuilds `indexed_controls` and
  `has_chords` as well, and `plan.rs:716`'s comment says the first is required rather than
  incidental: an override rewrites which controls a binding reads, so one has to move between
  indexed and not. The code is right and the sentence is short.
- **§5.3 says the exclusion ceiling "is set by the `PreUpdate` pass and read — never rewritten — by
  every `FixedPreUpdate` run".** `evaluate_context` raises it for any `C::EXCLUSIVE` with an active
  instance in whichever schedule it runs, which is what makes a `Fixed` exclusive context work at
  all. The consequence the sentence hides, verified: a fixed exclusive context shadows lower-
  priority *fixed* contexts and never a render one, in that frame or the next, because the reset
  runs at the top of `PreUpdate`. §5.2 states this for consumption and nothing states it for
  exclusion.
- **§6's "a test or replay harness can drive one directly"** — see 3.9.
- **§7 does not state R7.3's cost.** Two simultaneously-active layers hold separate action state, so
  a game reading `Actions<Base>` does not see the layer's answer and has to read both. R7.3 is a
  MUST that is met and claimed nowhere.
- **Four `§10.1` citations point at the wrong section.** `overrides.rs:311`, `:379` and `:608` are
  about the serialized form, now §10.3; `context.rs:1390` is about the override store being keyed by
  mapping alone, which is §10's preamble. New §10.1 is "Applying" and none of the four is about
  applying.

---

## 5. Cost and surface

### 5.1 One prompt lookup walks every declared context

`present.rs`, `BindingTable::prompts` → `context.rs:1244`, `read_bindings`

`prompts` rebuilds every declared context's binding list on every call — two fresh vectors per
context, every binding and every part — then does an O(n²) scan for earlier claims and an O(n²)
dedup. The cost does not depend on which action was asked for, so it is paid in full per call:
`examples/common/prompt_ui.rs:138` calls it once per span, so twenty spans over six contexts rebuild
a hundred and twenty context binding lists.

`PromptGeneration` bounds how *often* this runs and nothing bounds what one pass costs.
`BindingTable` holds the world for exactly the lifetime an amortization would want. Not on the
per-tick path, so R23.2 does not apply — but see 1.5, which makes "how often" every frame.

### 5.2 R23.2 is unenforced, and the register's count of violations is stale

`Roadmap.md` says two violations have reached the per-tick path and both were caught by reading. The
scan found two more:

- `eval.rs:750` calls `binding.source.controls()`, allocating a `Vec<Control>` once per consuming
  binding per tick it fires or is ongoing. `for_each_control` is the allocation-free form and its
  doc says so in as many words; `eval.rs:750` is `controls`' only caller in the tree. **One line.**
- `eval.rs:247` builds `let mut claims = Vec::new()` per instance per tick and allocates the moment
  anything is claimed. `chord_claims` sits on `InputContextState` and cites R23.2 in its comment for
  exactly this reason, and `dispatch_transitions` two hundred lines up takes and hands back its log
  to keep the allocation — so both idioms are established in the same file and `claims` follows
  neither.

The finding is not really either line. It is that four violations have now been found by reading,
which is what the register calls "a rule with no tooling behind it", and the count is the part that
keeps going stale.

### 5.3 Public items with no caller and no document

Twenty-two, accumulated across the scan. Splitting them by why they are here, because the answer
differs:

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

- `capture.rs`: `CaptureSession::mapping`, `slot`, `accepts`, `scheme`, `excluded`, `is_listening` —
  six readers, named by no document. `is_listening`'s own doc says it "exists for tests". `Captured`
  carries the mapping and the slot back, which is the path a screen actually takes.
- `device.rs`: `GamepadCalibration::clear_device`, `is_empty`.
- `mapping.rs`: `MappingKey::part` — plausible for a screen grouping a composite's four rows, and
  nothing does.
- `overrides.rs`: `Overrides::is_empty` — called only by its own test; design §10 enumerates twelve
  `Overrides` methods and this is not one.
- `action.rs`: `ActionValue::from_output` — no caller in `src/`, `examples/` or the tests, and it
  duplicates the four `From` impls twenty lines above it. `into_output` is called only by its own
  test. Four public names for two conversions.
- `action.rs`: `Intent::supports_output` — a public wrapper over `is_one_of`, which is the one the
  derive calls. Nothing else calls either.
- `action.rs`: `ActionState::new` — a `const fn` constructor for a two-field struct with both fields
  public and a `Default` impl.
- `plan.rs`: `Plan` is `pub` with **nothing public on it** — no field, no method, no constructor,
  and it appears in no public signature, every wrapper holding one being `pub(crate)`. It is on
  docs.rs as a struct a reader can name and do nothing with. Design §4 names `Plan<C>` in prose,
  which is architecture rather than a request for it to be public.

Worth stating for calibration: `docs/design.md` §7.3, §8.2, §9.1 and §10 *enumerate* their public
surface rather than describing it, so the sweep over those was a diff and came back nearly empty —
eleven conditions, ten modifiers, six presentation methods, eleven `Mapping` fields, eight problem
kinds, all matching one for one. The list above is concentrated where no document enumerates.

### 5.4 Machinery out of proportion

None of these costs anything measurable at run time. What each costs is an invariant a reader has to
confirm is still true.

- **The action registry holds one fact three ways.** `next_id` is always `entries.len()`; each
  entry's stored `ActionId` is always its own index; and `ActionId::info` linear-scans the vector
  that index would subscript. Written once per process, holding tens of rows.
- **`Plan<C>`'s type parameter is phantom.** No field and no method reads `C`. It buys that handing
  context A's plan to context B's state does not compile; it costs `compile`, 130 lines of it,
  monomorphized once per context type — and the three wrappers that hold a `Plan` carry `C`
  themselves already. A judgement call, which is why it is here rather than fixed.
- **`ConsumedControls` is a `HashMap<TypeId, HashMap<Control, &'static str>>` over two schedules.**
  `claim::<S>` is instantiated at `PreUpdate` and `FixedPreUpdate` and nowhere else, so the outer
  map holds at most two entries for the life of the process and `claimant` walks both per lookup.
  Two fields would say the same thing, and the `ScheduleLabel` bound would stop being propagated
  through `evaluate_context`, `release_consumed_in` and `declare_context`'s two arms to key a map of
  two.
- **`MappedPart` caches two facts it could derive.** `scheme` is always `control.scheme()` and `key`
  is always `MappingKey::new(prefix_of(binding), part)` — which `mappings_of`'s follower pass
  recomputes from the same inputs twenty lines later rather than reading.
- **`BindingsTable` and `TunablesTable` are two structs with the same `Serialize` impl**, differing
  only in the inner map's key type — roughly thirty lines to emit `{scheme_name: rows}` twice.
  Chunk 76 is already merging the neighbouring pair.

---

## 6. Drop

Named here rather than deleted, so nobody re-finds them and writes them up again.

- **`GamepadCalibration::clear_device` and `is_empty`, `MappingKey::part`, `Overrides::is_empty`,
  `ActionState::new`.** Ordinary API completeness on small types. "No caller in tree" is not a
  defect for a library; it is only worth acting on for items that are *also* misleading, and none of
  these is.
- **The four `§10.1` citations and the `R14.10` mis-citation.** Real, and worth a minute if
  something else is open in those files. Not worth a pass of their own.
- **`Plan<C>`'s phantom parameter.** The compile-time cost is real and the type safety it buys is
  also real. Nothing has measured either, and this is the kind of thing that should not be changed
  without a measurement.
- **R20.6's sticky modifiers.** A MAY, with no in-tree pressure and no case behind it.

---

## Checked and correct

Recorded so nobody re-derives it. Every one of these was read against the code and holds.

**Values and shapes.** R2.2's conversion table matches `to_bool`/`to_axis1`/`to_axis2`/`to_axis3`
cell for cell, including the two rows the requirement expects an argument about. R2.10's two
hardware cases hold in `Intent::accepts`. R1.1's declared path is required by the derive with no
default. Design §4's fourteen `DiagnosticKind` variants and both `Severity` variants match exactly.

**The frame.** R9.1–R9.5 and R9.7, including the two worth doubting: deltas are summed rather than
replaced (`eval.rs:347`, asserted at `eval.rs:1131`), and events are replayed singly rather than
folded once, which is what keeps a press-and-release inside one window from cancelling. The queue's
append-monotonic invariant survives `clear()`. R11.4's hot-plug policy holds at `eval.rs:422`.
`retire_read_events`, the per-instance cursor and the `level_changes == 0` top-up together give
R9.3 and R9.4, confirmed from both the frame's side and the evaluator's.

**Bindings.** R4.1a's three placements for a mouse button all hold. R4.2's five composites exist.
R4.7's two opposite defaults are both right. R5.1, R5.2, R5.3, R5.5, R5.7 and R5.8 hold. R5.4's
remainder is the netcode row's. D20's stage-1 no-rescale rule holds, clamp included. R12.5 holds by
construction rather than by a filter: keyboard actuation is a set insert/remove keyed on `state`, so
a `repeat: true` event moves nothing. R6.7 holds by construction — a condition's only clock is the
`delta` it is handed. The three enums over one control space were read for proportion and judged
proportionate.

**Evaluation and storage.** R23.5's O(1) holds with no hash on that path. R23.3 holds: activation
sets a flag and fills a vector. R23.7 holds by construction. R7.4's cancellation matches
`Fired | Ongoing`, with the `Started` gap already in `Roadmap.md`. R8.1's chord pre-pass is a pure
function of held state and `is_pressed` refuses a consumed control inside it. R10.2 holds but for
1.1. The three `Fold` kinds partition correctly. R7.5's default half holds for a newly spawned
context by the empty held-state map rather than by the require-reset latch. R24.4's app-build /
runtime split is honoured at both panics.

**Overrides and presentation.** R17.1 holds by construction, which is also D47. R17.7's three states
round-trip and the two bare words cannot collide with a control name. R17.8 holds by construction.
R17.2's tolerance holds on both axes. R19.4's four resets exist. R19.16 holds in both directions.
R19.9 holds at declaration — it is only the rewrite that lowers it, which is 1.2. The tunable pass
runs after the control rewrite and matches scheme as well as key. R18.2's consumption filter reads
only earlier contexts' claims; the sort is stable, so declaration order survives as the last
tiebreak. R18.5's invalidation covers every clause but the layout one its own aside withdraws.
R18.8 and R18.9's origin half hold. The four control tables round-trip exhaustively, unnamed
variants included. R22.6's migration path exists in `docs/comparison.md`. R21.1–R21.3 are met by the
test suite's shape. Capture's arming skips the press that opened the session, and a refused press is
claimed so it does not also play the game. `admissible` asks scheme before reserved before shape.
R15.1's many-to-many holds — neither `Paired` nor `is_claimed` enforces exclusivity, which is what
lets two players share one keyboard.

**Excluded rather than missed**, both already recorded: `apply_overrides_for` discards the rewritten
rows, which is the per-entity presentation deferred row; and `Override::NotOurs` leaves the crate's
binding live rather than silencing it, which is R0.6 and chunk 42's review surface.

---

## What was never done

**One thing the Verification section does not run.** `cargo doc --no-deps --all-features` warns —
`device.rs:8`, a redundant explicit link target. That command is the only one that reads doc
comments, so it belongs in `CLAUDE.md`'s list.
