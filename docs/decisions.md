# Decisions

Why `bevy_action_map` is shaped the way it is. [`design.md`](./design.md) says how it works and does
not say why; this is the other half.

**What earns an entry.** A decision belongs here only if reversing it would break the public API,
the save format, or the shape of the evaluation pipeline. The test is that the reversal cost can be
named in a sentence. If it cannot, it is a code comment, not a decision.

Each entry says what was decided, what it rules out, and what reversing it would cost. Where a
decision has an accepted price or an unresolved remainder, that is stated rather than left for a
reader to discover.

Numbers are identities, and an entry is never renumbered — a withdrawn one is struck and kept.

`Requirements.md` tags requirements with these numbers, and is the only other document that cites
them. It once carried a `D1`–`D9` of its own; that table is gone and its references were remapped
here, so there is one `D`-numbering in the project.

---

| #       | Decision                                                                      | Mechanism       |
| ------- | ----------------------------------------------------------------------------- | --------------- |
| **D1**  | Four layers, and L2 reads only L1                                             | design §1       |
| **D2**  | L1 is an event queue, not a level snapshot                                    | design §2       |
| **D3**  | Each context reads by cursor; retirement is separate and later                | design §2       |
| **D4**  | Timestamps order events; they do not time them                                | design §2       |
| **D5**  | An action is a type                                                           | design §3       |
| **D6**  | Serialized identity is a declared path, not the Rust type path                | design §3.3     |
| **D7**  | Intent is separate from output shape and from channel shape                   | design §3.1     |
| **D8**  | Action state is two dense tables; actions are not entities                    | design §6       |
| **D9**  | A context declares one tick domain and is evaluated once                      | design §1, §3   |
| **D10** | Bindings compile once into an immutable, shared plan                          | design §4       |
| **D11** | Arbitration splits: priority is system ordering, chord length is a pre-pass   | design §5.1     |
| **D12** | Consumption is recorded per schedule and flows forward                        | design §5.2     |
| **D13** | Exclusivity is a ceiling, not a third context state                           | design §5.3     |
| **D14** | A class binding is a second list, not an expanded set of controls             | design §5.4     |
| **D15** | Intent decides how several bindings fold into one action                      | design §5.5     |
| **D16** | Nothing user-defined runs inside the evaluator                                | design §5.6     |
| **D17** | Transitions are generic entity events on the context entity                   | design §5.6     |
| **D18** | Require-reset holds back buttons only                                         | design §7.2     |
| **D26** | Failures surface at the earliest tier that can catch them                     | design §4, §7.3 |
| **D19** | Modifiers and conditions are enums with a `Custom` variant                    | design §8.2     |
| **D20** | We own the whole dead-zone chain, in three stages, with one rescaling         | design §8.4     |
| **D21** | Calibration is measured by an explicit step, never detected                   | design §8.4     |
| **D22** | Backends enter at two seams, not one                                          | —               |
| **D23** | Focus integrates by activation, and interception is static                    | —               |
| **D24** | One crate, feature-gated by source                                            | design §11      |
| **D25** | What must not move upstream                                                   | —               |
| **D27** | The presentation model is separate from the binding model                     | design §9       |
| **D28** | Listing is the default; rebinding is opt-in                                   | design §9.1     |
| **D29** | A mapping is an ordered list of slots, and capacity is inferred               | design §9.1     |
| **D30** | `follow` declares a shared control once, against the leader's bindings so far | design §8.2     |
| **D31** | Every player-facing string is a key; the mapping owns the name                | design §9.1     |
| **D32** | A tunable is typed, so a settings screen is generic                           | design §9.1     |
| **D33** | A preset is a starting point, not a layer                                     | design §10.2    |
| **D34** | The reverse lookup is a trait, and the answer is not a `Control`              | design §9.2     |
| **D35** | A prompt is not a row of the settings screen                                  | design §9.2     |
| **D36** | The device is a scope the caller supplies; ranking devices is refused         | design §9.2     |
| **D37** | A prompt reads consumption from the declarations, not the frame               | design §9.2     |
| **D38** | Staleness is a counter, and the crate says what it cannot see                 | design §9.2     |
| **D39** | The control name table is ours, and one name is both identity and key         | design §9.2     |
| **D40** | Capture reads the frame directly, not through a binding                       | design §9.3     |
| **D41** | A capture session is a component on whatever entity the caller picks          | design §9.3     |
| **D42** | Reserved before shape, and excluded is a silent guard                         | design §9.3     |
| **D43** | Conflicts are detected, never resolved                                        | design §9.3     |
| **D44** | Two general combinators, not a navigation path                                | design §8.3     |
| **D45** | An override is a diff keyed by mapping and scheme, holding controls only      | design §10      |
| **D46** | Three row states, not two                                                     | design §10      |
| **D47** | Applying is the only path in, and overrides do not compose                    | design §10.1    |
| **D48** | Applying rewrites the authored bindings; a variant keeps the declared slots   | design §10.1    |
| **D49** | The control encoding is a format we own                                       | design §10.3    |
| **D50** | Loading is pure, and reports rather than drops                                | design §10.3    |
| **D51** | An authority backend writes a value, not a state                              | —               |
| **D52** | Pairing is a runtime handle; the join gesture reuses class bindings           | design §7.4     |
| **D53** | The crate detects and reports; the app decides                                | —               |
| **D54** | There is no pass-through action                                               | design §5.5     |
| **D55** | State-driven activation runs inside `StateTransition`                         | design §7.2     |
| **D56** | Activation answers per context type, and is declared on the builder           | design §7.2     |
| **D57** | Where two pads report one axis, the one that moved last speaks                | design §7.4     |

---

## Layering and the input frame

### D1 — Four layers, and L2 reads only L1

**Decided.** Sources, input frame, mapping, consumers. The mapping layer never reads
`ButtonInput`, `Axis`, or a raw message stream; it consumes the frame and nothing else.

**Rules out.** Reading device state from inside the evaluator, which is the shortest path to every
feature in this list and the one that forecloses the rest of them.

**Reversal.** Determinism, replay, headless testing, and both backend seams stop being one mechanism
and become four features, each needing its own way in. This is the rule the others are built on;
reversing it is not a change to the crate, it is a different crate.

### D2 — L1 is an event queue, not a level snapshot

**Decided.** The frame is an ordered queue of raw events with a position stamped on each, not a
per-frame snapshot of which controls are down.

**Rules out.** The model `bevy_enhanced_input` and `leafwing-input-manager` both use — sampling
`pressed()` inside the evaluator. That model is simpler and more universal: anything that can write
`ButtonInput` becomes an input source with no adapter, which for an engine-level crate is a serious
virtue that this crate gives up.

**Reversal.** A press and release inside one rendered frame collapse to nothing, so a fixed tick
spanning them sees neither. `InputFrame`, `RawEvent`, `Timestamp` and the whole capture path go with
it, and the frame stops being the serializable per-tick record that replay and rollback need.

### D3 — Each context reads by cursor; retirement is separate and later

**Decided.** A context instance keeps its own position in the queue and reads what arrived since.
Discarding events is a second, independent step, running in `FixedPreUpdate` after fixed-tick
evaluation.

**Rules out.** A single global read position, and retiring at sample time.

