# Choosing an input crate

`bevy_action_map` (this crate), [`bevy_enhanced_input`][bei] (BEI), and
[`leafwing-input-manager`][lwim] (LWIM) all turn hardware into named game actions. This document is
for someone deciding between them. It is written to be checkable: every behavioural claim names the
API or the source file it came from, so a maintainer of any of the three can correct it against the
code rather than against an impression.

## Status, before anything else

**This crate is not a competitor you should pick today unless you are looking for one.** It is
unpublished, unreleased, has one author, targets Bevy `main` rather than a released version, and has
never shipped a game. BEI and LWIM are published, maintained, versioned against a Bevy release, and
in real games. If you are starting a project this week, that difference outweighs every technical
one below, and the honest recommendation is BEI.

What follows is therefore not "why you should switch". It is a map of where the three crates
genuinely differ — which is useful whichever way you decide, and which is also the argument for why
this crate exists at all.

## What was examined

| | Version | Bevy | Read at |
| --- | --- | --- | --- |
| `bevy_enhanced_input` | 0.26.0 | 0.19 | crates.io source |
| `leafwing-input-manager` | 0.21.0 | 0.19 | crates.io source |
| `bevy_action_map` | unpublished, commit `774162e` | `main` @ `19f6742` (0.20-dev) | this repository |

Two of the three target Bevy 0.19 and one targets `main`; a handful of differences below are partly
differences between those two Bevy versions rather than between the crates, and are marked where
that is so. Date of reading: 2026-08-31.

## The short answer

- **Most games: BEI.** It is the most complete of the three, it is maintained, its model (actions,
  bindings, contexts, modifiers, conditions) is the one this crate also uses, and it is the one Bevy
  is looking at for a future first-party input abstraction.
- **A small game, or one that wants the smallest possible model: LWIM.** One enum, one component,
  `just_pressed`. It is much less machinery, and for a jam game or a prototype that is the right
  amount.
- **This crate**, if and when it is real, is for games where the *player-facing* half of input is a
  requirement rather than a nice-to-have: a rebinding screen with conflict detection, on-screen
  prompts that stay correct, per-player device pairing, and a persistence format that survives a
  patch changing the defaults. That is what it is built around, and it is where the differences
  below concentrate.

## One vocabulary, three shapes

All three crates share the same core idea — the game reads `Jump`, not `KeyCode::Space` — and BEI
and this crate share Unreal's vocabulary on top of it. Where they differ is what kind of *thing*
each concept is:

| | LWIM | BEI | `bevy_action_map` |
| --- | --- | --- | --- |
| An action is | a variant of your `Actionlike` enum | an entity with an `Action<A>` component | a type implementing `InputAction` |
| A binding is | a boxed `dyn Buttonlike` in an `InputMap<A>` component | an entity with a `Binding` component, related to the action | an entry in a plan compiled once at app build |
| A context is | — (one `InputManagerPlugin<A>` per enum) | any component, registered with `add_input_context` | a type implementing `InputContext`, used as a component |
| State lives | in an `ActionState<A>` component | in components on each action entity | in dense per-instance tables on the context entity |
| Bindings are declared | by building an `InputMap` value | by spawning entities (`actions!` / `bindings!` macros) | in a closure passed to `add_context` |

The middle column is the one that most shapes BEI's feel: because actions and bindings are entities,
a scene file can carry a whole input map, a third-party crate can add an action to a context it does
not own, and change detection works per-action for free. The right column is the one that most
shapes this crate's: because bindings are compiled once into an immutable plan, evaluation is an
array walk with no allocation, the declared bindings survive a rebind as a separate baseline, and
the whole state of a context is a couple of `Copy` slices.

Neither of those is straightforwardly better. They buy different things, and sections 8, 9 and 10
below are where the difference stops being aesthetic.

One row is missing from that table on purpose, because it is the one nobody thinks about until a
refactor: **what identifies an action in a player's saved settings**. LWIM's answer is the enum
variant, BEI's is the Rust type, and this crate's is a declared string that is deliberately neither.
Section 9 has it.

---

## 1. What the crate reads: levels or edges

This is the difference the rest of the timing story follows from, so it goes first.

