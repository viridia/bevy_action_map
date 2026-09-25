# How `bevy_action_map` works

An input-mapping crate for Bevy. It turns device messages into named game actions, and turns the
same declarations into the data a settings screen and a control prompt are drawn from.

This document describes what is built. It follows the path a keypress takes: into the frame, through
a compiled plan, out as action state, and back out again as the strings a player reads. It does not
argue for any of it.

---

## 1. Architecture

Four layers. The structural property everything else rests on is that **L2 reads only L1** —
everything downstream of the input frame is a pure function of it.

```
L0  device      device families, raw messages from keyboard, mouse and gamepad, per-unit calibration
L1  frame       one timestamped queue of raw events, sampled once per rendered frame
L2  mapping     compiled plans, evaluation, per-entity action state
L3  consumers   polling, observers, mappings, prompts, capture, overrides
```

`ActionMapPlugin` installs the whole pipeline and orders it through four system sets:

| Set | Schedule | Does |
| --- | --- | --- |
| `Sample` | `PreUpdate`, after Bevy's own input systems | drains Bevy's message streams into `InputFrame` |
| `Capture` | `PreUpdate`, after `Sample` | lets a live rebinding session take a control before any context acts on it |
| `Evaluate` | `PreUpdate` (render contexts), `FixedPreUpdate` (fixed contexts) | maps the frame onto action state |
| `Dispatch` | after `Evaluate` in the same schedule | drains the transition log to observers |

The plugin also adds `InputFramePlugin` and Bevy's `TimePlugin` if they are not already present, and
initializes `ButtonThreshold`, `ConsumedControls`, `ExclusionCeiling`, `ReservedControls` and
`PromptGeneration`.

---

## 2. The input frame

`InputFrame` is a resource holding one ordered queue of raw events.

```rust
pub struct FrameTimestamp { frame: u64, order: u32 }

pub enum RawEvent {
    Keyboard(KeyboardInput),
    MouseButton(MouseButtonInput),
    MouseMotion(Vec2),
    Gamepad(RawGamepadEvent),
    FocusLost,
}
```

A timestamp is a sequence position, not a wall clock: a frame counter and the event's order within
that frame. Bevy's input events carry no time of their own, so `sample_input` stamps them as it
records them. Order is preserved; the true instants are not, and every event sampled in one frame
compares equal on the only axis a time window could split.

`RawEvent::control()` gives the single physical `Control` an event reports on, or `None` for a
gamepad connection event and for `FocusLost`. `RawEvent::device()` gives the `DeviceHandle` that
sent it.

**Reading.** Consumers read by cursor rather than by window: `events_after(cursor)` returns
everything sampled after the timestamp the caller last saw. Each context instance keeps its own
cursor, so a press and release inside one rendered frame both sit in the queue and a fixed tick
spanning them sees both.

**Retirement.** `retire_read_events` clears the queue in `FixedPreUpdate`, after fixed-tick
evaluation — the one point at which every consumer is known to have read. Render contexts drained
earlier in the same frame; fixed contexts have just drained. If the simulation does not step that
frame nothing is retired, and the cursors are what stop a render context re-reading what it already
acted on. The queue is capped at 4096 events, dropping oldest-first and counting the drops in
`InputFrame::dropped()`.

**Calibration.** Gamepad axis values are corrected as they are recorded, not where they are read
(TD8.4). A backend writing values into the frame directly enters past that point.

---

## 3. Actions, contexts and values

An action is a type, not a value.

```rust
pub trait InputAction: Send + Sync + 'static {
    type Output: ActionOutput;      // bool, f32, Vec2, Vec3
    const INTENT: ActionIntent;
    const PATH: &'static str;       // "gameplay.jump"
    fn id() -> ActionId;
    // plus CATEGORY and CONSUME, with defaults
}

pub trait InputContext: Send + Sync + 'static {
    const TICK: TickDomain;         // Render | Fixed
    const PRIORITY: i32;
    const EXCLUSIVE: bool = false;
    const PATH: &'static str;
}
```

Both are written with derives:

```rust
#[derive(InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
struct Jump;

#[derive(InputContext)]
#[context(path = "ui.settings", tick = Render, priority = 10, exclusive)]
struct Settings;
```

`#[derive(InputContext)]` additionally emits `Component`, `Default`, `Clone` and `Copy`, so
spawning an entity with the context type is all it takes to give that entity live input.

### 3.1 Shape, intent and channel

Three related properties, deliberately distinct.

| | Belongs to | Values |
| --- | --- | --- |
| **Output** | the action's Rust type | `bool`, `f32`, `Vec2`, `Vec3` |
| **`ActionIntent`** | the action's meaning | `Button`, `Analog1`, `Directional2`, `Delta2` |
| **`ChannelShape`** | the control's report | `Button`, `Axis1`, `Axis2`, `Delta2` |

A stick deflection and a mouse delta are both `Vec2`; `Directional2` is a position implying a rate,
and `Delta2` is a displacement that already happened. `ActionIntent::accepts` decides which channel
shapes can serve which intent, and a binding whose channel cannot serve its action's intent is
refused when the context is declared. The derive checks output against intent in a compile-time
assertion.

A directional **composite** is four buttons written as one `bind` call and declared as four bindings
(TD8.1), each part reporting `Axis2` as the unit vector it pushes, which the fold of TD5.5 turns
back into one direction. A gamepad stick is a single `Control::GamepadStick` reporting a position
the same way `MouseMotion` reports a displacement.

### 3.2 Runtime values

```rust
pub enum ActionValue { Bool(bool), Axis1(f32), Axis2(Vec2), Axis3(Vec3) }

pub enum ActionPhase { Idle, Started, Building, Fired, Firing, Completed, Canceled }

pub struct ActionState { pub value: ActionValue, pub phase: ActionPhase }
```

A gerund or adjective is a level, still true next tick (`Idle`, `Building`, `Firing`); a past
participle is an edge, true for one tick only (`Started`, `Fired`, `Completed`, `Canceled`).
`Started` is a condition that has begun without being satisfied — a hold part way through —
and `Building` is the same condition on every tick after that while it keeps not being satisfied.
`Firing` is the equivalent for an action already active: `Fired` is the tick it became so, `Firing`
every tick after that it stays so. `Canceled` is a condition abandoned or an action interrupted;
`Completed` is an ordinary release.

### 3.3 Paths

`PATH` is required on every action and context, and it is the string a settings file stores. It does
not have to match the Rust type name and should not be updated to follow one.

```
<namespace>.<name>          gameplay.jump    menu.confirm    vehicle.flight.throttle
```

Separator is `.`; case is `snake_case`; at least two segments, a namespace and a name. For a library
the namespace is its crate name. `ActionId` is a dense `u32` interned at registration, stable within
a process run: it is the position of the action's entry in the registry, so `info` is a subscript
and `from_path` a scan, and `registered_actions()` lists them. An action's own value converts into
its id, so a `bsn!` field holding an `ActionId` can name the action bare. Such a field holds
`ActionId::PLACEHOLDER` until it does — an id the registry never hands out, so it is bound nowhere
and has no `info`.

---

## 4. The compiled plan

Bindings are authored as data and compiled once per context into a `Plan`.

| Compilation produces | Serves |
| --- | --- |
| action → slot assignment, and the reverse as a direct index | O(1) state access without hashing |
| scratch slot assignment per condition and stateful modifier | TD6 |
| each action's stage after the fold, its scratch placed after every binding's | TD5.5 |
| each binding's chord length, and whether the plan has any | the clash pre-pass, skipped when there are no chords |
| the set of controls any binding indexes | class-binding fallback, TD5.4 |
| resolved dispatch per slot | turning a transition into a typed event |
| diagnostics | reported before the context is installed |

The plan is immutable and `Arc`-shared between every instance of that context.

**Two per-context resources hold plans.** `InputContextPlan<C>` holds what the game declared and is
never rewritten. `AppliedPlan<C>` holds the variant compiled from an override set, and is what an
instance spawned later reads. Its presence is the answer to "has anything been overridden here".

