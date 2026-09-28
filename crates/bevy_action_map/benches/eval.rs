//! Evaluator throughput, measured through `App::update` on a headless app.
//!
//! Every scenario runs with 0, 1 and 8 instances of its context. The 0-instance run registers the
//! context and spawns nothing, so the difference from it is the cost of evaluation alone.
//!
//! ```sh
//! cargo bench -p bevy_action_map --bench eval -- --save-baseline <name>
//! cargo bench -p bevy_action_map --bench eval -- --baseline <name>
//! ```

use bevy_action_map::binding::InputContextBuilder;
use bevy_action_map::prelude::*;
use bevy_app::App;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::{Schedules, SingleThreadedExecutor};
use bevy_input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy_input::mouse::{MouseButton, MouseMotion};
use bevy_input::{ButtonState, InputPlugin};
use bevy_math::Vec2;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

// ---- the small context: shaped like Disasteroids' `Flying` ----

#[derive(InputAction)]
#[action(path = "bench.thrust", output = f32, intent = Analog1)]
struct Thrust;

#[derive(InputAction)]
#[action(path = "bench.turn", output = f32, intent = Analog1)]
struct Turn;

#[derive(InputAction)]
#[action(path = "bench.move", output = Vec2, intent = Directional2)]
struct Move;

#[derive(InputAction)]
#[action(path = "bench.look", output = Vec2, intent = Delta2)]
struct Look;

#[derive(InputAction)]
#[action(path = "bench.fire", output = bool, intent = Button)]
struct Fire;

#[derive(InputAction)]
#[action(path = "bench.bomb", output = bool, intent = Button)]
struct Bomb;

#[derive(InputAction)]
#[action(path = "bench.hyper", output = bool, intent = Button)]
struct Hyper;

#[derive(InputAction)]
#[action(path = "bench.after", output = bool, intent = Button)]
struct After;

#[derive(InputContext)]
#[context(path = "bench.small", tick = Render)]
struct Small;

fn bind_small(controls: &mut InputContextBuilder<Small>) {
    controls.bind::<Thrust>(KeyCode::KeyW);
    controls.bind::<Thrust>(KeyCode::ArrowUp);
    controls.bind::<Turn>(AxisButtons::ad());
    controls.bind::<Turn>(AxisButtons::left_right());
    controls.bind::<Move>(DirectionalButtons::wasd());
    controls.bind::<Look>(MouseMove);
    controls.bind::<Fire>(KeyCode::Space).pulse(0.2);
    controls.bind::<Fire>(MouseButton::Left).pulse(0.2);
    controls.bind::<Bomb>(KeyCode::KeyB).hold_once(1.0);
    controls.bind::<Hyper>(KeyCode::ShiftLeft).multi_tap(2, 0.3);
    controls.follow::<After, Thrust>(|binding| binding.hold(0.75));
}

const SMALL_KEYS: [KeyCode; 8] = [
    KeyCode::KeyW,
    KeyCode::KeyA,
    KeyCode::KeyS,
    KeyCode::KeyD,
    KeyCode::Space,
    KeyCode::KeyB,
    KeyCode::ArrowUp,
    KeyCode::ArrowLeft,
];

// ---- the large context: one button action per key, cycling plain, hold, pulse and chord ----

#[derive(InputContext)]
#[context(path = "bench.large", tick = Render)]
struct Large;

type BindButton = fn(&mut InputContextBuilder<Large>, KeyCode, usize, bool);

/// The `index`th action of the large context. Every fourth is chorded on left shift, which is the
/// shape that makes chord resolution cost something.
fn bind_button<A: InputAction>(
    controls: &mut InputContextBuilder<Large>,
    key: KeyCode,
    index: usize,
    chords: bool,
) {
    let binding = controls.bind::<A>(key);
    match index % 4 {
        1 => {
            binding.hold(0.5);
        }
        2 => {
            binding.pulse(0.2);
        }
        3 if chords => {
            binding.with(KeyCode::ShiftLeft);
        }
        _ => {}
    }
}

macro_rules! large_actions {
    ($($action:ident $path:tt $key:ident,)*) => {
        $(
            #[derive(InputAction)]
            #[action(path = $path, output = bool, intent = Button)]
            struct $action;
        )*

        const LARGE: &[(BindButton, KeyCode)] = &[$((bind_button::<$action>, KeyCode::$key)),*];
    };
}

large_actions! {
    B00 "bench.b00" KeyA,
    B01 "bench.b01" KeyC,
    B02 "bench.b02" KeyD,
    B03 "bench.b03" KeyE,
    B04 "bench.b04" KeyF,
    B05 "bench.b05" KeyG,
    B06 "bench.b06" KeyH,
    B07 "bench.b07" KeyI,
    B08 "bench.b08" KeyJ,
    B09 "bench.b09" KeyK,
    B10 "bench.b10" KeyL,
    B11 "bench.b11" KeyM,
    B12 "bench.b12" KeyN,
    B13 "bench.b13" KeyO,
    B14 "bench.b14" KeyP,
    B15 "bench.b15" KeyQ,
    B16 "bench.b16" KeyR,
    B17 "bench.b17" KeyS,
    B18 "bench.b18" KeyT,
    B19 "bench.b19" KeyU,
    B20 "bench.b20" KeyV,
    B21 "bench.b21" KeyX,
    B22 "bench.b22" KeyY,
    B23 "bench.b23" KeyZ,
    B24 "bench.b24" Digit0,
    B25 "bench.b25" Digit1,
    B26 "bench.b26" Digit2,
    B27 "bench.b27" Digit3,
    B28 "bench.b28" Digit4,
    B29 "bench.b29" Digit5,
    B30 "bench.b30" Digit6,
    B31 "bench.b31" Digit7,
    B32 "bench.b32" Digit8,
    B33 "bench.b33" Digit9,
    B34 "bench.b34" F1,
    B35 "bench.b35" F2,
    B36 "bench.b36" F3,
    B37 "bench.b37" F4,
    B38 "bench.b38" F5,
    B39 "bench.b39" F6,
    B40 "bench.b40" F7,
    B41 "bench.b41" F8,
    B42 "bench.b42" F9,
    B43 "bench.b43" F10,
    B44 "bench.b44" F11,
    B45 "bench.b45" F12,
    B46 "bench.b46" Tab,
    B47 "bench.b47" Enter,
}