**Reversal.** The two halves look redundant and are not. Retirement alone fails when the simulation
does not step: nothing is retired, the next frame appends to what is still queued, and a render
context reads events it has already acted on. Cursors alone fail by unbounded growth. Retiring at
sample time — which is what this did originally — discards events before a fixed tick that has not
yet run can see them, and is what made a frame with zero fixed ticks lose edges.

**Accepted cost.** The invariant is not local to the frame module: it holds only while evaluation
stays in `PreUpdate` and `FixedPreUpdate`. Moving either schedule breaks it silently. A system
reading the frame from `Update` sees contents that depend on whether the simulation stepped.

### D4 — Timestamps order events; they do not time them

**Decided.** A `Timestamp` is a frame counter and an order within that frame, stamped as the event
is sampled.

**Rules out.** Attributing an event to the instant it truly occurred, and therefore attributing it
to the fixed tick it truly fell in. Bevy's input events carry no time of their own, so there is
nothing to stamp with.

**Reversal.** Cheap, and expected. Every event in a frame compares equal on the only axis a time
window could split, so the first fixed tick to run takes all of them and later ticks in that frame
take none. Magnitude is conserved and each edge is seen exactly once either way; what changes when
real timestamps arrive is the attribution policy alone, not the shape of anything public.

**Still open.** Gamepad events are coarser again, because gilrs is polled once per frame, so they
arrive as a batch regardless of what keyboard and mouse gain. Timing-sensitive conditions are
therefore less precise on a pad than on a keyboard.

---

## The action model

### D5 — An action is a type

**Decided.** An action is a Rust type carrying its output type, its intent and its path as
associated constants, written with a derive.

**Rules out.** Enum actions, string-keyed actions, and declaring an action at run time. The set is
open — any crate may declare one — which is what a closed enum cannot express.

**Reversal.** Every read in the crate is generic over `A: InputAction`. `value::<Move>()` returning
`Vec2` rather than an `ActionValue` a caller has to unwrap depends on it, as does catching a shape
mismatch at compile time. The type-erased path in `inspect` exists precisely because this one cannot
serve a debug overlay, and it would become the only path.

**Accepted cost.** Modding and other run-time declaration are out of scope; the answer is a mapping
action, bound once and dispatched by the game.

### D6 — Serialized identity is a declared path, not the Rust type path

**Decided.** Every action and context declares a `PATH` such as `"gameplay.jump"`, and that string
is what a settings file stores. It is required, and unchecked against the Rust type name.

**Rules out.** Using the reflected type path as the save key, which is what makes moving a type
between modules a save-data migration.

**Reversal.** A save-format break, and worse, a silent one: renaming `Move` to `MoveOnFoot` or
relocating it would orphan every binding a player has saved against it. The registry is keyed by
path for the same reason, which is why it is not the reflect type registry.

**Accepted cost.** A second name to keep straight, and no compiler check that it is unique. The
naming convention in design §3.3 is what stands in for one.

### D7 — Intent is separate from output shape and from channel shape

**Decided.** Three properties, not one. The output is the Rust type; the `Intent` is what the value
means; the `ChannelShape` is what the control reports. They do not have to agree, and on real
hardware they frequently do not.

**Rules out.** Inferring meaning from the Rust type. A stick deflection and a mouse delta are both
`Vec2`, and one is a position implying a rate while the other is a displacement that already
happened.

**Reversal.** The fold in D15 has nothing to key on, and binding admissibility has nothing to check,
so summing a stick position into a mouse delta becomes expressible. Intent is also what a rebinding
UI filters candidate controls on, so the capture path loses its constraint.

### D8 — Action state is two dense tables; actions are not entities

**Decided.** Per context instance: one `ActionState` per action and one `Scratch` per condition and
per stateful modifier, both dense arrays of `Copy` types indexed by plan slot, plus a per-action
dirty bitset. Parameters — durations, thresholds — live in the immutable plan and never in state.

**Rules out.** Action-as-entity, a packed byte buffer, and a typed tuple per action. The framing
that made the last two look necessary was the error: the half that appeared to need a variable size
belongs to _bindings_ rather than to actions, and once its parameters move into the plan what is
left is uniform.

**Reversal.** Snapshot and restore stop being two slice copies and become an archetype traversal.
Activation stops being a flag and costs an insert or a removal per action. Per-action change
granularity, which the bitset gives, is not something a single component's change tick can express.

**Accepted cost.** None that was measured. The one criterion action-as-entity was uniquely strong on
turned out not to exist: a generic entity event carries the action identity in its type parameter
and targets the context entity, so every layout supports per-action observers equally.

### D9 — A context declares one tick domain and is evaluated once

**Decided.** `TickDomain::Render` or `TickDomain::Fixed`, declared on the context. A render context
evaluates in `PreUpdate`, a fixed one in `FixedPreUpdate`, each once per run of its schedule.

**Rules out.** Evaluating every context at both rates, and keeping two states per context with
per-tick accounting. Both double the state and the work to serve a case most actions never need.

**Reversal.** One state table per instance becomes two, and the accessor type stops following from
the domain, so reading a fixed-rate action from a render-rate system stops being catchable.

**Accepted cost.** An action wanted at both rates is declared in two contexts. In practice the split
falls where the semantics already differ — a camera wants the newest delta every frame, movement
wants one sample per simulation tick.

**Still open.** Enforcement is not airtight. `Actions<OnFoot>` where `OnFoot` is `Fixed` should be
unreadable from `Update`, but Bevy gives a `SystemParam` no way to know its own schedule. What
stands in is a plugin-time validation pass and a debug assertion.

---

## Plan and evaluation

### D10 — Bindings compile once into an immutable, shared plan

**Decided.** Authored bindings are compiled into a `Plan`, immutable and `Arc`-shared between every
instance of that context. Compilation assigns slots, computes chord lengths, and resolves the
control index. Applying an override compiles a _variant_ plan and swaps it in.

**Rules out.** Interpreting authored bindings per frame, and rewriting the declared bindings in
place when an override is applied.

**Reversal.** Ten local players sharing one binding set would hold ten plans, not one plan and
ten small state tables. More seriously, an override is a diff against the defaults, so the defaults
have to still be there to diff against — rewriting in place destroys them on the first apply, and
the next patch's revised defaults would never reach a player who never touched that row.

**Accepted cost.** Two players with different remaps need different plans, so per-player rebinding
multiplies plans rather than only tables. The state tables are the part that scales with players, so
this was judged acceptable rather than free.

### D11 — Arbitration splits: priority is system ordering, chord length is a pre-pass

**Decided.** The obvious shape — one list of every binding touching a control, sorted by context
priority and chord length — is not built. Priority becomes system ordering, fixed at app build:
each distinct priority gets its own system set, ordered against the others in its schedule. Two
contexts declared at the same priority get their own nested set in turn, ordered after the one
before it — declaration order breaks the tie a priority number left open, the same way it already
does everywhere else two things read together (D15's fold, D36's prompt scan). Chord length is
resolved by a pre-pass within one evaluation, finding the longest satisfied chord on each control
before any binding is read.

**Rules out.** The single sorted list. It cannot be built: a plan belongs to one context and cannot
see another's bindings, and one list cannot span two schedules.

**Reversal.** Both halves are fixed before the frame starts, which is what makes the pass
deterministic with respect to system scheduling. A per-frame arbitration decision would reintroduce
the dependence on scheduling order that this exists to remove.

