# Work log, archive

> Closed entries. [Log.md](./Log.md) is the one to read; this is here so that it does not have to
> carry these as well.
>
> An entry moves here when every obligation it created is stated somewhere else and nothing left in
> the sequence reasons from the entry itself. That is a rule about what is still load-bearing rather
> than about age, so a phase can be partly here and partly still open — Phase VII is.
>
> Nothing in this file is live. The requirements and roadmap items these entries produced are stated
> in [Requirements.md](./Requirements.md) and [Roadmap.md](./Roadmap.md) in their own words, so
> reach for this only when you want the reasoning behind one of them and the rationale given in
> place is not enough.

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
- **Mappings were allocated per binding rather than per action**, so a second binding on an action
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
| R1.6 / R19.6 / R19.9 | Name moves to the mapping, category stays on the action | Both claimed the same two fields (OQ-9). |
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

### Chunk 16: Disasteroids

`examples/disasteroids/` — an asteroids-like game, played on both keyboard and an Xbox pad. 455 lines,
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

**How a mapping finds its action type.** The evaluator works in `ActionId`s and mapping indices, and
neither can name a generic event. `bind::<A>()` is the only place the concrete type exists, so it
records a `dispatch_for::<A>` function pointer that the plan keeps per slot. One monomorphised
function per action, resolved at bind time — no registry and no downcasting. The generic
`EntityEvent` derive was the chunk's stated risk and turned out to need no special handling at all.

**R9.3's second half, which chunk 9 handed over.** L2 had been collapsing a window to its final
state: a press and release inside one window cancel in the held state, and a single fold afterwards
sees *nothing happen* — not one transition, zero. The fix is to replay events one at a time and fold
after each.

That collides with deltas, which have no value at an instant, only a total over an interval —
folding per event would hand a mouse-look action partial movements. What makes the two reconcilable
is chunk 15's legality table: `Intent::accepts` lets a `Delta2` action take only delta-shaped
sources and every other intent take none, so **no mapping can want both treatments**. The fold runs
in two passes over a partition that was already guaranteed to exist. A constraint added to prevent a
units error turned out to be what made this tractable.

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

**The first API was wrong and the example is what showed it.** Disasteroids' pause menu originally
held two facts — a state, and two contexts driven by hand to match — and kept them in step in an
observer. That works and reads like something waiting to drift the moment a third way to reach the
menu appears. `add_context_in_state` inverts it: the contexts follow the state, so there is one fact
and nothing to disagree with it. "Declared inactive" then falls out for free, which had been
recorded as an outstanding gap an hour earlier.

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

**Disasteroids' contexts were arranged the way the crate's history made them, not the way a
developer would.** Reviewing the chunk turned it up: pause was an action bound in *two* state-driven
contexts, `Flying` and `PauseMenu`, each hearing the button while the other was down. That
arrangement exists because chunk 13 built `add_context_in_state` and the example was written to show
it off. The arrangement a developer reaches for is one context with no condition at all — pause, and
later the settings screen — beside one conditional context holding everything else. Rewritten that
way: the `Shell` context is live from the moment its entity exists, and `Flying` is the only thing
following the state.

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

**Every existing test and example compiled unchanged**, which is the ground-rule-3 signal:
Disasteroids' `fly` still reads `input.value::<Turn>()` and now simply stops running when there is
no ship.

The first panic did want a value, and the value is rest — `false`, `0.0`, `Vec2::ZERO` by shape,
with a warning logged once per context-and-action pair rather than per tick, naming both and listing
what the context does bind. That list is the useful half: the action being read is usually a
neighbour of the one that was meant, or the same action in a context that does bind it. `try_value`
and `is_bound` are there for code that wants the distinction, and `why_not` already answered
`Obstacle::Unbound` for anyone asking deliberately.

**Two small dependencies, both already in the graph.** `log` for the warning, which is what
`bevy_input` uses — `bevy_log` is `std`-only and installs a `tracing-subscriber`, so its
`warn_once!` was not worth the weight. And `bevy_utils` for `once!`, the `no_std` half of that
macro. The flag it expands to is a static in the function body, so a generic function warns once per
instantiation, which is what makes "once per context-and-action" fall out rather than needing a
registry.

**The fix made a different failure quieter, and that debt is written onto 17b.** Zero instances used
to panic: the wrong failure, but a loud one. It is now silent, and silence is indistinguishable at
the call site from chunk 13's never-spawned bug — the one that cost a debugging session in
Disasteroids' own pause menu. BSN gives the same silence a second door, since an `on(...)` handler on
an entity that does not carry the context also never fires and never complains. Both are now recorded
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
Every public read is generic over `A: InputAction`, and each context's state is a *distinct
component type*. An overlay built on that would have to name Disasteroids' six actions and two
contexts, which is the opposite of what R22.2 asks for. So the chunk is not a UI: it is the crate's
first view of itself that names nothing.

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

---

## Phase VII — the player-facing model

Closed entries only. The phase itself is still open in [Roadmap.md](./Roadmap.md); what is here is the
part of it that no longer has anything outstanding.

### Chunk 18: derive completion

Small, as advertised, and it made three things smaller.

**`category` and `consume` are associated constants with defaults**, so every hand-written
`InputAction` impl in the tests kept compiling without knowing they exist. `CATEGORY` is a
localization key on the same terms as the path, and it lives on the action rather than on each
binding because four movement bindings sharing one category is four chances to disagree. `CONSUMES`
is the action-level default that bindings inherit; `without_consuming` is the exception one binding
at a time, which is the direction R8.2 did not have a way to say before.

**The registry grew metadata rather than a second registry.** It already mapped path to `ActionId`;
it now holds an `ActionInfo` beside each, and `ActionId::from_path` is what turns a name read from a
settings file back into something to look up. It answers `None` for a path this build does not
declare, which is exactly the case R17.2 wants reported rather than dropped — a binding saved
against an action since renamed.

**`#[derive(InputContext)]` now emits `Component`, `Default`, `Clone` and `Copy`.** A context that
is not a component is unusable — every entry point in the crate requires it — and the scene work in
Disasteroids had already forced `Default` and `Clone` onto every one by hand. Four derives became
one, across the tree. The `Component` impl is written out rather than delegated, which is two
associated items in this version of Bevy and will break loudly if that changes; the escape hatch is
to implement `InputContext` by hand, which is three constants.

Worth noting what did *not* happen: Design §9.3 said this chunk would register actions in the
**reflect** type registry. That would key them by Rust type path, which is the identity D8 spent a
requirement rejecting. What persistence and a rebinding screen need is a lookup by *declared* path,
which is our own registry, and it is what was built.

### Chunk 19: mappings

The chunk existed to answer one question, and the answer was better than the design's own sketch.

**Could a mapping address one part of a composite?**
`BindingSource::Directional2(DirectionalButtons)` compiles the four keys as one thing, and
`for_each_control` visited them in order without naming them — so nothing in the model could say
"the key that moves you forward". The fix was small once stated: parts are a named enum, and
`for_each_part` yields the name beside the control. Up, down, left, right; negative and positive for
a two-button axis; `Whole` for a binding that reads one control. A stick and the mouse report
`Whole` too, which is the right answer rather than a shrug — they are one thing to a player, and
what they get instead of per-part rebinding is a tunable.

**`mappable()` takes no arguments, and both halves of that are decisions Design §9.7 had the other
way.** It sketched `mappable_parts(Scheme::Kbm, ["forward", "back", "left", "right"])`:

- *The part names were the author's.* That is a positional list to keep in step with a struct's
  fields, and a second vocabulary for four things that already have names. The composite knows its
  parts; the key derives as `gameplay.move.up`, and the catalogue is where `up` becomes "Move
  Forward". Supplying "forward" at the binding would be naming the same part twice, in the one place
  no translator will look.
- *The scheme was declared.* But the binding's own controls already say whether it is keyboard or
  gamepad, and declaring it is a third thing that can disagree with what is actually bound. It is
  now inferred, and a binding whose parts span both devices is refused when the context is declared.

The result is that declaring a whole composite mappable is one call with nothing to get wrong, which
is what the chunk's review surface asked for. §9.7 has been rewritten to match, with the reasoning,
rather than left disagreeing with the code.

**The collision R19.15 predicted needed two checks, not one.** Two bindings of one action in one
context is a plan-build diagnostic like any other. The same action mappable in *two* contexts is
not: a plan is compiled without seeing the others. What made it findable was chunk 36's registry of
declared contexts — built for a debug overlay, and now the only thing in the crate that can see two
contexts at once. Both tests pass, and the second would have been unwriteable a chunk ago.

**Mappings ride the type-erased door too.** `mapping::mappings(world)` walks the same registry,
needing only `&World` because mappings come from the plan resource rather than from anything an
entity carries. Disasteroids' overlay grew four lines that list what a player would be shown — the
smallest possible consumer of D7's model, and enough to see that the keys read correctly without a
catalogue.

**Not doing:** tunables and presets. R19.11 and R19.12 are declarations over the same bindings and
neither is needed until something adjusts a value; capture (chunk 20) is what turns this list into a
screen that changes anything.

### Chunk 37: naming a control

Unclaimed work, found by asking what chunk 21's screen would actually print. Nothing in the crate
could turn a control into text — the overlay was printing `Key(KeyW)` through `Debug` — and R18.3,
which asks for a structured descriptor plus a fallback renderer, was claimed by no chunk at all.
§18 sits in "deliberately deferred" gated on asset-pipeline questions, but that gate is about
*glyphs*; the text half is not asset-dependent and was falling through the gap. Fourth
destination-less item found this way, after R3.7, unknown controls and R22.2.