| | Reads |
| --- | --- |
| LWIM | `ButtonInput<KeyCode>`, `ButtonInput<MouseButton>`, `Gamepad` components, `AccumulatedMouseMotion` — sampled into a `CentralInputStore` resource in `PreUpdate` (`src/user_input/keyboard.rs`, `updating.rs`) |
| BEI | the same resources, sampled inline by an `InputReader` system param during evaluation (`src/context/input_reader.rs`) |
| this crate | the `KeyboardInput`, `MouseButtonInput`, `MouseMotion` and `RawGamepadEvent` message streams, into a timestamped queue drained by time window (`src/frame.rs`) |

Bevy's `ButtonInput` is a **level**: `keyboard_input_system` clears it each frame and replays that
frame's events into it, so a key pressed *and* released within one frame leaves `pressed()` false.
LWIM reads `get_pressed()` and `get_just_released()`; BEI reads `pressed()`. Neither reads
`just_pressed`. So a tap shorter than one render frame is invisible to both, and visible to this
crate as two logged transitions:

```rust
// bevy_action_map, src/context.rs — both edges written in one frame
world.write_message(press(KeyCode::Space, ButtonState::Pressed));
world.write_message(press(KeyCode::Space, ButtonState::Released));
app.update();
assert_eq!(heard, ["fired", "completed"]);   // two observer calls, in order
```

**How much this matters is a real question, not a rhetorical one.** At 60 Hz a lost tap is one under
16 ms, which most games never notice and a rhythm or fighting game notices immediately. It matters
more the further the read is from the render frame — see the next section. And it is only half
recovered by polling even here: `Actions::fired::<A>()` returns one `Phase` per read, so a sub-tick
tap polls as `Completed`. The two transitions are recoverable through the observer path
(`On<Fired<A>>` / `On<Completed<A>>`) or the transition log, not through `fired()`.

Reading edges is also what makes sections 6 (dead zones), 7 (device routing) and 10 (replay)
possible in the shape they take here; it is one decision paying for three features, which is why it
is worth its cost.

## 2. Fixed timestep

All three crates have a story here. An earlier draft of this comparison said otherwise about BEI and
LWIM, and was wrong.

**BEI** lets a context name the schedule it is evaluated in:

```rust
app.add_input_context_to::<FixedPreUpdate, Player>();
```

Its consumed-input set is keyed by schedule `TypeId` and cleared when that schedule runs, so a
`FixedPreUpdate` schedule running several times in one frame gets an independent consumption
decision each time, while still seeing what `PreUpdate` consumed. Events fire once per schedule run
(`src/context.rs`, `src/context/input_reader.rs`). This crate reached the same arrangement
independently and for the same reason; Design.md §5.2 credits BEI for it.

**LWIM** keeps *two* `ActionState`s per entity and swaps between them: `swap_to_fixed_update` runs
in `RunFixedMainLoop::BeforeFixedMainLoop`, the fixed state is updated once per frame there, and
`swap_to_update` restores the render one afterwards (`src/plugin.rs`, `src/action_state/mod.rs`). A
`just_pressed` read from `FixedUpdate` therefore stays true for every fixed tick of that frame
rather than only the first — which is usually what a fixed-tick reader wants.

**This crate** makes the tick domain a property of the context type (`#[context(tick = Fixed)]`),
evaluates each context exactly once in its domain, and drains the timestamped event queue by time
window, so each fixed tick sees the events belonging to its own window.

The remaining difference is not "has a story" but **what happens to the input in the gaps**:

| | A tap shorter than one render frame | Zero fixed ticks in a frame | Several fixed ticks in a frame |
| --- | --- | --- | --- |
| LWIM | lost (level sampling) | held state carries to the next tick that runs | every tick reads the same frame's state |
| BEI | lost (level sampling) | held state carries to the next tick that runs | every tick reads the same frame's state; consumption is per-run |
| this crate | preserved, as two transitions | its events wait in the queue for the next tick's window | each tick drains its own window; no event seen twice, none skipped |

The cost of the last row is stated in Design.md §7 and is real: an action needed at both rates must
be declared in two contexts, because a context is evaluated in exactly one domain.

## 3. Arbitration: chords, priority, and consumption