### D12 — Consumption is recorded per schedule and flows forward

**Decided.** A consumed control is recorded per schedule. Each schedule clears its record when it
runs, and the frame's is cleared once at the top.

**Rules out.** Priority as a total order across tick domains.

**Reversal.** A render-tick context can take a control from a fixed-tick one; a fixed-tick context
cannot take one from a render-tick one, whatever the priorities say.

**Accepted cost.** That limitation is real and stated rather than hidden. It is also the direction
every motivating case runs in: the things that claim controls are menus, text fields and modal
overlays, which are render-tick, and the thing being claimed from is gameplay, which is fixed-tick.
If the reverse is ever wanted, the answer is to give the claimant a render-tick context rather than
to reorder the frame. `bevy_enhanced_input` reached the same arrangement from the same starting
point.

### D13 — Exclusivity is a ceiling, not a third context state

**Decided.** A context declared `exclusive` raises a ceiling to its own priority. Any context at or
below the ceiling is _shadowed_ — cancelled and held inactive — regardless of its own activation
condition. Shadowing is tracked in its own field rather than by clearing `active`.

**Rules out.** A third activation state, and making a modal screen enumerate the actions it takes.

**Reversal.** `active` is what the context's own condition wants and `shadowed` is what is being
forced on it; collapsing them into one flag makes the two fight, so a shadowed context would resume
or fail to resume depending on which wrote last. The diagnostic path needs no new case either — a
shadowed context is genuinely inactive, so "context inactive" already names it.

**Still open.** The ceiling is global. Nothing in it says whose exclusive context raised it, so in
local multiplayer one player's modal would shadow every other player's gameplay context. Single
player is unaffected.

### D14 — A class binding is a second list, not an expanded set of controls

**Decided.** A binding may name a class of controls rather than a control. The plan carries these as
a separate list, consulted only for an event on a control no plain binding in that context indexes,
and what is dispatched is the original raw event rather than a folded value.

**Rules out.** Expanding a class to its member controls at compile time. That is unavailable for the
class that matters most: which keys are character-producing is a function of keyboard layout and
live IME state, so the membership is not known when the plan is built.

**Reversal.** Without the second list there is nothing for a text field to claim the keyboard
with short of the app enumerating every key.

**Consequence worth stating.** A class binding is maximally unspecific, so D11's clash rule works
against it: a plain `W` bound in the same context beats "any character key", and so does `Ctrl+S`. A
class binding never wins by specificity — only by sitting in a higher-priority context, which is
exactly how a text field is meant to claim the keyboard.

### D15 — Intent decides how several bindings fold into one action

**Decided.** State is allocated per action, so several bindings feeding one action are folded into
one value. `Button`, `Analog1` and `Directional2` take the strongest contribution; `Delta2` sums.
Ties keep the earlier contribution, so declaration order is the tiebreak.

**Rules out.** Summing everything, which is the obvious default and produces a value with no meaning
when a key press is added to a stick position. Also per-binding state, which would avoid the
question by multiplying what a rebinding UI and a rollback snapshot have to handle.

**Reversal.** Two half-deflected sticks would read as a full deflection, and either of two jump
buttons would jump twice.

**Note.** Keying the fold on intent stops the units error between actions. What stops a mouse delta
being bound to a directional action in the first place is D7's channel check, at declaration time.

### D16 — Nothing user-defined runs inside the evaluator

**Decided.** Evaluation writes state and appends to a transition log. A separate system drains that
log and dispatches to observers.

**Rules out.** Calling observers from the evaluator.

**Reversal.** Observers run arbitrary code with `&mut World`, so running them inside the evaluator
makes it impossible for evaluation to be a pure function of the frame — which D1 exists to
establish and which rollback depends on. During resimulation the log is discarded rather than
drained, so a resimulated tick does not re-fire observers or re-dispatch effects.

**Accepted cost.** An effect fired during a resimulated tick is discarded, so an action whose only
observable result is an effect is invisible to rollback. Correct for UI dispatch; a gameplay action
that wants to work that way would be wrong.

**Consequence.** Because the log records transitions rather than final state, an action that fires
and completes within one tick produces two entries and two observer calls, in order — which a "read
the current phase" model cannot express.

### D17 — Transitions are generic entity events on the context entity

**Decided.** `Fired<A>`, `Started<A>`, `Completed<A>`, `Canceled<A>` are generic `EntityEvent`s
targeted at the entity carrying the context. The generic parameter carries the action identity, so
no per-action entity is needed.

**Rules out.** Concrete per-event structs, and a resource-level event stream.

**Reversal.** `bevy_picking` flattened its generic `Pointer<E>` into concrete types and could do so
because its event set is closed and known at compile time. Ours is open by D5 — any crate may
declare an action — so there is no finite set of concrete types to write. Picking's direction of
travel is not an argument against this one.

**What it buys.** `bevy_scene`'s `on()` takes any `EventPattern` over an `EntityEvent`, so `bsn!`
attaches observers to these with no adapter and no dependency in either direction. A scene can
declare that an entity has an input context and how it responds, with no system and no registration.
That is also why the context type is a component rather than a handle returned by a builder.

### D18 — Require-reset holds back buttons only

**Decided.** Activating a context arms a latch so a control the player was already holding does not
read as a fresh press. The latch holds back `Button` actions; an analog action's value simply
resumes.

**Rules out.** Applying the latch uniformly.

**Reversal.** Found by building it. Applying an override re-arms the latch, and removing a
stage-2 dead zone entirely leaves a drifting stick that is never seen at rest — so the action never
recovers. What the latch guards against is a _fire_ synthesized from a control already held, and an
analog action has none to synthesize.

### D26 — Failures surface at the earliest tier that can catch them

**Decided.** Three tiers. The compiler catches a wrong output shape. Plan build catches an unknown
control, a duplicate binding, a shape mismatch no conversion can fix, contradictory consume flags
and the rest, and `add_context` refuses the context rather than installing a plan that cannot work.
A runtime query answers everything situational: `why_not` returns an `Obstacle` naming which of the
several possible reasons applies.

**Rules out.** Deferring everything to run time, and failing silently at any tier.

**Reversal.** The runtime tier is the one that pays for itself and the one that is public API.
"Why didn't my action fire" has at least six causes that are indistinguishable from the call site —
inactive context, a higher-priority consumer, a longer chord winning the clash, a condition part way
through, a control nothing has touched, and an action simply not bound here — and the plan already
holds everything needed to tell them apart. Removing `Obstacle` leaves a developer with a printf.

**Why the tiers are separate passes.** Diagnosis is a distinct pass from compilation, not a step
inside it. That is what lets a rebinding UI ask whether a set of bindings is admissible without
having any intention of installing them.

---

## Extension points

### D19 — Modifiers and conditions are enums with a `Custom` variant

**Decided.** Built-in modifiers and conditions are enum variants; a third party implements the
`Modifier` or `Condition` trait and arrives through a `Custom` variant holding an `Arc`. Built-ins
dispatch statically and stay exhaustively matchable; extensions work.

**Rules out.** A closed enum, which blocks third-party modifiers outright, and trait objects
throughout, which cost dispatch on every binding every tick.

