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
L0  device      raw messages from keyboard, mouse and gamepad, plus per-unit calibration
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
pub struct Timestamp { frame: u64, order: u32 }

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
(§8.4). A backend writing values into the frame directly enters past that point.

---

## 3. Actions, contexts and values

An action is a type, not a value.

```rust
pub trait InputAction: Send + Sync + 'static {
    type Output: ActionOutput;      // bool, f32, Vec2, Vec3
    const INTENT: Intent;
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
| **`Intent`** | the action's meaning | `Button`, `Analog1`, `Directional2`, `Delta2` |
| **`ChannelShape`** | the control's report | `Button`, `Axis1`, `Axis2`, `Delta2` |

A stick deflection and a mouse delta are both `Vec2`; `Directional2` is a position implying a rate,
and `Delta2` is a displacement that already happened. `Intent::accepts` decides which channel shapes
can serve which intent, and a binding whose channel cannot serve its action's intent is refused when
the context is declared. The derive checks output against intent in a compile-time assertion.

A directional composite's `Axis2` is still four buttons read together; a gamepad stick's is the one
exception, a single `Control::GamepadStick` reporting a position the same way `MouseMotion` reports a
displacement.

### 3.2 Runtime values

```rust
pub enum ActionValue { Bool(bool), Axis1(f32), Axis2(Vec2), Axis3(Vec3) }

pub enum Phase { Idle, Started, Building, Fired, Firing, Completed, Canceled }

pub struct ActionState { pub value: ActionValue, pub phase: Phase }
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
a process run; `ActionId::from_path` resolves one back, and `registered_actions()` lists them.

---

## 4. The compiled plan

Bindings are authored as data and compiled once per context into a `Plan<C>`.

| Compilation produces | Serves |
| --- | --- |
| action → slot assignment, and the reverse as a direct index | O(1) state access without hashing |
| scratch slot assignment per condition and stateful modifier | §6 |
| each binding's chord length, and whether the plan has any | the clash pre-pass, skipped when there are no chords |
| the set of controls any binding indexes | class-binding fallback, §5.4 |
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
rather than compiling a plan that cannot work.

```rust
pub enum Severity { Error, Warning }

pub enum DiagnosticKind {
    IntentMismatch { .. }, RateFromDelta { .. }, ChainedRescaling { .. },
    DuplicateBinding { .. }, ConsumeDisagreement { .. },
    DuplicateMappingKey { .. }, RebindingDisagreement { .. }, MixedSchemeMapping,
    ReservedAndMappable, FollowsNothing { .. }, FollowsUnlisted { .. },
    DuplicateClassBinding { .. }, DuplicateTunableKey { .. },
    TunableShapeDisagreement { .. },
}
```

---

## 5. Evaluation

One pass per context per tick: raw events in, action state and a transition log out.

```
raw event
  → held-state update
  → chord clash pre-pass (once per fold, if the plan has chords)
  → modifier chain      negate · swizzle · scale · dead zone · curve · clamp · compass · custom
  → conditions          explicit: any satisfies · implicit: all hold · blocking: any vetoes
  → fold by intent      several bindings, one action
  → write ActionState, mark dirty, append transition
  → consume?            record the control so later contexts skip it
```

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

### 5.2 Consumption

A binding declared `consume` records its controls in `ConsumedControls` while it is `Fired` or
`Ongoing`, and contexts evaluating later see those controls as untouched.
`ConsumedControls::claimant` names the context that took one.

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
active. `ExclusionCeiling` holds the highest priority of any active exclusive context seen so far
this tick. Each context's activation system, already running in priority order, checks the ceiling
before trusting its own condition: at or below it, the context is *shadowed* instead.

Shadowing cancels in-flight actions and re-arms require-reset, exactly as deactivation does. It is
tracked in a field of its own (`shadowed`) rather than by clearing `active`, so a context's own
condition and what is being forced on it do not overwrite each other. `is_active` is the
conjunction.

The ceiling is set by the `PreUpdate` pass and read — never rewritten — by every `FixedPreUpdate`
run that frame, then cleared at the top of the next frame alongside the consumption release.

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
value. The action's `Intent` decides the rule:

| Intent | Fold |
| --- | --- |
| `Button`, `Analog1`, `Directional2` | strongest contribution wins |
| `Delta2` | contributions are summed |

Ties keep the earlier contribution, so declaration order is the tiebreak.

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
an unaffected device is untouched.

---

## 6. State and storage

State divides in two, and both halves are dense arrays of `Copy` types indexed by plan slot.

