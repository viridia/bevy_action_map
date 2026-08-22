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

### 5. First end-to-end slice — button actions, one context, polling ← **the gate**

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

---

## Phase II — the single-player slice

Phase II completes worked examples A and B from §9. Each chunk here adds one axis of capability and
extends an example to exercise it.

### 6. Axis sources and composites **[COMPLETED]**

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

### 7. Modifiers **[COMPLETED]**

The OQ-5 commitment made real: a built-in enum (deadzone, scale, negate, swizzle, clamp, curve) plus
`Custom(Box<dyn Modifier>)`, and the binding-combinator API of §9.4.

- **Verified by:** table-driven unit tests. Modifiers are pure functions; this chunk is nearly all
  testable without an `App`.
- **Review surface:** whether the combinator chain reads well at the call site, and whether the
  built-in set is the right closed set.

### 8. Gamepad and the deadzone chain

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

### 9. Tick domains and the windowed drain

`tick = Render` / `tick = Fixed` on contexts; evaluation once per context in its own domain; the
accessor type split that makes reading the wrong domain a compile error (R9.2). Under the shim,
windows partition by frame sequence.

`examples/fixed_timestep.rs`: the §2 sequence diagram made observable — a counter that must not lose
or double-count edges across 0-tick and 3-tick frames.

- **Verified by:** `App` tests that drive `FixedUpdate` zero, one, and three times in a frame and
  assert exact edge counts. These are the tests that prove R9.3/R9.4/R9.5.
- **Why before conditions:** conditions consume a clock and a window. Establishing both first avoids
  writing conditions against render time and then redoing them.

### 10. The compiled plan and slot allocation

Replace chunk 5's naive storage with §4 and §6: plan compilation, slot assignment, the `Vec<u16>`
action→slot map, the dirty bitset, the `Scratch` table's allocation (but not yet its users).

- **Success criterion: `examples/` does not change.** A diff there means the abstraction leaked.
- **Why this late:** it is an optimization of a shape we now understand, designed against real
  bindings rather than imagined ones. Landing it early would have meant designing the plan and the
  API it serves at the same time.
- **Risk:** if the naive storage from chunk 5 has quietly leaked into the public API, this chunk is
  larger than planned. Mitigated by keeping storage behind the accessor from chunk 5 onward.

### 11. Conditions and the `Scratch` table

Press, release, hold, tap, multi-tap, chord progress; `elapsed` and `progress`; the transition log.
Each condition claims a fixed-size scratch slot from the plan.

- **Verified by:** unit tests driving synthetic time; the R6.1 catalogue is directly a test list.
- **Review surface:** whether the 24-byte scratch record really covers every condition, which the
  design asserts and this chunk proves or refutes.

---

## Phase III — multiple contexts

### 12. Transition events and observers

`Fired<A>`, `Started<A>`, `Completed<A>` as generic `EntityEvent`s targeting the context entity;
dispatch from the transition log. §9.6's observer surface.

- **Verified by:** observer-based `App` tests; an example using `bsn!` to attach an observer
  declaratively (§9.6.1), which is the R22.15/R22.17 claim under test.
- **Review surface:** the generic-`EntityEvent` bet (the `FocusedInput<M>` precedent). If generic
  events prove awkward in practice, better to learn it here than after layering depends on them.

### 13. Context priority, layering, and activation

Multiple context instances, priority ordering, activation and deactivation lifecycle, and what
happens to in-flight state when a context deactivates mid-hold.

### 14. Arbitration and consumption

The single-pass consumption algorithm (R8.3); chords beating their component bindings; the
"why didn't this fire" diagnostic query (§9.5's third tier).

---

## Deliberately deferred

Not in scope for the sequence above, and each needs its own design pass before it needs code:

| Area                                                 | Gated on                                                                   |
| ---------------------------------------------------- | -------------------------------------------------------------------------- |
| Device identity, pairing, local multiplayer (§15)    | a real second device to test against                                       |
| Presentation, prompts, glyph ids (§18)               | asset-pipeline questions this document does not touch                      |
| Rebinding UI, mappable slots, tunables, presets (D7) | chunks 6–8, which define what a slot _is_                                  |
| Persistence of overrides (§17)                       | the binding model settling                                                 |
| Source and authority backends (D3)                   | one working in-tree path to generalize _from_                              |
| Netcode injection and rollback (§10)                 | chunk 9, plus a testbed that actually rolls back                           |
| **Guardian migration**                               | porting guardian from bevy 0.16.1 to 0.20-dev — four versions, its own job |

Guardian is worth restating: it is on **bevy 0.16.1** with `bevy_enhanced_input 0.12`, and we target
main. The migration is a genuine goal, but it is a port plus a rewrite, and doing both at once would
confuse "action*map is wrong" with "0.20 moved this". Examples first; guardian when Phase II is done
and there is something worth migrating \_to*.

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