A variant keeps the declared plan's slot allocation, so an action whose every binding the player
cleared keeps its slot and reads as at rest rather than as unbound, and an instance's action states
and require-reset flags stay aligned across the swap. Only the scratch is rebuilt.

**Diagnostics** are produced by a separate pass from compilation, so a rebinding UI can ask about
bindings it has no intention of installing. `add_context` runs it first and refuses the context
rather than compiling a plan that cannot work. A composite's parts share every combinator, so a
diagnostic repeated across the parts of one `bind` call is reported once.

```rust
pub enum Severity { Error, Warning }

pub enum DiagnosticKind {
    IntentMismatch { .. }, RateFromDelta { .. }, ChainedRescaling { .. },
    DuplicateBinding { .. }, ConsumeDisagreement { .. },
    DuplicateMappingKey { .. }, RebindingDisagreement { .. },
    ReservedAndMappable, FollowsNothing { .. }, FollowsUnlisted { .. },
    DuplicateClassBinding { .. }, DuplicateTunableKey { .. },
    TunableShapeDisagreement { .. }, DeadZoneAtFullDeflection { .. },
    BoundAndDelegated, DeltaFromAuthority, CombinedWithoutBindings,
}
```

---

## 5. Evaluation

One pass per context per tick: raw events in, action state and a transition log out.

```
raw event, one at a time
  → held-state update
  → chord clash pre-pass  once per fold, if the plan has chords
  → shared toggle latch   once per fold, per shared hold_or_toggle key
  → per binding
      read the control    a consumed control reads as untouched
      modifier chain      negate · swizzle · scale · dead zone · curve · clamp · compass · custom
      press threshold     a Button action fed by a non-bool value, remembered per binding
      conditions          explicit: any satisfies · implicit: all hold · blocking: any vetoes
      consume?            Building or Satisfied adds its controls to the instance's claims
  → fold by intent        several bindings, one action
  → stage after fold      modifiers · conditions, declared once per action
  → write ActionState, mark dirty, append transition
  → class dispatch        the event itself, if no plain binding indexes its control
```

Level events are replayed one at a time, each followed by a whole fold, so a press and a release
inside one tick are two readings rather than none. A tick with no level event still folds once, so
time-driven conditions advance. Mouse motion is summed across the tick and read by one further fold
at the end, in which only `Delta2` actions take part. An inactive context applies the held-state
update and nothing after it.

Claims gather in a list local to the instance and reach `ConsumedControls` once it has finished, so
an instance never reads back its own claim partway through a tick.

### 5.1 Arbitration

The sort a single global binding list would do is resolved in two places instead, each where it can
be.

**Context priority is system ordering.** `PRIORITY` is a const, so at app build each distinct
priority gets its own `EvaluateAt(i32)` system set, ordered against every other priority already
declared in that schedule. Higher priorities run first and claim controls before anyone else reads
them. Nothing is decided per frame.

Two contexts declared at the same priority — the default, if neither says otherwise — still need
an answer for who claims a control they both bind. Each gets its own set nested inside their
shared `EvaluateAt`, ordered after the one declared before it, so `add_context` call order is the
tiebreak. Registration order becoming meaningful this way is easy to miss, since Bevy plugins are
usually independent of each other's ordering: two contexts at the same priority are the one place
here that they are not.

**Chord length is a pre-pass inside one evaluation.** Whether a chord is satisfied is a pure
function of what is held, so before any binding is read the longest satisfied chord on each control
is found. A binding shorter than the winner on any of its controls reads as at rest. `Ctrl+S` beats
a bare `S` with nothing declared for it.

**A chord entry is a control or a modifier.** `ChordEntry::Control` names one control and is
satisfied by that control alone. `ChordEntry::Modifier` names one of `ModifierKey`'s four pairs and
is satisfied by either key of it, so a chord entry costs up to two lookups rather than one. The
disjunction is resolved at read time rather than expanded at bind time, because both keys of a pair
are live at once: expanding would duplicate the binding rather than the entry, and duplicate the
mapping row with it. Length counts entries, so a modifier contributes one however many keys satisfy
it.

A modifier reaches presentation as `ControlOrigin::Modifier`, which is the only variant that answers
`None` to `control()` — it stands for two controls, and a caption naming one of them would tell the
player the other will not do.

### 5.2 Consumption

A binding declared `consume` records its controls in `ConsumedControls` while its conditions report
`Building` or `Satisfied`, and contexts evaluating later see those controls as untouched.
`ConsumedControls::claimant` names the context that took one. A stick is recorded as its two axes,
whether a context's binding or a capture claimed it, so a stick binding reads zero on each axis that
was taken and keeps the other.

A claim also records the devices the claiming instance reads, taken from its `Paired`, and a reader
sees it only where the two device sets intersect (R15.3). So one player's menu consuming `South`
leaves another player's gameplay context free to read it, and two instances of one context paired to
two pads do not take controls from each other. A context with no `Paired` reads every device, which
makes its claims reach everyone and everyone's claims reach it — the single-player case, with no
opt-in. A `Paired` holding no device is the opposite, and the difference matters: it hears nothing,
so it claims nothing and no claim reaches it, which is why `why_not` answers `Unowned` there rather
than naming a claimant. `contains` and `claimant` therefore take the reader's devices; a live
capture claims under its own session's `Paired` for the same reason, so two rebinding panes do not
answer each other's presses.

Consumption is recorded per schedule and cleared at two points:

| | |
| --- | --- |
| `PreUpdate` consumes a control | every fixed tick that frame sees it as taken |
| a fixed tick consumes a control | the next fixed tick does not — each run decides afresh |
| no fixed tick runs | nothing is left behind; the next frame's clear runs regardless |

Consumption therefore flows forward in schedule order: a render-tick context can take a control from
a fixed-tick one, and not the reverse.

### 5.3 Exclusive contexts

A context declared `exclusive` treats every lower-priority context as inactive for as long as it is
active. `ExclusionCeiling` holds one entry per active exclusive instance — its priority and its
devices — and each context's activation system, already running in priority order, checks the
ceiling before trusting its own condition: below an entry it shares a device with, the context is
*shadowed* instead. Scoped by devices for the same reason a claim is, and read per instance rather
than per context type, so one player opening a pause menu does not deactivate the other player's
gameplay.

Shadowing cancels in-flight actions and re-arms require-reset, exactly as deactivation does. It is
tracked in a field of its own (`shadowed`) rather than by clearing `active`, so a context's own
condition and what is being forced on it do not overwrite each other. `is_active` is the
conjunction.

The ceiling is set by the `PreUpdate` pass and read — never rewritten — by every `FixedPreUpdate`
run that frame, then cleared at the top of the next frame alongside the consumption release. An
exclusive context re-asserts the same entry on every run, so a run that adds nothing new leaves the
list as it found it.

### 5.4 Class bindings

A class binding names a class of controls rather than one control. `bind_class` takes a
`ControlClass`:

```rust
pub enum ControlClass { AnyButton, AnyAxis, AnyStick, AnyDelta }
```

`bind_characters` is the other door, for keys that produce text rather than a fixed shape — a
focused text field's own case. Membership there is a property of the *event* a control produced, not
of the control's identity: the same key is a dead key on one press and a plain letter on the next, so
it is a separate method rather than a fourth `ControlClass` variant.

The plan carries both kinds as a second, separate list. Evaluation consults it only for an event on a
control no plain binding in that context indexes. What is dispatched is the original `RawEvent`, not
a folded value — there is no lifecycle to fold it into — as a `ClassFired<A>` event on the context
entity.

### 5.5 Folding several bindings into one action

State is allocated per action, so several bindings feeding one action have to be folded into one
value. The action's `ActionIntent` decides the rule:

| Intent | Fold |
| --- | --- |
| `Button` | strongest contribution wins |
| `Analog1`, `Directional2` | per axis, the strongest positive contribution plus the strongest negative one |
| `Delta2` | contributions are summed |

Contributions are widened to three components and split by sign on each, into two accumulators that
are locals of the loop. `Button` contributions are always `Bool` by then, so its per-sign maximum is
strongest-wins, and `Delta2` adds into the same two. The result takes the widest shape any
contribution had, so neither the value nor its shape depends on declaration order. The fold does not
clamp: no axis can exceed its strongest contributor, and a diagonal's length is the stage's
business.