| | Holds | Belongs to |
| --- | --- | --- |
| `ActionState` | value, phase | the action |
| `Scratch` | hold timers, tap counts, previous value, flags | the binding's conditions and modifiers |

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

- `plan: Arc<Plan<C>>`, and the two tables above;
- `dirty`, a per-action bitset — evaluation writes through a bypassed borrow and re-marks the
  component only where a bit was set, so a tick that changed nothing is invisible to a subscriber;
- `active` and `shadowed`;
- `tunable_scratch`, one cell per group of bindings sharing a tunable, rather than a private slot
  per binding;
- `chord_claims`, reused between folds;
- `require_reset`, parallel to the action table;
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

### 7.2 Activation

A declared context is live as soon as an entity carries it. `active_if` and `active_in_state` (the
latter behind the `state` feature) make activation follow something else; `activate`, `deactivate`
and `activate_including_held` drive it by hand.

Activation arms **require-reset**: a control the player was already holding does not read as a fresh
press, and must be released once first. The latch holds back `Button` actions only — an analog
action has no synthesized fire to guard against, and its value simply resumes.
`activate_including_held` skips the arming, which is what a context taking over from another
driving the same controls wants. Deactivation cancels whatever is in flight.

### 7.3 Reading

```rust
fn movement(input: Actions<OnFoot>) {
    let dir = input.value::<Move>();     // Vec2, checked at compile time
    if input.fired::<Jump>() { .. }
}
```

`Actions<C>` is for a context with exactly one instance; a system taking it is skipped when there is
no instance or several, following Bevy's `Single`. `ActionsQuery<C>` is the per-player form — `get`,
`iter`, `len`. Both expose `value`, `try_value`, `phase`, `fired` and `why_not`.

`why_not` answers the question a call site cannot:

```rust
pub enum Obstacle {
    None, Unbound, ContextInactive, AwaitingRelease,
    Consumed { control: Control, by: &'static str },
    Outranked { control: Control, chord: u8 },
    ConditionPending, NoInput,
}
```

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

**Joining** needs no separate evaluation path. A game declares "press anything to join" as an
ordinary action, bound with `bind_class` to `ControlClass::AnyButton` on a context with no `Paired`
of its own, so it reads every device. `ClassFired`'s untouched `RawEvent` names the device, and
`join::is_claimed` checks it against the world's `Paired` set so two waiting slots never race for
one device.

`ConsumedControls` and `ExclusionCeiling` are computed once per context type and are not scoped by
owner.

---

## 8. Bindings

### 8.1 Controls and sources

```rust
pub enum Control { Key(KeyCode), MouseButton(MouseButton), GamepadButton(GamepadButton),
                   GamepadAxis(GamepadAxis), GamepadStick(Stick), MouseMotion }
```

A `BindingSource` is one control or an arrangement of them. Composites carry a `Part` naming which
piece of the whole a control drives:

```rust
pub enum Part { Whole, Negative, Positive, Up, Down, Left, Right }
```

`AxisButtons` makes a bipolar axis from two buttons; `DirectionalButtons` makes a direction from
four (`DirectionalButtons::wasd()` is the named case); `Stick` and `MouseMove` are the analog
sources, and both read as `Part::Whole` — a stick has no part a player rebinds one of. `GamepadStick`
is `Control`'s only member naming what another one of its members names in part: a whole stick, for
presentation, override application and capture, reporting `ChannelShape::Axis2` the way `MouseMotion`
reports `Delta2`. Consumption does not follow it — `BindingSource::for_each_control` still decomposes
a stick binding into its two `GamepadAxis` atoms, which is the granularity `ConsumedControls` and
reservation key on. `Control::scheme()` and `Control::shape()` classify one control, and
`BindingSource::channel_shape` classifies an arrangement.

### 8.2 The builder

`bind::<A>(source)` returns a `BindingHandle` carrying every combinator. Chaining reads in
evaluation order, which is the order the plan stores them.

| Group | |
| --- | --- |
| Chord | `with(control)` |
| Conditions | `press`, `release`, `down`, `hold`, `hold_once`, `hold_and_release`, `tap`, `multi_tap`, `pulse`, `on_change`, `when(custom)` |
| Modifiers | `scale`, `negate`, `swizzle`, `clamp`, `curve`, `per_second`, `compass`, `dead_zone`, `tunable_dead_zone`, `custom` |
| Consumption | `consume`, `without_consuming` |
| Presentation | `mappable`, `mappable_as`, `mappable_upto`, `mappable_any`, `private`, `reserved` |

