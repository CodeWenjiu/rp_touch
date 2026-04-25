from build123d import *
from ocp_vscode import show

with BuildPart() as test_part:
    Box(20, 20, 10)
    with BuildSketch(test_part.faces().sort_by(Axis.Z)[-1]):
        Circle(radius=5)
    extrude(amount=-10, mode=Mode.SUBTRACT)

show(test_part)