//! The crate's reflected types reach an app's type registry with no `register_type` call anywhere.
//!
//! This crate turns no auto-registration feature on: whether derived types register themselves is
//! the app's choice, through `bevy`'s `reflect_auto_register`. The `bevy` dev-dependency has it on
//! by default, so this is the configuration a game built on the umbrella crate sees.

#![cfg(feature = "bevy_reflect")]

use bevy_action_map::ActionMapPlugin;
use bevy_action_map::binding::{ButtonThreshold, Control};
use bevy_action_map::mapping::ActionMapping;
use bevy_action_map::player::Paired;
use bevy_app::App;
use bevy_ecs::reflect::{AppTypeRegistry, ReflectComponent, ReflectResource};
use bevy_input::InputPlugin;

#[test]
fn derived_types_register_themselves() {
    let mut app = App::new();
    app.add_plugins((InputPlugin, ActionMapPlugin));

    let registry = app.world().resource::<AppTypeRegistry>().read();
    assert!(registry.contains(core::any::TypeId::of::<Control>()));
    assert!(registry.contains(core::any::TypeId::of::<ActionMapping>()));

    // Registered with the type data a scene needs to insert it, not merely known by name.
    let paired = registry
        .get(core::any::TypeId::of::<Paired>())
        .expect("Paired is registered");
    assert!(paired.data::<ReflectComponent>().is_some());
    let threshold = registry
        .get(core::any::TypeId::of::<ButtonThreshold>())
        .expect("ButtonThreshold is registered");
    assert!(threshold.data::<ReflectResource>().is_some());
}