`Modifier` and `Condition` are traits, and `BindingModifier` / `BindingCondition` are enums with a
`Custom` variant holding an `Arc`, so built-ins dispatch statically and stay exhaustively matchable
while extensions work. The boxes are allocated at compile time, never per tick.

A condition returns a `Verdict` of `Idle`, `Ongoing` or `Fired`, and has a `ConditionKind`:

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
- `hold_or_toggle::<A>(key)` declares a latch turning a momentary press into a sustained one, once
  per action rather than per binding, so every eligible control shares one runtime latch.

`bind_class::<A>()` declares the class binding of §5.4, and `diagnostics()` returns what the plan
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
control. A screen draws one cell per slot.

```rust
pub struct Mapping {
    pub key: MappingKey,             // "gameplay.move.up" — a localization key
    pub action: ActionId,
    pub action_path: &'static str,
    pub category: Option<&'static str>,
    pub scheme: Scheme,              // KeyboardMouse | Gamepad
    pub accepts: ChannelShape,
    pub slots: Vec<Control>,         // ordered; slot 0 is the primary
    pub capacity: Capacity,          // UpTo(n) | Any — how many columns to draw
    pub rebinding: Rebinding,        // Here | Fixed
    pub context: &'static str,
    pub followers: Vec<Follower>,
}
```

**Three listing states, plus a fourth for followers.** A binding is listed and fixed unless it says
otherwise:

| declaration | listed | rebindable |
| --- | --- | --- |
| (none) | yes | no |
| `mappable` | yes | yes |
| `private` | no | no |
| `follow::<F, L>()` | on `L`'s row, as a subordinate line | with `L`'s row |

`mappable` takes no arguments. The parts of a composite name themselves, so a key derives as
`gameplay.move.up`; the scheme is inferred from the controls, and a binding whose parts span both
schemes is refused. `mappable_as` replaces the derived key where one is needed.

**Capacity is inferred and raised, never lowered.** A plain `mappable` asks for one slot; several
bindings feeding one mapping take the widest anything asked for; and no mapping ends up narrower
than the defaults it already holds. Declaring two mappable bindings of one action in one scheme is
how a game ships a default primary *and* secondary — they merge into one row with two slots, not two
rows. `mappable_upto(n)` ships one control and leaves the rest for the player.

Uniqueness is per scheme, and two mappable bindings collide only when they name different actions.

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
`category` for headings, by `scheme` for which device's worth to show.

### 9.2 Prompts

The lookup that runs the other way: given an action, which control would fire it now.

```rust
pub trait Prompts {
    fn prompts(&self, action: ActionId, scope: PromptScope) -> Vec<Prompt>;
}

pub struct Prompt {
    pub origin: ControlOrigin,
    pub with: Vec<ControlOrigin>,       // what else is held — `Ctrl+S` reads as "S" without it
    pub part: Part,
    pub condition: ConditionDescriptor, // None | Hold { duration } | MultiTap { count }
    pub context: Option<&'static str>,
}

pub enum ControlOrigin {
    Ours(Control),
    Foreign { name: String, label: String, scheme: Option<Scheme>, class: Option<ControlClass> },
}
```

A trait rather than a function, because this crate's tables are not always the authority; the answer
is a `ControlOrigin` rather than a `Control` for the same reason, and both variants answer `name()`
and `fallback_label()`. `BindingTable` is the implementation that answers from this crate's own
plans.

**A prompt is not a row of the settings screen.** `mappings` is what the game declared and is
static; a prompt is what would fire now, so it is empty for a context nobody is carrying or that is
switched off, and it *includes* a `private` binding. The lookup reads the compiled plan rather than
filtering the mapping list.

**Ranking.** Contexts come back in the order they get to claim a control — render tick before fixed
tick, then by priority, then declaration order — and within a context, in declaration order.
Nothing ranks one device above another; the device is a scope the caller supplies through
`PromptScope`, which narrows by context path, scheme and control class. `PromptDevice` is the
game-wide setting for which device a bare prompt speaks for, and the crate never defaults it.

**Consumption is read from the declarations, not from the frame** — the standing fact that a control
bound with `consume` in a stronger active context does not reach a weaker one. It moves only when a
context activates or deactivates.

**Staleness** is signalled by `PromptGeneration`, a counter bumped when a context activates or
deactivates and when an instance of a context arrives or goes away. It is written as an insert
rather than a mutable deref, so it can be read either by a `resource_changed` run condition or by an
observer. `activate`, `deactivate` and `PromptDevice` are public, so a game changing any of those by
hand bumps the counter itself.

### 9.3 Capture

