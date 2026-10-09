//! Public items that no example or other test names, named here so that narrowing one breaks the
//! build rather than quietly changing the API.
//!
//! These check that each item can be reached and used from outside the crate, not what it does:
//! the tests pass by compiling.

use bevy_action_map::action::{ActionInfo, Registers, registered_actions};
use bevy_action_map::prelude::*;

/// A custom condition, the one place a game handles `Registers` itself.
struct Charging;

impl Condition for Charging {
    fn evaluate(
        &self,
        value: ActionValue,
        registers: &mut Registers,
        delta: f32,
    ) -> ConditionState {
        if value.to_bool() {
            registers.time += delta;
            ConditionState::Building
        } else {
            registers.time = 0.0;
            ConditionState::Idle
        }
    }
}

#[test]
fn a_custom_condition_is_handed_its_registers() {
    let _: &dyn Condition = &Charging;
}

#[test]
fn registered_actions_lists_what_has_been_reached() {
    let _: fn() -> Vec<ActionInfo> = registered_actions;
}

/// A backend's own identity for the devices it spawns, which `DeviceId` stores without knowing it.
#[cfg(feature = "bevy_reflect")]
#[derive(bevy::reflect::Reflect, Clone, PartialEq, Eq, Hash, Debug)]
struct SerialNumber(String);

#[cfg(feature = "bevy_reflect")]
impl bevy_action_map::device::DeviceIdentity for SerialNumber {
    const DOMAIN: &'static str = "example.serial";
}

#[cfg(feature = "bevy_reflect")]
fn register_serial_numbers(app: &mut bevy::app::App) {
    use bevy_action_map::device::RegisterDeviceIdentity;
    app.register_device_identity::<SerialNumber>();
}

#[cfg(feature = "bevy_reflect")]
#[test]
fn a_backend_can_declare_its_own_identity_type() {
    let _: fn(&mut bevy::app::App) = register_serial_numbers;
    let _ = bevy_action_map::device::DeviceId::new(SerialNumber("A1".into()));
}

/// Teaching the brand table a vendor it does not ship with, which only a game that names it can do.
#[cfg(feature = "gamepad")]
fn add_a_vendor(mut brands: bevy_ecs::prelude::ResMut<bevy_action_map::gamepad::GamepadBrands>) {
    brands.insert(0x2DC8, bevy_action_map::gamepad::GamepadBrand::Nintendo);
}

#[cfg(feature = "gamepad")]
#[test]
fn a_game_can_add_a_gamepad_vendor() {
    let _: fn(bevy_ecs::prelude::ResMut<bevy_action_map::gamepad::GamepadBrands>) = add_a_vendor;
}

/// A game that reads settings files it did not write, capping how long a row they can make.
fn cap_rows_from_untrusted_files(app: &mut bevy::app::App) {
    app.insert_resource(bevy_action_map::overrides::MaxSlots(8));
}

#[test]
fn a_game_can_cap_an_override_row() {
    let _: fn(&mut bevy::app::App) = cap_rows_from_untrusted_files;
}

#[test]
fn one_split_screen_player_can_apply_their_own_overrides() {
    use bevy::ecs::{entity::Entity, world::World};
    use bevy_action_map::overrides::{OverrideProblem, Overrides, apply_overrides_for};

    let _: fn(&mut World, Entity, &Overrides) -> Vec<OverrideProblem> = apply_overrides_for;
}