**One name, two jobs.** `key/KeyW` is both what a settings file stores (R17.9) and the key an app's
catalogue answers to (R18.3). That is the same economy `MappingKey` already has, and it means a
rebinding row is two keys and two lookups with nothing in it this crate renders.
`fallback_label` is the readable half for a game with no catalogue.

**The table is written out rather than derived, and the size is the point.** Two hundred-odd lines
of `KeyA => "A"` looks like something a macro over Bevy's variant names should generate, which is
exactly what R17.9 forbids: it would put every saved binding at the mercy of a rename in a crate we
do not control. Written out, an upstream rename is a compile error in an exhaustive match and the
stored string stays put. It also let the labels say what controls *are* rather than what Bevy calls
them — `LeftTrigger` is a bumper and `LeftTrigger2` is the trigger, and a player reading "Left
Trigger" for the shoulder button would be told something false.

**The `--features libm` build caught another one.** The table macro was defined unconditionally and
used only behind device features, so the build with no devices at all warned about it. Third time
that configuration has found something no other build sees.

**What is not fixed, and is now written down where it will be read.** The fallback answers for a US
keyboard, so a physical binding shows an AZERTY player the wrong letter — R12.2 calls that a bug and
the crate cannot fix it alone, because nothing in Bevy reports what a physical key produces on the
current layout outside an event that already happened. Two things can: an app supplying the control
half of its catalogue per layout, and — once capture exists — remembering the logical key observed
at the moment a player bound something, which is right for every binding they chose themselves.
Design §10.3 records both rather than leaving the gap to be rediscovered.

**Also noted while here: 17c shrank again.** R17.5 wants `Reflect` so third-party modifiers
round-trip, but §10.1 stores controls rather than modifier chains, so a custom modifier never
reaches a saved override file. What still needs it is serializing whole binding definitions, which
is deferred — so nothing scheduled depends on 17c any more.

### Chunk 20: interactive capture

The chunk as written covered five things, and reading it against the code found that two of them
could not be built: conflict *policies* and reset-to-default both need somewhere to write an answer,
and the overrides store §10.1 designs belongs to chunk 23. So the chunk split. Capture, the class
vocabulary, exclusions, reserved controls and conflict *detection* landed here; applying a rebind
became chunk 38, sitting where chunk 31 needs it. Detection is a pure query over the mapping list
and was buildable today, which is where the seam naturally was.

**A session is a component, and the first framing of that was wrong.** The proposal said "on the
entity being rebound", which read as the player or context entity — and a settings screen reached
from the main menu has neither, so the objection was fair on the words used. What was meant is any
entity the caller picks, normally the settings row the player activated, which exists from the main
menu exactly as it does from a pause menu. Capture touches no context entity at all: it reads L1,
which the sampler fills whether or not anything is spawned. What the component buys over a global
session is that "which row is listening" is answered by where it *is*, rather than by the screen
keeping that state beside a session and holding the two in step.

**Arming costs a frame, and the frame is the feature.** The press that opened the capture is still in
the queue when the session arrives, so a session reading the queue immediately binds whichever key
the player activated the row with — the classic version of this bug. A session skips what is already
queued on its first run.

**Excluded and reserved both refuse, and conflating them would have lost the useful half.** An
excluded control is silent, because it is not being refused: it is busy doing its normal job, which
is precisely how the key that cancels a capture reaches the thing that cancels it. A reserved control
is loud, because a player who pressed it meant to bind it and is owed the reason. The example makes
the distinction visible — `Escape` skips a row while `F1` is refused out loud — and it is read from
Bevy's own button state there, with no context spawned anywhere, which is R19.5 demonstrated rather
than asserted.

**Reserving's second half is the half that matters,** and it is what settles OQ-10. Taking no
mapping stops a player rebinding the settings key away; refusing it across the scheme stops them
binding something else *over* it. Only the first is the obvious reading, and only the first is
useless alone. Reserving and declaring a mapping contradict each other, which is a new plan-build
error and a new row in the diagnostics catalogue.

**Writing the example found a real bug, which is what examples are for.** Binding one action to a
key and to a pad button, both mappable, was reported as a duplicate mapping key — but R19.15 says
uniqueness is per *scheme*, and §10.1 stores the two in separate tables. The check was stricter than
the requirement, in the direction that refuses the ordinary way to write a game offering rebinding
on both devices. Both collision checks are now keyed by scheme and name together.

**The class vocabulary came out one short of what the roadmap expected.** There is no
any-directional class, because no single *control* reports a position in two dimensions — a stick is
two axes, a directional composite is four buttons. Since a player rebinds one part at a time, the
case it would serve never reaches capture, and a mapping that accepts `Axis2` is a stick bound
whole, which §9.7 gives a tunable instead. `CaptureSession::for_slot` returns `None` there rather
than offering a capture nothing could satisfy.

**The crate touched an entity after handing it to an observer, which is a rule rather than a
detail.** Capture triggered `Captured` and *then* queued the removal of the session component. An
observer is entitled to do anything to the entity it is answered on, including despawning it — a
settings row that closes when it is answered is ordinary — and the example did exactly that: it
despawned the answered row and spawned the next one, which took the freed index, so the crate's
queued removal found a live entity of the wrong generation. Removal now precedes the trigger, and is
fallible besides, since one run can answer several sessions and the first observer may despawn a
later one's entity. The observer also now sees the component already gone, so "is this row still
listening" reads the same from inside an observer as from outside.

Worth recording because the reproduction failed: the same observer, writing the same deferred
despawn-and-replace, does not error in a small headless app, because whether an observer's deferred
commands run before or after the ones already queued differs between that and a real `DefaultPlugins`
game. The unit test that does pin the fix asserts the *contract* — the component is gone by the time
the observer runs — rather than the crash. A test asserting the crash would have passed before the
fix, and a test that cannot fail is worse than no test.

**Conflicts are detected and deliberately not carried on the event.** Answering "what else holds
this" means reading every declared context, which capture cannot do from the middle of the input
pipeline — and it is the caller's question anyway, since what to do about a clash is a policy. Two
limits are stated rather than hidden: comparison is at control granularity, so chord-differentiated
bindings are reported as overlapping (a false positive, which is the safe direction), and a clash
across two contexts is reported as *possible*, because whether two contexts are ever live together is
a fact about the game's activation rules and not about its bindings.

### Chunk 39: a mapping holds a list

The model could not express "Jump has two keyboard bindings in one row", which is the arrangement
every shipped game's keyboard table has. A mapping held one control, so two mappable bindings of one
action in one scheme were a *collision*, and the only way to ship "W or Up Arrow" was a second row
under an alias name.

**The proof it was wrong was already in the tree.** Disasteroids had `disasteroids.thrust` and
`disasteroids.thrust_alt`, and `disasteroids.turn` and `disasteroids.turn_alt` — four rows telling a
player that two things are separate when they are the same thing bound twice. That was not written
as a workaround, it was written as the only thing that compiled, which is the more useful kind of
evidence. Both aliases are gone; each is now one row holding two controls.

**A list with a capacity, rather than a fixed two or an unbounded list.** The prior art splits three
ways — games use a small fixed number and label the columns "Primary" and "Secondary", tools
(Blender, VS Code) grow an "add shortcut" button, engines offer unbounded as an authoring surface —
and a capacity covers all three without making a game pay for the editor's case. `Capacity::UpTo(n)`
or `Capacity::Any`, and `slots()` is what a table asks to know how many columns to draw.

**Capacity is inferred and raised, never lowered,** which is the part that keeps the common case
free of ceremony. A plain `mappable` asks for one; several bindings feeding one mapping take the
widest anything asked for; and afterwards no mapping is narrower than the defaults it already holds.
So declaring two mappable bindings produces a two-slot row with nobody writing "2", and
`mappable_upto` exists for the other case — one default shipped, a second slot left for the player.

**The collision check inverted before it was right, and the mistake is worth recording** because the
obvious edit is the wrong one. Keying the uniqueness set by `(scheme, key, action)` reads like "a
repeat of the same action is fine", and does the exact opposite: the same action inserting the same
tuple twice is still a duplicate, so the merge case was refused and the collision case passed. What
it needs is a *map* from `(scheme, key)` to the action that claimed it, reported only when a later
claimant differs. Two tests said so immediately, in opposite directions, which is why there were two.

**The cross-context check does not consult the action, and the asymmetry is deliberate.** Two
mappable bindings of one action inside one context are a primary and a secondary and merge. The same
two in two different contexts stay a collision, same action or not, because they are separate rows
in contexts that may be live at different times while the overrides store is keyed by mapping alone.
It is worth stating in the source next to the check, because "same action, and still an error" reads
as a bug otherwise.

**Capture had to name a slot.** A mapping holding a list means a capture has to say *which* slot it
is filling, or the answer has nowhere to go but the front of the row and the secondary column can
never be filled. `CaptureSession::for_slot` names one and `for_mapping` takes slot zero; `Captured`
echoes it back. Slots past the capacity are refused, and so is any slot more than one past what the
mapping holds now — a hole in a list whose order is what primary and secondary *mean* would be a
silent promotion of the secondary later on.

**What is not answered, and where it went.** A control repeated across two slots of one mapping is
not reported by `conflicts`, which excludes the whole target mapping rather than the one slot.
That is a policy question and the policies are chunk 38's, so it is written onto chunk 38 rather
than left as a comment — along with the fact that 38's pending-override set now holds a list per
row, which a set valued by one control would get away with until someone edited a secondary.

