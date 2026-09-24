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
| **D1**  | Four layers, and L2 reads only L1                                             | TD1       |
| **D2**  | L1 is an event queue, not a level snapshot                                    | TD2       |
| **D3**  | Each context reads by cursor; retirement is separate and later                | TD2       |
| **D4**  | Timestamps order events; they do not time them                                | TD2       |
| **D5**  | An action is a type                                                           | TD3       |
| **D6**  | Serialized identity is a declared path, not the Rust type path                | TD3.3     |
| **D7**  | ActionIntent is separate from output shape and from channel shape                   | TD3.1     |
| **D8**  | Action state is two dense tables; actions are not entities                    | TD6       |
| **D9**  | A context declares one tick domain and is evaluated once                      | TD1, TD3   |
| **D10** | Bindings compile once into an immutable, shared plan                          | TD4       |
| **D11** | Arbitration splits: priority is system ordering, chord length is a pre-pass   | TD5.1     |
| **D12** | Consumption is recorded per schedule and flows forward                        | TD5.2     |
| **D13** | Exclusivity is a ceiling, not a third context state                           | TD5.3     |
| **D14** | A class binding is a second list, not an expanded set of controls             | TD5.4     |
| **D15** | ActionIntent decides how several bindings fold into one action                      | TD5.5     |
| **D16** | Nothing user-defined runs inside the evaluator                                | TD5.6     |
| **D17** | Transitions are generic entity events on the context entity                   | TD5.6     |
| **D18** | Require-reset holds back buttons only                                         | TD7.2     |
| **D26** | Failures surface at the earliest tier that can catch them                     | TD4, TD7.3 |
| **D19** | Modifiers and conditions are enums with a `Custom` variant                    | TD8.2     |
| **D65** | The device model is closed; a third-party kind needs one in hand             | —               |
| **D20** | We own the whole dead-zone chain, in three stages, with one rescaling         | TD8.4     |
| **D21** | Calibration is measured by an explicit step, never detected                   | TD8.4     |
| **D22** | Backends enter at two seams, not one                                          | —               |
| **D23** | Focus integrates by activation, and interception is static                    | —               |
| **D24** | One crate, feature-gated by source                                            | TD11      |
| **D25** | What must not move upstream                                                   | —               |
| **D91** | Bindings and presentation are one database, keyed by one declared id          | TD3.3, TD9 |
| **D27** | The presentation model is separate from the binding model                     | TD9       |
| **D28** | Listing is the default; rebinding is opt-in                                   | TD9.1     |
| **D29** | A mapping is an ordered list of slots                                         | TD9.1     |
| **D30** | `follow` declares a shared control once, against the leader's bindings so far | TD8.2     |
| **D31** | Every player-facing string is a key; the mapping owns the name                | TD9.1     |
| **D32** | A tunable is typed, so a settings screen is generic                           | TD9.1     |
| **D33** | A preset is a starting point, not a layer                                     | TD10.2    |
| **D34** | The reverse lookup is a trait, and the answer is not a `Control`              | TD9.2     |
| **D35** | A prompt is not a row of the settings screen                                  | TD9.2     |
| **D36** | The device is a scope the caller supplies; ranking devices is refused         | TD9.2     |
| **D37** | A prompt reads consumption from the declarations, not the frame               | TD9.2     |
| **D38** | Staleness is a counter, and the crate says what it cannot see                 | TD9.2     |
| **D39** | The control name table is ours, and one name is both identity and key         | TD9.2     |
| **D40** | Capture reads the frame directly, not through a binding                       | TD9.3     |
| **D41** | A capture session is a component on whatever entity the caller picks          | TD9.3     |
| **D42** | Reserved before shape, and excluded is a silent guard                         | TD9.3     |
| **D43** | Conflicts are detected, never resolved                                        | TD9.3     |
| **D44** | Two general combinators, not a navigation path                                | TD8.3     |
| **D45** | An override is a diff keyed by mapping and family, holding slots              | TD10      |
| **D46** | Three row states, not two                                                     | TD10      |
| **D47** | Applying is the only path in, and overrides do not compose                    | TD10.1    |
| **D48** | Applying rewrites the authored bindings; a variant keeps the declared slots   | TD10.1    |
| **D49** | The control encoding is a format we own                                       | TD10.3    |
| **D50** | Loading is pure, and reports rather than drops                                | TD10.3    |
| **D58** | An unrecognized version refuses the set; no migration exists yet              | TD10.3    |
| **D59** | Persistence goes through a separate, reflectable type                         | TD10.3    |
| **D51** | An authority backend writes a value, not a state                              | —               |
| **D92** | An authority is a binding source for one device family                        | TD5.8     |
| **D52** | Pairing is a runtime handle, filtered at the frame                            | TD7.4     |
| **D53** | The crate detects and reports; the app decides                                | —               |
| **D54** | There is no pass-through action                                               | TD5.5     |
| **D55** | State-driven activation runs inside `StateTransition`                         | TD7.2     |
| **D56** | Activation answers per context type, and is declared on the builder           | TD7.2     |
| **D57** | Where two pads report one axis, the one that moved last speaks                | TD7.4     |
| **D60** | The character-producing door is a method, not a fourth `ControlClass`         | TD5.4     |
| **D61** | A gamepad stick is a `Control`, named whole                                   | TD8.1     |
| **D66** | Control classes are a closed set                                             | TD5.4     |

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
spanning them sees neither. `InputFrame`, `RawEvent`, `FrameTimestamp` and the whole capture path
go with it, and the frame stops being the serializable per-tick record that replay and rollback
need.

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

**Decided.** A `FrameTimestamp` is a frame counter and an order within that frame, stamped as the
event is sampled.

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
naming convention in TD3.3 is what stands in for one.

### D7 — ActionIntent is separate from output shape and from channel shape

**Decided.** Three properties, not one. The output is the Rust type; the `ActionIntent` is what the
value means; the `ChannelShape` is what the control reports. They do not have to agree, and on real
hardware they frequently do not.

**Rules out.** Inferring meaning from the Rust type. A stick deflection and a mouse delta are both
`Vec2`, and one is a position implying a rate while the other is a displacement that already
happened.

**Reversal.** The fold in D15 has nothing to key on, and binding admissibility has nothing to check,
so summing a stick position into a mouse delta becomes expressible. `ActionIntent` is also what a
rebinding UI filters candidate controls on, so the capture path loses its constraint.

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

**Still open.** Enforcement is not airtight. `ContextActions<OnFoot>` where `OnFoot` is `Fixed`
should be unreadable from `Update`, but Bevy gives a `SystemParam` no way to know its own schedule.
What stands in is a plugin-time validation pass and a debug assertion.

### D62 — `ActionPhase::Ongoing` splits into `Building` and `Firing`, named by part of speech

**Decided.** `Ongoing` becomes two variants: `Building`, a condition still short of firing, and
`Firing`, an action still active. The split follows a rule now stated in R3.1: a gerund or adjective
names a level, true again next tick; a past participle names an edge, true for one tick only.

**Rules out.** Reading a phase and then re-reading the action's *value* to tell the two apart, which
`update_action_state`, `why_not_id`, and Disasteroids' exhaust flame each did — the value test is
gone from all three, not just moved.

**Reversal.** Re-merging the two brings the value test back to every call site it was removed from,
in this crate and in any app that has since matched on `Building` or `Firing` separately —
`ActionPhase` is not `#[non_exhaustive]`, so those matches would stop compiling rather than
silently misbehave.

### D63 — `cancel_in_flight` also cancels `Started`

**Decided.** Deactivating or shadowing a context cancels `Started` alongside `Fired`/`Firing`/
`Building`.

**Rules out.** The narrower match kept until now, `Fired | Ongoing`, which left a hold canceled on
the very tick it began sitting at `Started` rather than `Canceled`.

**Reversal.** `Started` stops being canceled again, and a hold interrupted on its first tick reads
as still building for as long as the context stays inactive — R7.4's "never left stuck as held
forever" holds everywhere except this one-tick window, which is exactly narrow enough to have gone
unnoticed until read rather than run.

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

**Whose ceiling it is.** An entry records the devices of the instance that raised it and shadows
only a context sharing one, so one player's modal leaves another player's gameplay alone — D88,
which scopes consumption the same way and for the same reason.

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

### D15 — ActionIntent decides how several bindings fold into one action

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

**Superseded in part by D76**: `Analog1` and `Directional2` add the strongest positive and strongest
negative contribution on each axis, and declaration order is no longer a tiebreak.

### D76 — A composite is a way to write bindings, and opposite contributions cancel

**Decided.** `DirectionalButtons` and `AxisButtons` are authoring shorthand, not a kind of binding.
Each expands at declaration into one binding per part, and the part is both the binding's name and
the direction it contributes. `Analog1` and `Directional2` fold per axis, as the strongest positive
contribution plus the strongest negative one, which reverses D15 for those two intents; `Button`
keeps strongest-wins and `Delta2` still sums. A stage after the fold, declared once per action, is
where anything that shapes the combined value goes. Chunks 125–127 built it: 125 the stage, 126 the
fold, and 127 the expansion.

**Rules out.** A composite the evaluator or the override path can see; a direction held as `negate`
and `swizzle` modifiers, which is BEI's form; and a clamp built into the fold.

