# One-way doors in upstreaming an input mapper

`bevy_enhanced_input` (BEI) is [intended for eventual upstream inclusion][bei] as Bevy's input
abstraction. Most of it will keep evolving after that: new conditions, better ergonomics, renamed
types. This document is about the smaller set of decisions that **stop being revisable** once it is
the engine's answer, because changing them would change what input *is* for every downstream crate
at once.

It is written for Bevy's reviewers, and asks one question: what is cheap to do now that would be
expensive later? Each door says what BEI commits to, what that buys, what it forecloses, and the
hedge that keeps the option open. The doors are ordered by how much acting now saves, roughly the
cost of reversing later over the cost of hedging now.

Written against BEI 0.26.0 (Bevy 0.19), read from source on 2026-08-31; one later change on its
`main`, to how a sub-frame tap is read, is noted in door 6. Written by the author of a different
input crate, `bevy_action_map`, so read it as an interested party's list. Where that crate stands on
a door is stated with it, and "this crate" means `bevy_action_map` throughout.

## What to do now

| Door | Do now | Cost now | Cost after upstreaming |
| --- | --- | --- | --- |
| 1. Action identity | An optional declared name per action, defaulting to the type name | One attribute | Every saved file keyed on type paths migrated, or two identities kept forever |
| 2. Consumption scope | Record which player's context made a consumption claim | One field | Every existing claim changes meaning; third-party contexts that read the set break |
| 3. The player-facing half | A reverse lookup behind a trait, and a structured name for a control | A trait and a type | Every controls screen and prompt crate builds its own, keyed differently |
| 4. A backend that owns the bindings | Nothing of its own: check doors 1 and 3 against a Steam integration | None | A Steam crate carrying its own names and prompt path, inherited by every game using it |
| 5. Actions as entities | Keep the declared bindings after spawning them, to save overrides as a diff and check a context before it runs | A retained template | Revised default bindings never reach a player who has rebound anything; a validation pass added later breaks contexts that used to load |
| 6. Level sampling | Make the input reader a public, substitutable layer | Widening `CustomInputs` | Every condition rewritten for edges, or two input paths |
| 7. Specificity | Document consumption and specificity as separate concerns | Documentation | Ecosystem code that relies on consumption to mean specificity |
| 8. Dead zone ownership | Bevy decides whether the mapper inherits or applies `GamepadSettings` | A decision | Changing what `Gamepad` reports |

## The doors

---

### 1. An action's identity is its Rust type

**The commitment.** There is no name for an action other than the type: `Action<A>` requires
`Name::new(any::type_name::<A>())`, and nothing else identifies it. BEI treats saving bindings as
out of scope. A game that wants to persist them declares its own settings struct, a field per
action, and copies bindings in and out. On that view a stable id is unnecessary, because the field
names are the ids and the game chose them.

**What it buys.** Nothing to declare, nothing to keep in sync, and no second name that can disagree
with the first. `#[derive(InputAction)] #[action_output(bool)] struct Jump;` is the whole
declaration, and the input crate owns no file format.

**What it forecloses.** A name that anything besides the game can use. The settings struct solves
persistence for one game, but its names live in that game, out of sight of everything else:

- A rebinding layer above the input crate (door 3) can list actions, but the only key it can save
  them under is the type path. Each such crate either invents its own naming or uses
  `my_game::actions::Jump`, and renaming `Jump` to `Leap`, or moving it into `actions/movement.rs`,
  then orphans every binding a player saved. The second of those is not even a rename, it is
  tidying. (LWIM has a milder version: serde keys on the enum variant, so a module move is harmless
  but a variant rename is not.)
- An external backend's manifest names actions by string, and is a file checked in beside the game
  (door 4).
- An action a third-party crate adds to a context it does not own, the strongest argument for door
  5's shape, is not in the game's struct. It goes unsaved unless the game learns about it by hand.
- Every game writes the field-per-action struct and the copying, and an action added without a field
  is silently not saved.

**Why it is one-way.** "Out of scope" holds only while there is no ecosystem. Once the crate is
upstream, the first crates that need a name will use the only one there is, and settings files keyed
on type paths will exist whether or not the input crate owns persistence. Adding a declared name
then means migrating those files or carrying two identities forever. It also compounds with door 5:
with no retained declaration *and* no stable name, a generic saved configuration is a full
replacement keyed on something a refactor can change.