**The public docs cite documents the reader does not have, and this chunk only found it.** Around
forty doc comments name an `R`-number, a `§`, an `OQ` or a `D`-decision — eighteen in `capture.rs`
alone, several of them in the module-level block that is its docs.rs landing page. On docs.rs the
requirements and the design document do not exist, so a developer is told that something satisfies
R19.5 and has no way to find out what R19.5 is. The house style already draws this line; nothing had
been checking it. None of chunk 39's own additions are affected, which is how it was noticed at all
— writing to the rule made the surrounding text look wrong. It goes to chunk 28 rather than here,
because it spans four files and none of it changes behaviour, and folding it in would have made this
diff unreadable for the thing it is actually about.

**The two nouns were the wrong way round, and finding out cost the chunk a rename.** The first
version called the row a `Slot` and then needed a second word for one position in it; "cell" was
what came to hand, and it never sat right — a cell belongs to the table a screen draws, so "a slot
holds cells" reads as a drawing inside a data structure. Chasing that discomfort turned up the
actual fault: `slot` already meant *an indexed position in a list* twice over in this crate, in the
plan's per-action state array and in the evaluator, and the public `Slot` was the one use that meant
something else. The good name was on the wrong thing.

So the row became `Mapping` — which also repairs a verb/noun mismatch nobody had noticed, since
`.mappable()` had been declaring a `Slot` — and `slot` moved to the position, where it agrees with
both existing uses. `cell` survives only as a presentation word: **a screen draws one cell per
slot**, and that sentence is the whole of the relationship.

Worth recording how it was done, because the obvious way is a trap. Renaming both nouns at once
would have meant a window in which `slot` meant the row in some files and the position in others,
with nothing to tell a reader or a compiler which. It went in two passes instead — `Slot`→`Mapping`
everywhere first, green, then `cell`→`slot` — so at no point did one word have two live meanings.
The checks after each pass are also why the state-array `slot` in `plan.rs` and `eval.rs` came
through untouched: it was enumerated first and protected explicitly, rather than trusted to a
regex.

**Grooming found three things with no destination, which is two more than expected.** Reverse lookup
(R18.1) had none at all and is now chunk 40 — it is what "Cancel (B)" needs, and Disasteroids' own
screen spec asks for shortcut captions on its buttons. Mouse buttons are chunk 41: `Control` has no
variant for them, `InputFrame` never samples `MouseButtonInput`, and the requirements do not mention
them, so the crate claims keyboard-and-mouse and supports half of it. That one has a hard ordering
constraint — `Control::name` is the stored persistence identity, so it must land before chunk 23 or
the save format needs migrating on its first day. And §18's deferral reason was wrong: it claimed an
asset-pipeline gate, which is true of glyphs and false of R18.5 and R18.6.

### Chunk 41: mouse buttons

The crate named keyboard-and-mouse as a control scheme and supported half of it. `Control` had no
variant for a mouse button, `InputFrame` never sampled `MouseButtonInput`, and no requirement
mentioned them — so "fire on left click", which is the commonest binding in the genre Disasteroids
belongs to, could not be written at all.

**Why it came before the settings screen and before persistence.** Persistence is the hard
constraint: `Control::name` is the stored identity (R17.9), so a variant added after a save format
ships means either a migration or a name chosen to fit around what is already written. The screen is
the soft one — a rebinding table a player cannot put a mouse button into is a hole they find in the
first minute, and finding it after the table is drawn means changing the table.

**A variant is never just a variant.** It reached `Control`, `ButtonControl`, `BindingSource`,
`BindingSourceSpec`, `RawEvent`, the sampler, the plugin's message registration, the evaluator's
held state and its pressed predicate, `capture::arrival`, and the name and label tables — the same
spread chunk 37 covered for the variants that already existed. The one place it did *not* need work
is the mapping model, which is what having a `ButtonControl` abstraction was for: a mouse button is a
composite part and a chord member for free.

**`MouseButton` is `Hash` but not `Ord`,** so the held set is a `HashSet` where the keyboard's is a
`BTreeSet`. Worth a comment in the source, because the asymmetry looks like carelessness otherwise
and R10.3 will eventually want both snapshot-able.

**The stored name and the shown label part company, deliberately, for the thumb buttons.** They are
stored as `mouse/Back` and `mouse/Forward` — what the backend calls them, and the stored string
must not drift — and shown as **Mouse 4** and **Mouse 5**, which is what every other settings
screen the player has seen calls them. Exactly the call §10.3 already made when it decided
`LeftTrigger` renders as a bumper. An unnamed button reads as "Mouse Button 7" rather than
"Mouse 7", so a raw index can never be mistaken for one of those two.

**A feature combination nobody had ever built was broken, and only a sweep found it.** Widening the
`any(keyboard, gamepad)` cfg groups to include `mouse` created a configuration that had not
existed before — mouse alone, no keyboard, no gamepad — and it did not compile:
`CompiledBinding::chord` and
the `ButtonState` import were both gated on keyboard-or-gamepad, and one `cfg(not(...))` fallback had
been left un-widened, so `chord_len` was specified twice. Building all eight combinations of the
three device features is now the check that catches this, and it is worth doing whenever a cfg group
changes rather than only when a feature is added. The `--features libm` build alone would not have
found it; that configuration has no devices at all.

**Where it shows up.** Disasteroids' `Fire` is now Space *and* left mouse, both mappable — one row
with both slots filled, and the first two-control row in the game that is not two keys. The spare
slot moved to `Hyperspace`, so the read-only screen still has a blank cell to draw.

**Not doing the wheel, and it now has a destination.** It is a delta on its own channel rather than a
button, wants the `Line`/`Pixel` normalization R13.3 describes, and shares nothing with a button but
the device. Nothing in tree asks for it, so it went to the deferred table rather than being written
badly here.

**The requirements gap was the real deliverable.** R4.1a states the bindable control set outright,
and R13.0 gives mouse buttons the section they should always have had — §13 separates position,
motion and buttons in its own problem statement and then had requirements for only the first two.

### Chunk 43: listed by default

The player-facing list was opt-in in both senses at once. A binding with no mapping was neither
rebindable nor *visible*, so the only rows a screen could draw were the ones a game had already
offered for remapping — and the commonest gamepad screen in the industry is a read-only list of what
the pad does, with the remapping owned by the platform. We could not draw it from our own data.
Disasteroids' gamepad table is exactly that screen, which is why this had to come before 21 rather
than after it.

**Two questions had been fused into one flag.** *May the player change this* is the developer's call,
because a fixed binding is a design decision. *May the player see this* is the player's business, and
the two want opposite defaults. Splitting them gives three states — listed and fixed, listed and
rebindable, unlisted — and `Rebinding { Here, Fixed }` on the mapping is what a screen reads to
decide whether the row gets a button. R19.10 was rewritten around the split; R4.7 now says only the
rebindable half, which is all it ever meant.

**The flip was proposed as `.listed()` and is better as `.private()`.** An opt-in verb would have
been the third thing to remember to write, and the failure mode of forgetting it is invisible: the
binding works and simply never appears. Opt-out fails the other way — the wrong thing shows up on a
screen, which is a bug you see. The escape hatch is named for what it means rather than for the
mechanism, since a game author's question is "is this the player's business", not "is this listed".

**Four checks had to narrow, and three existing tests found it.** Key uniqueness, the
rebinding-disagreement check, the cross-context collision report and `conflicts` all exist to protect
the override store, and a fixed row never reaches it. Left as they were, they turned an ordinary
arrangement into an error the moment listing became the default: one action bound in two contexts is
now two listed rows under one name, which is what R19.13 promises a game that offers no rebinding at
all. So all four require at least one side to be rebindable, and the diagnostic returns the moment a
`mappable` is added to either — which is the moment it can do harm. Worth noting that the tests
failed for the *right* reason and I nearly widened the exception instead of narrowing the check.

**`private` and `mappable` panic on each other, in both orders.** One says the player may not see it
and the other says they may change it, and a builder chain that quietly picked a winner would be a
silent wrong answer in the one place a wrong answer is invisible. `.private()` asserts the binding is
not already rebindable; `declare_mapping` asserts the mapping is still there, which covers
`.private().mappable()` without a second check.

**It found the gap chunk 44 exists for.** Listing by default put `Afterburner` on the screen beside
`Thrust`, under its own name, holding the same three controls — and the question it raised was not
about listing at all: if those are two mappings, a player can rebind `Thrust` to `J` and leave the
afterburner on `W`, and then put `Fire` on `W` and afterburn by holding fire. Nothing collides, so
`conflicts()` cannot see it; the failure is a *separation* that should not have been possible.
Afterburner is a logical extension of Thrust and should move with it, which the model has no way to
say. That is chunk 44, and Disasteroids carries `private` on those three bindings until it lands —
which produces the right screen and none of the linkage, and the comment there says so.

**Two rename regressions from chunk 39, found on the way.** `slot` had two live meanings before that
chunk renamed one of them, and my protect-list missed two places where the surviving meaning — the
state-array position — was written in prose: Design's "Action → slot assignment" and chunk 33's
"slots ordered topologically". Both had been silently converted to "mapping" and both were wrong.
The lesson is that a protect-list keyed on identifiers does not cover prose, and the sweep for a
rename this size has to be a reading rather than a grep.

**What is still opt-in, and deliberately.** Rebinding stays a declaration, and §0's accessibility
paragraph says why that is uncomfortable: the jam entry's likely outcome is a game with zero
remappable controls. The listing flip pays some of that back for free — a game that declares nothing
still has controls a player can read, which is the readable half of R20.1 — and the obligation the
paragraph names, making the accessible path *cheap*, is unchanged.

### Chunk 21: the settings screen, read-only

