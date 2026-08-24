//! Every mistake the binding compiler can catch, and what it says about it.
//!
//! Run it to read the messages: `cargo run --example diagnostics`.
//!
//! Nothing here spawns a window or an `App`. Checking a set of bindings reads the bindings and
//! nothing else, which is what lets a rebinding screen ask whether a binding a player is part way
//! through choosing would work, before committing to it.

#![allow(missing_docs)]

use bevy::prelude::*;
use bevy_action_map::binding::InputContextBuilder;
use bevy_action_map::plan::Severity;
use bevy_action_map::prelude::*;

#[derive(InputAction)]
#[action(path = "diagnostics.move", output = Vec2, intent = Directional2)]
struct Move;

#[derive(InputAction)]
#[action(path = "diagnostics.look", output = Vec2, intent = Delta2)]
struct Look;

#[derive(InputAction)]
#[action(path = "diagnostics.jump", output = bool, intent = Button)]
struct Jump;

#[derive(InputAction)]
#[action(path = "diagnostics.menu", output = bool, intent = Button)]
struct Menu;

#[derive(InputContext)]
#[context(path = "diagnostics.example", tick = Render)]
struct Example;

fn main() {
    report("Bindings that are fine", |controls| {
        controls.bind::<Move>(DirectionalButtons::wasd());
        controls.bind::<Jump>(KeyCode::Space);
    });

    report("A direction asked of a single button", |controls| {
        // One key says pressed or not; it cannot say which way. Four of them can, which is what a
        // directional composite is for.
        controls.bind::<Move>(KeyCode::KeyW);
    });

    report("A displacement read as a rate", |controls| {
        // The mouse reports how far it has already moved. Multiplying that by a frame time asks
        // how fast it is moving, which is a question about a quantity nobody measured.
        controls.bind::<Look>(MouseMove).per_second(180.0);
    });

    report("Two deadzones that both rescale", |controls| {
        controls
            .bind::<Move>(DirectionalButtons::wasd())
            .dead_zone(DeadZone::radial(0.05))
            .dead_zone(DeadZone::radial(0.15));
    });

    report("The same control bound twice", |controls| {
        controls.bind::<Jump>(KeyCode::Space);
        controls.bind::<Jump>(KeyCode::Space);
    });

    report("Two bindings that disagree about consuming", |controls| {
        controls.bind::<Menu>(KeyCode::Escape).consume();
        controls.bind::<Jump>(KeyCode::Escape);
    });

    report("A binding both reserved and rebindable", |controls| {
        // Reserving withholds a control from capture so that it cannot be rebound. A mapping
        // exists so that it can. One binding cannot mean both.
        controls.bind::<Menu>(KeyCode::Escape).mappable().reserved();
    });
}

/// Prints what the crate has to say about one set of bindings.
fn report(what: &str, configure: impl FnOnce(&mut InputContextBuilder<Example>)) {
    let mut controls = InputContextBuilder::<Example>::default();
    configure(&mut controls);

    println!("\n{what}");
    let found = controls.diagnostics();
    if found.is_empty() {
        println!("  nothing to report");
        return;
    }

    for diagnostic in found {
        // An error means the context is refused outright; a warning means it will do something,
        // just probably not what was meant.
        let severity = match diagnostic.severity() {
            Severity::Error => "error  ",
            Severity::Warning => "warning",
        };
        println!("  {severity} {diagnostic}");
    }
}