**Reversal.** The boxes are allocated during plan compilation, never in the steady state. Going to
trait objects throughout moves that cost into the per-tick path.

**How the framing changed.** This was posed as a trade of ergonomics against _serializability_ —
trait objects versus a reflected registry. That trade turned out not to apply. An override stores
controls and tunable values only; modifiers, conditions and chord structure are developer data and
never reach a save file. So `Modifier` and `Condition` carry no `Reflect` bound and custom
extensions are not serialized, because nothing asks them to be. The `Arc` rather than a `Box` is for
an unrelated reason: applying an override clones the authored bindings and rewrites their sources,
and the originals have to survive that intact.

### D20 — We own the whole dead-zone chain, in three stages, with one rescaling

**Decided.** Raw gamepad events are consumed before Bevy's own axis processing, and the dead zone is
modelled as three stages with different owners: calibration per device unit, design per binding,
preference per player. At most one stage may rescale, and it is the design stage.

**Rules out.** Reading Bevy's processed gamepad events, and treating the dead zone as one negotiated
number that three parties argue over.

**Reversal.** Rescaling is what makes a dead zone feel like nothing was taken away, and the design
stage is the only one whose threshold the player never sees a number for. If calibration rescaled
instead, a developer's `0.15` would stop denoting a physical stick position. The rule is enforced
where a plan is compiled: a chain stacking two rescaling modifiers is rejected, and
`Modifier::rescales` carries the same obligation to third-party modifiers, defaulting to `false`.

**Why calibration runs at sampling.** Drift is a wear characteristic of one physical unit, and the
raw message names the unit that sent it. Correcting there costs one pass instead of one per context,
and it means the evaluator never has to hold per-device state. It also puts calibration on the
correct side of the injection seam: a backend supplying its own values writes into the frame past
it.

### D21 — Calibration is measured by an explicit step, never detected

**Decided.** Stage 1 ships a manual API plus a sampling helper the app drives during an explicit
step, not background auto-detection.

**Rules out.** Learning a stick's centre while the game is running.

**Reversal.** A stick deflected while detection is running would be learned as centre, and hardware
that misreports would poison the measurement silently — which is the failure mode this exists to
prevent, so a background detector is worse than no calibration at all.

**Note.** The instruction to the player is "move the sticks and let go", not "hold still", because a
pad reports an axis only when it _changes_. A stick that settled before the step began reports
nothing during it, and that is exactly the drifting stick most in need of measuring.

**Accepted cost.** What is measured lasts as long as the process. Persisting needs a stable device
identity, which does not exist yet.

---

## Boundaries

### D22 — Backends enter at two seams, not one

**Decided.** A _source_ backend supplies the input frame and is indistinguishable from real hardware
to everything above it — replay, a network peer, a custom device, a test all use this door. An
_authority_ backend supplies action output directly, bypassing bindings, modifiers and conditions.
Steam Input is the second case: the binding UI, the conflict rules and the glyphs all live outside
the game.

**Rules out.** One backend trait, and scoping external bindings as a presentation concern. It is a
structural commitment, not a display one.

**Reversal.** Action state must be writable from outside the mapping pipeline, presentation must not
assume our binding tables exist, and rebinding must be delegable to someone else's UI. A design that
assumes our tables are always the authority cannot be retrofitted with any of those.

**Still open.** An authority backend's actions may not be able to participate in rollback at all,
since their state is not reproducible from our frames. The available answer is to record the
backend's output into the frame at sample time, at the cost of a larger frame.

**Also.** A backend authoritative for a device must be able to suppress that device at the source
layer, so its raw events never reach the frame. Preventing us from _computing_ an action the backend
owns does not stop us _sampling_ the hardware underneath it — and Steam presents a pad it is driving
as an emulated gamepad, which the platform enumerates and we sample, so every input arrives twice.
The same capability is what lets a replay backend mute live hardware.

### D23 — Focus integrates by activation, and interception is static

**Decided.** Focus _type_ activates a context; what a claim does once made is ordinary context
composition. There is no suppression mechanism and no second, bubbling arbitration. A
focus-activated context claims a control before dispatch, and a widget never decides at handling
time whether to let an input fall through.

**Rules out.** A bubbling interception pass beside the mapper's own, and dynamic interception.

**Reversal.** A mapper that already has priority and consumption does not need a second arbitration,
and adding one would give two mechanisms answering the same question differently. Dynamic
interception would break the single deterministic pass D11 establishes.

**The same rule one level down.** Consuming stops lower-priority contexts; it deliberately does not
stop the same action's other observers. An observer electing at handling time to suppress its peers
makes the outcome depend on which ran first — the same objection, and no more defensible for
observers than for contexts. The motivating case is answered better by the half that was kept: the
UI's context claims the control, so the gameplay action never fires at all, and `why_not` can name
the context that took it.

**Why bubbling was never the requirement.** `FocusedInput` bubbles because focus is the only
arbitration `bevy_input_focus` has of its own — a widget that declines a key lets it fall through to
whatever is listening further up the entity chain. A mapper with priority and consumption already
answers that question earlier: whether something else claims a control is decided by evaluation
order before a focus-activated context ever runs. A drop-in replacement for `InputDispatchPlugin`
was designed and built and then set aside, because building it with no widget in tree that needed it
was the thing to avoid; what replaced it needed no crate change at all, and is a context per widget
kind activated by an ordinary run condition.

**Accepted cost.** A widget kind with no context of its own gets no keyboard or gamepad input at
all, since the game disables the default dispatch plugin outright. That is the same additive bet the
presentation surface makes elsewhere, and it is a real cost of being explicit rather than a free
lunch.

### D24 — One crate, feature-gated by source

**Decided.** One crate with `keyboard`, `mouse`, `gamepad`, `serialize`, `focus`, `state`,
`bevy_reflect` and `std` features, plus the proc-macro crate Rust requires. Not a crate per layer.

**Rules out.** Five crates that must all bump together.

**Reversal.** The layers are real seams, but a seam is a module boundary before it is a crate
boundary. Splitting now would fix the API between layers before any code has tested it, and
cross-crate refactoring is far more expensive than moving a module. `bevy_input` is the precedent: a
single crate covering keyboard, mouse, gamepad and touch, separated by features, because Bevy splits
crates by domain rather than by internal layer.

**The gate for splitting.** A second crate wanting the frame without the mapping — a replay
recorder, a network transport, an input-debugging tool. Until such a consumer exists the split is
speculative, and the module layout makes it a move rather than a rewrite.

### D25 — What must not move upstream

**Decided.** Two things stay out of Bevy regardless of what else happens.

**Action mapping does not belong in `bevy_input`.** It is a policy layer over a data layer, with a
much larger API surface and far more contested design. Fusing them would make `bevy_input`
unadoptable for anyone wanting only raw input.

**Focus-context activation does not belong in `bevy_input_focus`.** It depends on the action
and context model, so putting it there inverts the dependency and drags the whole action system into
a crate that today does one small thing well. A `focus` feature here is the correct direction.

**Related, and enforced in the tree.** Nothing under `src/` may name Steam — not a feature, not a
variant, not a trait method. The real backend is `std`-only, `unsafe` FFI beneath, and wants the
Steamworks redistributable at link time, where this crate is `no_std` and forbids unsafe. So it is
someone else's crate, and the test of whether the seam is sufficient without being Steam-shaped is a
mock authority backend living entirely in `examples/`.