**Reversal.** The player is already shown one row per part (R19.9, D27), and a composite underneath
four independent rows is a model the player sees and cannot use: a row that is one part can neither
take another control nor be emptied, because the binding behind it is the other directions too.
Reinstating the composite reinstates `CompositeCannotGrow` and `CompositeCannotEmpty`, and the
Disasteroids screen that shows a cleared cell which Confirm then refuses. The fold is the other half
and is not separable: under strongest-wins, W and D held together read as whichever was declared
first, so split parts lose the diagonal.

**The player's side of it.** Shipped games show four movement rows that behave independently, and a
player has no reason to think Forward and Back are joined because they share a column. BEI's
`Cardinal` spawns four bindings and sums by default (`Accumulation::Cumulative`), as Unreal's
Enhanced Input does. Unity keeps a composite with addressable parts, and carries the same inability
to grow one part. The fold has no precedent found: Unity's per-composite choice of which side wins
is its nearest relative.

**Why the fold splits by sign.** It is the rule that answers every case the other two each got
wrong. Opposite contributions cancel, which strongest-wins does not: A and D on separate bindings
read as whichever was declared first. Same-direction contributions do not add, which a sum does: A
and Left together reach -2, and two half-deflected sticks read as one full deflection, which is
D15's own objection. Within one sign the strongest wins as before, so a player holding one key per
direction sees what the composite gave, and a maximum does not depend on declaration order.

**Why the direction is not a modifier.** The part names the row. A direction held as modifiers would
have to be read back into a name, which a custom modifier defeats, and it costs two modifier calls
per key per tick where one match arm does.

**Why the fold does not clamp.** It has no need to on an axis, since no sign can exceed its
strongest contributor. And modifiers run per binding before the fold, so a `scale` on an analog
binding is legitimate and a fold clamping to unit length would cap it silently. The length of a
diagonal is a different matter and was never automatic: W and D give (1, 1), as a four-key composite
does today, and `clamp_magnitude` in the stage after the fold is what normalizes it.

**Accepted price.** Taking each axis's maximum separately can bend the direction when two analog
vectors combine, which nothing in tree does; a button-sourced part feeds one axis and cannot. The
fold keeps two accumulators rather than one, which are locals of the loop and never state. A
composite costs four passes of per-binding bookkeeping rather than one; the control lookups, which
dominate, are unchanged.

**The stage takes conditions as well as modifiers.** Settled by driving the menu's bindings
headless: arrow keys under `on_change().pulse(0.25)`, as one composite and as four single-key
bindings. Up held then Right pressed fired the diagonal and repeated it on one timer as a composite;
split, it fired Right alone, then alternated Up and Right on two timers, and releasing Right moved
nothing. A condition per part judges a key, and a menu is asking about a direction.

### D78 — A modifier is a chord entry the runtime reads, not shorthand that expands

**Decided.** `ModifierKey` names one of the four keyboard modifier pairs and reaches the compiled
plan intact, as a `ChordEntry::Modifier` satisfied at read time by either key of the pair. Chord
length counts entries, so a modifier contributes one however many keys satisfy it.
`ControlOrigin::Modifier` carries it to presentation and is the only variant answering `None` to
`control()`. Side-agnostic is the short spelling and a `KeyCode` is how one physical key is named,
because either-side is the case a chord almost always wants. Chunk 94b built it.

**Rules out.** D76's treatment, applied to modifiers: expansion at declaration would double the
*binding* rather than the entry, since both keys of a pair are live at once. A `Side` enum whose
`Either` variant sits beside `Left` and `Right`, which would make the ordinary case the elaborate
one. Naming a representative key in a caption.

**Reversal.** Expansion is cheaper to evaluate — one lookup per entry rather than two — and is what
a plan with no presentation surface could afford. What it costs is that `Ctrl+S` becomes two
bindings on one row, so a rebinding screen lists the shortcut twice and a caption names a side the
player is free to ignore. This is the same question D76 answered the other way, and the answers
differ because a composite's parts are alternatives the player rebinds separately while a modifier's
two keys are one thing they press either of.

### D79 — A rebinding row's chord rides inside the slot

**Decided.** `ActionMapping::slots` holds `Option<BoundSlot>`, and a `BoundSlot` is a control plus
the `ControlOrigin` list held with it. `ControlOrigin` rather than a control list for D78's reason —
a modifier stands for either key of its pair — which also makes a row and a caption composable from
the same values. Chunk 128 built it, with `Overrides` still in controls; chunk 129 made the same
type what an override writes (D81), and `ActionMapping::controls()` is left as a bare view for
reading.

**Rules out.** A parallel `chords` list beside `slots`, index-aligned. Chords are per slot rather
than per row, so the two would have to stay aligned by convention, and "this slot is empty" would
have a place to be said in each of them and a way to disagree — the same shape the saved format
rejects for the same reason.

**Reversal.** The parallel list leaves every `.slots` call site alone, which is most of what the
change cost. What it buys back is the alignment invariant, and the point at which that bites is an
override: a row with a gap in it needs its chord list gapped identically, and a list that drifts by
one puts a chord on the wrong slot. With the chord inside the slot, an override has one list to
write.

### D80 — A chord is set, never captured

**Decided.** Capture listens for one control. It never accumulates whatever modifiers were down
alongside it, and a screen that wants a player-editable `Ctrl+S` sets the modifiers explicitly
rather than pressing them. So `admissible` has no chord case, `ControlCaptured` carries one control,
and a chord reaches a binding from the declaration or from an override.

**Rules out.** A capture that reports what was held with the press, and with it the conflict
question that follows — whether `Ctrl+S` captured over `S` is a rebind, a clash, or a new row.

**Reversal.** Hearing the chord is what a player would guess the screen does, and it is how every
text field's shortcut editor behaves. What stops it is that the interesting chords never arrive:
`Cmd+Q` quits the application before the key reaches it, and a window manager takes others first, so
a capture that listened would work for the combinations nobody needs and fail silently for the ones
they do. Blender's keymap editor captures a bare key and offers the modifiers as toggles for this
reason, and resets those toggles on each capture. Whether a rebind clears an existing chord or
preserves it is the screen's to decide, by what it writes (D81).

### D81 — An override slot states its chord

**Decided.** `Override::Slots` holds `BoundSlot`, the type a row reads, and a slot is the whole of
what is bound at its position. `rewrite` writes its chord onto the binding along with its control: a
bare control binds with no chord, a grown slot takes its own chord rather than the primary's, and a
follower takes its leader's. The saved string carries the chord ahead of the control under catalogue
names, `mod/ctrl+key/KeyS`. Conflict detection compares slots — the same control with the same
chord, order ignored. Chunk 129 built it.

**Rules out.** An inherited chord — `with: Option<..>`, where absence keeps the declared one — and
with it the crate choosing between Blender's reset and preserve on a game's behalf. A write type
holding `ChordEntry` beside the read type, which would make an unholdable entry unrepresentable at
the price of a fallible conversion at every edit and a `cfg` split for the no-devices build. The
bare modifier words of the first sketch, `ctrl+key/KeyS`, which cannot spell a chord of two buttons
without a second vocabulary.

**Reversal.** Explicit is what makes a saved row mean one thing. Under inheritance, `"key/KeyD"` on
a row declared `Ctrl+S` means `Ctrl+D` today and whatever the next patch declares tomorrow, and
"deliberately no chord" needs a spelling of its own. The cost is the same fact seen from the other
side: a preset or a save naming a bare control on a chorded row drops the chord, and a patch that
revises a declared chord does not reach a row the player rebound — as a revised control already does
not. Every saved row is also read differently, so reversing this after a release is a format
version.

**Accepted price.** `ControlOrigin` admits entries nothing can hold, a `Foreign` control or a stick,
so `NotChordable` exists where a narrower type would have made it impossible. The clash rule misses
two real clashes, stated in D43.

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

### D77 — A disabled action is out of evaluation, as an inactive context is

**Decided.** Disabling one action cancels what it had in flight and takes its slot out of the fold:
its bindings read nothing, advance no scratch, claim no control and out-rank no chord, and an
authority's value for it is ignored. Enabling re-arms require-reset on that slot alone, and only
when it was disabled. The switch is per instance. Chunk 35 built it.

**Rules out.** Evaluating a disabled action and discarding the result, which would keep a hold
counting across the gap.

**Reversal.** A disabled action would go on consuming its controls and winning chords, so a context
below it, or a shorter chord beside it, stays deaf to a control nothing visibly uses. Gating those
two separately is a second meaning of "off" beside the one an inactive context already has.

### D26 — Failures surface at the earliest tier that can catch them

**Decided.** Three tiers. The compiler catches a wrong output shape. Plan build catches an unknown
control, a duplicate binding, a shape mismatch no conversion can fix, contradictory consume flags
and the rest, and `add_context` refuses the context rather than installing a plan that cannot work.
A runtime query answers everything situational: `why_not` returns an `ActionObstacle` naming which
of the several possible reasons applies.

**Rules out.** Deferring everything to run time, and failing silently at any tier.

**Reversal.** The runtime tier is the one that pays for itself and the one that is public API.
"Why didn't my action fire" has at least six causes that are indistinguishable from the call site —
inactive context, a higher-priority consumer, a longer chord winning the clash, a condition part way
through, a control nothing has touched, and an action simply not bound here — and the plan already
holds everything needed to tell them apart. Removing `ActionObstacle` leaves a developer with a
printf.

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
controls, what is held with them, and tunable values; modifiers and conditions are developer data
and never reach a save file. So `Modifier` and `Condition` carry no `Reflect` bound and custom
extensions are not serialized, because nothing asks them to be. The `Arc` rather than a `Box` is for
an unrelated reason: applying an override clones the authored bindings and rewrites their inputs,
and the originals have to survive that intact.

