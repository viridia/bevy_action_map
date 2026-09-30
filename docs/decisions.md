# Decisions

Why `bevy_action_map` is shaped the way it is. [`design.md`](./design.md) says how it works and does
not say why; this is the other half.

**What earns an entry.** A decision belongs here only if reversing it would break the public API,
the save format, or the shape of the evaluation pipeline. The test is that the reversal cost can be
named in a sentence. If it cannot, it is a code comment, not a decision.

Each entry says what was decided, what it rules out, and what reversing it would cost. Where a
decision has an accepted price or an unresolved remainder, that is stated rather than left for a
reader to discover.

**What an entry keeps.** An entry is read by someone weighing whether to reverse it, so it keeps
what would change their mind: what going back would cost, and each fact that makes a rejected
alternative worse, cited so that its expiry can be noticed. A choice between alternatives that were
equally good needs no argument; saying it was arbitrary is enough, and tells the reader nothing is
waiting down the other path. An entry that has been revised states the decision as it now stands,
not how the argument got there.

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
| **D11** | Arbitration splits: priority is system ordering, chord length is plan data    | TD5.1     |
| **D12** | Consumption is recorded per schedule and flows forward                        | TD5.2     |
| **D13** | Exclusivity is a ceiling, not a third context state                           | TD5.3     |
| **D14** | A class binding is a second list, not an expanded set of controls             | TD5.4     |
| **D15** | ActionIntent decides how several bindings fold into one action                      | TD5.5     |
| **D16** | Nothing user-defined runs inside the evaluator                                | TD5.6     |
| **D17** | Transitions are generic entity events on the context entity                   | TD5.6     |
| **D18** | Require-reset holds back buttons only                                         | TD7.2     |
| **D94** | A key that arrives or goes away while down is treated alike on every path | TD5.7, TD5.8, TD7.2 |
| **D97** | A tick's time is charged once, to the state it ends in                        | TD5       |
| **D26** | Failures surface at the earliest tier that can catch them                     | TD4, TD7.3 |
| **D19** | Modifiers and conditions are enums with a `Custom` variant                    | TD8.2     |
| **D65** | The device model is closed; a third-party kind needs one in hand             | —               |
| **D20** | We own the whole dead-zone chain, in three stages, with one rescaling         | TD8.4     |
| **D21** | Calibration is set by the app, never detected                                 | TD8.4     |
| **D22** | Backends enter at two seams, not one                                          | —               |
| **D93** | Raw input is filtered at L0 by removal-only systems, and their authors declare | —               |
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

**Rules out.** Reading device state from inside the evaluator.

**Reversal.** Determinism, replay, headless testing and both backend seams (D22) stop sharing one
way in and each needs its own. Every other entry here assumes this one; reversing it is a different
crate.

### D2 — L1 is an event queue, not a level snapshot

**Decided.** The frame is an ordered queue of raw events with a position stamped on each, not a
per-frame snapshot of which controls are down.

**Rules out.** Sampling `pressed()` inside the evaluator, which `bevy_enhanced_input` and
`leafwing-input-manager` both do.

**Reversal.** A press and release inside one rendered frame collapse to nothing, so a fixed tick
spanning them sees neither. `InputFrame`, `RawEvent`, `FrameTimestamp` and the whole capture path
go with it, and the frame stops being the serializable per-tick record that replay and rollback
need.

**Accepted cost.** Under the snapshot model anything that writes `ButtonInput` is an input source
with no adapter. Here it is invisible until it writes the frame.

### D3 — Each context reads by cursor; retirement is separate and later

**Decided.** A context instance keeps its own position in the queue and reads what arrived since.
Discarding events is a second, independent step, running in `FixedPreUpdate` after fixed-tick
evaluation.

**Rules out.** A single global read position, and retiring at sample time.

**Reversal.** Retirement alone fails when the simulation does not step: nothing is retired, the next
frame appends to what is still queued, and a render context reads events it has already acted on.
Cursors alone fill the queue to its cap, which then drops events oldest-first whether read or not.
Retiring at sample time discards events before a fixed tick that has not yet run can see them, so a
frame with zero fixed ticks loses edges.

**Accepted cost.** The invariant is not local to the frame module: it holds only while evaluation
stays in `PreUpdate` and `FixedPreUpdate`. Moving either schedule breaks it silently. A system
reading the frame from `Update` sees contents that depend on whether the simulation stepped.

### D4 — Timestamps order events; they do not time them

**Decided.** A `FrameTimestamp` is a frame counter and an order within that frame, stamped as the
event is sampled.

**Rules out.** Attributing an event to the instant it truly occurred, and therefore attributing it
to the fixed tick it truly fell in. Bevy's input events carry no time of their own, so there is
nothing to stamp with.

**Reversal.** Cheap, and expected. Today the first fixed tick in a frame takes all of that frame's
events (TD2); real timestamps would spread them across the ticks they fell in. Magnitude is
conserved and each edge is seen once either way, so only the attribution policy changes, and nothing
public.

**Still open.** gilrs is polled once per frame, so gamepad events arrive as a batch whatever
keyboard and mouse gain, and timing-sensitive conditions are less precise on a pad.

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

**Rules out.** Action-as-entity, a packed byte buffer, and a typed tuple per action. The last two
serve state of varying size, and there is none: what varies belongs to _bindings_, and with its
parameters in the plan the rest is uniform.

**Reversal.** Snapshot and restore stop being two slice copies and become an archetype traversal.
Activation stops being a flag and costs an insert or a removal per action. Per-action change
granularity, which the bitset gives, is not something a single component's change tick can express.

**Accepted cost.** None measured. Per-action observers, which action-as-entity looks uniquely suited
to, work on any layout: a generic entity event carries the action in its type parameter and targets
the context entity (D17).

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

**Not enforced, by design.** Reading a `Fixed` context from `Update` is allowed and sees state as of
the last fixed tick, which is what a HUD or render-side interpolation wants. A `SystemParam` cannot
learn its schedule, so a check would be unreliable, and it would reject legitimate reads.

### D62 — An ongoing phase is `Building` or `Firing`, named by part of speech

**Decided.** Two phases, not one `Ongoing`: `Building`, a condition still short of firing, and
`Firing`, an action still active. The names follow R3.1: a gerund or adjective names a level, true
again next tick; a past participle names an edge, true for one tick only.

**Rules out.** One `Ongoing` phase, which a caller tells apart by re-reading the action's _value_;
`update_action_state`, `why_not_id` and Disasteroids' exhaust flame each needed to.

**Reversal.** The value test returns to those call sites, and any app matching `Building` or
`Firing` stops compiling, since `ActionPhase` is not `#[non_exhaustive]`.

### D63 — `cancel_in_flight` also cancels `Started`

**Decided.** Deactivating or shadowing a context cancels `Started` alongside `Fired`/`Firing`/
`Building`.

**Rules out.** Canceling only `Fired`, `Firing` and `Building`, which leaves a hold interrupted on
the tick it began at `Started` rather than `Canceled`.

**Reversal.** That hold reads as still building for as long as the context stays inactive, breaking
R7.4's "never left stuck as held forever" in a one-tick window narrow enough that no run found it.

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

### D11 — Arbitration splits: priority is system ordering, chord length is plan data

**Decided.** The obvious shape — one list of every binding touching a control, sorted by context
priority and chord length — is not built. Priority becomes system ordering, fixed at app build: each
distinct priority gets its own system set, ordered against the others in its schedule. Two contexts
declared at the same priority get their own nested set in turn, ordered after the one before it —
declaration order breaks the tie a priority number left open, the same way it already does
everywhere else two things read together (D15's fold, D36's prompt scan). Chord length is resolved
within one evaluation, against rivals the plan computes at build: each binding knows the longer
chords on its controls, and reads as rest while one of them is held.

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
keeps strongest-wins and `Delta2` still sums.

