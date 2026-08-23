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
   description says so and names the chunk that finishes the job. "Its own decision" and "later" are
   not destinations — an item with no chunk number is an item that will be dropped, and chunks 6–8
   are what that looks like in practice.

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
| **Never built** | Conditions, transition events, multiple contexts, arbitration, the whole player-facing model. |

---

## Phase I — walking skeleton

The goal of Phase I is not features. It is to get a runnable end-to-end path as early as possible so
that the ergonomics (R24.6) can be judged from real code while they are still cheap to change.

### 1. Workspace and module skeleton **[COMPLETED]**

Convert the directory into a two-crate workspace: `bevy_action_map` and `bevy_action_map_macros`
(Rust forces the second; it is re-exported so users never name it). Create the module tree from §11
as empty modules carrying their doc comments, plus a `prelude`.

- **Not doing:** any logic at all.
- **Verified by:** `cargo check --all-features`, `cargo check --no-default-features`.
- **Review surface:** whether the module tree in §11 is the right decomposition, judged now rather
  than after code is spread across it.
- **Size:** manifests plus ~100 lines of doc comments.

### 2. Action identity, value, and intent **[COMPLETED]**

`ActionValue`, `Intent`, `ActionId` and its interning registry, the `InputAction` trait, the
`ActionOutput` conversions. Pure data — no ECS, no `bevy_app`, no macros. Test impls are written by
hand.

- **Not doing:** derive macros, state, storage.
- **Verified by:** unit tests over the shape/intent conversion matrix — which `Intent` values are
  legal for which `Output`, and how each coercion behaves.
- **Review surface:** the intent-vs-shape split (R2.7–R2.9, D1) is load-bearing for everything that
  follows and costs nothing to change here.
- **Must handle from the start (R2.10):** _source channel shape_ is a third independent property. The
  motivating case is real and measured — an analog gamepad trigger arrives on a **button** channel
  with a fractional value, so `Analog1`-intent-from-button-shaped-source is not an edge case to bolt
  on later. Building the conversion matrix on (intent × output) alone and adding source shape
  afterwards would be a rewrite of exactly the code this chunk exists to get right.
- **Size:** ~300 lines and a test module.

### 3. Derive macros **[COMPLETED]**

`#[derive(InputAction)]` and `#[derive(InputContext)]`, generating what §9.3 describes.

- **Not doing:** binding syntax, anything that reads a plan.
- **Verified by:** `trybuild` — both pass cases and compile-fail cases, since the error messages are
  the deliverable as much as the expansion is.
- **Review surface:** what a mistake looks like. §9.5's first diagnostic tier is entirely this chunk.
- **Why this early:** D1 makes derives the primary declaration surface. Deferring them means writing
  every example twice.
- **Size:** ~250 lines plus fixtures.

### 4. Input frame, keyboard only **[COMPLETED]**

The `RawEvent` enum, the timestamped queue, an `InputFrame` resource, and a sampling system in
`PreUpdate` reading `MessageReader<KeyboardInput>`. The timestamp shim above lives here.

- **Not doing:** mouse, gamepad, touch; per-player routing; device identity; any windowed drain.
- **Verified by:** headless `App` tests that write synthetic `KeyboardInput` messages and assert queue
  contents and order.
- **Review surface:** the shape of the input frame as a standalone layer (R0.1) — this is what
  would eventually become `bevy_input_frame` if the split in §11 ever pays for itself.
- **Size:** ~350 lines.

### 5. First end-to-end slice — button actions, one context, polling **[COMPLETED]**

Bindings from a single key to a `bool` action; `ActionState` with `Phase`; a context component
holding a `Vec<ActionState>`; the polling accessor. Storage is **deliberately naive** — a plain map
from `ActionId` to slot — because chunk 10 replaces it and the point here is the API above it.

`examples/minimal.rs`: press Space, `Jump` fires, something prints.

- **Not doing:** modifiers, conditions beyond implicit press/release, composites, multiple contexts,
  observers, arbitration.
- **Verified by:** the example, plus `App` tests asserting phase transitions across frames.
- **Review surface:** **the whole developer experience.** This is the first chunk you can run, and
  the deliberate cheapness of everything under the accessor is what makes it affordable to throw the
  API away and try again. If the declaration/read ergonomics feel wrong, this is where we find out.