Disasteroids has a controls screen: `F2` or Y opens it, the same control closes it, and it lists
every binding the game declared in two tables. It reads nothing but `mappings()` — no action type,
no context, no key — so the same file would draw a different game's controls unchanged. The one
action it names is the one that closes it, and it names that to render its own caption.

**The column count is the data's, and the same code draws both tables.** A row says how many
controls it can hold and the widest row in the table decides how many cells every row draws, which
comes out as three columns for the keyboard — name, primary, secondary — and two for the pad, with
the screen saying neither. That falls out of chunk 39's model being right about a fixed row: a
mapping the player cannot change has exactly the slots its defaults fill, so a table of fixed rows
is one control wide without anything having to special-case it. Hyperspace's spare secondary is the
case that proves the other half — an empty cell the player will fill, drawn rather than absent.

**A category had no way to be rendered.** A mapping's name is a `MappingKey` with a fallback label
and its category is a bare `&'static str`, so the first screen to draw headings had to write its own
title-casing next to the crate's. `mapping::fallback_label(key)` is now the same courtesy for any
key, and `MappingKey::fallback_label` is written in terms of it. That was the only thing the screen
needed that D7 did not already offer, which is the answer to this chunk's review surface.

**The screen is a game state, not a flag.** Proposed as a `Showing(bool)` resource with change
detection, corrected to a `States` enum on the same terms as `Game::Paused`: there is one fact about
whether the screen is up rather than a screen and a flag that have to agree. Closing it then needs
no code at all — `DespawnOnExit` on the root is the whole of it, since closing *is* despawning — and
the pause banner, which had carried a hand-written despawn system since chunk 13, lost it in the
same edit. Two idioms for one thing in one example was the worse half of that diff.

**It is a separate state from `Game`, so the game keeps running behind it.** Making it a third
`Game` variant would have stood `Flying` down for free — and would have destroyed what chunk 30 is
for. The ship still answers the throttle while the screen is up, which is precisely the arrangement
that chunk needs: a screen over a live game, binding the same controls at a higher priority and
consuming them. Leaving the bug visible is what makes the fix demonstrable.

**The caption is a reverse lookup done by hand.** "Press F2 or North Button to close" comes from
filtering the mapping list by `ActionId` and reading the controls back out, which is chunk 40's job
done the slow way — and it confirms that chunk's instinct that a scan should be written before an
index. It is written onto 40 as the caller that already exists.

**A screen nobody can find is not a screen.** `F1` and `F2` were discoverable only by reading the
source, so the game now carries a dim line in the corner naming both. Its text is read out of the
mapping list rather than written down — the same move the close caption makes, and for the same
reason: a string naming a control is wrong the moment somebody changes the control.

**Two label overrides, and they are the app's.** `disasteroids.turn.negative` derives as "Turn
Negative" and a player should read "Turn Left", which is what a catalogue is for: the screen answers
for the two keys whose derived text is wrong and leaves the rest to the fallback. That the fallback
is legible for every other row is the point of it existing.
### Chunk 40: reverse lookup

The question every other path through the crate throws away the answer to. `Prompts` is a trait with
one method — given an action and a scope, the controls that would fire it now — and `BindingTable`
is this crate's answer to it. Disasteroids' two hand-rolled captions read through it instead of
scanning the mapping list.

**The two lists diverged, and that is the finding.** `mappings()` and a reverse lookup look like the
same query filtered differently, and they are not. A mapping list is what the game *declared*: a
controls screen has to draw a row whether or not anything is carrying the context, because the row
is a fact about the game. A prompt is what would fire *now*, so it is empty for a context nobody
carries and for one that is switched off, and it includes a `private` binding — `private` says the
row would duplicate another row, which is a statement about the list and not about whether the key
works. Once that was clear the implementation followed: the lookup reads the compiled plan through
its own type-erased door rather than filtering `mappings()`, and the two doors sit beside each other
on `DeclaredContext`.

**The first thing it broke was the caller it was built for.** Disasteroids' corner hint ran in
`Startup`, alongside — and unordered against — the `Startup` system that spawns the context whose
controls it names. The mapping list did not care, so nothing had ever needed that ordering; a lookup
that asks what is live does, and an unordered pair is a coin toss the build makes rather than a bug
that reproduces. It moved to `PostStartup`. That is the whole cost of the runtime/declared
distinction showing up in one line of a game, and it is worth knowing that it shows up at all: a
caller that used to be order-independent is not any more.

**Consumption has two readings and the literal one is wrong.** R18.2 says the answer must reflect
consumption, and the obvious implementation consults `ConsumedControls` — which holds what has
actually been claimed *this tick*. A claim lands only while the claiming action fires, so a caption
built on it would flicker as the player pressed things. What a prompt wants is the standing fact: a
control bound with `consume` in a stronger active context never reaches the weaker one. That is
computed from the plans and from which contexts are live, and it moves only when a context
activates. The ordering it needs — render tick before fixed tick, then priority, then declaration —
is §5.2's rule, reused rather than reinvented, which is also what makes it the ranking of the
result.

**The ranking says what it does not know.** Contexts rank, bindings rank within a context, and
devices do not rank at all, because nothing tracks which one the player is holding (R18.6). Ordering
keyboard before gamepad would have been a guess wearing a ranking's clothes. A caller that knows
passes a `Scope`; Disasteroids' corner hint passes `Scheme::KeyboardMouse` and takes the first,
which is the shape a caller that *does* know has. *Groomed straight afterwards:* the hole is not a
hole. R18.6 is withdrawn and the device is the caller's parameter for good, so this is the answer
rather than a placeholder for one.

**The return type is not a `Control`, and this was the cheap moment.** R18.9's point is that a
backend's origins are its own enumeration, covering device families we have no variant for, so
`Origin` is either one of ours or a name-plus-label pair from somewhere else. Both answer `name()`
and `fallback_label()` — the two strings chunk 37 established — so a caption renders one without
asking where it came from. Nothing in tree constructs the foreign variant yet; the deferred table's
R18.9 row loses its origin half and keeps its glyph half.

**The chord came along, the conditions did not.** A binding requiring a modifier alongside its own
control reports both, because a prompt that dropped it renders `Ctrl+S` as "S" — wrong rather than
unpolished, which is the line this chunk's review surface asked to be drawn. R18.3's structured
descriptor stays unbuilt on the other side of it: a held binding and a tapped one on the same key
still produce the same prompt, and `Afterburner` is the case in tree.

**A scan, and the index is still not warranted.** §10's sketch assumed the inverse of the plan's
control index. Writing the scan first was the honest order and the answer is that the callers are a
handful of captions rebuilt when a screen opens. Chunk 47 is what would change it, and its own
answer there is change detection rather than an index — so the index is the move after that one
fails, not before.

### Housekeeping, between 47 and the next chunk

Three commits with no chunk between them: the example renamed, `rebind` renamed to `mapping`, and
the document corrections both renames turned up.

**Disasteroids, and the reason the pun had to go.** The example was Dead Zone, which was funny and
pedagogically backwards — a reader meeting it while learning what a deadzone *is* meets a game that
has nothing to do with one. The mechanical half was worse than the joke: its action paths were
`dead_zone.thrust` and `dead_zone.flight`, so the game's namespace sat one token from the crate's
own `DeadZone` in `rebind.rs` and `binding.rs`, the two files that teach both. Two `///` comments
were using the example's namespace to explain `fallback_label` on docs.rs, where no reader has an
example to refer to; those moved to `gameplay.*`, which the rest of the file already used.

**`rebind` became `mapping`, and the argument was not that it holds mappings.** `binding.rs` holds
bindings, composites, modifiers and conditions and is named for the primary one; a module holding
mappings, tunables and presets and named for an *operation* performed on them was the odd one out.
The rename restores the pairing the module doc already claimed — `binding` is the developer's model,
`mapping` is the player's. `Rebinding`, `mappable` and every prose "rebind" as a verb stay; the type
is chunk 48's call and is a good name for what it says.

**The same rename lesson, for the third time.** Chunk 39 left two stale spellings that chunk 43
found; this pass left one that the *first* sweep of this session missed. It survived because the
deferred table's Tunables row does not spell the name: it said tunables were wanted "by a game named
after a deadzone". No grep for the old title finds a paraphrase of the old title. Chunk 43's entry
already says a rename this size has to be a reading rather than a grep — three instances is a
pattern, and the reading has to cover the places that describe the thing instead of naming it.

**Two ground-rule-5 findings, from re-reading the prelude instead of the list.** Chunk 48 enumerated
a dozen bare nouns; the prelude exports sixteen. `Obstacle`, `Timestamp`, `Rebinding` and `Actions`
were missing, and a name that chunk does not list is a name it walks past — `Timestamp` being the
one another crate is most likely to export too. Separately, R18.3's condition half had no
destination in either document: chunk 47 landed the chord half of the descriptor and said the
condition half "stays unbuilt", and nothing carried it. It is now a deferred row gated on chunk 44,
which is the chunk that takes `private` off `Afterburner` and so the moment a held binding and a
tapped one appear on one screen. 44 carries a bullet saying so, because a gate nobody is watching
is not a destination.

### Chunks 29 and 30: a screen you can move around

Landed as one chunk, because the two halves only test each other. The crate gained two combinators
and a screen to prove them on: a stick or a D-pad now moves the selection on Disasteroids' controls
screen, the game underneath keeps flying and never hears the keys the screen has taken, and the
whole thing can be operated from an Xbox pad without touching the keyboard.

