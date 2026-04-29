"""RP Touch development board 3D model.

All dimensions are in millimetres.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from build123d import Compound, export_gltf, export_step
from ocp_vscode import show

import pcb
import screen

MODEL_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT_BASE = MODEL_ROOT / "build" / "rp_touch"


def build_model(with_sockets: bool = False) -> Compound:
    housing, screen_subs = screen.build_screen()
    pcb_asm = pcb.build_pcb(with_sockets=with_sockets)

    pcb_board = pcb_asm.children[0]
    pcb_board.joints["top"].connect_to(housing.joints["bottom"])

    bezel, display = screen.position_screen_children(housing, *screen_subs)
    screen_asm = Compound(children=[housing, bezel, display], label="Screen")

    return Compound(children=[screen_asm, pcb_asm], label="RP Touch")


def resolve_output_base(raw: str | None) -> Path:
    if raw is None:
        return DEFAULT_OUTPUT_BASE

    output_base = Path(raw)
    if not output_base.is_absolute():
        output_base = MODEL_ROOT / output_base
    return output_base


def export_model(model: Compound, output_base: Path) -> Path:
    output_base.parent.mkdir(parents=True, exist_ok=True)
    export_gltf(model, output_base.as_posix())
    return output_base


def export_to_build(model: Compound) -> Path:
    return export_model(model, DEFAULT_OUTPUT_BASE)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build and export RP Touch model")
    parser.add_argument(
        "--out",
        type=str,
        default=None,
        help="Output base path for glTF + STEP export. Default: model/build/rp_touch",
    )
    parser.add_argument(
        "--show",
        action="store_true",
        help="Open interactive viewer after export",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    output_base = resolve_output_base(args.out)

    model = build_model()
    export_model(model, output_base)

    kicad_model = build_model(with_sockets=True)
    export_step(kicad_model, output_base.with_suffix(".step").as_posix())

    if args.show:
        show(model)


if __name__ == "__main__":
    main()