**The cheap hedge.** An optional attribute defaulting to today's behaviour:

```rust
#[derive(InputAction)]
#[action_output(bool)]
#[action_path = "gameplay.jump"]   // optional; falls back to type_name::<A>()
struct Jump;
```

Nothing changes for anyone who does not use it, a game that saves through its own struct included,
and one that does use it gets a name a refactor cannot touch. It should be a real declaration rather
than a serialization-only annotation, since the presentation layer of door 3 and the manifest of
door 4 want the same string.

*(This crate requires it rather than offering it: `#[action(path = "gameplay.jump")]`, with a
`<namespace>.<name>` convention and the rule that a path does not follow the type. Requiring it is a
defensible cost, one more line per action, but defaulting to the type name is the right migration
path for a crate that already has users.)*

---

### 2. Consumption claims are global

**The commitment.** `ContextPriority<C>` sorts a single world-wide `Vec<ContextInstance>` per
schedule; `ConsumedInputs` is a world-wide set keyed by schedule. Neither records *whose* context
made the claim.

**What it buys.** Priority is a component, so it is dynamic and inspectable; ordering is a total
order with an obvious tie-break (reverse spawn order); consumption needs no ownership concept.

**What it forecloses.** Local multiplayer, in the case where two players each have their own
contexts. Player 1's modal menu consuming Escape hides it from player 2's gameplay context, because
nothing in the consumed set says the claim belonged to player 1. The same applies to any
"exclusive"-style shadowing built on priority.

**Why it is one-way.** Adding an owner to a claim after the fact changes the meaning of every
existing consumption, and any third-party context that reasoned about the global set breaks. Adding
it now is a field.

**This crate scopes claims by device.** A claim in `ConsumedControls` and an entry in the exclusion
ceiling each carry the devices of the instance that made them, and reach only a reader sharing one;
`contains` and `claimant` take the reader's devices as a parameter. That is the field this door asks
for.

---

### 3. The player-facing half belongs to the app

**The commitment.** BEI leaves presentation out of scope. Which actions a player may rebind, what a
controls-screen row is called, how a prompt finds the control bound to an action, and how a rebind
is saved are each the game's own business. What the crate offers is binding entities to query and
mutate, and `impl Display for Binding`, which formats `{key:?}`: "KeyD", "Space", "Control + KeyD".
Prompts are not addressed.

**What it buys.** A smaller crate with a clear edge, and nothing to design. A game can build a
controls screen on it today, which is more than LWIM offers.

**What it forecloses, once this is the engine's answer.** Every game with a controls screen builds
the same database for itself: its bindable actions, a localization key for each, a slot structure, a
saved form, and a way from an action to its current controls for prompts. That work varies little
between games, and none of it is shared. Upstream, the layering makes it worse: an input crate
cannot depend on `bevy_ui`, so a shared rebinding or prompt crate has to live above it, with only
binding entities to build on. Then:

- Every controls screen reimplements which bindings are *shown* to a player, which are *rebindable*,
  and what a primary/secondary slot is, and they will not agree.
- Each one keys its database on the only name an action has, the type path (door 1).
- `Display`'s output is a debug string in a user-facing position. `{key:?}` is not localizable and
  not stable across `KeyCode` renames. Committing to it as *the* answer is the door; keeping it as a
  fallback beneath a structured form is not.
- A prompt ("Press W") has to stay correct after a rebind, so it needs the same records the screen
  edits. Kept in two places, they drift.
- A backend that owns the bindings has no way to answer "what is bound to Jump" in the shape the
  built-in path does, so prompts cannot be written once (door 4).

**Why it is one-way.** The layering is. Once games and crates have built their own presentation
databases against binding entities, the input crate cannot take that job on without displacing all
of them, and the surface they were written against cannot change without breaking them.

**The cheap hedge.** Leaving presentation to apps is a defensible scope for an engine's input crate.
What it must still provide, for a layer above it to be built well, is small, and neither part
requires building a UI or owning persistence:

1. A **reverse lookup behind a trait**, from an action to the controls it is bound to, so an
   external authority can answer it instead. Its answer type must not be limited to Bevy's own
   control types.
2. A **structured name for a control**, separable into a localization key plus a fallback string,
   rather than only a `Display` impl.

