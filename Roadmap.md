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
| **Known wrong today** | A stick cannot drive a look action at all, because the rate-to-delta conversion R2.9 requires does not exist yet (chunk 11). |
| **Never built** | Conditions, multiple contexts, arbitration, the whole player-facing model. |

---

## What has landed

Thirteen chunks are done. The [work log](./Log.md) says what each delivered, what it found, and where it
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
| 15 | Source channel shape | done; rate-to-delta → 11 |
| 16 | Dead Zone, first playable | playable; no death yet, hyperspace wants 11 |
| 12 | Transition log and observers | done |

Every obligation those chunks left is carried by the chunk that has to discharge it, below, rather
than by the chunk that incurred it — so what a chunk must do is stated in one place.

---

## Phase V — multiple contexts

Dead Zone grows a pause menu, which is what forces all of these.

### 13. Context priority, layering, and activation

Dead Zone's pause menu is what forces this, and it also covers the case player death would have
covered — an interstitial screen with a different context active. Death is therefore polish, and
stays out of the sequence: it would be a second demonstration of the switch this chunk already makes.

Multiple context instances, priority ordering, activation and deactivation lifecycle, and what
happens to in-flight state when a context deactivates mid-hold (R7.4, R7.5).

- **Verified by:** Dead Zone pausing and resuming without the pause key re-triggering on the way
  out — the "pressing E to close a menu instantly re-triggers Interact" bug class, from the game side.

### 14. Arbitration and consumption

The single-pass consumption algorithm (R8.3); chords beating their component bindings; the
"why didn't this fire" diagnostic query (§9.5's third tier).

- **Gates the rebinding UI.** R19.3 requires conflict detection to use *the same* arbitration rules
  the runtime applies, so there is nothing to check a candidate binding against until this exists.

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

### 11. Conditions and the `Scratch` table

Press, release, hold, tap, multi-tap, chord progress; `elapsed` and `progress`. Each condition claims
a fixed-size scratch slot from the plan. Carries chunk 7's outstanding modifier signature, since
scratch and `dt` are the same addition.

Conditions **append to** the transition log rather than introduce it: Design §5 makes the log a
property of the evaluator, and chunk 12 needs it first. What arrives here is the transitions a
condition can produce that a bare phase change cannot — a hold completing, a tap resolving.

- **Verified by:** unit tests driving synthetic time; the R6.1 catalogue is directly a test list.
- **Review surface:** whether the 24-byte scratch record really covers every condition, which the
  design asserts and this chunk proves or refutes.
- **In Dead Zone:** hyperspace on a double-tap, and hold-to-thrust. Hyperspace is a plain button
  press today, which is the whole of what chunk 16 could express.
- **Inherited from chunk 15: hysteresis where a press is derived** from something other than a single
  button — a stick axis, a composite. The button channel keeps its own pressed state per control, but
  a derived value has no control to hang that on, and the memory has to be per *binding* to survive
  two bindings feeding one action. That is what the scratch table is. Until then those paths use a
  plain threshold, which is right except at the boundary.
- **Inherited from chunk 15: the rate-to-delta conversion R2.9 requires.** Chunk 15 made binding a
  stick to a `Delta2` action an error, which is right — a position is a rate and a mouse delta is a
  displacement, and summing them is the units error R13.2 names. But mouse-and-stick look is the
  near-universal case, so refusing it is only half an answer: R2.9 asks for the conversion to be
  explicit, not absent. It needs the tick's `dt`, which is the same addition the modifier signature
  wants here, and `examples/move_and_jump.rs` carries a comment where the stick binding used to be.

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
- **Review surface:** error text, judged as the deliverable it is. R24.4 distinguishes runtime
  failures (must return errors) from app-build ones (may panic, must be actionable).

### 18. Derive completion

`category` and `consume` on the action (R1.6), and type-registry registration so persistence and
external backends can resolve an action by name (R1.7). Small, and needed by chunk 19.

---

## Phase VII — the player-facing model

D7 made real. This is the half of the crate a player ever sees, and per the audience commitment it
must stay additive: a game that declares none of it keeps working exactly as before (R19.13, R24.7).

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

### 21. The rebinding screen

In Dead Zone: a `bevy_ui_widgets` table of slots grouped by action category, a button per row that
enters capture, and the whole screen operable from the controller.

- **Review surface:** the rebinding API judged from a real consumer. If a UI author needs to reach
  past the slot list into this crate's internals, D7 has leaked.

### 23. Persistence of overrides

A rebind that does not survive a restart is a demo, not a feature. Diff against defaults, unknown
entries reported rather than dropped, a version field (R17.1–R17.3).

---

## Phase VIII — settling

Nothing here changes what the crate can do.

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
  ask for.
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
| Focus integration (D4, D5), and with it text input | chunks 13, 14 and 25 — priority, arbitration, and class bindings are what claiming a control means |
| **Guardian migration**                            | porting guardian from bevy 0.16.1 to 0.20-dev — four versions, its own job |

Guardian is worth restating: it is on **bevy 0.16.1** with `bevy_enhanced_input 0.12`, and we target
main. The migration is a genuine goal, but it is a port plus a rewrite, and doing both at once would
confuse "action_map is wrong" with "0.20 moved this". Dead Zone first; guardian when there is
something worth migrating _to_.

---

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
