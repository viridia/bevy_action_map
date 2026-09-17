# Findings awaiting routing

The register of things known to be wrong, missing, or out of proportion, and not yet given a chunk.
A finding lands here whatever turned it up — the six-session scan of `src/` that started the
document, a session that tripped over something, or a defect noticed while writing an example — and
leaves when a chunk takes it. Ordered by how much it matters rather than by what found it.

**What this document admits.** A finding is a claim that something is wrong and a statement of what
someone would observe; it is not a plan. The moment a finding acquires a chunk, the chunk is where
it lives and the entry is retired. Nothing here describes what the crate does, which is
`docs/design.md`'s job, and nothing here is a limitation accepted on purpose, which is a decision
and lives in `docs/decisions.md` where it says what reversing it would cost.

**How to read an entry.** Each says where the problem is, what someone would actually observe, and
whether it was confirmed by running something or only by reading. That last distinction is the
important one, and it is stated per entry rather than assumed.

**Line numbers are as of the entry, and several have since drifted** — `eval.rs:247` is now 257, and
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

**Next: 1062.**

**The calibration warning, stated up front because it is fair.** Most of these entries came from
asking a model to scan `src/`, and a model asked to find sixty problems will find sixty. Some of
what follows is real and some is a rule nobody would ever violate. The tiers below are the honest
attempt to separate them. Where a finding rests on reasoning rather than on a probe, or where the
reachable case is hypothetical, the entry says so in as many words. Where an entry judges a small
cost not worth acting on, that verdict is stated inline rather than collected into a tier of its
own.

**What the tiers mean.**

|                         | Test                                                                                             |
| ----------------------- | ------------------------------------------------------------------------------------------------ |
| **1. Live**             | An ordinary build, an ordinary API call, and the answer is wrong                                 |
| **2. Latent**           | The code is wrong and the path is not taken in tree — a feature exists that nothing has used yet |
| **3. Absent**           | Not wrong, missing, where a requirement or an example's own job says it should exist            |
| **4. Prose**            | A comment or document contradicts the code. No behaviour at stake                                |
| **5. Cost and surface** | Public items nothing asks for, and machinery out of proportion                                   |

This document does not _decide_ routing, which is ground rule 5's business and the author's. It
records routing once it happens: an entry given a chunk says so on its `Fix` line, so what is still
unrouted can be read off the entries that stay silent.

---

## 1. Live — an ordinary build gets a wrong answer

Every entry the `src/` scan filed here has landed (chunks 81, 84, 85, 86, 88, 89, 90, 91) and is
retired. What follows was found another way.

### 1056 `InputDispatchPlugin`, left enabled, bypasses consumption

R8.2a · `bevy_ui_widgets` · carried from `Roadmap.md`, **not re-probed** — that Disasteroids
disables the plugin outright is the strongest evidence here, and it is circumstantial

`bevy_ui_widgets::Button` activates on `Space` from a `FocusedInput<KeyboardInput>` that asks the
mapper nothing, so a focused button answers a control a context has already claimed.

What is wrong is the *default*, not the capability. A context per widget kind answers it, and
Disasteroids ships that way by disabling the plugin outright — so nothing in tree is bitten. What
bites is that `DefaultPlugins` brings the collision and nothing tells a game to opt out, which means
every game meets it once, by surprise, and has to work out what happened.

_Fix:_ the generic form is the **consumption-aware `FocusedInput` dispatch** deferred row, gated on
a game wanting `bevy_ui_widgets`' own widgets working unmodified. What has no home is the smaller
half — saying so somewhere a game reads before it hits this.

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

### 1057 A refused capture is silent on Disasteroids' screen

`examples/disasteroids/settings.rs` · carried from `Roadmap.md`, **not re-probed** — read from the
screen's own code, which never renders `CaptureRefused::reason`

Wrong shape, wrong device family, or reserved: the capture session keeps listening and says nothing
about why the press did not take. A player cannot tell a refusal from a key the game did not hear.

