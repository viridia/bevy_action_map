use bevy_math::Vec2;

// A jump is a press, but `Vec2` is a direction: no button action can have that shape.
#[derive(bevy_action_map::InputAction)]
#[action(path = "gameplay.jump", output = Vec2, intent = Button)]
struct Jump;

fn main() {}