With door 1's declared name, those are the whole of what a controls-screen or prompt crate needs
from below.

**This crate takes the other side.** Bindings and presentation are one database, keyed by one
declared id. A mapping model says which bindings a player sees, which they may rebind, and what the
slots are; capture, conflict detection, overrides and presets act on that model; a prompt reads the
same records, so a rebind cannot leave it stale; and a control has a stable string form to key a
localization catalogue, alongside the name a connected pad uses for its own buttons. The cost is a
declared path per action and a presentation declaration per rebindable binding. What it removes is
work every game with a controls screen would otherwise do itself, each in its own way. What sits
above, in `examples/common/`, is the part an input crate cannot own: drawing prompts, and a
`bevy_ui_widgets` bridge for a controls screen. Both are written against the public API with nothing
added to the crate for them.

---

### 4. A backend that owns the bindings

This door is a test of doors 1 and 3 rather than a commitment of its own. Steam Input is the
concrete case: Steam owns the bindings, the rebinding UI and the glyphs, and hands the game a value
per action.

**What BEI already has.** The value path. `ActionMock` skips input reading, conditions and modifiers
and reports a supplied state and value, and the transition events still fire, so a backend can drive
an action each frame and its observers see no difference. A context fed by Steam can set
`GamepadDevice::None`, which keeps it from also reading the pad Steam emulates. Neither needs
anything added.

**What it still needs, and which door provides it.**

- A name for each action that Steam's manifest can hold. The manifest is checked in beside the game,
  so the name has to survive a refactor: door 1. (Measured against a running client: a dotted name
  such as `pong.move` is a valid Steam action name.)
- An answer to "what is Jump bound to" in Steam's terms, its origins and glyphs, which are not Bevy
  control types: door 3's reverse lookup, with an answer type that is not limited to Bevy's own
  controls.
- A controls screen that shows Steam-owned rows as configured in Steam, and opens Steam's binding
  panel rather than starting a capture. With the player-facing half left to the app, every game
  integrating Steam solves this separately: door 3.

**Why it is one-way.** Only through doors 1 and 3. If the name and the reverse lookup exist before a
Steam integration crate does, that crate is a backend and nothing else. If they do not, it has to
carry its own action names and its own prompt path, and every game using it inherits both.

**This crate** was designed around this case. The declared path is the manifest name; a prompt
returns a `ControlOrigin`, which can be Steam's; and an action can be bound to a backend in place of
one device family, which writes a value that the crate's own lifecycle turns into events. Each part
has been measured against a running Steam client. A Steam backend has not yet been run end to end.

---

### 5. Actions and bindings are entities

**The commitment.** `Action<A>` is a component on its own entity, related to the context entity by
`ActionOf<C>`; `Binding` is a component on a further entity related by `BindingOf`. This is the API,
not an implementation detail: `actions!` and `bindings!` are how you declare input, and
`Query<&Action<Jump>>` is how you read it.

**What it buys.** A great deal, and it should be said first:

- Input maps are **authorable from a scene**, which for an engine is close to decisive.
- A third-party crate can add an action to a context **it does not own**, with no cooperation from
  the owner and no registration API. This is the thing a closed, compile-time model cannot do, and
  it is the strongest single argument for the shape.
- Change detection, inspectors, and editor tooling all work with no extra machinery.
- No parallel registry to keep in sync with the world.

**What it forecloses.** A declaration distinct from the live bindings, and with it two things:

- **A baseline to diff against.** Because the binding entities *are* the source of truth, a rebind
  mutates the only copy. A game that keeps its own record of the defaults can save a diff against
  them, but anything more general can only save a full replacement, and a patch that ships revised
  default bindings then reaches nobody who has ever opened the controls screen.
- **Checking a context before it runs.** A set of binding entities is never finished, since any of
  them can be spawned or despawned at any time, so there is no point at which the crate can look at
  a context whole and refuse one that cannot work, or warn about one that looks wrong: an action
  bound to a control that cannot drive it, an action both bound and fed by a backend, the same
  control bound twice. Those surface as behaviour, in play. The same gap stops a controls screen
  from asking whether a set of unconfirmed choices would conflict without applying them first.

**Why it is one-way.** The entity model itself is total: every downstream scene file, every
third-party crate that spawns an action, every tutorial. Nobody is proposing to change it, and this
document does not. The declaration is separable from it.