The crate side is built — capture already reports why it refused — so this is the screen declining
to render a reason it is handed. No requirement asks for it; R19 says what a rebinding UI may
legally offer, not what it must say when it says no. It is here because ground rule 3 makes the
examples the acceptance test, and an example that swallows a diagnostic is not demonstrating the
thing the diagnostic was built for.

### 1018 A platform modifier

R12.4: `Cmd` on macOS should be usable as `Ctrl` everywhere else, as a named modifier resolved at
binding time rather than something every cross-platform game re-derives by hand.

Self-contained, independent of 1017.

_Fix:_ **chunk 94c**. (R12.2 and R12.7, the physical-binding layout-label gap, wait on upstream
winit — [winit#4606][] and [winit#2678][] — and are the deferred table's row, not a chunk.)

[winit#4606]: https://github.com/rust-windowing/winit/issues/4606
[winit#2678]: https://github.com/rust-windowing/winit/issues/2678

### 1059 An app cannot edit a chord, because an override rewrites slots only

Drawing and editing a chord is the app's business, not this crate's. What is wrong is that the crate
blocks the second: an override addresses a mapping's `slots`, a chord is not a slot, and no public
type names one. So a game offering a player-editable `Ctrl+S` has nowhere to put the result.

Reading one is not blocked, and was never as blocked as this entry first said. A chord reaches
`Prompt::with` as `ControlOrigin`, which is public and exists for exactly this — so a caption reads
`Ctrl+S` today. What it does not reach is a rebinding *row*: `mappings_of` fills slots from the
binding's primary input and drops the chord, so the same binding captions as `Ctrl+N` and lists as
"N". That half is **chunk 128**.

Confirmed by reading `mapping.rs`, `overrides.rs` and `present.rs`.

No requirement is violated. R12.2 governs layout labels rather than chords, and nothing in R18 or
R19 says a row must name what is held alongside it. R19's rebinding surface is written in terms of
slots throughout, so the omission is consistent rather than an oversight.

_Fix:_ **unrouted**, but cheaper than it looks, and the sketch is worth keeping so nobody re-prices
it. A saved row is already `Vec<String>`, one string per slot, parsed into `Option<Control>` on load
(`SavedRow::Controls`), so a chord rides in the string a slot already has:

```json
"editor.save": ["ctrl+key/KeyS", "alt+key/KeyW"]
```

That leaves `Control` atomic, so the clash pass, `indexed_controls`, capture and reverse lookup are
untouched; what widens is the override layer's own slot type. The parse stays decidable on the
invariant the format already states — every real control name carries a `/`, so modifiers are the
`+`-separated tokens before the first segment containing one, and `char/+` survives intact. Applying
it is free: an override already recompiles a variant plan, and a chord is compiled from the same
spec as the slots.

Two parallel arrays — controls beside modifiers — is the shape to avoid. Chords are per slot rather
than per row, so the arrays must stay index-aligned, and `cleared` would then have a place to be
said in each of them and a way to disagree.

What is left is the genuine cost: the clash pass compares at control granularity and knowingly
over-reports, "two bindings that share a control but differ in their chords are reported as an
overlap" (`capture.rs`). That errs toward harmless noise while chords are fixed. Once a player can
edit one, they can author the overlap it reports, and the trade-off wants revisiting.

A game needing this today re-declares the context with the chord it wants.

### 1060 A chord spanning two device families escapes conflict detection

`binding_family` (`mapping.rs`) derives a row's family from the binding's primary input alone; the
chord is never consulted. So a gamepad binding chorded with a keyboard key files under `Gamepad`,
and since conflicts are per family, that key is invisible to keyboard conflict detection — a player
could bind it elsewhere and nothing would report the overlap. Confirmed by reading `mapping.rs` and
`capture.rs`.

What is wrong is the family crossing rather than the device. `DeviceFamily` has two variants, and a
chord inside either is ordinary: `Shift + Right Mouse` is one family, and so is a gamepad binding
that wants both triggers held, or a shoulder button standing in as a modifier. Only a chord with a
foot in each family has no family to file under, and it is also the combination nobody wants —
`Shift + Left Trigger` asks a player to reach for two devices at once. Nothing in tree writes either
kind today, so this is latent rather than live.

_Fix:_ **unrouted**. A plan-build diagnostic refusing a chord entry whose family differs from the
primary's, which chunk 17b's machinery already supports, and which leaves every same-family chord
alone. Cheap, but it is a new refusal, so it wants a deliberate yes rather than being folded into
whichever chunk next touches chords.

### 1061 An icon prompt drops what the chord requires

`examples/common/prompt_ui.rs` resolves a glyph for `prompt.origin` and returns `Resolved::Icon`,
which holds one icon; `prompt.with` is read only on the text path, by `caption`. So an action bound
to a chord and shown as an icon draws the primary control alone — a player told to press one bumper
when two are wanted. The text path is correct, and the crate hands over everything needed: this is
the example's shared prompt code, not `Prompt` or `Glyph`.

Latent, and reachable rather than hypothetical: no chorded binding in tree carries an icon prompt
today — `SmartBomb` has no prompt span at all — but chunk 128 puts `SmartBomb` on two bumpers, and
adding an `IconPromptSpan` for it afterwards is the obvious next step. Confirmed by reading
`prompt_ui.rs`.

_Fix:_ **unrouted**, and the mechanism wants checking before the shape is decided. `bevy_text` has
an `inline_box` module, which is the name for putting a non-text box inside a text run and would
make a chord one mixed span — glyph, `+`, glyph — rather than a row of siblings. **Not read**; noted
because it would beat the obvious alternative, `Resolved::Icon` holding a sequence of paths with the
text caption as the fallback whenever any entry lacks a glyph. `GhostNode` is the wrong tool either
way: it is `ghost_nodes`-gated and experimental, and it hoists children through *layout* rather than
through the text-span hierarchy, which is what would have to traverse it.

Either shape is the example's to build — arranging glyphs is a layout question `prompt_ui.rs` owns,
which is why this is not a crate change.

### 1019 The types a scene would author carry no `Reflect`, and auto-registration is switched off

R24.3 (MUST) · `action.rs`, `frame.rs`, `Cargo.toml`'s `bevy_reflect` dependency and feature ·
reasoned from `bevy_reflect 0.20.0-rc.1`'s own `Cargo.toml` feature graph, **not probed**

Deriving `Reflect` reads like a gap only manual `register_type` calls can close, but since
[bevyengine/bevy#15030][] a non-generic `#[derive(Reflect)]` type is registered automatically at app
startup — no call needed — unless it opts out with `#[reflect(no_auto_register)]`. A generic type is
the one case that still needs a manual call, per type parameter, because there is no single `TypeId`
to register on its behalf.

That mechanism is inert here, which is the finding. It lives behind `bevy_reflect`'s own
`auto_register_inventory` (or, on platforms `inventory` doesn't support, `auto_register_static`)
feature, bundled into `bevy_reflect`'s `default` set upstream — but this crate's `bevy_reflect`
dependency is declared `default-features = false` and its own forwarded `bevy_reflect` feature never
re-adds either. So today, deriving `Reflect` on `Prompt` or `ActionMapping` would still leave it out
of the registry — not because the crate has to hand-register everything, but because it never turned
on the feature that would do it for free.

`Control`, `DeviceFamily`, `ActionMapping`, `RebindPolicy`, `Prompt`, `ControlOrigin`,
`DeviceHandle`, `ActionObstacle` and `Paired` still carry no `Reflect` at all, which
auto-registration doesn't touch — deriving it is a separate step from registering what's derived.
`Paired` is a component a scene would author, and the five resources `ActionMapPlugin` initializes
are unregistered. `Identity` belongs on that list too: chunk 72 gave it to a device's entity and
left it, like `Paired` beside it, underived.

Two of the scan's supporting observations have since expired, and neither was what the finding
rested on. `overrides.rs` and `device.rs` both derive `Reflect` now, so `action.rs` and `frame.rs`
are no longer the only sites; and `register_type` has callers — `lib.rs`, `device.rs`, and three in
`examples/` — where the scan found none.

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

Unrouted.

### 1021 Accessibility has no citation anywhere in the project

R20, all six requirements, uncited in `src/`, `examples/`, `docs/`, `Roadmap.md` and `CLAUDE.md`.
The section's own preamble calls these "cheap to accommodate now and expensive to retrofit," which
is the argument for looking at it before more is built on top.

R20.2 and R20.5 are built (chunk 64) and R20.1 holds by construction. R20.4 is withdrawn, and R20.7,
the narrower requirement that replaced it, is 1045. R20.6 (MAY, sticky modifiers / one-handed
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

TD6 says "a test or replay harness can drive one directly." From outside, the only way to get an
instance is to spawn an entity and the only way to advance one is `App::update`. The struct's
freedom from ECS references is real and unreachable, and R23.6's standalone half has no citation
anywhere. The netcode deferred row is where this plausibly already belongs. Unrouted.

### 1026 Focus and picking ordering is neither documented nor enforced

R22.4 (MUST) wants documented ordering and integration with `bevy_input::InputSystems`,
`bevy_input_focus` and `bevy_picking`. The `InputSystems` third is met and documented
(`frame.rs:363`, TD1). The other two:

- **`bevy_picking` is named once, about something else.** `docs/decisions.md` mentions it flattening
  its generic `Pointer<E>`, which is a reversal note rather than an ordering. Nothing in `src/`,
  `examples/` or `Roadmap.md` mentions it at all, and no ordering constraint anywhere relates the
  two — which is the third R22.4 asks for. What that clause owes is narrower than "pointer actions
  coexist" sounds. The pipelines are parallel and neither feeds the other, so they contend over one
  signal only: the **buttons**, where a single physical press reaches picking as a click and this
  crate as a bound control (R13.0). Suppressing one side is the app's lever and it has several —
  cursor grab, a barrier entity covering the screen, deactivating the context — so what is owed is
  the ordering and which lever applies when, not a mechanism.
- R22.11 (MUST) — focus changes must resolve before the same frame's actions are evaluated.
  `active_if` schedules `condition.pipe(apply_active::<C>)` in `PreUpdate` `.before(Evaluate)` with
  no constraint against whatever writes `InputFocus`, and `examples/common/widget_focus.rs`'s
  `focus_is` adds none. Disasteroids is not bitten because it disables `InputDispatchPlugin` and
  moves focus from an observer in `Dispatch`, so the write lands after the read by construction
  rather than by an ordering — which is exactly the arrangement that stops holding for a game that
  keeps the plugin. **Reasoned, not probed.**

_Fix:_ the picking half is **chunk 121**, whose captured mode is the ordering being exercised rather
than asserted. The R22.11 half is unrouted.

### 1027 Two documentation requirements with no document

- R16.4 (SHOULD) — the web caveats: pointer lock and gamepad access needing a user gesture, gamepad
  events being polled, key codes and `vendor_id` being less reliable. What the crate says about the
  web is three comments in `device.rs` noting that `vendor_id` is often absent there, which is one
  clause of one caveat, written where it happened to matter rather than anywhere a reader would look
  for the list. Nothing mentions pointer lock or the user gesture.
- R16.5 (SHOULD) — name the OS-reserved combinations that are unavailable. Nothing.

Unrouted.

### 1028 `tracing` is contradicted by a dependency choice recorded only in `Cargo.toml`

R22.3 (SHOULD) wants spans and events at the sampling and firing boundaries. What exists is six
`log::warn!` sites, all app-build or misconfiguration.

The crate depends on `log` rather than `bevy_log`, and the comment in `Cargo.toml` gives the reason:
`bevy_log` installs a `tracing-subscriber` and is `std`-only. That is a decision by
`docs/decisions.md`'s own admission test — name what breaks if reversed, and the answer is R22.3 or
the `no_std` build — and that document does not carry it. Unrouted.

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

Unrouted.

---

## 4. Prose — a comment or document contradicts the code

No behaviour at stake. All are small, and the reason to do them together is that a reader trusting
any one of them is misled about a mechanism.

### 1031 Examples and sketches that do not compile

- TD3's trait sketch says `// plus CATEGORY and CONSUME, with defaults`. The constant is `CONSUMES`.
  Copying the sketch into a hand-written impl does not compile.

Unrouted.

### 1032 Internal comments whose stated reason is false

- `binding/builder.rs:24`, `BindingSpec` justifies its copies with "the plan keys state by
  `ActionId`, which does not reach back to the type." `ActionId::info` reaches back to exactly the
  three fields the comment is justifying. The copies are still right — `info` takes the registry
  lock and linear scans — but the stated reason is the one a reader would use to decide whether the
  duplication may go.
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
- **TD6's "a test or replay harness can drive one directly"** — see 1025.
- **TD7 does not state R7.3's cost.** Two simultaneously-active layers hold separate action state,
  so a game reading `ContextActions<Base>` does not see the layer's answer and has to read both.
  R7.3 is a MUST that is met and claimed nowhere.
- **A `TD10.1` citation points at the wrong section**, at `context/declare.rs:761`. It is about the
  override store being keyed by mapping alone, which is TD10's preamble; TD10.1 is "Applying," and
  this is not about applying. Two others, both in `overrides.rs` and both about the serialized form
  (now TD10.3), went with 117e. **Small — worth a minute alongside the R14.10 mis-citation above,
  not worth a pass of its own.**

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
behind it," as the register puts it, does not only fail to prevent them. Unrouted.

### 1036 Public items with no caller and no document

Splitting by why they are here, because the answer differs:

**Should probably be `pub(crate)`** — used only inside the crate, while a sibling doing the same job
is already crate-private:

- `eval.rs`: `release_consumed_controls`, `release_consumed_in`, `dispatch_transitions`,
  `dispatch_class_fires` — `pub` with `ActionMapPlugin` and `declare_context` as their only
  registrars, beside `reset_exclusion_ceiling` and `evaluate_context`, which are `pub(crate)` and do
  the same job in the same file.
- `device.rs`: `warn_on_unread_gamepad_settings`, and `frame.rs`: `retire_read_events` — same shape,
  only `InputFramePlugin` names them.
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
- `overrides.rs`: `Overrides::is_empty` — called only by its own test; TD10 enumerates twelve
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
  docs.rs as a struct a reader can name and do nothing with. TD4 names `Plan<C>` in prose, which is
  architecture rather than a request for it to be public.

**Reviewed and left alone**: `GamepadCalibration::clear_device`/`is_empty`, `MappingKey::part`,
`Overrides::is_empty` and `ActionState::new` are ordinary API completeness on small types. "No
caller in tree" is not a defect for a library; it is only worth acting on for items that are _also_
misleading, and none of these is.

Worth stating for calibration: TD7.3, TD8.2, TD9.1 and TD10 _enumerate_ their public surface rather
than describing it, so the sweep over those was a diff and came back nearly empty — eleven
conditions, ten modifiers, six presentation methods, eleven `ActionMapping` fields, eight problem
kinds, all matching one for one. The list above is concentrated where no document enumerates.

Unrouted.

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
- **`BindingInput::for_each_part` always visits exactly once.** A composite expands into a binding
  per part, so every input is one part holding one control, and the callback, `MappedPart` being
  collected per part, and `binding_family` returning an `Option` all describe more than one. The
  method is public, so narrowing it to a single `(BindingPart, Control)` is a breaking change.

Unrouted.

### 1038 One command the Verification list does not run

`cargo doc --no-deps --all-features` warned twice when the scan ran — a redundant explicit link
target in `device.rs`, and a link to `GamepadBrand::Generic` in `present.rs` with nothing importing
`GamepadBrand` into that scope. **Both are fixed**: rebuilt from a touched `lib.rs`, the command is
now warning-free.

The finding survives the fix, because it was never really about the two warnings. That command is
the only one in the project that reads doc comments at all, and it is not in `CLAUDE.md`'s
Verification list — so nothing would have caught either warning, and nothing will catch the next.

_Fix:_ **chunk 28**, which adds the command and extends this to the no-devices build, where eighteen
intra-doc links `--all-features` resolves go unresolved.

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
binding live rather than silencing it, which is R0.6 and chunk 112's.
