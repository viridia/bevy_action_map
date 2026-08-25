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

The target the remaining sequence aims at is **Disasteroids** — an asteroids-like game in primitive
shapes, playable on keyboard or gamepad, eventually with a rebinding screen built on
`bevy_ui_widgets` and operable from the controller. It is not a phase of its own. It arrives early,
badly, and grows a capability per chunk, because ground rule 3 wants something runnable at every
step and a real game is a better acceptance test than a synthetic one.

| | |
| --- | --- |
| **Works today** | Actions and contexts as types; keyboard, mouse buttons and motion, and raw gamepad into an input frame; per-entity context state; N bindings per action folded by intent; the design-stage deadzone; render/fixed evaluation ordered ahead of its readers; each context draining the frame from its own cursor; the three-property model — a source's channel shape checked against the action's intent, with the conversions between shapes settled; mappings and the names to render them with, each holding an ordered list of controls with a capacity, which is what a primary-and-secondary table is, every binding listed for the player to read and only the declared ones rebindable; interactive capture per slot, with reserved and excluded controls and read-only conflict detection; the first screen a player sees — Disasteroids' controls list, two tables drawn from the mapping list alone, one per device, whose column count comes out of the data rather than the layout; and the lookup that runs the other way, from an action to the controls that would fire it now, behind a trait an external authority can answer for; and that lookup as a **text span** a template can write, which fills in its own string and is told when the answer moves. |
| **Known wrong today** | The controls screen does not take the controls: the game hears them through it, because nothing yet declares a context for the screen (chunk 30). A prompt cannot tell a held binding from a tapped one, which is the condition half of R18.3's structured descriptor, deferred with chunk 44 as its gate. The prelude exports sixteen bare English nouns that a glob import drops into a template beside Bevy's own (chunk 48). Otherwise nothing is wrong so much as absent — the player-facing half of the crate is one read-only list and no way to change anything on it. |
| **Never built** | Rebinding itself: nothing can yet change what a control is bound to, or save the change. Also tunables, presets, glyphs, and every screen that does more than list what is already there. |

---

## What has landed

Thirty chunks are done. The [work log](./Log.md) says what each delivered, what it found, and
where it fell short of its own description — Phase VII onward there, and everything before it in the
[archive](./Log-archive.md). This table is only an index, and the sequence below is what remains.

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
| 16 | Disasteroids, first playable | playable; no death, which is polish |
| 12 | Transition log and observers | done |
| 13 | Context activation lifecycle | done |
| 11 | Conditions and the scratch table | done; forgiveness windows → 34 |
| 14 | Arbitration and consumption | done; chords on *actions* → 33 |
| 32 | Activation by run condition | done, out of order |
| 17a | Runtime failures (R24.4) | done; the silence it creates → 17b |
| 17b | Plan-build diagnostics | done; unknown controls → 23, observers → 36 |
| 36 | Type-erased inspection and the overlay | done |
| 18 | Derive completion | done |
| 19 | Mappings and localization keys | done; presets → 45, tunables → deferred table |
| 37 | Naming a control | done; composite structure → §18 |
| 20 | Interactive capture, conflicts, reserved controls | done; the mutation half → 38 |
| 39 | A mapping holds a list of slots | done; reverse lookup → 40, mouse buttons → 41 |
| 41 | Mouse buttons | done; scroll wheel → deferred table |
| 43 | Listed by default | done; the gap it found → 44 |
| 21 | The settings screen, read-only | done; the caption's reverse lookup → 40, taking the controls → 30 |
| 40 | Reverse lookup | done; invalidation and device ranking → 47, glyphs still deferred |
| 47 | A binding as a text span | done; the presentation layer lives in `examples/common/` until the deferred table's promotion gate trips, and the prelude's other bare nouns → 48 |

Every obligation those chunks left is carried by the chunk that has to discharge it, below, rather
than by the chunk that incurred it — so what a chunk must do is stated in one place.

---

## Phase V — multiple contexts

Disasteroids' pause menu forced the first three of these, which have landed; what is left is the one
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
pass was worth having on its own. That pass has landed; the two that follow have not.