**The stage after the fold.** `combined::<A>()` declares a modifier chain and conditions that run on
the folded value rather than on one binding's. It is held per slot, so an action that declares
nothing costs one emptiness check, and it sits after every binding in the scratch layout, so a
variant whose bindings shrank moves it down rather than rebuilding it. Its conditions replace the
bindings' combined state, with one exception: a binding still `Building` keeps the action there when
the stage alone would read `Idle`, since a hold in progress contributes rest to the fold. A stage on
an action with no bindings in the context is refused, and a rescale in it counts against the most
any one of the action's bindings already did.

### 5.6 Transitions and observers

Evaluation writes state and appends to a transition log. A separate system in the `Dispatch` set
drains that log, so nothing user-defined runs inside the evaluator.

```rust
#[derive(EntityEvent)]
pub struct Fired<A: InputAction> { #[event_target] pub context: Entity, pub value: A::Output }
// likewise Started<A>, Completed<A>, Canceled<A>, and ClassFired<A> for a class binding
```

Because the log records transitions rather than final state, an action that fires and completes
within one tick produces two entries and two observer invocations, in order. Cost is proportional to
transitions, so an idle frame dispatches nothing.

Events target the entity carrying the context, which makes them usable from `bsn!` through
`bevy_scene`'s `on()` with no adapter and no feature:

```rust
bsn! {
    OnFoot
    on(|ev: On<Fired<Jump>>, mut q: Query<&mut Velocity>| {
        q.get_mut(ev.context).unwrap().y = JUMP_SPEED;
    })
}
```

### 5.7 Interruption

A window losing focus (`RawEvent::FocusLost`) or a gamepad disconnecting cancels whatever was in
flight on that source — `Canceled`, not the `Completed` an ordinary release produces. A binding on
an unaffected device is untouched. An authority that stops supplying an action is the same case
(TD5.8).

### 5.8 Authority bindings

`controls.bind::<A>(Authority(family))` binds an action to the value an outside authority supplies
in place of one device family's controls. Whatever owns the authority — a platform input service, a
network peer — writes into an `AuthorityValues` component on the context entity from a system
ordered before evaluation.

The binding holds no control. Its input is `BindingInput::Authority(family, shape, source)`, where
`source` is the action whose value it reads and the shape is that action's own. `Authority` is
written before the action it binds is known, so it carries placeholders and `push_binding` stamps
both; D7's check runs on the shape unchanged. A follower copies its leader's input already stamped
and keeps it, so it reads the leader's value, and `leader_of` matches the two by input as it would
two bindings of one control. A `Delta2` action refuses one (`DeltaFromAuthority`), since a delta is
counted once and a level is read on every fold.

The value is held state, as a control's is. Once a tick, before `apply_frame`, an active instance
copies the entity's `AuthorityValues` into its own (`sample_authority`), and the fold's `Authority`
arm reads the binding's source from that copy, or rest where nothing was written. Before copying, a
binding whose source was absent from the old copy and is held in the new one is marked
`require_reset`, which gives R7.5's hold-over to an instance's first sample and to an action the
authority resumes supplying, a Steam action set changing under a held button among them. The mirror
case, a source present in the old copy and absent from the new, sets `authority_lost`, and the next
`apply_frame` folds once as `Fold::Interrupted` before replaying its events, as a disconnect event
would be folded: an action that was firing and now reads at rest is `Canceled`, and one another
binding still holds is untouched. From there it is an ordinary binding: its modifiers and conditions
run, it folds with the action's other bindings by intent (TD5.5), and `commit_slot` turns its level
into `Fired`, `Firing` and `Completed`. An authority reporting only a level, sampled when asked, is
what this is shaped for; nothing on the wire or in the platform API needs to carry an edge.

Holding no control, it claims nothing and cannot be claimed, has no mapping row, and has no prompt
of its own, since the backend's `Prompts` answer for its family. An inactive or shadowed context
does not sample it. Interruption does not reach it either: a window losing focus is this crate's
device going away, and the authority's has not.

The one contradiction is a control of the authority's own family bound to the same action, which is
`BoundAndDelegated` and a plan-build error. Controls of other families beside it are what the split
is for.

---

## 6. State and storage

State divides in two, and both halves are dense arrays of `Copy` types indexed by plan slot.

| | Holds | Belongs to |
| --- | --- | --- |
| `ActionState` | value, phase | the action |
| `Scratch` | hold timers, tap counts, previous value, flags | the binding's conditions and modifiers, and a `Button` action's derived press |

```rust
pub struct Scratch {
    pub prev: ActionValue,   // previous input, or a filter accumulator
    pub time: f32,           // press time, window start, or last fire
    pub count: u16,          // tap count, or progress through a sequence
    pub flags: u8,
}
```

Durations, thresholds and every other parameter live in the immutable plan, never here, which is
what keeps the shape uniform across every built-in condition and stateful modifier.

`InputContextState<C>` is the component on the context entity, and holds:

- `plan: Arc<Plan>`, and the two tables above;
- `dirty`, a per-action bitset — evaluation writes through a bypassed borrow and re-marks the
  component only where a bit was set, so a tick that changed nothing is invisible to a subscriber;
- `active` and `shadowed`;
- `tunable_scratch`, one cell per group of bindings sharing a tunable, rather than a private slot
  per binding;
- `chord_claims`, reused between folds;
- `require_reset` and `disabled`, parallel to the action table;
- `transitions` and `class_fires`, appended by evaluation and drained by dispatch;
- `read_through`, this instance's frame cursor, seeded at spawn so a context added mid-session
  starts from the present;
- held keyboard, mouse and gamepad state, behind their respective features.

The struct holds no ECS references, so a test or replay harness can drive one directly. Activation
flips a flag: no spawn, despawn, insert or remove.

---

## 7. Contexts

### 7.1 Declaring

```rust
app.add_context::<OnFoot>(|controls| {
    controls.bind::<Move>(DirectionalButtons::wasd()).mappable();
    controls.bind::<Jump>(KeyCode::Space).mappable();
    controls.bind::<Jump>(GamepadButton::South);
});
```

This compiles the bindings once and installs a component lifecycle hook, so any entity that later
carries `OnFoot` gets its state with no further registration. Because a hook can only be attached
while no entity carries the component yet, a context must be declared before anything spawns into
it; `add_context` panics if the plugin is missing, if a context is declared twice, or if an entity
already carries it.

Spawning is sufficient only once a context has reached this point: R22.14's promise that spawning
alone activates it, even from a scene, assumes `add_context` already ran for that type. A
`#[derive(InputContext)]` component spawned before that logs a warning instead of silently doing
nothing, since there is no plan yet to build state from and no other diagnostic — `dump` included —
can see a context nobody declared.

### 7.2 Activation

A declared context is live as soon as an entity carries it. `active_if` and `active_in_state` (the
latter behind the `state` feature) make activation follow something else; `activate`, `deactivate`
and `activate_including_held` drive it by hand.

Activation arms **require-reset**: a control the player was already holding does not read as a fresh
press, and must be released once first. The latch holds back `Button` actions only — an analog
action has no synthesized fire to guard against, and its value simply resumes.
`activate_including_held` skips the arming, which is what a context taking over from another
driving the same controls wants. Deactivation cancels whatever is in flight.

One action can be switched off on its own with `disable::<A>()`, and back on with `enable::<A>()`,
without unbinding it. Disabling cancels that action's in-flight state and takes its slot out of the
fold entirely, so it consumes nothing and out-ranks no chord; enabling arms require-reset for that
slot alone. `ActionObstacle::Disabled` reports it.

### 7.3 Reading

```rust
fn movement(input: ContextActions<OnFoot>) {
    let dir = input.value::<Move>();     // Vec2, checked at compile time
    if input.fired::<Jump>() { .. }
}
```

