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


### Chunk 13: the activation lifecycle

Delivered `activate`/`deactivate` with the two behaviours the requirements name: deactivating cancels
what was in flight rather than leaving a hold stuck (R7.4), and activating ignores controls the
player is already holding, with an opt-out (R7.5). An inactive context keeps tracking its devices, so
coming back is free (R7.6).

**The first API was wrong and the example is what showed it.** Dead Zone's pause menu originally held
two facts — a state, and two contexts driven by hand to match — and kept them in step in an observer.
That works and reads like something waiting to drift the moment a third way to reach the menu
appears. `add_context_in_state` inverts it: the contexts follow the state, so there is one fact and
nothing to disagree with it. "Declared inactive" then falls out for free, which had been recorded as
an outstanding gap an hour earlier.

Two smaller things came with it. `add_context` takes its closure as `impl FnOnce`, so
`add_context::<Flying, _>` lost its placeholder — the same trick `bind::<Jump>` got in chunk 15. And
the state sync is one idempotent system rather than a pair on `OnEnter`/`OnExit`, because those fire
only on the transition and would miss an instance spawned while the state was already current.

**A context declared and never spawned is silent.** Rewriting `pause.rs` for the new API dropped the
line that spawned the menu's entity, and the result was a game that paused and would not unpause:
`Flying` fired the pause action, and nothing carried `PauseMenu` to fire it back. Nothing anywhere
said so. The symptom — one key working in one direction only — took a reproduction test to localize,
and the test passed, which is what pointed at the example rather than the crate.


### Reading `bevy_enhanced_input`'s state integration

Compared after chunk 13, and it found a defect and a latency bug in ours.

**A substate has no `State` resource.** BEI reads `Option<Res<State<S>>>` with a comment saying the
resource may be absent for inactive substates and computed states. Ours read `Res<State<S>>` and
would have panicked the first time anyone declared a context in a nested state — and pause as a
substate of playing is the obvious way to write the very example we ship. Now tolerated, with a test
that fails by panic if the tolerance is removed.

**Placement.** Ours ran in `PreUpdate` before evaluation, which is a frame behind the transition,
because Bevy applies transitions *after* `PreUpdate`. BEI runs inside `StateTransition` itself,
after `DependentTransitions` and before `ExitSchedules` — so a context is already in step by the
time an `OnEnter` system looks at it. Adopted, and the one-frame caveat that had been written into
the docs and a test simply went away.

**The decomposition, which we did not adopt.** BEI puts the state binding on the *entity*
(`ActiveInStates<C, S>`, a component holding several values) and registers the (context, state) pair
separately. Two instances of one context can therefore follow different states, and a context can be
live in several. Ours binds one value per context type in one call. Theirs is more capable at the
cost of two places to get right; ours cannot be half-declared. Recorded against chunk 27, where
several contexts and several states meet for the first time.


---

## Phase VI — when an action fires

### Chunk 11: conditions and the scratch table

Taken before chunk 14 rather than after, because 14's own description leads with "chords beating
their component bindings" and chords are conditions — the same producer-after-consumer inversion the
log and the observers had. Chunk 11 had also become the debt sink: four obligations from chunks 7,
15 and 16 were routed here, and all four wanted the same two additions.

**`dt` arrives, and with it everything that was waiting on it.** The evaluator takes `Res<Time>`,
which Bevy points at the fixed timestep inside the fixed schedules, so a context is told how long its
own tick was rather than how long the frame was (R9.6). `ActionMapPlugin` installs `TimePlugin` if
absent — the lack of a clock in the test apps is why chunk 15 deferred this in the first place.

**The scratch record held up.** Design §6 asserts that one fixed four-field shape covers every
condition in R6.1, on the argument that the *parameters* belong to the plan rather than to the state.
Nine conditions later, `prev`, `time`, `count` and two flag bits between them cover all of it, with
nothing needing an escape hatch.

**A sixth phase was needed and a seventh was not.** The five phases could not express a hold
abandoned before it ever fired: that is neither `Completed`, which claims it happened, nor `Idle`,
which claims nothing did. `Started` fills it. The temptation was then to add another for "charging"
versus "firing", both of which land on `Ongoing` — but the action's *value* already separates them,
since a firing action has one and a charging action is at rest. Documented rather than duplicated.