The fold is the rule both alternatives get wrong. Under strongest-wins, opposite contributions do
not cancel and W with D loses the diagonal: each reads as whichever binding was declared first.
Under a sum, same-direction contributions add: A and Left reach -2, and two half-deflected sticks
read as one full deflection, which is D15's objection. Within one sign the strongest wins, so a
maximum does not depend on declaration order.

**Rules out.**

- **A composite the evaluator or the override path can see.** The player is shown one row per part
  (R19.9, D27), and a row that is one part of a composite can neither take another control nor be
  emptied. Unity's composites carry the same limit; BEI's `Cardinal` and Unreal's Enhanced Input
  expand to separate bindings.
- **A direction held as `negate` and `swizzle` modifiers**, BEI's form. The part names the row, and
  modifiers would have to be read back into a name, which a custom modifier defeats.
- **A clamp in the fold.** No sign can exceed its strongest contributor, and a per-binding `scale`
  is legitimate, so a clamp would cap it silently. A diagonal's length was never automatic: W and D
  give (1, 1), and `clamp_magnitude` in the stage after the fold normalizes it (D100).

**Reversal.** The composite returns with `CompositeCannotGrow` and `CompositeCannotEmpty`, and a
controls screen shows a cleared cell that Confirm then refuses. The fold cannot be reversed alone:
under strongest-wins, split parts lose the diagonal.

**Accepted cost.** Taking each axis's maximum separately can bend the direction when two analog
vectors combine, which nothing in tree does; a button-sourced part feeds one axis and cannot. A
composite runs per-binding bookkeeping four times rather than once; the control lookups, which
dominate, are unchanged.

### D100 — An action has one stage after the fold, and it takes conditions

**Decided.** Whatever shapes an action's combined value goes in one stage after the fold, declared
with `combined::<A>()`: a modifier chain and conditions, run on the combined value rather than on
one binding's.

**Rules out.** Conditions only per binding. A condition per part judges a key, and a menu asks about
a direction. Measured headless, with arrow keys under `on_change().pulse(0.25)`: as one composite,
Up held then Right pressed fired the diagonal and repeated it on one timer; as four bindings, it
fired Right alone, then alternated Up and Right on two timers, and releasing Right moved nothing.

**Reversal.** `combined` is public; without it a composite has no way to normalize a diagonal, and a
menu repeating on a direction repeats per key instead.

### D78 — A modifier is a chord entry the runtime reads, not shorthand that expands

**Decided.** `ModifierKey` names one of the four keyboard modifier pairs and reaches the compiled
plan intact, as a `ChordEntry::Modifier` satisfied at read time by either key of the pair. Chord
length counts entries, so a modifier contributes one however many keys satisfy it.
`ControlOrigin::Modifier` carries it to presentation and is the only variant answering `None` to
`control()`. Side-agnostic is the short spelling and a `KeyCode` is how one physical key is named,
because either-side is the case a chord almost always wants.

**Rules out.** A `Side` enum whose `Either` variant sits beside `Left` and `Right`, which makes the
ordinary case the elaborate one; naming a representative key in a caption; and D76's treatment,
expansion at declaration. Expansion is cheaper to evaluate, one lookup per entry rather than two,
but both keys of a pair are live at once, so it doubles the *binding*: `Ctrl+S` becomes two bindings
on one row, listed twice on a rebinding screen and captioned with a side the player is free to
ignore. A composite's parts are alternatives the player rebinds separately; a modifier's two keys
are one thing they press either of.

**Reversal.** The doubled rows and captions return; `ChordEntry::Modifier` and
`ControlOrigin::Modifier` are public.

### D79 — A rebinding row's chord rides inside the slot

**Decided.** `ActionMapping::slots` holds `Option<BoundSlot>`, and a `BoundSlot` is a control plus
the `ControlOrigin` list held with it. `ControlOrigin` rather than a control list for D78's reason —
a modifier stands for either key of its pair — which also makes a row and a caption composable from
the same values. An override writes the same type (D81), and `ActionMapping::controls()` is a bare
view for reading.

**Rules out.** A parallel `chords` list beside `slots`, index-aligned. It would leave `.slots` call
sites alone, but "this slot is empty" would have a place in each list and a way to disagree, the
shape the saved format rejects for the same reason.

**Reversal.** The alignment invariant returns, and it bites at an override: a row with a gap needs
its chord list gapped identically, and a list that drifts by one puts a chord on the wrong slot.

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
names, `mod/ctrl+key/KeyS`. Conflict detection compares slots: the same control with the same chord,
order ignored.

**Rules out.**

- **An inherited chord**, `with: Option<..>` where absence keeps the declared one, and with it the
  crate choosing between Blender's reset and preserve on a game's behalf. Under inheritance,
  `"key/KeyD"` on a row declared `Ctrl+S` means `Ctrl+D` today and whatever the next patch declares
  tomorrow, and "deliberately no chord" needs a spelling of its own.
- **A write type holding `ChordEntry` beside the read type.** It makes an unholdable entry
  unrepresentable, at the price of a fallible conversion at every edit and a `cfg` split for the
  no-devices build.
- **Bare modifier words**, `ctrl+key/KeyS`, which cannot spell a chord of two buttons without a
  second vocabulary.

**Reversal.** Every saved row is read differently, so reversing this after a release is a format
version.

**Accepted cost.** A preset or a save naming a bare control on a chorded row drops the chord, and a
patch that revises a declared chord does not reach a row the player rebound, as a revised control
already does not. `ControlOrigin` admits entries nothing can hold, a `Foreign` control or a stick,
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

**Where it is judged.** Per binding, after the press threshold and before the conditions, which see
rest while it is armed. Judged on the action's value after conditions, it lifted on the tick after
activation, since a hold still charging contributes rest, and the hold then charged and fired on the
pre-held key; a tap or a hold-and-release would fire on its release. Moving it back reopens both.

### D94 — A key that arrives or goes away while down is treated alike on every path

**Decided.** A context can gain or lose a key the player is holding without the player pressing or
releasing it: a higher context's claim lifting or arriving, an authority starting or stopping
supplying an action, and an interruption such as focus loss or a disconnect. Every such path follows
one rule. A key that arrives already down is ignored by `Button` actions until the player releases
it, as on activation (D18), while analog actions take its value at once. A key that goes away while
down ends what it was firing as `Canceled`, not `Completed`, since the player never let go. Only the
actions bound to that key or that authority source are affected.

**Rules out.** A per-path choice, such as a lifted claim delivering a fresh press on the grounds
that the lower context never saw the key go down.

**Reversal.** The same player gesture, a layer going away with a key down, would fire an action on
one path and not another, and an observer of `Canceled` would have to know which mechanism took the
key away to interpret it.

### D97 — A tick's time is charged once, to the state it ends in

**Decided.** A tick takes its events one at a time, and only the run of each binding's pipeline at
the closing step is handed the tick's `delta`; a run recording a reading superseded partway through
the tick is handed zero. The state the tick ends in is taken to have lasted the tick, so a press and
a release inside one tick read as a tap.

**Rules out.** Handing every fold the whole `delta`, which charges a hold once per event, so a pad
hold charges two to three times faster while a stick is moving. Also splitting `delta` across the
events: they carry no time between them, so any split is invented.

**Reversal.** After a hitch, a quick press would read as held for the whole long tick, turning a tap
into a hold: a dodge becomes a sprint. That is the worse of the two errors a long tick can make.

### D98 — A binding's pipeline runs once per reading

**Decided.** A binding's *reading* is what its input shows at one moment, taken from held state, the
claims and the plan, with an availability saying whether it counts. Its pipeline records a reading
once each time the reading changes within a tick, and once at the closing step; only the bindings an
event reaches are read, through an index the plan builds. An action commits after an event whose
superseded readings reached it, and once at the closing step, taking its other bindings' outputs as
they stand. A control going away while held, by claim, focus loss, disconnect or authority, is an
availability of the reading, and one rule makes the action `Canceled` (D94).