`ContextActions<C>` is for a context with exactly one instance; a system taking it is skipped when
there is no instance or several, following Bevy's `Single`. `ActionsQuery<C>` is the per-player form
— `get`, `iter`, `len`. Both expose `value`, `try_value`, `phase`, `fired`, `elapsed`, `progress`
and `why_not`.

`elapsed` and `progress` read a `hold` or `hold_and_release` binding's own timer back out — how long
its control has been down, and how far that is toward the duration it needs, clamped to `0.0..=1.0`.
Both are `0.0` where an action has no such binding, or none of its bindings is currently held; where
more than one qualifies, the one furthest along wins. Nothing new is tracked to answer this — the
timer is the same `Scratch` the binding's own condition already keeps between ticks, walked from
outside evaluation rather than threaded through it.

`why_not` answers the question a call site cannot:

```rust
pub enum ActionObstacle {
    None, Unbound, ContextInactive, AwaitingRelease,
    Consumed { control: Control, by: &'static str },
    Outranked { control: Control, chord: u8 },
    ConditionPending, Unowned, NoInput,
}
```

`why_not` and `why_not_id` also take the instance's `Paired`, if it has one: a binding on a device
outside that pairing reports `Unowned` rather than `NoInput`, so a context whose pairing is dropping
every event the player generates says so instead of looking like the player pressed nothing.

`InputContextState::iter` walks every action in a context as `ActionReading`s, and `inspect::dump`
produces a fully type-erased `InputDump` — contexts, instances and actions by path — for a debug
overlay or an editor reading a game it was not compiled against.

### 7.4 Device pairing

```rust
pub enum DeviceHandle { KeyboardMouse, Gamepad(Entity) }
```

Keyboard and mouse are one handle because a player uses them together; a gamepad is the backend's
own entity for it, which a reconnect reassigns and a save file must never compare. `DeviceHandleSet`
is a plain value type; `Paired` is the component that attaches one to a context entity.

`apply_frame` takes an optional `&Paired` and drops any event whose device the pairing does not
claim, before anything else sees it. A context with no `Paired` reads every device.

**Joining** needs no separate evaluation path. A game declares "join" as an ordinary action on a
context it spawns once per available device, each `Paired` to its own, so a press arrives on an
entity whose `Paired` names the presser. `join::is_claimed` tests a device against the world's
`Paired` set; it does not settle a race between two presses in one tick, because the `Paired`
inserts behind it are deferred commands, so a game decides that from state it updates
synchronously.

`ConsumedControls` and `ExclusionCeiling` are world-wide resources holding per-occupant entries: a
claim and an exclusion each carry the devices of the instance that made them, and reach only a
reader sharing one (TD5.2, TD5.3).

**Which device is "the" one for a prompt is the app's call**, on the same terms as `PromptDevice`
(TD9.2): a player with two gamepads paired is an edge case nothing in this crate ranks, and a game
that cares reaches for `Paired`'s own devices and its own notion of which is current.

### 7.5 Connection signals

```rust
pub struct DeviceDisconnected { pub entity: Entity, pub device: DeviceHandle }
pub struct DeviceConnected { pub device: DeviceHandle }
```

A gamepad's connection state reaches the crate the same way a button press does —
`RawGamepadEvent::Connection`, sampled into the frame before anything else sees it — so both fire
under a backend synthesizing that event exactly as they do under `gilrs`.

`DeviceDisconnected` triggers on whichever `Paired` entity's device set named the device that went
away. `Paired` is left untouched: keeping the slot open for a reconnect, or tearing the pairing
down, is the app's decision. `DeviceConnected` triggers once per gamepad that becomes available and
names no entity — the crate cannot tell a fresh join from an existing pairing's own device coming
back, so it does not choose between them.

Both answer "which player lost a device", and neither enumerates. Because `DeviceDisconnected` is
entity-targeted it says nothing at all about a device no `Paired` holds, which is the device a join
screen is waiting on. That question is `ConnectedGamepad`'s:

```rust
pub struct ConnectedGamepad;   // component, no payload
```

The entity carrying it is the one `DeviceHandle::Gamepad` is built from, so `Query<Entity,
With<ConnectedGamepad>>` is the set of gamepads available right now and `Add`/`Remove` on it is
their arrival and departure — symmetric, and covering claimed and unclaimed alike. Bevy's gamepad
backend gets the marker attached by observers mirroring `Add<Gamepad>` and `Remove<Gamepad>`; any
other backend inserts it on the entities it spawns.

It is not Bevy's `Gamepad`, which holds a pad's live button and axis readings behind private fields
that only raw device messages fill. A backend supplying resolved actions has no such readings, so
a pad it supplies would carry an empty one and read as untouched. The keyboard has no entry here:
it never connects or disconnects, so `DeviceHandle::KeyboardMouse` is available unconditionally.

### 7.6 Brand resolution

```rust
pub enum GamepadBrand { Xbox, PlayStation, Nintendo, Generic }
pub struct GamepadBrands { /* vendor_id -> GamepadBrand */ }
pub struct Brand(pub GamepadBrand);
```

A device's brand is a fact about that one gamepad, resolved from its `vendor_id` — which is
`Option` and often absent, so `Generic` is the ordinary answer for an unrecognized or unreported
pad, not an error. `GamepadBrands` is a resource seeded with the three current-generation console
makers' USB vendor ids and `init_resource`'d by `InputFramePlugin`; `insert` is the app-overridable
mapping for hardware this crate does not ship pre-resolved.

`Brand` is the resolved answer, attached once to the gamepad's own entity by an observer on
`Add<Gamepad>` rather than re-derived at each read site. A read path holding that entity — from a
`DeviceHandle::Gamepad`, say — queries `&Brand` directly; an entity a backend other than Bevy's
gamepad backend spawned works the same way as long as that backend inserts the same component. The
observer skips an entity that already carries `Brand`, so a game that knows a specific pad's brand
ahead of the vendor id table can insert it first and have the observer leave it alone.

Current-generation controllers only: DualSense, the Xbox Series pad, the Switch Pro Controller and
Joy-Con. A brand alone cannot tell one console generation from another — Xbox 360's "Back"/"Start"
became Xbox One's "View"/"Menu", and PS4's "Share" became PS5's "Create" — so nothing here tracks
an older pad's own labels.

### 7.7 Persistent identity

```rust
pub trait DeviceIdentity: Reflect + FromReflect + GetTypeRegistration
                        + Clone + Eq + Hash + Debug {
    const DOMAIN: &'static str;
}
pub struct DeviceId { /* a boxed DeviceIdentity, plus its operations */ }
pub struct Identity(pub DeviceId);
pub struct GamepadModelId { pub vendor: u16, pub product: u16 }
```

A `DeviceHandle` names a device plugged in now; a `DeviceId` names the device itself. What counts as
identity belongs to the backend, so the payload is the backend's own type and nothing in the crate
inspects it. A backend declares one by implementing `DeviceIdentity` and calling
`App::register_device_identity::<T>()`.

`DOMAIN` is the name the identity is stored under, declared rather than derived from the Rust type
path, on the same footing as an action's `PATH` (D6). It is also the discriminator: two identities
whose domains differ are never equal, and the domain is hashed alongside the payload, so one
backend can never answer for another's device.

`Clone`, `Eq` and `Hash` come from the trait bounds and are captured as function pointers when the
`DeviceId` is built, not from `reflect_clone`/`reflect_partial_eq`/`reflect_hash`. The derive
special-cases those three into the type's own impl rather than storing them as type data, so their
absence is undetectable at registration and would surface as a panic at the first use. Taken from
the bounds, the same omission fails to compile.

`DeviceId` is opaque to reflection — nothing implements `PartialReflect` for a boxed trait object —
and carries `SerializeWithRegistry`/`DeserializeWithRegistry` instead. That pair is the only route
to the `TypeRegistry` from inside a field whose serializer writes no type information, which is what
lets a settings group hold one. The stored form is a single entry keyed by the domain, with the
payload written by the typed serializer. A payload written field by field cannot be read by a
release that has added or removed a field; a backend expecting its type to change gives it
`#[reflect(Serialize, Deserialize)]` and owns the stored form, which is also the only way to store a
value above `i64::MAX` in a format whose integers are `i64`.