**Two of the tests were wrong before the code was.** One bound an `Analog1` action to mouse motion,
which chunk 15's legality table refused — the third time that table has caught something written
here. The other asserted that two taps too far apart leave a multi-tap `Idle`; they do not, because
the second tap legitimately begins a fresh sequence. The assertion had encoded a misunderstanding of
the feature rather than a property of it, and now checks the thing that actually matters: that no
`Fired` appears anywhere in the sequence.


---

## Phase V continued — who wins

### Chunk 14: arbitration and consumption

Preceded by settling the question the chunk could not start without: **whether consumption crosses
tick domains.** It cannot, in one direction. A render-tick context evaluates in `PreUpdate` and a
fixed-tick one in `FixedPreUpdate`, so the first can take a control from the second and never the
reverse — priority is not a total order across domains. Recorded in Design §5.2 with the reason it
is tolerable: the things that claim controls are UI, UI is render-tick, and what they claim from is
gameplay. `bevy_enhanced_input` keys its consumed set by schedule `TypeId` for the same reason, with
a comment giving the same multi-run argument, which is about as good as this evidence gets.

**Design.md described an implementation that cannot exist.** §5 had the plan sorting every binding
touching a control by (priority, chord length) and evaluation walking that list once. A plan belongs
to one context and cannot see another's bindings, so that list has nowhere to live — and §5.2 rules
out one spanning two schedules anyway. The two halves of the sort are now resolved separately and
each where it can be: priority as system ordering fixed at app build, chord length as a stateless
pre-pass within one evaluation. §5 says so.

**A chord is a property of the binding, not a condition.** `Ctrl+S` is "S while Ctrl is held", which
is what the binding *reads* rather than a rule about when it fires — closer to a composite source
than to a hold. The practical benefit is that the plan then knows a chord's length without
introspecting conditions, which is exactly what R8.1's clash needs.

**Two allocations caught in the hot path.** `controls()` returned a `Vec` and was about to run per
binding per tick; it became `for_each_control`, with the collecting version kept for a UI. And the
clash buffer lives on the context state rather than being allocated per fold. R23.2 is easy to
violate by accident in precisely this kind of code, which is the second time this has come up.

**The `--features libm` configuration found a bug again.** A `chord` field went in without the cfg
that `ButtonControl` carries. That build — a math backend and no devices at all — is the only one
that catches this class of mistake, and it has now caught two.

**Mutation testing earned its keep twice.** The consumption test passes if you disable the `consume`
flag *or* reverse the priority ordering — checking both was what confirmed it tests the ordering
rather than only the flag. And the chord test passes under the chord gate alone, so disabling only
the clash was needed to show that R8.1's rule does separate work.


---

## The second grooming, after §§9–16

A second pass over the requirements once conditions, arbitration and the player-visible state machine
existed. The first pass found requirements that were vague; this one found requirements that were
**wrong** — three of them contradicted by what had been built, which is a different and more useful
kind of finding.

| Requirement | Change | Why |
| --- | --- | --- |
| R7.1 | "a total order" → total *within a tick domain* | Design §5.2 says outright that priority cannot order contexts across schedules, so the requirement asserted something the design forbids. |
| R3.1 | Five states → six, with "started" separated from "ongoing" | One name was answering two questions: a hold that has just begun and a hold that is now firing are both "not an edge", and a player can tell them apart on screen. |
| R3.4 | "a clock the caller selects" → the context's own clock | Presumed a choice the tick-domain commitment removes. The same failure R9.2 had in the first pass: a requirement describing a layout rather than a guarantee. |
| R3.6 | Second half **withdrawn** | See below. |
| R23.2 | Adds "and a way to detect a violation" | Two allocations reached the per-tick path and were caught by reading, not tooling. |
| R3.7 | Given a home (chunk 17) | A `MUST` that no chunk claimed — the requirements' own version of the destination-less item ground rule 5 forbids. |

**Why half of R3.6 was withdrawn.** It required that consuming prevent *the same action's other
observers* from reacting, as well as lower-priority contexts. That is the dynamic interception D5
rules out, one level further down: an observer electing at handling time to suppress its peers makes
the outcome depend on which ran first, which R8.3 forbids for contexts and which is no more
defensible for observers.

