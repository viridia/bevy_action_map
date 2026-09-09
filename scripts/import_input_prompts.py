#!/usr/bin/env python3
"""Pick this crate's icons out of Kenney's Input Prompts pack and write the manifest.

Run with --apply to copy; without it, reports what it would do. The pack is not vendored, so
re-running needs it downloaded to SRC below.

Two things this encodes that are easy to get wrong by hand. Face buttons are named by *position*,
so Bevy's `South` is the bottom button whatever letter is printed on it — Xbox A, PlayStation
Cross, and Switch B, because Nintendo's letters sit where Xbox's do not. And `_color` art exists
only for the eight face buttons, so "colored, not outlined" means: prefer `_color` where there is
one, take the plain form otherwise, never the `_outline`.

The destination path is the control's own stored name under a tier directory, so an app resolves
art with `format!("input_prompts/{tier}/{}.png", control.name())` and no lookup table.
"""

import os, shutil, sys

SRC = os.path.expanduser("~/Downloads/kenney_input-prompts_1.5")
DST = "assets/input_prompts"

# Kenney folder per tier.
PACK = {
    "xbox": "Xbox Series",
    "playstation": "PlayStation Series",
    "nintendo": "Nintendo Switch",
    "generic": "Generic",
    "keyboard_mouse": "Keyboard & Mouse",
}

# Face buttons are by POSITION, not letter. Bevy's South is the bottom button:
# Xbox A, PlayStation Cross, Switch B. Nintendo's letters are swapped throughout.
PAD = {
    "xbox": {
        "South": "xbox_button_color_a", "East": "xbox_button_color_b",
        "North": "xbox_button_color_y", "West": "xbox_button_color_x",
        "LeftTrigger": "xbox_lb", "RightTrigger": "xbox_rb",
        "LeftTrigger2": "xbox_lt", "RightTrigger2": "xbox_rt",
        "Select": "xbox_button_view", "Start": "xbox_button_menu", "Mode": "xbox_guide",
        "LeftThumb": "xbox_stick_l_press", "RightThumb": "xbox_stick_r_press",
        "DPadUp": "xbox_dpad_round_up", "DPadDown": "xbox_dpad_round_down",
        "DPadLeft": "xbox_dpad_round_left", "DPadRight": "xbox_dpad_round_right",
    },
    "playstation": {
        "South": "playstation_button_color_cross", "East": "playstation_button_color_circle",
        "North": "playstation_button_color_triangle", "West": "playstation_button_color_square",
        "LeftTrigger": "playstation_trigger_l1", "RightTrigger": "playstation_trigger_r1",
        "LeftTrigger2": "playstation_trigger_l2", "RightTrigger2": "playstation_trigger_r2",
        "Select": "playstation5_button_create", "Start": "playstation5_button_options",
        "LeftThumb": "playstation_button_l3", "RightThumb": "playstation_button_r3",
        "DPadUp": "playstation_dpad_up", "DPadDown": "playstation_dpad_down",
        "DPadLeft": "playstation_dpad_left", "DPadRight": "playstation_dpad_right",
    },
    "nintendo": {
        "South": "switch_button_b", "East": "switch_button_a",
        "North": "switch_button_x", "West": "switch_button_y",
        "LeftTrigger": "switch_button_l", "RightTrigger": "switch_button_r",
        "LeftTrigger2": "switch_button_zl", "RightTrigger2": "switch_button_zr",
        "Select": "switch_button_minus", "Start": "switch_button_plus",
        "Mode": "switch_button_home",
        "LeftThumb": "switch_stick_l_press", "RightThumb": "switch_stick_r_press",
        "DPadUp": "switch_dpad_up", "DPadDown": "switch_dpad_down",
        "DPadLeft": "switch_dpad_left", "DPadRight": "switch_dpad_right",
    },
}

STICK = {
    "xbox": {"Left": "xbox_stick_l", "Right": "xbox_stick_r"},
    "playstation": {"Left": "playstation_stick_l", "Right": "playstation_stick_r"},
    "nintendo": {"Left": "switch_stick_l", "Right": "switch_stick_r"},
    "generic": {"Left": "generic_stick", "Right": "generic_stick"},
}