`SavedDeviceId` is an identity slot that may be empty, for a settings field that has to be writable
before a player has picked up anything. It stores as the identity's own single entry, or as an empty
table. It exists because a reflected `Option` writes its empty case as `none`, which TOML cannot
spell — a settings crate writing TOML fails on the whole file rather than leaving the field out, so
`Option<DeviceId>` is the wrong type for anything persisted.

`Identity` is the resolved answer on the device's own entity, attached by an observer on
`Add<Gamepad>` on the same terms as `Brand` — skipped when one is already present, so a backend that
knows a device better than its USB ids do can insert its own first. `GamepadModelId` is what Bevy's
gamepad backend can report, and it names a *model*: identical controllers report identical ids, so a
match is a candidate rather than a proof. A pad whose platform reports neither id simply has no
`Identity`. The signal ordering apps depend on holds by construction: `sample_input` runs
`.after(InputSystems)`, and the sync point that implies is where Bevy's deferred `Gamepad` insert
lands and where the observer runs — so `Identity` is attached before `DeviceConnected` fires.

---

## 8. Bindings

### 8.1 Controls and inputs

```rust
pub enum Control { PhysicalKey(KeyCode), LogicalKey(char), MouseButton(MouseButton),
                   GamepadButton(GamepadButton), GamepadAxis(GamepadAxis), GamepadStick(Stick),
                   MouseMotion }
```

A keyboard key is nameable two ways, and the choice is the caller's (R12.1). `PhysicalKey` names a
position and `LogicalKey(char)` names the character the player's layout produces there, so
`LogicalKey('z')` is the Z key on QWERTY and the W key on AZERTY. The evaluator keeps a second held
map, `held_characters`, keyed by `KeyCode` and holding the character each held key reported — keyed
by position so that a release always finds its press, since pressing shift mid-hold changes the
character the platform reports. A key qualifies only when its `logical_key` is a `Key::Character` of
exactly one `char`, which excludes dead keys, IME compositions and the named variants; the character
is lowercased on the way in, since a capital is shift's doing rather than a key of its own. Case
folding aside, nothing about a logical control is layout-aware at read time: the frame already
carries `logical_key`, and this reads it.

A `BindingInput` is what one binding reads, and a `BindingPart` names which piece of an arrangement
it is:

```rust
pub enum BindingPart { Whole, Negative, Positive, Up, Down, Left, Right }
```

`AxisButtons` makes a bipolar axis from two buttons and `DirectionalButtons` a direction from four
(`DirectionalButtons::wasd()` is the named case), but neither reaches the plan. They are authoring
shorthand: `IntoBindingInput` yields one input per value, so `bind` declares one binding per part,
each a `BindingInput::Part(ButtonControl, BindingPart)` that contributes its part's unit vector
while held. Every combinator chained onto that `bind` applies to each part, and a tunable declared
there is one tunable, because the parts share its key. `Stick` and `MouseMove` are the analog
inputs, and both read as `BindingPart::Whole` — a stick has no part a player rebinds one of.
`GamepadStick` is `Control`'s only member naming what another one of its members names in part: a
whole stick, for presentation, override application and capture, reporting `ChannelShape::Axis2` the
way `MouseMotion` reports `Delta2`. Consumption does not follow it —
`BindingInput::for_each_control` still decomposes a stick binding into its two `GamepadAxis` atoms,
which is the granularity `ConsumedControls` and reservation key on. `Control::family()` and
`Control::shape()` classify one control, and `BindingInput::channel_shape` classifies an
arrangement.

### 8.2 The builder

`bind::<A>(input)` returns a `BindingBuilder` carrying every combinator. Chaining reads in
evaluation order, which is the order the plan stores them.

| Group | |
| --- | --- |
| Chord | `with(control)` |
| Conditions | `press`, `release`, `down`, `hold`, `hold_once`, `hold_and_release`, `tap`, `multi_tap`, `pulse`, `on_change`, `when(custom)` |
| Modifiers | `scale`, `negate`, `swizzle`, `clamp`, `clamp_magnitude`, `rescale`, `curve`, `per_second`, `compass`, `dead_zone`, `tunable_dead_zone`, `custom` |
| Consumption | `consume`, `without_consuming` |
| Presentation | `mappable`, `mappable_as`, `private`, `reserved` |

`Modifier` and `Condition` are traits, and `BindingModifier` / `BindingCondition` are enums with a
`Custom` variant holding an `Arc`, so built-ins dispatch statically and stay exhaustively matchable
while extensions work. The boxes are allocated at compile time, never per tick.

A condition returns a `ConditionState` of `Idle`, `Building` or `Satisfied`, and has a
`ConditionKind`:

| Kind | Rule |
| --- | --- |
| `Explicit` | any one satisfying is enough |
| `Implicit` | all must hold |
| `Blocking` | any one vetoes |

Two further builder entry points:

- `follow::<Follower, Leader>(configure)` declares that a second action rides the leader's controls
  — tap to dodge, hold to sprint. It reads whatever `Leader` has declared *so far* and generates a
  matching binding per device found, so a follower can ride some of a leader's devices by being
  declared before the rest. Every generated binding reads exactly the control it copied, which is
  what makes a rebind move both.
- `combined::<A>()` returns a `CombinedBuilder` for the stage after the fold of TD5.5. It carries
  the modifiers and conditions that mean the same on a combined value as on one control, and leaves
  out `hold`, `hold_once`, `hold_and_release` and `multi_tap`, whose descriptor a prompt reads per
  binding, as well as `dead_zone`, `rescale`, `per_second` and everything that names a control.
- `hold_or_toggle::<A>(key)` declares a latch turning a momentary press into a sustained one, once
  per action rather than per binding, so every eligible control shares one runtime latch.

`bind_class::<A>()` declares the class binding of TD5.4, and `diagnostics()` returns what the plan
build found.

### 8.3 Dead zones and thresholds

```rust
pub enum DeadZoneShape { Radial, PerAxis }
pub enum CompassPoints { Four, Eight }
```

`ButtonThreshold` is the resource deciding when an analog reading counts as a press, with hysteresis
against the previous state. `compass` rounds a 2D value to four or eight points and discards the
magnitude; with `on_change` it fires once per point entered, and with `pulse` after that it is
auto-repeat.

At most one modifier in a chain may rescale. `BindingModifier::rescales` reports it, defaulting to
`false` for a third-party modifier, and a chain that stacks two is refused at plan build.

### 8.4 The three dead-zone stages

| Stage | Where it runs | Rescales |
| --- | --- | --- |
| **Calibration** — this unit's true centre and rest envelope | `sample_input`, as the raw message is recorded | no |
| **Design** — the shape and curve the mechanic wants | the binding's modifier chain | yes, by default |
| **Preference** — the player's adjustment | modulates the design stage, as a tunable | no |

Calibration is per device unit and is applied once as the frame is assembled, so by the time held
state exists the value has already been corrected by the right unit's calibration.
`GamepadCalibration` holds it, `AxisCalibration` is one axis's worth, and `CalibrationSampling` is
the helper an app drives during an explicit "move the sticks and let go" step — the instruction is
to move them because a pad reports an axis only when it changes. What is measured lasts as long as
the process; there is no persistent device identity to key it to.

---

## 9. The presentation surface

A deliberately smaller model than the binding one, in its own vocabulary. The whole of it is keys —
nothing here is a string this crate renders, and `fallback_label` is readable English for a game
that ships no catalogue.

### 9.1 Mappings

A **mapping** is the named thing a player rebinds; a **slot** is one position in it, holding one
control and whatever that control's binding requires held alongside it. A screen draws one cell per
slot.

```rust
pub struct ActionMapping {
    pub key: MappingKey,               // "gameplay.move.up" — a localization key
    pub action: ActionId,
    pub action_path: &'static str,
    pub category: Option<&'static str>,
    pub family: DeviceFamily,          // KeyboardMouse | Gamepad
    pub accepts: ChannelShape,
    pub slots: Vec<Option<BoundSlot>>, // ordered; slot 0 is the primary, None is an emptied cell
    pub rebind_policy: RebindPolicy,   // Here | Fixed
    pub context: &'static str,
    pub followers: Vec<Follower>,
}

pub struct BoundSlot {
    pub control: Control,
    pub with: Vec<ControlOrigin>,      // empty for almost every slot
}
```

