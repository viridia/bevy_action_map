use bevy_action_map::prelude::*;

#[derive(InputContext)]
#[context(path = "gameplay.on_foot", tick = Fixed, priority = 7)]
struct OnFoot;

fn main() {
    assert_eq!(
        <OnFoot as bevy_action_map::action::InputContext>::TICK,
        bevy_action_map::action::TickDomain::Fixed,
    );
    assert_eq!(
        <OnFoot as bevy_action_map::action::InputContext>::PRIORITY,
        7,
    );
}