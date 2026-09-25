# Guidelines

How this project makes the judgements it makes repeatedly: how much to build, and what shape code
takes. Read the first group before scoping a chunk and the second before writing code.

**What this document admits.** Advice a maintainer would otherwise have to learn by being corrected.
The test is whether someone new, following it, would choose differently than without it. A guideline
may be departed from, with the reason stated where the departure is made ("departs from G5 because
…"); a requirement in [`Requirements.md`](../Requirements.md) may not. What the crate is built to do
belongs there, and why a particular choice was made belongs in [`decisions.md`](./decisions.md).

**Numbering.** Each guideline's number is a permanent identity from a single counter, independent of
its group, and never reused. A gap in the sequence is a retired guideline.

**Next: 19.**

---

## 1. Scoping

### G1 — Build for a use that exists

Anchor every API and every chunk to a concrete use, never to a hypothetical one: an imagined problem
is unbounded, and a real one is not. Where a type could accept arbitrary combinations or only the
ones something asks for, take the narrow type, even at the cost of a slightly larger surface. A
chord modifier is `ChordEntry::Modifier(ModifierKey)`, a closed set of four, not a pair of arbitrary
buttons: the pair would admit combinations nothing wants, and each would need a fallback, an error
or a caveat.

### G2 — The cheaper defensible design wins

Between designs that are both correct enough, choose the one with less code and less retained state.
Runtime cost counts as investment alongside code. How uncertain the use case is sizes the
investment: the less sure the benefit, the less it is worth spending. Before engineering against a
defect a chunk names, ask what a player observes and whether prior art treats it as a problem; the
per-device axis map a chunk once planned evaporated when LWIM's "the pad that moved last wins"
turned out to draw no complaints, which is what the crate already did.

That nothing in tree exercises a use sizes its priority. It is not evidence that nobody wants it:
imagine the strongest case before dismissing one.

### G3 — Trace a constraint to its source

When a chunk justifies scope with "X must hold", find the requirement that says so before building
for it. A claim can be an assumption a later document inherited: a settings screen that "must stay
generic over contexts" traced to a debug overlay's registry, reused for convenience and never
required. Check too whether data already in hand answers the need; `ActionMapping::context` made
that chunk's new declaration unnecessary.

### G4 — A requirement can bend

A requirement is written down, not immovable. When a design contorts around one, say which
requirement is forcing it and what relaxing it would buy, and offer withdrawal when its reason has
gone. Test a requirement against how its output is consumed, not only against its own text. Read a
parenthesized list as illustration unless it says otherwise, and write illustrative lists with
"e.g.": a list of names read as exhaustive is how examples become scope. Many requirements came from
an early survey of prior art, so a motivating case may be stale.

### G5 — An exclusion has a cost too

A scope-limiting "not doing X" is not free until its enforcement is priced. Declaring that sticks
had no per-mapping rebinding took four special cases across three modules and caused two bugs;
making a stick one control, as mouse motion already was, collapsed them onto one path with less code
than the exclusion.

### G6 — The library does not inherit the example's limits

No refusal, validation or restriction goes into the crate because an in-tree example cannot do
better. Whether a row is editable is a fact about a screen, and a screen that implements modifier
editing must still be able to ship a chorded default. Prefer documenting a sharp edge over
forbidding it.

### G7 — Do not codify what is not understood

An observation nobody can explain does not become guidance in the README, a requirement or the
design. Record what was seen, and any proposed cause as a hypothesis a measurement could falsify, on
the chunk that will run it; it becomes a `docs/steam.md` entry once measured. A rule written around
a symptom hardens a guess into documentation.

## 2. Code

### G8 — A context and its observers share one scene

An entity carrying a context declares its reactions in the same `bsn!` scene, with `on(…)`, rather
than through a global `App::add_observer`. A transition targets the entity carrying the context, so
the entity, its controls and its reactions are one declaration. Import an observer function rather
than building a scene that holds only an observer.

### G9 — A controller changes the model, and a system redraws

An observer or controller mutates state and never patches the view. A separate system detects the
change and re-derives the view from the whole model, so there is no list of touched cells to get
wrong. `PromptGeneration` and Disasteroids' `redraw_pending` are the shape. If the state changes
every frame, split out the part that changes on discrete events, so change detection on it still
means something; reach for a counter only when ordinary change detection cannot.

### G10 — A new declaration matches the builders

A declaration against an action or context takes the shape `InputContextBuilder` established: a
closure receiving a builder, `bind::<A>(…)` by type, and a panic on an author's mistake rather than
a `Result`. A caller does not look up mappings or construct keys by hand.

### G11 — React to a component with a lifecycle observer

To derive state when a component appears, observe `On<Add<T>>` or `On<Insert<T>>` rather than
scheduling a system filtered on `Added<T>`: an observer needs no ordering against whatever inserted
the component, and does not run every frame. For an entity spawned from a scene whose observer needs
its children, observe `Ready`, which fires only after the entity's related scenes are applied.

### G12 — A component type travels only as a component

A type deriving `Component` is used as a component and nothing else. What crosses a function
boundary or sits inside another struct is the value it wraps: `Option<&DeviceHandleSet>`, not
`Option<&Paired>`.

### G13 — Inline and block variants stay separate

A span-like component and a block one are two types, even when one could render as the other. Their
layout options differ, and a block has to align with the blocks beside it: `IconPrompt` and
`IconPromptSpan`.

### G14 — Know an asset path is valid before loading it

A fallback never hangs off a failed load. Keep an explicit coverage set, generated by whatever
produced the assets, and test membership in it, so an absent asset is a `None` rather than a load
error: `assets/input_prompts/manifest.txt`, written by `scripts/import_input_prompts.py`. Failed
loads in normal operation fill logs until monitoring fires on them.

### G15 — A settings file may be missing or damaged

Reading one never panics or misbehaves. Dropping data that is not understood, back to defaults, is
acceptable, and preserving it is not worth effort unless asked. Assume old files are read by newer
schemas, and prefer encodings whose owner controls its own tolerance over structural reflection,
which fails on both a missing field and an unknown one.

### G16 — Size structures for dozens, not thousands

A game has around a dozen bindings, and one binding every key with modifiers stays under a hundred.
A player holds about four controls at once. Per-frame input structures default to a flat `Vec` and a
linear scan; anything more makes its case against a measurement.

### G17 — A test reads as a narrative

Read top to bottom, a test tells what happens in order. A helper is welcome when it compresses a
step the reader would otherwise write inline and its call site says what it does, as
`press(KeyCode::Space, …)` does wherever `press` is defined. A table-driven loop, a parametrized
helper or an assertion helper with its own branching makes the reader reconstruct the story.

### G18 — Swap in asset-backed children once their assets load

When a redraw spawns entities holding asset handles, it does not despawn the old ones first. The new
handles wait on a pending component while the old children stay drawn, and the swap happens in one
frame, before UI layout, once every asset is loaded; a newer answer replaces the pending one.
`prompt_ui.rs`'s `PendingIcons` is the shape. A headless test of it registers `ImageLoader` by hand,
which the renderer otherwise does.