**The screen Disasteroids is building, stated once.** These are the acceptance criteria for 21, 29,
30 and 31 together, written down here so that each pass can be judged against the finished thing
rather than against its own description:

- **Two tables**, one keyboard and one gamepad, because a rebind is scoped to a scheme (R19.7) and
  device tabs are near-universal in shipped games.
- **The keyboard table has three columns**: description, **primary**, **secondary**. Both cells are
  buttons that initiate a capture. This is also what makes the screen demonstrate horizontal *and*
  vertical navigation, which one column would not.
- **The gamepad table is read-only**, and has two columns: description and control. Neither fork of
  the audience rebinds a pad row from here. A game shipping on Steam or a console has the platform's
  own remapper and should link to it; a game with neither gets **presets** — Default and Southpaw as
  buttons under the table (chunk 45). That is the whole gamepad story, and it is why the pad rows
  are listed-and-fixed rather than absent (R19.10): without chunk 43 there would be no table here at
  all.
- **Confirm and Cancel at the bottom**, activatable three ways: mouse click; directional navigation
  then A; and a shortcut, B for cancel and X for confirm. **The button caption includes the
  shortcut**, which is R18.1's reverse lookup showing up as a UI requirement rather than a nicety —
  chunk 40 answered it and chunk 47 built the caption: a `PromptSpan` inside the button's `Text`,
  with a `PromptScheme(Gamepad)` beside it where the caption names a pad control regardless of what
  the game's prompts otherwise speak for.
- **Two-stage cancel.** B *during a capture* cancels only the capture. B again, with no capture
  live, leaves the screen **without committing**.
- **Working-copy semantics**, which follow from having a Confirm at all: the screen renders
  `pending.get(key)` falling back to the mapping's defaults, and `conflicts()` must be able to
  consult the pending set rather than only what is committed. Chunk 38 owns making it able to.

Chunk 39 built the model half of the three-column table — a mapping holds an ordered list with a
capacity, and a capture names the slot it fills — 43 put the fixed rows on it, and 21 drew both
tables from that model alone. What is left is everything that happens when the player presses one.

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
- **In Disasteroids:** a focus ring on the settings screen, drawn with `Outline`.
- **Review surface:** the two names, more carefully than usual. If bevy_input_focus ends up
  depending on these concepts, renaming them afterwards is somebody else's breaking change.

### 30. The settings screen, interactive

Chunk 21's list grows a button per mapping, focus moves between them on stick and D-pad, and the
screen can be dismissed. Still nothing rebinds — pressing a row does nothing yet.

- **Why separate from capture:** navigating a menu with a pad is where the awkwardness usually is,
  and mixing it with capture would make it unclear which half was at fault.
- **Inherited from chunk 21: the screen does not take the controls.** It is a state of its own
  rather than a third `Game` variant, so the ship still answers the throttle while the player reads
  the table. That was left visible deliberately — standing the flying context down would have hidden
  the very thing this chunk exists to demonstrate — and it is the acceptance test: with the screen
  up, the controls it binds must reach it and nothing else.
- **Demonstrates R7.3's additive layers,** which chunk 13 predicted would fall out of arbitration
  rather than need a mechanism of their own, and which nothing in tree has shown. A settings screen
  sitting over a running game is a higher-priority context binding a subset of the same controls and
  consuming them. If it needs anything declared beyond that, the prediction was wrong.
- **Verified by:** operating the whole screen from the Xbox pad without touching the keyboard.

### 44. Bindings that travel together

Several actions may deliberately share one physical control — tap to dodge, hold to sprint — and
the player rebinds *the control*, not one of the actions. The model has no way to say so: the unit of
rebinding is the action's path plus a part (R19.9), which assumes one action per binding, so the two
get separate rows and a rebind moves only one of them.

- **`.follows::<A>()` on the binding.** It rides `A`'s mapping: contributes no slots of its own, is
  not listed separately, and applying a rebind to that mapping rewrites it too.
- **Why before 38.** The bug is latent until a rebind can be applied, and then it is immediate:
  rebind Thrust to `J` and the afterburner stays on `W`, and if the player later puts Fire on `W`,
  holding Fire afterburns. Chunk 38 is the deadline rather than the discoverer.
