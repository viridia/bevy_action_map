"""Who has the mouse: a click on the view hands it to the camera, and Escape hands it back.

The keys orbit whoever has the mouse, and the screenshots show the camera moved. A grabbed cursor
locks the real one too, so the plan ends with the mouse given back.
"""

EXAMPLE = "orbit"

TEXT = "bevy_ui::widget::text::Text"
STATUS = "Shell/Status"
FREE = "Click the view to orbit with the mouse"
GRABBED = "Escape gives the mouse back"


def run(driver):
    driver.present("Shell", ready=True)
    driver.expect(STATUS, TEXT, FREE)
    driver.screenshot("start")

    driver.press("ArrowRight")
    driver.frames(30)
    driver.release("ArrowRight")
    driver.screenshot("orbited")

    driver.click("Shell/View")
    driver.expect(STATUS, TEXT, GRABBED)
    # Held keys still orbit while the camera has the mouse; they belong to the other context.
    driver.press("ArrowUp")
    driver.frames(20)
    driver.release("ArrowUp")
    driver.screenshot("grabbed")

    driver.key("Escape")
    driver.expect(STATUS, TEXT, FREE)
