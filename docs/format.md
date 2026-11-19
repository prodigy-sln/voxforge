# The `.mcvox` format

A `.mcvox` file describes one voxel model — a door, a torch, a chair, a
prop — as TOML text: a palette of characters and layers of grid art. Colours
live in separate **material files**. VoxForge reads both, and its commands
let you see what you wrote:

| Command | Does |
|---|---|
| `voxforge preview <file> --out <png>` | Renders the model from one view, or all fourteen tiled into a contact sheet. |
| `voxforge inspect <file>` | Prints statistics, observations and defects. |
| `voxforge texture <file> --out <dir>` | Emits flat, axis-aligned faces as block textures, with a tileability verdict. |
| `voxforge export <file> --out <path>` | Writes `.vox`, `.obj` or `.glb`. |
| `voxforge build <manifest>` | Bakes the faces a manifest names into a texture set (hidden from `--help`). |

The loop this is built for: write the file, run `preview` or `inspect`, fix
the text from what you see. There are no editing commands (`fill_box`,
`mirror`, `translate`); you edit the grid text.

## File layout

- **Models:** one model per `.mcvox` file, at any path. This repository keeps
  its model at `assets/models/human.mcvox`.
- **Materials:** one material per `.toml` file, all directly inside one
  directory — by default `assets/materials` (change it with `--materials`).

Every TOML table in either kind of file **rejects unknown fields**: the
document, each `[[parts]]` table, each `[[layers]]` table, each material file
and the build manifest. A typo fails to load instead of being ignored.

## Namespaced ids

Model names, material keys and texture keys are all namespaced ids: exactly
one `:`, with text on both sides — `base:oak_plank`. No other character set is
imposed. Part names and state names are plain strings and are not namespaced.

## The document

```toml
schema = 1
name   = "base:door_oak"
scale  = 16
size   = [4, 6, 2]
origin = [0, 0, 1]
slice  = "z"

[palette]
"." = "empty"
"w" = "base:oak_plank"
"h" = "base:iron"

[[layers]]                # z = 0, the back: plain
z = 0
grid = """
wwww
wwww
wwww
wwww
wwww
wwww
"""

[[layers]]                # z = 1, the front; the first row is y = 5
z = 1
grid = """
wwww
wwww
wwwh
wwww
wwww
wwww
"""
```

### Top-level fields

| Field | Required | Value |
|---|---|---|
| `schema` | yes | Integer. The only supported value is `1`. |
| `name` | yes | Namespaced id. |
| `scale` | yes | Integer ≥ 1: how many voxels span one block edge. `16` is a convention, not a rule. `texture` and `build` judge tileability against it. |
| `slice` | yes | `"x"`, `"y"` or `"z"`: the default axis layers are planes of. |
| `size` | implicit form | `[x, y, z]`, integers. |
| `origin` | no | `[x, y, z]`, integers, default `[0, 0, 0]`. Used only by the implicit form. |
| `palette` | yes | Table, see [Palette](#palette). |
| `parts` | explicit form | Array of `[[parts]]` tables. |
| `layers` | no | Array of `[[layers]]` tables, for every part. |

### Two geometry forms

A document uses exactly one:

- **Implicit single-part form:** a top-level `size` (and optional `origin`),
  no `[[parts]]`. A one-part model pays nothing for a rig it does not use.
- **Explicit form:** one or more `[[parts]]` tables, no top-level `size`. A
  top-level `origin` has no effect here.

Declaring both, or neither, is refused.

### Limits

- Every `size` axis is at least 1 and at most **64** voxels.
- The **assembled** model may reach at most 64 voxels on any axis, measured
  on the filled voxels after the parts are placed (a part may declare more
  extent than it fills).
- Part sizes are checked before any grid is read, so an over-large
  declaration is refused even if its layer art does not exist yet.
- A model that assembles to no filled voxel at all is refused.

`--max-extent <N>`, with `N` from 64 to 256, raises both limits for one run.
It works with every command, before or after the command name, and is
deliberately hidden from `--help`. Values outside that range are refused.

## Layers, slicing and orientation

A layer is one 2-D slab of grid art. The part's slice axis says which plane it
is, and the layer names its plane with the key matching that axis — `x = <k>`,
`y = <k>` or `z = <k>`.

| `slice` | Layer *k* is the plane | Rows run | First row printed | Columns run, first column |
|---|---|---|---|---|
| `y` | `y = k`, upward from the ground | along `z` | `z = 0` | along `x`, `x = 0` |
| `z` | `z = k`, from the back (`z = 0`) to the front | along `y` | `y = size.y − 1` (top) | along `x`, `x = 0` |
| `x` | `x = k`, from the left (`x = 0`) to the right | along `y` | `y = size.y − 1` (top) | along `z`, `z = 0` |

The axes are right-handed with `+y` up: the `front` view looks along `−z` from
the `+z` side, `x` grows to the right in that view, and `left` is the `−x`
side. A part may override the model's `slice` (a stool reads best as floor
plans, `slice = "y"`; a door as elevations, `slice = "z"`).