**Rules out.** Evaluating every binding after every event, which needs each stateful stage guarded
for time, require-reset and claims, and still steps the phase machine and a condition's previous
value on events for unrelated controls. Also a separate mechanism per way a control can go away.
Also collapsing one axis's events within a tick, which loses a trigger crossing its press point and
back, the edge R9.3 protects for buttons.

**Reversal.** A tick costs events times bindings again, which measured 68 µs for sixteen key events
on a 48-binding context. An unbound key changes what a bound action reports. Each stateful stage
needs its own guard against running twice on one reading, and each interruption path its own flag.

### D77 — A disabled action is out of evaluation, as an inactive context is

**Decided.** Disabling one action cancels what it had in flight and takes its slot out of
evaluation: its bindings read nothing, advance no scratch, claim no control and out-rank no chord,
and an authority's value for it is ignored. Enabling re-arms require-reset on that slot alone, and
only when it was disabled. The switch is per instance.

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
`Modifier` or `Condition` trait and arrives through a `Custom` variant holding an `Arc`, which
survives the clone an override's apply makes of the authored bindings (D48). Built-ins dispatch
statically and stay exhaustively matchable; extensions work. `Modifier` and `Condition` carry no
`Reflect` bound: they are developer data and never reach a save file, which stores controls, what is
held with them, and tunable values.

**Rules out.** A closed enum, which blocks third-party modifiers outright, and trait objects
throughout, which cost dispatch on every binding every tick.

**Reversal.** The `Arc`s are allocated during plan compilation, never in the steady state. Going to
trait objects throughout moves that cost into the per-tick path.

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

### D21 — Calibration is set by the app, never detected

**Decided.** Stage 1 is a manual API. The app sets each axis's calibration from code; measuring it,
storing it and showing it to a player are the app's. The crate applies it and names the device, by
`Identity`, so the app has a key to store it under.

**Rules out.** Learning a stick's centre while the game is running, and a measuring step in the
crate.

**Reversal.** A stick deflected while detection is running would be learned as centre, and hardware
that misreports would poison the measurement silently — which is the failure mode this exists to
prevent, so a background detector is worse than no calibration at all. A measuring step is X47's:
the one the crate shipped learned a released stick's deflection as rest, and doing it right needs
expertise most game developers lack, while popular titles leave it to the platform.

**Accepted cost.** A game that wants calibration to outlive a connection writes the store and the
restore itself, and the crate offers no vetted way to do it yet (`docs/issues.md`, 1074).

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

**Still open.** Whether an authority's actions can take part in rollback, which is X23.

### D93 — Raw input is filtered at L0 by removal-only systems, and their authors declare

**Decided.** A filter is an ordinary system in a set between sampling and the frame's first reader.
It removes events sampled this frame, and cannot insert, reorder or rewrite one (R0.6). Several
authors can each have one — a backend, a player's settings, the game — and because each only
removes, they compose as an AND in any order, with a run condition to turn one off. The first
customer is a single binary that serves a Steam launch and a direct one: it needs `bevy_gilrs` for
pads without Steam, and when Steam Input starts, the pad is read twice (`docs/steam.md` S2, S21).
Replay needs none: R10.4 injection already replaces live input for the player it drives.

**Rules out.**

- **A callback per event.** It costs a dynamic call on every event and cannot see the world, so a
  player's ignore list would have to be copied into it.
- **Filtering into a temporary buffer.** Two copies, and no more restrictive than a view of the
  queue.
- **Per-device filtering for Steam.** Steam's emulated pad carries the vendor and product id of the
  hardware under it (`docs/steam.md` S1), and nothing relates Steam's controller handle to an OS
  device (S3), so Steam's filter drops the gamepad family. Other authors can filter per device.
- **Detection built into the crate.** A filter that switches itself on and off between runs is the
  worst kind of input bug to diagnose. A filter can still do it; the crate documents against it.
- **A Cargo feature on this crate.** A feature that removes behaviour breaks Cargo's additive rule:
  one crate in the graph enabling it would silence gamepads for every other.

**Reversal.** Filters written as systems, and the order-independence of removal, are what games
build on. Allowing a filter to rewrite or insert events would break the queue's sort order, which
`events_after` binary-searches, and make the result depend on filter order.

### D23 — Focus integrates by activation, and interception is static

**Decided.** The kind of widget that has focus activates a context, and what that context claims is
ordinary context composition. It claims a control before dispatch; a widget never decides at
handling time whether to let an input fall through. A game replaces `InputDispatchPlugin` with a
context per widget kind, activated by an ordinary run condition, and needs no crate support to do
it.

**Rules out.**

- **A bubbling interception pass beside the mapper's own.** `FocusedInput` bubbles because focus is
  the only arbitration `bevy_input_focus` has. Here priority and consumption decide whether
  something else claims a control before a focus-activated context runs.
- **Dynamic interception, by a widget or an observer.** Consuming stops lower-priority contexts, not
  the same action's other observers; an observer suppressing its peers makes the outcome depend on
  which ran first. `Ctrl+Z` as undo-in-field while a text input has focus needs no election: the
  field's context claims it, and with focus elsewhere that context is inactive and the global
  binding wins.

**Reversal.** Two arbitrations answer one question and can disagree, and dynamic interception breaks
the single deterministic pass D11 establishes.

**Accepted cost.** A widget kind with no context of its own gets no keyboard or gamepad input, since
the default dispatch plugin is off. A widget that claims a control on its own internal state, such
as a text field swallowing `Ctrl+Z` only while its undo stack is non-empty, makes that state part of
activation (a `TextFieldWithUndoHistory` context), which keeps the claim inspectable by R22.1.

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

**Decided.** Action mapping stays out of `bevy_input`, and focus-context activation stays out of
`bevy_input_focus`, whatever else moves upstream. Focus activation is a `focus` feature here.

**Rules out.** Fusing mapping into `bevy_input`, a policy layer with a much larger and more
contested API over a data layer; and focus activation in `bevy_input_focus`, which depends on the
action and context model.

**Reversal.** `bevy_input` becomes unadoptable for anyone wanting only raw input, and
`bevy_input_focus` depends on the whole action system.

### D101 — The crate names no backend and draws nothing

**Decided.** Nothing under `crates/bevy_action_map/src/` names Steam: not a feature, not a variant,
not a trait method. Nothing in the crate depends on `bevy_ui`; what draws lives in the examples
until it earns a crate of its own. Both are enforced in the tree.

**Rules out.** A Steam backend inside this crate: it is `std`-only, `unsafe` FFI beneath, and wants
the Steamworks redistributable at link time, where this crate is `no_std` and forbids unsafe. And a
`bevy_ui` dependency, since `bevy_ui` already depends on `bevy_input` and `bevy_input_focus`.

**Reversal.** The seam loses its test, which is that the Steam backend in `steam_examples/` builds
against the public API alone. A `bevy_ui` dependency inverts the layering and forecloses `bevy_ui`
ever using action maps itself.

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
and a crate larger than a mapper alone, for work every game with a controls screen does anyway.

**Assumed, not weighed.** The first sketch of the API took this for granted, and it is recorded in
hindsight because D6, D27 and D31 rest on it; no alternative was argued against.

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

**Rules out.** Making listing follow rebindability.

**Reversal.** A gamepad `Jump` with no mapping vanishes from the screen, which is backwards for the
commonest gamepad screen, where the console or Steam owns remapping and the game still shows what
the pad does. Rebindability is the developer's call, since a fixed binding is a design decision;
seeing the controls is the player's.

**`mappable` takes no arguments.** A composite's parts name themselves, so the key derives as
`gameplay.move.up` and a catalogue turns `up` into "Move Forward"; an author-supplied name would
name the part twice, where no translator looks. The family is inferred from the controls, since
declaring it is one more chance to disagree with what is bound.

### D29 — A mapping is an ordered list of slots

