# One-way doors in upstreaming an input mapper

`bevy_enhanced_input` is [intended for eventual upstream inclusion][bei] as Bevy's input
abstraction. Once that happens it will keep evolving — new conditions, better ergonomics, renamed
types. This document is not about that. It is about the smaller set of decisions that **stop being
revisable** at the moment the crate becomes the engine's answer, because reversing them would mean
changing what input *is* for every downstream crate at once.

For each one: what it commits to, what that buys, what it forecloses, and whether there is a cheap
hedge that keeps the option open without paying for it now.

Written against `bevy_enhanced_input` 0.26.0 (Bevy 0.19), read from source on 2026-08-31. Written by
the author of a different input crate, so read it as an interested party's list. Doors 4 and 7 are
ones **this crate has not got through either**, and say so where they stand — a list that only found
faults in someone else's design would not be worth reading.

## The list, most expensive to reverse first

---

### 1. Input is sampled as a level, not consumed as an event

**The commitment.** `InputReader` reads `Res<ButtonInput<KeyCode>>`,
`Res<ButtonInput<MouseButton>>`, `Res<AccumulatedMouseMotion>`, `Res<AccumulatedMouseScroll>` and
`Query<&Gamepad>` directly, inside the evaluation system (BEI's `src/context/input_reader.rs`).
Every condition and modifier is written against "the value of this binding, now".

**What it buys.** Simplicity, and universality. Anything that can write `ButtonInput` is an input
source, with no adapter — which for an engine-level crate is a serious virtue. No queue, no
windowing, no timestamps to get wrong, no decision about how long an event lives.

**What it forecloses.**

- *Sub-frame edges.* Bevy's `ButtonInput` is cleared and refilled each frame, so a press and release
  delivered in the same frame leave `pressed()` false. `InputReader` reads `pressed()` and not
  `just_pressed()`, so that tap is not merely deprioritised, it is invisible. (LWIM has the same
  property.) Games that care are rhythm, fighting, and anything where a fixed tick runs slower than
  render.
- *Mapping as a pure function of a serializable record.* Determinism, replay, and rollback all want
  the same thing: an input record you can save, ship over a wire, and re-derive action state from.
  With level sampling the only replayable artifact is the *output* — action values and states — so a
  replay bypasses bindings, conditions, chords and consumption rather than exercising them.
  `ActionMock` and `ExternallyMocked` are that output-level seam and they work; what is foreclosed
  is the input-level one.
- *Anything that must happen below the mapper, per device unit.* Stick calibration is the concrete
  case: drift is a wear characteristic of one physical pad, so the correction has to be applied
  where the message still names its sender. Reading a merged level table means that information is
  already gone.
- *Per-window fixed-tick draining.* `add_input_context_to::<FixedPreUpdate, C>()` gives a fixed
  context its own consumption and its own event firing, which is most of what fixed timestep needs.
  What it cannot give is a *different value* per tick, because all ticks in a frame read the same
  level table.

**Why it is one-way.** Not because the sampling code is hard to change — it is one file — but
because the *contract* propagates. Every third-party condition in the ecosystem will be written
against a synchronous `value(binding)` call. Converting to edges later means either redefining what
that call returns (silently changing behaviour) or running both paths (two sources of truth, and
every condition rewritten).

**The cheap hedge.** Make the reader a public, substitutable seam rather than a private system
param. BEI already has most of this: `CustomInput` / `CustomInputs` is a public resource that
bindings read from, and `Binding::Custom` is a first-class variant. Widening that from "inputs Bevy
does not model" to "the layer all input arrives through" costs little now and is the whole
difference later. Even short of that, giving `InputReader` a named public type and documenting it as
replaceable leaves the door ajar.

---

### 2. Actions and bindings are entities

**The commitment.** `Action<A>` is a component on its own entity, related to the context entity by
`ActionOf<C>`; `Binding` is a component on a further entity related by `BindingOf`. This is the API,
not an implementation detail — `actions!` and `bindings!` are how you declare input, and
`Query<&Action<Jump>>` is how you read it.

**What it buys.** A great deal, and it should be said first:

- Input maps are **authorable from a scene**, which for an engine is close to decisive.
- A third-party crate can add an action to a context **it does not own**, with no cooperation from
  the owner and no registration API. This is the thing a closed, compile-time model genuinely cannot
  do, and it is the strongest single argument for the shape.
- Change detection, inspectors, and editor tooling all work with no extra machinery.
- No parallel registry to keep in sync with the world.

**What it forecloses.**

- **A declared baseline distinct from the live bindings.** This is the sharpest consequence and the
  one with player-visible effects. Because the binding entities *are* the source of truth, a rebind
  mutates the only copy. So a saved input configuration is necessarily a full replacement, and a
  patch that ships revised default bindings reaches nobody who has ever opened the controls screen.
  Storing overrides as a *diff* against a retained declaration is what avoids that, and it needs a
  declaration to diff against.
- **Cheap snapshot and restore.** Action state spread across components on many entities is
  entity-shaped to save and restore, which is exactly the operation rollback netcode wants to be
  nearly free. (This crate's equivalent is two `Copy` slices and a dirty bitset.)
- **Parallel evaluation, and per-instance cost.** Because contexts, actions, bindings, modifiers and
  conditions are all entities with dynamically-resolved component ids, evaluation is one system
  built with `ParamBuilder`/`QueryParamBuilder`, iterating a resource (`ContextInstances<S>`) that
  holds every context instance in the world in one sorted `Vec`. That is a sequential pass over
  global state. It is fine at the scale games actually use, and it is a ceiling that a data-oriented
  layout would not have.

**Why it is one-way.** Total. Every downstream scene file, every third-party crate that spawns an
action, every tutorial. This is the decision.

**The cheap hedge.** The first bullet is separable from the rest, and is the one worth acting on:
keeping a *retained declaration* — bindings spawned from a registered template that outlives them —
would preserve overrides-as-a-diff without giving up entities for anything else. It is much cheaper
to add before there is an ecosystem of saved configurations than after.

---

### 3. An action's identity is its Rust type

**The commitment.** There is no name for an action other than the type. `Action<A>` requires
`Name::new(any::type_name::<A>())`, and because the action↔binding association is an ECS
relationship, persisting an input configuration means reflect or scene serialization, which keys on
`TypePath`. So the string that ends up in a player's settings file is `my_game::actions::Jump`.

**What it buys.** Nothing to declare, nothing to keep in sync, and no second name that can disagree
with the first. `#[derive(InputAction)] #[action_output(bool)] struct Jump;` is the whole
declaration, which is genuinely nice.

**What it forecloses.** A refactor that is free. Renaming `Jump` to `Leap`, or moving it from
`actions.rs` into `actions/movement.rs`, orphans every binding every player has saved against it —
and the second of those is not even a rename, it is tidying. LWIM has a milder version of the same
problem: serde keys on the enum variant name, so a module move is harmless but a variant rename is
not.

The consequence is that the safest thing a game can do is never reorganise its action types, which
is a strange constraint for a refactor-friendly language to impose. In practice games discover it
the first time a player reports lost keybindings after an update, and the fix by then is a migration
table keyed on old type names.

**Why it is one-way.** Once an ecosystem of saved settings files exists keyed on type paths, adding
a declared name later means either migrating everything or carrying two identities forever. It also
compounds with door 2: with no retained declaration *and* no stable name, a saved configuration is a
full replacement keyed on something a refactor can change, which is the worst of both.

**The cheap hedge, and this is the cheapest one on the list.** An optional attribute defaulting to
today's behaviour:

```rust
#[derive(InputAction)]
#[action_output(bool)]
#[action_path = "gameplay.jump"]   // optional; falls back to type_name::<A>()
struct Jump;
```

Nothing changes for anyone who does not use it, and a game that does gets a name a refactor cannot
touch. It is worth noting that the string wants to do a second job — a controls screen needs a
localization key for the row's label, and the same declared name serves — which is an argument for
making it a real declaration rather than a serialization-only annotation.

*(This crate requires it rather than offering it: `#[action(path = "gameplay.jump")]`, with a
`<namespace>.<name>` convention and the rule that a path does not follow the type. Requiring it is a
defensible cost — one more line per action — but it is not the only workable answer, and defaulting
to the type name is clearly the right migration path for a crate that already has users.)*

---

### 4. Context priority and consumption are globally scoped

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

**This crate has the same defect**, and has not fixed it: its `ConsumedControls` and its exclusion
ceiling are both global, for the same reason — no in-tree game has yet observed the failure. Its
device *routing* is per-entity (a `Paired` component filters events before anything reads them), so
the two players do not hear each other's hardware; but their claims still share a table. So this is
a shared open problem, not a BEI-specific one. It is on the list because the cost of fixing it is
asymmetric: cheap before a public API exists, expensive after.

---

### 5. An action's type says its output shape and nothing else

**The commitment.** `InputAction` has exactly one associated item, `type Output: ActionOutput`, one
of `bool` / `f32` / `Vec2` / `Vec3`. How several bindings feeding one action combine is a *separate*
runtime setting, `ActionSettings::accumulation` (`Cumulative`, the default, or `MaxAbs`).

**What it buys.** One attribute to write, one concept to teach. `#[action_output(Vec2)]` and you are
done.

**What it forecloses.** The engine's ability to know what a value *means*, as opposed to what shape
it is. A stick deflection and a mouse delta are both `Vec2`, but one is a position implying a rate
and the other is a displacement that already happened. Consequences:

- The right combination rule differs between them — a delta should sum across two devices, a
  position should not — so with one shape and one default, one of the two cases is always wrong
  until the game overrides it per action.
- A nonsensical binding (a stick driving a mouse-look action) cannot be refused at declaration time,
  so it is discovered as camera drift.
- The conversion between the two, which needs the tick's `dt`, has no place to be *required*. Games
  reach for `DeltaScale` when they remember to.

**Why it is one-way.** Adding a required associated const to `InputAction` breaks every action type
in the ecosystem, and there will be a lot of them.

**The honest counterweight.** This may be a door worth walking through deliberately. The concept is
another thing to declare, it makes the both-a-mouse-and-a-stick-on-one-action case *harder* rather
than easier (this crate refuses it outright pending an explicit rate conversion), and `DeltaScale`
plus `SmoothNudge` covers what most games actually need. The argument for having it is that the
error it prevents is one that ships.

**The cheap hedge.** A *defaulted* associated const costs nothing today and is not breaking to give
meaning to later:

```rust
pub trait InputAction: 'static {
    type Output: ActionOutput;
    const SEMANTICS: ValueSemantics = ValueSemantics::Unspecified;
}
```

Even leaving it unused, it reserves the space.

---

### 6. Specificity is spelled through consumption, and only over modifier keys

**The commitment.** Actions within a context are sorted by the maximum `ModKeys` count across their
bindings, so `Ctrl+S` is evaluated before `S` (BEI's `src/context.rs:485`). But order alone decides
only who goes *first*; suppressing the shorter binding requires
`ActionSettings { consume_input: true }`, which is off by default. With defaults, pressing Ctrl+S
fires both actions. General chords are a separate mechanism (the `Chord` condition, referencing
another action) and do not participate in the ordering at all.

**What it buys.** One mechanism instead of two. Consumption already exists for the
menu-over-gameplay case; reusing it for chord specificity means no separate arbitration pass.

**What it forecloses.** Specificity as a property of the *bindings* rather than of a runtime flag. A
plain `S` binding fires whether or not Ctrl is held, so correctness depends on remembering to opt in
per action, and only for modifier keys. LWIM's `ClashStrategy::PrioritizeLongest` is automatic,
default, and general over its `BasicInputs` decomposition — so the alternative is not hypothetical.

**Why it is partly one-way.** The default is flippable in a major version; that part is ordinary
evolution. What is harder is that deciding "the longest satisfied chord on this control wins" before
anything is read requires a pre-pass over the bindings, which in an entity model means walking
binding entities twice per context per tick. Doable, but a different evaluation shape than the
current single ordered pass — so the further the ecosystem gets built on consumption-as-specificity,
the more expensive.

**The cheap hedge.** Document consumption and specificity as distinct concerns even while one
implements the other. That alone keeps a later pre-pass from being a behavioural surprise.

---

### 7. There is no query surface designed for the player-facing half

**The commitment.** Not an active decision so much as an absence: rebinding UIs are expected to
query `Binding` entities and mutate them. Rendering is `impl Display for Binding`, which formats
`{key:?}` — "KeyD", "Space", "Control + KeyD".

**What it buys.** Nothing to design, and it does work — a game can and does build a controls screen
on it today. That is more than nothing, and more than LWIM offers.

**What it forecloses, once this is the engine's answer.** A layering problem shows up the moment the
mapper is upstream: an input crate cannot depend on `bevy_ui`, so the rebinding and prompt layer has
to live above it and needs a stable query surface from below. If that surface is "iterate the
binding entities and call `Display`", then:

- Every controls screen in the ecosystem reimplements which bindings should be *shown* to a player,
  which are *rebindable*, and what a primary/secondary slot is — and they will not agree.
- `Display`'s output is a debug string in a user-facing position. `{key:?}` is not localizable and
  not stable across `KeyCode` renames. Committing to it as *the* answer is the door; keeping it as a
  fallback beneath a structured form is not.
- A backend that owns the bindings (Steam Input) has no way to answer "what is bound to Jump" in the
  same shape the built-in path does, so prompts cannot be written once.

**Why it is one-way.** The layering is. Once a rebinding crate exists above the engine's input crate
and is written against whatever surface was there, that surface is load-bearing.

**The cheap hedge, and it is genuinely cheap.** Two things, neither of which requires building a UI:

1. A **reverse lookup behind a trait** — action → the controls that would fire it now — so an
   external authority can answer it instead. This crate's version returns an origin that need not be
   one of its own control types, which is the whole cost of making Steam Input possible later.
2. A **structured name for a control**, separable into a localization key plus a fallback string,
   rather than only a `Display` impl.

**This crate does not get to be smug here either**, though the reason is worth stating precisely.
Its presentation layer and its `bevy_ui_widgets` bridge both live in `examples/common/` rather than
in the crate, hitting exactly this constraint from the other side. Both are built, and both were
written against the public API without anything being added to the crate for them — which is the
evidence that the seam holds. What has not happened is *packaging*: giving them a crate of their own
means splitting this repository into sub-crates, and until that happens they are a `#[path]` import.
"Exercised but not packaged" is a weaker claim than "shipped", and the distinction is the whole
reason to design the query surface before there is a crate that needs it.

---

### 8. Where the gamepad dead zone is owned

**The commitment.** `InputReader` reads the `Gamepad` component, which is downstream of Bevy's own
`GamepadSettings` filter. The mapper therefore inherits a filtering policy rather than setting one,
and applies its `DeadZone` modifier on top.

**What it buys.** Consistency with the rest of Bevy — `Gamepad` is *the* gamepad API, and an input
mapper that read something else would be the odd one out.

**What it forecloses.** Anything that must happen below the game's own dead zone: per-unit
calibration for a worn stick, and a player-facing dead-zone control that can honestly go to zero.
Once the mapper reads filtered values, "who owns the dead zone" is settled in favour of
`GamepadSettings`, and inserting a stage underneath means changing what `Gamepad` means.

**Why this one is different.** It stops being BEI's decision at the moment of upstreaming — it
becomes Bevy's, and Bevy could resolve it either way, including by making `GamepadSettings` a policy
the mapper *applies* rather than inherits. That is why it is on the list: it is a door that gets
walked through as a side effect of the move rather than by anyone choosing.

*(The precise current behaviour is version-dependent: on Bevy `main` the `Gamepad` component stores
the unscaled raw value and the dead zone affects the change-detection threshold and the emitted
event's scaled value. The ownership question is the same either way.)*

---

## What is not a one-way door

Worth stating, because a list of concerns is only useful if it excludes the ordinary:

- **Which conditions and modifiers ship.** `Flick`, `Cooldown`, `Toggle` — all additive, all
  reversible, none load-bearing.
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

## If only three things get done

Ranked by (cost of reversing later) ÷ (cost of hedging now):

1. **Make a saved configuration survive the game changing** — doors 2 and 3, which are one concern
   wearing two hats and compound badly together. Today a saved input map is a full replacement
   (nothing retained to diff against) keyed on a name a refactor can change (the Rust type path). So
   a patch's revised defaults reach nobody who has ever opened the controls screen, *and* tidying
   `actions.rs` into a module orphans everyone's keybindings. Two hedges, both additive: a retained
   declaration for the bindings, and an optional `#[action_path = "…"]` defaulting to today's
   behaviour. The second is the cheapest item on this whole list.
2. **Put an owner on a consumption claim** (door 4). One field, and the alternative is local
   multiplayer that quietly cross-talks. This crate should do it too.
3. **Make the input reader a public, substitutable seam** (door 1), by widening `CustomInputs`
   rather than inventing anything. It does not commit to event-based input; it just stops
   foreclosing it.

Doors 5 and 7 both have hedges that cost roughly one line and one trait respectively — a defaulted
associated const, and a reverse lookup behind a trait — and are worth the line.

---

For this crate's own reasoning behind each position: [Requirements.md](../Requirements.md) (§0 for
the layer seams, §9 for timing, §14 for dead zones, §15 for pairing, §18–19 for presentation),
[design.md](./design.md) (§1, §5, §6, §8.4), [decisions.md](./decisions.md) (D1, D20, D51), and
[Roadmap.md](../Roadmap.md)'s deferred
table for what it has not built. A user-facing comparison of the three crates is in
[comparison.md](./comparison.md).

[bei]: https://github.com/simgine/bevy_enhanced_input