**Check orientation by eye with `voxforge preview`.** A mirrored or transposed
reading still produces a plausible picture, so a wrong assumption here is easy
to "correct" in the wrong direction.

### Layer fields

| Field | Value |
|---|---|
| `x` / `y` / `z` | Integer, the plane. Only the key for the part's slice axis is read; it is required. |
| `grid` | String, one line per row. Use a `"""` block. |
| `part` | Part name. Explicit form only; a layer without one belongs to the first declared part. |
| `state` | State name, for a part that declares `states`. |

### Grid rules

- The grid must match the extent the slice axis implies **exactly**: row count
  and every row's width. Nothing is trimmed, so a trailing space is a refusal.
- A `"""` block's final newline before the closing delimiter is not a row.
  `\r\n` line endings are accepted.
- Every character must be ASCII and declared in the palette.
- The plane must lie within `0 … extent − 1` on the slice axis.
- An omitted plane is legal and means an empty slab.
- Two layers of one part (and one state) declaring the same plane are refused.
- Every part needs at least one layer.

## Palette

`[palette]` maps a single ASCII character to either `"empty"` (no voxel) or a
namespaced material key. Inline colours are not allowed.

```toml
[palette]
"." = "empty"
"w" = "base:oak_plank"
"f" = "base:flame"
```

- The palette must exist and hold at least one entry.
- A key longer than one character, a non-ASCII key, or a non-string value is
  refused.
- A palette entry no grid uses still loads; `inspect` reports it as a defect.
- `preview`, `texture`, `export` and `build` resolve **every** material key in
  the palette, used or not, against the materials directory, and refuse a key
  no file declares (naming the key and the directory). `inspect` reads no
  materials.

## Material files

```toml
# assets/materials/flame.toml
name     = "base:flame"
color    = "#ff9a3c"
emissive = 0.8
```

| Field | Required | Value |
|---|---|---|
| `name` | yes | Namespaced id; the key palettes refer to. |
| `color` | yes | `#rrggbb`, exactly six hex digits. No shorthand. |
| `emissive` | no | Float from `0.0` to `1.0`, default `0.0`. Must be written as a float (`1.0`, not `1`). |

`emissive` is a fraction of self-illumination, not a light level. In
`preview` it blends a face's shading factor toward 1:
`factor × (1 − emissive) + emissive`, so `1.0` draws the material unshaded.

The materials directory:

- Only `*.toml` files directly inside it are read, in **file-name sorted
  order**. The order matters: a name declared twice is refused naming the
  first file and the second.
- A missing directory, or one declaring no material, is refused.

Materials are shared keys rather than per-model colours so that many models
authored at different times stay on one palette.

## Parts, attachment and states

### Part fields

| Field | Required | Value |
|---|---|---|
| `name` | yes | String, unique within the document. |
| `size` | yes | `[x, y, z]`, within the limits above. |
| `origin` | no | `[x, y, z]`, the pivot in the part's own space, default `[0, 0, 0]`. May be negative or outside the part. |
| `slice` | no | Overrides the model's `slice`. |
| `attach` | all but the root | `{ to = "<parent>", at = [x, y, z] }`. |
| `states` | no | Array of state names; the first is the default. |

### The part tree

Parts form a tree rooted at the **one** part with no `attach`. Refused: two
parts with one name, a part attached to itself, an attachment to an
undeclared part, no root, more than one root, and a cycle.

A child's local voxel `p` lands at

```
parent_position + attach.at + p − child.origin
```

which may be negative: a pivot exists so a part can extend in `−x`, `−y` or
`−z` from it. The root sits at `(0, 0, 0)`; its own `origin` is not applied.
After placement the whole model is **normalised** so its minimum filled corner
is `(0, 0, 0)`.

### States

A state is a different set of voxels for one part — a flickering flame — as
opposed to a rotation, which the format does not represent. A state's layers
**replace** the part's art; they do not add to it.

- In a part with `states`, every layer names a declared `state`.
- Every declared state has at least one layer.
- A part without `states` has layers without `state`.

