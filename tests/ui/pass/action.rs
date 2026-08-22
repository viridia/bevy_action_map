use bevy_action_map::prelude::*;

#[derive(bevy_action_map::InputAction)]
#[action(path = "gameplay.jump", output = bool, intent = Button)]
struct Jump;

fn main() {
    let _ = <Jump as bevy_action_map::action::InputAction>::id();
    assert_eq!(
        <Jump as bevy_action_map::action::InputAction>::INTENT,
        Intent::Button,
    );
}