**A slot carries its chord**, in the same `ControlOrigin` terms `Prompt::with` uses and for the same
reason: a modifier stands for either key of its pair, and `Control` can only name one of the two. A
row that dropped it would list `Ctrl+N` as "N" while the caption built from the same binding went on
reading `Ctrl+N`. `ActionMapping::controls()` is the bare-control view, for reading only.

**The slot is also what an override writes**, and it says everything about what is bound there. A
screen edits the slots it read and hands them back to `Overrides::bind`; `rewrite` gives the binding
at that position the slot's control *and* its chord. Whether a new control keeps the old modifier is
therefore the screen's choice, made by which it writes: assigning `slot.control` keeps the chord, a
fresh `BoundSlot::from(control)` drops it. A grown slot takes its chord from the slot too, not from
the primary it was cloned from. A chord entry must be something a player can hold — a modifier or a
button of ours — and anything else is refused as `NotChordable`.

**Three listing states, plus a fourth for followers.** A binding is listed and fixed unless it says
otherwise:

| declaration | listed | rebindable |
| --- | --- | --- |
| (none) | yes | no |
| `mappable` | yes | yes |
| `private` | no | no |
| `follow::<F, L>()` | on `L`'s row, as a subordinate line | with `L`'s row |

`mappable` takes no arguments. The parts of a composite name themselves, so a key derives as
`gameplay.move.up`, and the family is inferred from the control. `mappable_as` replaces the action's
path in that key where one is needed; the part is still appended.

**A row has no declared width.** `slots` is however many controls the row holds, and how many cells
to draw beside them is the screen's own decision — a table with a spare column draws one more than
the row holds and a capture fills it. Declaring two mappable bindings of one action in one family is
how a game ships a default primary *and* secondary: they merge into one row with two slots, not two
rows.

**A slot may be empty.** `None` is a cell the player cleared with something still bound after it, so
clearing a primary leaves the secondary in the second column rather than promoting it — position is
what primary and secondary mean. Only an override makes a gap; an author cannot declare one, so a
row derived from bindings alone is always dense. Trailing empties are not kept: a row is as long as
its last filled slot, and a row with nothing left is `Override::Cleared` rather than a list of
`None`s.

Because a binding list records what is bound and never in which column, a gap cannot be recovered by
re-deriving rows from the rewritten bindings. `rewrite` therefore carries the accepted override's
own list through to the presentation rows — the derivation stays the authority on *which* controls,
and the override on where the holes are.

**`MaxSlots` is the one length the crate has an opinion about**, and only on the apply path: a game
that reads override sets it did not write inserts the resource, and a row naming more controls than
that comes back as `TooManyControls` rather than being applied. Without the resource there is no
limit, which is what a game whose save files are its own already assumes.

A capture can reach past it, because a capture fills the cell the player pressed and a screen may
draw more columns than the game's own ceiling allows. That is the app disagreeing with itself about
two numbers it owns both of, so the crate warns once when a session opens for a slot at or past the
ceiling rather than refusing the session: `slot >= limit` settles it without looking the row up, and
the apply path is still what turns the row down.

Uniqueness is per family, and two mappable bindings collide only when they name different actions.

**Tunables** are named, typed values a player adjusts:

```rust
pub enum TunableValue { Range { value: f32, min: f32, max: f32 }, Bool(bool) }
```

A tunable overwrites one field of one modifier already on a binding, and is applied by the same
variant-plan recompile a rebind uses. `Tunable` carries its key and value; the type is what lets a
UI render a slider or a checkbox without knowing what it drives.

**Reading the list.** `mappings(world)` returns what is bound *now*; `declared_mappings(world)` the
defaults, for a reset preview. `tunables` and `declared_tunables` are the same pair. Both lists are
flat across every context, and nothing in them names an action type or a context type, so a screen
written against them works for a game it was not compiled with. Grouping is the caller's: by
`category` for headings, by `family` for which device's worth to show.

### 9.2 Prompts

The lookup that runs the other way: given an action, which controls it is bound to.

```rust
pub trait Prompts {
    fn prompts(&self, action: ActionId, scope: PromptScope) -> Vec<Prompt>;
}

pub struct Prompt {
    pub origin: ControlOrigin,
    pub with: Vec<ControlOrigin>,       // what else is held — `Ctrl+S` reads as "S" without it
    pub part: BindingPart,
    pub condition: ConditionDescriptor, // None | Hold { duration } | MultiTap { count }
    pub context: Option<&'static str>,
}

pub enum ControlOrigin {
    Ours(Control),
    Modifier(ModifierKey),              // a chord entry only, either key of the pair
    Foreign {
        name: String, label: String, family: Option<DeviceFamily>, class: Option<ControlClass>,
    },
}
```

A trait rather than a function, because this crate's tables are not always the authority; the answer
is a `ControlOrigin` rather than a `Control` for the same reason, and both variants answer `name()`
and `fallback_label()`. `BindingTable` is the implementation that answers from this crate's own
plans.

**`Control::fallback_label_for_brand(GamepadBrand)`** (TD7.6) answers the way `fallback_label` does,
except a gamepad's face buttons, bumpers, triggers, Select/Start and Mode read in that brand's own
words — "Cross" rather than "South Button" on a PlayStation pad. Sticks and the D-pad read the same
either way, and `GamepadBrand::Generic` falls through to the positional answer.

**Glyphs.** `resolve_glyph(&ControlOrigin, GamepadBrand, has_art)` names the art for one control and
ships none: `has_art` is the app's atlas, answering whether a `GlyphTier` covers a control. A
gamepad control tries its brand's tier and then `Generic`'s; a keyboard or mouse control, a modifier
included, has the one `KeyboardMouse` tier; a foreign control answers `None`, its art being its
reporter's. `Glyph` is `#[non_exhaustive]` with one variant, `Own(GlyphTier, ControlOrigin)`, so a
backend's own image can arrive as another (R18.9). A chord resolves an entry at a time. Everything
that draws — the atlas, inline and block layout, the art a Mac labels differently — is
`examples/common/prompt_ui.rs`, outside the crate.

**A prompt is not a row of the settings screen.** `mappings` is what the game declared and is
static; a prompt answers from the contexts something carries, so it is empty for a context nobody is
carrying, and it *includes* a `private` binding. The lookup reads the compiled plan rather than
filtering the mapping list.

**Not present-tense.** A carried context answers whether or not it is active or shadowed by an
exclusive one, and a control a stronger context consumes is still named. Whether a hint belongs on
screen is the app's to decide (D84).

**Ranking.** Contexts come back in the order they get to claim a control — render tick before fixed
tick, then by priority, then declaration order — and within a context, in declaration order.
Nothing ranks one device above another; the device is a scope the caller supplies through
`PromptScope`, which narrows by context path, family and control class. `PromptDevice` is the
game-wide setting for which device a bare prompt speaks for, and the crate never defaults it.

**Staleness** is signalled by `PromptGeneration`, a counter bumped when bindings are applied and
when an instance of a context arrives or goes away. Activation does not bump it, since it does not
change the answer. It is written as an insert rather than a mutable deref, so it can be read either
by a `resource_changed` run condition or by an observer. `PromptDevice` is public, so a game
changing it by hand bumps the counter itself.

### 9.3 Capture

Rebinding wants the control every other path through the crate discards, so capture reads the frame
directly rather than through a binding. A settings screen reached from the main menu therefore works
with no gameplay context spawned and no evaluator stepping.

`CaptureSession` is a component, placed on whatever entity the caller picks — usually the cell
button the player activated, so "which cell is listening" is answered by where the component is.

```rust
CaptureSession::for_mapping(&mapping)        // the shape that mapping can hold
CaptureSession::accepting(ControlClass::AnyButton)
    .excluding([Control::PhysicalKey(KeyCode::Escape)])
```