- **Conflict detection cannot catch it**, which is why it needs saying in the model rather than in a
  diagnostic. Nothing collides — the failure is a *separation* that should not have been possible,
  and `conflicts()` only looks for two rows holding one control.
- **Found by chunk 43**, which made it visible: listing by default put Afterburner on the screen
  under its own name, next to Thrust, holding the same keys. It was equally broken before and simply
  could not be seen.
- **Checkable at plan build:** the target must exist, be `mappable`, be in the same scheme, and read
  the same controls. A `follows` that reads different controls is a different binding, not a linked
  one, and saying so early is cheaper than a player finding it.
- **Disasteroids is the test case**, and carries a stopgap in the meantime: its three `Afterburner`
  bindings are `private`, which produces the same screen this chunk will and none of the linkage.
  Replacing those three calls with `follows::<Thrust>()` is this chunk's acceptance test.
- **Landing this trips a deferred gate.** Taking `private` off those three bindings puts a held
  binding on a screen beside a tapped one reading the same key, which is the condition half of
  R18.3's descriptor — deferred with this chunk named as its gate. Answer it here or write it onto a
  chunk, but it must not land unnoticed: R18.3 is a `MUST`, and this is where it stops being
  invisible.
- **Not doing:** inferring the link from two bindings happening to read one control. That is true of
  coincidences as well as intentions, and the two want opposite handling.
- **Review surface:** whether `follows` is the right shape for the *other* case it resembles — chunk
  33's conditions that read another action. Afterburner is genuinely "Thrust, still held", and a
  game that could say that would need no link at all. If 33 subsumes enough of this, the two should
  be looked at together before both are built.

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
- **A fifth outcome: "not ours, delegate"** (R19.8). When a backend is authoritative for an action,
  rebinding is its overlay's job and conflict detection does not run, because we do not own the
  rules. This is a variant of the outcome type the four policies already need, so it costs a line
  here and a signature change afterwards. §10.1's third override state — "not ours" — is the same
  distinction already made on the persistence side, and Design §10.5 says what is on the other end.
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
- **Also inherited from 39: the pending set is a list per row.** The working-copy semantics
  Disasteroids' screen needs (see Phase VII's spec) mean `conflicts()` consults pending overrides,
  and a pending row now holds several controls. A pending set keyed by mapping and valued by one
  control would work until the day someone edits a secondary.
- **Inherited from chunk 47: applying a rebind raises `PromptGeneration`.** Chunk 47 raises it for
  everything that existed to raise it for — a context switching over, an instance arriving or
  leaving — and a binding *changing* is the clause of R18.5 nothing could reach yet. One call at the
  point a variant plan is swapped in, and every prompt on screen catches up; without it a caption
  goes on naming the key the player just replaced, which is the exact failure R18.5 exists to
  prevent and is invisible until someone rebinds with a HUD up.
- **Review surface:** whether applying mid-session is genuinely the same path as applying at
  startup. §10.1 says it must be, because an authority backend rewrites bindings while the game runs
  (R18.10), and a startup path with a reload bolted on afterwards gets it wrong twice.

### 31. The settings screen, rebinding

Pressing a row enters capture, the next control pressed takes the mapping, and conflicts are
reported per chunk 38's policy.

### 45. Presets

R19.12, which had no chunk until the gamepad table needed one. A preset is a named arrangement of
mappings and tunables applied as a unit — "Default", "Southpaw", "Lefty" — and for the device
classes where per-mapping rebinding is not offered it is not a convenience but *the entire remapping
story*. A stick has no row to press.

- **Why after 38.** A preset is a set of assignments, so it applies through the same path a rebind
  does (§10.1) rather than a second writer. Building it before there is an apply path means
  inventing one, and then having two.
- **Disasteroids is the acceptance test, and the fork is the lesson.** Its gamepad table is
  read-only either way; what sits under it is the choice. A game on Steam or a console links to the
  platform's remapper; a game with neither gets Default and Southpaw as buttons, which is the
  arrangement most shipped games actually have. Disasteroids teaches the second, since the first is
  chunk 42's territory and a demo cannot show both without becoming a lecture.