**The two names were the review surface, and the argument for both is that neither is about
navigation.** `.compass(CompassPoints::Four)` rounds a 2D value to a compass point and throws the
magnitude away; `.on_change()` fires on the ticks a value differs from the tick before. Eight-way
movement wants the first for its own reasons and it is the cheapest condition in the set, needing
only the previous value `Scratch` already carries. What makes them a menu is that they compose:
rounding alone still fires every tick, change detection alone fires on every wobble across a
boundary, and together they fire once per point *entered*. `.pulse(0.25)` beside them is auto-repeat
out of a third combinator that was already there.

**`Scratch::prev` was lying, and had been since chunk 11.** Its doc says "the previous input value";
`evaluate` wrote `ActionValue::Bool(actuated)` over it in the shared preamble before any condition
saw it, which was all the built-in set ever needed. A condition comparing two directions cannot work
from that, and neither can anyone's custom one. Storing the whole value costs nothing — every other
arm reads `prev.to_bool()`, which is the same answer either way — and the fix is one line. Worth
recording because the field's documentation was correct and the code was not, which is the direction
that stays invisible: nobody re-reads a doc comment that already says the right thing.

**Consumption stopped following the fire and started following the verdict.** `.consume()` claimed
its controls only on ticks where the binding reported `Fired`, with a comment explaining that a
binding merely *bound* to a control should not hold it against everyone else. That reasoning is
right and the rule drawn from it was too narrow: a menu binding that fires once per direction
entered says nothing between crossings, so holding the arrow key moved the selection once and then
turned the ship on every tick after. The claim now lasts while the verdict is `Ongoing` too, which
is one rule — a binding claims its controls while it has something to say — and it fixes two cases
nobody had complained about yet: a charging `.hold()` and a part-way `.multi_tap()` were both
leaking their keys to whatever was underneath. R8.2 carries the amendment.

**The screen is its own context, and that was not the plan.** Chunk 30 was written expecting
`active_in_state(Settings::Showing)` beside `Flying`'s. What it got is better: the screen's root node
carries the `Menu` component, so the context exists for exactly as long as the screen does and there
is no activation condition anywhere. That is R22.14 — spawning must be sufficient — turning out to
be the *simpler* option rather than the permissive one, and it means despawning the screen releases
every control it took with nothing saying so.

**The prediction chunk 13 made held.** A settings screen over a running game is a higher-priority
context binding a subset of the same controls and consuming them, and it needed nothing declared
beyond that. Cross-domain consumption carried it without being asked: `Menu` is render-tick and
`Flying` is fixed-tick, and §5.2's rule that a `PreUpdate` claim stands for every fixed tick in the
frame is what makes an arrow key reach the screen and not the ship.

**What the spatial heuristic cost was one component per widget**, and the roadmap was out of date
about it. Chunk 29 said `bevy_input_focus` had the directional half only as a `SystemParam`;
`bevy_ui` has since grown `AutoDirectionalNavigation` and an `AutoDirectionalNavigator` that scores
candidates by edge distance and perpendicular overlap. So there are no navigation links in the
example at all — the table's layout is the graph — and the only placement decision is `AutoFocus` on
Cancel, which is independent of tab order and saves the screen from naming a first cell and then
keeping that name true as the table changes.

**The finding worth the chunk: consumption cannot reach a widget that reads input itself.**
`bevy_ui_widgets::Button` activates on `Enter` and `Space`, and `Space` is the ship's trigger. The
mapper claiming `Space` does nothing about it, because the keyboard event reaches the button through
`InputDispatchPlugin`, which is the only thing turning a global key event into a focused one and
asks the mapper nothing. So R8.2 was met between contexts and unmet against the widget beside them
— written up as R8.2a, since a requirement that is true of the mechanism and false in the game needs
to say so in the requirements rather than in a log entry.

The first reading of this was that the fix belonged in Bevy. It does not: `DefaultPlugins` is a
plugin *group*, so an app can disable one member and add a mapper-aware replacement, which makes it
chunk 49 behind the `focus` feature and no upstream conversation at all. The workaround until then
is an action bound to `Enter` and `Space` that consumes them and has no observer anywhere —
Disasteroids' `Swallowed` — which is ugly in exactly the way that keeps it from being mistaken for
the design.

**Two things declined, both with gates rather than intentions.** R22.7's bubbling dispatch was chunk
29's stated deliverable and is not built: bubbling exists so that something can intercept, and this
screen has nothing that wants to swallow a direction, so a `FocusedInput` would have been ceremony
around a four-line call to the navigator. The gate is a widget that intercepts — a slider, a scroll
area, a text field. And R22.5's "initial delay + repeat rate" is one number rather than two, because
the pulse's clock starts on the tick the change fires; the gate is a screen long enough for equal to
be wrong.

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087

### Chunk 47: a binding as a text span

The presentation half of R18, made authorable. A component beside `TextSpan` names an action and
fills in its own string, so a template says "Press ⟨whatever fires this⟩" with nothing in it naming
a control. Disasteroids' two hand-formatted captions are now spans, and the `format!` and the `join`
are gone from both.

**The dependency graph decided where this lives, and that was not the plan.** The chunk was written
as two components — one requiring `TextSpan`, one requiring `Text` — until the question "what does
that cost" got asked. `TextSpan` is `bevy_text`, which drags `bevy_asset`, `bevy_image` and
`bevy_log`, the crate this project's own manifest says it avoided for being std-only. `Text` is
`bevy_ui`, which drags the render stack — and `bevy_ui` already depends on `bevy_input` and
`bevy_input_focus`. An input crate depending on it inverts the layering and forecloses `bevy_ui`
ever using action maps itself, which is a worse outcome than any amount of convenience is worth. So
the split is by dependency weight rather than by subject: the crate keeps the lookup and the
staleness signal, both free, and everything that draws is `examples/common/prompt_ui.rs` — shared
by `#[path]`, covered by `tests/prompt_ui.rs` so it is tested rather than merely compiled, and
waiting on a deferred row whose gate is Bevy taking the crate upstream. The two-component design
went with it: a whole `Text` that is nothing but a prompt is a `Text` with one span under it, which
is how Bevy already models it.

**A resource that is touched, not an event, and the reasoning is worth keeping.** R18.5 wants
invalidation that is not a per-frame poll. A broadcast event was the first proposal and the counter
won on two grounds: three changes in one frame — a rebind, a context switching off, the entity
carrying it despawning — coalesce into one pass rather than three sweeps over every prompt on
screen, and the pass happens at a point in the schedule the reader picks, which matters because a
caption wants rewriting before layout measures it. The half that made it uncontroversial is that
resources are now entities: the touch is written as an *insert* rather than a mutable deref, so
hooks fire, so a consumer can observe the resource instead of owning a system. There is a test
pinning that, because it is a claim about Bevy rather than about us.

**Component change detection is not an alternative, and it is worth writing down why.** The obvious
implementation watches `Changed<InputContextState<C>>`. Evaluation writes that component every
frame, so the filter is true constantly and detects nothing. What works is a signal raised
deliberately, which happens to travel on the change-tick mechanism.

**The third firing point is the one nobody would have written.** Two were obvious: a context
activating, and a binding changing — that one is chunk 38's to raise, since nothing can rebind yet.
The third is an instance of a context arriving or leaving, because the answer is folded over every
entity carrying the context, so it moves from empty to non-empty with nothing calling `activate`.
That is a hook on `InputContextState<C>` rather than on `C`, registered where `read_bindings`
already is. It is also chunk 40's `PostStartup` ordering, arriving as a fix rather than a
constraint: the corner hint went back to `Startup`, because a span asks after the fact and asks
again when the answer moves, so which system ran first stopped mattering.

**Absence as a third state, for the device a bare prompt speaks for.** The chunk had to decide what
a span with no companions renders, and chunk 40 had already refused to rank devices. The framing
that settled it is that nearly every game has one answer for its whole runtime — a console title
names pad buttons even with a keyboard plugged in — so this is configuration rather than state, set
once and usually never touched. `PromptDevice` therefore has no default: **absent** means the game
has not said and is diagnosed where prompts are drawn, and **present holding `None`** means the game
deliberately has no primary device. A default would have been a guess that fails silently, with
every prompt in the game naming the wrong control and nothing reporting it.

**A rename, and the sweep it exposed.** `Scope` and `Origin` became `PromptScope` and
`ControlOrigin` — a prelude glob-imported into a BSN template puts a bare noun beside components
from three other crates, and `Scope` in particular means something different in half of Bevy. The
sweep found a dozen more (`Scheme`, `Mapping`, `Part`, `Phase`, `Intent`, the four transition
events), which became chunk 48 rather than riding along in a feature chunk. That chunk has a shelf
life: it costs a rename today and a deprecation story the moment anything outside this repository
depends on the crate.

**What the class narrowing cost, and what it bought.** `PromptScope` grew `of(ControlClass)`, which
meant `ControlOrigin::Foreign` growing a `class` beside the `scheme` it already carried — symmetric,
and the alternative was a narrowing no backend-supplied control could ever satisfy. An origin whose
reporter said nothing about its class is excluded from a narrowed answer deliberately: handing a
caller who asked for a button something nobody has claimed is one is worse than handing them
nothing.

**Fell short of its own description in one place.** R18.5 lists a keyboard *layout* change among the
things that must invalidate a prompt, and nothing in Bevy reports either the current layout or a
change to it — the same gap §10.3 records for `fallback_label`. The requirement is annotated rather
than quietly satisfied: it is unobservable rather than unbuilt, and it becomes schedulable if winit
surfaces the layout.

### Chunk 44: bindings that travel together

`.follows::<A>()` on a binding: it rides `A`'s mapping, contributes no row of its own, and moves
with that row when the row moves. Disasteroids' three `Afterburner` bindings drop `private` for it,
which was the chunk's stated acceptance test and is the whole of its diff in the example.

