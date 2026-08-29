# Work log: `bevy_action_map`

> What has been built, in the order it was built, and what building it taught us.
>
> This document exists so the other two do not have to carry it. [Requirements.md](./Requirements.md)
> says what must be true and why; [Roadmap.md](./Roadmap.md) says what is left to do. Neither needs
> to explain how we arrived at them, and a reviewer coming to this project cold should be able to
> read those two and skip this one entirely.
>
> Read this when you want to know **why a requirement says what it says** beyond the rationale given
> in place, or when a decision looks arbitrary and you suspect there was a reason.
>
> **[Log-archive.md](./Log-archive.md) holds the closed entries** — everything whose obligations are
> stated elsewhere and which nothing left in the sequence reasons from. What stays here is what the
> work still in flight is built on, so this file is short by construction rather than by age.

---

## The shape of the record

Two kinds of entry appear below, and the distinction is the useful one:

- **A chunk landed and taught us something.** The lesson usually became an amendment to the
  requirements or a new item on the roadmap, and the entry says which.
- **A chunk landed and claimed more than it delivered.** These are recorded at least as carefully,
  because the pattern in them is the most useful thing this document contains.

---

## Phase VII — the presentation model

### The forgiveness grooming, before chunk 34

Outside feedback that the crate shoulders responsibility that belongs to the app named two
examples: disabling an action (R3.7, chunk 35) and forgiveness windows (R6.5, chunk 34). The two do
not hold up the same way under that question.

**R3.7 stays.** "The app just ignores the event" does not reproduce what disabling buys: R3.6 gives
a bound action consumption priority over lower-priority contexts, so an ignored-but-still-bound
action keeps its control away from anything else bound to it — exactly the gap R3.7 exists to close
without requiring an unbind. Chunk 35's own note already found the mechanism mostly built
(`require_reset`, a `StateFlags` bit); what is missing is the public verb, so the cost of keeping
this is close to zero.

**R6.5 is withdrawn.** Buffering and coyote time, in the form named, turn on a crossing the crate
cannot see: landing, leaving the ground, or whatever app-domain transition "valid" means for a given
action, is state the crate has no view into, so a crate-side condition could not implement the named
pattern either, only a narrower piece of it. That piece — "was this control active in the last N
ms" — is already answered by what §3 ships: R3.2's events give the press/release edges, R3.4 gives
elapsed time in the current state measured in the app's own simulated seconds, and recording a
timestamp off an event the app already receives is composition, not a missing primitive. Chunk 34
keeps R6.4's ordered sequences, a different question — the ordering is between the crate's own
actions, never against app state — and drops the forgiveness half entirely. Requirements.md marks
R6.5 withdrawn in place, same convention as R2.4.

### The first grooming sweep, before chunks 62–64

Prompted by tunables (R19.11) having a deferred-table row that argued against its own gate.
Grepped `src/` rather than trusting the documents' account of themselves, and three more MUSTs
turned up with no destination anywhere — not even a deferred-table row: R16.1–R16.3 (focus loss and
suspend release held controls), R11.4 (a disconnected device releases them the same way), and R13.5
(input frames carry no source window to filter a binding on). Chunks 62 and 63 are the destination
for those; chunk 64 is tunables' own, with R20.2's hold-vs-toggle riding along since R19.11 already
names it as the worked example.

**R11.6 (device brand/class resolution) is flagged, not chunked.** Loosely covered by the glyphs
row's asset-pipeline gate but never named in it — left for the next sweep rather than forced into a
chunk on a guess.

**Not exhaustive.** §9, §22, and §23 were not checked against code this pass. The author asked for
another sweep later rather than one pass covering everything now.

### A second widget kind, and a focus-claiming bug worth fixing upstream

`StepperFocused` (`examples/common/widget_focus.rs`) followed `ButtonFocused`'s exact shape — a
focus-kind-activated context, `WidgetKind::STEPPER` a required component of a new `Stepper` marker,
an `Adjusted` event fired at the stepper's own entity — for a widget kind `bevy_ui_widgets` does not
ship: a numeric stepper with a decrement and an increment chevron, bound to `-`/`+` on the keyboard
and the pad's D-pad left/right. R22.8's proof is no longer a single data point; two independent widget
kinds built from nothing but `active_if` and an ordinary context is the pattern holding, not a
coincidence of the first one.

