"""Driving Disasteroids from a gamepad: a button opens the controls screen, the stick moves the
selection through it, and two more buttons act on what the selection is on.

The bottom row — Cancel, Reset, Confirm — is where a stick is legible: each of the three has an id,
and the focus ring says which one the stick reached. Reset writes the screen's working copy only,
so cancelling out of it leaves the developer's saved controls as this found them.
"""

EXAMPLE = "disasteroids"
FEATURES = ["serialize"]

OUTLINE = "bevy_ui::ui_node::Outline"
TEXT = "bevy_ui::widget::text::Text"
# The ring is an `Outline` on every focusable, `Color::NONE` until focus arrives. None is a
# `LinearRgba` and the lit colour an `Srgba`, so which variant answers is already the answer.
RING = {"color": {"Srgba": {"alpha": 1.0}}}
THRUST = "Settings/KeyboardMouse/disasteroids.thrust/0"


def run(driver):
    # North opens the controls screen, whatever the game is doing.
    driver.pad("North")
    driver.present("Settings", ready=True)
    driver.expect("Settings/Cancel", OUTLINE, RING)

    # Right on the left stick. The screen reads a stick as a position rounded to one of four
    # compass points, repeating while it is held, so one push and one release is one move.
    driver.pad("LeftStickX", 1.0)
    driver.pad("LeftStickX", 0.0)
    driver.expect("Settings/Reset", OUTLINE, RING)
    driver.screenshot("reset-selected")

    # South presses what the selection is on, which is how the stick's move shows as more than a
    # ring: Reset puts every row back to what the game declares.
    driver.pad("South")
    driver.expect(THRUST, TEXT, "W")

    # East backs out without committing, so nothing here is written to disk.
    driver.pad("East")
    driver.absent("Settings")
