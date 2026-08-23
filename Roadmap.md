# Incremental build plan: `bevy_action_map`

> This document orders the work described in [Design.md](./Design.md) into chunks small enough to
> review individually. It is a _sequence_, not a schedule — there are no dates, and any chunk may be
> revised or reordered once the one before it has been read.

## Ground rules

1. **One chunk, one reviewable change.** Each chunk below is meant to be a single branch: code, its
   tests, and any doc changes it forces. If a chunk turns out to be more than roughly a day's reading,
   it gets split before it gets written.
2. **Every chunk is verifiable on its own.** Pure-data chunks get unit tests; chunks that touch ECS
   get either a headless `App` test or a runnable example. No chunk lands whose only justification is
   "the next one needs it".
3. **The examples are the acceptance test.** From chunk 4 onward there is always something to run.
   When a later chunk is an internal change, the criterion is that _the examples do not change_ —
   a diff in `examples/` during a refactor chunk is a signal the abstraction leaked.
4. **Deliberate omissions are stated.** Each chunk lists what it does _not_ do, so review can tell
   "not yet" from "overlooked".
5. **Nothing outstanding is left without a destination.** A chunk that lands short of its own
   description says so in the [work log](./Log.md), and the obligation is written onto the chunk
   that finishes the job — so what a chunk owes is stated where someone will read it, rather than
   having to be gathered from the chunks that incurred it. "Its own decision" and "later" are not
   destinations: an item with no chunk number is an item that will be dropped.

---

## Commit Messages

To be compatible with Bevy's AI policy (not just the policy but the discussions that preceded it):

Commit messages should not say "Co-authored by" an LLM. Rather, it should
have a section "LLM Usage Disclosure", which briefly explains the role the
LLM played in crafting the commit.

---

## House style

This crate is a candidate for eventual upstream inclusion, and game developers as a class are
sensitive to text that reads as machine-authored — a contribution that reads that way invites
controversy independently of whether the code is correct. So the standard is: **the code should read
as though a maintainer of the surrounding codebase wrote it.**

**Comments.** Internal comments are terse. Don't explain what a maintainer already knows — ECS
semantics, borrow rules, standard Bevy behavior. Comment the non-obvious decision: the thing that
would break if someone changed it. One or two lines, not a block explaining the mechanism.

**Doc comments are the exception.** Verbosity is welcome on `pub` items. This is a library whose
public documentation is part of the deliverable (R24.6), so doc comments can be full and
explanatory in a way internal comments must not be.

**Analysis belongs in the review conversation, not the source file.** The reasoning that produced a
design — why an alternative was rejected, what the tradeoff was — goes in the chunk's discussion.
When it genuinely needs to persist, it goes in [Design.md](./Design.md) or the requirements, both of
which are built for it. What it must not do is accumulate as prose in the code.

**Avoid the tells:** restating what the code says, hedging, enumerating the obvious, unusual
punctuation or phrasing in comments.

**Prefer sketches over applied refactors.** Small, staged, individually reviewable edits — which is
the same principle as Ground rule 1, applied within a chunk rather than across chunks.

**Comments should be addressed to the right audience**. _Doc comments_ are meant for public-facing documentation, targeted at game developers who want to use the
crate. These needs to explain basic concepts, usage examples, and in some cases,
educate the user as to why a function or type is important. They should not explain how the implementation works. It should not reference roadmap stages, design decisions, or numbered requirements, or in general discuss the development status of the project - users don't care about this.

_Internal comments_ are meant for people working on the crate, the author, maintainers, and agents who need to understand the code. This can explain
theories of operation and call out subtleties; if necessary it can go into
detail about an algorithm or data structure. However, it should avoid explaining
basic information that any bevy maintainer or rust programmer would already know.

---

## Cross-cutting: the timestamp shim

