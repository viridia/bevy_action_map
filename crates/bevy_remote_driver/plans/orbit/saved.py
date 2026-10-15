"""What `editor.py` left in the settings file comes back after a restart, modifier included. Run it
after that plan.

Ends on Reset, which puts the developer's own saved controls back to the defaults.
"""

EXAMPLE = "orbit"
FEATURES = ["serialize"]

TEXT = "bevy_ui::widget::text::Text"
VARIANT = "bevy_feathers::controls::button::ButtonVariant"
ROW = "Editor/orbit.zoom_in"


def run(driver):
    driver.present("Shell", ready=True)
    driver.click(f"{ROW}/title")
    driver.expect(f"{ROW}/0/label", TEXT, "E")
    driver.expect(f"{ROW}/1/label", TEXT, "H")
    driver.expect(f"{ROW}/Ctrl 1", VARIANT, "Primary")
    driver.expect(f"{ROW}/Shift 1", VARIANT, "Normal")

    driver.click("Shell/Reset")
    driver.absent(f"{ROW}/1")