**Also enforced.** Nothing in the crate depends on `bevy_ui`. `bevy_ui` already depends on
`bevy_input` and `bevy_input_focus`, so depending on it would invert that layering and foreclose
`bevy_ui` ever using action maps itself. Everything that draws lives in the examples until it earns
a crate of its own.

---

## The presentation surface

### D27 — The presentation model is separate from the binding model

**Decided.** Players get a smaller model than developers: named *mappings*, typed *tunables* and
*presets*. Modifiers, composites, conditions and chord structure stay developer-only and never reach
a screen.

**Rules out.** Showing the binding model to players. It has no player-comprehensible reading —
nobody rebinding "move forward" should meet a swizzle.

**Reversal.** The save format follows from this. An override row holds controls because only the
source belongs to the player, which is also what removed serializability from the extensibility
question in D19. A presentation surface over the full binding model would have to serialize
modifiers, and the trade D19 records as dead would be live again.

**Cost.** Three additive declarations over bindings that already exist. A game that declares none
still gets a listed controls screen.

### D28 — Listing is the default; rebinding is opt-in

**Decided.** A binding is listed and fixed unless it says otherwise. `mappable` makes it rebindable,
`private` hides it, `follow` puts it on another row.

**Rules out.** Making listing follow rebindability, which is what the first draft did.

**Reversal.** Under opt-in listing a gamepad `Jump` with no mapping vanished from the screen
entirely — backwards for the commonest gamepad screen there is, where the console or Steam owns the
remapping and the game still wants to *show* the player what the pad does. The crate knew the
binding and refused to say so. Rebindability is the developer's call because a fixed binding is a
design decision; seeing the controls is the player's business, and the default belongs to them.

**`mappable` takes no arguments, and both halves of that are decisions.** The parts of a composite
name themselves, so the key derives as `gameplay.move.up` and a catalogue is where `up` becomes
"Move Forward" — an author supplying "forward" would be naming the same part twice, in a place no
translator will look. The scheme is inferred from the controls, because declaring it would be a
third chance to disagree with what is actually bound.

### D29 — A mapping is an ordered list of slots, and capacity is inferred

**Decided.** A *mapping* is the named thing a player rebinds; a *slot* is one position in it holding
one control; a screen draws one cell per slot. Capacity is `UpTo(n)` or `Any`, widened by whatever
the defaults ask for and never narrowed below them.

**Rules out.** One control per mapping, and a fixed two.

**Reversal.** One control per mapping cannot express the two-cell row every shipped game's keyboard
table has. The workaround it forced was a second row under an alias name — `thrust` and
`thrust_alt` — telling the player two things are separate when they are the same thing twice. A
fixed two cannot express the "add shortcut" button that tools grow instead.

**Save format.** A row holds a list because a mapping does, and position is which slot, so a cleared
middle slot needs the cleared marker rather than a shortened list — which would silently promote the
secondary to primary.

**Note on the nouns.** The first version called the row a slot and had to invent a second word for
the position. "Cell" was what it reached for, and a cell belongs to the table a screen draws rather
than to the model behind it.

### D30 — `follow` declares a shared control once, against the leader's bindings so far

**Decided.** Two actions that deliberately share one control — tap to dodge, hold to sprint — are
declared with `follow::<Follower, Leader>`, which reads whatever the leader has declared *at that
point* and generates a matching binding per device found.

**Rules out.** Declaring the link per binding, which is what shipped first, and inferring it from
two bindings happening to name one control.

**Reversal.** Per-binding declaration meant retyping a control the leader had already named, once
per device, and nothing checked that the counts matched — so a forgotten repeat produced a follower
that silently rode part of a row while being drawn as if it rode all of it. Left alone that is a
gameplay bug rather than a display oddity: rebind the throttle and the afterburner stays on the old
key, and whatever the player later puts there acquires an afterburner.

**Why it is never inferred.** Two bindings reading one control are as often a coincidence as an
intention, and conflict detection cannot tell the difference either — it looks for two rows holding
one control, and this failure is a *separation* that should not have been possible.

**Why "so far" rather than the leader's final shape.** It is an ordering rule, which is what lets a
follower ride only some of a leader's devices on purpose. Declare it before the rest of the leader's
bindings and only the ones already there are covered.

**Riding a fixed row is the ordinary case.** A pad binding that is listed-and-fixed has nothing to
rewrite, and keeping the duplicate row off the screen is worth having on its own. Requiring the
target to be `mappable` would have failed the build of the game this exists for.

### D31 — Every player-facing string is a key; the mapping owns the name

**Decided.** A mapping carries a localization key, not a label. So does a category, a control name
and a condition descriptor. The crate renders no player-visible English except through
`fallback_label`, which exists so that shipping translations is never the price of a legible screen.

**Rules out.** A label in the binding declaration, and baking any of the four into the crate.

**Reversal.** A label would be a second string to translate, sitting where no translator will look.
Half-localizing is the specific failure: leaving the action half of a rebinding row as a literal
while the control half is a key gives a screen that is half translated.

**Where each name lives.** The mapping owns the player-facing name and the action owns the category.
A composite settles the first — `Move` has four mappings and the player must be shown "Move
Forward", never "Move". Repetition settles the second: four movement mappings share one category,
and hanging it on each is four chances to disagree.

### D32 — A tunable is typed, so a settings screen is generic

**Decided.** A tunable is a named, typed value that overwrites one field of one modifier already on
a binding — a range or a boolean — enumerated beside mappings and persisted in its own table.

**Rules out.** Exposing modifier parameters directly, and untyped values a UI has to be told how to
draw.

**Reversal.** The type is what lets a UI render a slider or a checkbox without knowing it drives a
dead-zone threshold. Without it, a game adjusting anything has to write a bespoke control per knob,
and the promise that modifiers are never shown to players cannot hold.

### D33 — A preset is a starting point, not a layer

**Decided.** A preset is a name paired with an `Overrides`. Selecting one writes its rows into the
same working copy a manual capture writes into, indistinguishably. There is no persisted "which
preset is active", in the crate or in what a screen keeps between visits.

**Rules out.** A preset as a layer that reapplies later and reconciles against what the player has
since changed.

**Reversal.** A layer needs machinery this crate does not have, and it contradicts D47: applying
always starts from the pristine declaration and never stacks. Keeping presets a starting point also
keeps the persisted format exactly the `Overrides` shape it already has — which preset is selected
is something a screen can *compute*, by comparing what is bound against each registered preset.

**Why applying one needed a second entry point.** The refusal that guards a capture — a `Fixed` row
is a design decision the player's own screen must not override — is wrong for a preset, whose whole
reason to exist is moving rows a capture screen never offers a button for. Every gamepad binding in
a typical game is such a row. `apply_overrides_with_preset` exempts exactly the rows that preset
names, and no others; a third `Rebinding` state would have forced every already-correct `Fixed`
declaration in every game to be revisited for a fact that has not changed.

---

## Prompts

### D34 — The reverse lookup is a trait, and the answer is not a `Control`

**Decided.** `Prompts::prompts(action, scope)` is a trait, and it returns `ControlOrigin` — either
one of ours or a name-and-label pair from somewhere else. Both answer `name()` and
`fallback_label()`.

**Rules out.** A free function over our own tables, and `Vec<Control>`.