Three mechanisms, and the crates draw the line between "automatic" and "declared" in different
places.

**Longer chord beats shorter.** `Ctrl+S` should save without also moving the character down.

- **LWIM** does this automatically and by default: `ClashStrategy::PrioritizeLongest` compares
  `BasicInputs` decompositions and suppresses any action whose inputs are a strict subset of
  another's (`src/clashing_inputs.rs`). It applies within one `InputMap`; two different `Actionlike`
  enums do not clash-resolve against each other.
- **BEI** orders actions within a context by the maximum modifier-key count of their bindings, so
  `Ctrl+S` is evaluated before `S` (`src/context.rs:485`). But ordering only decides *who goes
  first*; suppression requires `ActionSettings { consume_input: true }`, which is **off by
  default**. With defaults, pressing Ctrl+S fires both. It also only understands `ModKeys` (Ctrl,
  Shift, Alt, Super), not a general chord — a general chord is the separate `Chord` condition, which
  references another action.
- **This crate** runs a pre-pass over the plan: the longest satisfied chord on each control is found
  before anything is read, and a shorter binding on that control reads as rest. Automatic, no
  consumption involved, and general over any chord (`src/eval.rs:549`).

**One context taking a control from another.** A pause menu should stop the ship hearing Escape.

- **LWIM** has no context concept. You disable actions (`ActionState::disable`) or add run
  conditions.
- **BEI** has `ContextPriority<C>` (a `usize`, default 0; ties broken by reverse spawn order), and
  `consume_input` per action. Consumption is global across contexts within a schedule.
- **This crate** has `PRIORITY` as a const on the context type, resolved into priority-keyed system
  sets once at app build rather than sorted per frame; `CONSUMES` defaults per action and can be
  overridden per binding. It also has an `EXCLUSIVE` context, which treats every lower-priority
  context as inactive while it is up — so a modal screen does not have to enumerate the actions it
  is taking.

BEI's arrangement is more flexible at runtime (priority is a component you can change); this crate's
is fixed at build and cannot be changed per entity, which is a real limitation if you wanted two
players' contexts at different priorities.

