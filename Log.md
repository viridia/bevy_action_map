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

---

## The shape of the record

Two kinds of entry appear below, and the distinction is the useful one:

- **A chunk landed and taught us something.** The lesson usually became an amendment to the
  requirements or a new item on the roadmap, and the entry says which.
- **A chunk landed and claimed more than it delivered.** These are recorded at least as carefully,
  because the pattern in them is the most useful thing this document contains.

---

## Phase I — the walking skeleton

### Chunks 1–4: skeleton, action model, derives, input frame

The module tree, `ActionValue`/`Intent`/`ActionId`, the two derive macros, and a keyboard-only input
frame. Little to record: these were pure-data or pure-structure and the tests were written alongside.

The one decision worth keeping is from chunk 2. **The intent-versus-shape split (D1) was made before
any binding code existed**, on the strength of a single measured fact: an analog trigger arrives on a
*button* channel carrying a fraction. Building the conversion matrix on (intent × output) alone and
adding source shape afterwards would have been a rewrite. It was still a rewrite — see chunk 15 —
because the warning was written down and then not heeded, but the model it argued for was right.

### Chunk 5: the first end-to-end slice, and the gate that was passed late

The chunk was written, and its example did not compile and its `App` tests failed. This went
unnoticed because chunks 6–8 were built on top of it regardless.

When it was finally run, two structural defects surfaced:

- **Context state was a singleton resource**, which R0.3 forbids. The requirement caught a real
  defect rather than describing one: it is what identified the choice as structural rather than
  stylistic.
- **Slots were allocated per binding rather than per action**, so a second binding on an action
  silently disabled the first. This is R4.1, which sounds too obvious to write down, and which was
  violated in a way that disabled the keyboard half of the crate's own worked example.

Both are fixed (`5409238`). The lesson belongs to ground rule 2, and it is why every chunk since
states a verification that is actually executed rather than intended.

---

## Phase II — the single-player slice

Chunks 6, 7 and 8 all landed short of their descriptions. Each is recorded with what it delivered and
what it left, because three consecutive chunks failing the same way is a pattern rather than three
accidents. The common thread: **the code compiled and the prose around it claimed more than the code
did.**

### Chunk 6: axis sources and composites

Delivered mouse motion and a four-key directional composite. The chunk description had warned, in
terms, that the composite must be built source-agnostic across its four parts so that a D-pad — which
reaches L1 as four buttons and never as an axis pair — would drive it for free.

It was not. `DirectionalKeys` held four `KeyCode`s, so chunk 8 inherited nothing, and fixing it in
chunk 15 cost a type change where it would have cost a parameter here.

### Chunk 7: modifiers

Delivered the built-in modifier set and the combinator API. Left `Modifier::apply` taking only a
value, which makes a stateful modifier unwriteable (R5.4, R5.5) — routed to chunk 11, where the
scratch table and `dt` arrive together.

Also left "normalize" out of R5.2's list deliberately, which turned out to be right for the wrong
reason. The word names two incompatible operations — clamp to unit length, and remap a range — and
only the second rescales, so only the second falls under D6's one-rescaling-stage rule. That became
**R5.9**.

### Chunk 8: gamepad and the deadzone chain

Delivered `RawGamepadEvent` consumption and D6's **design stage**: radial and per-axis shapes, an
explicit `rescale` flag, and the one-rescaling-stage rule enforced at plan build.

**De-risked before building.** `RawGamepadEvent` was verified to be genuinely raw — `bevy_gilrs`
disables gilrs's default filters, including its radial 0.1 deadzone, and re-applies only
`axis_dpad_to_button`. So D6's claim to own the whole chain holds, rather than fighting a hidden
stage below us. This is the kind of check worth doing before a contested design becomes code.

**The hardware findings** recorded in §14 of the requirements come from here. They were measured
against an Xbox Series controller rather than assumed, and two of them are negative results that
would otherwise cost someone a day: the same controller over USB is claimed by Apple's DriverKit
dext, after which gilrs enumerates it but receives no values at all; and a Switch-protocol clone
advertises a HID descriptor that does not match the report it sends, producing hundreds of phantom
presses per second.

**Left short:** the trigger threshold landed hard-coded at 0.5 with no hysteresis — the exact
opposite of what the chunk description asked for. Fixed in chunk 15.

### The review, and the requirements amendments it produced

After §§1–8, the tree was reviewed as a whole and the requirements were amended (`2d366eb`). These
are recorded here rather than annotated in place, so that the requirements read linearly:

| Requirement | Change | Why |
| --- | --- | --- |
| R0.1 | "input snapshot" → "input API" | A snapshot cannot satisfy §9; L1 is a queue. |
| R1.1 | The action's path became mandatory | It appears in settings files, so it must be stable and chosen rather than derived from a Rust path that moves when the type does. |
| R1.8 | **New** — a naming convention for paths | A stable identifier with no convention is a stable identifier everyone spells differently. |
| R2.2 | Conversions must be settled in the requirements, not merely documented | Left open, they were decided by accident in code. |
| R2.4 | **Withdrawn** | See below. |
| R5.2 / R5.9 | "normalize" split out and disambiguated | The word names two incompatible operations. |
| R5.7 | `SHOULD` → `MUST` | R10.2 makes the enclosing step's purity a `MUST`. |
| R9.2 | Reworded to describe the guarantee, not the layout | The design gives one state per context per domain, which the original wording forbade. |
| R23.2 | Adds "no synchronization" | The first real violation was a lock, not an allocation. |
| R24.4 | Distinguishes runtime from app-build failure | Panicking during plugin setup is right, and the rule forbade it. |
| R1.6 / R19.6 / R19.9 | Name moves to the slot, category stays on the action | Both claimed the same two fields (OQ-9). |
| R19.14 | **New** — player-visible names are localization keys | R18.3 localized half a rebinding row and left the other half a baked literal. |
| Scope | **New** — "Who this is for" | The studio/long-tail tension drives most decisions here and was never named, so it was being rediscovered per section. |
| R24.6 | `SHOULD` → `MUST`, plus new R24.7 and R24.8 | The enforceable half of that commitment; a `SHOULD` made one constituency optional. |

**Why R2.4 was withdrawn.** It required distinguishing *value* actions from *pass-through* actions —
Unity's model, where an action's bound controls are normally disambiguated to the one with the
greatest magnitude, and `PassThrough` is the opt-out that reports every control separately.

Its motivating cases turned out to be device-shaped rather than value-shaped: telling which of four
pads pressed Start, reading sixteen MIDI knobs bound to one action, and showing every contributor in
a debug overlay. All three are answered elsewhere by mechanisms that have to exist anyway — device
scoping is §15's business, per-source visibility for debugging is R22.2's inspector dump reading the
plan's reverse index, and a value that remembers where it came from is R2.6.

What was left after removing those was a second storage shape — N live values per action instead of
one — carried on every action so that a few could use it. That cost is real and the benefit is
covered, so the distinction is not one this model should carry. If a case appears that genuinely
needs it, it should arrive as its own requirement with that case attached.

**Requirements that earned their keep**, worth defending if they are questioned upstream:

- **R0.2** (L2 reads only L1) is the one that makes everything else work. Determinism, headless
  tests, replay, and external backends are the same mechanism because of it, and every test in the
  crate exists because a synthesized frame is indistinguishable from a real one.
- **R0.3** and **R4.1**, for the reasons under chunk 5 above. Both caught defects rather than
  describing them.

---

## Phase III — repairs

### Chunk 24: housekeeping

The findings from the chunk 5–8 review that belonged to no feature, swept before the feature set grew
over them. Two are worth recording.

**Resolving an action id took a mutex and a linear scan on every read**, which R23.2 now names
explicitly. Rust has no generic statics, so the cache cannot sit on the trait's default method — but
the derive emits a concrete impl, and a concrete impl can hold a `static`. Steady state is now a
relaxed atomic load and a compare. The registry's mutex remains for the first resolution of each
action, which happens at plugin build because `bind::<A>()` already calls `A::id()`, so it never
appears in a frame.

Two notes for whoever touches this again. The atomic comes from `bevy_platform::sync::atomic` and not
`core` — `bevy_ecs` routes its own through there so the polyfill for platforms without atomic support
keeps working, and matching it costs nothing. And the per-type cache is only sound because `ActionId`
is process-global; Bevy cannot do the same for `ComponentId`, which is per-`World`.

**Module organization.** `InputContextState`, `Actions`, `ActionMapPlugin` and `add_context` had
accumulated in `player.rs`, which Design §11 reserves for device pairing. Moved while the call sites
were few.

Left undone: the doctests do not execute. `dynamic_linking` on the `bevy` dev-dependency breaks the
merged doctest binary, so every `///` example compiles but none runs. Deferred deliberately.

### Chunk 9: tick domains and the windowed drain

L1 previously cleared the frame on every sample, which lost edges inside a frame and repeated deltas
across fixed ticks. Both were measured before being fixed: a press and release inside one frame was
never seen at all, and one 9.0 delta read across three fixed ticks totalled 27.0.

**Retirement timing, not retention, was the defect.** Clearing moved from sample time to after fixed
evaluation, which is the only moment every consumer is known to have read — render-tick contexts
drained in `PreUpdate` earlier in the same frame, fixed-tick ones just now. Each context carries a
cursor and reads only what arrived since it last looked, seeded at spawn so a context added
mid-session does not react to input that predates it (R7.5).

Cursors and wholesale retirement look redundant and are not: retirement alone fails when the
simulation does not step, and cursors alone grow without bound. The queue is capped and counts what
it drops, so a stall degrades visibly.