The default state is the first declared. Commands select others with
`--state part=state` (repeatable); naming a state the part does not declare is
refused.

```toml
schema = 1
name   = "base:torch"
scale  = 16
slice  = "y"

[palette]
"." = "empty"
"w" = "base:oak_plank"
"f" = "base:flame"

[[parts]]
name   = "handle"
size   = [2, 10, 2]
origin = [1, 0, 1]
                          # no `attach`: this is the root
[[parts]]
name   = "flame"
size   = [4, 6, 4]
origin = [2, 0, 2]        # pivot at the flame's base centre
attach = { to = "handle", at = [1, 10, 1] }
states = ["low", "high"]

[[layers]]
part = "handle"
y    = 0
grid = """
ww
ww
"""

[[layers]]
part  = "flame"
state = "low"
y     = 0
grid  = """
....
.ff.
.ff.
....
"""

[[layers]]
part  = "flame"
state = "high"
y     = 0
grid  = """
.ff.
ffff
ffff
.ff.
"""
```

(Only one handle layer is shown; planes left out are empty.)

`voxforge preview torch.mcvox --out torch.png --state flame=high` renders the
second state.

## Commands

All commands that read a document take `--state part=state` (repeatable) and
the hidden `--max-extent <N>`. `preview`, `texture` and `export` take
`--materials <dir>` (default `assets/materials`).

### Refusals

A refusal prints one line on stderr, nothing on stdout, and exits non-zero.
The line names the file, then the part, layer and field where known, then the
cause with the value at fault:

```
torch.mcvox, part `flame`, layer 1 (plane 6): this layer declares y = 6, but its part has an extent of 6 on that axis, so its last plane is 5
```

`layer N` counts `[[layers]]` tables from zero in declaration order;
`(plane P)` is the plane it declared. A refusal leaves any existing output
file untouched: images are fully encoded before a file is opened.

### `inspect`

`voxforge inspect <file>` reports the model as assembled (with the selected
states):

```
filled 64 voxels
bounds (0, 0, 0) to (3, 3, 3) inclusive
  base:oak_plank 64
components 1
floating none
symmetry x Mirrored
defect: the palette declares `h` and no grid spells it
```

| Kind | Reports | Exit code |
|---|---|---|
| Statistics | Filled count, inclusive bounding box, voxels per material | — |
| Observation | 6-connected components (on the assembled model); floating voxels (voxels sharing a face with no other); mirror symmetry, including materials, about the `x` midplane (`Mirrored` / `NotMirrored`) | 0 |
| Defect | An unused palette entry | non-zero |

Connectivity and symmetry never fail a run: the file cannot distinguish a
detached hinge from a deliberately floating element. Connectivity is also not
structural soundness — a part hanging by one face is connected.

### `preview`

```
voxforge preview assets/models/human.mcvox --out human.png [--view <name>] [--pixels-per-voxel <n>]
```

- **Views (14):** `front`, `back`, `left`, `right`, `top`, `bottom`, the
  overhead corners `iso-fl`, `iso-fr`, `iso-bl`, `iso-br`, and the same
  corners from below, `iso-fl-under`, `iso-fr-under`, `iso-bl-under`,
  `iso-br-under`.
- Without `--view`, all fourteen are tiled into a **contact sheet**; stdout
  prints the image path, then one line per tile, such as
  `column 0, row 0: front` (the image carries no labels).
- `--pixels-per-voxel` defaults to **24**; `0` is refused.
- Rendering is an orthographic CPU ray march, one opaque sample per pixel,
  shaded in linear space before sRGB encoding: `+y` 1.00, `±x` 0.80, `±z`
  0.65, `−y` 0.50. The shading is a legibility aid, not a lighting model.
- Output is deterministic: the same model gives byte-identical PNGs whatever
  the declaration order.

### `texture`

```
voxforge texture <file> --out <dir> (--face <name> | --all-faces) [--seamless] [--pixels-per-voxel <n>]
```

Emits flat, unshaded faces — one of `front`, `back`, `left`, `right`, `top`,
`bottom`, or all six — as `<dir>/<face>.png`. The directory must exist.
`--pixels-per-voxel` defaults to 24. Exactly one of `--face` and `--all-faces`
is required.

- `--all-faces` requires the assembled model to be a cube of its declared
  `scale` on every axis.
- All faces are rendered and judged before any file is written; a write error
  partway removes the files already written.
- Stdout prints one line per face in the order `front`, `back`, `left`,
  `right`, `top`, `bottom`: face name, path, the model axes its columns and
  rows run along, and its verdicts.

