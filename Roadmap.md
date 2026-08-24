# Incremental build plan: `bevy_action_map`

> This document orders the work described in [Design.md](./Design.md) into chunks small enough to
> review individually. It is a _sequence_, not a schedule — there are no dates, and any chunk may be
> revised or reordered once the one before it has been read.

## Ground rules

1. **One chunk, one reviewable change.** Each chunk below is meant to be a single branch: code, its
   tests, and any doc changes it forces. If a chunk turns out to be more than roughly a day's reading,
   it gets split before it gets written.
2. **Every chunk is verifiable on its own.** Pure-data chunks get unit tests; chunks that touch ECS
   get either a headless `App` test or a runnable example. No chunk lands whose only justification is
   "the next one needs it".
3. **The examples are the acceptance test.** From chunk 4 onward there is always something to run.
   When a later chunk is an internal change, the criterion is that _the examples do not change_ —
   a diff in `examples/` during a refactor chunk is a signal the abstraction leaked.
4. **Deliberate omissions are stated.** Each chunk lists what it does _not_ do, so review can tell
   "not yet" from "overlooked".
5. **Nothing outstanding is left without a destination.** A chunk that lands short of its own
   description says so in the [work log](./Log.md), and the obligation is written onto the chunk
   that finishes the job — so what a chunk owes is stated where someone will read it, rather than
   having to be gathered from the chunks that incurred it. "Its own decision" and "later" are not
   destinations: an item with no chunk number is an item that will be dropped.

---

## Commit Messages

To be compatible with Bevy's AI policy (not just the policy but the discussions that preceded it):

Commit messages should not say "Co-authored by" an LLM. Rather, it should
have a section "LLM Usage Disclosure", which briefly explains the role the
LLM played in crafting the commit.

Effectively all of the code here is LLM-authored to a human's direction, so the disclosure is a
standing fact rather than a per-commit judgement. Unless a commit was unusual, one line does it:

```
LLM Usage Disclosure: implementation, tests and documentation written by
Claude Opus 5; design decisions, review and acceptance by the author.
```

Say more only where a commit departs from that — where the model chose something the author would
otherwise have decided, or where the author wrote the code and the model reviewed it.

---

## House style

This crate is a candidate for eventual upstream inclusion, and game developers as a class are
sensitive to text that reads as machine-authored — a contribution that reads that way invites
controversy independently of whether the code is correct. So the standard is: **the code should read
as though a maintainer of the surrounding codebase wrote it.**

**Comments.** Internal comments are terse. Don't explain what a maintainer already knows — ECS
semantics, borrow rules, standard Bevy behavior. Comment the non-obvious decision: the thing that
would break if someone changed it. One or two lines, not a block explaining the mechanism.

**Doc comments are the exception.** Verbosity is welcome on `pub` items. This is a library whose
public documentation is part of the deliverable (R24.6), so doc comments can be full and
explanatory in a way internal comments must not be.

**Analysis belongs in the review conversation, not the source file.** The reasoning that produced a
design — why an alternative was rejected, what the tradeoff was — goes in the chunk's discussion.
When it genuinely needs to persist, it goes in [Design.md](./Design.md) or the requirements, both of
which are built for it. What it must not do is accumulate as prose in the code.

**Avoid the tells:** restating what the code says, hedging, enumerating the obvious, unusual
punctuation or phrasing in comments.

**Prefer sketches over applied refactors.** Small, staged, individually reviewable edits — which is
the same principle as Ground rule 1, applied within a chunk rather than across chunks.

**Comments should be addressed to the right audience**. _Doc comments_ are meant for public-facing documentation, targeted at game developers who want to use the
crate. These needs to explain basic concepts, usage examples, and in some cases,
educate the user as to why a function or type is important. They should not explain how the implementation works. It should not reference roadmap stages, design decisions, or numbered requirements, or in general discuss the development status of the project - users don't care about this.

_Internal comments_ are meant for people working on the crate, the author, maintainers, and agents who need to understand the code. This can explain
theories of operation and call out subtleties; if necessary it can go into
detail about an algorithm or data structure. However, it should avoid explaining
basic information that any bevy maintainer or rust programmer would already know.

---

## Cross-cutting: the timestamp shim

