# Choosing an input crate

`bevy_action_map` (this crate), [`bevy_enhanced_input`][bei] (BEI), and
[`leafwing-input-manager`][lwim] (LWIM) all turn hardware into named game actions. This document is
for someone deciding between them. It is written to be checkable: every behavioural claim names the
API or the source file it came from, so a maintainer of any of the three can correct it against the
code rather than against an impression.

## Status, before anything else

**This crate is not ready to be chosen today.** It is unpublished and has one author. It targets a
Bevy release candidate rather than a release, and it has never shipped a game. BEI and LWIM are
published and maintained, they track Bevy releases, and real games use them. If you are starting a
project this week, that difference outweighs every technical one below, and the recommendation is
BEI.

So what follows is not a case for switching. It is a map of where the three crates differ, which is
useful whichever you choose. It is also the reason this crate exists.

## What was examined

| | Version | Bevy | Read at |
| --- | --- | --- | --- |
| `bevy_enhanced_input` | 0.26.0, and `main` @ `f42a68a` | 0.19 | crates.io source; GitHub |
| `leafwing-input-manager` | 0.21.0 | 0.19 | crates.io source |
| `bevy_action_map` | unpublished, commit `c271ba4` | 0.20.0-rc.1 | this repository |

Two of the three target Bevy 0.19 and one targets 0.20. A few differences below come partly from the
Bevy versions rather than the crates, and are marked where they do. BEI's `main` is cited only where
it has changed something since 0.26.0, and is marked as such. All three were read on 2026-08-31, and
BEI's `main` and this crate were read again on 2026-09-23.

## The short answer

- **Most games: BEI.** It is the most complete of the three, it is maintained, its model (actions,
  bindings, contexts, modifiers, conditions) is the one this crate also uses, and it is the one Bevy
  is looking at for a future first-party input abstraction.
- **A small game, or one that wants the smallest possible model: LWIM.** One enum, one input map,
  `just_pressed`. It is much less machinery, and for a jam game or a prototype that is the right
  amount.
- **This crate**, once it is released, is for games where the *player-facing* half of input is a
  requirement rather than a nice-to-have. That means a rebinding screen that detects conflicts,
  on-screen prompts that stay correct, devices paired to players, and saved settings that survive a
  patch changing the defaults. The crate is built around those, and most of the differences below
  are about them.

## One vocabulary, three shapes

All three crates share the same core idea: the game reads `Jump`, not `KeyCode::Space`. BEI and this
crate also share Unreal's vocabulary. Where they differ is what kind of *thing* each concept is:

| | LWIM | BEI | `bevy_action_map` |
| --- | --- | --- | --- |
| An action is | a variant of your `Actionlike` enum | an entity with an `Action<A>` component | a type implementing `InputAction` |
| A binding is | a boxed `dyn Buttonlike` in an `InputMap<A>` component | an entity with a `Binding` component, related to the action | an entry in a plan compiled once at app build |
| A context is | — (one `InputManagerPlugin<A>` per enum) | any component, registered with `add_input_context` | a type implementing `InputContext`, used as a component |
| State lives | in an `ActionState<A>` component | in components on each action entity | in dense per-instance tables on the context entity |
| Bindings are declared | by building an `InputMap` value | by spawning entities (`actions!` / `bindings!` macros) | in a closure passed to `add_context` |

In BEI, actions and bindings are entities, and most of what it is good at follows from that. A scene
file can carry a whole input map. A third-party crate can add an action to a context it does not
own. Change detection works per action for free. An inspector shows a binding's modifiers and
conditions as ordinary components, so a developer can retune a dead zone or a hold time in a running
game and feel the result at once.

In this crate, a context's bindings are compiled once into an immutable plan, and most of what it is
good at follows from that instead. The plan is complete when it is built, so it can be checked as a
whole before the game runs. The declared bindings stay as they were when a player rebinds, as a
baseline the rebind is applied over. Evaluating a context is a walk over arrays with no allocation.
What a plan cannot do is show up in an inspector the way an entity does. A running game can change
only the values a binding declares as tunables (section 8); any other change to a modifier or
condition means a rebuild.

Neither model is simply better. They buy different things, and sections 8, 9 and 10 below are where
the difference stops being a matter of taste.

One row is missing from that table on purpose, because it is the one nobody thinks about until a
refactor: **what identifies an action in a player's saved settings**. LWIM's answer is the enum
variant, BEI leaves it to the game, and this crate's is a declared string that is deliberately
neither the variant nor the type. Section 9 has it.

---

## What it looks like

The same small game in each crate: `Jump` on Space or gamepad South, `Move` on WASD or the left
stick, and a pause menu whose `Confirm` is also on South, which gameplay must not hear while the
menu is up. The three share a state and a player component, and imports are left out:

```rust
#[derive(States, Default, Clone, PartialEq, Eq, Hash, Debug)]
enum GameState { #[default] Playing, Paused }

#[derive(Component)]
struct Player;
```

**LWIM**, after its `default_controls` example:

```rust
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
enum PlayerAction {
    Jump,
    #[actionlike(DualAxis)]
    Move,
}

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
enum MenuAction { Confirm }

app.add_plugins((
    InputManagerPlugin::<PlayerAction>::default(),
    InputManagerPlugin::<MenuAction>::default(),
))
.add_systems(Update, (
    jump.run_if(in_state(GameState::Playing)),
    confirm.run_if(in_state(GameState::Paused)),
));

// at spawn
let mut player = InputMap::default();
player
    .insert(PlayerAction::Jump, KeyCode::Space)
    .insert(PlayerAction::Jump, GamepadButton::South)
    .insert_dual_axis(PlayerAction::Move, VirtualDPad::wasd())
    .insert_dual_axis(PlayerAction::Move, GamepadStick::LEFT);
commands.spawn((Player, player));

let mut menu = InputMap::default();
menu.insert(MenuAction::Confirm, KeyCode::Enter)
    .insert(MenuAction::Confirm, GamepadButton::South);
commands.spawn(menu);

fn jump(actions: Single<&ActionState<PlayerAction>, With<Player>>) {
    if actions.just_pressed(&PlayerAction::Jump) { /* ... */ }
}
```

**BEI**, after its crate docs and `state` module:

```rust
#[derive(InputAction)]
#[action_output(bool)]
struct Jump;

#[derive(InputAction)]
#[action_output(Vec2)]
struct Move;

#[derive(InputAction)]
#[action_output(bool)]
struct Confirm;

#[derive(Component)]
struct PauseMenu;

app.add_plugins(EnhancedInputPlugin)
    .add_input_context::<Player>()
    .add_input_context::<PauseMenu>()
    .sync_context_to_state::<Player, GameState>()
    .sync_context_to_state::<PauseMenu, GameState>()
    .add_observer(jump);

// at spawn
commands.spawn((
    Player,
    ActiveInStates::<Player, _>::single(GameState::Playing),
    actions!(Player[
        (Action::<Jump>::new(), bindings![KeyCode::Space, GamepadButton::South]),
        (
            Action::<Move>::new(),
            DeadZone::default(),
            Bindings::spawn((Cardinal::wasd_keys(), Axial::left_stick())),
        ),
    ]),
));
commands.spawn((
    PauseMenu,
    ActiveInStates::<PauseMenu, _>::single(GameState::Paused),
    actions!(PauseMenu[
        (Action::<Confirm>::new(), bindings![KeyCode::Enter, GamepadButton::South]),
    ]),
));

fn jump(jump: On<Start<Jump>>) { /* jump.context is the player */ }
```

**This crate**, after its `minimal` example and Disasteroids' menu:

```rust
#[derive(InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
struct Jump;

#[derive(InputAction)]
#[action(path = "gameplay.move", output = Vec2, intent = Directional2)]
struct Move;

#[derive(InputAction)]
#[action(path = "menu.confirm", output = bool, intent = Button)]
struct Confirm;

#[derive(InputContext)]
#[context(path = "gameplay.on_foot", tick = Render)]
struct OnFoot;

#[derive(InputContext)]
#[context(path = "menu.pause", tick = Render, priority = 10, exclusive)]
struct PauseMenu;

app.add_plugins(ActionMapPlugin)
    .add_context::<OnFoot>(|context| {
        context.bind::<Jump>(KeyCode::Space);
        context.bind::<Jump>(GamepadButton::South);
        context.bind::<Move>(DirectionalButtons::wasd());
        context.bind::<Move>(Stick::Left).dead_zone(DeadZone::radial(0.15));
    })
    .add_context::<PauseMenu>(|context| {
        context.active_in_state(GameState::Paused);
        context.bind::<Confirm>(KeyCode::Enter);
        context.bind::<Confirm>(GamepadButton::South);
    })
    .add_systems(Update, jump);

// at spawn
commands.spawn((Player, OnFoot));
commands.spawn(PauseMenu);

fn jump(input: ContextActions<OnFoot>) {
    if input.fired::<Jump>() { /* ... */ }
}
```

What each asks of you, read off the snippets:

**LWIM** is the least to write. Each enum is a derive and a plugin, and the bindings are a value you
build wherever is convenient. LWIM has no notion of a context being switched off, so pausing is done
with a run condition on the reading system. South still sets both `Jump` and `Confirm`; `jump` just
does not run.

**BEI** puts the bindings on the entity, as components in its spawn. That means two players can hold
different bindings with nothing extra, and a scene can carry them. The price is the nesting of
`actions!` and `bindings!`. Binding `Move` to keys and a stick also takes a preset such as
`Cardinal::wasd_keys()`; written out by hand, each key needs its own `SwizzleAxis` and `Negate`.

Code completion in your IDE helps less with BEI. A binding's modifiers and conditions are components
in a tuple, and a tuple accepts any bundle, so completion cannot narrow the list to the ones that
make sense there. The bracketed `Player[...]` syntax inside `actions!` is macro input, which an IDE
cannot complete inside at all. For pausing, each pair of context type and state is registered for
syncing, and gameplay stops hearing South because of its own `ActiveInStates`, not because the menu
took it.

