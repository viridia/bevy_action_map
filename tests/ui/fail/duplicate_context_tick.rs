// The same rule for every key the derive takes, not only `path`: two answers to one question is a
// question the macro should not be answering on its own.
#[derive(bevy_action_map::InputContext)]
#[context(path = "gameplay.on_foot", tick = Fixed, tick = Render)]
struct TwoTicks;

fn main() {}
