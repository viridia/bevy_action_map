# Changelog

Changes to `bevy_action_map` and `bevy_action_map_macros` that a game using them would notice, by
release. Each entry ends with the chunk that made it.

## Unreleased

### Fixed

- A consuming class binding, such as a focused text field's, keeps a key from lower-priority
  contexts for as long as the key is held, not only on the frame it was pressed. (206)

## 0.1.2 — 2026-10-09

- The README is trimmed for crates.io. (195)

## 0.1.1 — 2026-10-09

- The README's links work on crates.io, and the macros crate ships its license files. (195)

## 0.1.0 — 2026-10-09

The first release, on Bevy 0.20. (195)