/// The first `count` actions of the large context.
fn bind_large(controls: &mut InputContextBuilder<Large>, count: usize, chords: bool) {
    for (index, &(bind, key)) in LARGE.iter().take(count).enumerate() {
        bind(controls, key, index, chords);
    }
}

fn large_keys() -> Vec<KeyCode> {
    LARGE.iter().map(|&(_, key)| key).collect()
}

// ---- driving ----

fn key(key_code: KeyCode, state: ButtonState) -> KeyboardInput {
    KeyboardInput {
        key_code,
        logical_key: Key::Unidentified(NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    }
}

fn app<C: InputContext + Component + Default>(
    declare: impl FnOnce(&mut InputContextBuilder<C>),
    instances: usize,
) -> App {
    let mut app = App::new();
    app.add_plugins((InputPlugin, ActionMapPlugin));
    app.add_context::<C>(declare);
    for _ in 0..instances {
        app.world_mut().spawn(C::default());
    }
    // Startup work settles before measurement begins.
    for _ in 0..3 {
        app.update();
    }
    // The `bevy` dev-dependency turns on `multi_threaded`, whose task pool costs several times
    // what evaluation does and jitters with it.
    for (_, schedule) in app.world_mut().resource_mut::<Schedules>().iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
    }
    app
}

#[derive(Clone, Copy)]
enum Load {
    /// No input.
    Idle,
    /// `n` keys pressed in one update and released in the next.
    Burst(usize),
    /// One mouse motion per update.
    Motion,
}

impl Load {
    fn name(self) -> String {
        match self {
            Load::Idle => "idle".into(),
            Load::Burst(n) => format!("burst{n}"),
            Load::Motion => "motion".into(),
        }
    }
}

/// Two updates, whatever the load, so iterations compare across loads.
fn drive(app: &mut App, load: Load, keys: &[KeyCode]) {
    match load {
        Load::Idle => {
            app.update();
            app.update();
        }
        Load::Burst(n) => {
            for state in [ButtonState::Pressed, ButtonState::Released] {
                for &k in keys.iter().cycle().take(n) {
                    app.world_mut().write_message(key(k, state));
                }
                app.update();
            }
        }
        Load::Motion => {
            for _ in 0..2 {
                app.world_mut().write_message(MouseMotion {
                    delta: Vec2::new(1.5, -0.5),
                });
                app.update();
            }
        }
    }
    black_box(app.world());
}

fn run<C: InputContext + Component + Default>(
    c: &mut Criterion,
    group: &str,
    declare: impl Fn(&mut InputContextBuilder<C>) + Copy,
    keys: &[KeyCode],
    loads: &[Load],
) {
    let mut g = c.benchmark_group(group);
    for &load in loads {
        for instances in [0usize, 1, 8] {
            let mut a = app(declare, instances);
            g.bench_function(BenchmarkId::new(load.name(), instances), |b| {
                b.iter(|| drive(&mut a, load, keys))
            });
        }
    }
    g.finish();
}

fn small(c: &mut Criterion) {
    run::<Small>(
        c,
        "small",
        bind_small,
        &SMALL_KEYS,
        &[Load::Idle, Load::Burst(1), Load::Burst(8), Load::Motion],
    );
}

fn large(c: &mut Criterion) {
    run::<Large>(
        c,
        "large",
        |controls| bind_large(controls, LARGE.len(), true),
        &large_keys(),
        &[Load::Idle, Load::Burst(1), Load::Burst(16)],
    );
}

/// Cost against binding count, idle and under a burst.
fn scale(c: &mut Criterion) {
    let keys = large_keys();
    let mut g = c.benchmark_group("scale");
    for n in [6usize, 12, 24, 48] {
        for instances in [0usize, 1] {
            let mut a = app(|controls| bind_large(controls, n, true), instances);
            g.bench_function(BenchmarkId::new(format!("idle/{n}"), instances), |b| {
                b.iter(|| drive(&mut a, Load::Idle, &keys))
            });
            let mut a = app(|controls| bind_large(controls, n, true), instances);
            g.bench_function(BenchmarkId::new(format!("burst8/{n}"), instances), |b| {
                b.iter(|| drive(&mut a, Load::Burst(8), &keys[..n.min(8)]))
            });
        }
    }
    g.finish();
}

/// The same large context with its chords and without them.
fn chords(c: &mut Criterion) {
    let keys = large_keys();
    let mut g = c.benchmark_group("chords");
    for chords in [true, false] {
        for n in [24usize, 48] {
            for instances in [0usize, 1] {
                let mut a = app(|controls| bind_large(controls, n, chords), instances);
                let with = if chords { "with" } else { "without" };
                g.bench_function(
                    BenchmarkId::new(format!("{with}/idle/{n}"), instances),
                    |b| b.iter(|| drive(&mut a, Load::Idle, &keys)),
                );
            }
        }
    }
    g.finish();
}

criterion_group!(benches, small, large, scale, chords);
criterion_main!(benches);