- **Size:** ~400 lines plus the example.
- **In the event:** the gate was passed late. The chunk was written but its example did not compile
  and its `App` tests failed, which went unnoticed because chunks 6–8 were built on top regardless.
  Two structural defects surfaced when it was finally run: context state was a singleton resource
  (R0.3), and slots were allocated per binding rather than per action, so a second binding on an
  action silently disabled the first (R4.1). Both are fixed. The lesson is ground rule 2's, and it
  is why the phases below state a verification that must actually be executed rather than intended.

---

## Phase II — the single-player slice

Phase II completes worked examples A and B from §9. Each chunk here adds one axis of capability and
extends an example to exercise it.

### 6. Axis sources and composites **[PARTIAL]**

Mouse motion and buttons; the 2D composite (four keys → `Vec2`) with **named parts**, since named
parts are what a rebinding UI must present (R19.9) and getting them wrong late is expensive.
Intent-driven conversion between shapes.

- **Verified by:** unit tests on composite resolution; `examples/move_and_jump.rs` — worked example A
  minus gamepad.
- **Review surface:** the named-parts model (D7) in its first concrete form.
- **Worth more than it looks:** the D-pad reaches the input frame as four buttons and never as an
  axis pair (R14.3), so this one composite covers gamepad D-pads too. Build it to be
  source-agnostic across its four parts and chunk 8 inherits D-pad support with no hat-handling
  path at all.
- **In the event:** the composite was *not* built source-agnostic. `DirectionalKeys` held four
  `KeyCode`s, so a D-pad could not drive it and chunk 8 inherited nothing — the paragraph above
  going unheeded. Chunk 15 fixed it, at the cost of a type change rather than the parameter it
  would have been here.

### 7. Modifiers **[PARTIAL]**

The OQ-5 commitment made real: a built-in enum (deadzone, scale, negate, swizzle, clamp, curve) plus
`Custom(Box<dyn Modifier>)`, and the binding-combinator API of §9.4.

- **Verified by:** table-driven unit tests. Modifiers are pure functions; this chunk is nearly all
  testable without an `App`.
- **Review surface:** whether the combinator chain reads well at the call site, and whether the
  built-in set is the right closed set.
- **Outstanding → chunk 11:** `Modifier::apply` takes only a value. R5.4 and R5.5 need scratch and
  `dt`, so a stateful modifier cannot be written and the trait signature is a breaking change away
  from allowing one. Conditions need the same scratch, which is why this lands with them.
- **Outstanding → chunk 17:** no `Reflect`, so a custom modifier cannot round-trip (R5.6, R17.5).
- **Outstanding → chunk 17:** "normalize" is still missing from R5.2's list, deliberately. R5.9
  now splits the two meanings the one word was hiding; whichever names are chosen, neither may be
  the bare word, and one of the two rescales.

### 8. Gamepad and the deadzone chain **[PARTIAL]**