§2 of the design drains a timestamped event queue by time window. Bevy's input events carry no
timestamps ([bevy#9087][] is the upstream fix, still open), so we need a stand-in — and the whole
point of naming it here is to keep it in **exactly one place**.

Chunk 3 introduces a `Timestamp` newtype and one function that produces it. Until #9087 lands, that
function returns a monotonically increasing sequence number tagged with the frame it arrived in.
Consequences, which should be documented in the module and not papered over:

| Property                                  | Under the shim                    | Requirement |
| ----------------------------------------- | --------------------------------- | ----------- |
| Ordering within a frame                   | exact                             | R9.7        |
| No lost edges across a 0-tick frame       | holds                             | R9.3        |
| No duplicated edges across a 3-tick frame | holds                             | R9.4        |
| Delta magnitude conserved across windows  | holds                             | R9.5        |
| Sub-frame timing accuracy                 | **degraded to frame granularity** | R9.8        |

Everything downstream reads `Timestamp` and never `Instant`, so #9087 lands as a change to one
function plus the removal of a caveat. Gamepad stays frame-quantized regardless until gilrs polling
is rewritten (§11), so mixed fidelity across sources is permanent for now, not an artifact of the shim.

---

## Where this stands

Chunk numbers are **stable identities, not positions**. Ground rule 1 lets a chunk be reordered once
the one before it has been read, and several have been; the phase headings below carry the sequence
so that "chunk 8" still means what it meant in the discussion that produced it.

The target the remaining sequence aims at is **Dead Zone** — an asteroids-like game in primitive
shapes, playable on keyboard or gamepad, eventually with a rebinding screen built on
`bevy_ui_widgets` and operable from the controller. It is not a phase of its own. It arrives early,
badly, and grows a capability per chunk, because ground rule 3 wants something runnable at every
step and a real game is a better acceptance test than a synthetic one.

| | |
| --- | --- |
| **Works today** | Actions and contexts as types; keyboard, mouse and raw gamepad into an input frame; per-entity context state; N bindings per action folded by intent; the design-stage deadzone; render/fixed evaluation ordered ahead of its readers; each context draining the frame from its own cursor; the three-property model — a source's channel shape checked against the action's intent, with the conversions between shapes settled. |
| **Known wrong today** | Context priority is declared and ignored: two contexts binding one control both see it, and neither can consume it (chunk 14). |
| **Never built** | Arbitration and consumption, the whole player-facing model. |

---

## What has landed

Fifteen chunks are done. The [work log](./Log.md) says what each delivered, what it found, and where it
fell short of its own description; this table is only an index, and the sequence below is what
remains.

| # | Chunk | State |
| --- | --- | --- |
| 1 | Workspace and module skeleton | done |
| 2 | Action identity, value, and intent | done |
| 3 | Derive macros | done |
| 4 | Input frame, keyboard only | done |
| 5 | First end-to-end slice | done, after repair |
| 6 | Axis sources and composites | done, completed by 15 |
| 7 | Modifiers | done; stateful modifiers → 11, `Reflect` → 17 |
| 8 | Gamepad and the design-stage deadzone | done; stages 1 and 3 → 22 |
| 24 | Housekeeping | done; doctests still deferred |
| 9 | Tick domains and the windowed drain | done; the L2 half of R9.3 → 12 |
| 15 | Source channel shape | done |
| 16 | Dead Zone, first playable | playable; no death, which is polish |
| 12 | Transition log and observers | done |
| 13 | Context activation lifecycle | done; priority → 14 |
| 11 | Conditions and the scratch table | done; chords → 14 |

Every obligation those chunks left is carried by the chunk that has to discharge it, below, rather
than by the chunk that incurred it — so what a chunk must do is stated in one place.

---

## Phase V — multiple contexts

Dead Zone grows a pause menu, which is what forces all of these.

### 13. Context priority, layering, and activation **[PARTIAL]**

Dead Zone's pause menu is what forces this, and it also covers the case player death would have
covered — an interstitial screen with a different context active. Death is therefore polish, and
stays out of the sequence: it would be a second demonstration of the switch this chunk already makes.

- **Delivered:** the activation lifecycle. A context can be stood down and brought back; standing it
  down cancels whatever was in flight rather than leaving a hold stuck for as long as the menu is up
  (R7.4); bringing it back ignores controls the player is already holding, with an opt-out for the
  case where a context is taking over from one that was driving the same stick (R7.5). An inactive
  context keeps tracking its devices, so coming back costs nothing (R7.6). Dead Zone pauses and
  resumes on one key bound in both contexts.
- **Outstanding → chunk 14:** priority does nothing yet. `PRIORITY` is declared on every context and
  ignored, because what it orders is arbitration between contexts binding the same control, and
  that is chunk 14's single-pass algorithm. R7.1 is half-met: the order is explicit, not yet
  inspectable or effective.
- **Outstanding → chunk 14:** additive layers (R7.3). A layer is a higher-priority context binding a
  subset of the same controls, so it should fall out of arbitration rather than need a mechanism of
  its own — which is a claim worth testing there rather than asserting here.
- **Outstanding → chunk 17:** an analog action whose control has no deadzone can never satisfy
  require-reset, because a stick that idles at 0.02 is never at rest, and the action stays quiet
  after every activation. Documented on `activate`, but a binding with an analog source and no
  deadzone is something plan-build diagnostics should say out loud.
- **Also delivered, after the first attempt read badly:** `add_context_in_state` ties a context's
  activation to a Bevy state, which is where most of them belong — a pause menu is the poster child.
  The first version had the game hold both facts, a state and two hand-driven contexts, and keep
  them in step; the version that shipped has the contexts follow, so there is one fact and nothing
  to disagree. Declaring a context that starts inactive falls out, which was the gap this chunk
  would otherwise have left.
- **Activation is declared per context *type*, and that is the decision rather than a stopgap.**
  `bevy_enhanced_input` binds it per *entity* instead — an `ActiveInStates<C, S>` component naming
  the values — so two instances of one context can follow different states. More capable, and the
  extra capability is speculative: the per-instance case is already served by calling
  [`activate`](InputContextState::activate) on the instance, which is imperative but is also the
  thing a game reaches for when it has the entity in hand anyway. What would be gained is a
  *declarative* per-entity binding, and no case has yet been named where that beats the method.

  Revisit if Split Friction turns one up — two players whose contexts genuinely want to follow
  different states, declared rather than driven — and not before.

- **Outstanding → chunk 13 itself:** R7.7's mutually-exclusive stack. States cover the common case
  of it, so what remains is whether a stack that is *not* a state is worth its own mechanism.

### 14. Arbitration and consumption

The single-pass consumption algorithm (R8.3); chords beating their component bindings; the
"why didn't this fire" diagnostic query (§9.5's third tier).

- **Gates the rebinding UI.** R19.3 requires conflict detection to use *the same* arbitration rules
  the runtime applies, so there is nothing to check a candidate binding against until this exists.
- **Inherited from chunk 11: chord and blocked-by (R6.1).** Both read another *action's* state
  rather than their own value, so they need the action table that only the evaluator has — and a
  chord's whole point is out-ranking the bindings it is made of, which is this chunk's algorithm
  rather than a condition's.
- **Decide before building: whether consumption crosses tick domains.** Contexts evaluate as one
  system per context type, unordered against each other, and a render-tick context runs in
  `PreUpdate` while a fixed-tick one runs in `FixedPreUpdate`. Priority ordering within a domain is
  a matter of ordering those systems, which `C::PRIORITY` makes possible at declaration time.
  Across domains it is not: a high-priority *fixed* context cannot consume from a low-priority
  *render* one, because it runs later. Either consumption is defined within a domain only, or
  evaluation is restructured so that one pass walks every context in priority order. That belongs
  in [Design.md](./Design.md) before any of it is written.

### 25. Control classes and class bindings

The binding half of R4.9, which chunk 20 only needs as a filter. A binding may target a class; the
plan grows the second list Design §4.1 describes, consulted when the per-control index does not
claim an event.

- **Not doing:** focus integration. Nothing in-tree binds a class until a focused widget does, so
  this chunk lands the mechanism and its tests, and text input follows when D4 does.
- **Verify `CharacterInput` empirically, do not reason about it.** `KeyboardInput.text` looks like
  the answer, but IME composition arrives on a separate `bevy_window::Ime` channel and whether key
  events still carry text during composition is winit- and platform-specific. This deserves the
  treatment §14's gamepad findings got: measure it, on a real IME, and write down what was actually
  observed. It is the one predicate in the crate a developer is being told to trust rather than
  read (R4.9), so it had better be right.
- **Review surface:** whether R4.10's non-enumerability criterion held. If the set of classes grew
  past a handful while being written, the criterion was abandoned and the case for a closed set goes
  with it.

---

## Phase VI — the parts a solo developer trips over

Both chunks here are what R24.8 turned from polish into obligation: the long tail cannot verify what
it does not own, so mistakes have to be caught rather than discovered in QA that nobody is running.

### 11. Conditions and the `Scratch` table **[PARTIAL]**

- **Delivered:** the `Scratch` record and its per-binding allocation; `dt` and scratch reaching
  modifiers, which discharges chunk 7; the rate-to-delta conversion R2.9 asked for, so a stick and a
  mouse can drive one look action again; and the condition catalogue — press, release, down, hold,
  hold-once, hold-and-release, tap, multi-tap, pulse, and a trait for anything else. R6.2's
  explicit/implicit/blocking composition is Unreal's, adopted as the requirement invites.
- **`Phase::Started` was added**, because the five phases could not say "a hold abandoned before it
  ever fired" — which is neither `Completed` nor `Idle`. `Ongoing` now covers both still-firing and
  still-charging, told apart by the value: firing has one, charging is at rest. A sixth phase was
  considered and rejected on the grounds that the value already carries it unambiguously.
- **Also discharged chunk 15's hysteresis item.** A press derived from an axis or a composite now
  remembers what it decided last tick, per binding, which is what the scratch table was for.
- **In Dead Zone:** hyperspace on a double-tap, and an afterburner that opens up after holding the
  throttle — one control driving two actions that differ only in when they count as having fired.
- **Outstanding → chunk 14:** chord and blocked-by (R6.1). Both read *another action's* state rather
  than their own value, which is a different shape of condition, and chord in particular is
  entangled with the arbitration that decides whether a chord beats its own component bindings.
- **Outstanding → its own decision, then a chunk:** R6.4's sequences and R6.5's buffering and coyote
  time. Both are `SHOULD`s, both fit the scratch record as the design predicted, and neither has a
  consumer yet. R6.5 in particular is called out as a frequent reason teams abandon a general input
  crate, so it wants a real game asking for it rather than a speculative API.
- **Outstanding → chunk 17, and R3.4 is a MUST:** an action does not expose its **elapsed time** in
  its current state, nor R3.5's **progress toward firing**. Both only became meaningful with
  conditions — before this chunk there was nothing to be part-way through — and both are now sitting
  in the scratch record already, since a hold's timer is exactly R3.4's elapsed and its ratio to the
  hold duration is exactly R3.5's progress. What is missing is carrying them out to where a caller
  can read them, which means widening `ActionState` and is therefore worth doing alongside the other
  API-shaped work rather than bolted on here. Dead Zone's afterburner is the consumer: it currently
  shows "charging" as a colour because it cannot show how far along it is.
- **Review surface:** whether the 24-byte scratch record really covers every condition. So far it
  does, with `prev`, `time`, `count` and two flag bits between them covering all nine.

### 17. The diagnostics tier

§9.5's middle tier, which does not exist: plan-build failures collected and reported rather than
asserted one at a time. Unknown controls, shape mismatches a conversion cannot fix, duplicate
bindings, contradictory consume flags (R4.8). Plus `Reflect` on modifiers and conditions so
third-party ones round-trip (R5.6, R17.5), and the derive's duplicate-key error — declaring `path`
twice currently picks one silently, which for a serialized identity is the worst available outcome.

Also the **runtime** half of R24.4, which the crate currently fails: reading an unbound action
panics, and so does reading a context that has zero or several instances. R24.4 names both as
failures that befall a player rather than a developer, so both must return errors.

And R5.9's two `normalize` operations, which need naming before either can be written — one clamps
to unit length, the other remaps a range and therefore falls under D6's one-rescaling-stage rule.

- **Inherited from chunk 15:** the intent-versus-channel mismatch is an `assert!` at plan build
  rather than a collected diagnostic, alongside the rescaling check it sits next to.
- **Inherited from chunk 13: a context declared and never spawned says nothing.** Declaring one
  registers a plan and some systems; if no entity ever carries it, every action in it is silently
  dead and the failure looks like "that key does nothing". This cost a bug in Dead Zone's own pause
  menu, in the file that had just been rewritten. A blanket check at startup would be wrong — a
  per-player context legitimately arrives later — but a **state-driven** context whose state is
  current and whose instance count is zero is a much narrower signal, and one worth saying out
  loud.
- **Review surface:** error text, judged as the deliverable it is. R24.4 distinguishes runtime
  failures (must return errors) from app-build ones (may panic, must be actionable).

### 18. Derive completion

`category` and `consume` on the action (R1.6), and type-registry registration so persistence and
external backends can resolve an action by name (R1.7). Small, and needed by chunk 19.

---

## Phase VII — the player-facing model

D7 made real. This is the half of the crate a player ever sees, and per the audience commitment it
must stay additive: a game that declares none of it keeps working exactly as before (R19.13, R24.7).

**The screen arrives in three passes**, because each one is separately capable of being wrong and
mixing them would make it unclear which half was at fault: first a read-only list, then something
navigable, then something that rebinds. Navigation sits between the first two, since a screen you
cannot move around is a help screen rather than a settings screen — which is exactly why the first
pass is worth having on its own.

### 19. Mappable slots and localization keys

Slots as the unit of rebinding, one per composite part (R19.9, R19.10). Name keys derived from the
action path plus the part name, with an override, and a fallback renderer so a game with no
localization layer still reads sensibly (R19.14, R19.13).

- **Review surface:** whether declaring a whole context's buttons mappable is genuinely one line.
  The audience commitment says where a default trades away accessibility the accessible path must be
  cheap, and slots being opt-in is exactly that trade (R20.1).

### 20. Interactive capture, conflicts, and reserved controls

Capture filtered by the target slot's intent and shape (R19.1); exclusion lists (R19.2); conflict
detection against chunk 14's real arbitration rules with a policy the app selects (R19.3); reset to
default at every scope (R19.4).

- **Settles OQ-10.** A binding declared *reserved* takes no slot **and** its control is refused by
  capture across the scheme, so the button that opens the rebinding screen cannot be rebound away
  nor quietly shadowed by something else. Whether a reachability guarantee sits above that is the
  open half.
- **Capture reads L1 directly** (R19.1). It is a query over the input frame — "the first event
  matching this intent, this shape, not excluded, not reserved" — rather than a binding, because
  what it reports is a control identity that a binding would have thrown away. That also means an
  evaluator that never runs cannot fire a gameplay action, which is R19.5 for free.
- **Introduces the shape half of R4.9's class vocabulary** — any-button, any-analog, any-directional
  — as the filter's language. Exclusion lists and reserved controls use the same vocabulary, so
  there is one way of naming a set of controls rather than three.
- **Verified by:** capturing from the **gamepad**, not only the keyboard, which is the case that
  finds the mistakes.

### 21. The settings screen, read-only

In Dead Zone: a screen listing every mappable slot with what is currently bound to it. A help
screen, and nothing more — no buttons, no focus, dismissed by the same control that opened it.

- **Why this first:** it is the smallest thing that exercises iterating the slot list and rendering
  a binding as text, which are the two halves of D7 a UI actually needs. If either is awkward here
  it is awkward everywhere, and there is no capture machinery in the way to obscure it.
- **Review surface:** whether a UI author can build this without reaching past the slot list into
  this crate's internals. If they cannot, D7 has leaked.

### 29. Directional navigation

The half of D4 that is dispatch rather than activation (R22.7): an action whose firing moves the
focus. `bevy_input_focus` has the sequential half wired to events already, and the directional half
only as a `SystemParam` — deliberately, because it was waiting on an input mapper to settle before
going further. So what is chosen here is a candidate for what lands upstream.

- **Delivers, in the crate:** a modifier that quantises a 2D direction to the compass points, and a
  condition that fires when a value changes. Both are general — eight-way movement and radial menus
  want the first; the second is the cheapest condition in the set, needing only the previous value
  that `Scratch` already carries.
- **Why two pieces and not one.** Snapping alone fires every tick, because the stick stays off
  centre. Change-detection alone fires on every wobble. Together they fire once per compass point
  crossed, which is the behaviour a menu wants — and `.on_change().pulse(0.15)` is then auto-repeat,
  out of two conditions that exist for other reasons.
- **Not doing:** bubbling the instruction as a `FocusedInput`. Bubbling exists so that something can
  *intercept*, and until a widget wants to swallow a direction there is nothing to intercept; the
  observer calls `DirectionalNavigation::navigate` directly. The event-driven entry point belongs
  upstream beside `handle_tab_navigation`, where the interception cases live.
- **Not doing:** `InputFocusVisible`. It exists to hide focus rings from desktop mouse users, and a
  pad-driven game has no such ambiguity.
- **In Dead Zone:** a focus ring on the settings screen, drawn with `Outline`.
- **Review surface:** the two names, more carefully than usual. If bevy_input_focus ends up
  depending on these concepts, renaming them afterwards is somebody else's breaking change.

### 30. The settings screen, interactive

Chunk 21's list grows a button per slot, focus moves between them on stick and D-pad, and the screen
can be dismissed. Still nothing rebinds — pressing a row does nothing yet.

- **Why separate from capture:** navigating a menu with a pad is where the awkwardness usually is,
  and mixing it with capture would make it unclear which half was at fault.
- **Verified by:** operating the whole screen from the Xbox pad without touching the keyboard.

### 31. The settings screen, rebinding

Pressing a row enters capture, the next control pressed takes the slot, and conflicts are reported
per chunk 20's policy.

### 23. Persistence of overrides

A rebind that does not survive a restart is a demo, not a feature. Diff against defaults, unknown
entries reported rather than dropped, a version field (R17.1–R17.3).

- **Intended vehicle: `bevy_settings`** — overrides live in a reflected resource carrying its
  derives, so the file format and its location are somebody else's problem.
- **Check before committing to it:** R17.2 requires an entry that no longer resolves to be
  **reported rather than dropped**, which is what stops a renamed action silently discarding a
  player's rebind. A settings crate that deserializes and quietly ignores what it does not
  recognise cannot satisfy that, and most do exactly that. If it cannot be made to, the diff layer
  is ours and only the file handling is theirs.

---

## Phase VIII — settling

Nothing here changes what the crate can do.

### 32. Activation by run condition

`active_if` takes an ordinary Bevy run condition and makes it the thing that decides whether a
context is live. A condition is `IntoSystem<In, bool, Marker>`, so it pipes straight into a system
that applies the answer — full dependency injection, no exclusive world access, about fifteen lines.

- **Subsumes `add_context_in_state`,** which becomes `active_if(in_state(s))`. That is worth more
  than the tidying: `in_state` is `Option<Res<State<S>>>` internally, so the substate tolerance
  that the `bevy_enhanced_input` comparison caught us lacking comes from Bevy rather than from us
  remembering to write it a second time.
- **Polling is not a problem here.** Run conditions are polled rather than edge-triggered, but the
  edge is detected by comparing against the context's own `active` flag, which is how the state
  sync already works. `activate` and `deactivate` return immediately when there is nothing to do.
- **Two placements, one mechanism.** A state binding belongs in `StateTransition`, where the
  transition has just been applied; a general condition reads current data and belongs in
  `PreUpdate` before evaluation.
- **Says nothing about instances.** A condition returns one answer for the whole context type. The
  per-instance case is `activate` on the entity, and stays that way — see chunk 13.

### 22. The deadzone chain, stages 1 and 3

Calibration and preference, completing D6. Needs the evaluator to stop merging every pad into one
axis map, which is a defect in its own right. Manual calibration API plus an app-driven sampling
step per OQ-4; R14.9's pass-through warning; the preference stage modulating the design stage
without being able to reduce it below what the hardware needs.

- **Now downstream of chunk 26.** Per-device keying is exactly what routing has to introduce, so
  this chunk should follow it rather than build a second way of telling two pads apart.
- **Persistence of calibration** stays blocked on R11.5's stable device identity, which needs two
  units of the same kind to be worth testing.

### 10. The compiled plan and slot allocation

Replace the plan's `BTreeMap` with the `Vec<u16>` action→slot map, the dirty bitset, and the
`Scratch` table's allocation.

- **Success criterion: `examples/` does not change.** A diff there means the abstraction leaked.
- **Why last:** it is an optimization of a shape we now understand, and it is the only chunk that
  adds nothing a player or a developer can see.

---

## Phase IX — the second example

Dead Zone is one player reading one set of bindings. Everything in §15 is invisible to it, because
with a single player there is no question of *which* device drove an action — and a model that never
has to answer that question has not been tested on the thing it was designed for.

### 26. Device routing and the join flow

§15 made real: an input frame event carries the device it came from, a context entity can be paired
to a device, and an unpaired device drives nothing. Plus the join gesture — an app-driven query for
"a device that just pressed something and is not yet claimed" (R15.4), which is the same read of L1
that capture uses in chunk 20.

- **The gate here was miscounted.** This sat under "deliberately deferred, gated on a real second
  device", but a keyboard is a device: one pad plus a keyboard is two, and a mixed-scheme pair is the
  arrangement most likely to expose a routing bug, since the two do not share a code path. What
  genuinely needs two units of the *same kind* is per-device identity and calibration, which is
  chunk 22's problem and stays deferred.
- **Verified by:** two context entities driven at once, each deaf to the other's device — a test
  that fails loudly today, because every context reads the whole frame. It must pass with **either**
  pairing, and the two are not the same test. A pad and a keyboard exercise the mixed case, where
  the two devices share no code path and a routing bug cannot hide behind symmetry. Two pads of the
  same model exercise identity, where kind tells you nothing and only the device handle
  distinguishes them — which is the case that decides whether the handle is carried far enough.
- **Review surface:** whether pairing is a property of the context entity or a filter on the plan.
  The entity already carries the state, so it is the obvious home, but that puts a device handle in
  a component that has so far been pure.

### 27. Split Friction

`examples/split_friction/` — a split-screen game in the shape of Gauntlet: two players, top-down,
shared world, one viewport each.

- **Not doing:** rebinding. Dead Zone covers that, and a second UI would make this example about
  something other than the thing it is here to show.
- **What it is here to show:** the **device selection screen**. Two slots, each waiting for a device
  to claim it; press anything on a pad or a key on the keyboard and that slot is yours. This is the
  first flow in the crate where the player picks the device rather than the developer, and it is the
  one part of §15 a developer cannot get right by reading a doc comment.
- **Verified by:** playing it, with the pad on one slot and the keyboard on the other, then swapping
  them.

---

### 28. Docs that run

The documentation half of R24.6, gathered into a chunk because ground rule 5 is right: the
"documents that follow the code" list this replaced was explicitly not a chunk, which made it a list
of things that would quietly never happen.

- **Make the doctests execute.** `dynamic_linking` on the `bevy` dev-dependency breaks the merged
  doctest binary, so every `///` example compiles but none runs, and the public documentation is
  part of the deliverable. Fixing it means making `dynamic_linking` opt-in, at the cost of slower
  example builds — a tradeoff to make deliberately rather than inherit. Carried from chunk 24.
- **The README rewrite** — a user-facing introduction, feature list, and quickstart, with its
  examples lifted from a real game rather than invented.
- **Comparison with LWIM and `bevy_enhanced_input`** (R22.6) — the migration path the ecosystem will
  ask for. The useful question to ask of each BEI difference is not "is this more ECS-shaped" — it
  reliably is — but "does the ECS-ness earn its keep here". State activation is a case where it
  does; the comparison should say which cases do not, and why.
- **Why last:** the first item can be done at any time and the other two document a moving target.
  The README wants chunk 19, after which the feature list stops growing in the player-facing
  direction; the comparison wants chunks 11 and 14, since conditions and arbitration are where the
  three crates genuinely differ rather than differ in spelling.

---

## Deliberately deferred

Still out of scope for the sequence above. Rebinding, persistence and presentation have left this
table because Dead Zone needs them; device routing and local multiplayer have left it because
Split Friction does, and because the gate turned out to be met already.

| Area                                              | Gated on                                                                   |
| ------------------------------------------------- | -------------------------------------------------------------------------- |
| Persistent device identity and calibration (§11)  | two units of the *same kind*, which pad-plus-keyboard does not give         |
| Prompts and glyph ids (§18)                       | asset-pipeline questions this document does not touch                      |
| Source and authority backends (D3)                | one working in-tree path to generalize _from_                              |
| Netcode injection and rollback (§10)              | a testbed that actually rolls back; also wants held device state made snapshot-able (R10.3), which chunk 9 left as `BTreeSet`/`HashMap` |
| Focus-driven context activation (R22.8) and text input | chunks 14 and 25 — priority, arbitration, and class bindings are what *claiming* a control means. D4's other half, dispatch (R22.7), needs none of that and is chunk 29. |
| **Guardian migration**                            | porting guardian from bevy 0.16.1 to 0.20-dev — four versions, its own job |

Guardian is worth restating: it is on **bevy 0.16.1** with `bevy_enhanced_input 0.12`, and we target
main. The migration is a genuine goal, but it is a port plus a rewrite, and doing both at once would
confuse "action_map is wrong" with "0.20 moved this". Dead Zone first; guardian when there is
something worth migrating _to_.

---

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
