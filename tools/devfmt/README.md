# devfmt

Reflows prose in Rust comments (`///`, `//!`, `//`) and markdown files to a fixed column width —
`rustfmt` for the parts `rustfmt` won't touch.

```sh
devfmt [--check] [--width N] <path>...
```

- `<path>` may be a file or a directory (directories are walked recursively for `.rs` and `.md`
  files, skipping `target/`, `.git/`, and other dot-directories).
- Without `--check`, matching files are rewritten in place. With `--check`, nothing is written;
  changed files are listed on stdout and the process exits non-zero — the same contract
  `cargo fmt --check` has.
- `--width` defaults to 100.

Only paragraphs that actually violate the width are touched. A paragraph a human already wrapped
short of the limit is left exactly as written, even though a tighter repacking exists — this
tool's job is fixing violations, not renormalizing prose that was already fine.

Fenced code blocks, table rows (`|...`), headings (`#...`), divider lines (anything with no letter
or digit in it) and reference-style link definitions (`[label]: target`) are never reflowed. A
markdown list item (`- `, `* `, `+ `, `1. `) reflows with its continuation lines hung under the
marker, and a nested item keeps the indentation that makes it nested. A backtick span is never
broken across lines, so one longer than the width overflows it.

Not project-specific — this tool assumes nothing about `bevy_action_map` and is meant to move to
its own repo once its shape has proven out elsewhere too.