**This crate** declares bindings once per context type, when the app is built, and an entity gets
them by carrying the context component. There is more to write up front than in either of the
others: every action names a `path`, an `output` and an `intent`, and every context a `path` and a
`tick`. The path is what a settings file stores. Those attribute arguments do not complete in an IDE
either, but everything after `bind` is a method on a builder, so completion lists exactly the
modifiers and conditions a binding can take. The menu is `exclusive`, which silences gameplay while
the menu is up whatever either context binds, so gameplay needs no state of its own. Bindings that
differ per player are an override applied to one entity with `apply_overrides_for`, not a second
declaration.

---

## 1. What the crate reads: levels or edges

This is the difference the rest of the timing story follows from, so it goes first.

| | Reads |
| --- | --- |
| LWIM | `ButtonInput<KeyCode>`, `ButtonInput<MouseButton>`, `Gamepad` components, `AccumulatedMouseMotion` — sampled into a `CentralInputStore` resource in `PreUpdate` (`src/user_input/keyboard.rs`, `updating.rs`) |
| BEI | the same resources, sampled inline by an `InputReader` system param during evaluation (`src/context/input_reader.rs`) |
| this crate | the `KeyboardInput`, `MouseButtonInput`, `MouseMotion` and `RawGamepadEvent` message streams, into a timestamped queue each context reads from its own cursor (`src/frame.rs`) |