Consumption of `RawGamepadEvent` (bypassing Bevy's own per-axis deadzone), and D6's three-stage chain
— calibration, design, preference — with the invariant that at most one stage rescales.

Worked example A is complete after this: KBM and gamepad, both bound, one context.

- **Review surface:** D6 is the most contested decision in the requirements. Here it stops being prose.
- **Already de-risked:** `RawGamepadEvent` was verified to be genuinely raw — `bevy_gilrs` disables
  gilrs's default filters (including its radial 0.1 deadzone) and re-applies only
  `axis_dpad_to_button`, so no deadzone is applied anywhere below us on this path. D6's claim to own
  the whole chain holds rather than fighting a hidden stage.
- **Read the analog value, not the press.** Triggers arrive as `LeftTrigger2`/`RightTrigger2` buttons
  carrying `f32`; backends also synthesize press/release at their own threshold. R14.2 requires our
  threshold and hysteresis, so consume the value and ignore the synthesized edge.
- **Also produces:** the upstream bug report on the `Gamepad::analog` / event divergence (§11).
- **Delivered:** `RawGamepadEvent` consumption, and D6's **design stage** — radial and per-axis
  shapes, an explicit `rescale` flag, and the one-rescaling-stage rule enforced at plan build
  (R5.2, R5.3, R14.4).
- **Outstanding → chunk 22:** stages 1 and 3. Calibration is per device *unit*, and the evaluator
  still merges every pad into one axis map, so it needs per-device keying first. Per OQ-4 it ships
  as a manual API plus an app-driven sampling step, not background detection.
- **Outstanding → chunk 22:** R14.9's warning when `GamepadSettings` is not left at pass-through,
  which is a MUST and currently silent.
- **In the event:** the trigger threshold landed hard-coded at 0.5 with no hysteresis, the opposite
  of what the note above asked for. Chunk 15 replaced it with `ButtonThreshold`.


---

## Phase III — what is wrong now

Three chunks, all fixing things that are broken today rather than adding capability. None is
optional and all are load-bearing for everything after.

### 24. Housekeeping

The findings from the chunk 5–8 review that belong to no feature, swept before the feature set grows
over them. Small, and each is a foundation the later chunks stand on.

- **Doctests do not execute.** `dynamic_linking` on the `bevy` dev-dependency breaks the merged
  doctest binary, so every `///` example compiles but none runs. The public documentation is part of
  the deliverable, and right now none of it is verified. Fixing it means making `dynamic_linking`
  opt-in, at the cost of slower example builds — a tradeoff to make deliberately rather than inherit.
- **An orphan trybuild fixture.** `tests/ui/fail/missing_attrs.rs` and its `.stderr` exist but are
  not registered in `tests/derive.rs`, so the case they cover is untested. Register or delete.
- **Module organization.** `InputContextState`, `Actions`, `ActionMapPlugin` and `add_context` live
  in `player.rs`, which Design §11 reserves for device pairing and control schemes — and which will
  actually need that space at chunk 22. §11 lists "state" under `action/`; the plugin wants a home
  of its own. Moving them while the call sites are few costs nothing; moving them after §15's work
  lands costs a merge.

- **Resolving an action id took a mutex and a linear scan on every read**, which R23.2 now names
  explicitly. Rust has no generic statics, so the cache cannot sit on the trait's default method —
  but the derive emits a concrete impl, and a concrete impl can hold a `static`. Steady state is a
  relaxed atomic load and a compare. The registry's mutex remains for the first resolution of each
  action, which happens at plugin build because `bind::<A>()` already calls `A::id()`, so it never
  appears in a frame.

  Two notes for whoever touches this again. The atomic comes from `bevy_platform::sync::atomic`,
  not `core` — `bevy_ecs` routes its own through there so the polyfill for platforms without atomic
  support keeps working, and matching it costs nothing. And the per-type cache is only sound
  because `ActionId` is process-global; Bevy cannot do the same for `ComponentId`, which is
  per-`World`.
- **Verified by:** the existing suite, unchanged. This chunk adds no behaviour, so a behavioural
  diff is a mistake.
- **Why first:** ground rule 1 wants one reviewable change per chunk, and these would otherwise
  arrive as noise inside chunks that are about something else.

### 9. Tick domains and the windowed drain **[PARTIAL]**

Retire events by **window** instead of clearing the frame each sample. `tick = Render` drains
`[last frame, now]`; `tick = Fixed` drains its own tick's window. Accumulated deltas split across
the windows they span. The timestamp shim above is already in place for exactly this.

Dead Zone arrives in chunk 16 and is an integrating physics sim, so this is the difference
between a game that drops shots and one that does not.

- **Not doing:** conditions, real timestamps (that is bevy#9087), per-device windows.
- **Verified by:** `App` tests driving `FixedUpdate` zero, one and three times in a frame, asserting
  exact edge counts and conserved delta magnitude. These are the tests that prove R9.3/R9.4/R9.5,
  and both currently fail: a press and release inside one frame is never seen at all, and one 9.0
  delta read across three fixed ticks totals 27.0.
- **Also fixes:** held state currently lives in the context as a `BTreeSet`/`HashMap` rebuilt by
  replaying events, which is why deltas repeat and why the state is neither `Copy` nor cheap to
  snapshot (R10.3, R23.2). The drain and that storage are one problem.
- **Review surface:** the queue as the design's central bet (Design §2). It is the crate's stated
  advantage over LWIM and BEI, and until this lands it is prose.
- **Delivered:** retirement moved from sample time to after fixed evaluation, which is the moment
  every consumer has read — render-tick contexts drained in `PreUpdate` earlier in the same frame,
  fixed-tick ones just now. Each context carries a cursor and reads only what arrived since it last
  looked, seeded at spawn so a context added mid-session does not react to input that predates it
  (R7.5). The queue is capped and counts what it drops, so a stall degrades visibly rather than
  without bound.
- **Under the shim, a window is a frame.** Timestamps are frame-granular, so a frame's events
  cannot be meaningfully split across three fixed ticks — the first tick to run takes them all and
  the rest see nothing new. That conserves delta magnitude (R9.5) and fires an edge exactly once
  (R9.4) without pretending to a precision the timestamps do not have. Real per-tick splitting
  arrives with bevy#9087, and changes this one policy rather than the mechanism.
- **Outstanding → chunk 12:** an action that fires *and* completes inside one window is still only
  observable as its final phase, because a polling reader has one `Phase` to report. Design §5 is
  explicit that R3.3 is satisfied by the transition log rather than by polling, so the L1 half of
  R9.3 is done — the events reach the tick that wants them — and the L2 half lands with the log.
- **Outstanding → netcode (deferred table):** held device state is still `BTreeSet`/`HashMap` on the
  context, so it is neither `Copy` nor cheap to snapshot (R10.3). Draining incrementally removed the
  bug that mattered; making it snapshot-able is rollback's problem and wants rollback's testbed.

### 15. Source channel shape **[PARTIAL]**

R2.10's third property, which chunk 2 was warned to build in from the start and did not. A source's
channel shape is independent of both the action's intent and its output: an analog trigger arrives
on a **button** channel carrying a fraction, and a D-pad arrives as **four buttons**, never an axis
pair.

- **Delivers:** `BindingSourceSpec` keyed on more than output, so `LeftTrigger2` can drive an
  `Analog1` action; composite parts that accept any button-shaped source, so a D-pad drives the same
  composite as WASD (R14.3); trigger button-view derived from our own threshold with hysteresis
  (R14.2).
- **Unblocks:** R2.9, which Design §5.1 records as unsatisfiable while a binding declares no source
  kind of its own — nothing currently rejects a mouse delta bound to a directional action.
- **Also settles §2, which this completes rather than merely extends:** R2.2's conversion table is
  now required to be *decided* in the requirements rather than left to whoever wrote the code first,
  and it currently is not — a 1D value becomes 2D by copying itself into both components, so a
  half-pressed trigger reads as a diagonal. Intent is also never checked at bind time (R2.8), and
  `Vec3` claims every intent including `Button`. Fixing the source shape without these leaves the
  three-property model two-thirds enforced.
- **Verified by:** unit tests over the (intent × output × source shape) matrix; a D-pad and WASD
  proven interchangeable against one composite; a binding whose intent the source cannot serve
  rejected with a diagnostic.
- **Also lays the groundwork for R4.9:** a control class is defined over the properties a control
  declares, not over a list of control identifiers — which is the same declaration this chunk adds
  for source channel shape, generalized one step. Getting it here means a third-party device kind
  (R11.2) joins a class the day its backend ships, with no registry to be added to.
- **Why before Dead Zone:** analog thrust on a trigger is the motivating case, and it is the one
  control that makes an asteroids ship feel like anything.
- **Delivered:** `ChannelShape` as the third property, declared by every source and checked against
  the action's intent at plan build. A trigger drives an analog action with its travel and a button
  action with a hysteretic press, from one binding each. Composite parts became controls rather than
  keys, so `DirectionalButtons::dpad()` and `::wasd()` drive one action identically (R14.3). R2.2's
  table is settled in the requirements and implemented in one place, so widening no longer invents a
  diagonal. Intent against output shape is now a **compile** error from the derive, with a trybuild
  fixture holding the message.
- **Outstanding → chunk 11:** hysteresis where a press is derived from something other than a single
  button — a stick axis, a composite. The button channel keeps its own pressed state per control, but
  a derived value has no control to hang that on, and the memory has to be per *binding* to survive
  two bindings feeding one action. That is what the scratch table is. Until then those paths use a
  plain threshold, which is right except at the boundary.
- **Outstanding → chunk 17:** the intent/channel mismatch is an `assert!` at plan build rather than a
  collected diagnostic, alongside the rescaling check it sits next to.

---

## Phase IV — Dead Zone, first playable

### 16. Dead Zone **[PARTIAL]**

`examples/dead_zone/` — an asteroids-like game in primitive shapes. Thrust, rotate, fire, and
death; asteroids that split. Keyboard **and** gamepad bound to the same actions, which is the
arrangement that silently broke before chunk 5's repair.

**A directory, not a file.** Cargo picks up `examples/<name>/main.rs` with no manifest entry, and a
game large enough to be a fair test is large enough that a single file would bury the input code —
which is the part anyone reading this example came for. Splitting it also puts the thing under review
in one place: `actions.rs` holds every action, context and binding, and the rest of the directory is
ordinary game code that reads them. If that file is not short, this chunk has found what it came for.

- **Not doing:** menus, rebinding, persistence, sound, score. Those arrive as later chunks extend it.
- **Verified by:** playing it, on both schemes, with the Xbox pad over Bluetooth per the README.
- **Review surface:** **R24.6 and the audience commitment**, judged from a real game rather than a
  snippet. If binding a complete control scheme is not short, this is where that shows. Count the
  lines of input code a solo developer would have had to write.
- **Why here:** everything before it is claimed to work and only partly demonstrated. A game is a
  harsher test than an example that prints.
- **Delivered:** thrust, turn, fire and hyperspace on keyboard and pad, verified by playing it on
  both. 455 lines, of which `actions.rs` is 68 and the control scheme itself is 24 — which is the
  number this chunk existed to produce.
- **It found two gaps in the crate before it was playable**, both fixed here rather than recorded,
  because neither had a workaround an ordinary user would find.
  - There was no way to say "two keys make a signed axis" — a 2D composite existed and a 1D one did
    not, and `.negate()` on a key inverts the *press*, so binding `A` with it did nothing at all.
    `AxisButtons` is the missing sibling, and holding both keys cancels rather than letting
    declaration order win.
  - The prelude exported the `InputAction` **trait** but not the derive of the same name, so a glob
    import left `#[derive(InputAction)]` unresolved. Both existing examples had quietly worked
    around it by spelling the path in full, which is why it went unseen.
- **Outstanding → chunk 16 itself:** there is no ship-to-asteroid collision, so the player cannot
  die. It is the same distance check `shatter_on_hit` already does.
- **Outstanding → chunk 11:** hyperspace is a plain button. It wants a double-tap, which is a
  condition.

---

## Phase V — multiple contexts

Dead Zone grows a pause menu, which is what forces all three of these.

### 12. Transition events and observers

`Fired<A>`, `Started<A>`, `Completed<A>` as generic `EntityEvent`s targeting the context entity;
dispatch from the transition log. §9.6's observer surface.

- **Verified by:** observer-based `App` tests; Dead Zone firing on an observer rather than a poll;
  an example using `bsn!` to attach one declaratively (§9.6.1), which is the R22.15/R22.17 claim
  under test.
- **Review surface:** the generic-`EntityEvent` bet. The context entity it targets now exists, which
  it did not when this chunk was written.

### 13. Context priority, layering, and activation

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

Press, release, hold, tap, multi-tap, chord progress; `elapsed` and `progress`; the transition log.
Each condition claims a fixed-size scratch slot from the plan. Carries chunk 7's outstanding
modifier signature, since scratch and `dt` are the same addition.

- **Verified by:** unit tests driving synthetic time; the R6.1 catalogue is directly a test list.
- **Review surface:** whether the 24-byte scratch record really covers every condition, which the
  design asserts and this chunk proves or refutes.
- **In Dead Zone:** hyperspace on a double-tap, and hold-to-thrust.
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

- **Review surface:** error text, judged as the deliverable it is. R24.4 now distinguishes runtime
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

## Deliberately deferred

Still out of scope for the sequence above. Rebinding, persistence and presentation have left this
table because Dead Zone needs them; device routing and local multiplayer have left it because
Split Friction does, and because the gate turned out to be met already.

| Area                                              | Gated on                                                                   |
| ------------------------------------------------- | -------------------------------------------------------------------------- |
| Persistent device identity and calibration (§11)  | two units of the *same kind*, which pad-plus-keyboard does not give         |
| Prompts and glyph ids (§18)                       | asset-pipeline questions this document does not touch                      |
| Source and authority backends (D3)                | one working in-tree path to generalize _from_                              |
| Netcode injection and rollback (§10)              | chunk 9, plus a testbed that actually rolls back                           |
| Focus integration (D4, D5), and with it text input | chunks 13, 14 and 25 — priority, arbitration, and class bindings are what claiming a control means |
| **Guardian migration**                            | porting guardian from bevy 0.16.1 to 0.20-dev — four versions, its own job |

Guardian is worth restating: it is on **bevy 0.16.1** with `bevy_enhanced_input 0.12`, and we target
main. The migration is a genuine goal, but it is a port plus a rewrite, and doing both at once would
confuse "action_map is wrong" with "0.20 moved this". Dead Zone first; guardian when there is
something worth migrating _to_.

---

## Documents that follow the code

Not chunks, because they document a moving target and want to be written once it stops moving.

- **README rewrite** — a user-facing introduction, feature list, and quickstart. Best written after
  chunk 16, when its examples can be lifted from a real game rather than invented, and after chunk
  19, when the feature list stops growing in the player-facing direction.
- **Comparison with LWIM and bevy_enhanced_input** (R22.6) — the migration path the ecosystem will
  ask for. Wants chunks 11 and 14 done, since conditions and arbitration are where the three crates
  genuinely differ rather than merely differ in spelling.

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