**Reversal.** Our binding tables are not always the authority, and an authority backend's origins
are *its own* enumeration of physical controls, covering device families we have no variant for and
never will. A `Vec<Control>` would have made the trait ours-only while looking substitutable, which
is the expensive kind of wrong. Because both variants answer the same two strings, a caption renders
one without first asking where it came from.

### D35 — A prompt is not a row of the settings screen

**Decided.** Two lists, two type-erased doors. `mappings` is what the game *declared* and is static:
a screen must draw a row whether or not anything is carrying its context. A prompt is what would
fire *now* — empty for a context nobody is carrying or that is switched off, and inclusive of a
`private` binding.

**Rules out.** Deriving prompts by filtering the mapping list.

**Reversal.** The lookup reads the compiled plan for exactly this reason. `private` is a statement
about the list, not about whether the control works, so a filtered mapping list would drop a binding
that really does fire. The same distinction returns in the span components: picking the *n*th answer
indexes what would fire now, after consumption and after a composite has expanded, which is
emphatically not the settings screen's primary and secondary column.

### D36 — The device is a scope the caller supplies; ranking devices is refused

**Decided.** Contexts come back in the order they get to claim a control — render tick before fixed
tick, then priority, then declaration order — and within a context, in declaration order. Nothing
ranks one device above another. A caller that knows which device it means passes a `PromptScope`; a
caller that does not gets every device's answer in a stable order.

**Rules out.** Ordering keyboard before gamepad, and tracking the device the player used last.

**Reversal.** A fixed order would be a guess wearing a ranking's clothes. Tracking last-used was a
requirement and is withdrawn: an app knows why it is showing a prompt — which screen, opened with
what — where the crate would only be inferring from the last thing pressed.

**Related.** `PromptDevice` says which device a bare prompt speaks for, and the
crate never defaults it. A guess there is wrong *silently*, with every prompt in the game naming the
wrong control and nothing reporting it. Absence means the game has not said; holding `None` means it
deliberately has no primary device.

### D37 — A prompt reads consumption from the declarations, not the frame

**Decided.** The answer reflects the standing fact that a control bound with `consume` in a stronger
active context does not reach a weaker one. It is computed from the plans and the activity, and it
moves only when a context activates or deactivates.

**Rules out.** Reading the transient consumed set.

**Reversal.** A claim lands only while the claiming action fires, so a caption built from the
transient set would flicker as the player pressed things.

### D38 — Staleness is a counter, and the crate says what it cannot see

**Decided.** `PromptGeneration` is bumped by everything the crate can see change the answer: a
context activating or deactivating, and an instance arriving or going away. It is written as an
*insert* rather than a mutable deref, so it fires hooks and can be read either by a run condition or
by an observer.

**Rules out.** Component change detection.

**Reversal.** Evaluation writes to a context's state every frame, so `Changed` on it is true
constantly and detects nothing. The insert-not-deref detail is what keeps both reading styles
available: a run condition coalesces a frame's bumps into one pass at a point in the schedule the
reader chooses, which is what a text layer wants, since a caption should be rewritten before layout
measures it.

**Stated rather than papered over.** `activate`, `deactivate` and `PromptDevice` are public, so a
game changing any of them by hand raises the signal itself, as does a backend whose bindings are
edited elsewhere. The crate cannot see those.

### D39 — The control name table is ours, and one name is both identity and key

**Decided.** One string per control serves as the stored identity and the localization key.
`key/KeyW` is what a settings file holds and what an app's catalogue answers to. The table is
written out rather than derived from Bevy's names.

**Rules out.** `Debug`, serde on `KeyCode`, and deriving display text from upstream identifiers.

**Reversal.** Those names belong to Bevy, and a rename upstream would silently orphan every saved
binding. Owning the table costs about two hundred lines and turns an upstream rename into a compile
error in an exhaustive match while the stored string stays what it was.

**It also lets the labels say what the controls are.** `LeftTrigger` is a bumper and `LeftTrigger2`
is the trigger, which is worth correcting in the one place a player reads. The mouse thumb buttons
are the same call from the other direction: stored as `mouse/Back` and `mouse/Forward` because that
is what the backend reports and the stored string must not drift, shown as **Mouse 4** and **Mouse
5** because that is what every other settings screen calls them.

**What the fallback cannot do.** It answers for a US keyboard, so a binding to a physical key shows
an AZERTY player the wrong letter. Nothing in Bevy reports what a physical key produces on the
current layout outside an event that has already happened, so the crate cannot fix this alone; an
app supplies the control half of its catalogue per layout.

---

## Capture

### D40 — Capture reads the frame directly, not through a binding

**Decided.** Every other path through the crate turns a control into a value and discards the
control. Rebinding wants exactly the discarded half, so a capture session reads the frame in its own
system set, between sampling and evaluation.

**Rules out.** Capturing through a binding or an evaluated action.

**Reversal.** A main-menu settings screen has no gameplay contexts spawned and no evaluator
stepping, and capture does not notice — which makes "a settings screen works before a game starts"
structural rather than something to arrange. Running before evaluation is what lets a capture take a
control before any context acts on it.

**Arming costs a frame, deliberately.** The press that opened the session is still in the queue when
it arrives, so a session that read immediately would bind whichever key the player activated the row
with. A session therefore skips whatever is already queued on its first run.

### D41 — A capture session is a component on whatever entity the caller picks

**Decided.** `CaptureSession` goes on the entity the caller chooses — usually the cell button the
player activated. The crate answers with an event on that same entity and removes the component;
removing it yourself cancels.

**Rules out.** A global session resource.

**Reversal.** "Which cell is listening" is answered by where the component is, rather than by the
screen keeping that state beside a global session and keeping the two in step. Because the crate
never touches the player or context entities, a screen reached from the main menu works the same as
one reached from a pause menu.

### D42 — Reserved before shape, and excluded is a silent guard

**Decided.** Three refusals that look alike and are not. *Reserved* is declared on a binding and is
loud. *Shape* and *scheme* are the mapping's own constraints. *Excluded* is the screen's own
controls and is silent. Reserved is asked first, and one shared predicate answers for both a live
capture and a control loaded from a file.

**Rules out.** Asking in implementation order, and treating exclusion as a refusal.

**Reversal.** Pressing the settings key should hear that it is spoken for, not that its channel is
wrong. An excluded control is not being refused — it is busy doing its normal job, which is how the
key that cancels a capture reaches the thing that cancels it. One predicate is what stops a control
getting two different reasons depending on which direction it arrived from; before the two were
merged they genuinely disagreed, and no test noticed.

**Reserving has two halves and the second is the one that matters.** A reserved binding takes no
mapping *and* its controls are refused by capture across the scheme. Without the second half a
player cannot rebind the settings key away but can still bind something else over it, which is the
same trap through another door.

**Only deliberate arrivals are refused out loud.** A stick drifts and a mouse twitches. A press is
refused loudly, a continuous reading past its threshold is dropped quietly, and both are claimed so
that neither also plays the game.

### D43 — Conflicts are detected, never resolved

**Decided.** `conflicts` and `conflicts_pending` are pure queries over the mapping list, answerable
before anything is committed. What to *do* about a clash — reject, swap, unbind the other, allow the
duplicate — is the app's.

