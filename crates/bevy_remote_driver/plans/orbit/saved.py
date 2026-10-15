"""What `editor.py` left in the settings file comes back after a restart. Run it after that plan.

Ends on Reset, which puts the developer's own saved controls back to the defaults.
"""

EXAMPLE = "orbit"
FEATURES = ["serialize"]

TEXT = "bevy_ui::widget::text::Text"
ROW = "Editor/orbit.zoom_in"


def run(driver):
    driver.present("Shell", ready=True)
    driver.click(f"{ROW}/title")
    driver.expect(f"{ROW}/0/label", TEXT, "E")
    driver.expect(f"{ROW}/1/label", TEXT, "F")

    driver.click("Shell/Reset")
    driver.absent(f"{ROW}/1")