**The bug it fixes is a gameplay bug, and it was latent rather than absent.** Rebind Thrust to `J`
with the old model and the afterburner stays on `W` — and if the player later puts Fire on `W`,
holding Fire afterburns. Nothing collides, so conflict detection could never have caught it: the
failure is a *separation* that should not have been possible, and `conflicts()` looks for two rows
holding one control. Chunk 38 was the deadline rather than the discoverer, which is why this landed
first.

**Resolution is by the controls, not by the name, and that is one lookup doing three jobs.** A
follower names an action, and that action usually has several bindings — one per device. Matching
the *source* picks the right one without either side naming a device, checks "the same controls"
rather than trusting them, and guarantees that a follower's controls are its principal's slots,
which is what will let a sub-row inherit the row's columns. It also rules out a chain of followers
for free: a follower has no mapping, so nothing can ride one.

**The roadmap's own check would have failed the roadmap's own acceptance test.** It said the target
must "exist, be `mappable`, be in the same scheme, and read the same controls" — but Disasteroids'
pad `Afterburner` follows the pad `Thrust`, which is deliberately listed-and-fixed, because the pad
table is read-only and console remapping owns it. Requiring the target to be rebindable rejects it.
The check is *listed*, not mappable: following a fixed row leaves nothing to rewrite and still keeps
the duplicate off the screen, which is worth having alone. Worth recording because the plan and the
example disagreed and only the example was right.

**Two refusals rather than one, because the fix is in different places.** `FollowsNothing` is no
binding of that action reading this — a typo, or a device bound on one side only. `FollowsUnlisted`
is a binding that reads it and is `private`, so there is no mapping to lend; the repair is on the
*other* binding, and one diagnostic covering both would have named the wrong one.

**What the chunk deliberately did not do, and why the deferred table was wrong about it.** A
follower is not listed separately — so nothing about this puts `Afterburner` back on a screen, which
is what the deferred row for R18.3's condition half predicted would happen and used as its gate. Two
things in that row were untrue. `private` was never what concealed the held-versus-tapped prompt
collision: `a_private_binding_still_answers_a_prompt` has asserted since chunk 40 that a hidden
binding answers prompts, so the collision has been reachable all along and what conceals it is that
nothing in tree *prompts* `Afterburner`. And a gate that trips on a chunk which does not trip it is
not a gate.

So the row is deleted and the work is **chunk 50**, with a number rather than a gate. Asking what
the player wants to see is what settled it: a player looking up "how do I boost?" cannot find out
from the controls screen today, and will not be able to after this chunk either. The answer is one
row, one set of keys, and a subordinate line beneath it — dimmer, not activatable, carrying its own
whole formula ("Hold W") rather than a diff against the row above, because a bare "hold" in a cell
is a qualifier with no control in it. That needs R18.3's condition half, which also has a second
consumer in `PromptSpan`, which is what makes it a chunk instead of a corner of this one.

### Chunk 50: what a held control says

`ConditionDescriptor` in `condition.rs`: `None`, `Hold { duration }`, `MultiTap { count }`, derived
from a binding's own `Vec<BindingCondition>` by `describe` — first match wins, and `HoldAndRelease`
reads as `Hold` because the player still has to hold the control even though release is what fires.
Its `fallback_format` is the whole formula ("Hold W", "W ×2") rather than a qualifier alone, on the
same reasoning `fallback_label` already carries: a catalogue gets the pieces separately and composes
its own word order, and only a game with no catalogue gets English glued together.

Two consumers, as planned. `Prompt` gained a `condition` field, populated in `read_bindings`
alongside the chord it already carried; `examples/common/prompt_ui.rs`'s `caption` runs the chord
through first and the condition second, so a held chord renders "Hold Ctrl+S" rather than dropping
one or the other. `Mapping` gained `followers: Vec<Follower>` — action, path, and the follower's own
condition — built in a second pass over `InputContextBuilder::mappings` because a follower's row is
found by its *leader's* declaration, which wants the whole binding list resolved rather than whatever
`mappings` has accumulated so far. `examples/disasteroids/settings.rs` draws each follower as a
`line` indented under its principal, sharing `cells`' shape but never `changeable`, so the selection
cannot land on it.

**The bug worth recording: a follower rides a binding, and a row can be several bindings.**
Disasteroids' `Thrust` is one row with two keyboard slots — `KeyW` and `ArrowUp`, two separate
`mappable` bindings merged by key and scheme — and `Afterburner` declares `.follows::<Thrust>()`
once per key, because `leader_of` matches by exact source and a follower can only ride one binding
at a time. Both follows-declarations resolve to leader bindings that feed the *same* mapping row, so
the naive second pass pushed `Afterburner` onto that row's `followers` twice — one identical
subordinate line drawn under the other. The fix is a dedup by the follower's action within one row's
`followers` before pushing, which is also the right answer for the case that looks similar and
isn't: two *different* followers, each riding a different slot of one row, are two real facts about
it and both belong. `a_follower_riding_every_slot_of_a_row_is_still_one_sub_row` pins the one that
looked like a corner case and turned out to be the acceptance test's actual shape.

**Running Disasteroids to check the acceptance test found a second, unrelated thing.** The settings
screen is now visibly taller than its two device tables account for, because `Menu` binds
`Navigate`, `Accept` and `Back` with nothing marking them as machinery rather than controls, and
listing-by-default puts all of them on the screen — exactly what chunk 53 already describes and was
written to fix. Nothing about this chunk caused it; the extra row height chunk 50 adds is what made
a pre-existing problem large enough to see. No new destination needed — 53 already is one — so
"Known wrong today" now names it instead of leaving it for the next reader to rediscover.

### Chunk 53: a context the player never sees, closed without a crate change

**The chunk as written assumed a fact that turned out to be false.** It called for a builder-level
declaration — `private` or some other spelling, on the whole context rather than on each binding —
because the alternative was said to be filtering by context at every screen, which every future
screen would then have to know how to do. But `Mapping` already carries `context: &'static str`,
set from the declaring context's path since chunk 19 built it for the mapping-collision check
(`report_mapping_collisions`). The data chunk 53 was written to add already existed; nobody had
looked before designing around its absence.