Its motivating case — a UI handler out-ranking a gameplay one — is answered by the half kept, and
answered better: the UI's context claims the control, so the gameplay action never fires at all,
and R22.1 can say which context took it. The withdrawn half would have been a second mechanism for
a case the first already covers, which is the same argument that removed R2.4.

**Two risks recorded in Design §12 rather than as amendments**, because the requirements are right
and the implementation is what needs to move: R8.6's offline conflict query has no seam, since the
clash rule lives inside the fold and a rebinding UI needs it as a function; and R23.2 is unenforced.


---

## Out of order

### Chunk 32: activation by run condition

Taken well ahead of its place in the sequence, and it cost nothing to: it needs only chunk 13, and
nothing between the two touches activation.

Any Bevy run condition can now decide whether a context is live. The mechanism is one system —
`condition.pipe(apply_active::<C>)` — so the condition gets ordinary dependency injection and the
world is never accessed exclusively. `add_context_in_state` is gone, and what replaced it is a
builder method, so activation is declared beside the bindings it governs rather than in the name of
the call that declares them:

```rust
app.add_context::<PauseMenu>(|controls| {
    controls.active_in_state(Game::Paused);
    controls.bind::<Resume>(KeyCode::Escape);
});
```

**Why the builder and not a third `add_context_*`.** The trait would have grown a method per
activation policy, and D4's focus-driven activation is already a fourth. On the builder each policy
is one method on a type that is already the place a context says what it is. The cost is a boxed
installer closure held on the builder until `declare_context` can hand it the `App` — the condition's
type is known only inside the caller's closure, so it has to be erased to get out.

**Two placements, and the second one is not a shortcut for the first.** The roadmap read as though
`active_in_state` would collapse into `active_if(in_state(s))`, and it nearly does — `in_state` is
what supplies the substate tolerance, so our hand-written `Option<Res<State<S>>>` comparison was
deleted rather than reimplemented. What does not collapse is *where the system goes*. Bevy inserts
`StateTransition` **after** `PreUpdate`, so a condition polled in `PreUpdate` reads the state before
that frame's transition has been applied. Chunk 13 already learned this and moved the state sync into
`StateTransition` for it; putting states back through the general path would have undone that
finding. Concretely, the cases differ only for fixed-tick contexts and for `OnEnter`:

| | `StateTransition` (`active_in_state`) | `PreUpdate` (`active_if`) |
| --- | --- | --- |
| Render context's next evaluation | frame N+1 | frame N+1 — no difference |
| Fixed context's next evaluation | frame N | frame N+1 |
| What an `OnEnter` system sees | already in step | still the old answer |

So a general condition, which has no transition to sit behind, is polled in `PreUpdate` before
evaluation; a state keeps the placement that a pause menu's simulation half needs. One mechanism,
two installers, and the difference is a table rather than a caveat.

**Polling is not the cost it sounds like.** Run conditions are polled rather than edge-triggered, but
the edge is the comparison against the context's own `active` flag, which is how the state sync
already worked. The applier now makes that comparison itself instead of leaving it to
`activate`/`deactivate` — not to save the early return, but to keep the mutable deref, and with it a
change tick on every instance, off the frames where nothing happened.

**What it says about instances: nothing.** A condition answers once for the whole context type. Per
instance is still `activate` on the entity, and mixing the two would mean the condition winning every
frame — said in the doc comment rather than prevented, since preventing it would mean tracking which
door an activation came through.

**Dead Zone's contexts were arranged the way the crate's history made them, not the way a developer
would.** Reviewing the chunk turned it up: pause was an action bound in *two* state-driven contexts,
`Flying` and `PauseMenu`, each hearing the button while the other was down. That arrangement exists
because chunk 13 built `add_context_in_state` and the example was written to show it off. The
arrangement a developer reaches for is one context with no condition at all — pause, and later the
settings screen — beside one conditional context holding everything else. Rewritten that way: the
`Shell` context is live from the moment its entity exists, and `Flying` is the only thing following
the state.

It is a smaller example for it — one binding of `Pause` instead of two, and one less context — and
the reason it works is easier to state: the control that unpauses has to be heard by something
pausing did not switch off. Worth recording that the old shape was not wrong, only unidiomatic; it
demonstrated require-reset (R7.5) honestly, and it took the mechanism *not* being the point of the
example before the arrangement's oddness was visible.

