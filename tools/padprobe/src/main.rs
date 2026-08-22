//! Gamepad probe: what does a controller actually report through gilrs?
//!
//! gilrs is the crate Bevy's gamepad support *is*, so this answers "will this pad work with
//! bevy_action_map?" without building a Bevy app. It also measures the numbers the deadzone design
//! (Requirements §14, D6) needs from real hardware.
//!
//! Usage: `padprobe [seconds] [--bevy | --unfiltered]`
//!
//! Filter modes matter more than they look — see the table printed at startup.

use gilrs::ev::filter::{Filter, axis_dpad_to_button};
use gilrs::{Axis, Button, Event, Gilrs, GilrsBuilder};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// gilrs defaults: axis_dpad_to_button + Jitter + deadzone(radial 0.1, rescaling).
    GilrsDefault,
    /// Exactly what `bevy_gilrs` does: default filters off, `axis_dpad_to_button` re-applied.
    Bevy,
    /// No filters at all. More raw than Bevy — a hat D-pad stays as DPadX/DPadY axes.
    Unfiltered,
}

impl Mode {
    fn describe(self) -> &'static str {
        match self {
            Mode::GilrsDefault => {
                "gilrs defaults (dpad->button + jitter + RADIAL 0.1 DEADZONE w/ rescaling)\n  \
                 NOTE: stick values are deadzoned. Not what Bevy sees. Use --bevy for that."
            }
            Mode::Bevy => {
                "replicating bevy_gilrs: default filters OFF, axis_dpad_to_button re-applied\n  \
                 Stick values are raw (no deadzone anywhere). This is what RawGamepadEvent carries."
            }
            Mode::Unfiltered => {
                "no filters at all. More raw than Bevy: a hat D-pad stays as DPadX/DPadY axes\n  \
                 rather than four buttons."
            }
        }
    }
}

