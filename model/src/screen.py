from build123d import *

SCREEN_WIDTH = 29.12
SCREEN_LENGTH = 44.00
SCREEN_THICKNESS = 4.00
SCREEN_FILLET_R = 3.7
TOUCH_PANEL_WIDTH = 26.74
TOUCH_PANEL_LENGTH = 41.62
TOUCH_PANEL_FILLET_R = 2.50
TOUCH_PANEL_DEPTH = 0.3
DISP_WIDTH = 22.34
DISP_LENGTH = 36.06
DISP_FILLET_R = 2.50

HOUSING_COLOR = Color(0x111111)
BEZEL_COLOR = Color("black")
LCD_COLOR = Color(0x0a1010)


def build_screen() -> Compound:
    with BuildPart() as housing:
        Box(SCREEN_WIDTH, SCREEN_LENGTH, SCREEN_THICKNESS)
        fillet(
            housing.edges().filter_by(Axis.Z).group_by(Axis.Z)[0],
            radius=SCREEN_FILLET_R,
        )
        with BuildSketch(housing.faces().sort_by(Axis.Z)[-1]):
            RectangleRounded(
                TOUCH_PANEL_WIDTH,
                TOUCH_PANEL_LENGTH,
                radius=TOUCH_PANEL_FILLET_R,
            )
        extrude(amount=-TOUCH_PANEL_DEPTH, mode=Mode.SUBTRACT)
    housing.part.color = HOUSING_COLOR
    housing.part.label = "Housing"

    # Step 2: Joint on housing bottom face
    RigidJoint(
        "bottom",
        housing.part,
        Location((0, 0, -SCREEN_THICKNESS / 2)),
    )

    with BuildPart() as bezel:
        Box(TOUCH_PANEL_WIDTH, TOUCH_PANEL_LENGTH, 0.05)
        fillet(
            bezel.edges().filter_by(Axis.Z).group_by(Axis.Z)[0],
            radius=TOUCH_PANEL_FILLET_R,
        )
    bezel.part.color = BEZEL_COLOR
    bezel.part.label = "Bezel"

    with BuildPart() as display:
        Box(DISP_WIDTH, DISP_LENGTH, 0.1)
        fillet(
            display.edges().filter_by(Axis.Z).group_by(Axis.Z)[0],
            radius=DISP_FILLET_R,
        )
    display.part.color = LCD_COLOR
    display.part.label = "Display"

    return housing.part, [bezel.part, display.part]


def position_screen_children(
    housing: Part, bezel: Part, display: Part
) -> tuple[Part, Part]:
    bb = housing.bounding_box()
    recess_z = bb.max.Z - TOUCH_PANEL_DEPTH
    bezel.move(Pos(0, 0, recess_z + 0.025))
    display.move(Pos(0, 0, recess_z + 0.075))
    return bezel, display