- **Southpaw is the honest test case** — it swaps two *sticks*, so it cannot be expressed as a
  keyboard-style rebind at all, and a preset mechanism that only rearranges buttons would pass a
  weaker one.
- **What has to be decided: whether a preset is a starting point or a layer.** If a player picks
  Southpaw and then rebinds one row, does a later "Southpaw" reapply discard their edit? Both
  answers ship in real games. This is the chunk that has to pick one, and 23 stores whichever it is:
  a preset name, or a preset name plus a diff against it.
- **Tunables are in scope by R19.12 and may not be in scope here.** They are a separate declaration
  that does not exist yet, so the mapping half can land first — but the preset *format* has to leave
  room, since retrofitting a second kind of entry into a saved file is the migration R17 exists to
  avoid.
- **Review surface:** whether a game with one preset pays anything. R19.13 says presets are
  additive, and the shape that satisfies it is a default preset that no one has to declare.

### 23. Persistence of overrides

A rebind that does not survive a restart is a demo, not a feature. **Designed in Design §10.1**;
what remains here is what building it has to get right and what to look at in review.

- **The crate defines the structure and never learns where it goes.** `Reflect` plus serde behind
  the `serialize` feature; a game embeds it in its own settings resource, an account payload, or
  anywhere else. `bevy_settings` is one vehicle rather than the vehicle.
- **Rows keyed by mapping, valued by control** — not by action, and not by binding. Which makes this
  chunk depend on 19 for the keys, on 20 for the capture that produces the values, and on 38 for the
  structure itself: by the time this runs, the only thing missing is the file.
- **Plus whatever chunk 45 decides a preset selection is** — a name, or a name and a diff against
  it. That is one field or two, but it is the difference between reapplying a preset discarding the
  player's edits and preserving them, and a file written before the question is answered answers it
  by accident.
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

### 48. Names that survive a glob import

The prelude exports sixteen bare English nouns — `Scheme`, `Mapping`, `Capacity`, `Conflict`,
`Overlap`, `Captured`, `Refused`, `Condition`, `Verdict`, `Part`, `Intent`, `Phase`, `Obstacle`,
`Timestamp`, `Rebinding`, `Actions` — plus the four transition events, and a game glob-imports it
beside Bevy's own. Found in chunk 47, which renamed the two it touched (`Scope` → `PromptScope`,
`Origin` → `ControlOrigin`) and left the rest rather than smuggling a crate-wide rename into a
feature chunk.

- **Four of those were missed when this chunk was written**, and turned up by re-reading the prelude
  rather than the list: `Obstacle`, `Timestamp`, `Rebinding` and `Actions`. `Timestamp` is the one
  another crate is most likely to export as well; `Obstacle` is the one no reader would guess. The
  other two are here to be judged rather than assumed — `Actions` probably does earn its bareness in
  a crate called `bevy_action_map`.

- **Why it is not cosmetic.** BSN templates are where it bites: a scene lists components from
  several preludes with nothing saying which crate each came from, so a name that does not carry its
  domain reads as whatever the reader assumes. `Scope` was the worst of them and is done.
- **The criterion, so the pass is not taste.** A name earns its bareness if a reader who knows Bevy
  but not this crate would guess right. `Control` and `Prompt` pass. `Phase` and `Part` do not.
- **Not doing:** deprecation shims. Nothing outside this repository depends on the crate yet, and
  the moment that stops being true this chunk gets more expensive than it is worth — which makes
  this a chunk with a shelf life rather than one that can wait indefinitely.
- **Review surface:** whether the renames read as prefixes bolted on. `PromptScope` reads as one
  thing; `InputActionPhase` would not, and where that happens the answer is a better word rather
  than a longer one.

---

## Phase IX — the second example

Disasteroids is one player reading one set of bindings. Everything in §15 is invisible to it,
because with a single player there is no question of *which* device drove an action — and a model
that never has to answer that question has not been tested on the thing it was designed for.

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
- **Consumption is device-blind, and R15.3 says it must not be.** `ConsumedControls` is keyed on
  `Control`, which names South without naming *whose* pad, so one player's modal context consuming a
  button takes it from the other player's gameplay context. Instances of one context are already
  safe — claims are batched per context rather than applied one instance at a time, deliberately —
  and it is the cross-context case that collides. This is not visible in a one-player game and is
  the first thing two pads will do.