Bevy's `ButtonInput` is a **level**: `keyboard_input_system` clears it each frame and replays that
frame's events into it, so a key pressed *and* released within one frame leaves `pressed()` false.
LWIM reads `get_pressed()` and `get_just_released()`, and BEI 0.26.0 reads `pressed()`, so a tap
shorter than one render frame is invisible to both. BEI's `main` now reads `just_pressed()` as well
([bevy_enhanced_input#351](https://github.com/simgine/bevy_enhanced_input/pull/351)), so there a
single tap is seen, as a press lasting one evaluation. What a level still cannot carry is more than
one edge per frame: two taps in a frame are one, and the order of presses within it is gone. This
crate sees each edge as a logged transition:

```rust
// bevy_action_map, src/context/state.rs — both edges written in one frame
world.write_message(press(KeyCode::Space, ButtonState::Pressed));
world.write_message(press(KeyCode::Space, ButtonState::Released));
app.update();
assert_eq!(heard, ["fired", "completed"]);   // two observer calls, in order
```

**Whether this matters depends on the game.** At 60 Hz a frame is under 16 ms, and most games never
notice what happens inside one. A fighting game does, because the order and count of presses within
a frame are its input. It also matters more when input is read on a fixed tick rather than the
render frame, which the next section covers.

Even here, polling recovers only part of it. `phase::<A>()` returns one `ActionPhase` per read, so a
tap shorter than a tick polls as `Completed`. Both transitions are seen by an observer
(`On<Fired<A>>` and `On<Completed<A>>`) or in the transition log, but not by polling.

This crate reads Steam Input by polling it once a frame, so an action bound to it gets the frame's
resolution and no finer. Steam's API also has action event callbacks, which may report each change
between polls, but the `steamworks` Rust crate does not wrap them, and whether they carry every edge
is unmeasured. Even without edges from Steam, the queue is still worth having: it keeps fixed ticks
correct at any frame rate, keyboard and mouse never pass through Steam Input, and replay and device
routing are built on it.

Reading edges is also what gives sections 6 (dead zones), 7 (device routing) and 10 (replay) the
shape they take here.

## 2. Fixed timestep

All three crates have a story here.

**BEI** lets a context name the schedule it is evaluated in:

```rust
app.add_input_context_to::<FixedPreUpdate, Player>();
```

Its consumed-input set is keyed by schedule `TypeId` and cleared when that schedule runs, so a
`FixedPreUpdate` schedule running several times in one frame gets an independent consumption
decision each time, while still seeing what `PreUpdate` consumed. Events fire once per schedule run
(`src/context.rs`, `src/context/input_reader.rs`). This crate reached the same arrangement
independently and for the same reason; [decisions.md](./decisions.md) D12 credits BEI for it.

**LWIM** keeps *two* `ActionState`s per entity and swaps between them: `swap_to_fixed_update` runs
in `RunFixedMainLoop::BeforeFixedMainLoop`, the fixed state is updated once per frame there, and
`swap_to_update` restores the render one afterwards (`src/plugin.rs`, `src/action_state/mod.rs`). A
`just_pressed` read from `FixedUpdate` therefore stays true for every fixed tick of that frame, not
only the first, which is usually what a fixed-tick reader wants.

**This crate** makes the tick domain a property of the context type (`#[context(tick = Fixed)]`),
and evaluates each context in that domain alone. Each instance reads the timestamped event queue
from its own cursor, so a fixed tick sees every event sampled since the last one ran.

Where they differ is **what happens to input in the gaps**:

| | A tap shorter than one render frame | Zero fixed ticks in a frame | Several fixed ticks in a frame |
| --- | --- | --- | --- |
| LWIM | lost (level sampling) | held state carries to the next tick that runs; a tap is lost | every tick reads the same frame's state |
| BEI | lost in 0.26.0; on `main`, seen as a one-evaluation press, with two taps counting as one | held state carries to the next tick that runs; a tap is lost | every tick reads the same frame's state; consumption is per-run |
| this crate | preserved, as two transitions | its events wait in the queue for the next tick that runs | the first tick reads the frame's events, later ones the held state it left; no event seen twice, none skipped |

The cost of the last row is stated in [decisions.md](./decisions.md) D9 and is real: an action
needed at both rates must be declared in two contexts, because a context is evaluated in exactly one
domain.

## 3. Arbitration: chords, priority, and consumption

Three mechanisms, and the crates draw the line between "automatic" and "declared" in different
places.

**Longer chord beats shorter.** `Ctrl+S` should save without also moving the character down.

- **LWIM** does this automatically and by default: `ClashStrategy::PrioritizeLongest` compares
  `BasicInputs` decompositions and suppresses any action whose inputs are a strict subset of
  another's (`src/clashing_inputs.rs`). It applies within one `InputMap`; two different `Actionlike`
  enums do not clash-resolve against each other.
- **BEI** orders actions within a context by the maximum modifier-key count of their bindings, so
  `Ctrl+S` is evaluated before `S` (`src/context.rs`). But ordering only decides *who goes first*;
  suppression requires `ActionSettings { consume_input: true }`, which is **off by default**. With
  defaults, pressing Ctrl+S fires both. The ordering also only understands `ModKeys` (Ctrl, Shift,
  Alt, Super). A general chord is the separate `Chord` condition, which references another action.
- **This crate** does it automatically, for any chord. When a context's bindings are compiled, each
  binding learns which longer chords share its controls, and while one of those is held, the shorter
  binding reads as rest. Consumption is not involved (`src/plan.rs`, `src/eval/`).

**One context taking a control from another.** A pause menu should stop the ship hearing Escape.

- **LWIM** has no context concept. You disable actions (`ActionState::disable`) or add run
  conditions.
- **BEI** has `ContextPriority<C>` (a `usize`, default 0; ties broken by reverse spawn order), and
  `consume_input` per action. Consumption is global across contexts within a schedule.
- **This crate** has `PRIORITY` as a const on the context type, turned into ordered system sets when
  the app is built rather than sorted every frame. `CONSUMES` is set per action and can be
  overridden per binding. A context can also be `EXCLUSIVE`, which makes every lower-priority
  context inactive while it is up, so a modal screen does not have to list the actions it takes
  over.

BEI's arrangement is more flexible at runtime, since priority is a component you can change. This
crate's is fixed when the app is built and is the same for every entity, which is a real limitation
if you want two players' contexts at different priorities.

**A difference that only shows in local multiplayer.** BEI's consumption table does not record whose
claim it was, so when one player's menu consumes a button, it takes it from every player. BEI scopes
its gamepad reads to a context, but not its consumption. This crate scopes both. Consumption and
exclusivity each carry the devices of the instance responsible, and affect only contexts that share
one of those devices.

## 4. Combining several bindings into one action

`Jump` on both Space and gamepad South; `Move` on both WASD and the left stick. What is the value
when two contribute at once?

- **LWIM** resolves per input kind; buttonlike actions are pressed if any input is pressed.
- **BEI** takes the contributions with the most significant `TriggerState` and combines them by
  `ActionSettings::accumulation`: `Cumulative` (sum, the default) or `MaxAbs`.
- **This crate** chooses the rule from the action's declared **intent**, which BEI and LWIM do not
  have. A `Button` takes the strongest contribution. An `Analog1` or `Directional2` takes the
  strongest in each direction on each axis, so opposite directions cancel. A `Delta2` sums.

Intent exists because a stick and a mouse have the same shape, `Vec2`, but summing is right for one
and wrong for the other. A mouse delta is a movement that has already happened, so two devices
moving at once should both move you. Two half-deflected sticks are not a full deflection. Intent
also lets the crate *refuse* a binding whose control cannot serve the action, such as a stick bound
to a `Delta2` look action. The mistake is caught when the context is declared, rather than felt
later as camera drift.

The cost is one more thing to declare per action, and one case gets harder rather than easier. A
single action driven by *both* a mouse and a stick needs the stick's rate converted to a movement
explicitly, with `.per_second()`.

## 5. Conditions and modifiers

Broadly comparable between BEI and this crate, and much richer in both than in LWIM.

| | LWIM | BEI | this crate |
| --- | --- | --- | --- |
| Press / release / down | `just_pressed` / `just_released` / `pressed` | `Press`, `Release`, `Down` | press, release, down |
| Hold for a duration | via `timing` feature's `current_duration`, by hand | `Hold`, `HoldAndRelease` | `.hold(t)`, `.hold_once(t)`, `.hold_and_release(t)`, with progress reported |
| Tap, multi-tap | — | `Tap`, `Combo` | `.tap(t)`, `.multi_tap(n, gap)` |
| Repeat / pulse | — | `Pulse` | `.pulse(t)` |
| Toggle, cooldown, block-by | — | `Toggle`, `Cooldown`, `BlockBy` | `hold_or_toggle` (a player setting), no cooldown |
| Flick | — | `Flick` | — |
| Combining conditions | — | `ConditionKind` (implicit / explicit / blocker) | `ConditionKind` (implicit / explicit / blocking) |
| Modifiers | `AxisProcessor` chain: dead zone, bounds, sensitivity, inversion | negate, scale, clamp, swizzle, dead zone, exponential curve, linear step, smooth nudge, delta scale, accumulate-by | negate, scale, clamp, swizzle, dead zone, response curve, rate conversion, compass rounding |
| Third-party extension | `dyn` trait objects, registered | `add_input_condition` / `add_input_modifier`, as components | enum with a `Custom(Arc<dyn …>)` arm |
| Attached at | the input | the binding **or** the action | the binding **or** the action (`combined::<A>()`) |

Both BEI and this crate can attach modifiers and conditions after the bindings are combined, which
is what makes BEI's `Cardinal::wasd_keys()` + action-level `DeadZone` idiom work. Here the
action-level builder carries scale, clamp, curve and the like, and conditions such as `.pulse(t)`
for menu repeat, but not a dead zone: a dead zone belongs to one physical control, so it stays on
the binding.

BEI has more conditions than this crate, `Flick` and `Cooldown` in particular.

## 6. Dead zones

This is where reading raw events pays off. It is also where the difference is easiest to overstate,
so precisely:

Bevy applies a per-axis `GamepadSettings` filter to gamepad values before they reach the `Gamepad`
component. BEI and LWIM both read the `Gamepad` component, so they consume whatever that filter
produced and apply their own `DeadZone` / `AxisDeadZone` on top of it. This crate reads
`RawGamepadEvent`, which is emitted before that filter, and owns the whole chain.

The filter itself has changed between Bevy versions. On Bevy's `main`, the `Gamepad` component
stores the raw value unscaled, and the dead zone is applied only to the change-detection threshold
and to the scaled value in the emitted event. So whether BEI reads an already dead-zoned value
depends on the Bevy version; it was more true of older ones. What holds in every version is that BEI
and LWIM read a value whose filtering policy Bevy has already chosen, and this crate reads the value
before any policy is applied.

The crate splits dead-zone handling into three stages, because three parties have a say in the
number and each is answering a different question ([design.md](./design.md) TD8.4):

1. **Calibration.** Where this particular pad's stick actually rests, and how far it wanders there.
   The game sets it, and it is applied as the event is recorded. It is per device, because drift is
   wear on one pad.
2. **Design.** The shape and curve the mechanic wants. This is the stage that rescales, so full
   deflection still reads 1.0.
3. **Preference.** The player's own adjustment to stage 2.

Only one stage may rescale, and that is checked when the plan is compiled. Neither BEI nor LWIM
separates these. Both have "a dead zone", which is stage 2.

You need stages 1 and 3 only if you ship a settings screen with a dead-zone slider, or your players
have worn sticks. Many games do neither, and for them this is machinery for nothing.

## 7. Local multiplayer and device routing

- **LWIM**: `InputMap::with_gamepad(entity)` associates one map with one gamepad. Keyboard and mouse
  are global.
- **BEI**: a `GamepadDevice` component on the context entity, which is `Any`, `Single(entity)` or
  `None`. Keyboard and mouse are global.
- **This crate**: a `Paired(DeviceHandleSet)` component naming the devices one player owns. A
  `DeviceHandle` is `KeyboardMouse` or `Gamepad(Entity)`, so keyboard and mouse can go to one player
  and a pad to another. Input from devices a player does not own is filtered out before anything
  reads it.

  It also has a **join gesture**: a context bound to a *class* of control rather than a specific
  one, so "press anything to join" claims whichever device pressed. Two waiting slots cannot both
  claim the same device (`examples/split_friction/`).

Routing keyboard and mouse is the real difference. The join gesture is the part that is tedious to
write yourself. This crate treats keyboard and mouse as one device, so it cannot tell two keyboards
apart, but neither can Bevy.

## 8. Rebinding, and what a settings screen needs

This crate was built for this, so the gap is widest here, and it is easiest to overstate what the
other two lack. What they have comes first. BEI treats the settings screen as the game's job rather
than the input crate's, and what follows is what that leaves a game to write.

**What BEI has today.** Bindings are entities with a `Binding` component, so a settings screen can
query them, and rebinding is despawning one and spawning another. `Binding` implements `Display`
("Control + KeyD", "Mouse Left", "Scroll Wheel"), so a screen can render a binding as a string
without writing a match. `Binding` is `Serialize`/`Deserialize` under the `serialize` feature. That
is a real, workable basis for a rebinding UI, and a game can build one on it.

**What LWIM has today.** `InputMap` is a mutable component with `get_buttonlike`, `insert`,
`remove_at`, `clear_action` and iteration over every binding, serializable via `serde` and typetag,
and loadable as an asset. Same story: a workable basis, rendering left to you.

**What neither has**, and what this crate treats as first-class:

- A **presentation model separate from the binding model**. Dead zones and response curves are the
  developer's concern, and a player rebinding "Thrust" should not see them. Marking a binding
  `.mappable()` puts it in a smaller model of its own: a named *mapping* with an ordered list of
  slots, such as "Primary" and "Secondary", which is what a primary/secondary table shows. Every
  binding is *listed* for the player to read, but only the ones declared mappable can be changed.
- **Interactive capture**, which listens for the next control the player presses, with controls that
  are reserved or excluded from it.
- **Conflict detection** that runs on a working copy before it is applied, so a screen with
  unconfirmed choices can tell whether two of them clash. A new choice can take its control away
  from whatever held it.
- **Overrides as a diff, not a replacement.** The declared bindings stay intact, and a rebind is
  applied over them as a patch. So when a game update revises a default, the new default still
  reaches a player who never changed that row. In BEI and LWIM the live bindings *are* the record.
  Unless the game keeps its own copy of the defaults, a saved input map replaces them wholesale, and
  a revised default reaches nobody who has ever saved.
- **Prompts that stay true.** A lookup from an action to the controls it is bound to, available as a
  text span in a template and updated when a rebind changes the answer. A pad's buttons are named
  the way that pad names them: "Cross" on a DualSense, "A" on an Xbox pad.
- **Shared controls declared as shared.** Tap to dodge and hold to sprint can share one control.
  Rebinding one moves both, and the second is drawn as a line under the first rather than as a row
  of its own.
- **Tunables.** A named, typed value the player can adjust, which replaces one setting of one
  modifier. It is listed and saved the same way a mapping is.
- **Presets.** A named set of overrides the game ships, applied the same way a rebind is. A preset
  may move rows that a capture screen would refuse to.

None of this is exotic. It is what a shipped game's controls screen needs, and it is normally
written by hand for each game. Whether that makes a different crate worth it depends on whether you
were going to write it.

**Glyphs**, the button images in a prompt, are partly here. This crate works out which image a
prompt should draw, trying a pad brand's own art before generic art, and passes along an image Steam
supplies (`resolve_glyph`). It ships no art: the game provides its own. BEI and LWIM stop at text.

## 9. Persistence

- **LWIM**: `InputMap` derives `Serialize`/`Deserialize`; typetag handles the boxed trait objects.
  Also loadable as an asset.
- **BEI**: `Binding` and `ActionSettings` are `Serialize`/`Deserialize` under the `serialize`
  feature; conditions and modifiers are components, so reflection-based scene serialization is the
  route.
- **This crate**: `Overrides`, the diff rather than the map, serializes through `serde`. The format
  is written by hand so that a row with one control saves as a single value, and a golden TOML
  document in the tests holds it steady. Loading matches each saved mapping name against what the
  game declares now, and *reports* a name or a control it does not recognize rather than dropping it
  silently.

Two structural differences sit underneath those, and both are invisible until the game changes.

**The first is the one from section 8**: serializing a diff against a declared baseline behaves
differently across a game update than serializing the map.

**The second is what a saved row is keyed on.** A settings file has to name the action a binding
belongs to, and the three crates name it differently:

| | The name in saved data | A Rust rename | A module move |
| --- | --- | --- | --- |
| LWIM | the enum variant, via serde on `HashMap<A, _>` | orphans that action's bindings | harmless to serde; breaks a reflect-based save |
| BEI | whatever the game's own settings struct uses; BEI saves nothing itself, and the only name it gives an action is `any::type_name::<A>()` | harmless if the game chose its own keys; orphans them if keyed on the type | the same |
| this crate | the action's declared `PATH`, a string separate from the type | harmless | harmless |

`#[action(path = "gameplay.jump")]` exists for exactly this. The path is a name that lives outside
your code, so `Move` can become `MoveOnFoot` and move to another module without a player losing what
they bound to it. The convention is `<namespace>.<name>`. The discipline is that the path does
**not** follow the type: changing a path is a save-data migration, not a refactor. The path is also
the localization key for the row's label on a controls screen, which is why every action must
declare one. If a path does change, loading reports the saved one it cannot match, as above.

The cost is one more thing to declare per action, and a convention to keep. You can still rename the
path along with the type and get exactly LWIM's behaviour. What it buys is a safe default: a
refactor costs the player nothing, and breaking their settings takes a deliberate act.

**All three leave writing the bytes to a file to the app.** None of them is a settings-file crate.

## 10. Determinism, replay, and rollback

- **LWIM** has the most mature answer of the three today: `ActionDiff` streams and
  `generate_action_diffs`, plus `InputManagerPlugin::server()` which processes no input at all and
  expects `ActionState` to be supplied. This is designed for netcode and is used for it.
- **BEI** has `ActionMock` (drive an action's value and state for a span, skipping bindings),
  `ExternallyMocked` (a marker that excludes an action from evaluation entirely so you can write its
  data yourself), and `CustomInput`/`CustomInputs` (a resource of `ActionValue`s that bindings can
  read, for inputs Bevy does not model). Between them these cover testing, cutscenes, AI, and
  network-replicated input, and games ship rollback netcode on them.
- **This crate** splits the two jobs. Local replay and determinism tests use the *input frame*: an
  object holding one frame's raw input, which can be built by hand and serialized. Everything the
  crate computes is a function of the frames it is given, so a replayed frame passes through
  conditions, chords, consumption and contexts again, which a mock at the action level skips.

  A network peer uses the *authority* binding instead, the same entry point Steam Input uses
  ([decisions.md](./decisions.md) D22, D51). It supplies an action's finished value rather than raw
  input, so two peers never need the same bindings. That is the job LWIM's `ActionDiff` and BEI's
  `ActionMock` do, not a lower-level alternative to them.

The input frame is built, and this crate's own tests drive it, but a recorder and a replay backend
on top of it are not. The network half is **designed and not proven**. No testbed here sends
anything over a wire, and [deferred.md](./deferred.md) X35 records that, waiting on a networked game
to try it. The entry point it would use is built: an `Authority` binding and `AuthorityValues`,
through which `examples/pong_robot` drives a paddle.

Mocking at the action level (BEI, LWIM) and replaying at the frame level (this crate, for local
determinism) are not the same test. The first tests your game logic; the second also tests your
bindings. For a live network peer, all three work at the action level, which is what netcode wants:
each peer has its own bindings.

## 11. Backends that own the bindings

Steam Input is the motivating case: the binding UI, the conflict rules and the glyphs all live
outside the game, and the platform answers "is Jump pressed" for you.

**BEI** can be driven by Steam today. `ActionMock` skips input reading, conditions and modifiers,
and reports a value and state you supply. The transition events still fire, so observers cannot tell
where the value came from. A context fed by Steam can set `GamepadDevice::None` so that it does not
also read the pad through Bevy. The rest of the integration is left to the game: names for the
actions in Steam's manifest, prompts built from Steam's description of the controls, and
controls-screen rows that Steam rebinds rather than the game.

**This crate** has the same kind of value path, an `Authority` binding and `AuthorityValues`. The
backend writes a value, and the crate's own lifecycle turns it into events. The difference is scope.
The authority stands in for one device family rather than owning the whole action, so the keyboard
stays bound beside a pad that Steam drives. The game's own conditions, such as a rate of fire, run
on Steam's value as on any other.

It also covers what BEI leaves to the game. An action's declared path is its name in Steam's
manifest, and a dotted path has been checked to be a valid Steam action name. The prompt lookup
returns a `ControlOrigin`, which can describe one of Steam's controls as well as one of this
crate's. A game that also reads the pad through Bevy disables `GilrsPlugin` when Steam Input starts,
so the pad is not read twice. A Steam build of the `disasteroids` example runs on all of this,
though outside the workspace and outside any automated test. What a running Steam client actually
does is recorded in [steam.md](./steam.md).

So both can be driven by Steam, this crate also covers naming and prompts, and neither has shipped a
Steam game. Do not choose on this axis unless you are shipping on Steam Input, and if you are, check
the current state of both.

## 12. The rest

**UI focus.** BEI does not integrate `bevy_input_focus`; it offers an `ActionSources` resource so
you can switch whole input sources off while the UI is being used, with a worked example for
`Interaction`. LWIM reserves an `InputManagerSystem::Filter` system-set slot for a filter you write.

This crate does not depend on `bevy_input_focus` either. Instead it has a working *focus
orchestrator* for `bevy_ui_widgets`, which works out which widget an action was meant for and drives
that widget. The mapper underneath knows nothing about focus, and the widgets above know nothing
about devices, so the orchestrator is where the two meet.

`examples/common/widget_focus.rs` tags each widget with a *kind*, as a required component so that no
spawn site has to remember it. While a widget of that kind has focus, a context for the kind is
active, and it answers the keyboard **and the gamepad** through the same priority and consumption as
the rest of the game. The `disasteroids` example turns off Bevy's `InputDispatchPlugin` and lets the
orchestrator answer for both, so two mechanisms are not answering the same keys. The orchestrator
uses only the crate's public API (`add_context`, `active_if`, `bind`, `.consume()`).

It lives beside the examples rather than in the crate because of layering, not readiness. If an
input crate depends on `bevy_ui`, `bevy_ui` cannot depend on it, so the integration belongs in a
crate of its own. Until it has one, the examples share it as a `#[path]` import, and Disasteroids
exercises it end to end in place of a test of its own. Part of it may move upstream:
[bevy#25592](https://github.com/bevyengine/bevy/issues/25592) asks Bevy for an id for a widget's
kind.

What *is* still open here is narrower: making **unmodified** widgets respect consumption
generically, without declaring a context per widget kind. That is deferred, and the documented
advice is to use the orchestrator first.

**Keyboard layout.** BEI and LWIM bind `KeyCode`, which is a physical position. This crate binds
either a `KeyCode` or a `LogicalKey`, the character the player's own layout produces: movement wants
the first, so `WASD` keeps its shape on an AZERTY board, and a shortcut wants the second, so
`Ctrl+Z` reaches the key a French player reads as `z`.

**Diagnostics.** Two kinds, at two times.

*Before the game runs*, this crate checks each context whole, at the moment it is declared. A
declaration is finished: `add_context` receives every binding the context will have, so a check can
compare bindings with each other as well as inspect each one. A context that cannot work as written,
such as one binding a stick to a mouse look, is refused. One that will work but probably not as
meant, such as one reading the same control twice, draws a warning. The same pass runs on bindings
the game has no intention of installing, which is how a controls screen checks a player's
unconfirmed choices for conflicts.

BEI cannot check this way, and the reason is its model rather than an omission. Its bindings are
entities, and any system may spawn another one under an action on any frame, so a BEI context is
never finished: there is no moment at which its bindings are known to be complete, and a check that
compares them has no set to compare. Where a value has the wrong dimension, BEI converts it rather
than refusing it, so a mistake shows up in play.

*While it runs*, this crate has `why_not::<A>()`, which answers "why didn't this fire?" by naming
what stopped it. The answer might be that the context is inactive, or that a longer chord took the
control. BEI's answer is `RUST_LOG=bevy_enhanced_input=debug`, which is less structured but costs
nothing to turn on and covers a lot. LWIM has no equivalent of either.

**Build surface.**

| | `no_std` | Depends on | Feature gates |
| --- | --- | --- | --- |
| LWIM | no | `bevy` umbrella, subset of features | mouse, keyboard, gamepad, picking, asset, timing |
| BEI | **yes** | `bevy` umbrella, `default-features = false` | reflect, state, serialize |
| this crate | **yes** (`alloc` only) | Bevy *subcrates* individually | std, libm, keyboard, mouse, gamepad, touch, bevy_reflect, serialize, state |

BEI and this crate both support `no_std`, so that does not separate them. What does is the
dependency shape. This crate depends on `bevy_ecs`, `bevy_input`, `bevy_math` and the others
individually rather than on the `bevy` umbrella. That matters mainly if you want the smallest
dependency graph, or care about the crate moving into Bevy one day.

**Touch.** None of the three has touch bindings. This crate's `touch` feature exists but does
nothing yet.

**Mouse wheel.** BEI and LWIM have it. This crate does not yet (chunk 122 in Roadmap.md).

---

## What each crate is best at

**BEI: completeness and maintenance.** It has the largest set of conditions and modifiers, presets
that make common bindings one line, and an active maintainer. Its ECS model lets a scene author
input and a third-party crate extend it. It is the default answer.

**LWIM: smallness and netcode.** One enum, one input map, `just_pressed`, and the most mature
rollback support of the three. If you do not need contexts, conditions or a rebinding screen, the
other two are machinery you pay for and do not use.

**This crate: the player-facing half.** Its strengths are the rebinding model and the controls
screen built on it, overrides kept apart from the declared defaults, and prompts that name a pad's
own buttons. It also routes keyboard and mouse to a player as well as pads, binds keys by character
as well as position, and sees input edges inside a frame. Most of that is worth nothing if you never
build a controls screen, and none of it is released yet.

## If you are migrating

Concept-for-concept, BEI → this crate is close to mechanical:

| BEI | here |
| --- | --- |
| `#[derive(InputAction)] #[action_output(Vec2)]` | `#[derive(InputAction)] #[action(path = …, output = Vec2, intent = Directional2)]` |
| a context component + `add_input_context::<C>()` | `#[derive(InputContext)]` + `add_context::<C>(\|c\| …)` |
| `add_input_context_to::<FixedPreUpdate, C>()` | `#[context(tick = Fixed)]` |
| `actions!` / `bindings!` at spawn time | `c.bind::<A>(source)` at app build |
| `Fire<A>` / `Start<A>` / `Complete<A>` / `Cancel<A>` | `Fired<A>` / `Started<A>` / `Completed<A>` / `Canceled<A>` |
| `Query<&Action<A>>`, `ActionEvents` | `ContextActions<C>` with `value::<A>()` / `fired::<A>()` |
| `ContextPriority<C>` component | `PRIORITY` const on the context type |
| `ActionSettings { consume_input: true }` | `CONSUMES` on the action, or per binding |
| `ContextActivity<C>`, `ActiveInStates` | `active_if` / `active_in_state`, or `activate()`/`deactivate()` |

It is not mechanical in three places:

- An action must declare an **intent**.
- A context is evaluated in **one** tick domain, so an action needed at both rates is declared
  twice.
- Bindings are declared when the app is built rather than spawned, so code that changed binding
  entities at runtime becomes an override instead.

One thing to do deliberately rather than mechanically: **choose the paths, do not derive them.** The
obvious move when porting is `path = "jump"` for `struct Jump`. That makes the saved name a copy of
the type name, which throws away the reason the path exists (section 9). Pick the namespace and name
you would want in a settings file five years from now, because that is what they are. They are
cheapest to get right before anyone has saved one.

LWIM → either is a rewrite, because an enum of actions becomes one type per action.

## Corrections

If you maintain BEI or LWIM and something here is wrong, it is a bug in this document and I would
rather fix it than defend it. An earlier version of this comparison claimed neither crate addressed
fixed-timestep timing, which was false of both, and section 2 corrects it. Open an issue or mail the
author.

Deeper reasoning for this crate's side of each difference is in
[Requirements.md](../Requirements.md) (what must be true), [design.md](./design.md) (how it works)
and [decisions.md](./decisions.md) (why), with the current built/unbuilt line in
[Roadmap.md](../Roadmap.md).

[bei]: https://github.com/simgine/bevy_enhanced_input
[lwim]: https://github.com/Leafwing-Studios/leafwing-input-manager