**Orientation:** each image's top edge points toward the face's own up, and
its right edge toward the right of a viewer outside the face looking at it.
For `top` the image's top edge points toward `−z`; for `bottom`, toward `+z`.

**Tileability verdict**, always printed. It refuses the emission only with
`--seamless`. Three checks, in order:

| Check | Printed when it fails |
|---|---|
| Period | `the <axis> axis is <n> voxels across, where the declared scale of <s> makes the texture's period one block` — an in-plane extent is not exactly `scale`. |
| Coverage | `<n> pixel(s) are transparent, the first at row <r>, column <c>` — some pixel shows no voxel. |
| Edges | `the edges disagree along the <axis> axis: <line> steps <a> across the wrap, where its largest step within is <b>` — wrapping introduces a bigger step than any inside the texture. |

A face passing all three prints `tiles across every edge`. Without
`--seamless` every check runs and every failure is printed; with it, the first
failure refuses the emission.

Flatness is a property of this render, not of a material. Raising a
material's `emissive` to `1.0` to flatten one texture would also unshade every
model using it.

### `export`

```
voxforge export <file> --out <path>.vox|.obj|.glb
```

The extension picks the format; any other is refused. `.obj` also writes a
`.mtl` beside it. Stdout prints `Exported to <path>`.

`.glb` writes one node per part and takes these options (each refused with
`.vox` or `.obj` unless left at its default):

| Option | Values | Default |
|---|---|---|
| `--mesh` | `cubes` (six faces per voxel), `merged` (hidden faces dropped, coplanar faces merged; also prints vertex and triangle counts) | `cubes` |
| `--unit` | `voxel`, `block` (positions divided by `scale`) | `voxel` |
| `--colors` | `srgb` (as declared), `linear` | `srgb` |
| `--ao` | `off`, `vertex` (ambient occlusion baked into vertex colours) | `off` |
| `--ao-strength` | `0.0` to `1.0` | `0.5` |

`.vox` and `.obj` use the assembled model with the selected states. `.glb`
always uses each part's default state.

### `build`

`voxforge build <manifest>` bakes a named set of block textures from a
committed manifest, so "which art does this set contain" is a reviewable file
rather than a remembered command line. It writes one PNG per entry plus an
`index.txt` into the output directory.

```toml
# textures.toml
output           = "textures"
materials        = "materials"
blocks           = "blocks"
pixels_per_voxel = 1

[[texture]]
key   = "example:plank_top"
model = "models/plank-block.mcvox"
face  = "top"
```

| Field | Value |
|---|---|
| `output` | Directory, created if missing. |
| `materials` | Materials directory (rules as above). Every material in it is a source of the set. |
| `blocks` | Directory whose `*.luau` files are searched as plain text for each key. A key found nowhere is reported on stdout (`` `key` is baked here and named by no block declaration ``); never a refusal. The directory need not exist. |
| `pixels_per_voxel` | Integer ≥ 1. `scale × pixels_per_voxel` must equal 16. |
| `[[texture]]` | Zero or more entries: `key` (namespaced id, unique, no control characters), `model` (path to a `.mcvox`), `face` (one of the six faces). |

All paths are relative to the manifest's directory. Each named model must be
a cube of its `scale`, and each selected face must tile (faces no entry
selects are not judged). One model may serve several entries.

A key's image is named by replacing each `:` with `__` and appending `.png`
(`example:plank_top` → `example__plank_top.png`); the result may hold only
letters, digits, `.`, `-` and `_`.

Everything is rendered and encoded before any file is written, so a refusal
leaves the previous set intact. If the sources are unchanged and every image
is present, nothing is written and stdout says `nothing needed rebuilding`.
Otherwise stdout lists each file written, then any advisory lines.

The index:

```
mycraft-texture-set 1
fold b9a7b041e0760eed
source textures.toml
source models/plank-block.mcvox
source materials/plank.toml
key example__plank_top.png example:plank_top
```

`fold` is an FNV-1a-64 value (16 lowercase hex digits) over the `source`
files in the order listed: the manifest, each model in order of first mention,
then every material file by name. Paths are relative to the manifest and
`/`-separated. A `key` line puts the image name first and the key last,
because only the key may contain spaces.

## What the format does not do

- **No animation.** Parts, pivots, the attachment tree and discrete states
  only — no rotations, keyframes, durations or interpolation. `schema` exists
  so such fields can be added later without invalidating existing files.
- **No editing verbs.** Edit the grid text.
- **No import.** `export` writes `.vox`, `.obj` and `.glb`; nothing reads them
  back into `.mcvox`.
- **No procedural generation or image-to-voxel carving.**
