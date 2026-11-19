"""Generates 16x16x16 sample blocks from the bundled materials and renders them
into one sheet.

    cargo build --release
    python examples/blocks.py --out out/blocks

Writes one `.mcvox` and one isometric PNG per block, plus `blocks.png`, the
sheet shown in the README. Needs Pillow for the sheet.
"""

import argparse
import random
import subprocess
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
N = 16


def write_block(path: Path, palette: dict[str, str], voxel) -> None:
    """Writes a solid N³ block; `voxel(x, y, z)` returns its palette character."""
    lines = [
        "schema = 1",
        f'name   = "demo:{path.stem.replace("-", "_")}"',
        "scale  = 16",
        f"size   = [{N}, {N}, {N}]",
        'slice  = "y"',
        "",
        "[palette]",
    ]
    lines += [f'"{char}" = "{material}"' for char, material in palette.items()]
    for y in range(N):
        rows = ["".join(voxel(x, y, z) for x in range(N)) for z in range(N)]
        lines += ["", "[[layers]]", f"y = {y}", 'grid = """', *rows, '"""']
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def speckle(seed: int, weights: list[tuple[str, int]]):
    """Per-voxel weighted noise, stable for a given seed."""
    rng = random.Random(seed)
    chars, counts = zip(*weights)
    cache: dict[tuple[int, int, int], str] = {}

    def voxel(x, y, z):
        if (x, y, z) not in cache:
            cache[(x, y, z)] = rng.choices(chars, counts)[0]
        return cache[(x, y, z)]

    return voxel


def on_rim(x, y, z, width=1) -> bool:
    """Whether the voxel lies on the outer ring of the face it belongs to."""
    return sum(v < width or v >= N - width for v in (x, y, z)) >= 2


def face_coords(x, y, z) -> list[int]:
    """The two in-plane coordinates of a voxel on exactly one face."""
    return [v for v in (x, y, z) if v not in (0, N - 1)]


def planks(x, y, z):
    board = y // 4
    if y % 4 == 3 or (x + board * 5) % N == 0 or (z + board * 5) % N == 0:
        return "b"
    return "p" if random.Random(x * 131 + y * 17 + z).random() > 0.12 else "b"


def log(x, y, z):
    if y in (0, N - 1):
        ring = max(abs(x - 7.5), abs(z - 7.5))
        return "b" if ring > 6.5 or int(ring) % 3 == 0 else "p"
    grain = (x + z + y // 3) % 4 == 0
    return "p" if grain and random.Random(x * 7 + y * 977 + z * 31).random() > 0.5 else "b"


def copper(x, y, z):
    if on_rim(x, y, z):
        return "e"
    return "c" if random.Random(x * 3 + y * 101 + z * 7).random() > 0.08 else "e"


def iron(x, y, z):
    face = face_coords(x, y, z)
    if on_rim(x, y, z) or (len(face) == 2 and all(v in (3, 12) for v in face)):
        return "e"
    return "i"


def royal(x, y, z):
    if on_rim(x, y, z, 2):
        return "g" if on_rim(x, y, z) else "k"
    face = face_coords(x, y, z)
    if len(face) == 2 and abs(face[0] - 7.5) + abs(face[1] - 7.5) <= 3:
        return "g"
    return "b"


def lamp(x, y, z):
    face = face_coords(x, y, z)
    if on_rim(x, y, z, 2) or (len(face) == 2 and any(v in (7, 8) for v in face)):
        return "s"
    return "a"


def neon(x, y, z):
    if y in (4, 11):
        return "c"
    return "m" if on_rim(x, y, z) else "s"


def portal(x, y, z):
    if on_rim(x, y, z):
        return "o"
    return random.Random(x * 13 + y * 29 + z * 43).choices("dgcf", [5, 4, 2, 1])[0]


GENERATED = {
    "dirt": ({"d": "base:dirt", "k": "base:dirt_dark", "l": "base:dirt_light"},
             speckle(1, [("d", 6), ("k", 2), ("l", 2)])),
    "oak-planks": ({"p": "base:oak_plank", "b": "base:oak_beam"}, planks),
    "oak-log": ({"p": "base:oak_plank", "b": "base:oak_beam"}, log),
    "obsidian": ({"o": "base:obsidian", "v": "base:obsidian_violet"},
                 speckle(2, [("o", 10), ("v", 1)])),
    "copper-plate": ({"c": "base:copper_plate", "e": "base:iron_edge"}, copper),
    "iron-block": ({"i": "base:iron", "e": "base:iron_edge"}, iron),
    "royal-block": ({"b": "base:royal_blue", "k": "base:royal_blue_dark", "g": "base:royal_gold"}, royal),
    "amber-lamp": ({"s": "base:steel_dark", "a": "base:amber_lamp"}, lamp),
    "neon-panel": ({"s": "base:steel_dark", "c": "base:neon_cyan", "m": "base:neon_magenta"}, neon),
    "portal-block": ({"o": "base:obsidian", "d": "base:portal_deep", "g": "base:portal_glow",
                      "c": "base:portal_core", "f": "base:portal_flare"}, portal),
}

# Hand-authored models shipped beside this script, placed among the generated ones.
AUTHORED = {"grass-block", "stone-block"}

ORDER = ["grass-block", "dirt", "stone-block", "oak-planks", "oak-log", "obsidian",
         "copper-plate", "iron-block", "royal-block", "amber-lamp", "neon-panel", "portal-block"]


def render(voxforge: Path, model: Path, png: Path, materials: Path) -> None:
    subprocess.run([str(voxforge), "preview", str(model), "--view", "iso-fl",
                    "--pixels-per-voxel", "8", "--materials", str(materials), "--out", str(png)],
                   check=True, stdout=subprocess.DEVNULL)


def sheet(pngs: list[Path], out: Path, columns=6, gap=28) -> None:
    images = [Image.open(p).convert("RGBA") for p in pngs]
    images = [im.crop(im.getbbox()) for im in images]
    w = max(im.width for im in images)
    h = max(im.height for im in images)
    rows = -(-len(images) // columns)
    canvas = Image.new("RGBA", (columns * w + (columns + 1) * gap, rows * h + (rows + 1) * gap))
    for i, im in enumerate(images):
        r, c = divmod(i, columns)
        canvas.paste(im, (gap + c * (w + gap) + (w - im.width) // 2, gap + r * (h + gap) + h - im.height), im)
    canvas.save(out, optimize=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", type=Path, default=Path("out/blocks"))
    parser.add_argument("--voxforge", type=Path, default=ROOT / "target/release/voxforge")
    parser.add_argument("--materials", type=Path, default=ROOT / "assets/materials")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)

    pngs = []
    for name in ORDER:
        if name in AUTHORED:
            model = ROOT / "examples/models" / f"{name}.mcvox"
        else:
            model = args.out / f"{name}.mcvox"
            write_block(model, *GENERATED[name])
        png = args.out / f"{name}.png"
        render(args.voxforge, model, png, args.materials)
        pngs.append(png)
    sheet(pngs, args.out / "blocks.png")
    print(args.out / "blocks.png")


if __name__ == "__main__":
    main()
