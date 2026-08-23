// A path given twice used to pick one silently, which for a serialized identity is the worst
// available outcome: the binding a player saved is stored against whichever the macro happened to
// keep.
#[derive(bevy_action_map::InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
#[action(path = "gameplay.leap")]
struct TwoPaths;

fn main() {}
