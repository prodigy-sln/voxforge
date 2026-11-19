<p align="center">
  <img src="https://cdn.prodigy.solutions/voxforge/hero.png" alt="Voxel models rendered by VoxForge: a knight, a unicorn, a fox, a portal, a car and a workbench" width="100%">
</p>

<h1 align="center">VoxForge</h1>

<p align="center"><b>Voxel models as plain text. Previews, checks and game-ready exports from one CLI.</b></p>

VoxForge is a voxel model format built to be written by people and AI agents, together with a fast command-line tool around it. A model is a small TOML file where each layer is drawn as ASCII grid art. VoxForge renders it from every angle, tells you exactly what's wrong with it, and exports it to MagicaVoxel, OBJ or glTF.

It is built around a short loop: **describe → render → look → fix**. Every refusal names the file, the part and the cell at fault, so an agent can repair its own model from the error message alone.

## Highlights

- **Text-first format.** `.mcvox` files are TOML: diffable, reviewable and easy to generate. Shared material files keep many models on one palette.
- **Rigs and states.** Models are trees of parts attached to each other, and each part can have alternative states (a lit and an unlit torch, an open and a closed door).
- **CPU preview renderer.** Six orthographic views, eight isometric views or a full contact sheet, rendered deterministically to PNG with no GPU.
- **Model inspection.** Voxel counts per material, bounds, connected components, floating voxels and mirror symmetry.
- **Block textures.** Bake one face or all six of a model into tileable textures, with seam checks.
- **Exports.** `.vox` (MagicaVoxel), `.obj` + `.mtl`, and binary glTF `.glb` with optional merged meshes, block units, linear colours and baked vertex ambient occlusion.

<p align="center">
  <img src="https://cdn.prodigy.solutions/voxforge/fox-sheet.png" alt="Contact sheet of a voxel fox from all six orthographic and eight isometric views" width="640"><br>
  <sub>One <code>voxforge preview</code> call: every canonical view of a model on one sheet.</sub>
</p>

<p align="center">
  <img src="https://cdn.prodigy.solutions/voxforge/fox-run-2.gif" alt="Animated voxel fox running over scrolling grass" width="300"><br>
  <sub>Models are just text, so a script can generate them too. Each frame of this run cycle is a generated <code>.mcvox</code> rendered with <code>voxforge preview --view iso-fr</code>.</sub>
</p>

## Quick start

VoxForge is a Rust project (edition 2024, Rust 1.85 or newer).

```sh
git clone https://github.com/prodigy-sln/voxforge.git
cd voxforge
cargo build --release
```

The binary is `target/release/voxforge`. Try it on the bundled model:

```sh
voxforge inspect assets/models/human.mcvox
voxforge preview assets/models/human.mcvox --out human.png
voxforge export  assets/models/human.mcvox --out human.glb --mesh merged
```

`--materials` defaults to `assets/materials`, which ships about 50 ready-made materials.

## A model in 40 lines

```toml
schema = 1
name   = "demo:mushroom"
scale  = 16                 # voxels per block edge
size   = [5, 4, 5]
slice  = "y"                # each layer is a floor plan, bottom up

[palette]
"." = "empty"
"s" = "base:fox_cream"
"c" = "base:fox_fur"

[[layers]]
y = 0
grid = """
.....
.....
..s..
.....
.....
"""

[[layers]]
y = 1
grid = """
.....
.....
..s..
.....
.....
"""

[[layers]]
y = 2
grid = """
.ccc.
ccccc
ccccc
ccccc
.ccc.
"""

[[layers]]
y = 3
grid = """
.....
.ccc.
.ccc.
.ccc.
.....
"""
```

```text
$ voxforge inspect mushroom.mcvox
filled 32 voxels
bounds (0, 0, 0) to (4, 3, 4) inclusive
  base:fox_cream 2
  base:fox_fur 30
components 1
floating none
symmetry x Mirrored
```

The full specification, covering multi-part rigs, states, orientation rules and limits, is in **[docs/format.md](docs/format.md)**.

## Commands

| Command | What it does |
|---|---|
| `voxforge preview <model> --out <png>` | Renders a contact sheet of every view, or a single view with `--view` (`front`, `back`, `left`, `right`, `top`, `bottom`, `iso-fl`, `iso-fr`, `iso-bl`, `iso-br`, plus `-under` variants). |
| `voxforge inspect <model>` | Prints statistics and structural problems. Exits non-zero when the model is defective. |
| `voxforge texture <model> --out <dir> --face <face>` | Writes a block texture for one face, or all six with `--all-faces`. `--seamless` refuses textures that won't tile. |
| `voxforge export <model> --out <file>` | Exports to `.vox`, `.obj` or `.glb`, chosen by the file extension. |

Shared options:

- `--materials <dir>`: where material keys are resolved from.
- `--state part=state`: picks a part's state (repeatable).
- `--pixels-per-voxel <n>`: the render scale (default 24).

### glTF export options

| Option | Values | Default |
|---|---|---|
| `--mesh` | `cubes` (six faces per voxel), `merged` (hidden faces dropped, coplanar faces merged) | `cubes` |
| `--unit` | `voxel`, `block` (positions divided by the model's `scale`) | `voxel` |
| `--colors` | `srgb`, `linear` | `srgb` |
| `--ao` | `off`, `vertex` (ambient occlusion baked into vertex colours) | `off` |
| `--ao-strength` | `0.0`–`1.0` | `0.5` |

A typical game-engine export:

```sh
voxforge export model.mcvox --out model.glb --mesh merged --unit block --colors linear --ao vertex
```

### Larger models

Parts and assembled models are limited to 64 voxels per axis by default. Pass `--max-extent <N>` (64–256) to any command to raise the limit for that run.

## Materials

A material is one TOML file with a namespaced name, an sRGB colour and an optional emissive factor:

```toml
name     = "base:flame"
color    = "#ff9a3c"
emissive = 0.8
```

Every `*.toml` file in the materials directory is loaded. Palettes refer to materials by name, so many models stay on one consistent palette.

<p align="center">
  <img src="https://cdn.prodigy.solutions/voxforge/blocks.png" alt="Twelve 16×16×16 voxel blocks: grass, dirt, stone, oak planks, oak log, obsidian, copper, iron, royal, amber lamp, neon panel and portal" width="100%"><br>
  <sub>16³ blocks built only from the bundled materials. Emissive materials such as the amber lamp, neon and portal glow through the shading.</sub>
</p>

## Using VoxForge with AI agents

The format and the error messages are designed for agents. [`.agents/skills/voxel-asset-creation`](.agents/skills/voxel-asset-creation/SKILL.md) describes a workflow where one agent writes the model and renders previews, and an independent reviewer agent critiques only the images until the design holds up.

## Development

```sh
cargo test
```

The test suite is self-contained. Fixtures live in `tests/fixtures/`, and golden images and exports in `tests/goldens/`. The golden contact sheet in `tests/goldens/reference-asymmetric-sheet.png` is a human-reviewed picture: if it stops matching, re-shooting it means asking for a new review, not repairing the test.

### Compatibility

Other projects run this tool in their build pipelines. New behaviour is added as new options or commands. Existing defaults, file formats and output stay as they are.
