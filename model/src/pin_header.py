from build123d import *

HEADER_PITCH = 2.54
HEADER_PIN_SIZE = 0.64
HEADER_PIN_LENGTH = 11.0
HEADER_BODY_THICK = 2.5
HEADER_BODY_COLOR = Color("black")
HEADER_PIN_COLOR = Color(0xd4af37)


def build_pin_header(rows: int, cols: int) -> Compound:
    body_w = rows * HEADER_PITCH
    body_l = cols * HEADER_PITCH
    parts = []

    with BuildPart() as body:
        Box(body_w, body_l, HEADER_BODY_THICK)
    body.part.color = HEADER_BODY_COLOR
    body.part.label = "Body"
    parts.append(body.part)

    for r in range(rows):
        for c in range(cols):
            with BuildPart() as pin:
                Box(HEADER_PIN_SIZE, HEADER_PIN_SIZE, HEADER_PIN_LENGTH)
            pin.part.color = HEADER_PIN_COLOR
            pin.part.label = "Pin"
            px = (r - (rows - 1) / 2) * HEADER_PITCH
            py = (c - (cols - 1) / 2) * HEADER_PITCH
            pin.part.move(Pos(px, py, 0))
            parts.append(pin.part)

    return Compound(children=parts, label=f"PinHeader_{rows}x{cols}")
