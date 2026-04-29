from build123d import *

import pin_header

PCB_WIDTH = 28.6
PCB_LENGTH = 43.5
PCB_THICKNESS = 1.6
PCB_FILLET_R = 2.5

STANDOFF_H = 4.0
STANDOFF_SPAN_X = 22.0
STANDOFF_SPAN_Y = 38.5

PINHEADER_SPAN_X = 22.86
PINHEADER_BOTTOM_PIN_TO_STANDOFF_Y = 9.33

PCB_COLOR = Color(0x0b2a5a)
STANDOFF_COLOR = Color(0xcd7f32)


def build_pcb(with_sockets: bool = False) -> Compound:
    with BuildPart() as pcb:
        Box(PCB_WIDTH, PCB_LENGTH, PCB_THICKNESS)
        fillet(
            pcb.edges().filter_by(Axis.Z).group_by(Axis.Z)[0],
            radius=PCB_FILLET_R,
        )
    pcb.part.color = PCB_COLOR
    pcb.part.label = "PCB"

    RigidJoint("top", pcb.part, Location((0, 0, PCB_THICKNESS / 2)))

    pcb_bottom_z = pcb.part.bounding_box().min.Z
    standoffs = []
    hx = STANDOFF_SPAN_X / 2
    hy = STANDOFF_SPAN_Y / 2
    for x, y in [(hx, hy), (-hx, hy), (hx, -hy), (-hx, -hy)]:
        with BuildPart() as s:
            with BuildSketch(Plane.XY):
                RegularPolygon(radius=1.5, side_count=6)
            extrude(amount=STANDOFF_H)
        s.part.color = STANDOFF_COLOR
        s.part.label = "Standoff"
        s_top_z = s.part.bounding_box().max.Z
        s.part.move(Pos(x, y, pcb_bottom_z - s_top_z))
        standoffs.append(s.part)

    ph_dx = PINHEADER_SPAN_X / 2
    ph_c_y = -STANDOFF_SPAN_Y / 2 + PINHEADER_BOTTOM_PIN_TO_STANDOFF_Y + pin_header.HEADER_PITCH * 5
    base_half = pin_header.HEADER_BODY_THICK / 2
    pin_header_compounds = []
    for x in [ph_dx, -ph_dx]:
        ph = pin_header.build_pin_header(1, 11)
        ph.move(Pos(x, ph_c_y, pcb_bottom_z - base_half))
        pin_header_compounds.append(ph)

    parts = [pcb.part] + standoffs + pin_header_compounds

    if with_sockets:
        socket_z = pcb_bottom_z - pin_header.HEADER_BODY_THICK - pin_header.SOCKET_HEIGHT / 2
        for x in [ph_dx, -ph_dx]:
            socket = pin_header.build_pin_socket(1, 11)
            socket.move(Pos(x, ph_c_y, socket_z))
            parts.append(socket)

    return Compound(children=parts, label="PCB Assembly")