The crate answers with a `ControlCaptured` event on that same entity and removes the component;
removing it yourself cancels. It never touches the player or context entities.

A session carries a class and an exclusion list, and no target: which row and cell a capture answers
is whatever the screen put on the entity it listens on. `for_mapping` takes the class from the
mapping — `ControlClass::of(row.accepts)` — and nothing else from it.

A captured key is always a `PhysicalKey`. The player is sitting at a known layout, where the
position and the character it produces name the same key, so there is no ambiguity left for a
logical control to resolve — and a press is a position. A logical binding is therefore something a
game declares, and rebinding one overwrites it with the position captured; resetting to defaults
brings it back.

A session skips whatever is already queued on its first run, so the press that opened it is not what
it binds.

**One press, one answer, on the way up.** A capture ends on the control the player chose, whether or
not the row can hold it, and reports it. Whether it may be stored is asked once, where the row is
written — `Rebind::checked` (TD10.1) — so a press and a loaded save file cannot get different
answers, and the screen is what says why.

**The answer comes on the release**, and the session claims the control for as long as it is held. A
claim is an instant and a held control is a level: the evaluator records held state as events arrive
and applies consumption where it *reads*, so a session that answered on the press and vanished would
leave the control down with nothing claiming it, and the context underneath would read it on the
very next frame. Waiting means the control is already up when the claim stops. The mouse's motion is
the exception with nothing to wait for — a displacement that has already happened is never held — so
it answers immediately. Losing focus ends the wait too, since the release is going to another window
and the evaluator clears its held state to match.

Two kinds of arrival do not end a capture:

| | |
| --- | --- |
| *excluded* | the screen's own controls, so the key that cancels a capture still cancels it |
| a reading nobody chose | a deflection past `DEFLECTION` or a twitch past `MOUSE_MOTION`, on a session listening for some other class — a pad on a desk must not cancel a keyboard rebind |

A session listening for that class takes such a reading, because for a stick row the deflection *is*
the answer. Everything a capture does take is claimed, so a control pressed at a rebinding screen
never also plays the game — which is what makes reserving work: the settings key neither binds nor
re-opens the screen. Declaring a binding both `reserved` and `mappable` is a plan-build error.

**Conflicts are detected, not resolved.** `conflicts(world, candidate, target)` is a pure query over
the mapping list, answerable before anything is committed; `conflicts_pending` asks the same of a
working copy of overrides, which is what lets a screen with unconfirmed choices tell whether two of
them clash. `ConflictOverlap` says whether the clash is `SameContext` or `OtherContext` — a clash
across two contexts is *possible* rather than certain, since whether they are ever live together is
the game's own question. The candidate is a `BoundSlot`, and `BoundSlot::clashes_with` is the
comparison: the same control with the same chord, order ignored, so `S` and `Ctrl+S` do not clash.
It misses `Ctrl+S` beside a chord naming one Control key, and two same-length chords a player
satisfies at once by holding both sets. The whole target mapping is excluded rather than the one
slot.

---

## 10. Overrides and persistence

An override set is a diff against the declared defaults. The crate defines the structure and knows
nothing about where it ends up.

```rust
pub enum Override {
    Slots(Vec<Option<BoundSlot>>), // in slot order; None is an emptied cell, trailing ones dropped
    Cleared,                       // deliberately emptied — distinct from a missing row
    NotOurs,                       // an external authority owns this mapping
}
```

`Overrides::slots_of(row)` is the list to edit: the set's own slots where it has changed the row,
and the row's declared ones, chords included, where it has not. `with_cell(row, n, control)` is that
list with one cell replaced — **a cell is addressed, not appended**: the row grows to reach `n` and
the cells skipped on the way are left empty, so writing to the third cell of a one-control row gives
a row of three with a blank in the middle. A screen therefore offers whatever cells it draws without
first asking how long the row is. A filled cell keeps whatever it was held with; an empty one takes
the control alone.

Rows are keyed by `(DeviceFamily, MappingKey)` and tunables by `(DeviceFamily, key)`. Nothing in an
`Overrides` names a device: what a player bound is a control on a device *class*, and which physical
unit drives which player is a separate question.

`Overrides` has `bind`, `set`, `get`, `iter`, `tune`, `get_tunable`, `iter_tunables`, and the family
of resets: `reset` for one row, `reset_tunable`, `reset_action`, `reset_context`, `reset_all`.

### 10.1 Applying

```rust
apply_overrides(world, &overrides)                      -> Vec<OverrideProblem>
apply_overrides_with_preset(world, &overrides, &preset) -> Vec<OverrideProblem>
apply_overrides_for(world, entity, &overrides)          -> Vec<OverrideProblem>
apply_overrides_for_with_preset(..)                     -> Vec<OverrideProblem>
```

Applying is the only path in, and startup is simply the first call. It rewrites the *authored*
bindings rather than patching compiled ones — the `BindingSpec`s are retained beside the plan and
cloned per apply — then compiles a variant plan and swaps it into every live instance, which cancels
what was in flight and re-arms require-reset. Followers riding a row that changed move with it,
chord included. `AppliedPlan<C>` keeps the variant so an instance spawned later sees it too, and
`InputContextPlan<C>` is left untouched, so the next patch's revised defaults still reach a player
who never touched that row.

Three slot cases: a slot the defaults fill has its binding's input and chord rewritten; a slot they
left empty is filled by *copying* the binding beside it, so a secondary carries the same modifiers
and conditions as the primary, with its own chord; and a slot the override no longer has takes its
binding away. A row that is one direction of a composite is no exception, since that direction is a
binding of its own.

**Overrides do not compose.** Each apply starts from the pristine declaration, so the argument must
be the *whole* working copy — a preset's rows and any manual captures together. A smaller second
call silently reverts every row it does not mention.

`apply_overrides_for` reaches one entity's own instance, so two occupants sharing a context type
apply and persist independently, with the world-wide plan untouched.

**Problems are reported, never dropped:**

```rust
pub enum OverrideProblemKind {
    NoSuchMapping, NotRebindable, WrongFamily { .. }, WrongShape { .. },
    Reserved { .. }, NotChordable { .. }, TooManyControls { .. }, UnknownControl { .. },
}
```

A whole row is refused rather than partly applied: half a rebind is worse than none, and the player
still has the default.

**A screen asks before it writes.** Applying is where a *file* is judged, which is too late for a
player still looking at the cell they pressed, so the same predicate is reachable ahead of the
write:

```rust
Rebind::checked(world, &row, slots)                   -> Result<Rebind, OverrideProblemKind>
Rebind::checked_with_preset(world, &preset, &row, ..) -> Result<Rebind, OverrideProblemKind>
rebind.write(&mut overrides)                          // the only thing that writes a checked row
```

`Rebind` is a checked row and the token that writes it, so a row cannot be stored through this door
without having been judged. `Overrides::bind` stays unchecked beside it, because a set loaded from a
file has no world to be judged against and `apply_overrides` is what judges one — the guarantee is
about the screen's path, not an absolute. The preset form exempts the rows a preset authorized, on
the same terms as `apply_overrides_with_preset`; a screen that offers presets wants it, or a `Fixed`
row a preset legitimately moves reads as unchangeable.

`unbind` needs no token: emptying a cell removes a slot rather than adding one, can only shorten the
row, and leaves `rebind_policy` alone, so a row that was admissible stays admissible. Only the
adding side can be refused.

### 10.2 Presets

A `Preset` is a name paired with an `Overrides`. `Preset::build(world, name, |p| ..)` gives it the
builder's own ergonomics — `p.bind::<A>(family, controls)` resolves `A`'s declared mapping in that
family and writes the row, so an app never derives a key by hand. It panics rather than guessing
when an action has no mapping in that family or more than one, except where a context binds the
action to an `Authority` for that family: then it writes nothing, since the authority's layout is
where that family is bound. Each `DeclaredContext` records its `(action, family)` authority pairs at
declaration for this, as data, because no override ever rewrites one. `p.tune(family, key, value)`
sets a tunable alongside it, so "Southpaw" can move a dead zone the same step it moves a stick's
axis; it writes straight through to `Overrides::tune`, since a tunable's key is already the app's
own string rather than one the crate hands out.

