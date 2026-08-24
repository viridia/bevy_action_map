# Requirements: `bevy_action_map`

> Status: draft for review. `MUST` / `SHOULD` / `MAY` per RFC 2119. Requirements are numbered
> `R<section>.<n>` so they can be referenced from design docs and PRs. Tags like **(D3)** mark
> requirements that follow from a settled decision — see [Resolved decisions](#resolved-decisions) at
> the end, alongside the questions still open.

A greenfield input-mapping system for Bevy, incorporating lessons from leafwing-input-manager
(LWIM), bevy_enhanced_input (BEI), Unreal Enhanced Input, Unity's Input System, Steam Input, and
Godot. This document catalogs the design issues before any code exists, so that nothing structural
is discovered late. It is a requirements document: it states what the system must do and why, and
defers *how* to a design phase — where it does record a decision, that decision constrains the
design space rather than describing an implementation.

## Scope

Boundaries set before drafting, which explain why some obvious topics are absent and some
easily-deferred ones are not:

| | |
| -------------------------- | ------------------------------------------------------------------------------------------------ |
| Layers owned               | Raw→action mapping, device abstraction, binding presentation. **Not** focus/event dispatch.      |
| Codebase conventions       | Upstream-Bevy candidate: `no_std` where feasible, minimal deps, `Reflect`, upstream conventions. |
| Users served               | Both funded studios and the solo long tail — see [Who this is for](#who-this-is-for).            |
| First-class (not deferred) | Local multiplayer / device pairing; netcode determinism.                                         |

## Who this is for

Two constituencies, and the design has to hold both at once. Most of the tensions in this document
are really this one tension wearing different clothes, so it is worth stating plainly rather than
rediscovering it per section.

**Funded studios** need the ceiling. Localization, device pairing, control schemes, rollback
determinism, external binding backends, console certification behaviour — a crate that cannot
express these is one a studio replaces, and replacing an input layer late is expensive. They have
budget for setup, staff for QA, and translators. Configuration is not what they are short of.

**The long tail** — solo developers, jam entries, two-person teams — is the larger constituency by
count and is short of exactly what the studio has. No localization budget. One controller on the
desk, and per §14 possibly a controller that lies about itself. No QA pass. No time to read
twenty-four sections of requirements before moving a character. What they cannot tolerate is a
floor: setup that must be completed before anything works at all.

**The commitment is that the ceiling must not raise the floor.** Every mechanism in this document
that exists for the studio must be _additive_ — absent until declared, with defaulted behaviour that
is correct when it is absent. A solo developer should be able to bind WASD and ship, never having
learned that mappings, control schemes, or localization keys exist, and a studio should be able
to reach all three without leaving the crate.

Three consequences, each of which this document already obeys in places and should obey everywhere:

- **Additive, never prerequisite.** Mappings (R19.10), tunables and presets (R19.13), localization
  keys (R19.14), pairing (§15), and persistence (§17) are declarations a game opts into. None may
  become a step you must perform before an action fires.
- **Defaults must be right without being tested.** The long tail cannot verify what it does not
  own — a second gamepad, a Steam Deck, an AZERTY keyboard, a right-to-left locale. So a default
  that is merely _reasonable_ is not good enough; it has to be the one that survives hardware and
  locales the author never sees. This is also why the diagnostic tiers of §4.R4.8 matter more here
  than they would in a crate aimed only at studios: a mistake caught at build time is one the solo
  developer does not need a QA department to find.
- **Accessibility must not be what falls off the bottom.** This is the place the commitment is
  hardest to keep, and it deserves the scrutiny. Making rebinding opt-in (R19.10) is right for the
  jam entry's effort budget and wrong for its players, since the likely outcome is a game with zero
  remappable controls (§20.R20.1). The resolution is not to flip the default — that would ship
  unintended rebinding surfaces — but to make the accessible path *cheap*: declaring a whole
  context's buttons mappable should be one line, not one line per action. Where a default trades
  away accessibility, the obligation is to shorten the accessible path, not to accept the trade.

  The *listing* half of R19.10 went the other way for exactly this reason. A jam entry that declares
  nothing still has controls a player can read, which is the part of R20.1 that can be had for free;
  what stays opt-in is only the part that a developer has to have thought about.

## Orientation: if you have used LWIM or bevy_enhanced_input

Most of this document will be familiar in outline and unfamiliar in the details, because the details
are where input systems accumulate obscure requirements. What carries over from those crates, and
what does not:

| | |
| --- | --- |
| Actions as types with a derive | As in BEI. Unlike LWIM's one-enum-per-map, which cannot be extended by another crate (§1). |
| A richer state machine than pressed/just_pressed | `Ongoing` / `Fired` / `Canceled`, as in BEI and Unreal — needed for holds, chords, and hold-to-confirm UI (§3). |
| Contexts with priority | As in BEI, plus Steam's _additive layers_, which override part of a context rather than replacing it (§7). |
| Deadzones owned end to end | **Differs from both.** LWIM and BEI consume Bevy's already-deadzoned gamepad values; this crate consumes raw events and owns all three deadzone stages, because a clamp applied below you cannot be undone above you (§14). |
| Fixed vs. render timing | **Neither addresses this.** Reading `just_pressed` in `FixedUpdate` can miss or duplicate presses; §9 treats that as a requirement rather than a caveat. |
| Determinism and rollback | **Neither addresses this.** §10 requires mapping to be a pure, snapshot-able function so past ticks can be re-derived. |
| Local multiplayer | **Neither handles the general case.** Device-to-player assignment is many-to-many, not an index (§15). |
| Prompts, glyphs, rebinding UI | **Neither has a real answer.** §18 and §19 treat "what is bound to Jump right now, for this player, on this device" as a first-class query. |
| External binding backends | **Neither supports this.** Steam Input owns bindings entirely; §0 and §18 make that a supported configuration rather than an incompatibility. |

The sections most worth reading closely, because they contain requirements that are easy to discover
too late, are §9 (timing), §14 (deadzones), §15 (pairing), and §17 (persistence).

## 0. Purpose and layering

The crate translates _physical input_ into _semantic actions_, tracks the devices that produce it,
and can report back what is currently bound to what. It is explicitly four layers, and the seam
between each pair must be a public, replaceable API — this is what lets a Steam Input backend, a
replay system, or a test harness mapping in.

```
L0  Sources        device enumeration, identity, capabilities, hot-plug, calibration
L1  Input frame    normalized, timestamped, serializable per-tick snapshot of L0
L2  Mapping        bindings → modifiers → conditions → action value + state machine
L3  Presentation   reverse lookup: action → bindings → display string / glyph id
```

**Backend seam (D3).** An external backend may substitute for either of two layers. The distinction
between them is easy to miss, and getting it wrong scopes the feature as a presentation concern when
it is actually a structural one:

- a _source_ backend supplies L1 and lets our pipeline map it (a custom device, a replay, a network peer);
- an _authority_ backend supplies **L2 output directly** — action values and states — bypassing our
  bindings, modifiers, and conditions entirely. Steam Input is this case: the binding UI, the conflict
  rules, and the glyphs all live outside the game, and `GetDigitalActionData` returns the answer.

Supporting the second is the structural commitment. It means action state must be writable from
outside the mapping pipeline, presentation must not assume our binding tables exist, and rebinding
must be delegable to the backend's own UI.

- **R0.1 (MUST)** Each layer is usable without the layers above it. L1 alone must be a usable
  normalized input API; L2 must be drivable from a hand-constructed L1 frame. _(L1 is a timestamped
  queue drained by window (Design §2) and not a snapshot: §9's timing requirements are unsatisfiable
  by a snapshot, in which a press and release inside one frame collapse to nothing.)_
- **R0.2 (MUST)** L2 must not read `ButtonInput`/`Axis`/raw `Message`s directly; it consumes only L1.
  This is the single most important structural rule — determinism (§10), testing (§21), replay, and
  external binding backends all depend on it.
- **R0.3 (MUST)** No layer may require a `World` singleton resource that prevents multiple
  independent instances (per player, per replay stream, per test).
- **R0.4 (MUST)** A backend may act as an _authority_ for a subset of actions: their state is
  written by the backend and our mapping pipeline must not also compute them. The split must be
  per-action or per-context, not all-or-nothing — a game using Steam Input for gameplay may still
  map its own debug and editor bindings.
- **R0.5 (MUST)** Consumers of action state (gameplay code, prompts) must not need to know which
  backend produced it. Backend identity is queryable but never required at the call site.
- **R0.6 (MUST)** A backend that is authoritative for a device must be able to suppress that device
  at **L0**, so its raw events never reach the input frame at all. R0.4 stops us computing an action
  the backend owns; it does not stop us sampling the hardware underneath it, and on the case that
  motivates D3 the hardware is still there — Steam presents a pad it is driving as an emulated
  gamepad, which the platform enumerates and we sample, so every input arrives twice. The same
  capability is what lets a replay backend mute live hardware while it plays.

### Non-goals

- **Focus-based event dispatch and bubbling.** Delegated to `bevy_input_focus`
  (`InputFocus`, `FocusedInput<E>`) and `bevy_picking`. We must _interoperate_ closely — a focused
  widget has to be able to claim inputs that would otherwise reach the game (§22) — but we do not own
  the dispatch tree, and we trigger bubbling rather than implementing it.
- Pointer→world raycasting (`bevy_picking`), aim assist, anti-cheat, gesture recognition beyond what
  the OS reports, and full haptics authoring (§14 covers only routing haptics to the right device).
- Shipping glyph _assets_ (§18 defines identifiers; assets are the app's problem).

---

## 1. Action identity

**Problem.** How an action is named determines modularity, serializability, and whether actions can
be created at runtime (mods, scripting, data-driven bindings).

**Prior art.** [LWIM][lwim]: one `Actionlike` enum per `InputMap<A>` — cheap and type-safe, but a closed set;
two plugins cannot contribute actions to one map. [BEI][bei]/[Unreal][unreal-ei]: each action is a distinct type/asset —
open set, composable, but no exhaustive match and heavier per-action ECS cost. Unity/Godot/Steam:
interned strings or asset-relative paths — fully dynamic, no compile-time checking, stable across
serialization.

**Decision (D1).** Actions are declared as types, with a derive macro carrying the shape and metadata:

```rust
#[derive(InputAction)]
#[action(path = "gameplay.move", output = Vec2, name = "Move", category = "Movement")]
struct Move;
```

This gives an open set (R1.2), namespacing (R1.5), and — the real payoff — compile-time-checked
typed reads: `actions.get::<Move>() -> Vec2`, not `ActionValue` plus a runtime shape check.

**Runtime identity.** `TypeId` is not stable across builds, so it cannot be the serialized identity.
The derive therefore registers a stable name, and the runtime identity is an interned `ActionId`. The
type is a compile-time _handle_ to an id, not the id itself — a distinction forced by serialization
regardless of any view on dynamic actions.

**The stable name is declared, not derived (D8).** The obvious source for it is the reflected type
path, which costs the author nothing. That is the wrong default here, and the reason is what the name
is _for_: it is a key in the player's settings file, and it has to outlive every refactor of the code
that declares it. A derived path makes the name a function of where the type happens to live, so
moving `Move` into a submodule, renaming the module, or splitting a crate silently invalidates every
saved binding — a data-loss bug with no compile error and no runtime error, discovered by players
after the patch ships.

Declaring the path makes the identifier a deliberate, reviewable choice with the same stability
obligations as any other serialized key, and severs it from Rust's module structure so that
refactoring is free. The cost is one required attribute per declaration, paid once. Because a
declared name is no longer namespaced by construction, R1.5's collision-avoidance now rests on a
convention rather than on the compiler, which is what R1.8 exists to supply.

**On data-declared actions.** Declaring actions as types rules out creating them at runtime, which
raises the question of whether mods, scripting, or data-driven binding files need to mint their own.
The argument that they do not: any code affected by an action needs a priori knowledge that the action
exists, so an action nobody was compiled against has no one to act on it. Avoiding that by decomposing
actions into generic well-known units — so that arbitrary new actions become expressible as
combinations of primitives — trades a clear model for a vague one.

The one case that genuinely escapes the argument is a consumer that _has_ a priori knowledge but is
**not Rust code**: a scripting or modding layer where the script knows about its own action but
cannot declare a Rust type. Even there, the conventional answer is not dynamic declaration but
_indirection_ — ship well-known mapping actions (`Ability1..8`, `CustomBind1..N`) that mods attach
behavior to. That is indirection at the binding layer, not decomposition of actions into primitives,
and it covers most of the territory.

Conclusion: dynamic declaration is **not a v1 feature**. The door stays open at essentially zero cost
because R1.1 already forces the id to be decoupled from the type.

- **R1.1 (MUST)** _(D8)_ Serialized action identity is an author-declared path string, never `TypeId`,
  never a bare `Entity`, and never derived from the Rust type path. The derive must require it rather
  than defaulting, so that no action can acquire a serialized identity nobody chose.
- **R1.2 (MUST)** The action set must be open: two independent crates can define actions that coexist
  in one context with no coordinating enum. _(Satisfied by D1.)_
- **R1.3 (MUST)** The runtime representation is an interned `ActionId`, obtainable from a type
  (`ActionId::of::<Move>()`) and resolvable from a stable name (needed by §17 persistence and §18
  external backends anyway). Runtime _declaration_ of new actions is out of scope for v1; APIs should
  nonetheless accept `ActionId` rather than a type parameter wherever they do not need the type, so
  that adding it later is additive.
- **R1.4 (MUST)** `ActionId` must be `Reflect`, `Hash`, `Eq`, and `Copy`.
- **R1.5 (SHOULD)** Namespacing to avoid collisions between crates. _(Under D8 this is a property of
  the declared path rather than of the type path, so it is carried by the naming convention in R1.8
  rather than enforced by the compiler.)_
- **R1.6 (MUST)** The derive must be able to express, at minimum: output shape and intent
  (§2.R2.3, R2.7), a **category** for grouping in a rebinding UI (§19.R19.6), and default consume
  behavior (§8.R8.2). Rebindability is _not_ on the action — it is a property of a declared mapping
  (§19.R19.10), since one action can have several bindings of which only some are player-mappable.

  _(The player-visible **name** belongs to the mapping (R19.9) rather than here, because a composite
  settles it: `Move` has four mappings and the player must be shown "Move Forward", never "Move".
  The category stays here, because four movement mappings share one category and repeating it per
  mapping is four chances to disagree. Both are localization keys per R19.14, not display text.)_
- **R1.7 (MUST)** Action names must be expressible in an external backend's namespace (Steam IGA
  action names are authored in a separate file and must match exactly), which is a second reason the
  path is declared rather than derived (D8): the author has to be able to spell the name the backend
  expects.
- **R1.8 (MUST)** _(D8)_ Because declared paths are unchecked strings, the crate must publish a
  naming convention for them and follow it throughout its own documentation, examples, and tests. The
  convention must cover the separator, the case, and how a crate namespaces the actions it
  contributes, and must state the stability obligation: a path is a serialized key, so changing one
  breaks saved bindings and is a breaking change to be migrated (§17.R17.3), whereas renaming or
  relocating the Rust type is not.

---

## 2. Action value model

**Problem.** Actions carry values of different shapes; over-typing forces duplicate APIs, under-typing
loses safety.

**Prior art.** LWIM: `InputControlKind::{Button, Axis, DualAxis, TripleAxis}` declared per action.
BEI/Unreal: `ActionValue::{Bool, Axis1D, Axis2D, Axis3D}` with defined lossy conversions between
dimensions (a bool feeding a 2D action, etc.). Unity: Value / Button / PassThrough — the PassThrough
distinction (no conflict arbitration between concurrent controls) is often overlooked.

**Shape is not intent.** All of the above describe the *size* of a value, not what it means, and
that is not enough. A mouse delta and a stick deflection are both `Vec2`, but the first is a
displacement already expressed per frame and the second is a position that implies a rate — summing
them produces a value whose units are undefined. Steam makes the distinction first-class: an action
is declared as `StickPadGyro`, `AnalogTrigger`, or `Button`, and a two-axis action further declares
an `input_mode` of `joystick_move` or `absolute_mouse`, which Steam's binding UI then uses to decide
which physical controls may drive it ([IGA file][steam-iga]). Two things in this document need that
distinction: what a rebinding UI may legally offer for a mapping (§19.R19.1), and what happens when
sources of different kinds feed one action (R2.9, §13.R13.2).

- **R2.1 (MUST)** Support bool, 1D, 2D, 3D value shapes in one runtime value type.
- **R2.2 (MUST)** Dimension conversion rules must be explicit, documented, and total, and the
  **specific rule for each pair must be settled here rather than left to the implementation**.
  _(A requirement that a decision be written down is not a decision. Rules left to the
  implementation get settled by whoever writes the first conversion, and are then hard to revisit.)_

  Two rules cover the table, and both come from the same principle: **a conversion may discard
  information, but must never invent any.**

  - **Widening** places the value in the first component and leaves the rest at zero.
  - **Narrowing to a scalar** — to 1D or to bool — measures the value as a whole, so a vector
    becomes its length and a bool is that length being non-zero. **Narrowing between vector shapes**
    drops the trailing components, which are the ones the target shape does not name.

  | from ↓ to → | `bool`     | `Axis1`  | `Axis2`     | `Axis3`         |
  | ----------- | ---------- | -------- | ----------- | --------------- |
  | `bool`      | itself     | 1.0, 0.0 | `(v, 0)`    | `(v, 0, 0)`     |
  | `Axis1`     | `v != 0`   | itself   | `(v, 0)`    | `(v, 0, 0)`     |
  | `Axis2`     | `v != 0`   | length   | itself      | `(x, y, 0)`     |
  | `Axis3`     | `v != 0`   | length   | `(x, y)`    | itself          |

  Two consequences worth stating because they are the parts that get argued about. Narrowing to 1D
  **loses the sign**, so an action that wants a signed reading must be bound to a single axis rather
  than to a whole stick — the crate provides both as separate sources for exactly this reason. And
  the bool conversion is a test against rest, **not** against a press threshold: a control's press
  threshold (§14.R14.2) is applied where the control is read, before the value is stored, so by the
  time a stored value is being reshaped the question has already been answered.
- **R2.3 (MUST)** Each action declares its output shape in its derive (D1); a binding whose natural
  shape differs is either converted per R2.2 or rejected with a clear diagnostic. Because the shape is
  an associated type, typed reads must be checked at compile time — binding a `Vec2` action and
  reading it as `bool` should not compile, rather than failing a runtime shape check.
- **R2.4 (WITHDRAWN)** ~~Distinguish _value_ actions (continuous, arbitrated) from _pass-through_
  actions (every contributing source visible, no arbitration).~~

  Unity's distinction, whose motivating cases turn out to be device-shaped rather than value-shaped
  and are answered elsewhere here: device scoping by §15.R15.3 and R15.4, per-source visibility by
  R22.2's inspector dump, and a value that remembers its origin by R2.6. What remained was a second
  storage shape carried by every action so that a few could use it. If a case appears that genuinely
  needs it, it should arrive as its own requirement with that case attached.
  ([Log](./Log.md#the-review-and-the-requirements-amendments-it-produced))

- **R2.5 (SHOULD)** Values must not be normalized/clamped implicitly; clamping is an explicit modifier
  so that e.g. mouse deltas and analog sticks can share a pipeline (§5).
- **R2.6 (MAY)** Carry a "source" tag on the value (which device/binding produced it) for prompts and
  for last-used-device tracking (§18).
- **R2.7 (MUST)** An action declares an **intent** in addition to its output shape (R2.3). The
  taxonomy must at minimum distinguish: digital button; analog 1D (trigger-like); 2D directional
  (stick-like — a position implying a rate); and 2D delta (mouse-like — a displacement already
  expressed per frame). Intent is a property of the action, shape is a property of its value, and
  neither implies the other.
- **R2.8 (MUST)** Intent constrains which control sources a binding may use, and is what a rebinding UI
  filters candidate controls on (§19.R19.1). Without it, a rebinding UI can only filter by shape, and
  will offer the player bindings that are legal but nonsensical.
- **R2.9 (MUST)** When bindings of differing intent feed one action — mouse-and-stick look being the
  near-universal case — the conversion into the action's declared intent must be explicit and
  documented per source kind. Summing a per-frame displacement and a per-second rate into one value is
  a units error, and the pipeline must make it impossible rather than merely discouraged.
- **R2.10 (MUST)** The **shape of the source channel** is a third independent property, distinct from
  both the action's intent (R2.7) and its output shape (R2.3); the binding layer must not assume any
  two of the three agree. Two cases from real hardware, both verified against an Xbox Series
  controller on Bevy's gamepad path (see §14):
  - an **analog trigger** arrives on a *button* channel carrying a fractional value
    (`GamepadButton::LeftTrigger2`/`RightTrigger2`, `f32` in `0.0..=1.0`), not on an axis;
  - a **D-pad** arrives as *four discrete buttons*, never as an axis pair (§14).

  Therefore an `Analog1` action must be bindable to a button-shaped source without a special case, and
  a `Directional2` action must accept a four-button composite identically whether the parts are
  keyboard keys or D-pad buttons. A design that keys conversion off the source's *shape* alone will get
  both of these wrong.

---

## 3. Action lifecycle and state

**Problem.** `pressed` / `just_pressed` booleans cannot express hold progress, cancellation, or
"the chord was started but the second key never came".

**Prior art.** Unreal/BEI state machine: `None → Ongoing → Fired → Completed`, with `Canceled` when an
ongoing condition is abandoned; surfaced as five events. Unity: `started / performed / canceled`.
LWIM: polled `ActionState` with `current_duration()` and explicit `consume()`.

- **R3.1 (MUST)** Model an explicit state machine, not just edges. Minimum: idle; **started**, for a
  condition that has begun and not yet been satisfied; ongoing; fired; completed; and canceled, for
  a started condition abandoned before it ever fired.

  _(Six rather than five, because "started" and "ongoing" are different questions and one name
  cannot answer both. A hold that has just been pressed and a hold that is now firing are both
  "not an edge", and a player can tell them apart on screen, so the model must too. Canceled is
  only meaningful against started: what distinguishes it from completed is whether the action ever
  actually happened.)_
- **R3.2 (MUST)** Both polling and event/observer access to state. Polling is required for
  `FixedUpdate` simulation code (§9); events are required for UI and for one-shot commands.
  Event delivery must be attachable **declaratively**, not only from imperative setup code — a scene
  format that supports observer attachment should be able to bind a handler to an action without a
  system running first. Satisfying this means transitions are published as generic entity events
  targeted at the entity owning the context instance (`Fired<Jump>` on the player entity), the pattern
  `bevy_input_focus` already uses for `FocusedInput<M>`. Delivery must not require the action set to be known at
  plugin-build time.
- **R3.3 (MUST)** Every transition must be observable — an action that fires and completes within one
  tick must not be silently collapsed (§9.R9.4).
- **R3.4 (MUST)** Expose elapsed time in the current state, measured in the same simulated seconds
  the action's own conditions use (§9.R9.6), and part of serializable state (§10.R10.3).

  _(Not a clock the caller selects. A context declares one tick domain and is evaluated in it, so
  there is one clock the answer can honestly be given in — the same one the conditions counted with.
  Offering a choice at the point of reading would mean converting between rates after the fact,
  which is how a hold ends up reporting a duration it was never measured against.)_
- **R3.5 (SHOULD)** Expose _progress_ toward firing (0..1) for hold-to-confirm UI — Unreal's CommonUI
  hold indicators need this and it is painful to reconstruct externally.
- **R3.6 (MUST)** Provide "consume"/mark-handled semantics so a binding can prevent lower-priority
  contexts from also reacting this tick.

  _(Narrowed: this also required preventing **the same action's other observers** from reacting,
  and that half is withdrawn. It is the dynamic interception D5 rules out, one level further down —
  an observer electing at handling time to suppress its peers makes the outcome depend on which ran
  first, which is what R8.3 forbids for contexts and is no more defensible for observers. Its
  motivating case is a UI handler out-ranking a gameplay one, and that is already answered by the
  half kept: the UI's context claims the control and the gameplay action never fires at all, which
  is both stronger and inspectable. If a case appears that genuinely needs two observers of one
  firing to arbitrate between themselves, it should arrive as its own requirement with that case
  attached.)_
- **R3.7 (MUST)** Actions must be individually disable-able without unbinding, and re-enabling must not
  spuriously fire (require-reset semantics: a key held across the enable boundary does not count as a
  fresh press).

---

## 4. Binding model

**Problem.** What can be attached to an action, and how are multi-input constructs expressed?

**Prior art.** Unity [control paths][unity-bindings] with wildcards and usages
(`<Gamepad>/buttonSouth`, `*/{Submit}`) plus composites (`2DVector`, `ButtonWithOneModifier`). LWIM `VirtualDPad` / `ButtonlikeChord`. BEI
`bindings![...]` with per-binding modifier/condition lists. Unreal maps context→key→(triggers,
modifiers). Steam moves the whole binding layer out of the game.

- **R4.1 (MUST)** An action may have N bindings; a binding may target one control or a composite.
- **R4.1a (MUST)** The bindable control set covers **keyboard keys, mouse buttons, mouse motion,
  gamepad buttons and gamepad axes**. Mouse buttons are stated rather than implied: "keyboard and
  mouse" is one control scheme (§17.R17.4), and a crate that names that scheme while binding only
  half of it cannot express "fire on left click" — which is not an exotic binding, it is the
  commonest one in the genre. A mouse button reports on a button channel, so anywhere a key can go
  a mouse button can: as a whole binding, as a part of a composite, and as a member of a chord.
- **R4.2 (MUST)** Composites: 1D axis from two buttons, 2D from four (WASD/D-pad), 2D from a stick,
  chord (all-of), and "button with modifier(s)".
- **R4.3 (MUST)** Bindings must be expressible against a _device class_ (any gamepad) as well as a
  _specific device instance_ (player 2's pad) — see §14.
- **R4.4 (SHOULD)** Semantic control aliases (`Submit`, `Cancel`, `MenuLeft`) that resolve per device
  class, so UI code binds once. This is Unity's "usages" and Steam's action-set convention; it is also
  what makes console confirm-button region swaps (§18.R18.7) tractable. An alias resolves to _one_
  control per device class; R4.9 is the same idea where the answer is a set.
- **R4.5 (MUST)** Per-binding modifiers and conditions (§5, §6), not only per-action — the same action
  needs different deadzones for stick vs. mouse.
- **R4.6 (MUST)** Bindings are data: constructible at runtime, serializable, diffable against
  defaults (§17).
- **R4.7 (SHOULD)** Whether a binding is player-_rebindable_ is expressed by declaring a mapping for
  it (§19.R19.9, R19.10), not by a flag on the binding. Whether it is _listed_ is a separate question
  with the opposite default (R19.10): a binding is shown to players unless it asks not to be, and
  only rebinding waits to be declared.
- **R4.8 (MUST)** Building or mutating bindings must produce actionable errors (unknown control,
  shape mismatch, duplicate) rather than silently doing nothing.
- **R4.9 (MUST)** A binding may target a **control class** — a named set of controls — as well as a
  single control or a composite. §8.R8.4 and §12.R12.6 both require a focused text field to claim
  character-producing keys "as a class, without the app author enumerating them"; this is the
  mechanism that satisfies them, and the same vocabulary serves capture filtering (§19.R19.1),
  exclusion lists (R19.2), and reserved controls (OQ-10).

  Three properties, each ruling out an implementation that looks obvious:

  - **Classes are defined by the properties a control declares, never by enumerating controls.** A
    class means "every control whose declared shape is button-like", not a list of `KeyCode` and
    `GamepadButton` variants. This is what lets §11.R11.2's third-party device kinds join a class
    the day their backend ships, rather than requiring a registry they would have to be added to.
  - **Membership may depend on the event, not only on the control.** "Character-producing" cannot be
    a static set of keys: a dead key produces nothing until the following key decides what it
    becomes, and an IME composition consumes keys that produce no text until it commits (R12.6). The
    predicate must see the event.
  - **The set of classes is closed**, and third parties do not define new ones. This is a deliberate
    reversal of the extensibility this document grants modifiers (R5.6) and conditions (R6.6), for a
    reason those do not share: a class over text input is a correctness trap. An author writing one
    by hand will get AZERTY, dead keys, and IME wrong and will never learn it, because the QA pass
    that types Japanese does not exist (R24.8). Where a mechanism is a footgun rather than a
    convenience, the crate owns it.
- **R4.10 (MUST)** A control class must be justified by **non-enumerability**: it exists only where a
  developer could not reasonably write the set out. "Character-producing keys" qualifies, because the
  set is a function of layout and IME state. "Any button-shaped control" qualifies, because the
  device set is open (R11.2). Arrow keys do not — there are four of them, and binding them
  individually is clearer than a class that hides which ones. Modifiers do not either; R12.3's
  left/right/either distinction belongs to the chord mechanism.

  Stating the criterion is what keeps a closed set defensible. Without it the set accretes
  conveniences until it is a query language nobody chose to design, and the argument for closing it
  in the first place evaporates.

  _If a case later demands an open set, the additive answer is the hybrid R5.6 already uses for
  modifiers — a closed enum plus one `Custom` variant — not a registry retrofitted underneath._

---

## 5. Modifier / processor pipeline

**Problem.** Raw values need shaping, and the order of operations is semantically significant.

**Prior art.** Unreal/BEI modifiers: `DeadZone`, `Negate`, `Scale`, `SwizzleAxis`, `SmoothDelta`,
`AccumulateBy`, `ExponentialCurve`, `LinearStep`. Unity [processors][unity-processors]: `Invert`, `Normalize`,
`Scale`, `StickDeadzone` vs `AxisDeadzone`, `Clamp`. LWIM's axis processing pipeline with circle vs. axis
deadzones.

**Open questions.** Are modifiers trait objects (open, boxed, harder to serialize/determinize) or a
closed reflected enum (serializable, extensible only via a registry)? Frame-rate dependence is the
subtle trap: a "smoothing" modifier is a stateful filter and must therefore be rewindable (§10).

- **R5.1 (MUST)** An ordered, per-binding modifier chain with documented, deterministic evaluation order.
- **R5.2 (MUST)** Built-ins at minimum: negate, swizzle/reorder axes, scale/sensitivity, clamp,
  radial (circular) deadzone, per-axis deadzone, response curve (exponent/piecewise). See
  ["Doing Thumbstick Dead Zones Right"][deadzone-article] for why the radial/axial distinction is not
  cosmetic.

  _("normalize" is deliberately absent from this list. The word names two incompatible operations,
  so it is split out and disambiguated as R5.9.)_
- **R5.9 (MUST)** Both meanings of "normalize" must be available and must not share a name:
  - **clamp to unit length** — scale a vector down if it exceeds magnitude 1, leave it otherwise.
    This is what keeps a composite of four keys from exceeding a stick's reach on the diagonal.
  - **remap a range** — map an input range onto 0..1, the sense Unity's `Normalize` processor uses.

  Naming one of them `Normalize` invites the other. Whichever names are chosen, neither may be the
  bare word. Note the second rescales and so is bound by D6's one-rescaling-stage rule (R5.3), while
  the first does not.
- **R5.3 (MUST)** Deadzone semantics must be explicit about rescaling — whether output is remapped to
  0..1 after the inner radius is removed (almost always desired; frequently gotten wrong). Per D6
  (§14) at most one stage in the deadzone stack may rescale, so a deadzone modifier must be able to
  state that it does not.
- **R5.4 (MUST)** Stateful modifiers (smoothing, accumulation, rate limiting) must declare their state
  and make it serializable and resettable, or be forbidden from the deterministic path (§10).
- **R5.5 (MUST)** Time-dependent modifiers must receive `dt` explicitly and behave identically under
  variable frame rate for the same simulated time.
- **R5.6 (MUST)** Third-party modifiers must be registerable without forking the crate, and must
  round-trip through serialization via the type registry.
- **R5.7 (MUST)** Modifiers must be pure functions of (input, state, dt) with no world access, so
  they can run during rollback resimulation. _(A `MUST` rather than a `SHOULD` because §10.R10.2
  makes purity of the whole mapping step a `MUST` and a modifier runs inside that step: anything
  weaker here would admit an impure modifier that breaks a `MUST` there. The same reasoning applies
  to conditions via R6.6.)_
- **R5.8 (MUST)** Modifiers are a **developer-facing** mechanism and must never be surfaced directly in
  a player-facing UI. Negate, swizzle, and curve are adapters for fitting a source to an action, not
  choices a player can meaningfully make. Where a modifier parameter should be player-adjustable, it is
  exposed as a named tunable (§19.R19.11) that drives it.

---

## 6. Conditions / triggers

**Problem.** "When does this action fire" is a small language of its own.

**Prior art.** Unreal [triggers][unreal-ei] with Explicit/Implicit/Blocker semantics: `Down`, `Pressed`, `Released`,
`Hold`, `HoldAndRelease`, `Tap`, `Pulse`, `Chorded Action`, `Combo` (5.3+). Unity [interactions][unity-interactions]:
`Press`, `Hold`, `Tap`, `SlowTap`, `MultiTap`. BEI adds `BlockedBy`. Fighting-game engines add motion
inputs (quarter-circle) and input buffering, which none of the above handle natively.

- **R6.1 (MUST)** Built-in conditions: down/held, press edge, release edge, hold-for-duration,
  hold-and-release, tap (press+release within threshold), multi-tap/double-tap, pulse/repeat at
  interval, chord (requires another action or control), blocked-by (another action is active).
- **R6.2 (MUST)** Composition semantics defined when an action has multiple conditions: which are
  _explicit_ (any one satisfies), _implicit_ (all must hold), _blocking_ (any one vetoes). Unreal's
  three-way split is the clearest formulation found; adopt it or document a deliberate alternative.
- **R6.3 (MUST)** All duration/interval thresholds are configurable per binding and expressed in
  simulated seconds (§9.R9.6).
- **R6.4 (SHOULD)** Sequence/combo conditions (ordered inputs within a time window) — needed for
  double-tap-dash, motion inputs, and cheat codes; if deferred, the condition trait must be able to
  express them without a breaking change.
- **R6.5 (SHOULD)** Forgiveness windows, in both directions. _Buffering_ accepts an input pressed
  slightly **before** it became valid and fires it when it does (pressing jump just before landing,
  queuing the next attack mid-swing). _Coyote time_ is the mirror image: accepting an input slightly
  **after** it stopped being valid (jumping a few frames after walking off a ledge). Both are
  configurable windows, and the lack of them is a frequent reason teams abandon a general input crate
  and hand-roll one.
- **R6.6 (MUST)** Third-party conditions registerable, same constraints as R5.6/R5.7.
- **R6.7 (MUST)** Conditions must not depend on real time or on frame count (§10).

---

## 7. Contexts, layers, and activation lifecycle

**Problem.** Bindings change with game state (on-foot, vehicle, menu, dialogue, photo mode), and the
transition itself has semantics.

**Prior art.** Unreal `InputMappingContext` added to a subsystem with an integer priority; higher
priority consumes keys from lower. BEI: contexts are components with `ContextPriority`.
[Steam][steam-controller]: Action Sets plus _Action Set Layers_ (additive overlays rather than
replacements) — the layer concept is underused elsewhere and is exactly right for "while aiming" or
"while holding an item". Unity: action maps enabled/disabled individually, with no built-in priority
— a known pain point.

- **R7.1 (MUST)** Multiple contexts active simultaneously, ordered by an explicit, inspectable
  priority. That order is total **within a tick domain** and not across one: a context evaluated at
  the render rate reads before one evaluated at the simulation rate, whatever their priorities say,
  because the frame puts those schedules in that order.

  _(Stated as a limit rather than a goal, because the alternative is worse. Making priority total
  across domains would mean one evaluation pass for every context, which costs the per-domain
  evaluation §9 requires. The restriction only bites in the direction nobody wants: what claims
  controls is UI, UI runs at the render rate, and what it claims from is simulation.)_
- **R7.2 (MUST)** Contexts must be scoped per player/entity, not only global (§15).
- **R7.3 (MUST)** Additive layers: a layer that overrides a subset of bindings without redefining the
  whole context.
- **R7.4 (MUST)** Deactivating a context must resolve in-flight actions deterministically: ongoing
  actions must be _canceled_ (fire the cancel transition), never left stuck as "held forever".
- **R7.5 (MUST)** Activating a context must not fire actions for controls already physically held
  (require-reset), unless explicitly opted in — this is the "pressing E to close a menu instantly
  re-triggers Interact" bug class.
- **R7.6 (SHOULD)** Context activation must be cheap enough to do per-frame (no rebuild of the whole
  binding graph); if a rebuild is needed it must be incremental and change-detection driven (§23).
- **R7.7 (SHOULD)** A declarative way to express "these contexts are mutually exclusive" (a stack) as
  well as free-form sets, since most games want a stack and reimplement it every time.

---

## 8. Conflict resolution and consumption

**Problem.** `S` and `Ctrl+S` are both bound; a key is bound in two active contexts; two players share
a keyboard. Who wins?

**Prior art.** LWIM [`ClashStrategy::{PressAll, PrioritizeLongest}`][lwim-clash] — the longest-chord-wins rule is the
right default and is often missing elsewhere. Unreal: higher-priority context consumes the key,
lower ones never see it. Godot: [`set_input_as_handled`][godot-input] at dispatch level. Unity: no arbitration for
PassThrough actions, first-match for others.

- **R8.1 (MUST)** Define and document a default clash strategy across bindings on the same control;
  longest/most-specific chord wins is the recommended default (`Ctrl+S` suppresses `S`).
- **R8.2 (MUST)** Higher-priority contexts must be able to _consume_ a control so lower-priority
  contexts do not see it, and this must be opt-in per binding (a menu consumes `Escape`, but the
  "screenshot" global hotkey should still see `F12`).
- **R8.3 (MUST)** Consumption must be resolvable in one deterministic pass with no ordering ambiguity
  between systems.
- **R8.4 (MUST)** Interop with focus/UI: per D4 (§22), a focused widget claims controls by activating
  a context, and normal context priority does the rest — there is no separate suppression mechanism.
  A focused text field must be able to claim character-producing keys as a class, without the app
  author enumerating them — see R4.9 for the mechanism.
- **R8.5 (SHOULD)** A diagnostic that answers "why did action X not fire" by naming the consumer /
  clash / inactive context (§22).
- **R8.6 (SHOULD)** Conflicts must be _detectable statically_ for rebinding UI (§19), i.e. the same
  arbitration logic must be queryable offline against a hypothetical binding.

---

## 9. Scheduling, sampling, and time

**Problem.** This is where most input libraries break. Bevy's `ButtonInput` edge flags are per-`Update`
frame; a `FixedUpdate` running zero or three times per frame either misses or duplicates edges.
Simultaneously, camera look wants render-rate mouse deltas while gameplay wants sim-rate input.

**Prior art.** Unity [`InputSettings.updateMode`][unity-settings] (`ProcessEventsInDynamicUpdate` /
`InFixedUpdate` / `Manual`) is the only mainstream system that makes this a first-class choice. LWIM
carries a separate fixed-update `ActionState`. In Bevy this is a known, still-open defect:
[bevy#6183, "Inputs can be missed (or duplicated) when using a fixed time step"][bevy-6183], with the
same problem for events in [bevy#7691][bevy-7691].

- **R9.1 (MUST)** Define one canonical sampling point (in `PreUpdate`, ordered after
  `bevy_input::InputSystems`) that produces the L1 input frame.
- **R9.2 (MUST)** Render-rate and fixed-rate action state must both be available, and reading the
  wrong one must not be an easy mistake. _(Stated as a guarantee rather than a layout. The design
  gives each context one state and a declared tick domain (Design §7), so an action needed at both
  rates is declared in two contexts. What matters is that the rates stay distinct and are not
  silently interchangeable, which that satisfies by a route a
  original wording forbids. The requirement is about the guarantee, not the layout.)_
- **R9.3 (MUST)** Edges must not be lost when `FixedUpdate` runs zero times in a frame — a press and
  release inside one frame must still be observable by fixed-rate consumers.
- **R9.4 (MUST)** Edges must not be duplicated when `FixedUpdate` runs multiple times — a single press
  must produce exactly one press edge across all fixed ticks.
- **R9.5 (MUST)** Continuous deltas (mouse motion, wheel) must be attributed to fixed ticks with a
  documented policy that conserves total magnitude (no double-count, no loss) when N ticks run.
- **R9.6 (MUST)** All condition/state timing must be driven by a caller-selected clock. Hold durations
  in a pause menu must use real time; gameplay holds must use virtual/fixed time and must respect
  pause and time scaling.
- **R9.7 (MUST)** Multiple raw events for one control within a sampling window must be handled with a
  documented policy (coalesce vs. preserve ordering); order-sensitive conditions (tap, sequence) must
  not be corrupted by coalescing.
- **R9.8 (SHOULD)** Preserve per-event timestamps from the windowing layer where available, for
  sub-frame accuracy on high-polling-rate devices.
- **R9.9 (SHOULD)** A manual/pumped mode where the app decides when sampling occurs — needed for
  headless runs, tests, and lockstep networking (where every peer must consume an identical input set
  for a given tick, so sampling cannot be tied to local frame timing).

---

## 10. Determinism, replay, and netcode

**Problem.** Rollback netcode works by simulating forward using a prediction of what remote players
did, and then — when their real input arrives late — rewinding to the tick it applies to and
re-simulating everything since. That imposes two demands most input systems cannot meet. Input must
be reducible to a small value that can be sent over a wire and stored per tick; and the action state
derived from it must be **re-derivable**, meaning the mapping step has to be a pure function whose
every piece of internal memory (hold timers, tap counters, smoothing filters) can be saved and
restored along with the rest of the simulation.

Even a game with no networking gets the same properties for free under a different name: a replay is
a stored input stream, and a deterministic test is a replay with assertions.

- **R10.1 (MUST)** The L1 input frame must be a compact, `Serialize`/`Deserialize`, version-tolerant
  value suitable for transmission and for storing a replay.
- **R10.2 (MUST)** Mapping must be a pure function of (bindings, active contexts, previous action
  state, input frame, dt) — no wall-clock reads, no RNG, no frame counters, no global mutable state.
- **R10.3 (MUST)** All internal mapping state (hold timers, tap counters, smoothing filters, chord
  progress) must be snapshot-able and restorable, so rollback can rewind it.
- **R10.4 (MUST)** Injection: the app must be able to feed a synthetic input frame (from network, AI,
  replay, or test) in place of live device input, at L1, per player.
- **R10.5 (SHOULD)** Injection at L2 as well (force an action to a value/state) — needed for tutorials,
  cutscenes, and remote players whose actions are replicated rather than their raw input.
- **R10.6 (SHOULD)** A quantization hook so float values entering the simulation can be reduced to a
  fixed representation, avoiding cross-platform float divergence and shrinking the wire format.
- **R10.7 (SHOULD)** Document precisely which parts of the pipeline are guaranteed deterministic across
  platforms and which are not (trig in response curves, `f32` ordering in accumulation).
- **R10.8 (MUST)** Record/replay of raw input frames must reproduce identical action output given
  identical bindings — and must be testable in CI headlessly (§21).

---

## 11. Device abstraction and enumeration

**Problem.** Bevy models gamepads as entities via `gilrs`; keyboards/mice are global; everything else
(MIDI, HOTAS flight controllers, racing wheels, gyro, eye tracking, on-screen touch controls) is
unmodeled.

- **R11.1 (MUST)** A uniform device model covering keyboard, mouse, gamepad, touch, and
  application-defined virtual devices, each with a runtime handle and an inspectable class.
- **R11.2 (MUST)** Third-party crates must be able to register new device kinds and controls without
  forking — including their control identifiers, so bindings and rebinding UI work for them.
- **R11.3 (MUST)** Capability queries: available controls, analog vs digital, rumble, motion/gyro,
  touchpad, battery, LED — used by prompts (§18) and by "can this player play at all" checks.
- **R11.4 (MUST)** Hot-plug: connect/disconnect events, and a documented policy for the state of
  actions held on a device that disappears (must release, must cancel — never stick).
- **R11.5 (MUST)** Stable persistent device identity where the platform allows (vendor/product/serial
  or SDL GUID), distinct from the ephemeral runtime handle, so per-device settings and player
  assignments survive a reconnect or a restart.
- **R11.6 (MUST)** Device _class_ and _brand_ resolution (Xbox / PlayStation / Nintendo / generic)
  with an app-overridable mapping seeded from a database such as [SDL_GameControllerDB][sdl-db],
  since glyph choice depends on it and `vendor_id`/`product_id` are `Option` and often absent
  (notably on wasm and some Linux setups).
- **R11.7 (MUST)** Per-device calibration (stage 1 of D6: center offset and rest envelope) stored
  separately from bindings and keyed by persistent device identity (R11.5). Note this _supersedes_
  rather than interoperates with Bevy's `GamepadSettings` deadzone, per R14.9 — the two must not both
  be active.
- **R11.8 (SHOULD)** Virtual devices: on-screen touch sticks, AI/bot drivers, and test fixtures must be
  first-class devices, not special cases.
- **R11.9 (MAY)** Surface unhandled/unknown controls as opaque IDs rather than dropping them, so
  exotic hardware is at least bindable.

---

## 12. Keyboard specifics

**Problem.** Physical vs. logical keys, layouts, and text entry are all sharp edges.

- **R12.1 (MUST)** Bindings may target physical position (`KeyCode`) or logical character (`Key`), and
  the choice must be explicit — WASD must bind physically (so AZERTY gets ZQSD), while `Ctrl+Z` should
  bind logically.
- **R12.2 (MUST)** Display strings must show the _logical_ key for the user's current layout even when
  the binding is physical (§18) — showing "W" to an AZERTY user is a bug.
- **R12.3 (MUST)** Modifier handling: left/right variants plus "either" as a first-class concept; and
  a modifier participating in a chord must be able to suppress the unmodified binding (§8.R8.1).
- **R12.4 (MUST)** Platform-conventional modifier abstraction (`Cmd` on macOS ≡ `Ctrl` elsewhere) as a
  named modifier, resolved at binding time.
- **R12.5 (MUST)** OS key-repeat events (`KeyboardInput::repeat`) must be distinguishable and excluded
  by default from press-edge conditions, while remaining available for text/navigation repeat.
- **R12.6 (MUST)** Text entry: a focused text field must be able to claim character-producing keys as
  a class (R4.9) via D4's focus-activated context (§22), rather than through a bespoke suppression
  mode. Must
  cover the cases where a keypress is not one character: **IME composition**, where Chinese, Japanese,
  and Korean input builds a character over several keystrokes that must not reach gameplay bindings;
  **dead keys**, where a key such as `´` produces nothing until the following key decides what it
  becomes; and the resulting multi-character `text` field that Bevy reports when a Windows dead key
  cannot combine with what follows ([`KeyboardInput::text`][bevy-keyboard-src]).
- **R12.7 (SHOULD)** Keyboard layout changes at runtime must invalidate cached display strings (§18).

---

## 13. Pointer, mouse, and touch specifics

**Problem.** "Pointer input" is three unrelated signals sharing one name: an absolute position, a
relative motion delta, and a set of buttons. They have different units, different frame-rate
behavior, and different correct handling, and touch adds a fourth case — several simultaneous
pointers that appear and vanish. Most bugs in this area come from treating one of them as another.

- **R13.0 (MUST)** Mouse **buttons** are bindable controls in their own right, on the same terms as
  keyboard keys: a whole binding, a part of a composite, a member of a chord, and something capture
  will take for a mappable slot. They belong to the keyboard-and-mouse scheme (§17.R17.4), so a
  mouse button may be captured for a mapping a key currently holds and the two never conflict with a
  gamepad binding. This is the third of the three signals the problem statement above separates, and
  the only one that behaves like an ordinary button.

  The set is whatever the platform reports: left, right and middle, the two thumb buttons, and
  indexed buttons beyond them. The thumb buttons are stored under the names the backend gives them
  and _shown_ as Mouse 4 and Mouse 5, which is what a player's other games call them (R18.3).
- **R13.1 (MUST)** Distinguish pointer _position_ (absolute, window-relative, UI-scale-aware) from
  pointer _motion_ (relative delta), and never let a binding accidentally use one for the other.
- **R13.2 (MUST)** Mouse motion for camera look must be frame-rate independent and must not be
  multiplied by `dt` (a common bug: deltas are already per-frame quantities, unlike stick positions).
  The pipeline must let a binding declare which kind it is — this is the same distinction as the action
  intent of §2.R2.7, and R2.9 governs what happens when a delta-kind and a rate-kind binding drive one
  action.
- **R13.3 (MUST)** Scroll: handle `MouseScrollUnit::{Line, Pixel}` and normalize them with an
  app-configurable lines→pixels factor; high-resolution trackpad scroll must not be quantized away.
  The wheel is a delta on its own channel rather than a button, so it is a separate matter from
  R13.0 and is not satisfied by it.
- **R13.4 (MUST)** Cursor grab / relative mouse mode interaction: entering grab must not produce a
  spurious huge delta; leaving it must restore position; document the interaction with
  `bevy_window::CursorOptions`.
- **R13.5 (MUST)** Multiple windows: input frames must carry the source window and bindings must be
  filterable by it.
- **R13.6 (MUST)** Multi-touch: multiple simultaneous pointers with stable IDs; touch must not be
  silently emulated as mouse unless the app opts in.
- **R13.7 (SHOULD)** OS gestures (`PinchGesture`, `RotationGesture`, `PanGesture`, `DoubleTapGesture`)
  bindable as sources.
- **R13.8 (SHOULD)** Drag semantics that a mapping layer can express: press threshold, click-vs-drag
  disambiguation, double-click interval sourced from OS settings where available.
- **R13.9 (SHOULD)** Pointer capture during drag so a drag continues when the pointer leaves the
  window; this must coordinate with `bevy_picking` rather than compete with it (§22).

---

## 14. Gamepad specifics

**Problem.** Gamepads are the least uniform input device in common use. The same physical control
reports differently across brands and drivers, analog and digital views of the same trigger both need
to exist, and a stick's resting value is a property of the individual unit rather than the model.
Much of this section is about not discarding information before the game has decided what it needs.

- **R14.1 (MUST)** Bind to "any gamepad" (class), "the gamepad owned by this player" (§15), or "this
  specific device" (instance).
- **R14.2 (MUST)** Analog triggers exposed as axes _and_ as buttons with a configurable threshold and
  hysteresis (separate press/release thresholds), to avoid chatter at the boundary. Note the direction
  this runs in practice: the trigger reaches L1 on a **button** channel already carrying a fractional
  value (R2.10), so the *axis* view is the one we synthesize, and the button view must be derived from
  our own threshold rather than inherited from any press/release the backend synthesized.
- **R14.3 (MUST)** D-pad usable both as four buttons and as a 2D axis, interchangeably with a stick.
  At L1 the D-pad is **always** four buttons and never an axis pair (see Bevy's behavior below), so the
  2D view is likewise synthesized — by the same composite that turns four keyboard keys into a `Vec2`,
  not by separate hat-handling machinery.
- **R14.4 (MUST)** _(D6)_ Stick deadzone shape configurable (radial vs. per-axis) per binding, with
  rescaling (§5.R5.3), sourced from raw values per R14.9 and layered per the model below.

### The deadzone stack (D6)

**Problem.** A deadzone looks like a single number, but three parties have a legitimate claim on it,
and their claims do not reconcile into one value:

- the **player**, who has preferences about feel and may need accommodation;
- the **game developer**, who knows what the mechanic requires;
- the **hardware**, which has no opinions but does have characteristics — stick drift and rest
  envelope vary per manufactured unit and worsen with wear, outside anyone else's control.

The resolution is not to arbitrate one number between them. It is to recognize that they are answering
**three different questions**, which belong at three different stages:

| Stage              | Question                                                                         | Scope              | Owner                          |
| ------------------ | -------------------------------------------------------------------------------- | ------------------ | ------------------------------ |
| **1. Calibration** | Where is this physical stick's true center, and how much does it jitter at rest? | per device _unit_  | measured, with player override |
| **2. Design**      | What deadzone shape and response curve does this mechanic want?                  | per binding/action | game developer                 |
| **3. Preference**  | Scale the above for comfort, accessibility, or a worn thumbstick.                | per player         | player (§20.R20.5)             |

Stage 1 varies by individual unit, not just by model — drift is a wear characteristic — so it must be
_measured_, capturing a center **offset** as well as a radius, not assumed symmetric about zero.
Stage 2 is where radial-vs-axial and curves live (§5). Stage 3 modulates, and must not be able to
reduce stage 1 below what the hardware actually needs.

**The rule that makes them compose: at most one stage may rescale.** If a lower stage removes a
radius and remaps the remainder to full range, an upper stage's threshold no longer corresponds to any
physical stick position, and the two are no longer reasoning about the same quantity.

**The failure this prevents is information loss, and it is one-directional.** A stage that clamps to
zero destroys signal that no later stage can recover: if something below us zeroes everything inside
±0.05, a game wanting ±0.01 simply cannot have it. Deadzones must therefore be applied as late as
possible, which is what forces R14.9.

**Bevy's current behavior**, which this must be reconciled against. Line references are pinned to
[`bevy_input/src/gamepad.rs`][bevy-gamepad-src] at the commit this document was written against
(`17e28cd`, 0.20-dev):

- `GamepadSettings`/`AxisSettings` default to a **±0.05 per-axis** (not radial) deadzone with linear
  rescaling to 0..1 ([`impl Default for AxisSettings`][bevy-axissettings-default]) — an axial deadzone
  on a stick is the classic [square-corner artifact][deadzone-article].
- `GamepadAxisChangedEvent` carries the **scaled/deadzoned** value while `Gamepad::analog` stores the
  **unmodified raw** value, three lines apart in the same match arm
  ([gamepad.rs L1622-L1624][bevy-raw-vs-scaled]) — so polling and events disagree about what an axis
  reads.
- Both paths are gated by `AxisSettings::threshold` (default 0.01) evaluated on the _deadzoned_ value
  ([`filter`][bevy-axis-filter]), so sub-threshold motion updates neither, and the stored "raw" value
  freezes on entering the deadzone.
- [`RawGamepadEvent`][bevy-rawgamepadevent] / `RawGamepadAxisChangedEvent` are emitted before any of
  this, and converted by [`gamepad_event_processing_system`][bevy-gamepad-processing].
- Below Bevy, `bevy_gilrs` builds gilrs with [`.with_default_filters(false)`][bevy-gilrs-lib] and then
  **re-applies exactly one** of gilrs's three default filters,
  [`axis_dpad_to_button`][bevy-gilrs-system]. Two consequences, both verified on hardware (below):
  gilrs's own default deadzone — **radial 0.1 with rescaling**, substituted whenever a platform
  reports no deadzone hint, which macOS does for every axis ([`DEFAULT_DEADZONE`][gilrs-deadzone]) —
  never runs, so `RawGamepadEvent` is genuinely raw of it; and a hat-switch D-pad is converted to four
  synthetic buttons *before* `RawGamepadEvent` is emitted, so an axis-pair D-pad never reaches us.
  (`bevy_gilrs` also discards `gilrs::Axis::DPadX`/`DPadY` outright — [converter.rs][bevy-gilrs-conv]
  — and `GamepadAxis` has no D-pad variants, so there is no path by which one could.)

**Measured, not assumed.** The above was checked against an Xbox Series controller (VID `0x045e`, PID
`0x0b13`) over Bluetooth LE on macOS, reading gilrs directly. Findings worth recording because they
are easy to guess wrong: sticks report full ±1.0 range with no deadzone applied anywhere below Bevy;
the D-pad reports as a hat and reaches us as four buttons; analog triggers report on HID page 2
(Simulation Controls, usages 196/197) as *buttons with fractional values*, alongside separate digital
bumpers (R2.10). Two negative results are also worth keeping: the same controller over **USB** is
claimed by Apple's `com.apple.gamecontroller.driver.XboxGamepad` DriverKit dext, after which gilrs
enumerates it but receives no values at all; and a Switch-protocol clone advertises a HID descriptor
whose declared layout does not match the report it actually sends, so gilrs decodes its timer byte as
buttons and emits ~500 phantom presses per second with no stick data. Neither is fixable at our layer,
and both argue for §11's device-capability model and for R21.x mocking over hardware-dependent tests.

The honest caveat: "raw" is only raw relative to Bevy. XInput applies its own deadzone below the
driver — the documented constants are 7849/32767 ≈ 0.24 for the left thumb and 8689/32767 ≈ 0.27 for
the right ([`XINPUT_GAMEPAD`][xinput-gamepad]) — and an authority backend (D3) applies its own in its
binding config; neither is removable. This is a further argument for stage 1 being measurement-based
rather than assuming a centered zero.

- **R14.9 (MUST)** _(D6)_ Consume `RawGamepadEvent`/`RawGamepadAxisChangedEvent` at L1, not the
  filtered values or `Gamepad::analog`. Document that apps using this crate should leave
  `GamepadSettings` at pass-through, and detect and warn when they have not, since the result is a
  silent double deadzone that neither party can see.
- **R14.10 (MUST)** _(D6, D3)_ When an authority backend supplies action values, stages 1–3 are the
  backend's and must not be applied again on our side (§0.R0.4).
- **R14.11 (SHOULD)** Stage 1 calibration must be persistable per device identity (§11.R11.5) and
  offerable as an explicit player-facing calibration step, since auto-detection of a worn stick's
  resting envelope needs samples the game may not otherwise collect.
- **R14.5 (SHOULD)** Motion/gyro as a bindable 3D source where the platform exposes it (
  [gilrs][gilrs], Bevy's gamepad backend, does not
  expose it — which argues for R11.2 extensibility rather than a built-in).
- **R14.6 (SHOULD)** Rumble/haptic _routing_: given a player, address the right device
  (`GamepadRumbleRequest` needs the entity). Authoring effects is out of scope; addressing is not.
- **R14.7 (SHOULD)** Battery / wireless status surfaced for low-battery UI where available.
- **R14.8 (MAY)** Touchpad and adaptive-trigger support behind the extensibility API.

---

## 15. Local multiplayer and device pairing _(first-class)_

**Problem.** Device→player assignment is a relation, not a field; nearly every system that hardcodes
"gamepad index = player index" has to be rewritten later.

**Prior art.** Unity `InputUser` + `PlayerInput` + [`PlayerInputManager`][unity-playerinput] with join-on-button-press and
control-scheme-based device requirements. Steam Input's per-controller handles. Unreal's local player
subsystem. LWIM associates a `GamepadDevice` per `InputMap`. None handle "two players on one keyboard"
gracefully.

- **R15.1 (MUST)** Device→player assignment is many-to-many: one player may own keyboard+mouse+pad;
  one device may be shared by several players (split keyboard); devices may be unassigned.
- **R15.2 (MUST)** Per-player action state and per-player context stacks, queryable by player without
  filtering global state.
- **R15.3 (MUST)** A device's input must not reach a player who does not own it — this must be enforced
  at L1/L2, not left to per-action filtering.
- **R15.4 (MUST)** Join flow support: observe input from _unassigned_ devices (with bindings applied,
  so "press Start to join" works per device class) and assign on demand.
- **R15.5 (MUST)** Leave / disconnect: on device loss, the owning player must be identifiable, in-flight
  actions canceled (§7.R7.4), and a signal raised so the app can pause and show a reconnect prompt
  (handling this is a common console certification requirement, though the specific requirements
  documents are under NDA and cannot be cited here).
- **R15.6 (MUST)** Reconnect must be able to restore the previous assignment via persistent identity
  (§11.R11.5).
- **R15.7 (SHOULD)** Control schemes: named device-requirement sets (KBM, Gamepad) with required and
  optional devices, used for auto-assignment and prompt selection (§18).
- **R15.8 (SHOULD)** Auto-switching of a player's active scheme on input, with hysteresis and a
  noise/deadzone floor so a drifting stick does not flip prompts mid-sentence; and an opt-out.
- **R15.9 (SHOULD)** Attach opaque platform-user identity (PSN/Xbox/Steam account handle) to a player
  without the crate depending on any platform SDK.
- **R15.10 (MAY)** Split-screen: associate a player with a camera/viewport for pointer coordinate
  mapping.

---

## 16. Window, OS, and platform integration

**Problem.** The OS can take input away without saying so in terms the game understands. Alt-tab, a
lock screen, a mobile suspend, a browser tab losing focus — each leaves the process believing keys
are still held, and each produces the same class of bug. The fix is one policy applied at every such
boundary, not a special case per platform.

- **R16.1 (MUST)** On window focus loss (`KeyboardFocusLost`), all held controls must be released and
  ongoing actions canceled — the alt-tab stuck-key bug must be impossible.
- **R16.2 (MUST)** On regaining focus, controls physically held must not produce press edges
  (require-reset, per §7.R7.5).
- **R16.3 (MUST)** Suspend/resume (mobile, console) treated the same as focus loss, with device
  re-enumeration on resume.
- **R16.4 (SHOULD)** Web: document that [pointer lock][mdn-pointerlock] and [gamepad][mdn-gamepad]
  access require a user gesture, that
  gamepad events are polled, and that key codes/`vendor_id` are less reliable; the API must degrade
  rather than panic.
- **R16.5 (SHOULD)** Do not silently swallow OS-reserved combinations; document which are unavailable.
- **R16.6 (SHOULD)** `no_std` compatibility for the core mapping layer, with device backends behind
  features — an upstream-Bevy expectation.

---

## 17. Persistence and schema evolution

**Problem.** Saved bindings outlive the build that wrote them. Between the save and the load, actions
will be added and removed, controls will be renamed, devices will be gone, and shipped default
bindings will have been revised — after players have already customized theirs. A format that assumes
any of that is stable loses player data silently on the next patch.

- **R17.1 (MUST)** User binding overrides serialize as a _diff against defaults_, not a full snapshot,
  so that shipping new default bindings does not require migrating every save.
- **R17.2 (MUST)** Loading must tolerate unknown actions, unknown controls, and removed devices
  without failing the whole load; unresolved entries must be reported, not dropped silently.
- **R17.3 (MUST)** A version field with a documented migration path.
- **R17.4 (SHOULD)** Multiple named profiles per user, and separate override sets per control scheme
  (a KBM remap must not disturb the gamepad layout).
- **R17.5 (SHOULD)** Serialization must go through `Reflect` + the type registry so third-party
  modifiers/conditions (§5.R5.6) round-trip.
- **R17.6 (MAY)** Bindings as a hot-reloadable asset, for iteration without recompiling.
- **R17.7 (MUST)** A saved override set distinguishes three states per mapping, and a format with
  only two loses one of them:

  | | Means | Produced by |
  | --- | --- | --- |
  | **absent** | use whatever the game shipped | a mapping the player never touched |
  | **cleared** | the player deliberately removed the binding | R19.3's unbind-the-other policy, or an explicit "clear" |
  | **not ours** | an external backend owns this action (§0.R0.4, R19.8) | Steam Input and equivalents |

  The distinction is easy to miss because a diff against defaults (R17.1) makes absence meaningful:
  once "missing" already says "default", clearing a binding has nothing left to say with. The third
  state matters for the same reason — a backend-owned action must not read as one the player
  cleared, and saving must not invent rows for actions we do not own.
- **R17.8 (MUST)** Binding overrides must not carry device identity. What a player bound is a
  control on a device *class*; which physical unit drives which player is pairing state (§15.R15.6),
  and how a particular stick rests is calibration state (§11.R11.7). Three stores, keyed
  differently, and conflating them breaks the case they exist for: two players with identical
  controllers and identical mappings differ only in pairing, and must not need two copies of one
  binding table to say so.
- **R17.9 (MUST)** The serialized form of a control is a stable format this crate owns and
  round-trip tests, not the `Debug` or `serde` representation of an upstream type. A control name is
  a serialized key with D8's stability obligation, and deriving it from `KeyCode`'s variant names
  would put that obligation somewhere we do not control — an upstream rename would silently orphan
  every saved binding. The format must also carry what the binding layer already distinguishes:
  physical versus logical keys (§12.R12.1) and device class, at minimum.

---

## 18. Presentation: prompts, display strings, and glyphs

**Problem.** "Press [X] to open" needs a reverse lookup that is correct for the current device, the
current context, the current layout, and the current locale — and must update live.

**Prior art.** Steam Input is the gold standard. In its vocabulary an _origin_ is the physical
control currently bound to an action on this player's actual controller — the answer to "what should
the prompt show". The flow is [`GetDigitalActionOrigins`][steam-isteaminput] →
`GetGlyphSVGForActionOrigin` / `GetStringForActionOrigin`, with the binding owned entirely outside the
game. Unity `ToDisplayString` + `InputBinding.MaskByGroup`. Unreal's `PlayerMappableKeySettings`.

- **R18.1 (MUST)** Reverse lookup: given an action (and optionally a context and device class), return
  the bindings that would currently fire it, in a stable, ranked order.
- **R18.2 (MUST)** The result must reflect active contexts and consumption (§8) — showing a prompt for
  an action that a higher-priority context is currently consuming is wrong.
- **R18.3 (MUST)** Display strings must be produced without hard-coding English: return a structured
  descriptor (control identity + composite structure, e.g. "hold", "chord of A and B") that a
  localization layer renders, with a reasonable built-in fallback renderer.
- **R18.4 (MUST)** Glyph resolution returns an _identifier_ keyed by (device brand, control), not an
  asset handle; the app supplies the atlas. A fallback chain (brand → generic → text) is required.
- **R18.5 (MUST)** Live invalidation: prompts must update when bindings change, the active context
  changes, the player's active device changes, or the keyboard layout changes. Change detection or
  events must make this cheap — polling every prompt every frame is not acceptable.
- **R18.6 (MUST)** Track a per-player "most recently used device/scheme" for prompt selection, subject
  to §15.R15.8 hysteresis.
- **R18.7 (SHOULD)** Support a confirm/cancel button-convention policy as one setting rather than
  scattered `if cfg!` checks. The case that forces this: on PlayStation in Japan, ○ (East) has
  historically meant confirm and ✕ (South) cancel, while the rest of the world uses the opposite —
  so the same semantic action maps to a different physical button by region, and both the binding and
  the prompt must follow. Handling it per call site guarantees somewhere gets missed.
- **R18.8 (MUST)** _(D3)_ An external binding backend may be the source of truth for origins and
  glyphs; presentation must not assume our own binding tables are authoritative. Reverse lookup
  (R18.1) is therefore a trait method with our binding table as one implementation, not a concrete
  query over our own data.
- **R18.9 (MUST)** Backend-supplied glyphs arrive as something other than our own (brand, control)
  identifiers: an opaque handle, raw image bytes, or — the case Steam actually presents — a
  filesystem path to a PNG or SVG that the app must load itself. R18.4's identifier scheme must
  therefore be one variant of a wider glyph-source type, not the only shape. The same is true one
  level up: a backend's *origins* are its own enumeration of physical controls, covering device
  families we have no `Control` for, so reverse lookup must be able to answer with something that is
  not one of ours (R18.8).
- **R18.10 (SHOULD)** When a backend is authoritative, its origins may change without any input from
  us (the user edits bindings in the Steam overlay mid-session). R18.5's invalidation must be
  driveable by the backend, not only by our own binding mutations.

---

## 19. Rebinding

**Problem.** Interactive rebinding is a small state machine whose failure modes only show up in
practice. The UI has to capture input without acting on it, stay operable with its own controls
possibly being rebound, and evaluate conflicts using the same rules the runtime will later apply —
otherwise it will cheerfully accept a binding that can never fire.

Underneath that is a harder problem: **the internal binding model is not a model a player can be shown.**
Negate, swizzle, and response curves are adapters the developer uses to fit a source to an action; a
player rebinding "move forward" must never see them. The requirements below exist to make a *simple*
rebinding UI possible without making the internal model simple.

**Prior art.** What shipped games actually expose is far narrower than a general binding model would
suggest, and the narrowness is not a limitation players complain about:

- **Digital actions** (jump, reload, interact) are freely rebindable within a scheme. This is the
  overwhelming majority of any real rebinding screen.
- **Movement is decomposed, never bound as a composite.** The screen shows four rows — forward, back,
  left, right — each taking one key. Unity models this directly: a composite binding has named
  _parts_ (`up`/`down`/`left`/`right`), and interactive rebinding targets an individual part via
  [`WithTargetBinding()`][unity-bindings], filtered by `WithExpectedControlType()`, with
  `WithControlsExcluding()` and `WithCancelingThrough()` keeping the UI operable.
- **Sticks are remapped by preset, not by binding.** Unreal's Player Mappable Input Config presents
  named arrangements such as "Default" and "Southpaw" rather than free-form binding of a 2D axis.
- **Look and aim are not rebindable at all.** What players get instead is a small set of typed
  parameters: sensitivity, invert-Y, a response-curve preset, deadzone size.
- **Gamepad remapping is increasingly solved below the game** — PlayStation and Xbox both provide
  system-level controller remapping, and [Steam Input][steam-controller] replaces the game's binding
  UI outright. In-game rebinding is therefore primarily a keyboard-and-mouse concern.

Steam draws the same developer/player line this section adopts: the game declares an action's category
and intent, and the binding UI owns which physical control drives it, along with deadzones and response
curves ([IGA file][steam-iga]).

- **R19.1 (MUST)** Interactive capture ("press a key now") that reports the control that would be
  bound and can be canceled. Capture must be filtered by the target mapping's intent and shape
  (§2.R2.7), so a mapping expecting a button only accepts buttons.

  **Capture reads L1 directly; it is not a binding.** What it reports is a control _identity_, which
  a binding would have discarded on the way to producing a value — recovering it afterwards would
  mean promoting R2.6's source tag to a `MUST` purely to undo a loss capture caused. Reading the
  input frame also makes R19.5 structural: an evaluator that never runs cannot fire a gameplay
  action. The filter is expressed in R4.9's class vocabulary, so capture, exclusion lists (R19.2),
  and reserved controls share one way of naming a set of controls rather than three.
- **R19.2 (MUST)** Exclusion lists during capture (do not capture `Escape`, mouse position, or the UI's
  own navigation controls) so the rebinding UI remains operable.
- **R19.3 (MUST)** Conflict detection against the same arbitration rules used at runtime (§8.R8.6),
  with the app choosing the policy: reject, swap, duplicate-allowed, or unbind-the-other. Conflicts are
  scoped **per control scheme** — a keyboard binding cannot conflict with a gamepad binding, because
  the two are never active as alternatives for the same player at the same moment.
- **R19.4 (MUST)** Reset to default per binding, per action, per context, and globally.
- **R19.5 (MUST)** Rebinding must not require the game to be running its normal contexts, and must not
  fire gameplay actions while capturing.
- **R19.6 (SHOULD)** Rebinding must respect R4.7 (non-rebindable bindings) and expose a name per
  _slot_ and a category per _action_ (R1.6, R19.9) for the UI to label and group by. Both are
  localization keys (R19.14).
- **R19.7 (SHOULD)** Rebind per control scheme independently (§17.R17.4).
- **R19.8 (MUST)** _(D3)_ When a backend is authoritative for an action, rebinding must delegate to
  that backend's own UI (Steam's [`ShowBindingPanel`][steam-isteaminput]) rather than presenting our capture flow. The
  rebinding API must be able to report "not rebindable here, delegate instead" as a normal outcome —
  and R19.3's conflict detection does not apply to those actions, since we do not own the rules.

### The player-facing model

- **R19.9 (MUST)** The unit of rebinding is a **mapping**, not a binding. For a composite, each
  _part_ is its own mapping — "move forward" is a mapping, `Move` is not — so the composite is never
  exposed to the player. A mapping carries a **name key** (R19.14), the intent and shape it accepts
  (R19.1), and the control scheme it belongs to. Its category comes from the action it belongs to
  (R1.6).

  A mapping holds an **ordered list of slots**, each holding one control, with a **capacity** saying
  how many slots it has. "Primary and secondary" is the commercial arrangement — a keyboard row with
  two cells — and a model holding one control per mapping cannot express it at all: the workaround
  is a second row under an alias name, which tells the player two things are separate when they are
  the same. Order is what makes the first slot primary, so it is part of the data rather than an
  artefact of iteration. A screen draws one cell per slot; "cell" is the drawing and never the data.

  Capacity is **inferred from the declared defaults and raisable by the author**, never inferred
  downward: a mapping holding two defaults has room for two without anyone saying so, and an author
  who ships one default and wants a spare slot says so once. An unbounded capacity exists for the
  other kind of program — a tool whose command set is too large and open to lay out in a table — and
  is not what a game reaches for.
- **R19.10 (MUST)** A binding is **listed by default and rebindable only when declared**. Three
  states, and every binding is in exactly one:

  - **Listed and fixed**, which is what saying nothing gets: the player reads it on a controls screen
    and cannot change it. This is the whole of the gamepad story on a console, and most of it on
    Steam.
  - **Listed and rebindable**, which is a declared mapping. Rebindability is the presence or absence
    of a mapping rather than a flag on the binding, which is R4.7.
  - **Unlisted**, which must be asked for. Reserved for a binding that is another binding's
    implementation detail — a second reading of a control that already appears under a different
    name — rather than a control the player operates.

  The two questions must not be conflated. An earlier framing had listing follow rebindability, so
  declining to offer a rebind also hid the binding, and the commonest gamepad screen in the industry
  — a read-only list of what the pad does, with the remapping owned by the platform — could not be
  drawn from our own data at all. Rebindability is the developer's call because a fixed binding is a
  design decision; being able to see the controls is the player's business, and the default belongs
  to them.
- **R19.11 (MUST)** Player-adjustable parameters are exposed as **named tunables**, not as modifier
  chains. A tunable is a declared, typed, named, range-bounded parameter on a binding — `sensitivity:
  f32 in 0.1..=10.0`, `invert_y: bool`, `deadzone: f32 in 0.0..=0.5`, `hold_or_toggle: enum`,
  `curve: enum of presets` — that a generic UI can render as a slider, checkbox, or dropdown without
  knowing what it drives. **Modifiers (§5) must never be surfaced directly to players**; a tunable that
  happens to drive a modifier parameter is the supported path, and it is what satisfies R20.5.
- **R19.12 (SHOULD)** **Presets**: named alternative arrangements of mappings and tunables
  ("Default", "Southpaw", "Lefty") that a player selects as a unit. For device classes where
  per-mapping rebinding is not offered — sticks especially — a preset is the entire remapping story,
  and it is also how a game ships a sensible starting point per control scheme.
- **R19.13 (SHOULD)** A game that offers no rebinding UI at all must still work: mappings,
  tunables, and presets are additive declarations, never a precondition for binding an action.
- **R19.14 (MUST)** Every player-visible name this crate carries — mapping names (R19.9), action
  categories (R1.6), tunable and preset names (R19.11, R19.12) — is a **localization key, not
  display text**. Rendering it is the app's business, exactly as R18.3 already requires for the
  control half of a rebinding row.

  Without this the rebinding screen is half-localized: R18.3 makes "Space" and "A button"
  translatable while the "Move Forward" beside them is a literal baked into the binding declaration.
  A localized game would have to shadow every one of those strings with a lookup of its own, which
  is the table the crate was supposed to provide.

  Consequences, since they constrain the design rather than describe it:

  - **A key inherits D8's stability problem.** It appears in files outside the code — a translation
    catalogue rather than a save — and renaming one silently drops the game back to fallback text
    with no compile error. Keys need the same deliberate, convention-governed treatment as action
    paths (R1.8), and the convention should cover both.
  - **A key SHOULD be derivable rather than declared twice.** A mapping's natural key is its
    action's path plus its part name — `gameplay.move` plus `forward` — and both already exist and
    are already stable. Note this does _not_ reopen D8: what D8 rejected was deriving identity from
    the Rust module path, which tracks code structure. Deriving from an author-declared path does
    not, because the thing being derived from is itself stable by declaration. An explicit override
    must remain available.
  - **A game with no localization layer must still read sensibly** (R19.13). A fallback renderer
    turning a key into presentable text is required, so that shipping a translation catalogue is
    never the price of seeing a readable rebinding screen.

- **R19.15 (MUST)** Mapping keys must be unique within a control scheme, and a collision must be
  reported when the context is declared rather than discovered by a player, since in a saved file a
  rebind of one mapping silently lands on the other. R19.14's explicit override is the remedy; this
  is what makes an author reach for it.

  Three cases derive one key twice, and only one of them is a collision:

  - **Two mappable bindings of one action, in one scheme, in one context** are a default primary and
    secondary. They are one mapping holding two controls (R19.9), not two mappings, and must merge
    silently — this is the ordinary way to ship "W or Up Arrow", and refusing it forces the alias
    row R19.9 exists to remove.
  - **Two fixed rows deriving one key** are not a collision either. Uniqueness exists to stop a saved
    override landing on the wrong row, and a row nobody can rebind is never written to a save (§17).
    This is what makes listing by default affordable: under R19.10 one action bound in two contexts
    produces two listed rows under one name, and demanding a distinct key for each would tax every
    game that never offers a rebind at all. The collision returns the moment either side becomes
    rebindable, which is the only moment it can do harm.
  - **Two different actions answering to one name**, and **the same action mappable in two
    contexts**, are collisions when either side is rebindable. The second is a collision even though
    the action is the same, because the two are separate rows in contexts that may be live at
    different times, while the override store is keyed by mapping alone (§17).

---

## 20. Accessibility

These are `SHOULD`s rather than `MUST`s: none is a goal of the crate in its own right, but each is
cheap to accommodate in the architecture now and expensive to retrofit. Accessibility is also
increasingly a certification requirement — see the [Xbox Accessibility Guidelines][xags] and the
[Game Accessibility Guidelines][gag], and in the US the CVAA (the 21st Century Communications and
Video Accessibility Act, which reaches game communication features).

- **R20.1 (SHOULD)** Every gameplay action must be remappable in principle — the architecture must not
  make any binding permanently hardcoded.
- **R20.2 (SHOULD)** Hold-vs-toggle must be expressible as a binding-level option, not reimplemented
  per action by the game.
- **R20.3 (SHOULD)** No action should _require_ simultaneous inputs that cannot be re-expressed as a
  sequence; chord conditions must therefore have a sequential alternative.
- **R20.4 (SHOULD)** All timing thresholds (hold duration, double-tap window, repeat rate) must be
  globally scalable by a user preference.
- **R20.5 (SHOULD)** Sensitivity and deadzone must be user-adjustable per device without editing
  bindings — via named tunables (§19.R19.11), which is the mechanism that makes this possible without
  exposing the modifier chain.
- **R20.6 (MAY)** Sticky-modifier / one-handed support at the mapping layer.

---

## 21. Testing, mocking, and tooling

**Problem.** Input is normally untestable: it comes from hardware, depends on wall-clock timing, and
arrives through a windowing backend that does not exist in CI. Everything here follows from R0.2 —
the mapping layer reads only the L1 input frame — which is what makes a synthesized frame
indistinguishable from a real one.

- **R21.1 (MUST)** Drive the whole pipeline from a synthesized input frame in a headless `App` with no
  windowing backend — this is what makes the crate testable at all, and follows from R0.2.
- **R21.2 (MUST)** Time must be injectable so hold/tap conditions can be tested without sleeping.
- **R21.3 (MUST)** Mock a device (including hot-plug and disconnect) in tests.
- **R21.4 (SHOULD)** Record and replay raw input streams as a supported feature, not a test-only hack
  (§10.R10.8) — also useful for bug reports and automated soak tests.
- **R21.5 (SHOULD)** Ship an example per major area (rebinding UI, local multiplayer join, fixed-update
  gameplay, prompts) — for an upstream Bevy contribution these are effectively required.

---

## 22. Debugging, diagnostics, and ecosystem interop

**Problem.** When an action does not fire, the cause is invisible. It could be an inactive context, a
higher-priority consumer, a longer chord winning the clash, a condition that never completed, or a
device this player does not own — and all five look identical from the call site. This section also
covers the boundary with the rest of the Bevy input ecosystem, which is where ownership questions
surface.

- **R22.1 (SHOULD)** A "why didn't this fire" query returning the failing link: context inactive,
  consumed by X, clashed with longer chord Y, condition Z at 40% progress, device not owned by player.
- **R22.2 (SHOULD)** An inspector-friendly dump of active contexts, bindings, and action states — the
  same data must be sufficient to drive a live debug overlay, not only a one-shot dump.
- **R22.3 (SHOULD)** `tracing` spans/events at the sampling and firing boundaries.
- **R22.4 (MUST)** Documented ordering and integration with `bevy_input::InputSystems`,
  `bevy_input_focus` (`InputFocus`, `FocusedInput`), and `bevy_picking` — including how pointer
  actions coexist with picking.
- **R22.5 (SHOULD)** Interop with `bevy_input_focus::{tab_navigation, directional_navigation}`: UI
  navigation (including analog-stick navigation with initial delay + repeat rate) should be expressible
  as actions in this system rather than as a parallel input path.
- **R22.6 (SHOULD)** A documented migration path from LWIM and bevy_enhanced_input, since the ecosystem
  will ask.

### Focus integration (D4)

Rather than UI suppressing gameplay wholesale, focus participates in the action system itself, via two
mechanisms:

1. **Dispatch is an action effect.** Certain actions, when they fire, emit a bubbling `FocusedInput`
   event at the current focus entity instead of (or as well as) exposing a value. Dispatch stays
   `bevy_input_focus`'s job; we only trigger it.
2. **Focus type drives context activation.** Contexts are activated by what kind of widget currently
   has focus — a focused slider activates a context mapping `Increment`/`Decrement` plus the
   directional-navigation actions. The focused widget thereby intercepts the inputs it cares about,
   while anything it does not claim falls through to global shortcuts.

This is a better shape than blanket suppression: interception becomes declarative and inspectable
(R22.1 can say _which_ focus-activated context consumed a control), and text-entry suppression
(§12.R12.6) stops being a special case — a focused text field simply activates a context that claims
character keys.

- **R22.7 (MUST)** An action's effect must be expressible as "dispatch as a bubbling event at the
  current focus entity", not only as "produce a value". This is a distinct axis from the value model
  in §2 and the state machine in §3, both of which describe only what an action *reports*, not what
  firing it *does*.
- **R22.8 (MUST)** Context activation must be bindable to the _kind_ of the focused entity, re-evaluated
  when focus changes. Must handle: nothing focused, a focus entity matching several such contexts, and
  a focus entity despawned while focused.
- **R22.9 (MUST)** **Neither crate may depend on the other.** A widget library must not gain a
  dependency on this crate, and this crate must not require a widget library — using widgets without
  input mapping, and input mapping without widgets, are both first-class. This rules out any
  mechanism that requires widgets to implement our traits, carry our components, or name our
  contexts.

  Consequences worth stating, since they constrain the design rather than describe it:

  - the association must be expressible by a **third party** — the application, or a bridging crate —
    which is the only place allowed to know about both sides;
  - if instead the widget side is to carry anything, it can only be a **neutral identifier it would
    plausibly have anyway** (a stable well-known id, a static string slice) that means "this is a
    slider", never "this activates that context";
  - any such identifier that appears in serialized binding data inherits R1.5's stability problem: it
    must survive renames and refactors, or saved bindings silently stop matching.

  The mechanism itself — component-type registration by the app, a well-known widget id, reflection
  over type paths, or something else — is deferred to the design phase. What is fixed here is the
  dependency direction, because it is the part that cannot be renegotiated later without breaking
  users.
- **R22.10 (SHOULD)** Focus integration as a whole should sit behind a feature flag (§24.R24.1), so
  that a game using this crate with no UI at all pays nothing for it — the same "vice versa" that
  R22.9 requires of the widget side.
- **R22.11 (MUST)** Focus changes must resolve _before_ the same frame's actions are evaluated, or a
  one-frame window exists in which the previously focused widget's map is still live.
- **R22.12 (MUST)** Focus changing mid-action must cancel that action's in-flight state per §7.R7.4 —
  holding `Increment` on a slider and then tab-navigating away must not leave a hold running.
- **R22.13 (MUST)** _(D5)_ Interception is **static**: a focus-activated context claims a control
  before dispatch, and §8's ordinary priority decides the winner in one deterministic pass. A widget
  does not decide at handling time whether to let an input fall through, so no two-phase
  dispatch/collect/resolve ordering is needed and R8.3 is preserved.

  The case that appears to demand dynamic interception is covered without it: `Ctrl+Z` meaning
  undo-in-field when a text input has focus and
  undo-in-document otherwise is _already_ static. The text field's focus-activated context claims
  `Ctrl+Z`; when focus is elsewhere that context is inactive and the global binding wins. Nothing
  about it requires the widget to elect anything at runtime — the election is expressed by which
  context is active, which is a consequence of what has focus.

  What static-only interception gives up is narrower than it first appears: only a widget that claims
  a control _conditionally on its own
  internal state_ — a text field that swallows `Ctrl+Z` only while its undo stack is non-empty. The
  workaround is to make that state part of context activation (activate a
  `TextFieldWithUndoHistory` context) rather than a runtime decision, which keeps the claim
  inspectable by R22.1.

### Declarative scene formats (BSN)

Bevy Scene Notation ships in `bevy_scene`: the [`bsn!`][bevy-scene-docs] macro authors an entity's
components, children, and — the feature that matters here — its **observers**, inline via
[`on()`][bevy-scene-on], which accepts any `EventPattern<Event: EntityEvent>` and attaches an observer
to the scene entity. Attaching handlers declaratively next to components is among BSN's most-used
capabilities.

This is a live integration target, not a forward-looking hedge, and it constrains this design in two
specific ways. Both are cheap to honor now and expensive to retrofit, because both are about *where
events are targeted* and *what it takes to activate a context* — neither is a compatible change later.

The requirements are stated as properties rather than as BSN specifics, so any declarative format,
editor, or serialized template gets the same benefit.

- **R22.14 (MUST)** Spawning must be sufficient. Adding a context to an entity — as a component, from
  a scene, from a template, with no imperative registration call — must fully activate it, including
  resolving or compiling its bindings on demand. A context that only works after a setup function has
  run cannot be expressed in a scene at all.
- **R22.15 (MUST)** Action transitions must be deliverable as `EntityEvent`s targeted at an entity a
  scene author already has — the entity carrying the context — so a handler can be attached inline
  with `on(...)`. This is why R3.2's delivery mechanism is constrained rather than left open: a global
  event stream, a resource-level callback, or an event targeted at an internal entity cannot be
  attached declaratively.
- **R22.16 (MAY)** Binding sets, mappings (§19.R19.9), and tunables (§19.R19.11) may themselves
  be authorable as scene or asset data, letting a game ship alternative control schemes without
  recompiling. Lower priority than R22.14 and R22.15, and it must remain optional — code-defined
  bindings stay the primary path (§17.R17.6).
- **R22.17 (SHOULD)** None of the above may become a dependency. The crate must build and function
  with `bevy_scene` absent or unused, and must not require its macros or types in the public API —
  the same both-directions independence R22.9 requires of widget libraries. Satisfying R22.15 with a
  plain `EntityEvent` achieves this for free: BSN can consume it, and nothing about it refers to BSN.

---

## 23. Performance and ECS storage

**Problem.** The costs that matter are not the arithmetic of mapping a few dozen bindings per frame.
They are structural: what happens when a context is activated, how much state a rollback tick must
copy, and whether a prompt can subscribe to one action without waking on all of them. These are the
properties that constrain the state layout left open in OQ-3.

- **R23.1 (MUST)** Per-frame cost proportional to _active_ bindings, not to all defined actions ×
  entities.
- **R23.2 (MUST)** No allocation **and no synchronization** in the steady-state hot path, and a way
  to **detect a violation** rather than only a rule against one.
  _(Synchronization and not only allocation: a lock on a per-frame path is worse than an allocation,
  and a rule naming only allocation does not catch it. Detection because a rule nobody can check is
  a rule that gets broken by accident — a helper returning a collection is the ordinary way to write
  it, and the ordinary way is the one that ends up in a loop that runs every tick.)_
- **R23.3 (MUST)** Context activation/deactivation must not cause structural ECS churn proportional to
  the number of actions — activating a context should not spawn, despawn, insert, or remove per
  action. A layout that does so must show the cost is acceptable at the action counts in §23.R23.1.
- **R23.4 (SHOULD)** Change detection on action state, so that UI which reacts to bindings or
  action values (§18 prompts especially) can subscribe rather than poll; unchanged actions must not
  mark themselves changed every frame. Note that if state is one component, Bevy's change ticks are
  all-or-nothing across it — per-action granularity must then be built in (a dirty set or
  per-mapping change tick), or every prompt wakes whenever any action moves. This is an evaluation
  criterion for OQ-3, not a settled cost.
- **R23.5 (MUST)** Action state must be snapshot-able and restorable cheaply enough to run per
  rollback tick (§10.R10.3), and reachable from an `ActionId` in O(1) without a hash lookup on the hot
  path. _How_ — see OQ-3.
- **R23.6 (SHOULD)** A context instance may live as a component on an entity or standalone; the
  storage model must be identical in both cases, so per-player, global, and test-harness contexts
  share one code path (§0.R0.3).
- **R23.7 (MUST)** The same action may be present in two simultaneously-active layers (§7.R7.3) and
  must be able to hold **independent in-flight state in each** — a half-completed hold in the base
  context must not be clobbered by the overriding layer's copy. Any storage keyed globally by
  `ActionId` alone fails this; state must be keyed by (context instance, action).

---

## 24. API design and upstream constraints

**Problem.** Targeting eventual upstream inclusion in Bevy imposes constraints a standalone crate
would not face: dependency scrutiny, `no_std`, reflection, and conformance to conventions that move
between releases. The tension to manage is that generality of the kind this document demands tends to
produce APIs in which the simplest case stops being simple.

- **R24.1 (MUST)** Core is `no_std`-compatible; device backends, serialization, and reflection behind
  feature flags mirroring Bevy's conventions (`keyboard`, `mouse`, `gamepad`, `touch`, `serialize`,
  `bevy_reflect`).
- **R24.2 (MUST)** Minimal dependency surface (upstream review will scrutinize every new dep).
- **R24.3 (MUST)** All public data types `Reflect` where Bevy's conventions require it.
- **R24.4 (MUST)** Fallible operations return Bevy-style results/errors, not panics; misconfiguration is
  a first-class error case with actionable messages (§4.R4.8).

  Two failure kinds live here and must not be conflated. **Runtime**
  failures — a device gone, an unresolved binding, an action read that finds nothing — must return
  errors, never panic, because they befall a player rather than a developer. **App-build** failures
  — a context declared twice, bindings that cannot compile, a chain that violates a documented
  invariant — may panic, and generally should: they are unreachable in a shipped build, they are
  Bevy's own convention for plugin setup, and an error returned from a builder chain tends to be
  dropped. The obligation that applies to both is the actionable message.
- **R24.5 (SHOULD)** Follow current Bevy event conventions: buffered `Message` + `MessageReader` for
  streams, `Event`/observers for per-entity notification. (Bevy 0.20-dev has completed this split; the
  crate must not be written against the old `EventReader` model.)
- **R24.6 (MUST)** The common case must be short — binding WASD to a movement action should be a few
  lines. Comprehensiveness (this document) must not produce a system that requires 40 lines for the
  trivial case; an ergonomic façade over the general model is a requirement, not a nicety. _(Raised
  from `SHOULD`: this is the enforceable half of [Who this is for](#who-this-is-for), and a `SHOULD`
  makes the constituency it protects optional.)_
- **R24.7 (MUST)** Every mechanism that exists for a funded studio must be **additive**: absent until
  declared, and with defaulted behaviour that is correct in its absence. Nothing in §15, §17, §18,
  §19, or R19.14 may become a step a game must perform before an action fires. A new requirement
  that fails this test is a finding, not a feature.
- **R24.8 (MUST)** Defaults must be correct on hardware, layouts, and locales the author cannot test
  — a second gamepad, a controller that misreports itself (§14), AZERTY, a right-to-left locale.
  Where correctness cannot be defaulted, the mistake must be caught by a diagnostic (§4.R4.8) rather
  than left to a QA pass the author does not have.

---

## Resolved decisions

| #      | Decision                                                                                                                                                                   | Sections affected |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------- |
| **D1** | Actions are declared as **types**, with a derive macro specifying shape and metadata.                                                                                      | §1, §2, §3        |
| **D3** | **External binding backends are supported** (Steam Input and equivalents may be the source of truth).                                                                      | §0, §4, §18, §19  |
| **D4** | **Focus integration is by action, not by suppression**: dispatch-to-focus is an action effect, and focus _type_ drives context activation.                                 | §8, §22           |
| **D5** | **Interception is static only.** A focus-activated context claims a control before dispatch; a widget never decides at handling time whether to let an input fall through. | §8, §22           |
| **D6** | **We own the whole deadzone chain**, consuming Bevy's _raw_ gamepad events, and model it as three separate stages rather than one negotiated number.                       | §5, §11, §14      |
| **D7** | **The player-facing model is separate from the binding model**: players see opt-in _mappings_, _named tunables_, and _presets_; modifiers and composites stay developer-only. | §2, §4, §5, §19, §20 |
| **D8** | **Serialized identity is a declared path, not the Rust type path**, and the derive requires it — a saved binding must not depend on where a type lives. Namespacing moves to the naming convention (R1.8). | §1, §17, §18 |

There is deliberately no decision here about how action state is stored, or about whether actions are
entities. That is a design question for the next phase, not a requirement; §23 states the properties
any layout must satisfy and OQ-3 maps the options.

---

## Top design decisions to resolve next

These are the forks where the choice cascades; everything else is comparatively local.

1. ~~**OQ-1 — Crate identity/name.**~~ _Resolved: renamed `bevy_input_router` → `bevy_action_map`._
2. ~~**OQ-2 — Action identity representation**~~ _Resolved as **D1**: types + derive macro, with an
   interned `ActionId` as the runtime representation and the reflected type path as the serialized
   identity. Sub-question also resolved: dynamic declaration is out of scope for v1 (§1), with the
   mapping-action pattern as the answer for modding._
3. **OQ-3 — State layout** (§23), **open**. The real axis is _when the layout of a context's state
   record is decided_:

   |                              | Layout decided                | Sketch                                                       |
   | ---------------------------- | ----------------------------- | ------------------------------------------------------------ |
   | **(a)** action-as-entity     | never — ECS archetypes        | BEI's model; one entity per action                           |
   | **(b)** uniform dense table  | never — one slot size for all | `Vec<ActionState>` indexed by `ActionId`                     |
   | **(c)** variadic typed tuple | compile time, by rustc        | context `(A, B, C)` → state `(A::State, B::State, C::State)` |
   | **(d)** compiled plan        | context-build time, by us     | computed offsets into a packed byte buffer                   |

   (a) gets change detection, reflection, and inspector support for free from the ECS; it is the
   option under most pressure from R23.3 (activation churn) and R10.3 (snapshot cost), so those two are
   where it has to be measured rather than argued about. Note that (a) was previously credited with a
   third advantage — being the only layout that gives a per-action observer something to attach to —
   and that turned out to be false: a *generic* entity event (`Fired<Jump>`) carries the action
   identity in its type parameter and targets the context entity, so every layout supports per-action
   observers equally. That removes the one criterion on which (a) was uniquely strong.

   (c) and (d) are closer than they look: (d) is (c) with the layout computed at runtime instead of by
   the compiler. (c) buys exact-size state, zero lookup, and compile-time-checked reads, at the cost
   of enormous type signatures that are painful to read and debug — plus monomorphization bloat,
   tuple arity limits, and no way to admit a dynamic action (R1.3) into a typed context. (d) keeps
   packed exact-size state and O(1) offset access while admitting dynamic actions, and pairs naturally
   with the compiled binding plan that §7.R7.6 and §23.R23.1 want anyway; it costs a build step and
   careful typed-accessor code over a byte buffer.

   Evaluation criteria, all already required elsewhere: rollback snapshot cost (R10.3), no activation
   churn (R23.3), per-action change granularity (R23.4), independent per-layer state (R23.7), dynamic
   actions coexisting with typed ones (R1.3), external backends writing state from outside (R0.4),
   generic enumeration for debug UI and serialization (R22.2, R17.5), error-message quality (R24.6),
   and `no_std` + no steady-state allocation (R23.2, R24.1).

4. ~~**OQ-4 — Deadzone ownership**~~ _Resolved as **D6** (§14): consume raw, own all three stages —
   calibration (per unit), design (per binding), preference (per player) — with at most one rescaling
   stage. Sub-question also resolved: stage 1 ships with a **manual calibration API plus a sampling
   helper the app drives during an explicit calibration step**, not background auto-detection —
   detection running while a stick is deflected would learn that position as centre, and hardware
   that misreports (see the README) would poison it silently. Stage 2 is the stage that rescales
   (Design §8.1)._
5. **OQ-5 — Modifier/condition extensibility mechanism** (§5.R5.6/§6.R6.6): trait objects vs. reflected
   registry — trades ergonomics against serializability and determinism.
6. ~~**OQ-6 — Fixed/render dual state**~~ _Resolved as **tick domains** (Design §7): neither of the
   two options offered. A context declares its domain and is evaluated exactly once, in that domain,
   so there is one state per context rather than two per context or one with per-tick accounting.
   The cost is that an action wanted at both rates is declared in two contexts. R9.2 has been
   reworded accordingly. Remaining sub-question: enforcement is not
   airtight, because Bevy gives a `SystemParam` no way to know its own schedule (Design §12)._
7. ~~**OQ-7 — Where UI suppression lives**~~ _Resolved as **D4** (§22): neither — there is no
   suppression mechanism. Dispatch-to-focus becomes an action effect, and focus type activates
   contexts, so interception falls out of ordinary context priority. Sub-question also resolved as
   **D5**: interception is static only, preserving §8's single deterministic pass (R22.13)._
8. ~~**OQ-8 — External binding backends**~~ _Resolved as **D3**: supported. Note this turned out to be
   larger than the original R18.8 framing — it is an L2 authority seam (§0.R0.4), not only a
   presentation concern. New open sub-question: whether an authority backend's actions participate in
   §10 determinism at all, since Steam's action state is not reproducible from our input frames and so
   cannot be resimulated during rollback._
9. ~~**OQ-9 — Where the player-facing name and category live**~~ _Resolved: the **mapping owns
   the name, the action owns the category**. A composite settles the first — `Move` has four
   mappings and the player must be shown "Move Forward", never "Move" — and repetition settles the
   second, since four movement mappings share one category and hanging it on each is four chances to
   disagree._

   _Resolving it turned up a larger point, now **R19.14**: neither field is display text. Both are
   **localization keys**, because R18.3 already requires the control half of a rebinding row to be
   localizable and leaving the action half as a baked literal would half-localize one screen. This
   also relieves R1.6 — the derive is back to five fields, three of them required, which is no
   longer the "configuration language" Design §12 warned about._

---

## References

Prior art and primary sources for the claims made above. Bevy source links are pinned to commit
`17e28cd` (0.20-dev), the tree this document was checked against; the line numbers will drift on
`main`.

**Prior art — input mapping systems**

- [leafwing-input-manager][lwim] (LWIM) — enum actions, [clash strategies][lwim-clash], axis
  processing pipeline.
- [bevy_enhanced_input][bei] (BEI) — Unreal-style contexts, conditions, and modifiers for Bevy;
  [API docs][bei-docs].
- [Unreal Enhanced Input][unreal-ei] — input actions, mapping contexts with priority, triggers
  (Explicit/Implicit/Blocker), modifiers.
- Unity Input System — [bindings and composites][unity-bindings], [interactions][unity-interactions],
  [processors][unity-processors], [settings and update modes][unity-settings],
  [PlayerInputManager][unity-playerinput].
- [Steam Input][steam-controller] — action sets and layers; [ISteamInput API][steam-isteaminput] for
  action data, origins, and glyphs; [IGA file format][steam-iga].
- [Godot InputEvent][godot-input] — event propagation and `set_input_as_handled`;
  [input examples][godot-examples].

**Bevy internals**

- [`bevy_input/src/gamepad.rs`][bevy-gamepad-src] — [`AxisSettings` defaults][bevy-axissettings-default],
  [`filter` / threshold][bevy-axis-filter], [raw-vs-scaled divergence][bevy-raw-vs-scaled],
  [`RawGamepadEvent`][bevy-rawgamepadevent], [`gamepad_event_processing_system`][bevy-gamepad-processing].
- [`bevy_input/src/keyboard.rs`][bevy-keyboard-src] — `KeyCode` vs `Key`, `repeat`, and the
  multi-character `text` field.
- [`bevy_input_focus`][bevy-input-focus] — `InputFocus`, `FocusedInput`, tab and directional navigation.
- [bevy#6183][bevy-6183] — inputs missed or duplicated under a fixed timestep (open).
- [bevy#7691][bevy-7691] — the same problem for events.

**Hardware and platform**

- [`XINPUT_GAMEPAD`][xinput-gamepad] — documented thumbstick deadzone constants.
- [gilrs][gilrs] — Bevy's gamepad backend; bounds what device data is available.
- [SDL_GameControllerDB][sdl-db] — community mapping database for controller identification.
- [MDN Pointer Lock API][mdn-pointerlock], [MDN Gamepad API][mdn-gamepad] — web platform constraints.

**Technique**

- ["Doing Thumbstick Dead Zones Right"][deadzone-article] — radial vs. axial deadzones and rescaling.

**Accessibility**

- [Xbox Accessibility Guidelines][xags]
- [Game Accessibility Guidelines][gag]

[lwim]: https://github.com/Leafwing-Studios/leafwing-input-manager
[lwim-clash]: https://docs.rs/leafwing-input-manager/latest/leafwing_input_manager/clashing_inputs/enum.ClashStrategy.html
[bei]: https://github.com/projectharmonia/bevy_enhanced_input
[bei-docs]: https://docs.rs/bevy_enhanced_input/latest/bevy_enhanced_input/
[unreal-ei]: https://dev.epicgames.com/documentation/en-us/unreal-engine/enhanced-input-in-unreal-engine
[unity-bindings]: https://docs.unity3d.com/Packages/com.unity.inputsystem@1.14/manual/ActionBindings.html
[unity-interactions]: https://docs.unity3d.com/Packages/com.unity.inputsystem@1.14/manual/Interactions.html
[unity-processors]: https://docs.unity3d.com/Packages/com.unity.inputsystem@1.14/manual/UsingProcessors.html
[unity-settings]: https://docs.unity3d.com/Packages/com.unity.inputsystem@1.14/manual/Settings.html
[unity-playerinput]: https://docs.unity3d.com/Packages/com.unity.inputsystem@1.14/manual/PlayerInputManager.html
[steam-controller]: https://partner.steamgames.com/doc/features/steam_controller
[steam-isteaminput]: https://partner.steamgames.com/doc/api/ISteamInput
[steam-iga]: https://partner.steamgames.com/doc/features/steam_controller/iga_file
[godot-input]: https://docs.godotengine.org/en/stable/tutorials/inputs/inputevent.html
[godot-examples]: https://docs.godotengine.org/en/stable/tutorials/inputs/input_examples.html
[bevy-gamepad-src]: https://github.com/bevyengine/bevy/blob/17e28cdedca8f66cd01ba88bd40ec33591e6bf37/crates/bevy_input/src/gamepad.rs
[bevy-axissettings-default]: https://github.com/bevyengine/bevy/blob/17e28cdedca8f66cd01ba88bd40ec33591e6bf37/crates/bevy_input/src/gamepad.rs#L1004-L1014
[bevy-axis-filter]: https://github.com/bevyengine/bevy/blob/17e28cdedca8f66cd01ba88bd40ec33591e6bf37/crates/bevy_input/src/gamepad.rs#L1294-L1307
[bevy-raw-vs-scaled]: https://github.com/bevyengine/bevy/blob/17e28cdedca8f66cd01ba88bd40ec33591e6bf37/crates/bevy_input/src/gamepad.rs#L1622-L1624
[bevy-rawgamepadevent]: https://github.com/bevyengine/bevy/blob/17e28cdedca8f66cd01ba88bd40ec33591e6bf37/crates/bevy_input/src/gamepad.rs#L73
[bevy-gamepad-processing]: https://github.com/bevyengine/bevy/blob/17e28cdedca8f66cd01ba88bd40ec33591e6bf37/crates/bevy_input/src/gamepad.rs#L1588
[bevy-keyboard-src]: https://github.com/bevyengine/bevy/blob/17e28cdedca8f66cd01ba88bd40ec33591e6bf37/crates/bevy_input/src/keyboard.rs#L111-L141
[bevy-input-focus]: https://docs.rs/bevy_input_focus/latest/bevy_input_focus/
[bevy-6183]: https://github.com/bevyengine/bevy/issues/6183
[bevy-7691]: https://github.com/bevyengine/bevy/issues/7691
[xinput-gamepad]: https://learn.microsoft.com/en-us/windows/win32/api/xinput/ns-xinput-xinput_gamepad
[gilrs]: https://docs.rs/gilrs/latest/gilrs/
[bevy-gilrs-lib]: https://github.com/bevyengine/bevy/blob/17e28cdedca8f66cd01ba88bd40ec33591e6bf37/crates/bevy_gilrs/src/lib.rs#L92-L94
[bevy-gilrs-system]: https://github.com/bevyengine/bevy/blob/17e28cdedca8f66cd01ba88bd40ec33591e6bf37/crates/bevy_gilrs/src/gilrs_system.rs#L48
[bevy-gilrs-conv]: https://github.com/bevyengine/bevy/blob/17e28cdedca8f66cd01ba88bd40ec33591e6bf37/crates/bevy_gilrs/src/converter.rs#L37-L40
[gilrs-deadzone]: https://docs.rs/gilrs/0.11.2/src/gilrs/gamepad.rs.html
[sdl-db]: https://github.com/mdqinc/SDL_GameControllerDB
[mdn-pointerlock]: https://developer.mozilla.org/en-US/docs/Web/API/Pointer_Lock_API
[mdn-gamepad]: https://developer.mozilla.org/en-US/docs/Web/API/Gamepad_API
[deadzone-article]: http://www.third-helix.com/2013/04/12/doing-thumbstick-dead-zones-right.html
[xags]: https://learn.microsoft.com/en-us/gaming/accessibility/guidelines
[gag]: https://gameaccessibilityguidelines.com/
[bevy-scene-docs]: https://docs.rs/bevy/latest/bevy/scene/index.html
[bevy-scene-on]: https://github.com/bevyengine/bevy/blob/17e28cdedca8f66cd01ba88bd40ec33591e6bf37/crates/bevy_scene/src/scene.rs#L568-L577
