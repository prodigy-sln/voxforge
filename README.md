# VoxForge (export fork)

This folder is a fork of `MyCraft/tools/voxforge`, taken at MyCraft commit `f483a363` (2026-08-15). **MyCraft's copy is the original**; this one exists for the `export` command.

## How the two copies differ

| | This fork | MyCraft `tools/voxforge` |
|---|---|---|
| `preview`, `inspect`, `texture` | Same code as MyCraft at `f483a363` | Same, plus later changes |
| `export` (`.vox`, `.obj`, `.glb`) | Yes | No |
| `build` (texture sets from a manifest) | Yes, ported from MyCraft; hidden from `--help` so every help text stays as it was | Yes (since 2026-08-21) |
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

Run targets one at a time with `cargo test --test <name>`. `cargo test` as a whole does not compile, and 9 tests fail (state of 2026-11-12). The other 223 pass.

The failing tests were written for MyCraft's tree and need things this repository does not have. None was changed, skipped or special-cased.

- `reference_golden` does not compile: it needs `mc-testkit`, a MyCraft crate. Adding it as a path dev-dependency compiles it but raises `image` in `Cargo.lock` from 0.25.9 to 0.25.10 (it needs Rust 1.88, the fork declares 1.85) and `moxcms` to 0.8, which changes the normal build, so it is not added. The test would still fail (next point).
- `reference_golden`, one test in `command_line`, and the `build_writes_a_set` and `build_determinism` tests that use the shipped content root (4 + 1 tests) read `<package>/../../content/base/...` and `<package>/../../tools/voxforge/tests/goldens/...`. That is the MyCraft repository root when the package sits at `tools/voxforge`; here it is outside this repository. They pass when the package sits at `<root>/tools/voxforge` with `<root>/content/base` taken from MyCraft at commit `bdbb0214` (7 manifest entries; MyCraft's current 8 do not match the tests' count of 7).
- Two tests in `texture_streams` (`a_single_face_texture_writes_its_image_and_prints_one_line_about_it`, `an_undeclared_face_set_writes_all_six_and_reports_each_faces_verdict`) run `texture` without `--pixels-per-voxel` and expect MyCraft's default of 8. This fork's default is 24 and must not change.

The `build_*` tests pass except the five that read the shipped content root. `build_help` pins that the top-level help lists only the original commands. `export_glb` covers the export options and compares default output with files in `tests/goldens/export/` byte for byte.

## Users

- IdleSettler (`cargo xtask assets`) calls `export … --mesh merged --unit block --colors linear --ao vertex`.