**And it left the require-reset default standing somewhere less flattering.** With pause out of
`Flying`, the remaining case is analog: hold thrust, pause, unpause while still holding it, and the
ship does not burn again until the control has been released and pressed. That is R7.5 working as
specified, and it was true before the rearrangement too — the old example simply had a more
convincing case in front of it. `active_in_state` always requires the reset, the
`activate_including_held` variant of it being the thing this chunk deliberately did not ship. If
that reads wrong in the hand, the variant is what answers it.


---

## Phase VI — the parts a solo developer trips over

### Chunk 17a: runtime failures

Chunk 17 was three chunks wearing one number, so it was split before it was written: 17a is the two
runtime panics, 17b the plan-build diagnostics it is named for, 17c the `Reflect` and `normalize`
items that were riding along because 17 was the open chunk when they were found. This is the first
half.

**Two panics, and only one of them was really about errors.** R24.4 says a failure that befalls a
player must return an error rather than panic, and the crate had two that did not: reading an action
the context never bound, and reading a context that had zero or several instances. They read like
one problem and are not.

The second has an answer that is better than an error, and Bevy had already written it. `Single<T>`
does not fail when the world has no match — it reports `SystemParamValidationError::skipped` and the
system does not run for that tick. That is exactly right for the motivating case: the ship is dead,
so the system that flies it has nothing to do, and neither a panic nor a `Result` at the call site
describes that as well as simply not running. `Actions<C>` is now a `Single` over the context state,
which the `SystemParam` derive propagates for free — it forwards each field's error with `skipped`
intact — and the many-instance case moved to a new `ActionsQuery<C>` with `get`/`iter`. Bevy's own
`Query`/`Single` pairing, with the short name on the common case.

**Every existing test and example compiled unchanged**, which is the ground-rule-3 signal: Dead
Zone's `fly` still reads `input.value::<Turn>()` and now simply stops running when there is no ship.

The first panic did want a value, and the value is rest — `false`, `0.0`, `Vec2::ZERO` by shape,
with a warning logged once per context-and-action pair rather than per tick, naming both and listing
what the context does bind. That list is the useful half: the action being read is usually a
neighbour of the one that was meant, or the same action in a context that does bind it. `try_value`
and `is_bound` are there for code that wants the distinction, and `why_not` already answered
`Obstacle::Unbound` for anyone asking deliberately.

**Two small dependencies, both already in the graph.** `log` for the warning, which is what
`bevy_input` uses — `bevy_log` is `std`-only and installs a `tracing-subscriber`, so its
`warn_once!` was not worth the weight. And `bevy_utils` for `once!`, the `no_std` half of that macro.
The flag it expands to is a static in the function body, so a generic function warns once per
instantiation, which is what makes "once per context-and-action" fall out rather than needing a
registry.

**The fix made a different failure quieter, and that debt is written onto 17b.** Zero instances used
to panic: the wrong failure, but a loud one. It is now silent, and silence is indistinguishable at
the call site from chunk 13's never-spawned bug — the one that cost a debugging session in Dead
Zone's own pause menu. BSN gives the same silence a second door, since an `on(...)` handler on an
entity that does not carry the context also never fires and never complains. Both are now recorded
against 17b, which is the chunk that has to tell a dead ship apart from a context nobody spawned.

**R3.7 had no destination after all.** The second grooming recorded it as homed in chunk 17, but
chunk 17's description never mentioned it — so the home existed only in this log, and splitting the
chunk would have dropped it silently. It is now chunk 35. Worth noting because it is the exact
failure ground rule 5 exists to prevent, and it survived a grooming pass by hiding in the gap
between two documents that each assumed the other held it.


### Chunk 17b: plan-build diagnostics

**The question that shaped the chunk was how to demonstrate it.** Ground rule 3 makes the examples
the acceptance test, and a diagnostic fires when something is wrong — so a correct game cannot show
one, and shipping a broken example to prove the point is not a trade worth making. Three homes
instead: the message text pinned by tests, since the text is what is being delivered; `tests/ui/`,
which already held the compile-time tier; and `examples/diagnostics.rs`, a catalogue that authors
six wrong binding sets and prints what the crate says about each. Run it and read the output — that
is the review surface, and it beats grepping a test file for string literals.