**Rules out.** A crate-owned `ConflictPolicy`, and an `Overrides::rebind` that resolves conflicts
and writes several rows on the app's behalf. Both were built and rejected on review.

**Reversal.** `Overrides::bind`, `set` and `get` already say everything a policy needs to say:
reject is not writing, allow-the-duplicate is writing anyway, and swap and unbind-the-other are the
app reading the conflicting row's current list and writing it back with one control removed or
traded. The four policies are worked examples in a doc comment instead of an enum.

**Three limits, stated rather than hidden.** Comparison is at control granularity, so two bindings
differing only in their chords are reported as overlapping — a false positive rather than a false
negative. A clash across two contexts is *possible* rather than certain, because whether two
contexts are ever live together is a question about the game's activation rules. And the whole
target mapping is excluded rather than the one slot, so a control repeated across two slots of one
row is invisible here; a caller about to write a row already holds that list and needs no help
spotting a duplicate in it.

---

## Navigation

### D44 — Two general combinators, not a navigation path

**Decided.** The crate adds `compass`, which rounds a 2D value to four or eight points and discards
the magnitude, and `on_change`, which fires on the ticks the value differs from the tick before.
Neither is about navigation. Together they fire once per compass point *entered*; with `pulse` after
them, that is auto-repeat.

**Rules out.** A virtual cursor, and a bespoke navigation input path beside the mapper.

**Reversal.** A stick held off centre is off centre every tick, so a naive binding runs a menu off
the end of the list before the player has let go. Of the two usual fixes, the cursor is slow to use
and the separate path puts a game's most-pressed controls somewhere the rebinding screen cannot see
them. Three combinators that all exist for other reasons cover it instead.

**Where the crate stops is the value.** It rounds the direction and says when it changed. It does
not call `bevy_input_focus` and does not know the focus exists — the observer turning a direction
into a focus move is four lines and lives in the app, because the association between a widget
library and an input mapper may only be expressed by whoever depends on both.

**Two consequences that were not obvious.** The previous value has to be the whole value rather than
a boolean, or two directions cannot be compared. And the claim on a control is held while the
binding is `Ongoing` as well as `Fired`, because a binding that fires once per direction entered
says nothing in between — a claim lasting only as long as the fire would hand the stick back to the
game underneath for exactly the ticks the player was still holding it.

---

## Overrides and persistence

### D45 — An override is a diff keyed by mapping and scheme, holding controls only

**Decided.** Rows are keyed by `(scheme, mapping)` and hold controls. Not by action, not by binding.
Nothing in an override names a device.

**Rules out.** One row per action, and any device identity in the file.

**Reversal.** An action has several bindings, so `Jump` is Space *and* South; the unit of rebinding
is the mapping, since the player rebinds "move forward" and never `Move`; and only the source
belongs to the player, because modifiers, conditions and chord structure are developer data (D27)
and the knobs a player does get are tunables (D32). Per-scheme separation is what keeps a keyboard
remap from disturbing the gamepad layout.

**No device identity.** A row names a control on a device *class*. Which physical unit drives which
player is pairing state and which stick rests where is calibration state, both keyed by persistent
device identity rather than by profile. That separation is what lets two players with identical
controllers and identical mappings share one override table and differ only in pairing.

### D46 — Three row states, not two

**Decided.** A row is `Controls`, `Cleared`, or `NotOurs`. The loader knows all three and the
writer never invents a row for the third.

**Rules out.** Absence as the only way to say nothing.

**Reversal.** Absence already means "use the default", so a player who deliberately empties a row
has nothing left to say with unless clearing has its own value. And an action an external backend
owns must read as neither — writing a control there or treating it as emptied are both wrong.

### D47 — Applying is the only path in, and overrides do not compose

**Decided.** An override set is applied to a live context, and startup is simply the first call.
Each apply starts from the pristine declaration, so the argument must be the *whole* working copy —
a preset's rows and any manual captures together.

**Rules out.** A separate startup path, and applying a partial set.

**Reversal.** An authority backend can rewrite its bindings mid-session, so mid-session application
is the normal case on at least one platform; building a startup path with a reload path bolted on
afterwards would get it wrong twice. And because applies do not stack, a smaller second call
silently reverts every row it does not mention — which is why the parameter is documented as the
whole copy rather than a delta.

**What applying does.** Swapping cancels in-flight actions and re-arms require-reset, which is what
deactivation and activation already do, and moves every follower riding a row that changed.

### D48 — Applying rewrites the authored bindings; a variant keeps the declared slots

**Decided.** The authored binding specs are retained beside the plan and cloned per apply, and the
variant plan keeps the declared plan's slot allocation.

**Rules out.** Patching the compiled bindings, and deriving a fresh slot allocation.

**Reversal.** Rewriting authored bindings is what makes loading the pure function D50 requires, and
it is why the custom modifier and condition variants hold an `Arc` rather than a `Box`. Keeping the
slot allocation is not an optimization: an action whose every binding the player cleared would
otherwise lose its slot and read as *unbound*, firing the "not bound in this context" diagnostic —
which exists to catch a typo and is precisely wrong for a control somebody deliberately emptied.
Keeping the table also means an instance's action states and require-reset flags stay aligned across
the swap, so only the scratch is rebuilt.

**Three slot cases, and the third bites.** A slot the defaults fill has its source rewritten. A slot
they left empty is filled by *copying* the binding beside it, so a secondary carries the same
modifiers and conditions as the primary rather than arriving bare. A slot the override no longer has
takes its binding away. Copying only works where a binding reads one control — copy a composite and
its other three directions land in their own rows a second time — so a row that is one part of a
composite is refused a slot the defaults did not ship.

### D49 — The control encoding is a format we own

**Decided.** `key/Space`, `pad/South`, `key/ControlLeft+key/KeyS`. Written by hand rather than
derived, with one table per scheme, a scalar accepted and written where a row holds one control, and
the three row states spelled as words no control name could collide with.

**Rules out.** Deriving the wire format from Bevy's type names, and a shape that is unpleasant to
edit by hand.

**Reversal.** Same reason as D39: an upstream rename must not orphan a save file. The encoding also
has to carry the physical-versus-logical distinction and the device class, and it is round-trip
tested against a golden document. Accepting both a scalar and a list on the way in is because most
rows hold one control and a player editing a file by hand should not have to type brackets to say
so.

### D50 — Loading is pure, and reports rather than drops

**Decided.** Loading maps declarations and a document onto bindings and problems. A saved
mapping name is resolved against what the game currently declares; an unresolved name or an
unrecognized control is reported, never dropped in silence.

**Rules out.** Loading that mutates, and loading that silently discards what it cannot place.

**Reversal.** A `MappingKey` can only ever be one already declared, so resolution is the only way in
— and a player whose saved binding quietly vanished has no way to find out why. The problems come
back in the same diagnostic shape the plan-build tier produces.

**Accepted.** A renamed action's row is dropped on the next save rather than preserved unresolved.

---

## Backends and devices

### D51 — An authority backend writes a value, not a state

**Decided.** Extends D22 with how the second seam actually works. An authority backend supplies the
value the fold would otherwise have produced, entering at the button state machine rather than after
it, and the existing transition code diffs it and synthesizes the edges. Bindings, modifiers and
conditions are skipped; the dead-zone stages are not reapplied, because there is no binding to apply
them from.