**The D-pad half needed `.consume()`, not a new mechanism.** The fold already treats a claimed
control as unactuated for every lower-priority binding that reads it, composites included — "one
part of a composite going away should leave the other three working" is a comment already in
`eval.rs`, not something this session added. What was missing was simpler: `StepperFocused`'s
`Adjust` bindings never called `.consume()`, so nothing was ever recorded in `ConsumedControls` for
`Menu`'s `Navigate` (reading the same D-pad as one four-button composite) to see. Once claimed,
up/down keeps moving the row selection and only left/right are taken — exactly the degradation the
fold was already built for.

**Mouse clicks on the stepper's chevrons blinked the focus ring, which turned out to be a real bug
in already-committed code, not something new.** `focusable()`'s `claim_focus` (chunk 31) reclaims
`InputFocus` reactively, when `Activate` fires at pointer release — and its own doc comment already
named the reason: `bevy_input_focus`'s `click_to_focus` fires a bubbling `AcquireFocus` on pointer
*press*, before `bevy_ui_widgets` decides whether a click landed, and since this game's selection is
`AutoDirectionalNavigation` rather than `TabIndex`, nothing intercepts that request — it bubbles to
the window and clears focus. Reclaiming after the fact patches the symptom but not the gap: for a
plain button, press and release land on the same entity, so the clear-then-reclaim happens within a
frame or two and was never reported as visible. A stepper's chevron is a different entity from the
stepper itself, so the same race has a whole gesture to be seen in.

The fix moved the claim earlier: `acquire_focus_directional`, a global observer on `AcquireFocus`
that stops propagation and claims focus the instant a press lands on anything carrying
`AutoDirectionalNavigation` — the same job `bevy_input_focus::tab_navigation::acquire_focus_tab_index`
already does for `TabIndex`, just keyed to this game's own navigation marker instead. `claim_focus`
is gone; nothing needs it once the request never reaches the window.