**Under the timestamp shim, a window is a frame.** Timestamps are frame-granular, so a frame's events
cannot be meaningfully split across three fixed ticks — the first tick to run takes them all. That
conserves delta magnitude (R9.5) and fires an edge exactly once (R9.4) without pretending to a
precision the timestamps do not have. Real per-tick splitting arrives with [bevy#9087][] and changes
this one policy rather than the mechanism.

---

## Phase III continued — the three-property model

### Chunk 15: source channel shape

R2.10's third property, which chunk 2 was warned to build in from the start and did not. Delivered
`ChannelShape` as a property every source declares, checked against the action's intent at plan
build; the trigger serving an analog action with its travel and a button action with a hysteretic
press; composite parts as controls rather than keys, so a D-pad and WASD drive one action
identically; and R2.2's conversion table settled and implemented in one place.

Four things it turned up that were not on anyone's list:

- **`Vec2::splat` was the widening rule**, so a trigger at 40% read as a diagonal. Two copies of the
  conversion logic existed and disagreed with each other — which is precisely the failure mode R2.2
  was strengthened to prevent, sitting in a second file.
- **A binding's intent was never checked against its output shape.** It is now a *compile* error from
  the derive, with the message built at expansion time so it names both halves of the mistake.
- **`Vec3` claimed every intent including `Button`**, so a jump action could have declared itself a
  `Vec3`.
- **A Button-intent action driven by an axis** decided its press by asking whether the value was
  non-zero. A stick never rests at exactly zero, so that action would have read as permanently held.

The chunk also made binding a stick to a `Delta2` look action an error rather than silently summing a
rate with a displacement — and in doing so removed a binding from the shipped `move_and_jump`
example, which had been wrong in exactly the way R2.9 describes. The explicit conversion R2.9 asks
for needs the tick's `dt` and is routed to chunk 11.

**One tradeoff made deliberately:** binding legality moved from compile time to plan build. A
rebinding UI has to make the same judgement at runtime against a control the player just pressed, and
one mechanism used twice beats two that can disagree — but it does move a class of mistake from the
compiler to first run.

---

## Phase IV — the first game

### Chunk 16: Dead Zone

`examples/dead_zone/` — an asteroids-like game, played on both keyboard and an Xbox pad. 455 lines,
of which the input layer is 68 and the control scheme itself is 24.

**It found two gaps before it was playable**, both fixed rather than recorded, because neither had a
workaround an ordinary user would find:

- **There was no way to say "two keys make a signed axis."** A 2D composite existed and a 1D one did
  not, and `.negate()` on a key inverts the *press* — so binding `A` with it did nothing at all.
  `AxisButtons` is the missing sibling, and holding both keys cancels rather than letting declaration
  order win.
- **The prelude exported the `InputAction` trait but not the derive of the same name**, so a glob
  import left `#[derive(InputAction)]` unresolved with a confusing error. Both existing examples had
  quietly worked around it by spelling the path in full, which is why it had gone unseen. A trait and
  a derive macro occupy different namespaces, so both can be exported under one name.

A third find, unrelated to the example: **the macros crate's doctest has never compiled.** It
references `bevy_action_map`, which that crate cannot depend on without a cycle. It stayed invisible
because the main crate's doctests die at the `dynamic_linking` error long before anyone runs the
macros crate's separately.

**Playtest findings**, all fixed: `fly` queried `(&mut Transform, &mut Velocity)` with no
`With<Ship>`, so turning the ship rotated every asteroid and thrusting accelerated them — and the
bullets too. The file contains a correct example of the same filter three functions further down,
which is what let it survive a read-through.


---

## Phase V — reading actions the other way

### Chunk 12: the transition log and observers

Delivered the log itself, `Fired<A>`/`Completed<A>`/`Canceled<A>` as generic `EntityEvent`s targeting
the context entity, and the dispatch system that turns one into the other. `Started<A>` waits for
conditions, since without them it would be indistinguishable from `Fired<A>`.

**How a slot finds its action type.** The evaluator works in `ActionId`s and slot indices, and
neither can name a generic event. `bind::<A>()` is the only place the concrete type exists, so it
records a `dispatch_for::<A>` function pointer that the plan keeps per slot. One monomorphised
function per action, resolved at bind time — no registry and no downcasting. The generic
`EntityEvent` derive was the chunk's stated risk and turned out to need no special handling at all.

**R9.3's second half, which chunk 9 handed over.** L2 had been collapsing a window to its final
state: a press and release inside one window cancel in the held state, and a single fold afterwards
sees *nothing happen* — not one transition, zero. The fix is to replay events one at a time and fold
after each.

That collides with deltas, which have no value at an instant, only a total over an interval — folding
per event would hand a mouse-look action partial movements. What makes the two reconcilable is
chunk 15's legality table: `Intent::accepts` lets a `Delta2` action take only delta-shaped sources
and every other intent take none, so **no slot can want both treatments**. The fold runs in two
passes over a partition that was already guaranteed to exist. A constraint added to prevent a units
error turned out to be what made this tractable.

**A test that was true but vacuous.** The first version of "a held key is silent" asserted over
observers, and passed even when `Ongoing` was deliberately added to the logged phases — because
dispatch drops non-edges on the way out, so an observer-based test cannot see a log that records
them. Rewritten to assert against the log directly, driving `apply_frame` with no `App` at all,
which the design's claim that `InputContextState` holds no world references makes possible. Worth
recording as a pattern: a test that only observes the far end of a pipeline cannot verify a
property of the near end.

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