**Decided.** A *mapping* is the named thing a player rebinds; a *slot* is one position in it holding
one control, **or nothing**, as `None` in the list; a screen draws one cell per slot, and how many
is the screen's business. A slot is addressed, not appended: `Overrides::with_cell` grows the row to
reach whatever slot the screen names, leaving the slots it skips empty.

**Rules out.**

- **One control per mapping, and a fixed two.** One cannot express the two-cell row every shipped
  game's keyboard table has, and forces a second row under an alias (`thrust`, `thrust_alt`) that
  tells the player one thing is two. Two cannot express the "add shortcut" button tools grow.
- **`Control::Empty`.** Every consumer of a control (the frame, prompts, labels, admissibility,
  conflict comparison) gains an arm meaning "not a control", and two empty slots compare equal as a
  conflict. `Some(control)` never matches an empty cell.
- **A map keyed by index**, which answers a sparse question nobody asked and loses the scalar
  shorthand the save format keeps.
- **A per-mapping capacity.** No shipped screen read one; a caller wanting a column count has it
  from the list or a constant of its own. A global ceiling on a row's length remains, as a resource
  the app sets, because a corrupt or hostile save is the crate's business.
- **Refusing a slot more than one past the end.** Once a hole is legal, that rule allows the primary
  of an emptied two-cell row and refuses the secondary, on no principle a screen could explain.

**Reversal.** The row is save format: narrowing it orphans every saved secondary, and a
`Control::Empty` changes a public enum.

**Save format.** Position is which slot, so a cleared middle slot is written with the cleared marker
an emptied row uses, rather than as a shorter list, which would promote the secondary to primary.
Trailing empties are not written, so a two-column table with mostly blank secondaries does not fill
a settings file with the marker.

### D30 — `follow` declares a shared control once, against the leader's bindings so far

**Decided.** Two actions that deliberately share one control — tap to dodge, hold to sprint — are
declared with `follow::<Follower, Leader>`, which reads whatever the leader has declared *at that
point* and generates a matching binding per device found. "So far" is an ordering rule, which lets a
follower ride only some of a leader's devices on purpose. A follower may ride a listed-and-fixed
row, which has nothing to rewrite but still keeps a duplicate row off the screen.

**Rules out.**

- **Declaring the link per binding.** It retypes a control the leader already named, once per
  device, and nothing checks that the counts match, so a forgotten repeat gives a follower that
  rides part of a row while drawn as riding all of it. Rebind the throttle and the afterburner stays
  on the old key, and whatever the player later puts there acquires an afterburner.
- **Inferring the link** from two bindings naming one control, which is as often coincidence as
  intention. Conflict detection cannot tell either: it looks for two rows holding one control, and
  this failure is a separation.
- **Requiring the leader's row to be `mappable`**, which fails the build of the game `follow` exists
  for.

**Reversal.** The per-binding form returns, and with it the afterburner bug.

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

**Reversal.** A layer contradicts D47, where applying starts from the pristine declaration and never
stacks, and adds a "which preset" field to the persisted format. Today a screen computes the
selected preset by comparing what is bound against each registered one.

**A preset may move a `Fixed` row.** Moving rows a capture screen offers no button for, such as
every gamepad binding in a typical game, is what a preset is for. `apply_overrides_with_preset`
exempts exactly the rows that preset names. A third `RebindPolicy` state would instead make every
game revisit each correct `Fixed` declaration.

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
written out rather than derived from Bevy's names, and so are the fallback labels, which say what a
control is: `LeftTrigger` shows as a bumper and `LeftTrigger2` as the trigger, and the stored
`mouse/Back` and `mouse/Forward` show as **Mouse 4** and **Mouse 5**, as every other settings screen
calls them.

**Rules out.** `Debug`, serde on `KeyCode`, and deriving display text from upstream identifiers.

**Reversal.** Those names belong to Bevy, and a rename upstream would silently orphan every saved
binding. Owning the table costs about two hundred lines and turns an upstream rename into a compile
error in an exhaustive match while the stored string stays what it was.

**Accepted cost.** The fallback answers for a US keyboard, so a binding to a physical key shows an
AZERTY player the wrong letter, and an app supplies the control half of its catalogue per layout.
Bevy reports what a physical key produces only in an event that has already happened, and winit
builds the per-key table without exposing it; X12 carries the upstream request.

### D82 — A prompt names the control, and the condition belongs to the prose

**Decided.** A prompt, as text or as an icon, draws the control that fires the action and whatever
must be held with it, and nothing about how it has to be pressed. "Hold ⟨X⟩ to reload" is a sentence
the game writes around a prompt that reads "X". `Prompt::condition` still says which condition a
binding has, and a rebinding screen still formats one: a row describes the binding, where a prompt
names what to press.

**Rules out.** Captioning a hold or a multi-tap in the prompt itself, and giving an icon prompt a
condition to draw.

- **A custom condition cannot be rendered.** `ConditionDescriptor` knows a hold and a multi-tap, and
  a `Custom` condition describes as nothing, so a captioning prompt is silently wrong for the rest.
- **The wording is the game's.** A condition has no depiction, only a wording: "hold", "long-press",
  or a charge meter and no word at all. A game may not want it shown.
- **A condition does not move.** A prompt exists to follow what can change under it: a rebind, a
  preset, a brand, a context. All of these move the control and leave the condition as declared, so
  prose describing it can be static. `hold_or_toggle`, the one player-facing switch that changes how
  a control is pressed, is read from the tunable by the prose around the prompt.

**Reversal.** Prompts caption conditions again, wrongly for every custom one, and a game loses the
wording and the choice to omit it.

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
carries is left out. A context arriving or leaving raises `PromptGeneration`; activation does not.
R18.2, which asked otherwise, is withdrawn.

**Rules out.** A present-tense lookup in any form: a scope flag either way round, a second trait
method, and a liveness field on `Prompt`.

- **A prompt never stands alone.** It sits in a sentence or a table row only the app knows when to
  show, so an emptied prompt leaves "— new game" on screen, and an app hiding the hint from its own
  state has no use for a lookup that also hides it.
- **Players read a binding as what a control does in its mode.** A dialog over the game does not
  make "Ctrl+N: new game" false, and the rebinding screen already lists every gameplay binding while
  none of them can fire.
- **Consumption mattered only for one binding of several.** Space consumed by an always-on context
  and J beside it made the filtered answer "J". Two actions on one control is a clash for
  `conflicts` to report (R19.3), not for a prompt to hide.

**Reversal.** Prompts go blank inside prose that stays on screen. A liveness predicate a hint can
follow is deferred as X9, gated on reactive UI, rather than a filter on this lookup.

### D85 — An inline icon prompt and a block one are two components

**Decided.** `IconPromptSpan` is a span in a line of text, drawing `InlineImage`s sized from that
line's font. `IconPrompt` is a node of its own, drawing image nodes scaled from the full-size art to
whatever height its `Node` is given. Each falls back to text in its own layout kind. They share
resolution, the wait for art, and the manifest. Chunk 110 built the block one.

**Rules out.** One component that is a span or a node depending on where it is spawned.

**Reversal.** Their layout knobs differ, and a block prompt has to align like any other node on its
screen, which a component shaped around the inline case cannot promise. A merged component would
change layout kind underneath its caller.

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
controls and is silent: an excluded control is busy doing its normal job, which is how the key that
cancels a capture reaches the thing that cancels it, and it is the one case capture decides alone.
Reserved is asked first, so the settings key pressed at a rebinding screen is told it is spoken for,
not that its channel is wrong. One predicate answers for a live capture and a control loaded from a
file; the screen gets its answer from `Rebind::checked` at the write (D89).

Reserving has two halves: a reserved binding takes no mapping, and its controls are refused by
capture across the family. Without the second, a player cannot rebind the settings key away but can
bind something else over it.

**Rules out.** Asking in implementation order, and treating exclusion as a refusal.