- **Split Fiction is the precedent, and it cuts both ways.** When one player opens the options menu
  there, the other is locked out, symmetrically — so a modal screen owned by one player standing the
  other down is a *shipped* answer and not a bug to design away. What the crate must not do is
  deliver it by accident: a game whose inventory screen should leave the other player flying has no
  way to say so if consumption cannot tell two pads apart. So the lockout has to be the app's
  choice, which means routing has to reach consumption rather than stopping at the frame.
- **Review surface:** whether pairing is a property of the context entity or a filter on the plan.
  The entity already carries the state, so it is the obvious home, but that puts a device handle in
  a component that has so far been pure — and the bullet above is the case that decides it, since a
  filter on the plan leaves the consumed set global whichever way the rest goes.

### 27. Split Friction

`examples/split_friction/` — a split-screen game in the shape of Gauntlet: two players, top-down,
shared world, one viewport each.

- **Not doing:** rebinding. Disasteroids covers that, and a second UI would make this example about
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
not exist. Both read a neighbouring slot rather than their own value, which needs the operand
evaluated first: slots ordered topologically, and a cycle rejected at plan build with a
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
  questions only a real game answers. Disasteroids does not need it. A platformer would,
  immediately.
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

### 42. The authority backend, faked

D3 made real against something that is not Steam, because the seam is only proven by a second
implementer and the real one cannot live here. **Depends on Design §10.5**, which records what Steam
Input actually is and settles the three places its model disagrees with ours.

- **The traits land in `src/backend.rs`**, which has been a doc comment and no code since chunk 1.
  An authority backend supplies an `ActionValue` per owned action, substituted for the fold's output
  inside the evaluator so our state machine still synthesizes the edges (§10.5). A source backend
  needs nothing new — `InputFrame::record` is already public and already the door.
- **The mock lives entirely in `examples/`**, not in `src/` behind a feature. The traits are public
  API and carry a maintenance promise; the fake is a test fixture and gets deleted when a real
  backend exists. Ground rule 3 already makes the examples the acceptance test.
- **It must fake the API, not the concept.** Level-only reads with no timestamps, an "is this bound"
  flag distinct from a zero value, origins as a type that is deliberately not `Control`, a glyph as
  a path, and a binding panel that is ugly on purpose. A mock nicer than Steam proves nothing, and
  every one of those is a place §10.5 guessed.
- **The acceptance criterion is a non-diff, and it is no longer Disasteroids' pad.** The original
  had Disasteroids' pad become backend-owned with every other file in the example untouched. The
  preset fork rules that out: Disasteroids' pad is where presets get taught (chunk 45), and a pad
  the backend owns has no presets of ours to show. The proof still has to be a non-diff — screen
  code running unchanged against a backend-owned context — but it needs a vehicle that is not
  already spoken for. Choosing one is part of this chunk, and the awkward part is that sharing the
  settings screen between two examples is a `#[path]` trick rather than a module.
- **R0.6, the half that is not about Steam.** A backend suppresses its devices at L0 so their raw
  events never reach the frame. This is what makes the demo usable at all — without it Disasteroids
  reads the pad twice — and it is why this chunk sits after 26 rather than after 38: the filter's
  key is the device identity chunk 26 puts on the frame, and the runtime entity is a stopgap that is
  wrong across a reconnect.
- **Blocked on 40 and 38** for the other two halves. Reverse lookup has to be a trait before a
  backend can answer it (R18.8) and rebinding has to be able to say "not ours" (R19.8); both are
  written onto those chunks. The authority-write half needs neither and could be split out early if
  something starts wanting it sooner.
- **Review surface:** whether §10.5's three decisions survive contact. Each was written to be
  falsifiable by this chunk — a backend-owned action that accepts a `.hold()` without a diagnostic,
  two modal contexts that must be live on one pad at once, or an input observed twice — and a
  decision this chunk cannot break is a decision that was not made.

