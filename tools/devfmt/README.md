# devfmt

Reflows prose in Rust comments (`///`, `//!`, `//`) and markdown files to a fixed column width —
`rustfmt` for the parts `rustfmt` won't touch.

```sh
devfmt [--check] [--preview] [--width N] (--diff[=REF] | --sweep) <path>...
```

- `<path>` may be a file or a directory (directories are walked recursively for `.rs` and `.md`
  files, skipping `target/`, `.git/`, and other dot-directories).
- One scope flag is required. `--diff` (default ref `HEAD`) restricts the work to paragraphs
  overlapping a line `git diff` reports as changed, leaving the rest of the file alone; `--sweep`
  takes every paragraph in the paths given. A run naming neither is refused.
- Given neither reporting flag, matching files are rewritten in place. `--check` writes nothing and
  lists the files that would change. `--preview` writes nothing and prints a unified diff of the
  change itself, its hunks sized to the paragraph that moved. Both exit non-zero when something
  would have changed — the same contract `cargo fmt --check` has.
- `--width` defaults to 100.

A paragraph in scope is refilled whether or not its lines already fit. Editing prose shortens a line
as often as it lengthens one, and a shortened line leaves the paragraph ragged without ever crossing
the width — so checking the width finds only half of what goes wrong, and a tool that reflowed only
violations would leave the other half to be noticed by eye or not at all.

Two things follow. A line broken short on purpose does not survive a run that reaches it, which is
the real cost of this and the reason scope is asked for explicitly rather than defaulted. And
refilling is idempotent, so running over already-refilled prose does nothing: bringing a tree into
this shape is a one-time job, not a standing one.

Fenced code blocks, table rows (`|...`), headings (`#...`), divider lines (anything with no letter
or digit in it) and reference-style link definitions (`[label]: target`) are never reflowed. A
markdown list item (`- `, `* `, `+ `, `1. `) reflows with its continuation lines hung under the
marker, and a nested item keeps the indentation that makes it nested. A backtick span is never
broken across lines, so one longer than the width overflows it.

Not project-specific — this tool assumes nothing about `bevy_action_map` and is meant to move to
its own repo once its shape has proven out elsewhere too.