**Reversal.** Two predicates give a control two reasons depending on which way it arrived; before
they were merged they disagreed, and no test noticed. Asked in implementation order, the settings
key is refused for its channel.

### D43 — Conflicts are detected, never resolved

**Decided.** `conflicts` and `conflicts_pending` are pure queries over the mapping list, answerable
before anything is committed. What to *do* about a clash — reject, swap, unbind the other, allow the
duplicate — is the app's.

**Rules out.** A crate-owned `ConflictPolicy`, and an `Overrides::rebind` that resolves conflicts
and writes several rows on the app's behalf. `Overrides::bind`, `set` and `get` already express
every policy: reject is not writing, allow-the-duplicate is writing anyway, and swap and
unbind-the-other are reading the conflicting row and writing it back with one control removed or
traded. The four are worked examples in a doc comment.

**Reversal.** A policy enum becomes public API that has to cover every game's rule.

**Accepted cost: three limits.**

- **Comparison is of whole slots**, the same control with the same chord, order ignored (D81).
  Comparing controls alone reports `S` beside `Ctrl+S` as a steal. Two real clashes go unreported:
  `Ctrl+S` beside a chord naming one Control key by `KeyCode`, and two same-length chords a player
  holding both sets satisfies at once.
- **A clash across two contexts is possible, not certain**: whether they are ever live together is
  the game's activation rules.
- **The whole target mapping is excluded**, not the one slot, so a control repeated within one row
  is invisible here. A caller about to write that row already holds its list.

---

## Navigation

### D44 — Two general combinators, not a navigation path

**Decided.** The crate adds `compass`, which rounds a 2D value to four or eight points and discards
the magnitude, and `on_change`, which fires on the ticks the value differs from the tick before.
Neither is about navigation. Together they fire once per compass point *entered*; with `pulse` after
them, that is auto-repeat. The crate stops at the value: it does not call `bevy_input_focus`. The
observer turning a direction into a focus move is four lines in the app, because only whoever
depends on both a widget library and an input mapper may associate them.

**Rules out.** A virtual cursor, which is slow to use, and a bespoke navigation path beside the
mapper, which puts a game's most-pressed controls where the rebinding screen cannot see them.

**Reversal.** A stick held off centre is off centre every tick, so a naive binding runs a menu off
the end of the list before the player lets go; one of the two alternatives above returns.

### D102 — A claim lasts while its binding is `Building` or `Firing`

**Decided.** A consuming binding claims its control on every tick it is `Building` or `Firing`, not
only on the tick it fires. A charging `.hold()` and a part-way `.multi_tap()` claim throughout.

**Rules out.** A claim scoped to the firing tick. A binding that fires once per compass point
entered (D44) says nothing in between, so the stick would go back to the game underneath for exactly
the ticks the player is still holding it.

**Reversal.** Every multi-tick condition leaks its control to lower contexts while it builds.

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
player is pairing state and which stick rests where is calibration state, both kept per device
rather than per profile. That separation is what lets two players with identical controllers and
identical mappings share one override table and differ only in pairing.

### D46 — A cleared row is a state of its own

**Decided.** A row is `Slots` or `Cleared`, and absence is the third answer: use the default.

**Rules out.** Absence as the only way to say nothing.

**Reversal.** Absence already means "use the default", so a player who deliberately empties a row
has nothing left to say with unless clearing has its own value.

**Revised by chunk 151d**, which withdrew a third saved state, `NotOurs`, for a row an external
backend owns. Once an authority is a binding (D92), its row is `Delegated` by declaration and holds
no slots, so a screen has nothing to write there and nothing produced the state. A saved marker also
answered the wrong question: whether a backend owns a row is the build's, not the player's. A file
still carrying the old word reads it as an unknown control, and the row keeps its default.

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
variant plan keeps the declared plan's slot allocation; how each slot is rewritten is TD10.1.
Presentation rows are re-derived from the rewritten bindings except for their holes: a binding list
says what is bound and never in which column, so an emptied primary and a row that only ever held a
secondary compile alike, and the accepted override's own list is carried through to the row.

**Rules out.** Patching the compiled bindings, and deriving a fresh slot allocation.

**Reversal.** Rewriting authored bindings is what makes loading the pure function D50 requires, and
why the custom modifier and condition variants hold an `Arc` rather than a `Box`. A fresh allocation
loses the slot of an action whose every binding the player cleared, so it reads as unbound and fires
the "not bound in this context" diagnostic meant for a typo; and an instance's action states and
require-reset flags no longer stay aligned across the swap.

### D49 — The control encoding is a format we own

**Decided.** `key/Space`, `pad/South`, `key/ControlLeft+key/KeyS`. Written by hand rather than
derived, with one table per family, a scalar accepted and written where a row holds one control, and
an emptied row spelled as a word no control name could collide with.

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
this crate has shipped; any other value returns `UnsupportedVersion` and resolves nothing. There is
no migration mechanism, since no second version exists to say what one would convert from.

**Rules out.** Silently reinterpreting an unrecognized set as the current version, resolving the
rows that look familiar and discarding the rest.

**Reversal.** Cheap: a real second version replaces the refusal with a migration. One designed now
would be designed against a guess.

**Accepted cost.** A save from a later build (a rollback, a second machine on a newer patch, a Steam
beta branch) is rejected outright. That is stricter than R17.2's tolerance for an unresolved row,
because a row resolved against the wrong version's meaning is a mismatch the game cannot see, as it
can see an `Unresolved`.

**What forces a bump.** Growing the vocabulary never does: a new control, family, mapping, tunable
or row-state word fails safely on an older build, as an `UnknownControl`, an `Unresolved` or a
skipped family table. A new row-state word is safe only because every control name carries a `/`. A
bump is forced by giving an existing word a new meaning, such as redefining `"cleared"` or
reassigning a control name to a different physical control, which an old build silently gets wrong;
or by changing a row's shape, which it fails on with an unlabeled parse error.

### D59 — Persistence goes through a separate, reflectable type

**Decided.** `Overrides` is never itself the wire type. `SavedOverrides` is a plain, `Reflect`-derived
struct — `action_map_version`, `bindings`, `tunables`, each an owned `String`-keyed map — that
`save_overrides`/`resolve_saved` convert to and from. Its own fields are the only ones it claims;
`action_map_version` rather than a bare `version`, since a settings layer may place these fields
beside an unrelated struct's under one shared table (R17.10).

**Rules out.** Deriving `Reflect` on `Overrides` itself. A hand-rolled `Serialize`/`Deserialize` pair
on `Overrides` as the crate's only persistence path, with no plain-data type standing in for it.

**Reversal.** `Overrides` holds `MappingKey`s, constructible only from a `&'static str` the game
compiled in (D50), which no generic reflection walk can manufacture from loaded data. And a settings
crate that lets several resources share one TOML table turns every bare field name into a claim
against whatever else lands there; R1.8 answers that for action paths with a namespacing convention,
but here the app picks the shared table, not this crate.

**Accepted cost.** `bindings` and `tunables` are unprefixed and carry the same risk, far smaller
than another crate's `version`, which is why only that name is prefixed. Structural reflection sorts
`bindings`' family tables alphabetically rather than in `DeviceFamily`'s order, and writes an empty
`tunables` table rather than omitting it.

---

## Backends and devices

### D51 — An authority backend writes a value, not a state

**Decided.** An authority backend supplies a level, sampled once a tick, and this crate's own state
machine diffs it and synthesizes the edges. The backend never writes action state. Where the value
enters, and which of the game's conditions run on it, is D92.

**Rules out.** A second write path into action state.

**Reversal.** Steam returns a level, sampled when asked, with no edge and no timestamp, so this
crate's timing is unsatisfiable from it; yet `fired()` and `ActionPhase` must keep working, or a
consumer has to know which backend produced a value. A second write path would reimplement the state
machine, and two implementations of the lifecycle drift.