### D65 — The device model is closed; a third-party device kind needs one in hand to design against

**Decided.** `DeviceHandle`, `Control` and `RawEvent` stay closed — keyboard, mouse, gamepad — with
no `Custom` variant and no `#[non_exhaustive]`. An unhandled control can still surface as an opaque
id through `ControlOrigin::Foreign` (R11.9); the binding and evaluation pipeline itself does not
open to a device kind this crate did not write.

**Rules out.** A `Custom(Arc<dyn Device>)` extension the way D19 opened `Modifier` and `Condition`.
The difference is that D19's `Custom` had one fixed interface to satisfy — a value and a scratch
slot, evaluated once a tick — while "a device" has no equivalent fixed point across the candidates
R11's own Problem statement names: MIDI, a HOTAS, a racing wheel, gyro, eye tracking. Building the
trait now means guessing which of their shapes it should fit.

**Reversal.** A concrete third-party device, not a hypothetical one. Once one exists to design
against, the questions that decide the interface — polled or event-driven, does it have held
state, does it need calibration, does it hot-plug — stop being guesses, and R11.2 (withdrawn) can
be reproposed against it.

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
_authority_ backend supplies an action's input for one device family, already resolved, in place of
our bindings for that family (D92). Steam Input is the second case: the binding UI, the conflict
rules and the glyphs all live outside the game.

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

**Checked against `steamworks` 0.13, the Rust binding of the Steamworks SDK.** ISteamInput has no
call that suppresses the emulated pad — that emulation is a per-title setting the player controls in
the Steam client, not something a game can query or toggle.

**Device-identity filtering was the plan, and it does not work.** This decision proposed dropping
raw events from a Valve-vendor device while a Steam authority was active for that family, on the
theory that an emulated pad enumerates under Valve's own vendor id and so needs no mechanism beyond
the one D64 built for `GamepadBrand`. It asked for that id to be confirmed against a running client
before a real backend shipped. Measured, it is wrong: the emulated pad carries Microsoft's vendor id
and the Xbox 360 product id, sharing a vendor with the hardware underneath it (`docs/steam.md` S1),
and nothing in the API relates Steam's controller handle to an OS device (S3). There is no way to
identify *which* device is the emulation.

**What replaces it: suppression is per family, declared by the game.** A game that ships a Steam
build knows it does; it tells this crate to stop recording a whole device family at L0, and the
sampler skips that family's events. Wholesale rather than per device because per device is not
implementable, not because it is cheaper — if Valve ever exposes a handle-to-device mapping the
policy narrows and nothing above L0 notices. Declared rather than detected, because a suppression
that silently turns itself on and off between runs is the worst kind of input bug to diagnose.

**Replay is what the switch is for; Steam is the contingent case.** A replay backend mutes live
hardware in a build that compiled the driver in, switching at runtime, and no build configuration
expresses that (R0.6, R10.8). Steam may need nothing here: Bevy's `gamepad` and `bevy_gilrs` are
separate features and `bevy_gilrs` enables `gamepad` rather than the reverse, so a Steam build can
take the types and the connection systems without the driver — and then no hardware event is
produced to suppress. Per-channel builds are the expected shape, a storefront SDK being a coupled
integration that already ships a dylib another channel's build does not.

**Not a Cargo feature on this crate.** A feature that *removes* behaviour breaks Cargo's additive
rule: one crate in the graph enabling it would silence gamepads for every other. A game declining
`bevy/bevy_gilrs` is the opposite shape and is fine. What that costs is feature unification — any
dependency asking for the driver turns it back on for the whole graph — which is build discipline
rather than a rule violation, and `cargo tree` shows it.

**One reason here was assumed, not measured.** This decision argued that a Steam build launched with
the client absent must still read its pads, and therefore that one binary has to serve both. Nothing
established that, unlike S1 and S3 beside it, and `docs/steam.md` now carries the question. Recorded
so it is not re-derived: the switch stands on replay, which does not depend on the answer.

**The gap this leaves** is keyboard and mouse. Steam creates emulated devices for both alongside the
pad (S1), and suppressing the gamepad family does not touch them. Whether a configuration can emit
keys for a pad while the game reads Steam Input natively is unmeasured, and `docs/steam.md` carries
the question.

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

**The `Ctrl+Z` case.** `Ctrl+Z` meaning undo-in-field when a text input has focus and
undo-in-document otherwise looks like it needs a widget to elect at runtime, and it does not: the
text field's focus-activated context claims `Ctrl+Z`, and when focus is elsewhere that context is
inactive and the global binding wins. The election is expressed by which context is active, a
consequence of what has focus, not a runtime decision.

**What static-only interception gives up.** Only a widget that claims a control conditionally on its
own internal state — a text field that swallows `Ctrl+Z` only while its undo stack is non-empty. The
workaround is to make that state part of context activation (a `TextFieldWithUndoHistory` context)
rather than a runtime decision, keeping the claim inspectable by R22.1.

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
someone else's crate, and the test of whether the seam is sufficient without being Steam-shaped is
that the Steam backend in `steam_examples/` builds against the public API alone.

**Also enforced.** Nothing in the crate depends on `bevy_ui`. `bevy_ui` already depends on
`bevy_input` and `bevy_input_focus`, so depending on it would invert that layering and foreclose
`bevy_ui` ever using action maps itself. Everything that draws lives in the examples until it earns
a crate of its own.

---

## The presentation surface

### D91 — Bindings and presentation are one database, keyed by one declared id

**Decided.** An action's bindings, the rows a controls screen shows for it, their localization keys,
the prompts that name its controls and the overrides a player saves against it all hang off one
record, keyed by the action's declared path. The crate owns the player-facing half of input rather
than leaving each game to build it.

**Rules out.** Leaving that database to the app, which is `bevy_enhanced_input`'s position: which
actions are bindable, what they are called, how a prompt finds a control and how a rebind is saved
are each game's own business.

**Reversal.** Everything keyed on the path comes apart: the save key (D6), the mappings (D27), the
localization keys (D31), the reverse lookup a prompt reads (D34) and the override format (D45). A
game would be back to maintaining a parallel table of its actions, and each screen or prompt crate
in the ecosystem would key that table its own way.

**Accepted cost.** A declared path per action, a presentation declaration per rebindable binding,
and a crate larger than a mapper alone. The work it removes is work every game with a controls
screen does anyway, and doing it once makes it standard.

**Provenance.** An assumption rather than a choice: the LLM's first sketch of the API took it for
granted, before the repository's first commit and probably from its survey of prior art, and the
author went along with it. It is recorded as a decision in hindsight, because D6, D27 and D31 all
rest on it and none of them states it.

### D27 — The presentation model is separate from the binding model

**Decided.** Players get a smaller model than developers: named *mappings*, typed *tunables* and
*presets*. Modifiers, composites and conditions stay developer-only and never reach a screen. A
slot's chord — which keys are held with its control — was on the same list until D81 gave it to the
player; it is named controls rather than adapters, so it reads as a player's choice where the rest
would not.

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
translator will look. The family is inferred from the controls, because declaring it would be a
third chance to disagree with what is actually bound.

### D29 — A mapping is an ordered list of slots

**Decided.** A *mapping* is the named thing a player rebinds; a *slot* is one position in it holding
one control, **or nothing**; a screen draws one cell per slot. How many cells a screen draws is the
screen's business, not the mapping's.

**An empty slot is `None` in the list, not a variant of `Control`.** A `Control::Empty` would cost
every consumer of a control an arm meaning "not a control" — the frame, prompts, labels,
admissibility, conflict comparison — and conflict detection acquires a bug the first time two empty
slots compare equal. With the container it cannot: `Some(control)` never matches an empty cell. A
map keyed by index answers a sparse-at-index-9000 question nobody asked and gives up the scalar
shorthand the save format keeps on purpose.

**Rules out.** One control per mapping, and a fixed two. Also a per-mapping capacity: how long a row
*may* grow was once a property of the mapping, and it stopped being one when the width it expressed
turned out to be read by no shipped screen — every caller that wanted a column count already had
that count from the list itself, or from a constant of its own. A global ceiling on a row's length
remains, as a resource the app sets, because a corrupt or hostile save is the one case where a
boundary is the crate's business rather than the screen's.

**Reversal.** One control per mapping cannot express the two-cell row every shipped game's keyboard
table has. The workaround it forced was a second row under an alias name — `thrust` and
`thrust_alt` — telling the player two things are separate when they are the same thing twice. A
fixed two cannot express the "add shortcut" button that tools grow instead.

**A slot is addressed, not appended.** `Overrides::with_cell` takes whatever slot number the screen
names and grows the row to reach it, leaving the slots skipped on the way empty — assignment, the
way writing to index four of a JavaScript array gives you five. The rule it replaced refused
anything more than one past the end, which existed to stop a capture leaving a hole and became
arbitrary once a hole was legal: it allowed the primary of an emptied two-cell row and refused the
secondary, on no principle a screen could explain. What bounds a row now is the number of cells the
screen draws.

**Save format.** A row holds a list because a mapping does, and position is which slot, so a cleared
middle slot needs the cleared marker rather than a shortened list — which would silently promote the
secondary to primary. It is the same word an emptied *row* uses, one level down, so a person opening
the file has one thing to learn rather than two. Trailing empties are not written, because a row is
as long as its last filled slot and a two-column table whose secondaries are mostly blank should not
fill a settings file with the word.

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
names, and no others; a third `RebindPolicy` state would have forced every already-correct `Fixed`
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
a screen must draw a row whether or not anything is carrying its context. A prompt is what the
action is bound to in the contexts something carries (D84): empty for a context nobody is carrying,
and inclusive of a `private` binding.

