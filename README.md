# VoxForge (export fork)

This folder is a fork of `MyCraft/tools/voxforge`, taken at MyCraft commit `f483a363` (2026-08-15). **MyCraft's copy is the original**; this one exists for the `export` command.

## How the two copies differ

| | This fork | MyCraft `tools/voxforge` |
|---|---|---|
| `preview`, `inspect`, `texture` | Same code as MyCraft at `f483a363` | Same, plus later changes |
| `export` (`.vox`, `.obj`, `.glb`) | Yes | No |
| `build` (texture sets from a manifest) | No | Yes (since 2026-08-21) |
| Default `--materials` | `assets/materials` | `content/base/materials` |
| Default preview scale | 24 | 8 |
| Depends on | `mc-core` by path (`../MyCraft/crates/mc-core`) | MyCraft workspace |

The two defaults differ, so neither copy can replace the other without changing behaviour for one of its users. Merging them is the owner's decision.

## Rules for changes here

- Other projects call this tool. Add behaviour as new options or commands; do not change existing defaults, formats or output.
- Format, preview, inspect and texture changes belong in MyCraft's copy first.

## Larger models

By default a part and an assembled model may reach 64 voxels on an axis. `--max-extent <N>` (64 to 256, any command, before or after the command name, deliberately absent from `--help` so help texts stay unchanged) raises both limits for `inspect`, `preview`, `export` and `texture`. Other values are refused. Without it nothing changes, including the refusal at 65. In the library, `load_document_with`, `parse_document_with`, `assemble_with` and `inspect_document_with` take a `MaxExtent`; the old functions keep their signatures and the 64 limit. The preview ray start distance grows with the model's diagonal (unchanged up to 64 voxels a side); `.vox` export already allowed 256.

## Tests

`cargo test` does not compile as a whole. Run targets one at a time: `cargo test --test <name>`.

- `reference_golden` needs MyCraft's `mc-testkit`, which is not a dependency here.
- `build_*` test the `build` command, which this fork does not have. They fail.
- `export_glb` covers the export options and compares default output with files in `tests/goldens/export/` byte for byte.

## Users

- IdleSettler (`cargo xtask assets`) calls `export … --mesh merged --unit block --colors linear --ao vertex`.