**A caveat both BEI and this crate share:** consumption and exclusivity are recorded in global
tables, so in local multiplayer one player's claim is visible to another's contexts. BEI scopes its
gamepad reads per context but not its consumption; this crate scopes device *events* per context via
`Paired` but leaves the consumed set global (Roadmap.md's deferred table has the row).

## 4. Folding several bindings into one action

`Jump` on both Space and gamepad South; `Move` on both WASD and the left stick. What is the value
when two contribute at once?

- **LWIM** resolves per input kind; buttonlike actions are pressed if any input is pressed.
- **BEI** takes the contributions with the most significant `TriggerState` and combines them by
  `ActionSettings::accumulation`: `Cumulative` (sum, the default) or `MaxAbs`.
- **This crate** keys the rule off the action's declared **intent** — a property BEI and LWIM do not
  have. `Button`, `Analog1` and `Directional2` take the strongest contribution; `Delta2` sums.

The reason for the third of those is that shape does not distinguish a stick from a mouse — both are
`Vec2` — but summing is right for one and wrong for the other. A mouse delta is a displacement that
already happened, so two devices moving at once should both move you; two half-deflected sticks are
not a full deflection. Intent also lets the crate *refuse* a binding whose source channel cannot
serve the action (a stick bound to a `Delta2` look action), which is caught when the context is
declared rather than felt as camera drift later.

The honest cost: intent is a fourth thing to declare, and it makes one case harder rather than
easier — a single action driven by *both* a mouse and a stick needs an explicit rate-to-delta
conversion (`.per_second()`) rather than just working.

## 5. Conditions and modifiers

Broadly comparable between BEI and this crate, and much richer in both than in LWIM.

| | LWIM | BEI | this crate |
| --- | --- | --- | --- |
| Press / release / down | `just_pressed` / `just_released` / `pressed` | `Press`, `Release`, `Down` | press, release, down |
| Hold for a duration | via `timing` feature's `current_duration`, by hand | `Hold`, `HoldAndRelease` | `.hold(t)`, with progress reported |
| Tap, multi-tap | — | `Tap`, `Combo` | `.tap()`, multi-tap |
| Repeat / pulse | — | `Pulse` | `.pulse(t)` |
| Toggle, cooldown, block-by | — | `Toggle`, `Cooldown`, `BlockBy` | latch (hold-vs-toggle), no cooldown |
| Flick | — | `Flick` | — |
| Combining conditions | — | `ConditionKind` (implicit / explicit / blocker) | any-of / all-of / none-of |
| Modifiers | `AxisProcessor` chain: dead zone, bounds, sensitivity, inversion | negate, scale, clamp, swizzle, dead zone, exponential curve, linear step, smooth nudge, delta scale, accumulate-by | negate, scale, clamp, swizzle, dead zone, response curve, rate conversion |
| Third-party extension | `dyn` trait objects, registered | `add_input_condition` / `add_input_modifier`, as components | enum with a `Custom(Box<dyn …>)` arm, reflected |
| Attached at | the input | the binding **or** the action | the binding |

BEI's ability to attach a modifier at the *action* level, applying after the bindings are folded, is
a genuine convenience this crate does not have — its `Cardinal::wasd_keys()` + action-level
`DeadZone` idiom is neat, and here the equivalent has to go on each binding.

BEI also has more conditions than this crate, `Flick` and `Cooldown` in particular.

## 6. Dead zones

This is where reading raw events pays off, and it is also the place the difference is easiest to
overstate, so precisely:

Bevy applies a per-axis `GamepadSettings` filter to gamepad values before they reach the `Gamepad`
component. BEI and LWIM both read the `Gamepad` component, so they consume whatever that filter
produced and apply their own `DeadZone` / `AxisDeadZone` on top of it. This crate reads
`RawGamepadEvent`, which is emitted before that filter, and owns the whole chain.

*(The exact behaviour of Bevy's filter has changed between versions — on `main` the `Gamepad`
component stores the unscaled raw value and the deadzone is applied to the change-detection
threshold and the emitted event's scaled value. So "BEI consumes an already-deadzoned value" is
version-dependent and was more true in older Bevy. What is not version-dependent is that BEI and
LWIM read a value someone else has already decided the filtering policy for, and this crate reads
one nobody has.)*

What the crate does with that is three stages, because three parties have a claim on the number and
they are answering different questions (Design.md §8.1):

1. **Calibration** — this physical unit's true centre and rest envelope, measured by an explicit
   "move the sticks and let go" step the game drives, applied as the event is recorded. Per device
   unit, because drift is a wear characteristic of one pad.
2. **Design** — the shape and curve the mechanic wants. This is the stage that rescales, so full
   deflection still reads 1.0.
3. **Preference** — the player's own adjustment, modulating stage 2.

Only one stage may rescale, and that is enforced when the plan is compiled. Neither BEI nor LWIM
distinguishes these; both have "a dead zone", which is stage 2.

Whether you need stages 1 and 3 depends entirely on whether you ship a settings screen with a dead
zone slider, and whether your players have worn sticks. Many games do not, and for those this is
machinery for nothing.

## 7. Local multiplayer and device routing

- **LWIM**: `InputMap::with_gamepad(entity)` associates one map with one gamepad. Keyboard and mouse
  are global.
- **BEI**: a `GamepadDevice` component on the context entity — `Any`, `Single(entity)`, or `None`.
  Keyboard and mouse are global.
- **This crate**: a `Paired(DeviceHandleSet)` component naming the devices one occupant owns.
  `DeviceHandle` is `KeyboardMouse` or `Gamepad(Entity)`, so keyboard-and-mouse can be routed to one
  player and a pad to another; filtering happens when the frame is applied, before anything else
  reads it. It also ships a **join gesture** — a context bound to a control *class* rather than a
  specific control, so "press anything to join" claims the device that pressed, with a check that
  two waiting slots cannot race for it (`examples/split_friction/`).

The keyboard-and-mouse routing is the substantive difference; the join gesture is the thing that is
tedious to write yourself. Note that this crate treats keyboard and mouse as one indivisible device,
so it cannot split two keyboards — but neither can Bevy, which does not distinguish them.

## 8. Rebinding, and what a settings screen needs

This is the axis the crate was actually built for, so it is where the gap is widest — and therefore
where it is easiest to overstate what the other two lack. What they have goes first.

**What BEI has today.** Bindings are entities with a `Binding` component, so a settings screen can
query them, and rebinding is despawning one and spawning another. `Binding` implements `Display`
("Control + KeyD", "Mouse Left", "Scroll Wheel"), so a screen can render a binding as a string
without writing a match. `Binding` is `Serialize`/`Deserialize` under the `serialize` feature. That
is a real, workable basis for a rebinding UI, and a game can build one on it.

**What LWIM has today.** `InputMap` is a mutable component with `get_buttonlike`, `insert`,
`remove_at`, `clear_action` and iteration over every binding, serializable via `serde` and typetag,
and loadable as an asset. Same story: a workable basis, rendering left to you.

**What neither has**, and what this crate treats as first-class:

- A **presentation model distinct from the binding model**. Dead zones and response curves are
  developer concerns; a player rebinding "Thrust" should not see them. Marking a binding
  `.mappable()` puts it in a separate, smaller model — a named *mapping* with an ordered list of
  slots ("Primary", "Secondary") and a declared capacity, which is what a primary/secondary table
  is. Everything is *listed* for the player to read; only what was declared is rebindable.
- **Interactive capture** with reserved and excluded controls, and **conflict detection** that can
  be run against an uncommitted working copy — so a screen with unconfirmed choices can tell whether
  two of them clash before either is applied, and a clash can steal the control from whatever held
  it.
- **Overrides as a diff, not a replacement.** The declared bindings stay intact; a rebind is a patch
  applied over them by recompiling a variant plan. That is what lets a patch ship revised defaults
  that still reach a player who never touched that row. In BEI and LWIM the live bindings *are* the
  source of truth, so a saved input map is a full replacement and revised defaults reach nobody who
  has ever saved.
- **Prompts that stay true.** A reverse lookup from an action to the controls that would fire it
  right now, exposed as a text span a template can write, which is told when the answer moves.
- **Shared controls declared as shared** — tap to dodge, hold to sprint, on one control: rebinding
  moves both, and the second is drawn as a subordinate line rather than a row of its own.
- **Tunables** — a named, typed, player-adjustable value that overwrites one field of one modifier,
  enumerated and persisted the same way a mapping is.
- **Presets** — a named override set the game ships, applied through the same path a rebind uses,
  and allowed to move rows a capture screen would refuse.

None of this is exotic; it is what a shipped game's controls screen needs, and it is normally
written by hand per game. Whether it is worth a different crate depends entirely on whether you were
going to write it.

**Not built here either:** glyphs (button images). All three crates render text.

## 9. Persistence

- **LWIM**: `InputMap` derives `Serialize`/`Deserialize`; typetag handles the boxed trait objects.
  Also loadable as an asset.
- **BEI**: `Binding` and `ActionSettings` are `Serialize`/`Deserialize` under the `serialize`
  feature; conditions and modifiers are components, so reflection-based scene serialization is the
  route.
- **This crate**: `Overrides` — the diff, not the map — serializes through `serde`, hand-written so
  a single-control row writes as a bare scalar, pinned by a golden TOML document. Loading resolves a
  saved mapping name against what the game currently declares and *reports* an unresolved name or an
  unrecognized control rather than dropping it silently.

Two structural differences sit underneath those, and both are invisible until the game changes.

**The first is the one from section 8**: serializing a diff against a declared baseline behaves
differently across a game update than serializing the map.

**The second is what a saved row is keyed on.** A settings file has to name the action a binding
belongs to, and the three crates name it differently:

| | The name in saved data | A Rust rename | A module move |
| --- | --- | --- | --- |
| LWIM | the enum variant, via serde on `HashMap<A, _>` | orphans that action's bindings | harmless to serde; breaks a reflect-based save |
| BEI | the Rust type — actions carry `Name::new(any::type_name::<A>())`, and reflect/scene serialization keys on `TypePath` | orphans them | orphans them |
| this crate | the action's declared `PATH`, a string separate from the type | harmless | harmless |

`#[action(path = "gameplay.jump")]` exists for exactly this. The path is a name that lives outside
your code, so `Move` can become `MoveOnFoot` and relocate to another module without a player losing
what they bound to it. The convention is `<namespace>.<name>`, and the discipline is that the path
does **not** follow the type — changing a path is a save-data migration, not a refactor. It does a
second job as the localization key a controls screen renders the row's label from, which is why it
is a required declaration rather than an optional one.

Loading also *reports* a path it cannot resolve rather than dropping it, so a rename that did happen
is visible instead of silent.

The cost is a third thing to declare per action, and a convention to hold to — nothing stops you
renaming the path alongside the type and getting exactly LWIM's behaviour. What it buys is that the
default is right: a refactor is free, and breaking a player's settings takes a deliberate act.

**All three leave writing the bytes to a file to the app.** None of them is a settings-file crate.

## 10. Determinism, replay, and rollback

- **LWIM** has the most mature answer of the three today: `ActionDiff` streams and
  `generate_action_diffs`, plus `InputManagerPlugin::server()` which processes no input at all and
  expects `ActionState` to be supplied. This is designed for netcode and is used for it.
- **BEI** has `ActionMock` (drive an action's value and state for a span, skipping bindings),
  `ExternallyMocked` (a marker that excludes an action from evaluation entirely so you can write its
  data yourself), and `CustomInput`/`CustomInputs` (a resource of `ActionValue`s that bindings can
  read, for inputs Bevy does not model). Between them these cover testing, cutscenes, AI, and
  network-replicated input.
- **This crate** puts the seam one layer lower: the input frame (L1) is a distinct, constructible,
  serializable object, and the whole mapping layer is a pure function of it. So a replay or a
  network peer writes *events*, not action states, and everything downstream — conditions, chords,
  consumption, contexts — is re-derived rather than bypassed. Action state is two `Copy` slices plus
  a dirty bitset, so snapshot/restore is two slice copies.

The rollback half of this is **designed and not proven** — there is no testbed in tree that actually
rolls back, and Roadmap.md's deferred table says so. Treat LWIM's `ActionDiff` as the shipping
answer and this as an argument about where the seam belongs.

Mocking at the action level (BEI, LWIM) and injecting at the event level (this crate) are not the
same test. The first tests your game logic; the second also tests your bindings.

## 11. Backends that own the bindings

Steam Input is the motivating case: the binding UI, the conflict rules and the glyphs all live
outside the game, and the platform answers "is Jump pressed" for you.

BEI's `ExternallyMocked` is per-action and does most of what is needed — evaluation skips the
action, you write its state. This crate designs the same seam more explicitly (an *authority*
backend writing action state directly, plus a *source* backend supplying the input frame, plus a
reverse lookup behind a trait so prompts do not assume our binding tables exist, plus device
suppression at L0 so Steam's emulated pad is not also sampled directly). Only the reverse-lookup
trait is built; the rest is Design.md §10.5 and an unbuilt chunk.

So: **BEI has the more useful thing today**, and this crate has the more complete design. Do not
choose on this axis unless you are actually shipping on Steam Input, in which case check the current
state of both.

## 12. The rest

**UI focus.** BEI does not integrate `bevy_input_focus`; it offers an `ActionSources` resource so
you can switch whole input sources off while the UI is being used, with a worked example for
`Interaction`. LWIM reserves an `InputManagerSystem::Filter` system-set slot for a filter you write.
This crate has an optional `focus` feature and focus-activated contexts — and a known gap:
`bevy_ui_widgets` handles its own keyboard through `InputDispatchPlugin`, which asks the mapper
nothing, so a widget activating on Space does so whether or not a context claimed Space. Roadmap.md
lists it under "Known wrong today". None of the three has this fully solved.

**Diagnostics.** This crate has `why_not::<A>()`, which answers "why didn't this fire?" with a named
obstacle — inactive context, a higher-priority consumer, a longer chord winning, an unmet condition,
a device that is not this player's. BEI's answer is `RUST_LOG=bevy_enhanced_input=debug`, which is
less structured but costs nothing to add and covers a lot. LWIM has no equivalent of either.

**Build surface.**

| | `no_std` | Depends on | Feature gates |
| --- | --- | --- | --- |
| LWIM | no | `bevy` umbrella, subset of features | mouse, keyboard, gamepad, picking, asset, timing |
| BEI | **yes** | `bevy` umbrella, `default-features = false` | reflect, state, serialize |
| this crate | **yes** (`alloc` only) | Bevy *subcrates* individually | std, libm, keyboard, mouse, gamepad, touch, bevy_reflect, serialize, focus, state |

`no_std` is not a differentiator between BEI and this crate — both are. The dependency shape is:
this crate depends on `bevy_ecs`, `bevy_input`, `bevy_math` and so on individually rather than on
the `bevy` umbrella, which matters mainly if you care about the minimal graph or about eventual
upstream inclusion.

**Touch.** None of the three has touch bindings. This crate has a `touch` feature flag that is
currently a stub.

**Mouse wheel.** BEI and LWIM have it. This crate does not (Roadmap.md's deferred table).

---

## What each crate is genuinely best at

**BEI** — completeness and maintenance. The largest condition and modifier set, action-level as well
as binding-level modifiers, presets that make common bindings one line, an ECS model that makes
input authorable from a scene and extensible by a third-party crate, and an active maintainer. It is
the default answer.

**LWIM** — smallness and netcode. One enum, two components, `just_pressed`, and the most mature
action-diff/rollback story of the three. If you do not need contexts, conditions or a rebinding
screen, the other two are machinery you are paying for and not using.

**This crate** — the player-facing half. The presentation and rebinding model, overrides as a diff
against declared defaults, per-player device routing including keyboard-and-mouse, three-stage dead
zones, sub-frame input edges, and a pure-function mapping layer with an injectable input frame. All
of which is worth exactly nothing if you never build a controls screen — and none of which is
shipped or published yet.

## If you are migrating

Concept-for-concept, BEI → this crate is close to mechanical:

| BEI | here |
| --- | --- |
| `#[derive(InputAction)] #[action_output(Vec2)]` | `#[derive(InputAction)] #[action(path = …, output = Vec2, intent = Directional2)]` |
| a context component + `add_input_context::<C>()` | `#[derive(InputContext)]` + `add_context::<C>(\|c\| …)` |
| `add_input_context_to::<FixedPreUpdate, C>()` | `#[context(tick = Fixed)]` |
| `actions!` / `bindings!` at spawn time | `c.bind::<A>(source)` at app build |
| `Fire<A>` / `Start<A>` / `Complete<A>` / `Cancel<A>` | `Fired<A>` / `Started<A>` / `Completed<A>` / `Canceled<A>` |
| `Query<&Action<A>>`, `ActionEvents` | `Actions<C>` with `value::<A>()` / `fired::<A>()` |
| `ContextPriority<C>` component | `PRIORITY` const on the context type |
| `ActionSettings { consume_input: true }` | `CONSUMES` on the action, or per binding |
| `ContextActivity<C>`, `ActiveInStates` | `active_if` / `active_in_state`, or `activate()`/`deactivate()` |

The three places it is not mechanical: an action must declare an **intent**, a context is evaluated
in **one** tick domain so an action needed at both rates is declared twice, and bindings are
declared at app build rather than spawned — so anything that manipulated binding entities at runtime
becomes an override instead.

One thing worth doing deliberately rather than mechanically: **choose the paths, do not derive
them.** The obvious move when porting is `path = "jump"` for `struct Jump`, which reproduces BEI's
identity exactly and throws away the reason the field exists (section 9). Pick the namespace and the
name you would want in a settings file five years from now, because that is what they are, and they
are cheapest to get right before anyone has saved one.

LWIM → either is a rewrite, because an enum of actions becomes one type per action.

## Corrections

If you maintain BEI or LWIM and something here is wrong, it is a bug in this document and I would
rather fix it than defend it. An earlier version of this comparison claimed neither crate addressed
fixed-timestep timing, which was false of both — section 2 is the correction. Open an issue or mail
the author.

Deeper reasoning for this crate's side of each difference is in
[Requirements.md](../Requirements.md) (what must be true) and [Design.md](../Design.md) (how), with
the current built/unbuilt line in [Roadmap.md](../Roadmap.md).

[bei]: https://github.com/simgine/bevy_enhanced_input
[lwim]: https://github.com/Leafwing-Studios/leafwing-input-manager