**Rules out.** Deriving prompts by filtering the mapping list.

**Reversal.** The lookup reads the compiled plan for exactly this reason. `private` is a statement
about the list, not about whether the control works, so a filtered mapping list would drop a binding
that really does fire. The same distinction returns in the span components: picking the *n*th answer
indexes the lookup's answer across contexts and after a composite has expanded, which is
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

**Checked upstream rather than assumed.** winit already builds the per-key table this would need —
`ToUnicodeEx`/`MapVirtualKeyEx` on Windows, `UCKeyTranslate` on macOS, libxkbcommon state on
Linux — but only to fill in a `KeyEvent`'s own fields, and none of it is public. Requesting the
query is [rust-windowing/winit#4606](https://github.com/rust-windowing/winit/issues/4606); the
broader tracking issue, [#2678](https://github.com/rust-windowing/winit/issues/2678), has been open
since February 2023, assigned, and unimplemented. Not a gap to plan around closing soon.

### D82 — A prompt names the control, and the condition belongs to the prose

**Decided.** A prompt, as text or as an icon, draws the control that fires the action and whatever
must be held with it, and nothing about how it has to be pressed. "Hold ⟨X⟩ to reload" is a sentence
the game writes around a prompt that reads "X". Chunk 136's gallery found the two paths disagreeing;
chunk 133 makes the text path agree with the icon one.

**Rules out.** Captioning a hold or a multi-tap in the prompt itself, as `fallback_format` did for
`PromptSpan`, and giving an icon prompt a condition to draw.

**Reversal.** Four things break.

- **A custom condition cannot be rendered.** `ConditionDescriptor` knows a hold and a multi-tap, and
  a `Custom` condition describes as nothing, so a prompt that captions conditions is right for two
  and silently wrong for the rest.
- **How a condition is described is a choice, and the prose writer's.** "Hold", "press and hold",
  "long-press", or a charge meter beside the prompt instead of any word at all. A button glyph
  depicts what the player sees; a condition has no depiction, only a wording, and a prompt that
  supplies one takes it from the game.
- **A condition does not move, so its wording never goes stale.** A prompt exists to follow what can
  change under it: a rebind, a preset, a brand, a context switching. Capture, overrides and presets
  all move the control and leave the condition as declared, so the words describing it can be static
  text. The one player-facing switch that does change how a control is pressed, `hold_or_toggle`,
  was never in `ConditionDescriptor` either, and prose around it reads the tunable.
- **A game may not want the condition shown at all.** A prompt that always captions it gives that
  game no way to leave it out short of rewriting the prompt.

**What stays.** `Prompt::condition` still says which condition a binding has, and a rebinding screen
still formats one: a row describes the binding, where a prompt only names what to press.

### D83 — Glyph resolution takes a `ControlOrigin`, and a platform is not a tier

**Decided.** `resolve_glyph` resolves one `ControlOrigin`, and `Glyph::Own` carries it, so a
modifier resolves like a key and an atlas keys art by `name()`. A foreign origin answers `None`.
Which picture a Mac draws for Alt or Super is decided by what draws, from its own art, and
`GlyphTier` does not name a platform. Chunk 133 built it, for icon chords.

**Rules out.** A `Glyph::Modifier` variant beside `Own`, a second resolver for modifiers, and a
`GlyphTier` per platform.

**Reversal.** Back to `Control`, and every caller matches on the origin before it can resolve, with
a separate path for the one entry kind a chord has that is not a control (D78). A platform tier puts
platform detection in a crate that otherwise has none, and multiplies the keyboard tier for the
handful of keys a platform labels differently. `Glyph::Own` can hold a foreign origin it is never
given; that is the accepted price.

### D84 — A prompt names what an action is bound to, not what would fire it now

**Decided.** `prompts` answers from every context something carries, whether or not it is active,
shadowed by an exclusive context, or has a control consumed by a stronger one. A context nobody
carries is still left out. Activation no longer raises `PromptGeneration`; arriving and leaving
does. Chunk 134 built it, and withdrew R18.2 to do so.

**Rules out.** A present-tense lookup in any form: a scope flag either way round, a second trait
method, and a liveness field on `Prompt`.

**Reversal.** The filtered answer was never usable on its own, for three reasons.

- **A prompt never stands alone.** It is inside a sentence or a table row that only the app knows
  when to show, so emptying the prompt leaves "— new game" on screen. The crate cannot remove the
  prose, and an app hiding the hint from its own state has no use for a lookup that also hides it.
- **Players read a binding as what a control does in its mode.** A dialog over the game does not
  make "Ctrl+N: new game" false, and the rebinding screen already lists every gameplay binding while
  none of them can fire.
- **Consumption only mattered for one binding of several.** Space consumed by an always-on context
  and J beside it made the filtered answer "J". That is two actions on one control, which is a clash
  for the bindings to resolve and `conflicts` to report (R19.3), not for a prompt to hide.

A liveness predicate a hint can follow, for an app that wants one, is a deferred row gated on
reactive UI, rather than a filter on this lookup.

### D85 — An inline icon prompt and a block one are two components

**Decided.** `IconPromptSpan` is a span in a line of text, drawing `InlineImage`s from art
pre-scaled to sit in that line. `IconPrompt` is a node of its own, drawing image nodes scaled from
the full-size art to whatever height its `Node` is given. Each falls back to text in its own layout
kind. They share resolution, the wait for art, and the manifest. Chunk 110 built the block one.

**Rules out.** One component that is a span or a node depending on where it is spawned.

**Reversal.** Their layout knobs differ, and a block prompt has to align like any other node on its
screen, which a component shaped around the inline case cannot promise while `InlineImage` has no
size of its own. A merged component would change layout kind underneath its caller.

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
loud. *Shape* and *family* are the mapping's own constraints. *Excluded* is the screen's own
controls and is silent. Reserved is asked first, and one shared predicate answers for both a live
capture and a control loaded from a file.

**Rules out.** Asking in implementation order, and treating exclusion as a refusal.

**Reversal.** Pressing the settings key should hear that it is spoken for, not that its channel is
wrong. An excluded control is not being refused — it is busy doing its normal job, which is how the
key that cancels a capture reaches the thing that cancels it. One predicate is what stops a control
getting two different reasons depending on which direction it arrived from; before the two were
merged they genuinely disagreed, and no test noticed.

**Where the loudness comes out changed, and the order did not.** D89 moved the refusal from an event
the crate fires to an answer the screen gets from `Rebind::checked` at the write. The ordering here
is what that predicate still does, so pressing the settings key is still answered with the reason it
cannot be bound rather than a complaint about its channel; *excluded* is still the silent case, and
still the only one capture decides by itself.

**Reserving has two halves and the second is the one that matters.** A reserved binding takes no
mapping *and* its controls are refused by capture across the family. Without the second half a
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

**Three limits, stated rather than hidden.** Comparison is of whole slots: the same control with the
same chord, order ignored (D81). It used to be at control granularity, which over-reported `S`
beside `Ctrl+S` — harmless while chords were fixed, and a false steal once a player could author
both. Two real clashes now go unreported instead: `Ctrl+S` beside a chord naming one Control key by
`KeyCode`, and two same-length chords a player holding both sets satisfies at once. A clash across
two contexts is *possible* rather than certain, because whether two contexts are ever live together
is a question about the game's activation rules. And the whole target mapping is excluded rather
than the one slot, so a control repeated across two slots of one row is invisible here; a caller
about to write a row already holds that list and needs no help spotting a duplicate in it.

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
binding is `Building` or `Firing`, not only on the tick it fires, because a binding that fires once
per direction entered says nothing in between — a claim scoped to the firing tick alone would hand
the stick back to the game underneath for exactly the ticks the player was still holding it. The
same rule covers a charging `.hold()` and a part-way `.multi_tap()`: nothing about consumption is
special to navigation, this is just where the gap was first found.

---

## Overrides and persistence

### D45 — An override is a diff keyed by mapping and family, holding slots

**Decided.** Rows are keyed by `(family, mapping)` and hold slots: a control, and what is held with
it (D81). Not by action, not by binding. Nothing in an override names a device.

**Rules out.** One row per action, and any device identity in the file.

**Reversal.** An action has several bindings, so `Jump` is Space *and* South; the unit of rebinding
is the mapping, since the player rebinds "move forward" and never `Move`; and only the source
belongs to the player, because modifiers and conditions are developer data (D27) and the knobs a
player does get are tunables (D32). Per-family separation is what keeps a keyboard remap from
disturbing the gamepad layout.

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

**Four slot cases, and the last two bite.** A slot the defaults fill has its source rewritten. A
slot they left empty is filled by *copying* the binding beside it, so a secondary carries the same
modifiers and conditions as the primary rather than arriving bare. A slot the override no longer has
takes its binding away, and so does a slot the override *emptied* while a later one still holds
something. Copying only works where a binding reads one control — copy a binding that read four and
its other three directions would land in their own rows a second time — which is one reason a
composite expands into a binding per part (D76).

**A gap survives the rewrite only because it is carried, not derived.** Rows are otherwise
re-derived from the rewritten bindings so that the two cannot disagree, but a binding list says what
is bound and never in which column — an emptied primary and a row that only ever held a secondary
compile to the same single binding. So the accepted override's own list is carried through to the
presentation row. The derivation stays the authority on which controls are bound; only the override
knows where the holes are.

### D49 — The control encoding is a format we own

**Decided.** `key/Space`, `pad/South`, `key/ControlLeft+key/KeyS`. Written by hand rather than
derived, with one table per family, a scalar accepted and written where a row holds one control, and
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

### D58 — An unrecognized version refuses the set; no migration exists yet

**Decided.** `resolve_saved` requires `SavedOverrides::action_map_version` to equal the one format
this crate has ever shipped; any other value returns `UnsupportedVersion` rather than resolving
anything. There is no migration mechanism, because no second version has ever existed to say what it
would convert from.

**Rules out.** Silently reinterpreting a newer or otherwise unrecognized set as the current version —
resolving whatever rows happen to look familiar and discarding the rest without saying so.

**Reversal.** A migration path designed now would be designed against a guess, since nothing has
ever shipped a second version to specify what changed. Refusing is what a version field is for in
the meantime: rejecting with a stated reason costs nothing to undo once a real second version needs
a real migration, where guessing wrong now would already have shipped a converter for the wrong
shape.

**Accepted.** A save from a build that came later — a rollback, a second machine on a newer patch, a
Steam beta branch — is rejected outright rather than partially salvaged. That is a stricter tolerance
than R17.2 gives an unresolved row, and deliberately so: a resolved row from the wrong version's
default is a mismatch the game cannot see the way it can see an `Unresolved`.

**What forces a bump, and what doesn't.** Growing the vocabulary never does: a new `Control` name, a
new family, a new mapping or tunable name, or a third row-state word all fail safely on an older
build, because unknown text in any of those positions is already reported rather than guessed at —
an `UnknownControl`, an `Unresolved`, a skipped family table. A new row-state word is safe
only because it cannot be mistaken for a control name (every real one carries a `/`); the two words
that exist and the control-name table are exactly the vocabulary this crate must never redefine.
What forces a bump is reusing one of those with a new meaning, or changing a row's shape rather than
its vocabulary — redefining what `"cleared"` means, reassigning a control-name string to a different
physical control, or moving a row from a scalar-or-list to some other shape. The first kind an old
build silently gets wrong; the second it fails on with an unlabeled parse error instead of a labeled
refusal. Either is what the version field exists to catch.

### D59 — Persistence goes through a separate, reflectable type

**Decided.** `Overrides` is never itself the wire type. `SavedOverrides` is a plain, `Reflect`-derived
struct — `action_map_version`, `bindings`, `tunables`, each an owned `String`-keyed map — that
`save_overrides`/`resolve_saved` convert to and from. Its own fields are the only ones it claims;
`action_map_version` rather than a bare `version`, since a settings layer may place these fields
beside an unrelated struct's under one shared table (R17.10).

**Rules out.** Deriving `Reflect` on `Overrides` itself. A hand-rolled `Serialize`/`Deserialize` pair
on `Overrides` as the crate's only persistence path, with no plain-data type standing in for it.

**Reversal.** `Overrides`'s own fields hold a `MappingKey`, constructible only from a `&'static str`
the game already compiled in (D50), and no generic reflection walk can manufacture one from loaded
data — the same reason `OverridesLoader` was a `DeserializeSeed` rather than a plain `Deserialize`
before this decision replaced it. A settings crate that lets several resources share one TOML table
by name (rather than one file per resource) turns every bare field name `SavedOverrides` has into a
claim against whatever else lands in the same table — the same problem R1.8 solves for action paths
by requiring a namespacing convention, except here there is no author to apply one: the app picks the
shared table, not this crate. Two crates both wanting a field named `version` is far likelier than
both wanting one named `bindings`, which is why only the generic name is renamed.

**Accepted.** `bindings` and `tunables` are themselves unprefixed field names and carry the same
collision risk in principle — accepted as unlikely rather than eliminated, since prefixing every field
this crate ever writes would cost legibility for a risk this small. Structural reflection also costs
two things a hand-rolled encoding controlled: `bindings`' family tables sort alphabetically by name
rather than in `DeviceFamily`'s own declared order, and an empty `tunables` table still gets written
rather than omitted.

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
crate's timing is unsatisfiable from it — but `fired()` and `ActionPhase` have to keep working or
the promise that a consumer need not know which backend produced a value is false. A second write
path would have to reimplement the state machine, and two implementations of the lifecycle is
exactly the drift that promise forbids.

**A condition on a backend-owned action was to be a plan-build error.** The backend has its own
activators and will not deliver a hold or a multi-tap, so the game asked for behaviour it will not
get and nothing else would tell it. What shipped needs no diagnostic: `delegate` takes no input and
returns no builder, so there is nothing to chain a condition onto in the first place (D71). The one
contradiction the declarations can still express — binding an action the same context delegates — is
the error that remains.

**A context is a layer.** Steam allows one action set active per controller plus a stack of layers,
where this crate runs any number of contexts at once. **Measured** (`docs/steam.md` S19):
`ActivateActionSet` is exclusive and last-call-wins, so two sets genuinely cannot be live together.
But a set per context was never required (S20): one control can drive several actions in one set and
Steam picks no winner, so a backend may declare every delegated action in a single set and let this
crate's own contexts, priorities and consumption do the arbitrating. Sets then partition contexts by
what can be live together, not one per context — and mutually exclusive contexts, which `EXCLUSIVE`
and the exclusion ceiling already name, are exactly where a set boundary can fall. Layers stack and
override in the direction priorities already do, so a backend activates one base set and pushes a
layer per active context — and an action bound in several contexts is one action declared once in
the base set. Consumption is the part layers cannot express: a lower layer's action is shadowed or
it is not, and there is no equivalent of one context claiming a control for a frame.

**Checked against `steamworks` 0.13.** Layers are real in the Steamworks SDK
(`ActivateActionSetLayer`/`DeactivateActionSetLayer`), but the safe Rust binding exposes only the
base set — `activate_action_set_handle` and nothing for layers. The function exists in the SDK a
real backend links against, so this is a binding gap rather than a platform one: reachable by a
patch upstream to that crate, or by a direct FFI call past it. Chunk 151b does not need them: S20's
single set covers one player.

**Superseded in part by D92**: the value enters the fold as a binding's input rather than at the
state machine after it, and the game's conditions and modifiers on that binding run. The
level-to-edges half stands, and so does "no second write path".

### D69 — Netcode replication targets L2, not L1

**Decided.** A network peer's actions are replicated through the authority-backend seam (D22, D51)
— an already-resolved `ActionValue`, keyed by the action's own stable path (R1.1) — not by shipping
raw `InputFrame`s for a remote peer to run back through bindings and conditions. Local replay,
record/replay CI tests (R10.8) and chunk 83's rewind stay on L1, where re-deriving through the
mapping layer is the point; a network peer is treated as just another authority backend.

**Rules out.** A raw-frame wire protocol as netcode's default path, and any design that assumes two
peers share a `Plan`.

**Reversal.** L1 replication needs every peer to evaluate a remote player's frame against an
identical `Plan` — same bindings, same modifier thresholds, same tunables — to reproduce the same
action, so a patch, a mod, or a player's own rebind has to stay in lockstep across peers or the
resimulation silently diverges, and the wire carries device and calibration detail no receiving peer
needs. Authority-backend replication carries none of that: whichever `Plan` produced the value stays
local, and the receiving peer never re-derives anything from it.

**Note.** The record/replay argument for L1 — a replay re-derives through bindings and conditions,
an action-level mock cannot — is real, but it argues for testing rigor, not for a network wire
format. R10 conflated the two before this decision separated them.

### D70 — `mouse`'s feature entry also asks for `bevy_input/keyboard`

**Decided.** `RawEvent::FocusLost` is read whenever `keyboard` or `mouse` is enabled (R16.1),
because a real windowed game gets Bevy's `KeyboardFocusLost` either way — `bevy_window`'s own
`Cargo.toml` asks for `bevy_input/keyboard` unconditionally, and Cargo unifies that on across the
whole build no matter what a game's manifest requests. This crate's own isolated builds never pull
`bevy_window` in, so the same unification has to come from this crate's own graph instead: `mouse`
requests `bevy_input/keyboard` alongside `bevy_input/mouse`.

**Rules out.** Gating the read on `feature = "keyboard"` alone, which is the bug this decision
fixes, and a marker feature of this crate's own for "wants focus tracking" — the fact being modeled
belongs to `bevy_input`'s module boundary, not to a choice one of this crate's games makes.

**Reversal.** `mouse`-only, `cargo check`ed in isolation, stops compiling, because
`bevy_input::keyboard` leaves that build's feature graph; and if the read reverted with it, a mouse
button held through alt-tab would stick again with `keyboard` disabled, the R16.1 gap chunk 108
closed.

### D71 — An authority's values arrive as a component, not through a trait object

**Decided.** The value an authority backend supplies reaches the evaluator through
`AuthorityValues`, a component on the context entity, written by an ordinary system ordered before
evaluation. Actions it drives are named by `controls.delegate::<A>()`, which allocates a plan slot
with no binding behind it. There is no `dyn AuthorityBackend` the evaluator calls.

**Rules out.** A backend resource the evaluator asks — the `World` singleton R0.3 forbids, and with
it any game whose two players are on two different backends — and a boxed trait object held per
entity, which would run the backend inside the evaluator system with no system parameters of its
own.

**Reversal.** Per-entity values are what make R0.4's per-context split fall out with nothing extra,
and what let a network backend's receive queue, an asset handle or a platform SDK's own resource be
ordinary system parameters. A trait object the evaluator pulls from has access to none of those, so
every implementor would have to carry its own way in — which is the interior mutability and the
hidden channel a `&self` call inside a system forces.

**Note.** D51's "sampled when asked" survives the change of direction: a system ordered immediately
before evaluation samples once a tick, which is what a pulled call would have done. What a trait is
still the right shape for is the half this does not cover — origins, glyphs, whether an action is
bound at all, and delegating a rebind to the backend's own UI (R18.8, R19.8). Those are asked on
demand by a settings screen rather than once a tick by the evaluator. `Prompts` is already that
trait for the first three; chunks 151c and 151d are where a real backend implements it and delegates
a rebind.

**Superseded in part by D92**: `delegate` and the whole-action slot it allocates. The component
survives, and is what an authority binding reads.

### D92 — An authority is a binding source for one device family

**Decided.** An authority backend supplies an input, not an action. The game declares it as a
binding in the context, naming the device family it stands in for, and its value enters the fold
beside the context's own bindings for other families. Conditions and modifiers chained onto that
binding run on it as on any other. Binding the authority's own family as well is the contradiction
that remains an error.

**Rules out.** An action owned whole by one source, which is what `delegate` expressed; and skipping
the game's conditions for an authority's value.

**Reversal.** Steam Input owns the gamepad and nothing else, so a Steam game's `Thrust` comes from
the keyboard and the pad at once. Under whole-action ownership that cannot be declared: a context
may not both bind and delegate one action, and a second context is a second type the gameplay code
does not read. Reverting puts every Steam game back to choosing between the keyboard and the pad.

**Conditions come in two kinds, and only one is the backend's.** A hold or a double-tap as a way of
pressing is the player's, and under Steam the player sets it in Steam's layout. A rate of fire or a
bomb's charge time is the game's rule, declared on a binding because that is where conditions go.
D51 skipped both, so a Steam player holding fire got one shot. The crate does not tell the two
apart; the game says per binding what the authority's input carries. What stays skipped is the stick
shaping the backend has already applied (R14.10) — a game does not chain a deadzone onto an
authority binding.

**What survives from D51 and D71.** The value is a level sampled once a tick, and the state machine
synthesizes the edges. It arrives through `AuthorityValues` on the context entity, written by a
system ordered before evaluation, with no trait object.

**The family is what presentation reads.** An authority binding is a mapping row whose rebind goes
to the backend (R19.8), so a controls screen shows the keyboard rebindable here and the pad
delegated, from the declarations alone. Prompts for that family come from the backend's `Prompts`
(R18.8).

**How it was missed.** R0.4 split authority per action and R0.6 had a backend own a device, and the
two were never checked against each other. `pong_robot` fit the first, because its two owners are
two players. Steam, the case both requirements were written for, is the second.

### D52 — Pairing is a runtime handle, filtered at the frame

**Decided.** `DeviceHandle` models keyboard and mouse as one value, a gamepad as the backend's own
entity — nothing a save file should ever compare across a restart. Filtering happens once, at the
earliest point a raw event reaches a context, before anything else sees it.

**Rules out.** Treating the runtime handle as persistent identity, and layering pairing onto the
consumption or exclusion machinery.

**Reversal.** A backend reassigns gamepad entities on reconnect. Filtering at the frame is what
keeps consumption and the exclusion ceiling computed once per context *type* and untouched by
pairing, and a context with no pairing reads every device, so nothing that predates the component
changed behaviour.

**The join gesture needed no new evaluation path**, and chunk 116 narrowed which existing one it
takes. The design both replace proposed evaluating a designated context against every unassigned
device — a second per-device evaluation cycle running parallel to the main one. What shipped first
was an ordinary action bound with `bind_class` on a context with no pairing of its own, reading
every device as any unpaired context does and taking the presser off the raw event. What ships now
is that same ordinary action on a context spawned once per available device, each `Paired` to its
own, so the entity a press arrives on already names who pressed it.

**Why the narrowing.** A class binding answers only while input is hardware events. A backend that
supplies action values directly reports no control it read, so there is no raw event to take a
device from and the recipe has no answer at all — which is the one backend this crate means to
support. The paired listener asks the pairing instead of the event, so it holds under either. The
class-binding recipe survives in `join.rs` as the shorter option for a game that will never run that
way, rather than as the one the crate teaches first.

**Still open.** Owner-scoping consumption and the exclusion ceiling. Nothing in tree needs
it: no game pairs two different-priority contexts to different devices where one's consumption would
wrongly reach the other.

### D64 — Brand is a fact about one pad, resolved by vendor id, current generation only

**Decided.** `GamepadBrand` (Xbox / PlayStation / Nintendo / Generic) is resolved from a connected
gamepad's `vendor_id` alone, through `GamepadBrands` — a resource seeded with the three current
console makers' USB vendor ids and extended with `insert` for hardware this crate does not ship
pre-resolved. `Control::fallback_label_for_brand` reads face buttons, bumpers, triggers,
Select/Start and Mode in that brand's own current-generation words; sticks and the D-pad are
unaffected.

**Rules out.** Keying the override by device entity instead of vendor id; tracking which of a
player's several paired gamepads is "the" one a prompt speaks for; and spanning more than one
console generation's naming per brand.

**Reversal.** A per-entity override would handle a pad that misreports its own vendor id — a rarer
case than an unlisted vendor, and not one anything in tree hits. Which device a prompt speaks for
is D36's refusal already, extended to a new axis: an occupant with two gamepads paired is an edge
case nothing here ranks, on the same terms as `PromptDevice` never being defaulted. Spanning
generations would need a fourth axis — Xbox 360's "Back"/"Start" became Xbox One's "View"/"Menu",
PS4's "Share" became PS5's "Create" — that R11.6 does not ask for and brand alone cannot resolve;
committing to current-generation-only sidesteps guessing at it.

### D72 — `Brand` is attached to the gamepad's own entity, by an observer on `Add<Gamepad>`

**Decided.** The resolved `GamepadBrand` is cached as a `Brand` component on whichever entity
`DeviceHandle::Gamepad` already names — Bevy's own gamepad entity for the built-in backend — rather
than on an entity this crate spawns for the purpose. An observer on `Add<Gamepad>` attaches it,
skipping an entity that already carries `Brand`.

**Rules out.** A crate-owned device entity mirroring Bevy's, and a scheduled system filtered on
`Added<Gamepad>` in place of the observer.

**Reversal.** A crate-owned entity would need every consumer holding a `DeviceHandle::Gamepad` to go
through a second mapping to reach it, and an authority backend would have to spawn and keep that
mirror in sync instead of inserting one component on the entity it already controls — the
`Query<&Brand>` read path D64 wants to survive an authority backend depends on there being no such
indirection. The observer over the scheduled system is a smaller reversal: nothing outside this
module can tell which one attached `Brand`, so switching back would touch only
`resolve_gamepad_brand` and its registration, not any caller.

**Note.** An observer fires the instant something inserts `Gamepad`, with no ordering to arrange
against whichever system did the inserting — a scheduled `Added<Gamepad>` system would need either
an explicit `.after` on that system or Bevy's own auto-inserted `apply_deferred` sync points, and
would also run every frame rather than only when a gamepad connects.

### D73 — Connection signals derive from the raw gamepad event, not Bevy's own

**Decided.** `DeviceDisconnected` and `DeviceConnected` (R15.5) are raised from
`RawGamepadEvent::Connection` reaching the frame — the same vehicle `apply_frame` already reads to
clear held state on disconnect (chunk 62) — rather than from Bevy's own `GamepadConnectionEvent`,
which only `gilrs`'s plugin writes.

**Rules out.** An app reading `GamepadConnectionEvent` directly, which `docs/issues.md` 1020 had
already declined to endorse.

**Reversal.** Under Steam no `GamepadConnectionEvent` is written at all, since only `gilrs`'s plugin
writes one — reasoned in `docs/steam.md`'s appendix, and measured by chunk 151e; a backend already
has to synthesize `RawGamepadEvent::Connection` to keep held-state clearing working under Steam.
Reading Bevy's own event instead would leave every non-`gilrs` backend unable to raise either signal
— the same one-backend trap D64 and D65 already refuse elsewhere in this group.

**Note.** `DeviceDisconnected` is entity-targeted; `DeviceConnected` is not. The crate knows exactly
which `Paired` a lost device belonged to, but not which pairing, if any, a newly connected one is
*for* — per D53, that judgment is the app's, so the connect side is a plain, unscoped event rather
than a guess.

### D74 — A persistent device identity carries the backend's own type, under a declared domain

**Decided.** `DeviceId` wraps a payload the backend defines, and the backend declares a
`DeviceIdentity` implementation carrying `const DOMAIN: &'static str`. The domain is the save key.
Each backend keeps whatever guarantee its own identity actually has: Bevy's gamepad backend can
offer only `GamepadModelId`, a vendor and product id, while Steam's `InputHandle_t` may survive a
restart without colliding — `docs/steam.md` S14's layout suggests it does, and nothing has measured
it.

`Clone`, `Eq` and `Hash` come from the trait's bounds and are captured as function pointers when a
`DeviceId` is built — never from `reflect_clone`/`reflect_partial_eq`/`reflect_hash`.

**Rules out.** One identity type shared across backends — a string, an enum with a variant per
backend, or any shape that averages a strong guarantee down to a weak one. Also the Rust type path
as the save key, and `Option<DeviceId>` as a stored field.

**Reversal.** The domain and the payload's encoding are both save format, so changing either
orphans every pairing and calibration a player has stored. The bounds are public API: a backend
already implementing `DeviceIdentity` would stop compiling.

**Why the bounds rather than reflection.** `Hash`, `PartialEq` and `Debug` are special-cased by the
`Reflect` derive and generated into the type's own impl rather than stored as type data. Nothing can
add them afterwards, and nothing can detect at registration that a backend left them out — so a
reflection-sourced `Hash` compiles clean and panics the first time that device is plugged in. From
the bounds, the same omission does not compile. This is the whole reason the trait has bounds at all
rather than being a marker.

**Accepted: identical controllers collide.** A vendor and product id names a model, not a unit, so
two of the same pad are indistinguishable. That is gilrs's gap rather than an OS limit — [gilrs#154]
has a maintainer confirming no per-unit id survives an unplug, and [gilrs#158] shows Windows itself
distinguishing two Joy-Cons that gilrs collapses. Neither has a fix in progress. A separate macOS
bug, [gilrs#207], loses the runtime id across every Bluetooth reconnect; persistent identity is
needed regardless, since nothing at the runtime-handle level survives a restart by definition.

**Accepted: an unreadable entry costs its whole field.** A domain no running backend claims fails to
deserialize, the failure propagates out of whatever collection held it, and a settings layer that
swallows a failed field drops the readable entries beside it. Settings may drop what they cannot
read and revert to defaults, so this is priced rather than designed out.

**Accepted: identity requires `bevy_reflect`.** The trait requires `Reflect`, so a build without
that feature has no persistent identity at all.

**The empty case is a table, not a null.** `SavedDeviceId` exists because a reflected `Option`
writes its empty case as `none`, which TOML cannot spell — a settings layer writing TOML fails on
the whole file rather than omitting the field. Anything persisted stores `SavedDeviceId`, never
`Option<DeviceId>`.

[gilrs#154]: https://gitlab.com/gilrs-project/gilrs/-/work_items/154
[gilrs#158]: https://gitlab.com/gilrs-project/gilrs/-/work_items/158
[gilrs#207]: https://gitlab.com/gilrs-project/gilrs/-/work_items/207

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
mappings hold a control, which control an action is bound to, whether a row is rebindable — is the
crate's. A decision that depends on what the game is — what to do about a clash, which device a
prompt speaks for, how two controls read on one row — is the app's, and the crate's job is to make
it cheap to answer rather than to answer it.

### D75 — The pointer is picking's pipeline, and the mapper carries what picking leaves

**Decided.** Pointer *position* is not a signal this crate carries, and nothing binds to it. The
mouse reaches the mapper as buttons, relative motion and the wheel; everything positional — hover,
drag, click gestures, touch, the raycast into the world — is `bevy_picking`'s, which runs parallel
to this pipeline with neither feeding the other. R13.1, R13.6, R13.8, R13.9 and R15.10 are withdrawn
on this.

**Rules out.** An absolute position on the input frame; a binding whose source is where the pointer
is; click-vs-drag disambiguation derived from raw buttons; split-screen pointer-to-viewport mapping
keyed on a player.

**Reversal.** Two arguments, and the second is the one that holds. A mapper's contribution is the
rebinding layer, and no game lets a player rebind where the mouse is — asked directly, LWIM's
maintainer priced it at "extremely low, probably none. I don't think I've ever seen a game with that
design." That is one maintainer's judgement and would be thin alone. What carries it is that a
position means nothing except against a camera: a game wants the pointer in world or UI coordinates,
and a mapper not owning the camera cannot supply them, so it would hand over a window coordinate
that every caller converts itself. This crate's own rebinding screen is the demonstration — wholly
pointer-driven, and it reaches none of it through the mapper.

**What is left is coexistence, not a pipeline.** The two systems contend over one signal, the mouse
buttons, which R13.0 makes bindable and picking reads as clicks. Keeping them off each other is
suppression, and the levers are the app's: cursor grab, a barrier entity covering the screen,
deactivating the context. R22.4 owns documenting that, so what the crate owes is an ordering rather
than a mechanism.

### D86 — Registering a reflected type is the app's decision

**Decided.** A type derives `Reflect` when something reaches it through the registry: a scene
authors it, a tool reads or tunes it without being compiled against it, or reflection-based
persistence stores it. Evaluator state and transient handles do not qualify, and a marginal case is
left out, since adding a derive later breaks nothing and removing one does. The crate turns on no
`auto_register*` feature and keeps no `register_type` list; derived types reach the registry through
the app's `reflect_auto_register`, as Bevy's own library crates' do.

**Rules out.** `bevy_reflect/auto_register_inventory` in this crate's features; a list of
`register_type` calls in `ActionMapPlugin`.

**Reversal.** The feature alone registers nothing, because `App` builds its registry from derived
types only under `bevy_app/reflect_auto_register`, and enabling it from a library forces `inventory`
onto a `no_std` build and onto an app that chose `auto_register_static`. A hand-kept list is one
every new type silently skips.

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

### D61 — A gamepad stick is a `Control`, named whole

**Decided.** `Control` gains `GamepadStick(Stick)`, reporting `ChannelShape::Axis2` on the same
terms `MouseMotion` already reports `Delta2`. `ControlClass::of` becomes total — `AnyStick` fills
the one gap `Axis2` used to leave — so a stick is admissible, capturable and rebindable exactly as
the mouse already was: `part`, `set_part` and `arrival` all resolve a stick push to this one
control, never to one of its two axes.

**Rules out.** R19.12 as first written, which named sticks as the paradigm case of a device class
with no per-mapping rebinding, presets the only way to move one. That premise is what left
`ControlClass::of(Axis2)` with no answer, and `admissible` refused every control against it —
R19.12 is revised alongside this decision.

**What stays split.** Consumption does not follow: `for_each_control` still decomposes
`BindingInput::GamepadStick` into its two `GamepadAxis` atoms, unchanged, which is the granularity
`ConsumedControls` and reservation already key on. `Control::GamepadStick` is the first `Control`
naming something another `Control` also names in part, and it stays confined to presentation,
override and capture — it is never a claim.

**Reversal.** Every settings screen offering a capture button for a stick row would need to go back
to not offering one, and a saved file's `stick/Left` row would need to be read as unrecognized rather
than as a control.

### D60 — The character-producing door is a method, not a fourth `ControlClass`

**Decided.** `ControlClass` keeps the three shape classes, each true of a control's identity.
Character-producing keys get their own builder method, `bind_characters`, backed by a private filter
that is not a `ControlClass` variant.

**Rules out.** A fourth `ControlClass` variant standing for something that is a property of the
*event* a control produced rather than of the control itself — the same key is a dead key on one
press and a plain letter on the next.

**Reversal.** That variant shipped once (chunk 25) and had no caller until this one. Both places that
took a bare `ControlClass` — `CaptureSession::accepting` and `PromptScope::of` — could not honor it:
a capture accepting it refused every key and never ended, and a prompt scope narrowed to it came back
empty. `contains` carried a variant it could never say yes to, and `contains_event` existed only to
work around that. Reversing this brings all of it back.

**Measured, not documented.** The filter's shape comes from `examples/ime_diagnostic.rs` on macOS
rather than from Winit's documentation: a kana source delivers each keystroke as its own `Pressed`
with `text: Some(...)`, and no `Pressed` carries `text: None` mid-composition. A dead key (Option+I
then A) looked like a counterexample — through the bare diagnostic window it arrived as two plain
letters — but the same keystroke through Bevy's own text-input example produced one composed
character, so the gap was that window lacking IME composition, not a shape the filter misses.
Committing a multi-candidate kana-to-kanji conversion from an IME popup was never measured.

### D66 — Control classes are a closed set

**Decided.** The classes a binding can name (button-like, character-producing, and so on) are a
fixed enum. Third parties extend modifiers (R5.6) and conditions (R6.6) but do not add classes.

**Rules out.** The `Custom` variant D19 gives modifiers and conditions, applied here too.

**Reversal.** A class over text input is a correctness trap: an author writing one by hand gets
AZERTY, dead keys, and IME wrong and never learns it, because the QA pass that would catch it — one
that types Japanese — is exactly what a solo developer does not have (R24.8). Where a mechanism is a
footgun rather than a convenience, the crate owns it rather than opening it up.


### D67 — Capture reports a position, and logical keys are declared

**Decided.** A binding names a keyboard key either by position (`KeyCode`) or by the character the
player's layout produces (`LogicalKey(char)`), and the choice is the author's (R12.1). Capture
always answers with a position. Rebinding a logical row overwrites it with the captured control; the
default, logical binding returns on reset.

**Rules out.** A per-session physical-or-logical flag on `CaptureSession`, and a rule that preserved
the kind of the row being rebound.

**Reversal.** The distinction exists because an author cannot know the player's layout. The player
can — they are sitting at it, where the position and the character name the same key — so capture
has no ambiguity to resolve and gains nothing from being told which kind to record. Where the two
come apart is a layout change after the rebind, and there position is the better answer twice over:
a player switching scripts (US to Cyrillic, the common case) keeps working bindings where a logical
one would break outright, and a captured press *is* a position. Blender resolves letters logically
and is the case study for the cost: its GHOST layer compiles out the physical mapping for letters,
keeps it for digits — which AZERTY reaches only with shift — and the result is a long-running bug,
an add-on written to undo it, and a user population that believes the software does the opposite of
what it does. What makes that cheap for us to have got wrong is that only the *capture* default is
at stake: an author wanting the Blender behaviour declares the row logical, which is what R12.1
buys.

### D68 — One key, two control identities, and the crate does not reconcile them

**Decided.** `Control::PhysicalKey(KeyCode::KeyZ)` and `Control::LogicalKey('z')` are distinct
values that never compare equal, though on a QWERTY board they are the same key. Consumption,
conflict detection, chord out-ranking and reservation all key on `Control`, so none of them sees the
two as one. Documented, not reconciled.

**Rules out.** Normalizing one form to the other, and a layout-aware equality that would make
`Control`'s `Eq` depend on what keyboard is plugged in.

**Reversal.** What it costs is real: a context claiming the physical key does not suppress a logical
binding on it, and a rebind onto a position clashing with a logical row is not reported. What it
would cost to fix is worse. Equality would have to consult the current layout, which makes a `Hash`
and `Eq` that change under the player's hands — `ConsumedControls` and the plan's index are built on
those being stable — and the layout is exactly what the crate cannot see (R12.2, and winit#4606).
The collision needs two bindings on one key declared two different ways in one game, which is a
shape an author chooses rather than one they fall into.

### D87 — Which widget an action was about is the game's to remember

**Decided.** A game can bind one action inside a context that is active only while some widget has
focus, and have the observer act on whichever entity `InputFocus` currently names. One `Activate`
serves every button on the screen, and focus decides which button it meant.

That holds as long as the observer asks the question once. It stops holding when the game keeps
state between an action's `Fired` and its paired `Completed` or `Canceled`, which a pressed
highlight is the usual reason to do: the observer adds `Pressed` to the focused entity on `Fired`
and removes it from the focused entity on the paired event. Those are two separate reads of
`InputFocus`, and if focus moved in between, the second names a different button — so the first
keeps its highlight, with no event left anywhere that would clear it.

Neither event carries a target, and the crate will not grow one. An action's events report what the
action did, not which entity a game decided it was about. A game holding state across the pair
records the entity when `Fired` arrives and addresses the paired event to that entity instead of
reading focus a second time. A game that finishes its work at `Fired` and keeps nothing, as
`widget_focus.rs` does, has no second read to disagree with the first.

**Rules out.** A schedule ordering that resolves focus before evaluation (R22.11, withdrawn); a
target the crate supplies alongside the event; anything focus-shaped in this crate's own surface.

**Reversal.** The crate cannot supply the target, because it cannot tell that a target exists.
`active_if` takes an arbitrary run condition, and nothing marks one that reads `InputFocus` apart
from one that reads the clock — there is no notion of focus in the crate's model at all. Reversing
this means an activation condition grows a declared subject, which is a second activation mechanism
beside the one contexts already have.

**Why the game's share of this is small.** A mouse-driven button activates on release so that the
press can be taken back: hold the button, slide off the widget, release, and nothing happens. That
gesture needs a pointer that can move off a widget while held. Focus cannot — it jumps, and only
when something moves it, so there is no equivalent to slide off with. A focus-driven activation can
therefore settle at `Fired`, and nothing is left to decide when the control is released. What
crosses the pair is presentation and only presentation, and removing a highlight from the entity
that got it requires knowing nothing about what kind of widget it is.

### D88 — A claim names whose input it is

**Decided.** A consumption claim, and an entry in the exclusion ceiling, carry the devices of the
instance that made them. A reader sees a claim only where the two device sets intersect, so
`ConsumedControls::contains` and `claimant` take the reader's devices as a parameter. One player's
menu consuming `South` leaves another player's gameplay context free to read it; two instances of
one context paired to two pads do not take controls from each other; one player's pause menu does
not deactivate the other player's game. A live capture claims under its own session's pairing for
the same reason.

**The type that travels is `DeviceHandleSet`, not `Paired`.** `Paired` derives `Component`, and a
component type is used as a component and nothing else — what a function takes and a struct holds is
the plain value type it wraps. A caller holding a `Paired` derefs at the call site.

**No pairing and an empty pairing are different answers.** No pairing means every device: a
single-player context claims against everyone and is claimed against by everyone, with no opt-in. An
empty pairing means no device — a player entity spawned before a device reaches it, which is what a
join flow produces — so such a context hears nothing, claims nothing and shadows nobody. Collapsing
the two by reading an empty set as "unconstrained" is free in the evaluator, where a context that
hears nothing never actuates a binding, and wrong in `why_not`, which would then name a claimant
where the honest answer is `Unowned`.

**Rules out.** One priority ceiling for the world; a claim keyed by control alone;
`ConsumedControls` as a map, since a lookup matches a control *and* an overlapping device set and no
single key expresses both.

**Reversal.** Cheap while no public API has shipped and expensive afterwards, which is the whole
point of paying for it now: `contains` and `claimant` are public, and a third-party context that
reasoned about a world-wide claim table breaks when the table stops being world-wide.
`docs/one-way-doors.md` door 2 is this door seen from `bevy_enhanced_input`'s side, where it is
still open.

### D89 — A capture reports a control on the way up, and the store judges it

**Decided.** `CaptureSession` carries a class and an exclusion list. It carries no target — no
mapping, no slot — and makes no judgement about admissibility. A deliberate press ends the capture
and is reported, whatever it was; whether it may be stored is asked once, at the write, through
`Rebind::checked`. The row's own cell arithmetic is `Overrides::with_cell`, and `Rebind` is the
token that writes a checked row.

**Rules out.** `CaptureSession::for_slot` and `within`; `CaptureSession::mapping`, `slot` and
`family`; the target fields on `ControlCaptured`; `CaptureRefused` and a public `RefusedReason`; and
the observer that warned when a screen opened a capture past `MaxSlots`, which needed a slot number
to warn about.

**Reversal.** One question was being answered in three places. `for_slot` returned `None` for a row
the player may not change, `run_captures` fired a refusal for a wrong shape, family or reserved
control, and applying asked all of it again plus the row length and chordability the first two could
not see. Only the last was authoritative, and it is a strict superset, so the crate was holding a
write-time policy decided at listen time and could disagree with itself. Reversing this brings that
back, and makes `for_mapping` fallible again for a reason unrelated to the control it is listening
for.

**The target was an echo.** Both callers already correlate the answer through the entity the event
fires on, because they must: a screen has to know which cell is listening before the answer arrives.
Carrying the row and slot on the session as well meant two copies of one fact, of which the crate's
was the one nobody read.

**Blender's rule, deliberately.** A refusal used to leave the session listening. It now ends, which
is what lets the screen say why while the player is still looking at the cell — the alternative was
a whole row silently accepted into a working copy and turned down minutes later at Confirm. The cost
is that an arrival nobody chose must not end a capture either, so a deliberate press and a
continuous reading past its threshold are treated differently: the press always answers, the reading
only answers a session listening for that class. Without that split, a pad drifting on a desk
cancels every keyboard rebind in the game.

**The answer comes on the release, and that is a fix rather than a flourish.** A claim is an
instant; a held control is a level. `apply_level_event` records held state as events arrive and
consumption is applied where that state is *read*, so a session claiming on the press and then
vanishing left the control down with nothing claiming it — and the context underneath read it on the
very next frame. Observed: a capture on a settings screen took the arrow key *and* moved the
selection, because the menu's own navigation is a composite over those keys and `on_change` saw the
direction appear one frame late. The session therefore holds the control it is waiting on and
re-claims it every frame, so the claim lasts exactly as long as the press and the control is already
up when it stops. The defect predates this chunk — it reproduces unchanged on the tree before it —
and is folded in here because this is the chunk that decides when a capture ends.

**Rules out.** A claim that outlives the session, which would need an owner with a lifetime of its
own; `ConsumedControls` learning about releases, which would put a level concept in a per-schedule
claim table.

**What it costs.** `CaptureSession` carries the pending control, a second press while one is held is
ignored rather than latched, and every test that used to press now has to let go. The mouse's motion
keeps answering immediately, since a displacement that has already happened is never held and there
is nothing to wait for.

**What did not move.** The predicate itself, and the claim. `admissible` is still one function
answering for a press and for a save file, which is D42's point and survives it; and capture still
claims everything it takes, including a control the row cannot hold, so the settings key pressed at
a rebinding screen neither binds nor re-opens the screen.

**The screen pays for it, once.** A game now needs a line for the reason, where before it had an
event it could ignore — and ignoring it is what `examples/disasteroids` did, which is why that
silence was a known finding rather than a feature. A screen that wants the old silence drops the
`Err` arm.

---

### D90 — The default `ActionId` is the id of no action

**Decided.** `ActionId::PLACEHOLDER` is `u32::MAX - 1`, and `Default` answers it rather than
`ActionId(0)`. The registry hands out positions from zero upward and asserts it stays below the
placeholder, so no action is ever registered under it and every id-indexed lookup — `info`, a plan's
slot table — answers as it would for an action nobody bound.

**Rules out.** Dropping `Default` from `ActionId`, which `bsn!` needs on every component that holds
one; and handing ids out from anywhere but the registry's own length.

**Reversal.** `ActionId(0)` is the first action reached in the process, which is not a property of
anything the game wrote, so a prompt spawned without an action showed whatever that was — a caption
that reads as correct and names the wrong control. The failure is silent in both directions: no
warning, and no test can pin which action it would name.

**Two ids are not actions, not one.** `ActionIdCache`'s `UNRESOLVED` is `u32::MAX` and means "not
looked up yet", which a cache must tell from a resolved id. The placeholder sits immediately below
it so the two never collide, and the registry's exhaustion assert is against the lower of them.