AXIS = {
    "xbox": {"LeftStickX": "xbox_stick_l_horizontal", "LeftStickY": "xbox_stick_l_vertical",
             "RightStickX": "xbox_stick_r_horizontal", "RightStickY": "xbox_stick_r_vertical"},
    "playstation": {"LeftStickX": "playstation_stick_l_horizontal", "LeftStickY": "playstation_stick_l_vertical",
                    "RightStickX": "playstation_stick_r_horizontal", "RightStickY": "playstation_stick_r_vertical"},
    "nintendo": {"LeftStickX": "switch_stick_l_horizontal", "LeftStickY": "switch_stick_l_vertical",
                 "RightStickX": "switch_stick_r_horizontal", "RightStickY": "switch_stick_r_vertical"},
    "generic": {"LeftStickX": "generic_stick_horizontal", "LeftStickY": "generic_stick_vertical",
                "RightStickX": "generic_stick_horizontal", "RightStickY": "generic_stick_vertical"},
}

KEY = {}
for c in "abcdefghijklmnopqrstuvwxyz":
    KEY["Key" + c.upper()] = "keyboard_" + c
for d in range(10):
    KEY["Digit%d" % d] = "keyboard_%d" % d
for f in range(1, 13):
    KEY["F%d" % f] = "keyboard_f%d" % f
KEY.update({
    "Space": "keyboard_space", "Enter": "keyboard_enter", "Escape": "keyboard_escape",
    "Tab": "keyboard_tab", "Backspace": "keyboard_backspace", "Delete": "keyboard_delete",
    "Insert": "keyboard_insert", "Home": "keyboard_home", "End": "keyboard_end",
    "PageUp": "keyboard_page_up", "PageDown": "keyboard_page_down",
    "ArrowUp": "keyboard_arrow_up", "ArrowDown": "keyboard_arrow_down",
    "ArrowLeft": "keyboard_arrow_left", "ArrowRight": "keyboard_arrow_right",
    "ShiftLeft": "keyboard_shift", "ShiftRight": "keyboard_shift",
    "ControlLeft": "keyboard_ctrl", "ControlRight": "keyboard_ctrl",
    "AltLeft": "keyboard_alt", "AltRight": "keyboard_option",
    "SuperLeft": "keyboard_command", "SuperRight": "keyboard_win",
    "Minus": "keyboard_minus", "Equal": "keyboard_equals",
    "BracketLeft": "keyboard_bracket_open", "BracketRight": "keyboard_bracket_close",
    "Backslash": "keyboard_slash_back", "Slash": "keyboard_slash_forward",
    "Semicolon": "keyboard_semicolon", "Quote": "keyboard_quote",
    "Comma": "keyboard_comma", "Period": "keyboard_period", "Backquote": "keyboard_tilde",
    "NumpadEnter": "keyboard_numpad_enter", "NumpadAdd": "keyboard_numpad_plus",
})

MOUSE = {"Left": "mouse_left", "Right": "mouse_right", "Middle": "mouse_scroll",
         "Back": "mouse_side_back", "Forward": "mouse_side_forward", "motion": "mouse_move"}

def plan():
    jobs = []
    for tier, table in PAD.items():
        for name, f in table.items():
            jobs.append((tier, "pad/%s" % name, f))
    for tier, table in STICK.items():
        for name, f in table.items():
            jobs.append((tier, "stick/%s" % name, f))
    for tier, table in AXIS.items():
        for name, f in table.items():
            jobs.append((tier, "axis/%s" % name, f))
    for name, f in KEY.items():
        jobs.append(("keyboard_mouse", "key/%s" % name, f))
    for name, f in MOUSE.items():
        jobs.append(("keyboard_mouse", "mouse/%s" % name, f))
    return jobs

jobs = plan()
missing, copied = [], 0
for tier, control, stem in jobs:
    src = os.path.join(SRC, PACK[tier], "Default", stem + ".png")
    if not os.path.exists(src):
        missing.append((tier, control, stem)); continue
    dst = os.path.join(DST, tier, control + ".png")
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    if "--apply" in sys.argv:
        shutil.copy2(src, dst)
    copied += 1

print("planned %d, resolvable %d, missing %d" % (len(jobs), copied, len(missing)))
for m in missing:
    print("  MISSING", m)

if "--apply" in sys.argv:
    entries = sorted(
        "%s/%s" % (tier, control)
        for tier, control, stem in jobs
        if os.path.exists(os.path.join(SRC, PACK[tier], "Default", stem + ".png"))
    )
    with open(os.path.join(DST, "manifest.txt"), "w") as f:
        f.write("# Which (tier, control) pairs have art, generated by scripts/import_input_prompts.py.\n")
        f.write("# Resolution reads this rather than probing for a file: a pair absent here steps to\n")
        f.write("# the next tier, and a missing path is never a load error.\n")
        for e in entries:
            f.write(e + "\n")
    print("wrote %s/manifest.txt with %d entries" % (DST, len(entries)))