**Rules out.** A second write path into action state.

**Reversal.** Steam returns a level, sampled when asked, with no edge and no timestamp, so this
crate's timing is unsatisfiable from it — but `fired()` and `Phase` have to keep working or the
promise that a consumer need not know which backend produced a value is false. A second write path
would have to reimplement the state machine, and two implementations of the lifecycle is exactly the
drift that promise forbids.

**A condition on a backend-owned action is a plan-build error.** The backend has its own activators
and will not deliver a hold or a multi-tap, so the game asked for behaviour it will not get and
nothing else would tell it.

**A context is a layer.** Steam allows one action set active per controller plus a stack of layers,
where this crate runs any number of contexts at once. Layers stack and override in the direction
priorities already do, so a backend activates one base set and pushes a layer per active context —
and an action bound in several contexts is one action declared once in the base set. Consumption is
the part layers cannot express: a lower layer's action is shadowed or it is not, and there is no
equivalent of one context claiming a control for a frame.

### D52 — Pairing is a runtime handle; the join gesture reuses class bindings

**Decided.** `DeviceHandle` models keyboard and mouse as one value, a gamepad as the backend's own
entity — nothing a save file should ever compare across a restart. Filtering happens once, at the
earliest point a raw event reaches a context, before anything else sees it.

**Rules out.** Treating the runtime handle as persistent identity, and layering pairing onto the
consumption or exclusion machinery.

**Reversal.** A backend reassigns gamepad entities on reconnect. Filtering at the frame is what
keeps consumption and the exclusion ceiling computed once per context *type* and untouched by
pairing, and a context with no pairing reads every device, so nothing that predates the component
changed behaviour.

**The join gesture needed no new evaluation path.** The design this replaces proposed evaluating a
designated context against every unassigned device — a second per-device evaluation cycle running
parallel to the main one. What ships instead is an ordinary action bound with `bind_class` on a
context with no pairing of its own, so it reads every device exactly as any other unpaired context
does. The one piece of new code checks that a device no pairing already names is the one that
counts, which is what stops two waiting slots racing for it.

**Still open.** Owner-scoping consumption and the exclusion ceiling. Nothing in tree needs
it: no game pairs two different-priority contexts to different devices where one's consumption would
wrongly reach the other.

---

## What the crate refuses to own

### D53 — The crate detects and reports; the app decides

**Decided.** Where a question has a defensible answer the crate could compute and an app might
reasonably want differently, the crate answers the factual half and stops. It reports conflicts and
does not resolve them; it keeps the prompt lookup and does not draw; it holds no registry of presets
and no record of which one is active; it ranks contexts and refuses to rank devices; it rounds a
direction and does not move a focus; it serializes an override set and does not decide where the
bytes go.

**Rules out.** A policy API for each of those.

**Reversal.** Each was considered and most were built at least once. A `ConflictPolicy` enum and a
resolving `rebind` were written and rejected on review as the crate accreting a decision that is the
app's to make — not a hypothetical concern, but feedback already heard from collaborators about this
crate taking on more than it needs to. The general shape of the error is that the crate's answer
would be *plausible*, so an app that wanted something else would have to work around it rather than
simply not use it.

**The test that separates the two halves.** A fact the crate is uniquely placed to know — which
mappings hold a control, which control would fire an action now, whether a row is rebindable — is
the crate's. A decision that depends on what the game is — what to do about a clash, which device a
prompt speaks for, how two controls read on one row — is the app's, and the crate's job is to make
it cheap to answer rather than to answer it.

---

## Late entries

Found by reading the archived work log rather than the design document, and appended rather than
renumbered.

### D54 — There is no pass-through action

**Decided.** An action holds one value. There is no second kind that reports every contributing
control separately.

**Rules out.** Unity's model, where an action's bound controls are normally disambiguated to the one
with the greatest magnitude and `PassThrough` is the opt-out.

**Reversal.** It is a second storage shape — N live values per action rather than one — carried on
every action so that a few could use it, which is a change to D8's layout and to D15's fold.

**Why the motivating cases did not need it.** All three turned out to be device-shaped rather than
value-shaped: telling which of four pads pressed Start is device scoping, seeing every contributor
in a debug overlay is the type-erased inspection dump reading the plan, and a value that remembers
where it came from is its own smaller question. Each is answered by a mechanism that has to exist
anyway. If a case appears that genuinely needs the distinction, it should arrive with that case
attached rather than be reinstated on the strength of the original three.

### D55 — State-driven activation runs inside `StateTransition`

**Decided.** A context whose activation follows a game state is synchronised inside Bevy's
`StateTransition`, not in `PreUpdate` with the general run-condition path. The state resource is
read as an `Option`, because a substate or a computed state may have none.

**Rules out.** One placement for both activation paths.

**Reversal.** Bevy applies transitions *after* `PreUpdate`, so a condition polled there reads the
state before that frame's transition has been applied. The difference is invisible for render
contexts and real for the other two cases:

| | in `StateTransition` | in `PreUpdate` |
| --- | --- | --- |
| a render context's next evaluation | frame N+1 | frame N+1 — no difference |
| a fixed context's next evaluation | frame N | frame N+1 |
| what an `OnEnter` system sees | already in step | still the old answer |

Reading the state resource unconditionally would panic the first time anyone declared a context in a
nested state — and a pause menu as a substate of playing is the obvious way to write the example
this crate ships.

**One mechanism, two installers.** A general run condition has no transition to sit behind, so it is
polled in `PreUpdate` before evaluation. A state keeps the placement its simulation half needs. The
difference is a table rather than a caveat.

### D56 — Activation answers per context type, and is declared on the builder

**Decided.** A run condition decides whether a context is live, answering once for the whole context
type. It is declared on the builder beside the bindings it governs, not by a variant of the call
that declares the context. Per-instance activation stays a method on the instance.

**Rules out.** A method per activation policy on the app extension trait, and binding activation to
the entity so that two instances of one context can follow different conditions.

**Reversal.** The extension trait would have grown a method per policy, and focus-driven activation
is already a fourth; on the builder each policy is one method on the type that is already where a
context says what it is. The per-entity decomposition — which `bevy_enhanced_input` chose, letting
two instances follow different states and one context be live in several — is more capable at the
cost of two places to get right, where this one cannot be half-declared.

**Accepted cost.** Mixing a condition with per-instance activation means the condition wins every
frame. That is documented rather than prevented, since preventing it would mean tracking which door
an activation came through.

### D57 — Where two pads report one axis, the one that moved last speaks

**Decided.** A context instance keys its held gamepad state by control rather than by device. Where
two pads drive one context and both report the same axis, the most recent reading is the one that
stands.

**Rules out.** A per-device map of held state in every context instance.

**Reversal.** Per-device state was once scheduled, on the grounds that per-unit calibration needed
it. It did not — calibration is applied where the raw message still names its own sender, before
held state exists — so what was left was the merge alone, and a per-device map in every instance
costs more than the symptom is worth.

**The whole observable consequence.** On an *unpaired* context driven by two pads, a still-held
stick on the second pad reads zero until it next moves, and a disconnect clears every pad's readings
rather than only that one's. A paired instance never sees this, because it reads one device by
construction. `leafwing-input-manager` takes the same position, and its maintainer reports never
having had a complaint.
