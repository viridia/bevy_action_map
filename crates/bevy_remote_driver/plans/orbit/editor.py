"""The controls panel: a row's list grows, takes keys, refuses a reserved one, and closes up when a
key in the middle is removed.

Leaves Zoom In bound to E and F, which `saved.py` reads back after a restart and then resets. Run
this one first.
"""

EXAMPLE = "orbit"
FEATURES = ["serialize"]

TEXT = "bevy_ui::widget::text::Text"
ROW = "Editor/orbit.zoom_in"


def run(driver):
    driver.present("Shell", ready=True)
    driver.click("Shell/Reset")
    driver.click(f"{ROW}/title")
    driver.expect(f"{ROW}/0/label", TEXT, "E")
    driver.absent(f"{ROW}/1")

    add(driver, 1, "KeyR", "R")
    add(driver, 2, "KeyF", "F")
    driver.screenshot("three-keys")

    # The middle one goes, and the one after it moves up.
    driver.click(f"{ROW}/remove 1")
    driver.expect(f"{ROW}/1/label", TEXT, "F")
    driver.absent(f"{ROW}/2")

    # Escape is how the mouse comes back, so a capture hears it and the row turns it down.
    driver.click(f"{ROW}/add")
    driver.key("Escape")
    driver.expect("Shell/Status", TEXT, "Esc gives the mouse back, so it stays as it is")
    driver.present(f"{ROW}/add")
    driver.absent(f"{ROW}/2")
    driver.screenshot("refused")


def add(driver, slot, code, label):
    """Grows the row to `slot`, and fills it by pressing `code`."""
    driver.click(f"{ROW}/add")
    driver.expect(f"{ROW}/{slot}/label", TEXT, "Press a key")
    driver.key(code)
    driver.expect(f"{ROW}/{slot}/label", TEXT, label)
