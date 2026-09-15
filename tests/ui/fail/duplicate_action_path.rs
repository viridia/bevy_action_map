// A path given twice must not compile: silently picking one is the worst available outcome for a
// serialized identity, since the binding a player saved is then stored against whichever the macro
// happened to keep.
#[derive(bevy_action_map::InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
#[action(path = "gameplay.leap")]
struct TwoPaths;

fn main() {}