§2 of the design drains a timestamped event queue by time window. Bevy's input events carry no
timestamps ([bevy#9087][] is the upstream fix, still open), so we need a stand-in — and the whole
point of naming it here is to keep it in **exactly one place**.

Chunk 3 introduces a `Timestamp` newtype and one function that produces it. Until #9087 lands, that
function returns a monotonically increasing sequence number tagged with the frame it arrived in.
Consequences, which should be documented in the module and not papered over:

| Property                                  | Under the shim                    | Requirement |
| ----------------------------------------- | --------------------------------- | ----------- |
| Ordering within a frame                   | exact                             | R9.7        |
| No lost edges across a 0-tick frame       | holds                             | R9.3        |
| No duplicated edges across a 3-tick frame | holds                             | R9.4        |
| Delta magnitude conserved across windows  | holds                             | R9.5        |
| Sub-frame timing accuracy                 | **degraded to frame granularity** | R9.8        |

Everything downstream reads `Timestamp` and never `Instant`, so #9087 lands as a change to one
function plus the removal of a caveat. Gamepad stays frame-quantized regardless until gilrs polling
is rewritten (§11), so mixed fidelity across sources is permanent for now, not an artifact of the shim.

---

## Where this stands

Chunk numbers are **stable identities, not positions**. Ground rule 1 lets a chunk be reordered once
the one before it has been read, and several have been; the phase headings below carry the sequence
so that "chunk 8" still means what it meant in the discussion that produced it.

The target the remaining sequence aims at is **Dead Zone** — an asteroids-like game in primitive
shapes, playable on keyboard or gamepad, eventually with a rebinding screen built on
`bevy_ui_widgets` and operable from the controller. It is not a phase of its own. It arrives early,
badly, and grows a capability per chunk, because ground rule 3 wants something runnable at every
step and a real game is a better acceptance test than a synthetic one.

| | |
| --- | --- |
| **Works today** | Actions and contexts as types; keyboard, mouse buttons and motion, and raw gamepad into an input frame; per-entity context state; N bindings per action folded by intent; the design-stage deadzone; render/fixed evaluation ordered ahead of its readers; each context draining the frame from its own cursor; the three-property model — a source's channel shape checked against the action's intent, with the conversions between shapes settled; mappings and the names to render them with, each holding an ordered list of controls with a capacity, which is what a primary-and-secondary table is; interactive capture per slot, with reserved and excluded controls and read-only conflict detection. |
| **Known wrong today** | Nothing outstanding is wrong so much as absent; the player-facing half of the crate does not exist yet. |
| **Never built** | Rebinding itself: nothing can yet change what a control is bound to, or save the change. Also tunables, presets, prompts, and every screen a player would meet. |

---

## What has landed

Twenty-six chunks are done. The [work log](./Log.md) says what each delivered, what it found, and
where it fell short of its own description; this table is only an index, and the sequence below is
what remains.

| # | Chunk | State |
| --- | --- | --- |
| 1 | Workspace and module skeleton | done |
| 2 | Action identity, value, and intent | done |
| 3 | Derive macros | done |
| 4 | Input frame, keyboard only | done |
| 5 | First end-to-end slice | done, after repair |
| 6 | Axis sources and composites | done, completed by 15 |
| 7 | Modifiers | done; stateful modifiers → 11, `Reflect` → 17 |
| 8 | Gamepad and the design-stage deadzone | done; stages 1 and 3 → 22 |
| 24 | Housekeeping | done; doctests still deferred |
| 9 | Tick domains and the windowed drain | done; the L2 half of R9.3 → 12 |
| 15 | Source channel shape | done |
| 16 | Dead Zone, first playable | playable; no death, which is polish |
| 12 | Transition log and observers | done |
| 13 | Context activation lifecycle | done |
| 11 | Conditions and the scratch table | done; forgiveness windows → 34 |
| 14 | Arbitration and consumption | done; chords on *actions* → 33 |
| 32 | Activation by run condition | done, out of order |
| 17a | Runtime failures (R24.4) | done; the silence it creates → 17b |
| 17b | Plan-build diagnostics | done; unknown controls → 23, observers → 36 |
| 36 | Type-erased inspection and the overlay | done |
| 18 | Derive completion | done |
| 19 | Mappings and localization keys | done; tunables and presets → 23 and later |
| 37 | Naming a control | done; composite structure → §18 |
| 20 | Interactive capture, conflicts, reserved controls | done; the mutation half → 38 |
| 39 | A mapping holds a list of slots | done; reverse lookup → 40, mouse buttons → 41 |
| 41 | Mouse buttons | done; scroll wheel still unclaimed |

Every obligation those chunks left is carried by the chunk that has to discharge it, below, rather
than by the chunk that incurred it — so what a chunk must do is stated in one place.

---

## Phase V — multiple contexts

Dead Zone's pause menu forced the first three of these, which have landed; what is left is the one
that only a focused widget wants.

### 25. Control classes and class bindings

The binding half of R4.9. Chunk 20 landed the shape half — `ControlClass`, decided by the channel a
control reports on — as capture's filter language; what is left is a *binding* that targets a class,
which means the plan grows the second list Design §4.1 describes, consulted when the per-control
index does not claim an event.

- **Not doing:** focus integration. Nothing in-tree binds a class until a focused widget does, so
  this chunk lands the mechanism and its tests, and text input follows when D4 does.
- **Verify `CharacterInput` empirically, do not reason about it.** `KeyboardInput.text` looks like
  the answer, but IME composition arrives on a separate `bevy_window::Ime` channel and whether key
  events still carry text during composition is winit- and platform-specific. This deserves the
  treatment §14's gamepad findings got: measure it, on a real IME, and write down what was actually
  observed. It is the one predicate in the crate a developer is being told to trust rather than
  read (R4.9), so it had better be right.
- **Review surface:** whether R4.10's non-enumerability criterion held. If the set of classes grew
  past a handful while being written, the criterion was abandoned and the case for a closed set goes
  with it.

---

## Phase VI — the parts a solo developer trips over

What R24.8 turned from polish into obligation: the long tail cannot verify what it does not own, so
mistakes have to be caught rather than discovered in QA that nobody is running.

### 17b. Plan-build diagnostics

§9.5's middle tier, which does not exist: plan-build failures collected and reported rather than
asserted one at a time. Unknown controls, shape mismatches a conversion cannot fix, duplicate
bindings, contradictory consume flags (R4.8). Plus the derive's duplicate-key error — declaring
`path` twice currently picks one silently, which for a serialized identity is the worst available
outcome.

- **Inherited from chunk 15:** the intent-versus-channel mismatch is an `assert!` at plan build
  rather than a collected diagnostic, alongside the rescaling check it sits next to.
- **Inherited from chunk 13: a context declared and never spawned says nothing.** Declaring one
  registers a plan and some systems; if no entity ever carries it, every action in it is silently
  dead and the failure looks like "that key does nothing". This cost a bug in Dead Zone's own pause
  menu, in the file that had just been rewritten. A blanket check at startup would be wrong — a
  per-player context legitimately arrives later — but a **state-driven** context whose state is
  current and whose instance count is zero is a much narrower signal, and one worth saying out
  loud.
- **Inherited from chunk 17a, and it made this one more urgent.** Reading a context with no
  instance used to panic, which was the wrong failure but a loud one. It now skips the system
  silently, per Bevy's own rule for `Single`. That is right for the case it was built for — the
  ship is dead, so the system that flies it has nothing to do — and it is indistinguishable at the
  call site from the never-spawned bug above. The signal is what tells the two apart.
- **Inherited from chunk 17a: BSN gives the same silence a second door.** An `on(...)` handler
  attached to an entity that does not carry the context compiles, spawns, and never fires. The
  crate now advertises that pattern, so it owns the diagnostic for getting it wrong.
- **How it is demonstrated, since a correct game shows nothing.** A diagnostic fires when something
  is wrong, so Dead Zone cannot exercise one without being broken on purpose. Three homes instead:
  the message text pinned by tests, since 17b's deliverable *is* the text; `tests/ui/`, which
  already holds the compile-time tier and gains the derive's duplicate-`path` case; and
  `examples/diagnostics.rs`, a runnable catalogue that authors half a dozen wrong binding sets and
  prints what the crate says about each.
- **The catalogue forces validation to be callable without an `App`,** which is worth having on its
  own: it is half of the offline check R8.6 and R19.3 need to test a binding a player has not
  committed to yet. Only half — the clash rule itself stays inside the evaluator until chunk 19,
  and this chunk should not go after it.
- **Review surface:** error text, judged as the deliverable it is. R24.4 distinguishes runtime
  failures (must return errors) from app-build ones (may panic, must be actionable).

### 36. The debug overlay

R22.2, which until now no chunk claimed — an inspector-friendly dump of active contexts, bindings
and action states, and a live overlay in Dead Zone driven by it. `why_not` appears in no example
today, so the runtime diagnostic tier is tested and never seen.

- **Why it is a chunk rather than a nicety.** R22.2 is a `SHOULD` whose only destination was an
  assumption, which ground rule 5 says is an item that will be dropped. It is also the second such
  item found in one sitting; see chunk 35.
- **What it shows:** which contexts are active and at what priority, each action's phase and value,
  and `why_not` for the action under the cursor — the five obstacles that look identical from a
  call site.
- **Why before rebinding rather than after.** When a rebind does not take, the question is whether
  it was capture, conflict, or arbitration, and the overlay is what answers it. Building it after
  chunk 31 means debugging chunk 31 without it.
- **Not doing:** an editor integration. The requirement asks that the same data drive an overlay,
  not that we ship an inspector.

### 17c. Reflect, and the two normalizes

`Reflect` on modifiers and conditions so third-party ones round-trip (R5.6, R17.5). Plus R5.9's two
`normalize` operations, which need naming before either can be written — one clamps to unit length,
the other remaps a range and therefore falls under D6's one-rescaling-stage rule.

- **Why separate:** neither is a diagnostic, and both were riding in 17 because it was the open
  chunk when they were found.
- **Smaller than when it was written.** R17.5 wanted `Reflect` so third-party modifiers round-trip
  through persistence — but the design that settled in §10.1 stores *controls*, so a custom modifier
  never reaches a saved override file. What still wants it is serializing whole binding
  *definitions* (R17.6, R22.16), which is deferred, so this is no longer on the path to anything
  scheduled.

---

## Phase VII — the player-facing model

D7 made real. This is the half of the crate a player ever sees, and per the audience commitment it
must stay additive: a game that declares none of it keeps working exactly as before (R19.13, R24.7).

**The screen arrives in three passes**, because each one is separately capable of being wrong and
mixing them would make it unclear which half was at fault: first a read-only list, then something
navigable, then something that rebinds. Navigation sits between the first two, since a screen you
cannot move around is a help screen rather than a settings screen — which is exactly why the first
pass is worth having on its own.

**The screen Dead Zone is building, stated once.** These are the acceptance criteria for 21, 29, 30
and 31 together, written down here so that each pass can be judged against the finished thing rather
than against its own description:

- **Two tables**, one keyboard and one gamepad, because a rebind is scoped to a scheme (R19.7) and
  device tabs are near-universal in shipped games.
- **The keyboard table has three columns**: description, **primary**, **secondary**. Both cells are
  buttons that initiate a capture. This is also what makes the screen demonstrate horizontal *and*
  vertical navigation, which one column would not.
- **The gamepad table has two columns** — only a primary binding is offered there.
- **Confirm and Cancel at the bottom**, activatable three ways: mouse click; directional navigation
  then A; and a shortcut, B for cancel and X for confirm. **The button caption includes the
  shortcut**, which is R18.1's reverse lookup showing up as a UI requirement rather than a nicety —
  see chunk 40.
- **Two-stage cancel.** B *during a capture* cancels only the capture. B again, with no capture
  live, leaves the screen **without committing**.
- **Working-copy semantics**, which follow from having a Confirm at all: the screen renders
  `pending.get(key)` falling back to the mapping's defaults, and `conflicts()` must be able to
  consult the pending set rather than only what is committed. Chunk 38 owns making it able to.

Chunk 39 built the model half of the three-column table — a mapping holds an ordered list with a
capacity, and a capture names the slot it fills — so 21 draws cells for slots that already exist.

### 21. The settings screen, read-only

In Dead Zone: a screen listing every mapping with what is currently bound to it. A help
screen, and nothing more — no buttons, no focus, dismissed by the same control that opened it.

- **Why this first:** it is the smallest thing that exercises iterating the mapping list and
  rendering a binding as text, which are the two halves of D7 a UI actually needs. If either is
  awkward here it is awkward everywhere, and there is no capture machinery in the way to obscure it.
- **Both tables, and the keyboard one has its cells.** `mapping.capacity.slots()` says how many
  columns to draw and `mapping.slots` fills them in order, so an empty secondary is a blank cell
  rather than an absent one. Dead Zone ships two-control rows for Thrust, Turn and Fire — the last
  of them a key and a mouse button in one row — and a spare slot on Hyperspace, so the read-only
  pass already has every case to draw.
- **Review surface:** whether a UI author can build this without reaching past the mapping list into
  this crate's internals. If they cannot, D7 has leaked.

### 40. Reverse lookup

R18.1, which no chunk claimed: given an action — optionally narrowed by context and device class —
return the controls currently bound to it, in a stable ranked order. This is what "Cancel (B)"
needs, and Dead Zone's settings screen needs it for the button captions its own spec asks for.

- **Why it is not in §18 and not gated with it.** §18's deferral is about the asset pipeline, which
  is true of glyphs (R18.4) and false of this: a reverse lookup returns controls, and chunk 37
  already turned a control into a stored name and a fallback label. Nothing about it waits on an
  atlas.
- **It is the inverse of the plan's control index**, which already maps control → bindings, so the
  work is a second index rather than a scan — and building it as a scan first, to see whether the
  index is worth it, is the honest order.
- **Must reflect active contexts and consumption** (R18.2). A prompt for an action a higher-priority
  context is currently consuming is wrong, and this is the part that cannot be answered from the
  plan alone.
- **Not doing:** R18.3's structured descriptor for composite structure — "hold", "chord of A and B".
  Chunk 37 left that half unbuilt and Design §10.3 records why; it wants a descriptor type wrapping
  the names, and nothing in tree renders one yet. It stays without a chunk, which ground rule 5 says
  is a thing that will be dropped — so it is named here as this chunk's review surface: if the
  captions need it, it belongs to this chunk after all.
- **Review surface:** the ranking. "Stable, ranked order" is doing real work — a player on a pad
  should see the pad control first — and R18.6's most-recently-used-device tracking is what would
  decide it. That is not built either, so this chunk should say plainly what its ranking is based on
  rather than implying it knows which device the player is holding.

### 29. Directional navigation

The half of D4 that is dispatch rather than activation (R22.7): an action whose firing moves the
focus. `bevy_input_focus` has the sequential half wired to events already, and the directional half
only as a `SystemParam` — deliberately, because it was waiting on an input mapper to settle before
going further. So what is chosen here is a candidate for what lands upstream.

- **Delivers, in the crate:** a modifier that quantises a 2D direction to the compass points, and a
  condition that fires when a value changes. Both are general — eight-way movement and radial menus
  want the first; the second is the cheapest condition in the set, needing only the previous value
  that `Scratch` already carries.
- **Why two pieces and not one.** Snapping alone fires every tick, because the stick stays off
  centre. Change-detection alone fires on every wobble. Together they fire once per compass point
  crossed, which is the behaviour a menu wants — and `.on_change().pulse(0.15)` is then auto-repeat,
  out of two conditions that exist for other reasons.
- **Not doing:** bubbling the instruction as a `FocusedInput`. Bubbling exists so that something can
  *intercept*, and until a widget wants to swallow a direction there is nothing to intercept; the
  observer calls `DirectionalNavigation::navigate` directly. The event-driven entry point belongs
  upstream beside `handle_tab_navigation`, where the interception cases live.
- **Not doing:** `InputFocusVisible`. It exists to hide focus rings from desktop mouse users, and a
  pad-driven game has no such ambiguity.
- **In Dead Zone:** a focus ring on the settings screen, drawn with `Outline`.
- **Review surface:** the two names, more carefully than usual. If bevy_input_focus ends up
  depending on these concepts, renaming them afterwards is somebody else's breaking change.

### 30. The settings screen, interactive

Chunk 21's list grows a button per mapping, focus moves between them on stick and D-pad, and the
screen can be dismissed. Still nothing rebinds — pressing a row does nothing yet.

- **Why separate from capture:** navigating a menu with a pad is where the awkwardness usually is,
  and mixing it with capture would make it unclear which half was at fault.
- **Demonstrates R7.3's additive layers,** which chunk 13 predicted would fall out of arbitration
  rather than need a mechanism of their own, and which nothing in tree has shown. A settings screen
  sitting over a running game is a higher-priority context binding a subset of the same controls and
  consuming them. If it needs anything declared beyond that, the prediction was wrong.
- **Verified by:** operating the whole screen from the Xbox pad without touching the keyboard.

### 38. Applying a rebind

The mutation half of what chunk 20 was originally written to cover, split out because it needs
something chunk 20 does not: somewhere to write an answer. **Depends on Design §10.1**, which
designs the structure it writes into.

- **The overrides store, in memory.** §10.1's structure — rows keyed by mapping, valued by that
  mapping's *list* of controls since chunk 39, separated per scheme — plus applying a set to a live
  context: compiling a variant plan and swapping it into that entity's state, which cancels what was
  in flight and re-arms require-reset exactly as `deactivate`/`activate` already do. Chunk 23 then
  adds a file at one end and changes nothing else.
- **The four conflict policies** (R19.3): reject, swap, duplicate-allowed, unbind-the-other, chosen
  by the app. Chunk 20 delivered the detection these act on.
- **Reset to default per binding, per action, per context, and globally** (R19.4). Trivial without a
  store — "reset" means "remove a row" — and impossible with one that does not exist, which is why
  it moved here rather than shipping as a no-op.
- **Inherited from chunk 20: chords are invisible to conflict detection.** Two bindings that share a
  control and differ in their chords are reported as overlapping. Conservative, so it errs toward
  mentioning something harmless; whether a policy should act on it is this chunk's call.
- **Inherited from chunk 39: a control repeated within one row.** `conflicts` excludes the whole
  target mapping rather than the one slot being filled, so binding W into a row whose other slot
  holds W is reported as nothing at all. That is deliberate — it is a policy question, and the
  policies are here — but it means one of these policies has to have an answer for it, and "the
  screen will notice" is not one.
- **Also inherited from 39: the pending set is a list per row.** The working-copy semantics Dead
  Zone's screen needs (see Phase VII's spec) mean `conflicts()` consults pending overrides, and a
  pending row now holds several controls. A pending set keyed by mapping and valued by one control
  would work until the day someone edits a secondary.
- **Review surface:** whether applying mid-session is genuinely the same path as applying at
  startup. §10.1 says it must be, because an authority backend rewrites bindings while the game runs
  (R18.10), and a startup path with a reload bolted on afterwards gets it wrong twice.

### 31. The settings screen, rebinding

Pressing a row enters capture, the next control pressed takes the mapping, and conflicts are
reported per chunk 38's policy.

### 23. Persistence of overrides

A rebind that does not survive a restart is a demo, not a feature. **Designed in Design §10.1**;
what remains here is what building it has to get right and what to look at in review.

- **The crate defines the structure and never learns where it goes.** `Reflect` plus serde behind
  the `serialize` feature; a game embeds it in its own settings resource, an account payload, or
  anywhere else. `bevy_settings` is one vehicle rather than the vehicle.
- **Rows keyed by mapping, valued by control** — not by action, and not by binding. Which makes this
  chunk depend on 19 for the keys, on 20 for the capture that produces the values, and on 38 for the
  structure itself: by the time this runs, the only thing missing is the file.
- **Three states per mapping** (R17.7): absent, cleared, and owned by someone else. A format with
  two cannot express a player deliberately unbinding something, because absence already means
  default.
- **No device identity in it** (R17.8). Bindings name a device class; pairing and calibration are
  separate stores keyed by persistent identity. Two players with identical pads and identical
  mappings differ only in pairing.
- **The control encoding is ours** (R17.9) and **built in chunk 37**: `Control::name` and
  `from_name`, round-trip tested over every variant. The `key/` prefix leaves room for the logical
  half of R12.1, which the binding layer cannot express yet.
- **Applying to a live context is the only path in,** with startup as the first call. An authority
  backend can rewrite bindings mid-session (R18.10), so a startup-only path would be wrong on at
  least one platform before it shipped.
- **Inherited from chunk 17b: unknown controls.** R4.8 names them, and 17b could not write the
  check — `Control` is a typed enum, so a control that does not exist cannot be spelled. A name
  read from a file can be anything, which is where the check belongs.
- **Check before committing to `bevy_settings`:** R17.2 requires an entry that no longer resolves
  to be **reported rather than dropped**, which is what stops a renamed action silently discarding
  a player's rebind. A settings crate that deserializes and quietly ignores what it does not
  recognise cannot satisfy that, and most do exactly that. If it cannot be made to, the diff layer
  is ours and only the file handling is theirs.
- **Review surface:** open the file in a text editor and see whether you can tell what it says. It
  is a format players will edit by hand whatever we intend.

---

## Phase VIII — settling

Nothing here changes what the crate can do.

### 22. The deadzone chain, stages 1 and 3

Calibration and preference, completing D6. Needs the evaluator to stop merging every pad into one
axis map, which is a defect in its own right. Manual calibration API plus an app-driven sampling
step per OQ-4; R14.9's pass-through warning; the preference stage modulating the design stage
without being able to reduce it below what the hardware needs.

- **Now downstream of chunk 26.** Per-device keying is exactly what routing has to introduce, so
  this chunk should follow it rather than build a second way of telling two pads apart.
- **Persistence of calibration** stays blocked on R11.5's stable device identity, which needs two
  units of the same kind to be worth testing.

### 10. The compiled plan and slot allocation

Replace the plan's `BTreeMap` with the `Vec<u16>` action→slot map, the dirty bitset, and the
`Scratch` table's allocation.

- **Success criterion: `examples/` does not change.** A diff there means the abstraction leaked.
- **Why last:** it is an optimization of a shape we now understand, and it is the only chunk that
  adds nothing a player or a developer can see.

---

## Phase IX — the second example

Dead Zone is one player reading one set of bindings. Everything in §15 is invisible to it, because
with a single player there is no question of *which* device drove an action — and a model that never
has to answer that question has not been tested on the thing it was designed for.

### 26. Device routing and the join flow

§15 made real: an input frame event carries the device it came from, a context entity can be paired
to a device, and an unpaired device drives nothing. Plus the join gesture — an app-driven query for
"a device that just pressed something and is not yet claimed" (R15.4), which is the same read of L1
that capture uses in chunk 20.

- **Inherited from chunk 14:** R22.1's fifth obstacle, "device not owned by this player", which is
  unanswerable until something knows who owns what.
- **The gate here was miscounted.** This sat under "deliberately deferred, gated on a real second
  device", but a keyboard is a device: one pad plus a keyboard is two, and a mixed-scheme pair is the
  arrangement most likely to expose a routing bug, since the two do not share a code path. What
  genuinely needs two units of the *same kind* is per-device identity and calibration, which is
  chunk 22's problem and stays deferred.
- **Verified by:** two context entities driven at once, each deaf to the other's device — a test
  that fails loudly today, because every context reads the whole frame. It must pass with **either**
  pairing, and the two are not the same test. A pad and a keyboard exercise the mixed case, where
  the two devices share no code path and a routing bug cannot hide behind symmetry. Two pads of the
  same model exercise identity, where kind tells you nothing and only the device handle
  distinguishes them — which is the case that decides whether the handle is carried far enough.
- **Review surface:** whether pairing is a property of the context entity or a filter on the plan.
  The entity already carries the state, so it is the obvious home, but that puts a device handle in
  a component that has so far been pure.

### 27. Split Friction

`examples/split_friction/` — a split-screen game in the shape of Gauntlet: two players, top-down,
shared world, one viewport each.

- **Not doing:** rebinding. Dead Zone covers that, and a second UI would make this example about
  something other than the thing it is here to show.
- **What it is here to show:** the **device selection screen**. Two mappings, each waiting for a
  device to claim it; press anything on a pad or a key on the keyboard and that mapping is yours.
  This is the first flow in the crate where the player picks the device rather than the developer,
  and it is the one part of §15 a developer cannot get right by reading a doc comment.
- **Verified by:** playing it, with the pad on one mapping and the keyboard on the other, then
  swapping them.

---

### 33. Conditions that read other actions

A chord may require another *control* — `with()` — but not another *action*, and `BlockedBy` does
not exist. Both read a neighbouring mapping rather than their own value, which needs the operand
evaluated first: mappings ordered topologically, and a cycle rejected at plan build with a
diagnostic naming the loop.

- **Why it waits:** self-contained, and nothing in tree wants it. The motivating cases are a
  modal that blocks an action while it is open, and a chord on an action rather than a key — both
  of which the settings screen may turn up, and neither of which is worth guessing at first.
- **Carried from chunks 11 and 14.**

### 34. Forgiveness and sequences

R6.5's **buffering** — accepting an input pressed slightly before it became valid, and firing it
when it does — and **coyote time**, its mirror image. Plus R6.4's ordered sequences, for combos and
cheat codes.

- **Why it waits, and why it is not deferred outright:** the requirements name the absence of
  forgiveness windows as a frequent reason teams abandon a general input crate and hand-roll one.
  That makes it load-bearing rather than optional — but it is also exactly the kind of thing that
  goes wrong when guessed at, because the right window lengths and the right *which input* are
  questions only a real game answers. Dead Zone does not need it. A platformer would, immediately.
- **Both fit the scratch record** as Design §6 predicted, so this is a condition each, not a
  redesign.
- **Carried from chunk 11.**

### 35. Disabling an action

R3.7: an action switched off without being unbound, and switched back on without firing for a
control the player was already holding — the same require-reset the context lifecycle already has,
one level down.

- **Why it exists as its own chunk.** The second grooming recorded it as homed in chunk 17, but
  chunk 17's description never mentioned it, so splitting that chunk would have dropped it. A
  `MUST` whose only record of a destination was in the log is exactly what ground rule 5 forbids.
- **Not a diagnostic**, which is why it does not belong in what 17 became.
- **The mechanism is probably already there.** `require_reset` is per slot and `StateFlags` has
  room; what is missing is the public verb and what it means for a disabled action's in-flight
  state — cancel, on the same terms as deactivating a context, is the answer to beat.

### 28. Docs that run

The documentation half of R24.6, gathered into a chunk because ground rule 5 is right: the
"documents that follow the code" list this replaced was explicitly not a chunk, which made it a list
of things that would quietly never happen.

- **Make the doctests execute.** `dynamic_linking` on the `bevy` dev-dependency breaks the merged
  doctest binary, so every `///` example compiles but none runs, and the public documentation is
  part of the deliverable. Fixing it means making `dynamic_linking` opt-in, at the cost of slower
  example builds — a tradeoff to make deliberately rather than inherit. Carried from chunk 24.
- **Get the requirement and design references out of the public docs.** Found in chunk 39's review:
  around forty doc comments cite an `R`-number, a `§`, an `OQ`, or a `D`-decision, concentrated in
  `capture.rs` (18) and `context.rs` (10). Those render on docs.rs, where the documents they point
  at do not exist — a game developer reading the landing page for `capture` is told that something
  satisfies R19.5 and has no way to find out what R19.5 is. The house style already says this: doc
  comments address users, and roadmap stages, design decisions and numbered requirements are not
  their business.
  - **Module-level `//!` blocks first.** They are the landing pages, and they are the worst of it.
  - **The reasoning usually survives the edit** — "because a rebind is scoped to one control scheme
    (R19.7)" loses nothing by dropping the parenthesis. What must not happen is deleting the
    sentence along with the citation: the *why* is what makes these docs worth reading, and it is
    already written.
  - **`pub(crate)` and internal comments keep their references**, which is where they belong and
    where a maintainer wants them.
  - **Review surface:** read the rendered docs, not the diff. `cargo doc --all-features --open` and
    look at the module pages the way a stranger would.
- **The README rewrite** — a user-facing introduction, feature list, and quickstart, with its
  examples lifted from a real game rather than invented.
- **Comparison with LWIM and `bevy_enhanced_input`** (R22.6) — the migration path the ecosystem will
  ask for. The useful question to ask of each BEI difference is not "is this more ECS-shaped" — it
  reliably is — but "does the ECS-ness earn its keep here". State activation is a case where it
  does; the comparison should say which cases do not, and why.
- **Why last:** the first item can be done at any time and the other two document a moving target.
  The README wants chunk 19, after which the feature list stops growing in the player-facing
  direction; the comparison wants chunks 11 and 14, since conditions and arbitration are where the
  three crates genuinely differ rather than differ in spelling.

---

## Deliberately deferred

Still out of scope for the sequence above. Rebinding, persistence and presentation have left this
table because Dead Zone needs them; device routing and local multiplayer have left it because
Split Friction does, and because the gate turned out to be met already. Reverse lookup (R18.1) left
it in chunk 39's grooming, which is also when the §18 row was found to be claiming an asset gate for
two requirements that have nothing to do with assets.

| Area                                              | Gated on                                                                   |
| ------------------------------------------------- | -------------------------------------------------------------------------- |
| Persistent device identity and calibration (§11)  | two units of the *same kind*, which pad-plus-keyboard does not give         |
| Mouse wheel as a binding source (R13.3)           | nothing in tree wants it. Chunk 41 landed mouse *buttons* and stopped there deliberately: the wheel is a delta on its own channel, needs the `Line`/`Pixel` normalization R13.3 describes, and shares nothing with a button but the device |
| Glyph ids (R18.4)                                 | asset-pipeline questions this document does not touch                      |
| Live prompt invalidation (R18.5), most-recently-used device (R18.6) | nothing that displays a prompt, so there is nothing whose staleness could be observed. Neither is asset-gated; the row above used to claim they were |
| Source and authority backends (D3)                | one working in-tree path to generalize _from_                              |
| Netcode injection and rollback (§10)              | a testbed that actually rolls back; also wants held device state made snapshot-able (R10.3), which chunk 9 left as `BTreeSet`/`HashMap` |
| Focus-driven context activation (R22.8) and text input | chunks 14 and 25 — priority, arbitration, and class bindings are what *claiming* a control means. D4's other half, dispatch (R22.7), needs none of that and is chunk 29. |
| **Guardian migration**                            | porting guardian from bevy 0.16.1 to 0.20-dev — four versions, its own job |

Guardian is worth restating: it is on **bevy 0.16.1** with `bevy_enhanced_input 0.12`, and we target
main. The migration is a genuine goal, but it is a port plus a rewrite, and doing both at once would
confuse "action_map is wrong" with "0.20 moved this". Dead Zone first; guardian when there is
something worth migrating _to_.

---

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