### 46. Documents an outsider will read

`Requirements.md` and `Design.md` are 30,000 words between them, and the point of writing them was
to get them critiqued by people who know Bevy — who owe this project nothing and will not read
30,000 words to do us a favour. Length is the barrier, and it has to come down before the ask, not
after. Numbered here rather than left as an intention, because a documentation pass with no chunk
number is the thing ground rule 5 says will be dropped.

Two questions it has to **answer rather than assume**, since the obvious framing — "remove the
redundancy" — may be the wrong job:

- **Which document does an outsider actually read?** A reviewer needs one entry point, not two, and
  neither of these is it: one is a specification and the other is an architecture. What the ask
  probably wants is a *third*, much shorter document — the argument, not the spec — that says what
  problem this crate solves, which decisions are load-bearing, and where a reader who disagrees
  should aim. The other two become the appendix it cites. That is a different job from shortening,
  and a cheaper one; if it is the right job, most of the rest of this chunk evaporates.
- **Is the problem redundancy, or is it staleness?** Some duplication is deliberate and should
  survive: R19.10 is normative and §9.7 is explanatory, they overlap on purpose, and collapsing them
  costs more than it saves. What is worth hunting is the copy that *did not follow* when a decision
  moved — chunk 43 found two of those in prose the chunk 39 rename had missed, and neither was
  visible from the diff. That sweep is worth doing on its own terms whatever happens to the length.

- **The sweep is a reading, not a grep.** This is chunk 39's lesson arriving again: a protect-list
  keyed on identifiers does not cover prose, and both regressions were found by reading around the
  change rather than by searching for it.
- **Not doing: shortening for its own sake.** The rationale given in place is why these documents
  are worth reading at all, and a requirement stripped to its `MUST` clause is one nobody can argue
  with — which is the opposite of what the ask is for.
- **Why this late.** The documents are still moving. Phase VII rewrites the player-facing half of
  both, and a redundancy pass run before it lands gets redone.
- **Review surface:** hand it to someone who has read none of it and watch where they stop. That is
  the only measurement of this chunk that means anything, and it is available cheaply — the same
  people who would eventually give the critique will give the *first ten minutes* of one for free.

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
table because Disasteroids needs them; device routing and local multiplayer have left it because
Split Friction does, and because the gate turned out to be met already. Reverse lookup (R18.1) left
it in chunk 39's grooming, which is also when the §18 row was found to be claiming an asset gate for
two requirements that have nothing to do with assets.

Those two — live prompt invalidation (R18.5) and most-recently-used device (R18.6) — left it as
well, and only one of them survived the trip. R18.5's gate is a good example of one a chunk meets by
existing: it waited on something displaying a prompt whose staleness could be observed, and chunk 47
is that thing, so it is written onto 47. R18.6 is **withdrawn** instead of scheduled — which device
a prompt speaks for is a parameter the app supplies, not a thing the crate infers from what was
pressed last, and inferring it is scope this crate does not need to carry. Glyphs (R18.4) stay,
because theirs is an asset-pipeline gate and 47 renders text.

Chunk 47 then added a row of its own, for a reason it went looking for and found: the component that
draws a prompt needs `bevy_text` and `bevy_ui`, and `bevy_ui` already depends on `bevy_input` and
`bevy_input_focus`. So the presentation layer lives in `examples/common/prompt_ui.rs` — imported by
path, tested by `tests/prompt_ui.rs`, and shared with Split Friction when it arrives — rather than
inverting the layering this crate is arranged to keep. The gate on promoting it is below, and it is
worth saying what is *not* gating it: the code is finished, and moving it is a `git mv` plus a
manifest.

Backends (D3) left it in the Steam grooming, and the gate is worth restating because the row used to
read as though any second implementer would do. It named "one working in-tree path to generalize
_from_", which is true and was doing no work: what the seam actually waits on is chunks 40 and 38,
because R18.8 and R19.8 are surfaces those two either build as substitutable or foreclose. That is
now written onto both, the seam is designed in Design §10.5, and the implementer is chunk 42. Chunk
40 has since discharged its half: reverse lookup is a trait, and its return type admits an origin
that is not one of our `Control`s, which is R18.9's demand met at the only price it was ever going
to be cheap at.

