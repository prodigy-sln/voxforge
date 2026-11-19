"""Animates the example fox into a looping run cycle GIF.

    cargo build --release
    python examples/fox_run.py --out out/fox

Every frame is a generated `.mcvox`: the fox's legs swing on a gallop, its body
bobs, its tail sways, and a strip of grass scrolls underneath. VoxForge renders
each frame, and Pillow assembles them into `fox-run.gif`.
"""

import argparse
import math
import subprocess
import tomllib
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
FRAMES = 8
# The fox inside the padded box, before it is turned to run along +x.
WIDTH, LENGTH, HEIGHT = 11, 24, 15
OFFSET_X, OFFSET_Z = 1, 4
GROUND = {"g": "base:grass", "G": "base:grass_dark", "l": "base:grass_light", "d": "base:dirt"}


def load_voxels(path: Path) -> tuple[dict[str, str], dict[tuple[int, int, int], str]]:
    """The palette and filled voxels of a single-part, `y`-sliced document."""
    doc = tomllib.loads(path.read_text(encoding="utf-8"))
    voxels = {}
    for layer in doc["layers"]:
        for z, row in enumerate(layer["grid"].strip("\n").split("\n")):
            for x, char in enumerate(row):
                if doc["palette"].get(char, "empty") != "empty":
                    voxels[(x, layer["y"], z)] = char
    return doc["palette"], voxels


def grass(x: int, z: int, frame: int) -> str:
    shade = (x * 7 + (z + frame * 3) * 13) % 11
    return "G" if shade < 3 else "l" if shade > 8 else "g"


def frame_voxels(fox, frame: int) -> dict[tuple[int, int, int], str]:
    legs = {k: v for k, v in fox.items() if k[1] <= 2}
    tail = {k: v for k, v in fox.items() if k[1] >= 6 and k[2] <= 2}
    body = {k: v for k, v in fox.items() if k not in legs and k not in tail}
    phase = 2 * math.pi * frame / FRAMES

    grid = {}
    for x in range(WIDTH):
        for z in range(LENGTH):
            grid[(x, 0, z)] = "d"
            grid[(x, 1, z)] = grass(x, z, frame)

    base = 2 + (1 if math.sin(phase) > 0.3 else 0)
    for (x, y, z), char in body.items():
        grid[(x + OFFSET_X, y + base, z + OFFSET_Z)] = char

    # The tail only dips: rising would lift its root off the body.
    sway = round(math.sin(phase + 1.0))
    dip = -1 if math.cos(phase) < -0.3 else 0
    for (x, y, z), char in tail.items():
        grid[(x + OFFSET_X + sway, y + base + dip, z + OFFSET_Z)] = char

    # The hip stays put and each voxel below steps one further, so a reaching
    # leg turns diagonal while its voxels keep sharing faces.
    for (x, y, z), char in legs.items():
        front = z >= 8
        reach = round(2 * math.sin(phase + (0 if front else math.pi)))
        step = round(reach * (2 - y) / 2)
        grid[(x + OFFSET_X, y + base, z + OFFSET_Z + step)] = char

    # Turn the fox to run along +x, so an isometric view shows its side.
    return {(z, y, WIDTH - 1 - x): char for (x, y, z), char in grid.items()}


def write_frame(path: Path, palette: dict[str, str], grid) -> None:
    lines = ["schema = 1", f'name   = "demo:{path.stem}"', "scale  = 16",
             f"size   = [{LENGTH}, {HEIGHT}, {WIDTH}]", 'slice  = "y"', "", "[palette]",
             '"." = "empty"']
    lines += [f'"{c}" = "{m}"' for c, m in {**palette, **GROUND}.items() if m != "empty"]
    for y in range(HEIGHT):
        rows = ["".join(grid.get((x, y, z), ".") for x in range(LENGTH)) for z in range(WIDTH)]
        lines += ["", "[[layers]]", f"y = {y}", 'grid = """', *rows, '"""']
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def gif(pngs: list[Path], out: Path, duration_ms=90) -> None:
    """Aligns frames on their bottom edge (the ground) and writes a transparent GIF."""
    frames = [Image.open(p).convert("RGBA") for p in pngs]
    w = max(f.width for f in frames)
    h = max(f.height for f in frames)
    canvases = []
    for f in frames:
        canvas = Image.new("RGBA", (w, h))
        canvas.paste(f, ((w - f.width) // 2, h - f.height), f)
        canvases.append(canvas)

    # One palette for every frame; index 255 is transparency.
    strip = Image.new("RGB", (w * len(canvases), h))
    for i, canvas in enumerate(canvases):
        strip.paste(canvas.convert("RGB"), (i * w, 0))
    palette = strip.quantize(colors=255, dither=Image.Dither.NONE)
    indexed = []
    for canvas in canvases:
        frame = canvas.convert("RGB").quantize(palette=palette, dither=Image.Dither.NONE)
        frame.paste(255, mask=canvas.getchannel("A").point(lambda a: 255 if a < 128 else 0))
        indexed.append(frame)
    indexed[0].save(out, save_all=True, append_images=indexed[1:], duration=duration_ms,
                    loop=0, transparency=255, disposal=2)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", type=Path, default=Path("out/fox"))
    parser.add_argument("--model", type=Path, default=ROOT / "examples/models/fox.mcvox")
    parser.add_argument("--voxforge", type=Path, default=ROOT / "target/release/voxforge")
    parser.add_argument("--materials", type=Path, default=ROOT / "assets/materials")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)

    palette, fox = load_voxels(args.model)
    pngs = []
    for frame in range(FRAMES):
        model = args.out / f"fox_run_{frame}.mcvox"
        png = model.with_suffix(".png")
        write_frame(model, palette, frame_voxels(fox, frame))
        subprocess.run([str(args.voxforge), "preview", str(model), "--view", "iso-fr",
                        "--pixels-per-voxel", "10", "--materials", str(args.materials),
                        "--out", str(png)], check=True, stdout=subprocess.DEVNULL)
        pngs.append(png)
    gif(pngs, args.out / "fox-run.gif")
    print(args.out / "fox-run.gif")


if __name__ == "__main__":
    main()
