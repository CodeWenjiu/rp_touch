"""RP Touch — development board 3D model.

All dimensions in millimetres.
"""

from build123d import *
from ocp_vscode import show

import screen
import pcb

# 1. Build parts at origin
housing, screen_subs = screen.build_screen()
pcb_asm = pcb.build_pcb()

# 2. Connect: screen housing bottom → PCB top face
#    PCB stays in place; housing moves to align
pcb_board = pcb_asm.children[0]
pcb_board.joints["top"].connect_to(housing.joints["bottom"])

# 3. Position screen sub-parts relative to the moved housing
bezel, display = screen.position_screen_children(housing, *screen_subs)

# 4. Wrap into Compounds
screen_asm = Compound(children=[housing, bezel, display])

show([screen_asm, pcb_asm])