fn main() {
    let mut secs = 30u64;
    let mut mode = Mode::GilrsDefault;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--bevy" => mode = Mode::Bevy,
            "--unfiltered" => mode = Mode::Unfiltered,
            other => {
                if let Ok(n) = other.parse() {
                    secs = n;
                } else {
                    eprintln!("usage: padprobe [seconds] [--bevy | --unfiltered]");
                    return;
                }
            }
        }
    }

    let built = match mode {
        Mode::GilrsDefault => Gilrs::new(),
        _ => GilrsBuilder::new().with_default_filters(false).build(),
    };
    let mut gilrs = match built {
        Ok(g) => g,
        Err(e) => {
            println!("Gilrs::new() failed: {e}");
            return;
        }
    };

    println!("mode: {}\n", mode.describe());

    // gilrs discovers devices on its first poll, so drain briefly before enumerating.
    let warmup = Instant::now();
    while warmup.elapsed() < Duration::from_millis(300) {
        while gilrs.next_event().is_some() {}
        std::thread::sleep(Duration::from_millis(10));
    }

    println!("=== connected pads ===");
    let mut any = false;
    for (id, pad) in gilrs.gamepads() {
        any = true;
        println!(
            "[{id}] {name:?}\n     uuid       = {uuid}\n     map_source = {src:?}\n     power      = {power:?}",
            name = pad.name(),
            uuid = uuid_str(pad.uuid()),
            src = pad.mapping_source(),
            power = pad.power_info(),
        );

        let axes: Vec<&str> = AXES
            .iter()
            .filter(|(a, _)| pad.axis_code(*a).is_some())
            .map(|(_, n)| *n)
            .collect();
        println!("     axes       = {axes:?}");

        let buttons: Vec<&str> = BUTTONS
            .iter()
            .filter(|(b, _)| pad.button_code(*b).is_some())
            .map(|(_, n)| *n)
            .collect();
        println!("     buttons    = {buttons:?}");
    }
    if !any {
        println!("(none — gilrs sees no gamepads at all)");
    }

    println!(
        "\n=== live events ({secs}s) — move both sticks fully, squeeze both triggers, press every \
         face button and the D-pad ===\n"
    );

    let start = Instant::now();
    let mut ext = [[0f32; 2]; 4]; // [axis][min, max]
    let mut seen_buttons: Vec<String> = Vec::new();
    // Smallest non-zero magnitude seen per stick axis: a proxy for resolution and jitter floor.
    let mut min_nonzero = [f32::MAX; 4];

    while start.elapsed() < Duration::from_secs(secs) {
        loop {
            let ev = gilrs.next_event();
            let ev = match mode {
                Mode::Bevy => ev.filter_ev(&axis_dpad_to_button, &mut gilrs),
                _ => ev,
            };
            let Some(Event { id, event, .. }) = ev else {
                break;
            };
            match event {
                gilrs::EventType::AxisChanged(axis, v, code) => {
                    println!("[{id}] axis  {axis:?} = {v:+.4}   (code {code})");
                    if let Some(i) = stick_index(axis) {
                        ext[i][0] = ext[i][0].min(v);
                        ext[i][1] = ext[i][1].max(v);
                        if v != 0.0 {
                            min_nonzero[i] = min_nonzero[i].min(v.abs());
                        }
                    }
                }
                gilrs::EventType::ButtonChanged(b, v, code) => {
                    println!("[{id}] btn~  {b:?} = {v:.3}   (code {code})");
                }
                gilrs::EventType::ButtonPressed(b, code) => {
                    println!("[{id}] PRESS {b:?}   (code {code})");
                    let s = format!("{b:?}");
                    if !seen_buttons.contains(&s) {
                        seen_buttons.push(s);
                    }
                }
                gilrs::EventType::ButtonReleased(b, code) => {
                    println!("[{id}] rel   {b:?}   (code {code})");
                }
                other => println!("[{id}] {other:?}"),
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    println!("\n=== summary ===");
    for (i, (_, n)) in AXES.iter().take(4).enumerate() {
        let floor = if min_nonzero[i] == f32::MAX {
            "n/a".to_string()
        } else {
            format!("{:.4}", min_nonzero[i])
        };
        println!(
            "{n:<12} range [{:+.4}, {:+.4}]   smallest non-zero |v| = {floor}",
            ext[i][0], ext[i][1]
        );
    }
    println!("buttons seen: {seen_buttons:?}");

    // Resting values: the D6 stage-1 calibration number. Only meaningful in --bevy/--unfiltered,
    // since gilrs's default deadzone snaps a resting stick to exactly 0.0.
    println!("\nresting stick values (hands off the pad):");
    if mode == Mode::GilrsDefault {
        println!("  (meaningless in this mode — gilrs's default deadzone zeroes them; use --bevy)");
    }
    std::thread::sleep(Duration::from_millis(400));
    while gilrs.next_event().is_some() {}
    for (id, pad) in gilrs.gamepads() {
        for (a, n) in AXES.iter().take(4) {
            if let Some(d) = pad.axis_data(*a) {
                println!("[{id}] {n:<12} = {:+.4}", d.value());
            }
        }
    }
}

const AXES: &[(Axis, &str)] = &[
    (Axis::LeftStickX, "LeftStickX"),
    (Axis::LeftStickY, "LeftStickY"),
    (Axis::RightStickX, "RightStickX"),
    (Axis::RightStickY, "RightStickY"),
    (Axis::LeftZ, "LeftZ"),
    (Axis::RightZ, "RightZ"),
    (Axis::DPadX, "DPadX"),
    (Axis::DPadY, "DPadY"),
];

const BUTTONS: &[(Button, &str)] = &[
    (Button::South, "South"),
    (Button::East, "East"),
    (Button::North, "North"),
    (Button::West, "West"),
    (Button::LeftTrigger, "L(bumper)"),
    (Button::LeftTrigger2, "LT(analog)"),
    (Button::RightTrigger, "R(bumper)"),
    (Button::RightTrigger2, "RT(analog)"),
    (Button::Select, "Select"),
    (Button::Start, "Start"),
    (Button::Mode, "Mode"),
    (Button::LeftThumb, "LeftThumb"),
    (Button::RightThumb, "RightThumb"),
    (Button::DPadUp, "DPadUp"),
    (Button::DPadDown, "DPadDown"),
    (Button::DPadLeft, "DPadLeft"),
    (Button::DPadRight, "DPadRight"),
];

fn stick_index(axis: Axis) -> Option<usize> {
    match axis {
        Axis::LeftStickX => Some(0),
        Axis::LeftStickY => Some(1),
        Axis::RightStickX => Some(2),
        Axis::RightStickY => Some(3),
        _ => None,
    }
}

fn uuid_str(b: [u8; 16]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