Asking for the catalogue is what moved validation out of compilation. `Plan::from_bindings` no
longer asserts anything; `diagnose` is a pure function over authored bindings, reachable as
`InputContextBuilder::diagnostics()` with no `App` in sight, and `add_context` runs it and refuses a
context that cannot work. That is half of the offline check R8.6 and R19.3 need — only half, and
worth being exact about it: the *clash* rule is still a closure inside the evaluator reading held
state, and extracting that is chunk 19's job. What this chunk establishes is that validating a
binding set does not require installing it.

**Severity, which was not in the plan.** Once the checks were collected rather than asserted, two of
them turned out not to be fatal. A control bound twice does something — nothing, mostly, though for
a delta action it doubles — and two bindings disagreeing about consuming one control is ambiguous
rather than broken. Panicking over either would be a crate telling a developer their working game
does not run. So a diagnostic carries a severity: errors refuse the context, warnings are logged
and the context is built. The split falls exactly along "can this binding work at all".

**Five checks, and one that could not be written.** R4.8 also names unknown controls, and there is
no such case: `Control` is a typed enum, so a control that does not exist cannot be spelled. That
changes when bindings arrive from a file, which is chunk 23, and it is recorded there rather than
invented here.

**The derive's duplicate key was a three-line fix with a wider blast radius than expected.** The
roadmap named `path`, because a serialized identity silently taking the last of two values is the
worst available outcome. But every key had the same hole, and one `set_once` helper closed all of
them — with the span landing on the duplicate, so the compiler underlines the offending half.

**Warning about a context nobody carries needed a guard, and the guard needed a test that could
see the warning.** The first version warned as soon as a context was live with no instances, which
is wrong: a context activated by entering a state, whose entity is spawned by that state's
`OnEnter`, is empty at the moment we look — the enter schedules run after us. So the warning waits
for a second consecutive empty run. That is a behaviour a test has to observe rather than infer, so
this is the first test in the crate to install a `log` logger and count what came out. Removing the
guard makes it fail on the right assertion, which is what was checked.

### Chunk 36: type-erased inspection, and the overlay that proves it

**Asking how to demonstrate the diagnostics is what turned this up.** The runtime tier — `why_not`,
the five obstacles that look identical from a call site — was tested and appeared in no example, and
R22.2, which asks for an inspector-friendly dump sufficient to drive a live overlay, was claimed by
no chunk. That is the third destination-less item in two sittings, after R3.7 and the unknown-control
check. Ground rule 5 keeps earning its place.

**The overlay could not be written against the crate as it stood, and that was the real finding.**
Every public read is generic over `A: InputAction`, and each context's state is a *distinct component
type*. An overlay built on that would have to name Dead Zone's six actions and two contexts, which is
the opposite of what R22.2 asks for. So the chunk is not a UI: it is the crate's first view of itself
that names nothing.

Three pieces, in order of how much thought they took:

- `InputContextState::iter` walks the actions of a context yielding identity, path and state, and
  `why_not_id` answers the obstacle question for an action named at run time. Both fell out of data
  the plan already had — one more parallel `Vec<ActionId>` beside the paths added in 17a.
- A registry of declared contexts, holding a `fn(&mut World) -> Vec<InstanceDump>` recorded at
  `add_context` — the last moment the type is available. Type erasure by function pointer rather
  than by trait object, because the only thing being erased is one query.
- `dump(&mut World)`, which walks the registry and hands back a plain description.

**It takes the world exclusively, and that is not laziness.** A query over a type you cannot name has
to be built by the world, and `World::query` wants `&mut`. The alternative — storing a `QueryState`
per context and driving it with `iter_manual` — buys a shared borrow at the cost of archetype-update
caveats, for a tool that runs once a frame in a debug build. It also allocates, which is stated in
the module docs rather than hidden: R23.2 governs the path actions travel, and this is not it.

**What the overlay showed about the arrangement chosen last week.** `ToggleOverlay` went into the
`Shell` context — the always-active one that was created for pause — and needed no new plumbing:
one action, two bindings, one more `on(...)` line in the same `bsn!` block. That is the second use
the Shell doc comment predicted, arriving sooner than expected and costing nothing.

**Not doing:** an editor integration. R22.2 asks that the same data be sufficient to drive an
overlay, not that we ship an inspector, and `InputDump` is plain enough for anyone else's.

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
