"""Rebinding a row: what the screen shows, what Confirm commits, and what survives a reopen.

Disasteroids saves a confirmed rebind, so the file outlives the run and the second run of a plan
that leaves one behind starts somewhere else. Reset and Confirm at both ends is this plan reaching
a known state the way a player would.
"""

EXAMPLE = "disasteroids"
FEATURES = ["serialize"]

TEXT = "bevy_ui::widget::text::Text"
# Thrust's primary keyboard cell: the family names the table, the row's mapping key names the row,
# and the slot names the cell in it.
THRUST = "Settings/KeyboardMouse/disasteroids.thrust/0"


def run(driver):
    open_screen(driver)
    reset(driver)
    open_screen(driver)
    driver.screenshot("settings")

    driver.expect(THRUST, TEXT, "W")
    driver.click(THRUST)
    driver.key("KeyK")
    # The working copy, which is all a capture writes. Nothing has reached the game yet.
    driver.expect(THRUST, TEXT, "K")

    confirm(driver)
    open_screen(driver)
    # And now what Confirm committed, read back off a screen built from the mapping list again.
    driver.expect(THRUST, TEXT, "K")
    driver.screenshot("rebound")

    # Leaves the developer's own saved controls as this found them.
    reset(driver)


def open_screen(driver):
    """Opens the controls screen. `Shell` binds F2, so it answers whatever the game is doing."""
    driver.key("F2")
    driver.present("Settings", ready=True)


def confirm(driver):
    """Commits the working copy and leaves, which is its only path into the running game."""
    driver.click("Settings/Confirm")
    driver.absent("Settings")


def reset(driver):
    """Puts every row, tunable and preset back to what the game declares, and commits that."""
    driver.click("Settings/Reset")
    confirm(driver)