Rebinding wants the control every other path through the crate discards, so capture reads the frame
directly rather than through a binding. A settings screen reached from the main menu therefore works
with no gameplay context spawned and no evaluator stepping.

`CaptureSession` is a component, placed on whatever entity the caller picks — usually the cell
button the player activated, so "which cell is listening" is answered by where the component is.

```rust
CaptureSession::for_mapping(&mapping)        // first slot
CaptureSession::for_slot(&mapping, 1)        // the secondary
CaptureSession::accepting(ControlClass::AnyButton)
    .within(Scheme::KeyboardMouse)
    .excluding([Control::Key(KeyCode::Escape)])
```

The crate answers with a `Captured` or `Refused` event on that same entity and removes the
component; removing it yourself cancels. It never touches the player or context entities.

A session skips whatever is already queued on its first run, so the press that opened it is not what
it binds. A slot past the mapping's capacity, or more than one past what it currently holds, is
refused — which keeps a capture from leaving a hole in a list whose order is what primary and
secondary mean.

**Three refusals, and one silent guard.**

| | |
| --- | --- |
| `RefusedReason::Reserved` | declared on a binding; loud, because the player meant to bind it |
| `RefusedReason::Shape` | the mapping cannot hold that kind of control |
| `RefusedReason::Scheme` | the control belongs to the other scheme |
| *excluded* | the screen's own controls; silent, so the key that cancels a capture still cancels it |

Reserved is asked before shape, so pressing the settings key hears that it is spoken for rather than
that its channel is wrong. Declaring a binding both `reserved` and `mappable` is a plan-build error.
A deliberate press is refused out loud; a continuous reading past its threshold (`DEFLECTION`,
`MOUSE_MOTION`) is dropped quietly, and both are claimed so neither also plays the game.

**Conflicts are detected, not resolved.** `conflicts(world, control, target)` is a pure query over
the mapping list, answerable before anything is committed; `conflicts_pending` asks the same of a
working copy of overrides, which is what lets a screen with unconfirmed choices tell whether two of
them clash. `Overlap` says whether the clash is `SameContext` or `OtherContext` — a clash across two
contexts is *possible* rather than certain, since whether they are ever live together is the game's
own question. Comparison is at control granularity, so two bindings differing only in their chords
are reported as overlapping. The whole target mapping is excluded rather than the one slot.

---

## 10. Overrides and persistence

An override set is a diff against the declared defaults. The crate defines the structure and knows
nothing about where it ends up.

```rust
pub enum Override {
    Controls(Vec<Control>),   // in slot order; replaces the mapping's whole list
    Cleared,                  // deliberately emptied — distinct from a missing row
    NotOurs,                  // an external authority owns this mapping
}
```

Rows are keyed by `(Scheme, MappingKey)` and tunables by `(Scheme, key)`. Nothing in an `Overrides`
names a device: what a player bound is a control on a device *class*, and which physical unit drives
which player is a separate question.

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
what was in flight and re-arms require-reset. Followers riding a row that changed move with it.
`AppliedPlan<C>` keeps the variant so an instance spawned later sees it too, and
`InputContextPlan<C>` is left untouched, so the next patch's revised defaults still reach a player
who never touched that row.

Three slot cases: a slot the defaults fill has its binding's source rewritten; a slot they left
empty is filled by *copying* the binding beside it, so a secondary carries the same modifiers and
conditions as the primary; and a slot the override no longer has takes its binding away. Copying
only works where a binding reads one control, so a row that is one part of a composite is refused a
slot the defaults did not ship.

**Overrides do not compose.** Each apply starts from the pristine declaration, so the argument must
be the *whole* working copy — a preset's rows and any manual captures together. A smaller second
call silently reverts every row it does not mention.

`apply_overrides_for` reaches one entity's own instance, so two occupants sharing a context type
apply and persist independently, with the world-wide plan untouched.

**Problems are reported, never dropped:**

```rust
pub enum OverrideProblemKind {
    NoSuchMapping, NotRebindable, WrongScheme { .. }, WrongShape { .. },
    Reserved { .. }, TooManyControls { .. }, CompositeCannotGrow, UnknownControl { .. },
}
```

### 10.2 Presets

A `Preset` is a name paired with an `Overrides`. `Preset::build(world, name, |p| ..)` gives it the
builder's own ergonomics — `p.bind::<A>(scheme, controls)` resolves `A`'s declared mapping in that
scheme and writes the row, so an app never derives a key by hand. It panics rather than guessing
when an action has no mapping in that scheme or more than one.