Passing a preset to `apply_overrides_with_preset` exempts exactly the rows that preset names from
the `NotRebindable` refusal — which is what lets a preset move a `Fixed` row a capture screen never
offers a button for. Every other refusal still applies.

There is no crate-owned registry of presets, and no record of which one is applied. A game keeps its
own list and its own answer, as it keeps its own working copy; merging a preset's rows with the
player's own captures into the one set to apply is the caller's job.

### 10.3 Serialization

Behind the `serialize` feature. `Overrides` itself cannot be the wire type: its fields hold a
`MappingKey`, constructible only from a `&'static str` the game already compiled in, which no
generic reflection walk can manufacture from loaded data. [`SavedOverrides`] is the portable shape
that stands in for it — plain, owned strings, needing no context to construct:

```rust
pub struct SavedOverrides {
    pub action_map_version: u32,
    pub bindings: BTreeMap<String, BTreeMap<String, SavedRow>>,   // family -> mapping -> row
    pub tunables: BTreeMap<String, BTreeMap<String, SavedTunableValue>>,
}

pub enum SavedRow { Slots(Vec<String>), Cleared, NotOurs }
pub enum SavedTunableValue { Number(f32), Bool(bool) }
```

`SavedOverrides` derives `Reflect` and is walked structurally — its own field names *are* the TOML
keys, so a `Reflect`-based settings layer (`bevy_settings` and similar) can register and load it with
no code of this crate's own in the path. `SavedRow` and `SavedTunableValue`, reached only as nested
field values, carry their own hand-written `Serialize`/`Deserialize` and are marked
`#[reflect(Serialize, Deserialize)]`, so a `Reflect`-based deserializer bridges to that encoding
rather than its own generic enum representation:

```toml
action_map_version = 1

[bindings.gamepad]
"gameplay.jump" = "cleared"

[bindings.keyboard_mouse]
"gameplay.jump"    = ["key/Space", "key/KeyJ"]   # primary, secondary
"gameplay.fire"    = ["cleared", "key/KeyF"]     # primary emptied; the secondary stays second
"gameplay.move.up" = "key/KeyI"                  # a scalar is a one-element list
"editor.save"      = "mod/ctrl+key/KeyS"         # held first, then the control
"ui.settings"      = "external"

[tunables]
```

A row holding one control writes as a bare scalar and reads back from either form; position in a
list is which slot, so a cleared middle slot needs `"cleared"` rather than a shortened list — the
same word a whole emptied row uses, meaning the same thing one level down. Trailing empties are not
written, so a short list read back means the rest are empty, and a list of nothing but the word
reads as a cleared row. The two state words cannot collide with a control name, because the control
encoding is a format this crate owns rather than `Debug` or serde on Bevy's own types — an upstream
rename becomes a compile error in an exhaustive match while the stored string stays what it was. A
logical key writes as `char/` and the character, `char/z`; the remainder is taken whole, so the
separator needs no escape, and a name carrying anything but one character reads back as no control
at all. `bindings`/`gamepad` sorts ahead of `bindings`/`keyboard_mouse` alphabetically rather than
in `DeviceFamily`'s own declared order, and an empty `tunables` still gets a header — both accepted
costs of a plain, structurally reflected type over a hand-rolled one.

A slot's chord rides the string the slot already has: each entry under the name a catalogue looks it
up by, a `+` after each, then the control — `mod/ctrl+key/KeyS`, or `pad/LeftTrigger+pad/South` for
a chord of two buttons. `char/` is the one name that can hold a `+`, and since it always holds
exactly one character it is measured rather than split, so `mod/shift+char/+` is Shift held with the
`+` key. A slot with any name in it this build does not know is `UnknownControl`, carrying the
slot's whole text.

**`SavedOverrides` claims no field besides `action_map_version`, `bindings` and `tunables`, and none
of those is a bare `version`** (R17.10, D59). A settings layer that lets several resources share one
TOML table by name can put `SavedOverrides`'s fields beside an unrelated struct's, so the one field
likely to collide with something else's is the namespaced one.

**`save_overrides`/`resolve_saved` are the pure functions** in and out of this shape.
`resolve_saved` resolves a saved mapping or tunable name against what the game currently declares —
a `MappingKey` or a tunable key can only ever be one already declared — and reports an `Unresolved`
row rather than dropping either kind in silence. A renamed action's row is dropped on the next save
rather than preserved unresolved.

`action_map_version` is checked before any row is read: a `SavedOverrides` naming a version this
build never shipped is refused as a whole (`UnsupportedVersion`) rather than resolved as the one
version that exists today. There is no migration path yet, because there has never been a second
version for one to convert from. `SavedOverrides::default()` stamps the version this build writes,
so the empty set a settings layer builds for a game with no file yet resolves like any other rather
than being refused on its first launch.

---

## 11. Crate layout

One crate, feature-gated by input source, plus the proc-macro crate Rust requires for the derives.
The macro crate is re-exported, so nothing names it.

```
src/
  device.rs      L0  families and handles, pairing sets, gamepad calibration and brand resolution
  frame.rs       L1  the event queue, sampling, retirement
  action.rs          identity, intent, channel shape, value, phase, scratch
  binding/mod.rs     re-exports the three below, which are private to it
    control.rs       controls, inputs, composites, parts, the conversion into a binding input
    modifier.rs      dead zones, compasses, curves, toggles, and the value math they apply
    builder.rs       the declaration API, and the BindingSpec record it writes
  condition.rs       conditions and their verdicts and descriptors
  context/mod.rs     re-exports the two below, which are private to it
    state.rs         one instance's live state, and the params that read it
    declare.rs       the app wiring, the records declaration writes, the type-erased reads
  plan.rs            compilation, slot allocation, diagnostics
  eval.rs            the evaluator, consumption, the exclusion ceiling, dispatch
  event.rs           Fired/Started/Completed/Canceled, class bindings
  player.rs          the Paired component
  join.rs        L3  is_claimed
  mapping.rs         mappings, slots, tunables, and their derivation from bindings
  overrides.rs       the diff structure, applying it, serialization
  preset.rs          a named Overrides and its builder
  capture.rs         capture sessions, reserved controls, conflict detection
  present.rs         control naming, prompts, prompt scope and staleness
  inspect.rs         the type-erased read of contexts and actions
  backend.rs         Authority, and AuthorityValues, the component an authority binding reads
bevy_action_map_macros/   #[derive(InputAction)], #[derive(InputContext)]
```

`inspect.rs` exists for code outside a game: every other read in the crate is generic over the
action type, which is right for game code and unusable for a debug overlay, an editor, or a settings
screen rendering actions it was never compiled against.

**Features**, mirroring `bevy_input`'s own layout:

| Feature | Gates | Default |
| --- | --- | --- |
| `keyboard`, `mouse`, `gamepad` | source vocabularies and their sampling | yes |
| `touch` | reserved | no |
| `std` | `no_std` + `alloc` otherwise | yes |
| `libm` | glam's math backend for a `no_std` build | no |
| `bevy_reflect` | `Reflect` on the crate's data, components and resources | yes |
| `serialize` | persistence of overrides and input frames; pulls in `bevy_reflect` | no |
| `state` | a context's activation following a `bevy_state` state | yes |
| `focus` | the `bevy_input_focus` dependency | no |

Under `bevy_reflect`, what a scene authors, a tool reads or tunes, or a save file stores derives
`Reflect`, with `ReflectComponent` or `ReflectResource` where it is one; evaluator state does not
(D86). Nothing is registered by hand: derived types reach `AppTypeRegistry` through the app's
`reflect_auto_register`. `register_device_identity` and `SavedOverrides` are the exceptions, the
first because it inserts type data auto-registration cannot, the second so that a settings layer
loading it in an app with auto-registration off does not drop it silently.

There is no `scene`/BSN feature. Transitions are plain `EntityEvent`s, so `bsn!` attaches observers
with no adapter and no dependency in either direction.

The crate is `no_std` + `alloc` and `forbid(unsafe_code)` throughout.