| Area                                              | Gated on                                                                   |
| ------------------------------------------------- | -------------------------------------------------------------------------- |
| Persistent device identity and calibration (§11)  | two units of the *same kind*, which pad-plus-keyboard does not give         |
| Mouse wheel as a binding source (R13.3)           | nothing in tree wants it. Chunk 41 landed mouse *buttons* and stopped there deliberately: the wheel is a delta on its own channel, needs the `Line`/`Pixel` normalization R13.3 describes, and shares nothing with a button but the device |
| Glyph ids (R18.4)                                 | asset-pipeline questions this document does not touch — but *the art is no longer one of them*: **Kenney's input prompt set** covers keyboard, mouse and the three pad brands and is CC0, so an example can ship one without a licensing conversation. Confirm the licence and the coverage before relying on either. What stays open is the identifier scheme, and Kenney is the way to falsify it: R18.4 wants a key of (brand, control) with a brand → generic → text fallback, and chunk 37's stored names are already the control half — `pad/South` plus a brand is nearly the whole id. If that mapping does not survive contact with a real atlas's file names, R18.4 is wrong rather than merely unbuilt |
| The condition half of R18.3's descriptor          | chunk 44. The chord half landed with 47 — a binding needing a modifier alongside its own control reports both, so `Ctrl+S` does not render as "S" — and the condition half did not: a `.hold(0.75)` and a tap on the same key still produce an identical prompt. Nothing in tree shows it, because the only case is Disasteroids' `Afterburner` and chunk 44's stopgap keeps those three bindings `private`. Chunk 44 replaces `private` with `follows::<Thrust>()`, which puts them back on a screen — so 44 landing is the gate, and it is where a MUST stops being merely unbuilt and starts being visibly unmet |
| Tunables (R19.11)                                 | nothing in tree adjusts one. Chunk 19 landed mappings and left tunables reading "23 and later", which is the destination ground rule 5 refuses; chunk 45 needs the preset *format* to leave room for them and does not need them to exist. Wanted by R20.5 and by Disasteroids, whose stick deadzone is the tunable a player would reach for first, so this row is a question to reopen rather than a settled no |
| Glyphs from a backend (R18.9) | the same asset-pipeline questions as the R18.4 row, arriving from the other side. The *origin* half of this row is closed: chunk 40's `ControlOrigin` has a variant for a control that is not one of ours, carrying the same stored name and fallback label everything else renders from, so what is deferred is the image rather than room for it. Chunk 47 widened that variant with the class its reporter claims, so a foreign control can be narrowed to like one of ours |
| **A presentation crate** (`bevy_action_map_ui`, or wherever it lands) | **Bevy deciding to take this crate upstream.** That is the point at which the workspace has to be arranged properly regardless, and it is also when the layering matters to someone other than us: an input crate that pulls `bevy_ui` cannot go upstream, and `bevy_ui` could not then use action maps. Until then the layer is `examples/common/prompt_ui.rs`, which every example shares and an integration test covers, and the cost of waiting is a `#[path]` import |
| Netcode injection and rollback (§10)              | a testbed that actually rolls back; also wants held device state made snapshot-able (R10.3), which chunk 9 left as `BTreeSet`/`HashMap` |
| Focus-driven context activation (R22.8) and text input | chunks 14 and 25 — priority, arbitration, and class bindings are what *claiming* a control means. D4's other half, dispatch (R22.7), needs none of that and is chunk 29. |
| **Guardian migration**                            | porting guardian from bevy 0.16.1 to 0.20-dev — four versions, its own job |

Guardian is worth restating: it is on **bevy 0.16.1** with `bevy_enhanced_input 0.12`, and we target
main. The migration is a genuine goal, but it is a port plus a rewrite, and doing both at once would
confuse "action_map is wrong" with "0.20 moved this". Disasteroids first; guardian when there is
something worth migrating _to_.

---

[bevy#9087]: https://github.com/bevyengine/bevy/issues/9087
