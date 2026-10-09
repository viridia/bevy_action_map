# bevy_action_map_ui

UI for [`bevy_action_map`](https://github.com/viridia/bevy_action_map): drawing the input prompts
that tell a player which button performs an action.

`bevy_action_map` decides what is bound and how to name it; this crate puts it on screen with
`bevy_ui`. A game that draws its own prompts needs only the base crate.

**This version is an early one, and its API is not settled.** It holds the list of art providers a
prompt is drawn from; the prompt components themselves join it in a later version, and what is
public may change before then.

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE))
- MIT license ([LICENSE-MIT](./LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in this crate by you shall be dual-licensed as above, without any additional terms or
conditions.