**Steam's action sets are not contexts.** One set is live per controller and the last activation
wins (`docs/steam.md` S19), but one control can drive several actions in one set with no winner
picked (S20), so a backend declares every authority action in one set and this crate's contexts,
priorities and consumption arbitrate. Layers, which would let sets stack, are in the Steamworks SDK
and not in `steamworks` 0.13 (S10).

### D69 — Netcode replication targets L2, not L1

**Decided.** A network peer's actions are replicated through the authority-backend seam (D22, D51),
as an already-resolved `ActionValue` keyed by the action's stable path (R1.1), not as raw
`InputFrame`s a remote peer runs back through bindings and conditions. Local replay, record/replay
tests (R10.8) and rewind stay on L1, where re-deriving through the mapping layer is the point.

**Rules out.** A raw-frame wire protocol as netcode's default path, and any design that assumes two
peers share a `Plan`.

**Reversal.** L1 replication needs every peer to evaluate a remote player's frame against an
identical `Plan` — same bindings, same modifier thresholds, same tunables — to reproduce the same
action, so a patch, a mod, or a player's own rebind has to stay in lockstep across peers or the
resimulation silently diverges, and the wire carries device and calibration detail no receiving peer
needs. Authority-backend replication carries none of that: whichever `Plan` produced the value stays
local, and the receiving peer never re-derives anything from it.

**Note.** That a replay re-derives through bindings and conditions, where an action-level mock
cannot, argues for L1 in testing, not for a network wire format.

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
evaluation, which samples once a tick as a pulled call would. There is no `dyn AuthorityBackend` the
evaluator calls.

**Rules out.** A backend resource the evaluator asks — the `World` singleton R0.3 forbids, and with
it any game whose two players are on two different backends — and a boxed trait object held per
entity, which would run the backend inside the evaluator system with no system parameters of its
own.

**Reversal.** Per-entity values are what make R0.4's per-context split fall out with nothing extra,
and what let a network backend's receive queue, an asset handle or a platform SDK's own resource be
ordinary system parameters. A trait object the evaluator pulls from has access to none of those, so
every implementor would have to carry its own way in — which is the interior mutability and the
hidden channel a `&self` call inside a system forces.

**Where a trait is right.** For what a settings screen asks on demand rather than the evaluator once
a tick: origins, glyphs, whether an action is bound, and delegating a rebind to the backend's own UI
(R18.8, R19.8). `Prompts` is that trait for the first three.

### D92 — An authority is a binding source for one device family

**Decided.** An authority backend supplies an input, not an action. The game declares it as a
binding in the context, naming the device family it stands in for, and its value enters the fold
beside the context's own bindings for other families. Conditions and modifiers chained onto that
binding run on it as on any other.

**Rules out.** An action owned whole by one source, which `delegate` expressed; and skipping the
game's conditions for an authority's value.

**Reversal.** Steam Input owns the gamepad and nothing else, so a Steam game's `Thrust` comes from
the keyboard and the pad at once. Under whole-action ownership that cannot be declared: a context
may not both bind and delegate one action, and a second context is a second type the gameplay code
does not read. Reverting puts every Steam game back to choosing between the keyboard and the pad.

**Conditions come in two kinds, and the game says which.** A hold or a double-tap as a way of
pressing is the player's, and under Steam the player sets it in Steam's layout. A rate of fire or a
bomb's charge time is the game's rule, declared on a binding. Skipping both gave a Steam player
holding fire one shot. The crate does not tell the two apart; the game says per binding what the
authority's input carries. Stick shaping the backend has already applied stays skipped (R14.10): a
game does not chain a dead zone onto an authority binding.

**A level has a level's limits.** An authority's resolution is its own poll (D51), so R9.3's press
and release inside one frame stops at the queue; R9.4 holds, since the state machine makes one edge
where the level changes. A level has no event for a new instance to miss, so R7.5's hold-over
applies where the authority starts supplying an action while it is held: an instance's first sample,
and an action resuming after going unsupplied, as Steam's do across a change of action set. Absence
therefore means "not supplied", distinct from rest: a backend writes every action it supplies every
tick, at rest included, and a first write already held waits for a release.

**A follower rides its leader's value.** An authority binding is an input, so `follow` copies it as
it copies a control: the follower reads the value the backend wrote for the leader, and its own
conditions run on that. Reading the follower's own id instead would make the backend write every
follower separately, break `follow`'s promise that a rebind of the leader carries its followers, and
have a network peer send an already-held value that the follower's hold then runs over again.

**The family is what presentation reads.** An authority binding is a mapping row whose rebind goes
to the backend (R19.8), so a controls screen shows the keyboard rebindable here and the pad
delegated, from the declarations alone. Prompts for that family come from the backend's `Prompts`
(R18.8).

**Revised by chunk 112a**, which withdrew the error for a control of the authority's own family on
the same action. One binary serving a Steam launch and a direct one needs `Thrust` on both the
authority and `RightTrigger2`, and the error made it undeclarable. R0.4 is met at L0 instead: the
backend filters its family while it runs (R0.6), which also covers the actions it does not supply.
The control's row stands for the family and the authority adds none, so an override or a preset
lands on the control; a screen shows the pad delegated only where the game says its backend runs.
Reversing it puts such a game back to shipping two builds.

### D52 — Pairing is a runtime handle, filtered at the frame

**Decided.** `DeviceHandle` models keyboard and mouse as one value, a gamepad as the backend's own
entity, and nothing a save file compares across a restart. Filtering happens once, at the earliest
point a raw event reaches a context, before anything else sees it; a context with no pairing reads
every device. What pairing does to claims and the exclusion ceiling is D88.

**Rules out.** Treating the runtime handle as persistent identity, and a second, per-device
evaluation cycle beside the main one.

**Reversal.** A backend reassigns gamepad entities on reconnect, so a stored handle names the wrong
pad or none.

**Joining needs no evaluation path of its own.** A join gesture is an ordinary action on a context
spawned once per available device, each `Paired` to its own, so the entity a press arrives on names
who pressed it. A class binding on an unpaired context, taking the presser off the raw event, is
shorter and survives in `join.rs`, but a backend that supplies values directly reports no raw event
to take a device from.

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

**Rules out.**

- **Keying the override by device entity.** It handles only a pad that misreports its vendor id,
  rarer than an unlisted vendor and not hit in tree.
- **Tracking which of a player's paired gamepads a prompt speaks for.** That is D36's refusal to
  rank devices, on a new axis.
- **More than one console generation's naming per brand.** Xbox 360's "Back"/"Start" became Xbox
  One's "View"/"Menu", and PS4's "Share" became PS5's "Create": a generation axis R11.6 does not ask
  for and brand alone cannot resolve.

**Reversal.** `GamepadBrands` and `fallback_label_for_brand` are public; a generation axis changes
both.

### D72 — `Brand` is attached to the gamepad's own entity, by an observer on `Add<Gamepad>`

**Decided.** The resolved `GamepadBrand` is cached as a `Brand` component on whichever entity
`DeviceHandle::Gamepad` already names — Bevy's own gamepad entity for the built-in backend — rather
than on an entity this crate spawns for the purpose. An observer on `Add<Gamepad>` attaches it,
skipping an entity that already carries `Brand`.

**Rules out.** A crate-owned device entity mirroring Bevy's, and a scheduled system filtered on
`Added<Gamepad>` in place of the observer.

**Reversal.** A crate-owned entity puts a second mapping between every `DeviceHandle::Gamepad` and
its brand, and has an authority backend keep a mirror in sync instead of inserting one component on
the entity it already controls, which breaks the `Query<&Brand>` read path. The observer is local:
switching to a scheduled system touches only `resolve_gamepad_brand`, which would then need ordering
against whatever inserts `Gamepad` and would run every frame.

### D73 — Connection signals derive from the raw gamepad event, not Bevy's own