Passing a preset to `apply_overrides_with_preset` exempts exactly the rows that preset names from
the `NotRebindable` refusal — which is what lets a preset move a `Fixed` row a capture screen never
offers a button for. Every other refusal still applies. A preset is a starting point, not a layer:
selecting one writes its rows into the same working copy a manual capture writes into, and there is
no persisted "which preset is active" anywhere.

There is no crate-owned registry of presets. A game keeps its own list, as it keeps its own working
copy.

### 10.3 Serialization

Behind the `serialize` feature. `Overrides` itself cannot be the wire type: its fields hold a
`MappingKey`, constructible only from a `&'static str` the game already compiled in, which no
generic reflection walk can manufacture from loaded data. [`SavedOverrides`] is the portable shape
that stands in for it — plain, owned strings, needing no context to construct:

```rust
pub struct SavedOverrides {
    pub action_map_version: u32,
    pub bindings: BTreeMap<String, BTreeMap<String, SavedRow>>,   // scheme -> mapping -> row
    pub tunables: BTreeMap<String, BTreeMap<String, SavedTunableValue>>,
}

pub enum SavedRow { Controls(Vec<String>), Cleared, NotOurs }
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
"gameplay.move.up" = "key/KeyI"                  # a scalar is a one-element list
"ui.settings"      = "external"

[tunables]
```

A row holding one control writes as a bare scalar and reads back from either form; position in a
list is which slot, so a cleared middle slot needs `"cleared"` rather than a shortened list. The two
state words cannot collide with a control name, because the control encoding is a format this crate
owns rather than `Debug` or serde on Bevy's own types — an upstream rename becomes a compile error in
an exhaustive match while the stored string stays what it was. `bindings`/`gamepad` sorts ahead of
`bindings`/`keyboard_mouse` alphabetically rather than in `Scheme`'s own declared order, and an empty
`tunables` still gets a header — both accepted costs of a plain, structurally reflected type over a
hand-rolled one.

**`SavedOverrides` claims no field besides `action_map_version`, `bindings` and `tunables`, and none
of those is a bare `version`** (R17.10, D59). A settings layer that lets several resources share one
TOML table by name can put `SavedOverrides`'s fields beside an unrelated struct's, so the one field
likely to collide with something else's is the namespaced one.

**`save_overrides`/`resolve_saved` are the pure functions** in and out of this shape.
`resolve_saved` resolves a saved mapping name against what the game currently declares — a
`MappingKey` can only ever be one already declared — and reports an `UnresolvedMapping` or
`UnresolvedTunable` rather than dropping either in silence. A renamed action's row is dropped on the
next save rather than preserved unresolved.

`action_map_version` is checked before any row is read: a `SavedOverrides` naming a version this
build never shipped is refused as a whole (`UnsupportedVersion`) rather than resolved as the one
version that exists today. There is no migration path yet, because there has never been a second
version for one to convert from.

---

## 11. Crate layout

One crate, feature-gated by input source, plus the proc-macro crate Rust requires for the derives.
The macro crate is re-exported, so nothing names it.

```
src/
  device.rs      L0  device handles, pairing sets, gamepad calibration
  frame.rs       L1  the event queue, sampling, retirement
  action.rs          identity, intent, channel shape, value, phase, scratch
  binding.rs         controls, sources, composites, modifiers, the context builder
  condition.rs       conditions and their verdicts and descriptors
  context.rs         declaring a context, its per-entity state, the reading params
  plan.rs            compilation, slot allocation, diagnostics
  eval.rs            the evaluator, consumption, the exclusion ceiling, dispatch
  event.rs           Fired/Started/Completed/Canceled, class bindings
  player.rs          the Paired component
  join.rs        L3  is_claimed
  mapping.rs         mappings, slots, capacity, tunables
  overrides.rs       the diff structure, applying it, serialization
  preset.rs          a named Overrides and its builder
  capture.rs         capture sessions, reserved controls, conflict detection
  present.rs         control naming, prompts, prompt scope and staleness
  inspect.rs         the type-erased read of contexts and actions
  backend.rs         reserved for the source and authority backend traits — a stub today
  focus.rs           reserved for bevy_input_focus integration — a stub today
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
| `bevy_reflect` | reflection, and with it serialization of custom modifiers | yes |
| `serialize` | persistence of overrides and input frames; pulls in `bevy_reflect` | no |
| `state` | a context's activation following a `bevy_state` state | yes |
| `focus` | the `bevy_input_focus` dependency | no |

There is no `scene`/BSN feature. Transitions are plain `EntityEvent`s, so `bsn!` attaches observers
with no adapter and no dependency in either direction.

The crate is `no_std` + `alloc` and `forbid(unsafe_code)` throughout.