**Fixed at the one call site that has it, not in the crate.** `settings.rs`'s `screen` function
already names `Menu`, `Navigate`, `Accept` and `Back` concretely — it is the function building this
game's own screen, not the generic table renderer beneath it — so filtering `mapping.context !=
Menu::PATH` there costs one line and touches nothing `table`, `cells`, or `follower_cells` read. The
claim in this file's module doc, that nothing below `screen` names a context, stays true; the filter
sits above it.

**Why not build the crate feature anyway, for the games that will hit this later.** The case for it
was that a screen written once should work in a different game without knowing its context types —
but that was never a stated requirement, only a property chunk 19 inherited by reusing chunk 36's
type-erased registry (built for R22.2's debug overlay) for the mapping list too, and later prose
treated the inheritance as a design goal. With exactly one consumer of `mappings` in the tree today,
declaring the exclusion once in the crate and filtering it once at the call site cost the same
number of lines; the crate version added a builder method, a name to bikeshed, and a panic-ordering
check against `mappable` for a case that has not happened yet. Revisit if a second screen needs the
same exclusion and duplicates the filter — that is the point at which the crate is the one paying
for the repetition rather than one call site.

### Chunk 38: applying a rebind

Twenty chunks of the player-facing model and none of them could change anything. Capture reported a
choice, `mappings` read the compiled defaults, `conflicts` read the compiled defaults, and chunk
44's follower link had nothing to follow *through*. What was missing in all four cases was the same
thing: somewhere to write an answer.

**Split before it was written, into 38 and 54.** The chunk as groomed carried the store, the apply
path, the follower rewrite, `mappings()`'s meaning, the four conflict policies, R19.8's delegation
outcome, the repeat-within-a-row question and the pending working copy. That is two chunks, and the
seam is not arbitrary: a policy needs somewhere to put its answer, so every one of the deferred
items sits *on top of* the store rather than beside it. Worth noting that ground rule 1's "split it
before you write it" is easy to apply to a chunk that reads long and hard to apply to one that reads
coherent — this one read coherent.

**`Overrides` is a plain value, not a resource,** which was the author's call and changed the shape
of the module. The crate defines the structure and applies it; where it lives is the app's business,
so it can be a working copy on a settings screen, a field in a settings resource, or a payload sent
to an account service. That is the same arrangement `bevy_feathers` gives a theme map, and it is
what makes chunk 54's pending set free rather than a second type.

**Three decisions §10.1 left to whoever built it, now in §10.1.**

*Where the variant lives.* "Swapped into an entity's own state" reads as *only* the entity, and that
answer has a bug in it: an instance spawned after a rebind — a player joining, a context respawned
with a game state — silently gets the shipped bindings back. So `AppliedPlan<C>` is a second
per-context resource holding the variant plan and the variant rows, `InputContextPlan<C>` is now
literally untouched rather than untouched by convention, and its presence is exactly the answer to
"has anything been overridden here".

*What `mappings()` means*, which R17.1's note flagged as unanswerable and now is not: it reads the
variant, and `declared_mappings()` reads the defaults. Every existing caller wanted current values
and got them without changing. The one that was easy to miss is the *reverse lookup* — a prompt
names the control that would fire the action, so it reads the variant too, and the test that caught
it was the one about spawning an instance after a rebind rather than anything about prompts.

*A variant keeps the declared plan's slot allocation*, which turned out not to be an optimization.
An action whose every binding the player cleared would otherwise lose its slot and read as
**unbound** — firing the "not bound in this context" warning, which exists to catch a typo and is
exactly wrong for a control somebody deliberately emptied. Keeping the table also means action
states and require-reset flags stay aligned across the swap.

**The `Arc` change, and why the alternative lost.** `BindingModifier::Custom` and
`BindingCondition::Custom` held a `Box`, so neither `BindingSpec` nor `Plan` could be cloned, so a
variant could not be built by the obvious route. The alternative was sharing compiled per-binding
data behind an `Arc` and rewriting only the source — cheaper per apply, no public type change — but
it needs a precomputed key-to-binding index and a second code path deriving the row list. Cloning
the authored specs and recompiling makes applying *literally* the pure function §10.1 already
specified, and reuses `diagnose` on the result. Applying is rare by construction; the recompile is
not the cost worth optimizing.

**One walk, two readers.** The pass that assembles player-facing rows and the pass that rewrites
them both need "which bindings feed this row, in slot order". They were going to be two walks
agreeing by inspection, which is the failure mode worth a function to make impossible — a row built
one way and written another puts the player's control in a slot the screen is not showing it in.
`mapped_parts` is that function, and `mappings()` was refactored onto it rather than the applier
duplicating it.

**What the example found, which nothing else would have.** `examples/capture.rs` stopped at "nothing
is rebound"; it now applies what it captured and prints the row, the rider, and the untouched
declaration. Two things fell out of running it:

- *Growing a slot on one part of a composite is wrong.* An empty slot is filled by copying the
  binding beside it, which is right for a whole binding and produces "Move Down: S | S" for a
  composite — the other three directions land in their own rows a second time. Refused now, with the
  remedy being the second `mappable` composite a two-column movement table is written with anyway.
  Found by writing `mappable_upto(2)` on a directional binding to see what happened, not by reading
  the code.
- *A follower riding only some of a row's slots is drawn as riding all of them.*
  `Mapping::followers` is row-level and carries no slot, so the demo's `WallJump` — following one of
  Jump's two bindings — printed "Hold R, Hold J" when only the first was true. Chunk 44 got this
  right for Disasteroids, where `Afterburner` rides both of Thrust's bindings, so nothing in tree
  had ever shown it. The example follows both now; the modelling question was written onto chunk 31,
  with a plan-build diagnostic as the likely answer since a rider on some slots and not others is
  far more likely a missing line than an intention — chunk 58 closed it a different way, replacing
  `.follows()` with a builder call that derives coverage instead of declaring it.

**Deliberately not here, each with a destination.** The conflict policies and the delegation outcome
→ 54. Serde, `Reflect` and the file format → 23, which also inherits the question the `MappingKey`
key raises: a row this build cannot resolve is reported and then dropped, and whether a save should
preserve it is a format decision, not an apply decision. Per-player override sets → 26, which is the
first chunk with two players; note that the variant already lives per instance, so a per-entity
apply is a second entry point rather than a second implementation.

**One new chunk, from noticing that a review note is not a destination.** Chunk 23 carried "open the
file in a text editor and see whether you can tell what it says" as a review surface, which ground
rule 5 does not accept — nobody is accountable for a note. It is now chunk 55, and it has something
to be accountable *with*: a golden TOML document in a test, which fails when the file stops reading
well. Writing 38's store made the reason concrete rather than aesthetic. `Overrides` keys rows by
`(Scheme, MappingKey)`, and a derived `Serialize` over that map emits a tuple key — unreadable in
every format and not a legal TOML table key at all. So the serialized shape has to differ from the
in-memory one deliberately, and "deliberately" is the word that wants a test under it.

### Chunk 54: Conflict policy

Smaller than the roadmap section that described it. `conflicts_pending(mappings, pending, control,
target)` in `capture.rs`: the same question `conflicts` answers, against a working copy of
`Overrides` instead of what is applied, which is what a screen holding unconfirmed rebinds needs
before either choice is committed. `conflicts` itself is now a thin call onto a shared walk with
`pending: None`, so it is unchanged in behavior and untouched in its own tests.

**Everything else the roadmap section asked for turned out to already exist, or not belong here.**
The four named policies — reject, swap, duplicate-allowed, unbind-the-other — do not need a
crate-owned enum or a `rebind()` that mutates rows on the app's behalf. `Overrides::bind`, `set` and
`get` already say everything a policy needs to say: reject is not writing, duplicate-allowed is
writing anyway, and swap and unbind-the-other are the app reading the conflicting row's own current
list — `pending.get(scheme, key)` falling back to `Mapping::slots`, the same rule
`conflicts_pending` itself uses — and writing it back with one control removed or traded. A first
pass built the crate side of that anyway, as a `ConflictPolicy` enum and an `Overrides::rebind`
resolving conflicts and writing several rows internally. Correctly rejected on review as the crate
accreting a decision that is the app's to make, not a gap the app cannot fill itself — and not a
hypothetical concern: this is feedback already heard from collaborators about the crate taking on
more than it needs to. The doc comment on `conflicts_pending` now carries the four policies as
worked examples instead, so a reader is not left to invent the pattern.

R19.8's "not ours, delegate" outcome needed nothing at all: `Override::NotOurs`, landed with the
store in chunk 38, already is that answer, read with `Overrides::get` before a screen ever starts a
capture. Nothing about it required the delegation to be phrased as an outcome a rebind attempt
returns.

**The repeat-within-one-row gap chunk 39 left** (`conflicts` excludes the whole target mapping, so
two slots of one row holding the same control is invisible to it) **resolves the same way, not with
a crate-side check.** A caller about to write a row already has the candidate list in hand — the
same list it is about to hand to `Overrides::bind` — and a duplicate in a `Vec<Control>` needs no
help from this crate to notice. Recorded here rather than turned into an API, on the same reasoning
as above.

**The roadmap's own "review surface"** — whether a policy is a value the app picks once or a
decision it makes per rebind — turned out to be answered by not building a crate-side policy API at
all: the app already owns both cases with the primitives it has, a fixed policy applied every time
or a modal that tries `Reject` first and calls its own resolution afterward. There is no outcome
type here that would have needed to carry what a policy would have done instead.

### Chunk 25: Control classes and class bindings

The binding half of R4.9. Chunk 20 landed the shape half — `ControlClass` as capture's filter
language; this adds a *binding* that targets a class, and a fourth class, `CharacterProducing`,
whose membership depends on the event rather than the control.

**Not `InputAction`.** The first design pass reused `InputAction`/`ActionOutput` so a class binding
could dispatch through the same `Fired<A>` observers everything else does, and immediately hit a
type it had no honest answer for: `ActionValue` has four shapes (`Bool`, `Axis1`, `Axis2`, `Axis3`),
none of which is "which control fired, with its original event." Making `Control` satisfy
`ActionOutput` would have meant an `into_action_value`/`from_action_value` pair that is never
actually called, `unreachable!()`'d out — dead code standing in for a conversion nothing needed. A
class binding never enters the per-tick fold, carries no modifiers or conditions, and has nothing to
hold between ticks, so it got its own trait (`ClassBinding`, one associated `PATH`) and its own
event (`ClassFired<A>`, carrying the original `RawEvent` untouched) instead. Smaller than reusing
`InputAction` would have been, once the reuse stopped being free.

**The payload is the raw event, not a synthesized `Control`.** The motivating consumer is a
text-edit widget, which wants what `bevy_input_focus`-style widgets already work with — the
logical key, the text, the repeat flag — not an identity it would have to reconstruct those from.
`RawEvent` already carries all of it and was already `Clone + Debug + PartialEq`, so it is the
payload verbatim. Different classes want different fields out of it; that is left to the app,
matching to `RawEvent::Keyboard` or reading `RawEvent::control()` for the identity-only case,
rather than this crate inventing one payload shape to fit every consumer.

**The plan's second structure is a membership test, not a re-derivation of arbitration.** Design
§4.1 describes "the per-control index" as something a class binding is consulted behind; in this
evaluator (which folds from held state each tick rather than dispatching per binding per event)
that turned out to mean exactly one thing: `Plan::indexed_controls`, every control any plain binding
in the context reads, recomputed on every compile — including a variant's, since a rebind changes
which controls are indexed even though it never touches the class list itself. A class binding
never wins a control by being more specific; it is excluded outright the moment any plain binding
names that control, which is what "it only ever wins by sitting in a higher-priority context"
(§4.1) already said, just enforced up front rather than re-litigated per tick.

**`character_producing` was measured, not reasoned, using a new example**
(`examples/ime_diagnostic.rs`) **run against real input on the author's machine.** A macOS kana
input source composed correctly: every keystroke arrived as its own `Pressed` `KeyboardInput` with
`text: Some(single kana character)`, and the matching `Released` always carried `text: None`. That
is exactly what the predicate (`text.is_some() && state == Pressed`) assumes, and it held with no
exceptions across the run. A dead key looked wrong at first — Option+I then A, which should compose
to `â`, arrived through this crate's bare diagnostic window as two independent plain letters, `i`
then `a` — but a side-by-side run of Bevy's own text-input example against the same keystroke
produced one composed character (visible as the font's missing-glyph box, not two letters), which
means the composition the diagnostic window was missing is a property of *that window* — most
likely IME not being enabled on it — and not something winit or Bevy fails to deliver in general.
So the predicate needs nothing extra for dead keys: wherever composition happens upstream, it
already lands as one `KeyboardInput` with `text: Some(the composed character)`, which is the exact
shape the predicate already recognizes. Left genuinely unmeasured: committing a multi-candidate
conversion through an IME's candidate popup. Reasoned rather than measured: it should be fine,
since that commit happens through ordinary keystrokes (Space or Enter to pick a candidate) that the
same per-event rule already judges independently — recorded as a comment next to the predicate
rather than a chunk, since nothing about it currently looks wrong.

**Deliberately not here.** Wiring an actual focused text field to `CharacterProducing` → 49, which
already owns "the rest of D4" per Requirements.md's own note by R8.4/R12.6. Nothing in-tree
declares a class binding yet; the mechanism and its tests are the whole of this chunk, same as the
roadmap section said going in.

### Chunk 56: Split Friction's tileset

`examples/split_friction/` did not exist before this chunk — chunk 27 (the device-selection screen
this example exists to demonstrate) has not landed, contrary to how it reads elsewhere in this
document's own recent history. This chunk needed nothing from it and landed anyway; `main.rs` today
is only a hand-placed room proving [`tileset`]'s indices, with no `bevy_action_map` usage at all.
Chunk 27 is still owed before the game is actually about anything.

**Kenney's own tiles have no names, but Kenney's own sample map does.** Tiny Dungeon ships
`tile_0000.png`…`tile_0131.png` — numbered, not described — so a tile's role has to come from its
pixels. Eyeballing a scaled preview image got this **wrong**: a crop meant to isolate one quadrant
for closer inspection landed on a row boundary that wasn't a multiple of the tile size, and every
row/column read off it afterward was off by a fraction of a row. The reliable source turned out to
be the `.tmx` sample map Kenney bundles alongside the sheet — a real dungeon Kenney built from these
same tiles, with real tile-index data in its CSV layers. Decoding it (stripping Tiled's flip-flag
bits from each GID) and rendering it back with the actual atlas confirmed the indices immediately: a
coherent dungeon, not noise. Every constant in `tileset.rs` was checked against that render rather
than read off a picture.

**`Sprite`'s `texture_atlas` field wants a `TextureAtlasTemplate`, not `Option<TextureAtlas>`
constructed by hand.** `Some(TextureAtlas { layout: ..., index })` inside `bsn!` fails two ways in
sequence: `Some(...)` itself is parsed as a component-construction call (`bsn!`'s own syntax for
`Foo(...)`), and even past that, the field's derived template type (`OptionTemplate
<TextureAtlasTemplate>`) doesn't accept a raw `TextureAtlas` literal. Bevy's own
`examples/usage/cooldown.rs` (for `ImageNode`, the UI equivalent) has the answer: write a plain
function returning `bevy::image::TextureAtlasTemplate { layout: handle.into(), index }`, and assign
its *call* to the `texture_atlas` field directly, no `Some` and no manual `Option`. The handle itself
still wants building the ordinary way — `Assets<TextureAtlasLayout>::add` in a system with resource
access, passed down as a plain `Handle` parameter — rather than through `asset_value`, which exists
for constructing an asset inline and would have added one duplicate `TextureAtlasLayout` per tile.

**A `Children`-only scene root needs `Visibility` alongside `Transform`, or its sprite children warn
every frame.** `bsn! { Transform::default() Children [...] }` runs, but Bevy's hierarchy propagation
logs a B0004 warning per child once render systems notice the parent has no `Visibility` for
`InheritedVisibility` to propagate through. Adding `Visibility::default()` next to `Transform::
default()` on the root silences it; nothing else in the render output changes.

**Found, not fixed: a door's art doesn't share the plain wall-top tile's silhouette.** `DOOR_CLOSED`
(index 46) has its stone arch reaching slightly higher within its 16×16 cell than `WALL_TOP` (index
2) does, so a door dropped into a wall run pokes up above the row's otherwise level skyline.
Cosmetic at this scale, and left as a note for chunk 57 rather than chased here — a generator
choosing where doors go is the more natural place to decide whether that matters.

### Chunk 57: A generated dungeon

Landed twice. The first pass built what the roadmap section described — rooms placed as
non-overlapping rectangles, joined pairwise by straight 1-wide corridors — and it passed every unit
test written for it (determinism, connectivity, no directional wall standing with no floor beside
it). Looking at the render was still worth doing: reviewed against a 64×64 Gauntlet-sized playfield,
it read as a maze of small disconnected rooms threaded by narrow halls, and a maze was not the
target. **Gauntlet's actual shape is one open arena with obstacles in it, not a maze** — the fix was
a different generator, not a bigger one: floor now fills the whole interior by default, and a
handful of solid rectangles are punched back out of it, kept apart from each other and from the
outer wall by [`CLEARANCE`](examples/split_friction/dungeon.rs) cells of guaranteed-open floor
(2, so a walkway never narrows to a single-tile squeeze). No corridor-carving code survived; there
is nothing left for it to connect.

**Every solid cell needed to draw *something*, not "wall or nothing."** The room-and-corridor
version only ever placed a wall tile adjacent to floor, by construction, so anything not floor and
not wall-adjacent was background — invisible, correctly, since it was always outside every room.
Punching obstacles out of an otherwise-solid-free arena breaks that assumption: an obstacle's own
interior, and the corners of the outer border no directional rule reaches, are solid cells with no
floor neighbor in any of the five directional patterns. Left as `Empty`, those cells rendered as
literal holes — the arena's obstacles looked hollow, and the outer boundary had gaps at its
corners. The fix is a sixth role, `WallFill`, the fallback for solid ground no directional piece
claims, drawn from one of the flat brick tiles (`tileset::WALL_FILL`, index 36) that were sitting
unused in the atlas already. `TileRole::Empty` is gone entirely now: every cell resolves to a real
tile, so `tile_index` returns `usize` rather than `Option<usize>` and the render loop lost its
`filter_map`.

**A real bug, found while rewriting the file for the design change rather than while fixing it.**
The floor branch of `resolve_cell` originally read: plain floor if the cell to the north is *also*
floor, otherwise shadow-or-decorated depending on a second check
(`is_wall_neighbor_north`) that, read closely, always agreed with the first — it was called only
from the branch where north was already known not to be floor, so its own "is north floor"
disjunct was a tautology, and the other two disjuncts it `||`ed against never mattered. Net effect:
a floor cell only ever got a decorative variant when its north neighbor was floor, which silently
excluded every shadowed row from ever showing a variant and was three lines doing the work of one.
Replaced with the obvious rule — shadow if north is not floor, decorated floor otherwise — and the
now-redundant helper deleted outright rather than kept dead.

**Deliberately not here.** Doors: the previous chunk's `DOOR_CLOSED` constant has no generator-side
placement logic yet, and isn't reintroduced by this chunk — nothing currently carves an opening that
would want one. T-junctions and non-rectangular obstacle shapes: the directional-wall rule only
promises a plain outer corner, which is what a rectangle always presents; an obstacle shape this
rule cannot draw correctly was never generated in the first place, rather than generated and drawn
wrong.

### Chunk 58: `follow` replaces per-binding `follows`

Found while scoping chunk 31, which inherited chunk 38's leftover: a follower drawn as riding every
slot of a row when it had only declared `.follows()` on some of them. The roadmap's proposed fix was
a plan-build diagnostic requiring full coverage. Looking at why partial coverage was writable at all
found the actual defect one layer down: `.follows::<A>()` was a per-binding declaration, so a
follower had to retype every control its leader already named — once per device, verbatim — and
nothing tied the *count* of those repeats to the leader's own. A diagnostic would have caught the
typo after the fact; it would not have removed the reason to make one.

**`InputContextBuilder::follow::<Follower, Leader>(configure)` replaces `BindingHandle::follows`.**
It reads every binding `Leader` has declared, generates one matching binding of `Follower` per
control found, and runs `configure` on each — `controls.follow::<Afterburner, Thrust>(|b|
b.hold(0.75))` where three hand-written `.bind().follows::<Thrust>()` calls stood before. Partial
coverage of one row is no longer expressible: there is nothing left to under-declare, since the
bindings are derived rather than retyped.

**Resolves against the leader's bindings *so far*, not a snapshot taken at the end — a deliberate,
costed choice.** An order-independent version (a pending list, resolved once every binding is known)
was designed and rejected: it needs a boxed closure held in builder state to survive past the
`follow` call, a struct re-deriving what `push_binding` already gets for free from a type parameter,
and — because diagnostics run before the builder is finished — a second call site in `context.rs`,
resolving pending follows before `report_diagnostics` runs so a private leader binding is still
caught. Roughly three to four times the code, in two files, for a pattern nothing else in this
builder uses. The order-dependent version costs none of that, and the rule it imposes — declare what
you are naming before you name it — is the same rule every other call in this builder already
follows. It has one real consequence: `examples/capture.rs`'s `WallJump` follows `Jump`'s keyboard
bindings and not its pad one, and that is now because `follow` is called before `Jump`'s pad binding
is declared, not because of a third follows-call nobody wrote.

**The internal representation is untouched, on purpose.** `FollowsDecl`, `leader_of`'s
match-by-source resolution, `rewrite_followers`, and the second pass in `mappings_of` all still work
exactly as before — `follow` only changes how the `BindingSpec`s they operate on get authored, not
what they are once declared. Follows is a build-time and rebind-time bookkeeping question; the
evaluator never knows a binding is a follower at all, so there was nothing to gain at that layer from
also making the relationship action-based rather than per-binding, and a real cost (losing the
"provably reads what it claims to ride" self-check that source-matching gives for free).

**One test deleted outright rather than adapted.**
`following_an_action_that_reads_something_else_is_refused` exercised a mistake — a follower
hand-bound to a control its leader never used — that `follow` cannot construct, since its sources
are always copied from the leader. Likewise `a_binding_cannot_be_mappable_and_follow` (`mappable`
declared *before* `follows`): `follow` always applies `follows` first, so that order is unreachable
through the public API; the reverse order (`follows` then `mappable`, attempted inside `configure`)
still panics, off the same guard in `declare_mapping` that was already independent of how `follows`
got there.