**The cheap hedge.** Keep a *retained declaration*: bindings spawned from a registered template that
outlives them. That preserves overrides-as-a-diff, and gives a validation pass something whole to
check, without giving up entities for anything else. It is much cheaper to add before there is an
ecosystem of saved configurations than after, and a check added later is a new error for contexts
that used to load.

*(This crate compiles its declaration into an immutable plan, validated as a whole before the
context is installed. A diagnostics pass runs on declared data alone, so a controls screen can check
a working copy it has no intention of installing, and an override compiles a variant plan while the
declared one is kept. Compiling is more than the hedge needs: a retained template gets the diff and
the validation.)*

---

### 6. Input is sampled as a level, not consumed as an event

**The commitment.** `InputReader` reads `Res<ButtonInput<KeyCode>>`,
`Res<ButtonInput<MouseButton>>`, `Res<AccumulatedMouseMotion>`, `Res<AccumulatedMouseScroll>` and
`Query<&Gamepad>` directly, inside the evaluation system (BEI's `src/context/input_reader.rs`).
Every condition and modifier is written against "the value of this binding, now".

**What it buys.** Simplicity, and universality. Anything that can write `ButtonInput` is an input
source, with no adapter, which for an engine-level crate is a serious virtue. No queue, no
windowing, no timestamps to get wrong, no decision about how long an event lives.

**What it forecloses.**

- *Sub-frame edges.* Bevy's `ButtonInput` is cleared and refilled each frame, so a press and release
  delivered in the same frame leave `pressed()` false. BEI's `main` now reads `just_pressed()` as
  well, so a single tap in the render schedule is seen, as a press lasting one evaluation. What a
  level table still cannot carry is more than one edge per frame: two taps count as one, and the
  order of presses within the frame is gone. And since `just_pressed()` lasts one frame, a fixed
  tick that runs slower than render still misses a tap made in a frame where it did not run. Games
  that care are fighting games, where order and count within a frame are the input, and anything
  simulated in a fixed tick slower than its frame rate. (LWIM reads `pressed()` too.)
- *Replaying input through the mapper.* A test or a deterministic replay that wants to exercise
  bindings, conditions, chords and consumption needs an input record to feed in and re-derive action
  state from. With level sampling the replayable artifact is the output: `ActionMock` replays action
  values and states, which is what netcode wants anyway, since each peer has its own bindings. What
  is foreclosed is the input-level record.
- *Anything that must happen below the mapper, per device unit.* Stick calibration is the concrete
  case: drift is a wear characteristic of one physical pad, so the correction has to be applied
  where the message still names its sender. Reading a merged level table means that information is
  already gone.
- *Per-window fixed-tick draining.* `add_input_context_to::<FixedPreUpdate, C>()` gives a fixed
  context its own consumption and its own event firing, which is most of what fixed timestep needs.
  What it cannot give is a *different value* per tick, because all ticks in a frame read the same
  level table.

**Why it is one-way.** Not because the sampling code is hard to change (it is one file) but because
the *contract* propagates. Every third-party condition in the ecosystem will be written against a
synchronous `value(binding)` call. Widening what that call reports is cheap, as the `just_pressed()`
change shows: a tap that used to read as nothing now reads as a brief press, and nothing that relied
on the old answer was correct to. Converting to edges is not that kind of change. It means either a
call that returns several values per frame, which every condition has to be rewritten to handle, or
running both paths, with two sources of truth.

**The cheap hedge.** Make the reader a public, substitutable layer rather than a private system
param. BEI already has most of this: `CustomInput` / `CustomInputs` is a public resource that
bindings read from, and `Binding::Custom` is a first-class variant. Widening that from "inputs Bevy
does not model" to "the layer all input arrives through" costs little now and is the whole
difference later. It does not commit to event-based input; it stops foreclosing it. Even short of
that, giving `InputReader` a named public type and documenting it as replaceable leaves the door
ajar.

---

### 7. Specificity is spelled through consumption, and only over modifier keys

**The commitment.** Actions within a context are sorted by the maximum `ModKeys` count across their
bindings, so `Ctrl+S` is evaluated before `S`. But order alone decides only who goes *first*;
suppressing the shorter binding requires `ActionSettings { consume_input: true }`, which is off by
default. With defaults, pressing Ctrl+S fires both actions. General chords are a separate mechanism
(the `Chord` condition, referencing another action) and do not participate in the ordering at all.

**What it buys.** One mechanism instead of two. Consumption already exists for the
menu-over-gameplay case; reusing it for chord specificity means no separate arbitration pass.

**What it forecloses.** Specificity as a property of the *bindings* rather than of a runtime flag. A
plain `S` binding fires whether or not Ctrl is held, so correctness depends on remembering to opt in
per action, and only for modifier keys. LWIM's `ClashStrategy::PrioritizeLongest` is automatic,
default, and general over its `BasicInputs` decomposition, so the alternative is not hypothetical.

**Why it is partly one-way.** The default is flippable in a major version; that part is ordinary
evolution. What is harder is that deciding "the longest satisfied chord on this control wins" before
anything is read requires a pre-pass over the bindings, which in an entity model means walking
binding entities twice per context per tick. Doable, but a different evaluation shape than the
current single ordered pass, so the further the ecosystem gets built on consumption-as-specificity,
the more expensive.

**The cheap hedge.** Document consumption and specificity as distinct concerns even while one
implements the other. That alone keeps a later pre-pass from being a behavioural surprise.

---

### 8. Who owns the gamepad dead zone

**The commitment.** `InputReader` reads the `Gamepad` component, which is downstream of Bevy's own
`GamepadSettings` filter. The mapper therefore inherits a filtering policy rather than setting one,
and applies its `DeadZone` modifier on top.

**What it buys.** Consistency with the rest of Bevy: `Gamepad` is *the* gamepad API, and an input
mapper that read something else would be the odd one out.

**What it forecloses.** Anything that must happen below the game's own dead zone: per-unit
calibration for a worn stick, and a player-facing dead-zone control that can honestly go to zero.
Once the mapper reads filtered values, "who owns the dead zone" is settled in favour of
`GamepadSettings`, and inserting a stage underneath means changing what `Gamepad` means.

**Why this one is different.** It stops being BEI's decision at the moment of upstreaming and
becomes Bevy's, and Bevy could resolve it either way, including by making `GamepadSettings` a policy
the mapper *applies* rather than inherits. It is on the list because it is a door that gets walked
through as a side effect of the move rather than by anyone choosing.

*(The precise current behaviour is version-dependent: on Bevy `main` the `Gamepad` component stores
the unscaled raw value and the dead zone affects the change-detection threshold and the emitted
event's scaled value. The ownership question is the same either way.)*

---

## What is not a one-way door

Worth stating, because a list of concerns is only useful if it excludes the ordinary:

- **What an action's value means.** A stick deflection and a mouse delta are both `Vec2`, but one is
  a position implying a rate and the other a displacement, and the right way to combine them
  differs. This crate declares it per action. BEI can add the same later as a defaulted associated
  const on `InputAction`, which breaks nothing, so there is nothing to do now.
- **Which conditions and modifiers ship.** `Flick`, `Cooldown`, `Toggle`: all additive, all
  reversible.
- **The default `Accumulation`.** A default is a major-version change, not a door.
- **Naming.** `TriggerState` vs `ActionState`, `Fire` vs `Fired`. BEI has already done this
  migration once with deprecations, which is the proof it is survivable.
- **The `serialize` format.** As long as it is behind a feature and not the *only* persistence
  story.
- **Whether contexts are components.** They are, and it works; nothing downstream would break if the
  registration API changed shape.
- **Schedule per context type.** `add_input_context_to::<S, C>()` fixes the schedule per type rather
  than per instance. Both crates do this, nobody has complained, and widening it later is additive.
- **`no_std`.** Already done, in both crates.

---

For this crate's own reasoning behind each position: [Requirements.md](../Requirements.md) (R0 for
the layer seams, R9 for timing, R14 for dead zones, R15 for pairing, R18–19 for presentation),
[design.md](./design.md) (TD1, TD5, TD6, TD8.4), [decisions.md](./decisions.md) (D1, D20, D34, D51,
D88, D91), [steam.md](./steam.md) for what a running Steam client was measured to do, and
[deferred.md](./deferred.md) for what it has decided not to build yet. A user-facing comparison of
the three crates is in [comparison.md](./comparison.md).

[bei]: https://github.com/simgine/bevy_enhanced_input