**Decided.** `DeviceDisconnected` and `DeviceConnected` (R15.5) are raised from
`RawGamepadEvent::Connection` reaching the frame, which also clears held state on disconnect, rather
than from Bevy's own `GamepadConnectionEvent`. `DeviceDisconnected` targets the `Paired` entity the
lost device belonged to; `DeviceConnected` is unscoped, since which pairing a new device is for is
the app's judgement (D53).

**Rules out.** An app reading `GamepadConnectionEvent` directly.

**Reversal.** Only `bevy_gilrs` writes `GamepadConnectionEvent`, so under Steam none arrives
(`docs/steam.md`'s appendix), while a backend already synthesizes `RawGamepadEvent::Connection` for
held-state clearing. Reading Bevy's event leaves every other backend unable to raise either signal.

### D74 — A persistent device identity carries the backend's own type, under a declared domain

**Decided.** `DeviceId` wraps a payload the backend defines, and the backend declares a
`DeviceIdentity` implementation carrying `const DOMAIN: &'static str`. The domain is the save key.
Each backend keeps whatever guarantee its own identity has: Bevy's gamepad backend can offer only
`GamepadModelId`, a vendor and product id, while Steam's `InputHandle_t` may survive a restart
without colliding (`docs/steam.md` S14 suggests it; unmeasured). Anything persisted stores
`SavedDeviceId`, never `Option<DeviceId>`: a reflected `Option` writes its empty case as `none`,
which TOML cannot spell, so a TOML settings layer fails on the whole file.

`Clone`, `Eq` and `Hash` come from the trait's bounds, captured as function pointers when a
`DeviceId` is built, and never from reflection. The `Reflect` derive generates `Hash`, `PartialEq`
and `Debug` into the type's own impl rather than storing them as type data, and registration cannot
detect that a backend left them out: a reflection-sourced `Hash` compiles clean and panics the first
time that device is plugged in. From the bounds, the same omission does not compile.

**Rules out.** One identity type shared across backends: a string, an enum with a variant per
backend, or any shape that averages a strong guarantee down to a weak one. Also the Rust type path
as the save key, and `Option<DeviceId>` as a stored field.

**Reversal.** The domain and the payload's encoding are both save format, so changing either
orphans every pairing and calibration a player has stored. The bounds are public API: a backend
already implementing `DeviceIdentity` would stop compiling.

**Accepted costs.**

- **Identical controllers collide.** A vendor and product id names a model, not a unit. That is
  gilrs's gap, not the OS's: [gilrs#154] has a maintainer confirming no per-unit id survives an
  unplug, and [gilrs#158] shows Windows distinguishing two Joy-Cons that gilrs collapses. Neither
  has a fix in progress. [gilrs#207], on macOS, loses even the runtime id across a Bluetooth
  reconnect.
- **An unreadable entry costs its whole field.** A domain no running backend claims fails to
  deserialize, the failure propagates out of the collection that held it, and a settings layer that
  swallows a failed field drops the readable entries beside it. Settings may revert to defaults, so
  this is priced rather than designed out.
- **Identity requires `bevy_reflect`.** A build without it has no persistent identity.

[gilrs#154]: https://gitlab.com/gilrs-project/gilrs/-/work_items/154
[gilrs#158]: https://gitlab.com/gilrs-project/gilrs/-/work_items/158
[gilrs#207]: https://gitlab.com/gilrs-project/gilrs/-/work_items/207

### D95 — Rumble is one level per pad, and the last write wins

**Decided.** `Rumble` is a component on the pad's entity holding one intensity, held until it
changes. Several reasons to rumble at once are the game's to combine, in components of its own and
one system that writes `Rumble`. Chunk 167 built it.

**Rules out.** The component combining sources itself, whether by keeping the strongest request or
by holding a keyed entry per source: either puts an ownership scheme in the API that one game in
several needs, and a keyed entry left behind by a despawned source holds the pad rumbling.

**Reversal.** Setting `Rumble` to zero, or removing it, stops the pad today; with sources combined
it would stop only one source, and a game relying on it to silence the pad would leave it buzzing.
Every backend reading the component, Steam's included (chunk 168), would read a combination instead
of a value.

### D96 — The upstream gamepad prototype is a module, not a crate

**Decided.** The backend-neutral gamepad layer proposed to Bevy is
`crates/bevy_action_map/src/gamepad/`, written as it would sit in `bevy_input`: it depends on Bevy
alone, and the rest of the crate depends on it. The proposal itself is `docs/proposals/gamepad.md`,
posted as a gist. Chunk 165 built the module.

**Rules out.** A crate of its own under `crates/`.

**Reversal.** A crates.io name is permanent, and this one would be abandoned once `bevy_input` takes
the work. `bevy_action_map` cannot publish while it depends on an unpublished crate, so the crate
would have to be published first, under that name.

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

**Reversal.** The crate's answer would be *plausible*, so an app wanting something else works around
it rather than simply not using it. Collaborators have already said this crate takes on more than it
needs to.

**The test.** A fact the crate is uniquely placed to know, such as which mappings hold a control or
whether a row is rebindable, is the crate's. A decision that depends on what the game is, such as
what to do about a clash or which device a prompt speaks for, is the app's, and the crate makes it
cheap to answer.

### D75 — The pointer is picking's pipeline, and the mapper carries what picking leaves

**Decided.** Pointer *position* is not a signal this crate carries, and nothing binds to it. The
mouse reaches the mapper as buttons, relative motion and the wheel; everything positional — hover,
drag, click gestures, touch, the raycast into the world — is `bevy_picking`'s, which runs parallel
to this pipeline with neither feeding the other. R13.1, R13.6, R13.8, R13.9 and R15.10 are withdrawn
on this.

**Rules out.** An absolute position on the input frame; a binding whose source is where the pointer
is; click-vs-drag disambiguation derived from raw buttons; split-screen pointer-to-viewport mapping
keyed on a player.

**Reversal.** A position means nothing except against a camera, which a mapper does not own, so it
would hand over a window coordinate every caller converts itself. And a mapper's contribution is
rebinding, which no game offers for where the mouse is: asked, LWIM's maintainer priced the demand
at "extremely low, probably none." This crate's own rebinding screen is wholly pointer-driven and
reaches none of it through the mapper.

**Accepted cost.** The two systems contend over the mouse buttons, which R13.0 makes bindable and
picking reads as clicks. Keeping them apart is the app's, by cursor grab, a barrier entity covering
the screen, or deactivating the context; R22.4 owns documenting it.

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

**The motivating cases are device-shaped.** Which of four pads pressed Start is device scoping;
every contributor in a debug overlay is the inspection dump reading the plan; a value that remembers
where it came from is its own smaller question. A case that needs the distinction should arrive with
that case attached.

### D55 — State-driven activation runs inside `StateTransition`

**Decided.** A context whose activation follows a game state is synchronised inside Bevy's
`StateTransition`; a general run condition, having no transition to sit behind, is polled in
`PreUpdate` before evaluation. The state resource is read as an `Option`, because a substate or a
computed state may have none, and reading it unconditionally panics for a pause menu written as a
substate of playing.

**Rules out.** One placement for both activation paths.

**Reversal.** Bevy applies transitions *after* `PreUpdate`, so a condition polled there reads the
state before that frame's transition has been applied. The difference is invisible for render
contexts and real for the other two cases:

| | in `StateTransition` | in `PreUpdate` |
| --- | --- | --- |
| a render context's next evaluation | frame N+1 | frame N+1 — no difference |
| a fixed context's next evaluation | frame N | frame N+1 |
| what an `OnEnter` system sees | already in step | still the old answer |

### D56 — Activation answers per context type, and is declared on the builder

**Decided.** A run condition decides whether a context is live, answering once for the whole context
type. It is declared on the builder beside the bindings it governs, not by a variant of the call
that declares the context. Per-instance activation stays a method on the instance.

**Rules out.** A method per activation policy on the app extension trait, and binding activation to
the entity so that two instances of one context can follow different conditions.

**Reversal.** The extension trait grows a method per policy, focus-driven activation already a
fourth. Per-entity activation, `bevy_enhanced_input`'s choice, lets two instances follow different
states, at the cost of two places to get right; declared on the builder, activation cannot be
half-declared.

**Accepted cost.** Mixing a condition with per-instance activation means the condition wins every
frame. That is documented rather than prevented, since preventing it would mean tracking which door
an activation came through.

### D57 — Where two pads report one axis, the one that moved last speaks

**Decided.** A context instance keys its held gamepad state by control rather than by device. Where
two pads drive one context and both report the same axis, the most recent reading is the one that
stands.

**Rules out.** A per-device map of held state in every context instance.

**Reversal.** A per-device map in every instance, for the merge alone: per-unit calibration does not
need one, since it is applied where the raw message still names its sender, before held state exists
(D20).

**Accepted cost.** On an *unpaired* context driven by two pads, a still-held stick on the second pad
reads zero until it next moves, and a disconnect clears every pad's readings rather than that one's.
A paired instance reads one device and never sees it. `leafwing-input-manager` takes the same
position, and its maintainer reports no complaint.

### D61 — A gamepad stick is a `Control`, named whole

**Decided.** `Control::GamepadStick(Stick)` reports `ChannelShape::Axis2`, as `MouseMotion` reports
`Delta2`, and `ControlClass::AnyStick` makes `ControlClass::of` total. A stick is admissible,
capturable and rebindable as the mouse is: `part`, `set_part` and `arrival` resolve a stick push to
this one control, never to one of its axes. Consumption stays split: a stick binding decomposes into
its two `GamepadAxis` atoms, which `ConsumedControls` and reservation key on, so
`Control::GamepadStick`, the one `Control` naming what others name in part, is never a claim.

**Rules out.** Sticks as a device class with no per-mapping rebinding, moved only by presets.
`admissible` then refuses every control against an `Axis2` row.

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

**Reversal.** Everything taking a bare `ControlClass` fails on the variant: a capture accepting it
refuses every key and never ends, a prompt scope narrowed to it comes back empty, and `contains`
carries a variant it can never say yes to.

**Measured, not documented.** The filter's shape comes from `examples/ime_diagnostic.rs` on macOS: a
kana source delivers each keystroke as its own `Pressed` with `text: Some(...)`, and no `Pressed`
carries `text: None` mid-composition. A dead key (Option+I then A) composes to one character through
Bevy's text-input example. Committing a multi-candidate kana-to-kanji conversion from an IME popup
is unmeasured.

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

**Reversal.** Cheap: only the capture default is at stake, and an author wanting logical rows
declares them (R12.1). Against logical capture: at the moment of capture position and character name
the same key, so there is no ambiguity to resolve; and after a layout change a player switching
scripts (US to Cyrillic, the common case) keeps working positional bindings where logical ones
break. Blender resolves letters logically and digits physically, which AZERTY reaches only with
shift; the result is a long-running bug, an add-on written to undo it, and users who believe it does
the opposite of what it does.

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

**Decided.** An action's events report what the action did, and carry no entity it was about. A game
binds one `Activate` in a context active while a widget has focus, and the observer acts on
whichever entity `InputFocus` names. A game keeping state from `Fired` to the paired `Completed` or
`Canceled`, such as a pressed highlight, records the entity at `Fired` and addresses the paired
event to it: read twice, focus that moved in between leaves the first button highlighted with no
event to clear it. `widget_focus.rs` finishes at `Fired` and has no second read.

**Rules out.** A schedule ordering that resolves focus before evaluation (R22.11, withdrawn); a
target the crate supplies alongside the event; anything focus-shaped in this crate's own surface.

**Reversal.** The crate cannot tell that a target exists: `active_if` takes any run condition, and
one reading `InputFocus` looks like one reading the clock. Supplying one means an activation
condition grows a declared subject, a second activation mechanism beside the one contexts have.

**Accepted cost.** Small, and presentation only. A pointer can slide off a held button to take the
press back, which is why a mouse button activates on release; focus jumps instead, so a focus-driven
activation settles at `Fired`. What crosses the pair is a highlight, and removing it from the entity
that got it needs nothing about the widget.

### D88 — A claim names whose input it is

**Decided.** A consumption claim, and an entry in the exclusion ceiling, carry the devices of the
instance that made them, and a reader sees a claim only where its own devices intersect them:
`ConsumedControls::contains` and `claimant` take the reader's devices. One player's menu consuming
`South` leaves another player's gameplay free to read it, two instances of one context on two pads
do not take controls from each other, and one player's pause menu does not deactivate the other's
game. A live capture claims under its session's pairing. The devices travel as `DeviceHandleSet`,
not `Paired`, which is a component; a caller holding a `Paired` derefs.

**No pairing and an empty pairing differ.** No pairing is every device: a single-player context
claims against everyone, with no opt-in. An empty pairing is no device, a player spawned before a
device reaches it: it hears nothing, claims nothing and shadows nobody. Reading empty as
unconstrained is harmless in the evaluator and wrong in `why_not`, which would name a claimant where
the answer is `Unowned`.

**Rules out.** One priority ceiling for the world; a claim keyed by control alone;
`ConsumedControls` as a map, since a lookup matches a control *and* an overlapping device set and no
single key expresses both.

**Reversal.** A world-wide table puts every player behind every other player's menu, and changes
`contains` and `claimant`, which are public.

### D89 — A capture reports what it took, and the store judges it

**Decided.** `CaptureSession` carries a class and an exclusion list, no target (no mapping, no
slot), and makes no judgement about admissibility. A deliberate press ends the capture and is
reported, whatever it was; whether it may be stored is asked once, at the write, through
`Rebind::checked`, the predicate a save file meets when applied (D42). Capture still claims
everything it takes, including a control the row cannot hold, so the settings key pressed at a
rebinding screen neither binds nor re-opens the screen.

A refusal ends the session, as Blender's does, so the screen can say why while the player is still
looking at the cell. An arrival nobody chose must then not end a capture, so a deliberate press
always answers and a continuous reading past its threshold answers only a session listening for that
class; otherwise a pad drifting on a desk cancels every keyboard rebind.

**Rules out.**

- **A target on the session or on `ControlCaptured`.** The screen already correlates the answer
  through the entity the event fires on, because it must know which cell is listening before the
  answer arrives, so a second copy is one nobody reads.
- **Judging at listen time**, by a constructor that refuses a row or by a refusal event. The write
  must ask the row length and chordability as well, which listen time cannot see, so the write is a
  strict superset and an earlier answer can only disagree with it.
- **Leaving a refused session listening.** The row is then accepted into a working copy and turned
  down at Confirm, minutes later.

**Reversal.** The listen-time policy returns beside the write-time one and can disagree with it, and
`for_mapping` becomes fallible for a reason unrelated to the control it listens for.

**Accepted cost.** A screen needs a line for the refusal's reason. One that wants silence drops the
`Err` arm.

### D99 — A capture answers on the release, and claims the control until then

**Decided.** A session holds the control it has taken and re-claims it every frame until the
release, and answers then, so the control is already up when the claim stops. Consumption applies
where held state is read, and held state outlives an instant claim: a session answering on the press
and vanishing left the control down and unclaimed, and the context underneath read it the next
frame. Observed: a capture on a settings screen took the arrow key _and_ moved the selection,
through the menu's navigation composite.

**Rules out.** A claim that outlives the session, which would need an owner with a lifetime of its
own; `ConsumedControls` learning about releases, which would put a level concept in a per-schedule
claim table.

**Reversal.** The observed defect returns for every control the game underneath binds.

**Accepted cost.** A second press while one is held is ignored rather than latched, and a test
capturing a press has to release it. Mouse motion still answers at once: a displacement is never
held.

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