**The author intends to raise this upstream.** `PointerFocusPlugin` bridges `TabIndex`-based
navigation for free (`acquire_focus_tab_index` is installed by the plugin itself), but a game using
any other focus-navigation scheme — `AutoDirectionalNavigation` here, and anything else a third
party writes — gets no equivalent bridge and no signal that it needs one; the natural-looking fix
(reclaim on the widget's own activation event) is the one that leaves the gap open. No issue filed
yet, unlike [bevy#25592][] — recorded here so the finding survives until one exists.

### Chunk 64: Tunables and hold-vs-toggle

R19.11 and R20.2 landed together: a named, typed value declared on a binding, enumerable the same
way a mapping is, persisted the same way, and applied by the same variant-plan recompile a rebind
already uses.

**R20.2 turned out to be an accident worth keeping, not a gap worth closing quietly.** Both it and
`hold_or_toggle` (R19.11's own worked example) trace to the project's first commit — a
bulk-drafted requirements pass, not a line-by-line ask — and the author said as much when the
session surfaced it. Kept anyway: hold-vs-toggle is a well-established accessibility
accommodation, and the session's own finding was that it costs little beyond the mechanism the
deadzone tunable needed regardless, because both value shapes reduce to the same operation —
overwrite one field on one modifier a binding already has, then recompile.

**Two shapes only, not R19.11's five.** `Range` (a deadzone amount) and `Bool` (hold-vs-toggle)
cover both tunables anything in tree wants. `invert_y` and a curve preset stay unbuilt — the same
"gated on a real consumer" reasoning the deferred table already used elsewhere, not a limitation of
the mechanism; a third shape would be an N-way `Choice`, and nothing has asked for one yet.

**Hold-vs-toggle is a modifier, not a condition.** `BindingModifier::Toggle { active: bool }` sits
beside `DeadZone` — identity when inactive, and when active it reports a latch instead of the raw
value. Every condition downstream (`Down`, `Press`, `Hold`) reads whatever the chain produced
without knowing which mode is in effect, which is what lets it ride in under R20.2's own
description — "a binding-level option, not reimplemented per action" — without touching
`condition.rs` at all.

**The scope boundary the settings screen had already written down held.** Disasteroids'
`Prefs.dead_zone` stepper carried a comment saying its wiring into `Turn`'s actual binding was
chunk 22's job, not something a bare resource could do. That gap is now closed for a different
reason than a resource's limits — `tunable_dead_zone` reaches the binding just fine — but chunk
22 still owns it: stage 3's preference clamp needs a floor derived from stage 1's per-device
calibration, and this chunk has no calibration data to clamp against. Declaring the tunable
without that floor would let a player turn the deadzone off entirely, so the deadzone tunable
landed declared and enumerable but disconnected, exactly as the comment already said, and chunk
22's own roadmap entry now names the mechanism it inherits rather than needing to build one.

**Using the game found what the model of it missed, twice, before the worked example was actually
right.** The first cut declared `hold_or_toggle` per binding — chained onto `Thrust`'s primary
keyboard key alone — and playing it exposed the checkbox lying: labelled "Thrust", it governed
only `KeyW`, and neither the secondary key (`ArrowUp`) nor the gamepad trigger moved with it. Two
separate findings came out of chasing that down, and both changed the shipped design rather than
patching the symptom.

**First: hold-vs-toggle is a fact about the action, not about which control drives it, so it is
now declared once per action.** `InputContextBuilder::hold_or_toggle::<A>(key)` replaces the
per-binding method chunk 64 first shipped — scanning `A`'s bindings declared so far (the same
ordering rule `follow` already has) and wiring every eligible one to one key itself, rather than
asking the caller to repeat the same string on each binding and trust it to stay in sync.
Eligibility turned out to need the action's own intent, not just the control's shape: a
`GamepadButton` reads as `Bool` only when the action wants a plain press and as a continuous
`Axis1` fraction otherwise (`BindingSource::GamepadButton` in `eval.rs`, R2.10's own duality), so
toggling a trigger feeding an analog action would silently flatten real analog data. `Thrust` being
`Analog1` overall does not disqualify it from `hold_or_toggle` at all — only a per-binding check
against the source and the intent together does, which is what `always_reports_bool` is.

**Second: reaching more than one binding needed a real shared latch, not just a shared key — and
the first attempt at one was actively wrong, not just incomplete.** Sharing a `BindingModifier`'s
own private `Scratch` slot between bindings (swapped in for the call, swapped back after) reads as
plausible and is not: two bindings visited in the same tick each do their own edge detection
against the one cell, so whichever runs second sees a "previous value" the first one just
overwrote with its *own* raw state — and a binding that is not the one currently pressed keeps
resetting that shared "previous" back to false every tick, which reads as a fresh press the moment
the other binding's own state changes and re-flips the latch on ticks nothing the player did
changed. A test pinning "press key A, release, press key B" caught it: the fold read a stale
pre-flip value because bindings were resolving the group's edge one at a time rather than once for
all of them. The fix mirrors `chord_claims`, which already solves the same shape of problem for a
different reason — resolved once per tick, before any binding's own evaluation, from the OR of
every sharing binding's raw actuation combined; every member then simply reads the resolved bit
back out of a plan-level shared scratch table (`Plan::tunable_scratch_count`,
`CompiledBinding::tunable_shared`) instead of running its own chain at all. `tunables_of` merges
bindings sharing a key into one presentation row the same way `mappings_of` already merges a
primary and a secondary, and `diagnose` gained the tunable-key analogues of `DuplicateMappingKey`
and `RebindingDisagreement` — two different actions sharing a key, or two bindings of one action
disagreeing about the tunable's shape, are both build-time errors now rather than silent.

**The scope question the "reaches every scheme" framing raised got asked and reversed.** Given a
real mechanism for sharing one latch across bindings, the natural next question was whether the
gamepad trigger should join the keyboard's shared latch too, or get an independent one, or stay
untouched. Shipped-game prior art settled it: hold-vs-toggle is almost always one setting per
action, not split per device, unlike sensitivity or deadzone (R20.5's own text says "per device";
R20.2 never does) — and the trigger already has the better answer to the fatigue a toggle exists
to solve, since it can rest at partial travel instead of being held fully down. `Thrust` lands with
`hold_or_toggle` reaching `KeyW` and `ArrowUp` only, sharing one latch between them; the trigger is
untouched, both by the eligibility check (it fails `always_reports_bool` for an `Analog1` action
regardless) and by choice.

**A test-harness bug cost more time than the actual fix.** The first version of the shared-latch
test built a fresh `InputFrame::default()` per simulated keypress, which reset the frame's read
cursor each time and made every event after the first look already-read to `apply_frame` — the
test saw the *first* key's raw state forever and nothing else, which looked exactly like the
spurious-reflip bug above until traced through by hand. One frame, reused across calls with
`.record()` accumulating events on it, is the pattern every other test in `eval.rs` already uses;
this one just did not follow it at first.

**Doctests remain the one thing this session could not verify by running them** — the pre-existing
`dynamic_linking` gap chunk 28 owns — and the GUI could be built but not driven headlessly, so the
controls-screen checkbox is verified by what applies it (`apply_overrides`, exercised in tests) and
by a clean build, not by a screenshot.

### Chunk 67: Per-entity `apply_overrides`

Split off chunk 26's own inherited question (from chunk 38) rather than left for chunk 26 to answer
speculatively: a split-screen app persisting two players' `Overrides` independently needs each one
applied to only its own player's entity, and the existing `apply_overrides` is world-wide by
construction.

`apply_overrides_for`/`apply_overrides_for_with_preset` are a second entry point onto
`InputContextState::adopt`, the exact machinery a rebind already uses, not a second implementation.
`apply_to_entity::<C>` is `apply_to_context::<C>`'s body with the `AppliedPlan<C>` write dropped and
`adopt` called on one named entity instead of every instance a query finds. Both `InputContextPlan<C>`
(the pristine declaration every diff is taken against) and `AppliedPlan<C>` (what a freshly spawned
instance inherits) stay untouched, so two entities diverge independently without either becoming the
new default, and a third instance spawned afterward still reads the world's unmodified default.

**The one test failure this session produced was not a bug in the new code — it was `adopt`'s own
require-reset semantics, working exactly as documented, against a test that did not yet know about
them.** A first draft applied the per-entity override and pressed the newly-bound key on the very
next frame, expecting it to fire; it read `Idle` instead. `adopt` re-arms require-reset on every slot
so that a player still holding the key they just rebound cannot get a fresh press out of the swap
(R7.5), and the fold only clears that flag on a tick where the slot reads at rest — which the press
itself is not. The fix was an intervening idle `app.update()` between the apply and the press, not a
change to `adopt` or the new entry point — useful confirmation that per-entity `adopt` runs through
the exact same activation lifecycle the world-wide path does.

**Left as a stated gap rather than built:** presentation stays world-wide.
`read_mappings`/`read_bindings`/`PromptScope` are all keyed by context *type*, so once two entities
of one context diverge, nothing can yet ask "what does *this* one currently show." `present.rs`'s own
doc comment on `BindingTable` already names the shape of this gap ("a per-player record"); it is
recorded in Roadmap.md's deferred table rather than answered here, since nothing in tree needs it yet.

### Chunk 26: Device routing (core)

§15 made real, cut down to what single-player owes nothing for and local co-op only has to make
*possible* — the roadmap's own framing for why this landed narrower than its original scope (once
"device routing and the join flow" together, before the join gesture split off as chunk 66 and
per-entity overrides split off as chunk 67, both alongside this session's own work).

`DeviceHandle` (`device.rs`) is a runtime-only handle — the keyboard and mouse as one
`KeyboardMouse` value, a gamepad as the backend's own `Entity` for it — explicitly not persistent,
so nothing reaches for it across a reconnect. `DeviceHandleSet` backs it with a `smallvec` inline
array rather than a hard cap, and is a plain value type with no component derive, deliberately: that
is what lets `Paired` (`player.rs`), the component that actually attaches one to a context entity,
expose it through `Deref` for `.contains()`/`.owner_for()`, and what will let a future presentation
filter take a set by value without threading `Option<&Paired>` through a query tuple.

**The device dimension does not touch `Control` or the plan.** `RawEvent::device()` (`frame.rs`)
answers "which device produced this," mirroring `control()`'s existing cfg pattern; the one new
filter is in `InputContextState::apply_frame`, which now takes `pairing: Option<&Paired>` and drops
any event whose device the pairing does not claim before anything else — not in `evaluate_context`,
which needed only a widened query to pass that component through, and not in `ConsumedControls` or
`ExclusionCeiling`, both untouched (see below). An unpaired instance's filter is `pairing.is_none_or`
against `None`, which is unconditionally true, so a game that never mentions `Paired` reads every
device exactly as it always has — bit-for-bit, not just in intent, since the filter predicate itself
degenerates to a no-op rather than being skipped by a branch.

**One `cfg` wrinkle, resolved by checking rather than assuming.** The design sketch gated
`DeviceHandle::KeyboardMouse` on `any(keyboard, mouse)`, mirroring `Control`'s own gating. But
`RawEvent::MouseMotion` — the variant `device()` must answer for — is deliberately *not*
feature-gated (`frame.rs`'s own comment: "keeps this enum inhabited when every source feature is
off"), and an existing test (`records_mouse_motion_events`) constructs one with no `cfg` at all,
confirming it really is reachable with every device feature off. `KeyboardMouse` needed the same
"always inhabited" treatment, not the gate `Control` uses — the two enums answer different
questions and don't have to share a shape.

**This alone satisfies R15.3's literal text** ("a device's input must not reach a player who does
not own it... enforced at L1/L2") and fixes, for any instance that does pair, the
`held_gamepad_buttons`/`held_gamepad_axes` flat-map bug the roadmap had already named: a disconnect
clears only the paired device's readings, because the event never reaches an instance paired
elsewhere. Nothing in tree pairs yet — Disasteroids is single-player and unpaired, so the
known-wrong-today row in Roadmap.md still describes what a reader will actually see until chunk 27.

**Deferred rather than built, on the same reasoning the plan gave rather than reasoned fresh here:**
owner-scoping `ConsumedControls`/`ExclusionCeiling` (R15.3's cross-context remainder) and per-entity
presentation both went to Roadmap.md's deferred table with a stated gate — a real in-tree case
neither has today, since Split Friction (chunk 27) declines rebinding UI and pairs through chunk
66's `JoinSession` rather than a declared `InputContext`. `evaluate_context` needed no change at all
as a result: `shadowed`/`ceiling.raise` stay computed once per context type, not per pairing.

**Test plan followed the roadmap's own two-part shape exactly, and it was right to insist on both.**
A pad-and-a-keyboard test exercises the mixed case (the two devices share no code path, so a routing
bug there cannot hide behind symmetry); a same-model two-pads test exercises identity, where kind
alone tells a filter nothing and only the device handle distinguishes them. Both are in `context.rs`
alongside the crate's other full-`App` gamepad tests, driving two spawned context entities through
real `evaluate_context` scheduling rather than calling `apply_frame` directly — `InputContextState`
is queried back per entity the same way chunk 67's own per-entity test already reads it.

[bevy#25592]: https://github.com/bevyengine/bevy/issues/25592

### Chunk 66: The join gesture

Landed with almost none of the code its own Roadmap text proposed, once R15.4's own words —
"observe input from unassigned devices (*with bindings applied*)" — were read literally rather than
through the roadmap entry's own paraphrase ("reusing capture's own `arrival()` classifier"). That
paraphrase misread its own citation: `arrival()` is a raw, un-bound classifier ("any button, from
anywhere"), which answers nothing about "bindings applied," and reusing it would have needed a
second per-device evaluation path running parallel to the one §3 already runs, just to get device
identity back out of it.

What R15.4 actually wants was already sitting in chunk 25's class-binding mechanism, unused for
this. `bind_class::<Join>(ControlClass::AnyButton)` — or a narrower class — declares "join" as an
ordinary action on an ordinary context. Left with no `Paired` of its own, that context reads every
device exactly as any other unpaired context does (chunk 26's own behavior, unconditional).
`ClassFired`'s event is the untouched `RawEvent` a class binding already dispatches, and
`RawEvent::device()` — the same method `apply_frame`'s own pairing filter calls — answers which
device fired it. Neither half needed new code.

The one real gap: nothing filters a class binding's dispatch against the world's `Paired` set,
because `Paired` did not exist when class bindings landed. `join::is_claimed` (`join.rs`) is the
whole of the new code — an iterator over `&Paired` checked against one `DeviceHandle` — meant to be
called from the app's own `ClassFired` observer rather than threaded into `apply_frame` itself: an
unpaired join context already reads every device on purpose, and pushing "already claimed" into the
sampling filter would only move one `.any()` call the app writes anyway into the crate, in exchange
for needing a way to say "read everyone except the claimed ones" that nothing else here needs.

**Lesson for the record:** a roadmap entry's own paraphrase of a requirement is not the requirement.
`arrival()` reuse read as settled and cited an R-number, and was still the wrong mechanism — caught
only by rereading R15.4's actual clause against what `arrival()` actually does, and by checking
whether an existing mechanism (class bindings) already answered the question before writing a new
one.
