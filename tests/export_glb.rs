//! `export` to `.glb`: the `--mesh`, `--unit`, `--colors`, `--ao` and `--ao-strength` options.

mod common;

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use common::cli::{document_at, invoke, material_file, materials_at};
use common::TestResult;
use serde_json::Value;
use tempfile::TempDir;
use voxforge::inspect::ExitCode;

/// One `.glb`, decoded enough to grade.
struct Glb {
    json: Value,
    bin: Vec<u8>,
}

impl Glb {
    fn read(path: &Path) -> Result<Self, Box<dyn Error>> {
        let bytes = fs::read(path)?;
        let u32_at = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize;
        assert_eq!(&bytes[0..4], b"glTF");
        let json_len = u32_at(12);
        let json = serde_json::from_slice(&bytes[20..20 + json_len])?;
        let bin_start = 20 + json_len + 8;
        let bin = bytes[bin_start..bin_start + u32_at(20 + json_len)].to_vec();
        Ok(Self { json, bin })
    }

    fn floats(&self, accessor: usize, width: usize) -> Vec<Vec<f32>> {
        let acc = &self.json["accessors"][accessor];
        let view = &self.json["bufferViews"][acc["bufferView"].as_u64().unwrap() as usize];
        let offset = view["byteOffset"].as_u64().unwrap() as usize;
        let count = acc["count"].as_u64().unwrap() as usize;
        (0..count)
            .map(|v| {
                (0..width)
                    .map(|c| {
                        let at = offset + (v * width + c) * 4;
                        f32::from_le_bytes(self.bin[at..at + 4].try_into().unwrap())
                    })
                    .collect()
            })
            .collect()
    }

    fn primitive(&self, mesh: usize) -> &Value {
        &self.json["meshes"][mesh]["primitives"][0]
    }

    fn attribute(&self, mesh: usize, name: &str) -> usize {
        self.primitive(mesh)["attributes"][name].as_u64().unwrap() as usize
    }

    fn positions(&self, mesh: usize) -> Vec<Vec<f32>> {
        self.floats(self.attribute(mesh, "POSITION"), 3)
    }

    fn normals(&self, mesh: usize) -> Vec<Vec<f32>> {
        self.floats(self.attribute(mesh, "NORMAL"), 3)
    }

    fn colors(&self, mesh: usize) -> Vec<Vec<f32>> {
        self.floats(self.attribute(mesh, "COLOR_0"), 4)
    }

    fn indices(&self, mesh: usize) -> Vec<usize> {
        let acc = &self.json["accessors"][self.primitive(mesh)["indices"].as_u64().unwrap() as usize];
        let view = &self.json["bufferViews"][acc["bufferView"].as_u64().unwrap() as usize];
        let offset = view["byteOffset"].as_u64().unwrap() as usize;
        (0..acc["count"].as_u64().unwrap() as usize)
            .map(|i| u32::from_le_bytes(self.bin[offset + i * 4..offset + i * 4 + 4].try_into().unwrap()) as usize)
            .collect()
    }

    fn vertex_count(&self) -> u64 {
        (0..self.mesh_count())
            .map(|m| self.json["accessors"][self.attribute(m, "POSITION")]["count"].as_u64().unwrap())
            .sum()
    }

    fn triangle_count(&self) -> u64 {
        (0..self.mesh_count())
            .map(|m| {
                let idx = self.primitive(m)["indices"].as_u64().unwrap() as usize;
                self.json["accessors"][idx]["count"].as_u64().unwrap() / 3
            })
            .sum()
    }

    fn mesh_count(&self) -> usize {
        self.json["meshes"].as_array().unwrap().len()
    }
}

/// A directory holding red, blue and grey materials.
fn workspace() -> Result<(TempDir, PathBuf), Box<dyn Error>> {
    let dir = TempDir::new()?;
    let materials = materials_at(
        &dir,
        "materials",
        &[
            ("blue.toml", material_file("test:blue", "#0000ff", "0.0")),
            ("grey.toml", material_file("test:grey", "#808080", "0.0")),
            ("red.toml", material_file("test:red", "#ff0000", "0.0")),
        ],
    )?;
    Ok((dir, materials))
}

/// A single-part, `y`-sliced model of `size`, whose voxel `(x, y, z)` holds the
/// palette character `at` answers ('.' for empty).
fn single_part(size: [u32; 3], scale: u32, at: impl Fn(u32, u32, u32) -> char) -> String {
    let mut text = format!(
        "schema = 1\nname = \"test:shape\"\nscale = {scale}\nsize = [{}, {}, {}]\norigin = [0, 0, 0]\nslice = \"y\"\n\n[palette]\n\".\" = \"empty\"\n\"r\" = \"test:red\"\n\"b\" = \"test:blue\"\n\"g\" = \"test:grey\"\n",
        size[0], size[1], size[2]
    );
    for y in 0..size[1] {
        text.push_str(&format!("\n[[layers]]\ny = {y}\ngrid = \"\"\"\n"));
        for z in 0..size[2] {
            let row: String = (0..size[0]).map(|x| at(x, y, z)).collect();
            text.push_str(&row);
            text.push('\n');
        }
        text.push_str("\"\"\"\n");
    }
    text
}

fn export(document: &Path, materials: &Path, out: &Path, extra: &[&str]) -> Result<common::cli::Invocation, Box<dyn Error>> {
    let mut args = vec![
        "export",
        document.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--materials",
        materials.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    invoke(&args)
}

fn exported(document: &Path, materials: &Path, out: &Path, extra: &[&str]) -> Result<Glb, Box<dyn Error>> {
    let run = export(document, materials, out, extra)?;
    assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
    Glb::read(out)
}

const TWO_PARTS: &str = r#"schema = 1
name = "test:two"
scale = 4
slice = "z"

[palette]
"." = "empty"
"r" = "test:red"

[[parts]]
name = "body"
size = [2, 1, 1]
origin = [0, 0, 0]

[[parts]]
name = "arm"
size = [1, 1, 1]
origin = [0, 0, 0]
attach = { to = "body", at = [2, 0, 0] }

[[layers]]
part = "body"
z = 0
grid = """
rr
"""

[[layers]]
part = "arm"
z = 0
grid = """
r
"""
"#;

/// A path inside this repository.
fn project_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn golden_path(name: &str) -> PathBuf {
    project_path("tests/goldens/export").join(name)
}

/// The `export --help` lines that predate the `.glb` options, apart from the
/// `--out` text, which now also names `.glb`.
const OLD_ARGUMENT_LINE: &str = "  <DOCUMENT>  The `.mcvox` document to read";
const OLD_OPTION_LINES: [&str; 3] = [
    "      --out <OUT>              Where to write the exported file (extension determines format: .vox, .obj, .glb)",
    "      --materials <MATERIALS>  Where to resolve material keys from",
    "      --state <STATES>         Which state a part takes, as `part=state`. Repeatable",
];
const HELP_LINE: &str = "  -h, --help                   Print help";

#[test]
fn export_help_keeps_one_line_per_option() -> TestResult {
    let mut texts = Vec::new();
    for flag in ["--help", "-h"] {
        let run = invoke(&["export", flag])?;
        let lines: Vec<&str> = run.err.lines().collect();
        assert!(lines.contains(&OLD_ARGUMENT_LINE), "{flag}: {}", run.err);
        assert!(!run.err.contains("Possible values:"), "{flag}: {}", run.err);
        assert!(!run.err.contains("Print help ("), "{flag}: {}", run.err);

        let start = lines.iter().position(|l| *l == "Options:").expect("Options section");
        let block: Vec<&str> = lines[start + 1..].iter().copied().take_while(|l| !l.is_empty()).collect();
        assert_eq!(block.len(), 9, "{flag}: {block:#?}");
        assert_eq!(block[..3], OLD_OPTION_LINES, "{flag}");
        assert_eq!(block[8], HELP_LINE, "{flag}");
        for (line, (name, default, values)) in block[3..8].iter().zip([
            ("--mesh <MESH>", "cubes", Some("cubes, merged")),
            ("--unit <UNIT>", "voxel", Some("voxel, block")),
            ("--colors <COLORS>", "srgb", Some("srgb, linear")),
            ("--ao <AO>", "off", Some("off, vertex")),
            ("--ao-strength <N>", "0.5", None),
        ]) {
            assert!(line.starts_with(&format!("      {name}")), "{flag}: {line}");
            assert!(line.contains(&format!("[default: {default}]")), "{flag}: {line}");
            match values {
                Some(values) => assert!(line.contains(&format!("[possible values: {values}]")), "{flag}: {line}"),
                None => assert!(!line.contains("possible values"), "{flag}: {line}"),
            }
        }
        texts.push(run.err);
    }
    assert_eq!(texts[0], texts[1]);
    Ok(())
}

/// A default `.glb` export of `model` is byte-for-byte the golden written by
/// the tool before the `.glb` options existed.
fn assert_matches_golden(model: &Path, golden: &str) -> TestResult {
    let dir = TempDir::new()?;
    let out = dir.path().join("out.glb");
    let run = export(model, &project_path("assets/materials"), &out, &[])?;
    assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
    assert_eq!(run.out, format!("Exported to {}\n", out.display()));
    let (actual, expected) = (fs::read(&out)?, fs::read(golden_path(golden))?);
    assert!(actual == expected, "{golden}: {} bytes written, {} expected", actual.len(), expected.len());
    Ok(())
}

#[test]
fn default_export_of_human_matches_the_golden() -> TestResult {
    assert_matches_golden(&project_path("assets/models/human.mcvox"), "human.glb")
}

#[test]
fn default_export_of_horse_matches_the_golden() -> TestResult {
    assert_matches_golden(&project_path("horse.mcvox"), "horse.glb")
}

#[test]
fn default_export_of_two_parts_matches_the_golden() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "two.mcvox", TWO_PARTS)?;
    let out = dir.path().join("two.glb");
    export(&doc, &materials, &out, &[])?;
    assert!(fs::read(&out)? == fs::read(golden_path("two_parts.glb"))?);
    Ok(())
}

#[test]
fn default_export_of_three_levels_matches_the_golden() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "three.mcvox", &three_levels())?;
    let out = dir.path().join("three.glb");
    export(&doc, &materials, &out, &[])?;
    assert!(fs::read(&out)? == fs::read(golden_path("three_levels.glb"))?);
    Ok(())
}

/// Body, arm on the body, hand on the arm; scale 10; two materials.
fn three_levels() -> String {
    fs::read_to_string(golden_path("three_levels.mcvox")).expect("fixture")
}

#[test]
fn default_export_is_identical_to_explicit_defaults() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "two.mcvox", TWO_PARTS)?;
    let plain = dir.path().join("plain.glb");
    let explicit = dir.path().join("explicit.glb");
    let run = export(&doc, &materials, &plain, &[])?;
    assert_eq!(run.out, format!("Exported to {}\n", plain.display()));
    export(&doc, &materials, &explicit, &["--mesh", "cubes", "--unit", "voxel", "--colors", "srgb"])?;
    assert_eq!(fs::read(&plain)?, fs::read(&explicit)?);
    assert_eq!(Glb::read(&plain)?.vertex_count(), 3 * 24);
    Ok(())
}

#[test]
fn solid_cube_merges_to_six_quads() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "cube.mcvox", &single_part([4, 4, 4], 4, |_, _, _| 'r'))?;
    let out = dir.path().join("cube.glb");
    let run = export(&doc, &materials, &out, &["--mesh", "merged"])?;
    assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
    assert_eq!(
        run.out,
        format!("Exported to {}\nMerged mesh: 24 vertices, 12 triangles\n", out.display())
    );
    let glb = Glb::read(&out)?;
    assert_eq!(glb.vertex_count(), 24);
    assert_eq!(glb.triangle_count(), 12);
    let positions = glb.positions(0);
    for axis in 0..3 {
        let max = positions.iter().map(|p| p[axis]).fold(f32::MIN, f32::max);
        assert_eq!(max, 4.0);
    }
    Ok(())
}

#[test]
fn two_materials_split_the_merge() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(
        &dir,
        "split.mcvox",
        &single_part([4, 4, 4], 4, |x, _, _| if x < 2 { 'r' } else { 'b' }),
    )?;
    let glb = exported(&doc, &materials, &dir.path().join("split.glb"), &["--mesh", "merged"])?;
    // -x all red, +x all blue, the other four sides split in two.
    assert_eq!(glb.vertex_count(), 10 * 4);
    let colors = glb.colors(0);
    assert!(colors.contains(&vec![1.0, 0.0, 0.0, 1.0]));
    assert!(colors.contains(&vec![0.0, 0.0, 1.0, 1.0]));
    Ok(())
}

#[test]
fn faces_between_parts_are_kept() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "two.mcvox", TWO_PARTS)?;
    let glb = exported(&doc, &materials, &dir.path().join("two.glb"), &["--mesh", "merged"])?;
    assert_eq!(glb.mesh_count(), 2);
    assert_eq!(glb.vertex_count(), 12 * 4);
    // The body's +x face, at the seam with the arm, is present.
    let body = glb.positions(0);
    let normals = glb.normals(0);
    assert!(body.iter().zip(&normals).any(|(p, n)| n == &vec![1.0, 0.0, 0.0] && p[0] == 2.0));
    Ok(())
}

#[test]
fn block_unit_divides_by_scale() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "two.mcvox", TWO_PARTS)?;
    let voxel = exported(&doc, &materials, &dir.path().join("voxel.glb"), &[])?;
    let block = exported(&doc, &materials, &dir.path().join("block.glb"), &["--unit", "block"])?;
    for mesh in 0..2 {
        let (v, b) = (voxel.positions(mesh), block.positions(mesh));
        assert_eq!(v.len(), b.len());
        for (pv, pb) in v.iter().zip(&b) {
            for axis in 0..3 {
                assert_eq!(pb[axis], pv[axis] / 4.0);
            }
        }
        let acc = &block.json["accessors"][block.attribute(mesh, "POSITION")];
        let max: Vec<f64> = acc["max"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        let expected_x = if mesh == 0 { 0.5 } else { 0.25 };
        assert_eq!(max, vec![expected_x, 0.25, 0.25]);
    }
    let arm_translation = &block.json["nodes"][1]["translation"];
    assert_eq!(arm_translation, &serde_json::json!([0.5, 0.0, 0.0]));

    let cube = document_at(&dir, "cube.mcvox", &single_part([8, 8, 8], 8, |_, _, _| 'r'))?;
    let merged = exported(&cube, &materials, &dir.path().join("cube.glb"), &["--mesh", "merged", "--unit", "block"])?;
    let acc = &merged.json["accessors"][merged.attribute(0, "POSITION")];
    assert_eq!(acc["max"], serde_json::json!([1.0, 1.0, 1.0]));
    assert_eq!(acc["min"], serde_json::json!([0.0, 0.0, 0.0]));
    Ok(())
}

fn f32_at(value: &Value) -> f32 {
    value.as_f64().unwrap() as f32
}

#[test]
fn merged_block_units_hold_through_three_levels() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "three.mcvox", &three_levels())?;
    let voxel = exported(&doc, &materials, &dir.path().join("voxel.glb"), &["--mesh", "merged"])?;
    let block = exported(&doc, &materials, &dir.path().join("block.glb"), &["--mesh", "merged", "--unit", "block"])?;

    // The chain is intact in both: body > arm > hand.
    for glb in [&voxel, &block] {
        assert_eq!(glb.mesh_count(), 3);
        assert_eq!(glb.json["scenes"][0]["nodes"], serde_json::json!([0]));
        assert_eq!(glb.json["nodes"][0]["children"], serde_json::json!([1]));
        assert_eq!(glb.json["nodes"][1]["children"], serde_json::json!([2]));
        assert!(glb.json["nodes"][2].get("children").is_none());
    }

    for mesh in 0..3 {
        let (v, b) = (voxel.positions(mesh), block.positions(mesh));
        assert_eq!(v.len(), b.len());
        assert!(!v.is_empty());
        for (pv, pb) in v.iter().zip(&b) {
            for axis in 0..3 {
                assert_eq!(pb[axis], pv[axis] / 10.0);
            }
        }
    }
    // Spot values that are not exact in binary: a 2-voxel hand is 0.2 blocks.
    let hand = &block.json["accessors"][block.attribute(2, "POSITION")];
    let max: Vec<f32> = hand["max"].as_array().unwrap().iter().map(f32_at).collect();
    assert_eq!(max, vec![2.0 / 10.0, 1.0 / 10.0, 1.0 / 10.0]);

    // Each child sits at its own attach point in its parent, not summed up the chain.
    for (node, voxels) in [(1, 2.0f32), (2, 3.0f32)] {
        let at_voxels: Vec<f32> = voxel.json["nodes"][node]["translation"].as_array().unwrap().iter().map(f32_at).collect();
        let at_blocks: Vec<f32> = block.json["nodes"][node]["translation"].as_array().unwrap().iter().map(f32_at).collect();
        assert_eq!(at_voxels, vec![voxels, 0.0, 0.0]);
        assert_eq!(at_blocks, at_voxels.iter().map(|t| t / 10.0).collect::<Vec<_>>());
        assert_eq!(at_blocks, vec![voxels / 10.0, 0.0, 0.0]);
    }
    assert_eq!(f32_at(&block.json["nodes"][1]["translation"][0]), 0.2);
    assert_eq!(f32_at(&block.json["nodes"][2]["translation"][0]), 0.3);
    Ok(())
}

#[test]
fn linear_colors_apply_the_srgb_transfer_function() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(
        &dir,
        "grey.mcvox",
        &single_part([2, 1, 1], 1, |x, _, _| if x == 0 { 'g' } else { 'r' }),
    )?;
    let srgb = exported(&doc, &materials, &dir.path().join("srgb.glb"), &[])?;
    let linear = exported(&doc, &materials, &dir.path().join("linear.glb"), &["--colors", "linear"])?;
    let grey_srgb = srgb.colors(0)[0].clone();
    assert!((grey_srgb[0] - 128.0 / 255.0).abs() < 1e-7);
    let colors = linear.colors(0);
    let grey = &colors[0];
    // ((128/255 + 0.055) / 1.055) ^ 2.4
    for c in &grey[..3] {
        assert!((c - 0.215_860_5).abs() < 1e-6, "got {c}");
    }
    assert_eq!(grey[3], 1.0);
    assert_eq!(colors[colors.len() - 1], vec![1.0, 0.0, 0.0, 1.0]);
    Ok(())
}

#[test]
fn glb_options_are_refused_for_other_formats() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "two.mcvox", TWO_PARTS)?;
    let options: [&[&str]; 3] = [&["--mesh", "merged"], &["--unit", "block"], &["--colors", "linear"]];
    for ext in ["obj", "vox"] {
        for option in options {
            let out = dir.path().join(format!("refused.{ext}"));
            let run = export(&doc, &materials, &out, option)?;
            assert_eq!(run.code, ExitCode::Defective, "{ext} {option:?}");
            assert!(run.out.is_empty());
            assert!(run.err.contains("applies only to .glb"), "stderr: {}", run.err);
            assert!(run.err.contains(&format!("field `{}`", &option[0][2..])), "stderr: {}", run.err);
            assert!(!out.exists(), "{} was written", out.display());
            assert!(!dir.path().join("refused.mtl").exists());
        }
        // Defaults spelled out are still accepted.
        let out = dir.path().join(format!("fine.{ext}"));
        let run = export(&doc, &materials, &out, &["--mesh", "cubes", "--unit", "voxel", "--colors", "srgb"])?;
        assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
    }
    Ok(())
}

/// An irregular two-material shape with holes and overhangs.
fn irregular(x: u32, y: u32, z: u32) -> char {
    let h = (x * 7 + y * 13 + z * 5 + x * y * z) % 11;
    match h {
        0..=3 => '.',
        4..=7 => 'r',
        _ => 'b',
    }
}

/// A second irregular shape, in grey and red.
fn irregular_arm(x: u32, y: u32, z: u32) -> char {
    match (x * 5 + y * 3 + z * 11 + x * z) % 7 {
        0..=1 => '.',
        2..=4 => 'g',
        _ => 'r',
    }
}

type Shape = fn(u32, u32, u32) -> char;

/// One part of [`multi_part`]: name, size, what it attaches to and where, shape.
type PartSpec<'a> = (&'a str, [u32; 3], Option<(&'a str, [u32; 3])>, Shape);

/// A `y`-sliced model of `parts`, all with origin zero.
fn multi_part(scale: u32, parts: &[PartSpec]) -> String {
    let mut text = format!(
        "schema = 1\nname = \"test:multi\"\nscale = {scale}\nslice = \"y\"\n\n[palette]\n\".\" = \"empty\"\n\"r\" = \"test:red\"\n\"b\" = \"test:blue\"\n\"g\" = \"test:grey\"\n"
    );
    for (name, size, attach, _) in parts {
        text.push_str(&format!("\n[[parts]]\nname = \"{name}\"\nsize = [{}, {}, {}]\norigin = [0, 0, 0]\n", size[0], size[1], size[2]));
        if let Some((to, at)) = attach {
            text.push_str(&format!("attach = {{ to = \"{to}\", at = [{}, {}, {}] }}\n", at[0], at[1], at[2]));
        }
    }
    for (name, size, _, at) in parts {
        for y in 0..size[1] {
            text.push_str(&format!("\n[[layers]]\npart = \"{name}\"\ny = {y}\ngrid = \"\"\"\n"));
            for z in 0..size[2] {
                let row: String = (0..size[0]).map(|x| at(x, y, z)).collect();
                text.push_str(&row);
                text.push('\n');
            }
            text.push_str("\"\"\"\n");
        }
    }
    text
}

fn colour_char(colour: &[f32]) -> char {
    if colour == [1.0, 0.0, 0.0, 1.0] {
        'r'
    } else if colour == [0.0, 0.0, 1.0, 1.0] {
        'b'
    } else if colour[..3].iter().all(|c| (c - 128.0 / 255.0).abs() < 1e-6) && colour[3] == 1.0 {
        'g'
    } else {
        panic!("unexpected colour {colour:?}")
    }
}

/// Exposed face area keyed by (part, normal, plane coordinate along it, colour).
type AreaKey = (usize, [i64; 3], i64, char);

/// Merged output of `parts` covers exactly the faces not hidden by a voxel of
/// the same part, on the same plane and in the same colour.
fn assert_merged_area_matches(parts: &[PartSpec]) -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "multi.mcvox", &multi_part(4, parts))?;
    let cubes = exported(&doc, &materials, &dir.path().join("cubes.glb"), &[])?;
    let glb = exported(&doc, &materials, &dir.path().join("merged.glb"), &["--mesh", "merged"])?;
    assert_eq!(glb.mesh_count(), parts.len());

    let directions: [[i64; 3]; 6] = [[0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0], [1, 0, 0], [-1, 0, 0]];
    let mut exposed: BTreeMap<AreaKey, i64> = BTreeMap::new();
    for (part, (_, size, _, shape)) in parts.iter().enumerate() {
        let filled = |x: i64, y: i64, z: i64| {
            x >= 0 && y >= 0 && z >= 0
                && x < i64::from(size[0]) && y < i64::from(size[1]) && z < i64::from(size[2])
                && shape(x as u32, y as u32, z as u32) != '.'
        };
        for x in 0..i64::from(size[0]) {
            for y in 0..i64::from(size[1]) {
                for z in 0..i64::from(size[2]) {
                    if !filled(x, y, z) {
                        continue;
                    }
                    let colour = shape(x as u32, y as u32, z as u32);
                    for d in directions {
                        if !filled(x + d[0], y + d[1], z + d[2]) {
                            let axis = (0..3).find(|a| d[*a] != 0).unwrap();
                            let at = [x, y, z][axis] + i64::from(d[axis] > 0);
                            *exposed.entry((part, d, at, colour)).or_default() += 1;
                        }
                    }
                }
            }
        }
    }

    let mut area: BTreeMap<AreaKey, i64> = BTreeMap::new();
    for part in 0..parts.len() {
        let positions = glb.positions(part);
        let normals = glb.normals(part);
        let colours = glb.colors(part);
        for (quad, corners) in positions.chunks(4).enumerate() {
            let n = &normals[quad * 4];
            let e1: Vec<f64> = (0..3).map(|a| f64::from(corners[1][a] - corners[0][a])).collect();
            let e2: Vec<f64> = (0..3).map(|a| f64::from(corners[3][a] - corners[0][a])).collect();
            let cross = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            // Counter-clockwise from outside: the cross product points along the normal.
            let along = cross[0] * f64::from(n[0]) + cross[1] * f64::from(n[1]) + cross[2] * f64::from(n[2]);
            assert!(along > 0.0, "part {part} quad {quad} is wound inwards");
            let key = [n[0] as i64, n[1] as i64, n[2] as i64];
            let axis = (0..3).find(|a| key[*a] != 0).unwrap();
            let plane = corners[0][axis];
            assert!(corners.iter().all(|c| c[axis] == plane && c[axis].fract() == 0.0));
            let colour = colour_char(&colours[quad * 4]);
            assert!(colours[quad * 4..quad * 4 + 4].iter().all(|c| colour_char(c) == colour));
            *area.entry((part, key, plane as i64, colour)).or_default() += along.round() as i64;
        }
    }
    assert_eq!(area, exposed);
    assert!(glb.vertex_count() < cubes.vertex_count());
    Ok(())
}

#[test]
fn merged_quad_area_matches_exposed_faces() -> TestResult {
    assert_merged_area_matches(&[("body", [5, 4, 6], None, irregular as Shape)])
}

#[test]
fn merged_quad_area_matches_exposed_faces_of_two_parts() -> TestResult {
    assert_merged_area_matches(&[
        ("body", [5, 4, 6], None, irregular as Shape),
        ("arm", [3, 4, 3], Some(("body", [5, 0, 0])), irregular_arm as Shape),
    ])
}

#[test]
fn merged_output_is_deterministic() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "irregular.mcvox", &single_part([5, 4, 6], 4, irregular))?;
    let first = dir.path().join("first.glb");
    let second = dir.path().join("second.glb");
    let flags = ["--mesh", "merged", "--unit", "block", "--colors", "linear"];
    exported(&doc, &materials, &first, &flags)?;
    exported(&doc, &materials, &second, &flags)?;
    assert_eq!(fs::read(&first)?, fs::read(&second)?);
    Ok(())
}

// ---- --ao vertex ----

fn assert_close(actual: f32, expected: f32, what: &str) {
    assert!((actual - expected).abs() < 1e-6, "{what}: got {actual}, expected {expected}");
}

/// `1 - strength * level / 3`
fn factor(strength: f32, level: u32) -> f32 {
    1.0 - strength * level as f32 / 3.0
}

const MODES: [&str; 2] = ["cubes", "merged"];

fn ao_flags<'a>(mode: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut flags = vec!["--mesh", mode, "--ao", "vertex"];
    flags.extend_from_slice(extra);
    flags
}

#[test]
fn ao_off_equals_no_option() -> TestResult {
    let (dir, materials) = workspace()?;
    let docs = [
        document_at(&dir, "two.mcvox", TWO_PARTS)?,
        document_at(&dir, "irregular.mcvox", &single_part([5, 4, 6], 4, irregular))?,
    ];
    for doc in &docs {
        for mode in MODES {
            let plain = dir.path().join("plain.glb");
            let off = dir.path().join("off.glb");
            let off_strength = dir.path().join("off_strength.glb");
            export(doc, &materials, &plain, &["--mesh", mode, "--colors", "linear"])?;
            let run = export(doc, &materials, &off, &["--mesh", mode, "--colors", "linear", "--ao", "off"])?;
            assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
            let run = export(
                doc,
                &materials,
                &off_strength,
                &["--mesh", mode, "--colors", "linear", "--ao", "off", "--ao-strength", "0.9"],
            )?;
            assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
            assert_eq!(fs::read(&plain)?, fs::read(&off)?, "{mode}");
            assert_eq!(fs::read(&plain)?, fs::read(&off_strength)?, "{mode}");
        }
    }
    Ok(())
}

#[test]
fn ao_on_a_single_voxel_changes_nothing() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "one.mcvox", &single_part([1, 1, 1], 1, |_, _, _| 'r'))?;
    for mode in MODES {
        let off = dir.path().join("off.glb");
        let on = dir.path().join("on.glb");
        export(&doc, &materials, &off, &["--mesh", mode])?;
        let glb = exported(&doc, &materials, &on, &ao_flags(mode, &["--ao-strength", "1"]))?;
        assert!(glb.colors(0).iter().all(|c| c == &vec![1.0, 0.0, 0.0, 1.0]), "{mode}");
        assert_eq!(fs::read(&off)?, fs::read(&on)?, "{mode}");
    }
    Ok(())
}

/// Index of the lone 1x1 top quad over voxel `(x, 0, z)` whose plane is `y = 1`.
fn top_quad(glb: &Glb, x: u32, z: u32) -> usize {
    let (positions, normals) = (glb.positions(0), glb.normals(0));
    let (x, z) = (x as f32, z as f32);
    (0..positions.len() / 4)
        .find(|q| {
            let corners = &positions[q * 4..q * 4 + 4];
            normals[q * 4] == vec![0.0, 1.0, 0.0]
                && corners.iter().all(|c| c[1] == 1.0 && (x..=x + 1.0).contains(&c[0]) && (z..=z + 1.0).contains(&c[2]))
                && corners.iter().any(|c| c[0] == x && c[2] == z)
                && corners.iter().any(|c| c[0] == x + 1.0 && c[2] == z + 1.0)
        })
        .unwrap_or_else(|| panic!("no top quad over ({x}, 0, {z})"))
}

/// The vertex colour factor (red channel of a red material) at corner `(cx, cz)`
/// of the 1x1 top quad over voxel `(x, 0, z)`.
fn top_corner_factor(glb: &Glb, (x, z): (u32, u32), (cx, cz): (u32, u32)) -> f32 {
    let quad = top_quad(glb, x, z);
    let corners = &glb.positions(0)[quad * 4..quad * 4 + 4];
    let k = corners.iter().position(|c| c[0] == cx as f32 && c[2] == cz as f32).expect("corner");
    let c = &glb.colors(0)[quad * 4 + k];
    assert!(c[1] == 0.0 && c[2] == 0.0 && c[3] == 1.0, "{c:?}");
    c[0]
}

/// 3x3 floor with `(1,1,0)` and `(1,1,1)` on it.
fn step(x: u32, y: u32, z: u32) -> char {
    if y == 0 || (x == 1 && y == 1 && z <= 1) { 'r' } else { '.' }
}

/// 3x3 floor with `(1,1,0)` and `(0,1,1)` on it; `(1,1,1)` stays empty.
fn notch(x: u32, y: u32, z: u32) -> char {
    if y == 0 || (y == 1 && ((x, z) == (1, 0) || (x, z) == (0, 1))) { 'r' } else { '.' }
}

#[test]
fn ao_levels_at_known_corners() -> TestResult {
    let (dir, materials) = workspace()?;
    let step_doc = document_at(&dir, "step.mcvox", &single_part([3, 2, 3], 1, step))?;
    let notch_doc = document_at(&dir, "notch.mcvox", &single_part([3, 2, 3], 1, notch))?;
    let s = 0.6;
    for mode in MODES {
        for colours in ["srgb", "linear"] {
            let flags = ao_flags(mode, &["--ao-strength", "0.6", "--colors", colours]);
            let glb = exported(&step_doc, &materials, &dir.path().join("step.glb"), &flags)?;
            // Level 2: edge neighbour (1,1,0) and diagonal (1,1,1) filled, other edge empty.
            assert_close(top_corner_factor(&glb, (2, 0), (2, 1)), factor(s, 2), "level 2");
            // Level 1: only the edge neighbour (1,1,0), or only the diagonal (1,1,1).
            assert_close(top_corner_factor(&glb, (2, 0), (2, 0)), factor(s, 1), "edge only");
            assert_close(top_corner_factor(&glb, (2, 2), (2, 2)), factor(s, 1), "diagonal only");
            // Level 0 away from the steps.
            assert_close(top_corner_factor(&glb, (2, 0), (3, 0)), 1.0, "free");
            assert_close(top_corner_factor(&glb, (2, 2), (3, 3)), 1.0, "free");

            let glb = exported(&notch_doc, &materials, &dir.path().join("notch.glb"), &flags)?;
            // Both edge neighbours filled, diagonal empty: 3, not 2.
            assert_close(top_corner_factor(&glb, (0, 0), (1, 1)), factor(s, 3), "both sides filled");
            assert_close(top_corner_factor(&glb, (0, 0), (1, 0)), factor(s, 1), "one side filled");
            assert_close(top_corner_factor(&glb, (0, 0), (0, 0)), 1.0, "free");
        }
    }
    Ok(())
}

#[test]
fn ao_strength_one_blackens_level_three() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "notch.mcvox", &single_part([3, 2, 3], 1, notch))?;
    for mode in MODES {
        let glb = exported(&doc, &materials, &dir.path().join("one.glb"), &ao_flags(mode, &["--ao-strength", "1"]))?;
        assert_eq!(top_corner_factor(&glb, (0, 0), (1, 1)), 0.0, "{mode}");
        assert_close(top_corner_factor(&glb, (0, 0), (1, 0)), 2.0 / 3.0, "level 1");
    }
    Ok(())
}

#[test]
fn ao_flips_the_diagonal_through_the_darker_corners() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "step.mcvox", &single_part([3, 2, 3], 1, step))?;
    let glb = exported(&doc, &materials, &dir.path().join("step.glb"), &ao_flags("cubes", &[]))?;
    let indices = glb.indices(0);
    // Voxel (2,0,2): corner levels [0, 0, 0, 1]; 0 + 0 < 0 + 1, so split along corners 1-3.
    let v = top_quad(&glb, 2, 2) * 4;
    assert_eq!(indices[v / 4 * 6..v / 4 * 6 + 6], [v + 1, v + 2, v + 3, v + 1, v + 3, v]);
    // Voxel (2,0,0): levels [2, 0, 0, 1]; 2 + 0 >= 0 + 1, so the default diagonal.
    let v = top_quad(&glb, 2, 0) * 4;
    assert_eq!(indices[v / 4 * 6..v / 4 * 6 + 6], [v, v + 1, v + 2, v, v + 2, v + 3]);
    Ok(())
}

/// Brute-force check of every vertex colour against the occlusion rule, derived
/// from each quad's own geometry. Also that quads larger than 1x1 are uniform.
fn assert_vertex_colours_follow_the_rule(glb: &Glb, size: [i64; 3], shape: Shape, strength: f32) {
    let filled = |p: [i64; 3]| {
        (0..3).all(|a| p[a] >= 0 && p[a] < size[a]) && shape(p[0] as u32, p[1] as u32, p[2] as u32) != '.'
    };
    let (positions, normals, vertex_colours) = (glb.positions(0), glb.normals(0), glb.colors(0));
    for quad in 0..positions.len() / 4 {
        let corners = &positions[quad * 4..quad * 4 + 4];
        let n = &normals[quad * 4];
        let axis = (0..3).find(|a| n[*a] != 0.0).unwrap();
        let dir = n[axis] as i64;
        let (u, v) = match axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let lo = |a: usize| corners.iter().map(|c| c[a]).fold(f32::MAX, f32::min);
        let hi = |a: usize| corners.iter().map(|c| c[a]).fold(f32::MIN, f32::max);
        let plane = corners[0][axis] as i64;
        let mut seen = Vec::new();
        for (k, c) in corners.iter().enumerate() {
            let mut cell = [0i64; 3];
            cell[axis] = if dir > 0 { plane - 1 } else { plane };
            let su = if c[u] == hi(u) { 1 } else { -1 };
            let sv = if c[v] == hi(v) { 1 } else { -1 };
            cell[u] = if su > 0 { hi(u) as i64 - 1 } else { lo(u) as i64 };
            cell[v] = if sv > 0 { hi(v) as i64 - 1 } else { lo(v) as i64 };
            let probe = |du: i64, dv: i64| {
                let mut p = cell;
                p[axis] += dir;
                p[u] += du;
                p[v] += dv;
                filled(p)
            };
            let (s1, s2, corner) = (probe(su, 0), probe(0, sv), probe(su, sv));
            let level = if s1 && s2 { 3 } else { u32::from(s1) + u32::from(s2) + u32::from(corner) };
            let expected: [f32; 3] = match shape(cell[0] as u32, cell[1] as u32, cell[2] as u32) {
                'r' => [1.0, 0.0, 0.0],
                'b' => [0.0, 0.0, 1.0],
                other => panic!("unexpected shape char {other}"),
            };
            let got = &vertex_colours[quad * 4 + k];
            for ch in 0..3 {
                assert_close(got[ch], expected[ch] * factor(strength, level), &format!("quad {quad} corner {k}"));
            }
            assert_eq!(got[3], 1.0);
            seen.push(level);
        }
        if (hi(u) - lo(u)) * (hi(v) - lo(v)) > 1.0 {
            assert!(seen.iter().all(|l| *l == seen[0]), "merged quad {quad} has differing levels {seen:?}");
        }
    }
}

#[test]
fn ao_matches_the_rule_at_every_vertex() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "irregular.mcvox", &single_part([5, 4, 6], 4, irregular))?;
    for mode in MODES {
        // Red and blue are 0 or 1 in both colour spaces, so the factor shows directly.
        for colours in ["srgb", "linear"] {
            let flags = ao_flags(mode, &["--ao-strength", "0.8", "--colors", colours]);
            let glb = exported(&doc, &materials, &dir.path().join("ao.glb"), &flags)?;
            assert_vertex_colours_follow_the_rule(&glb, [5, 4, 6], irregular, 0.8);
        }
    }
    Ok(())
}

#[test]
fn ao_darkens_in_the_written_colour_space() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "notch.mcvox", &single_part([3, 2, 3], 1, |x, y, z| if y == 0 && x + z == 0 { 'g' } else { notch(x, y, z) }))?;
    for (colours, base) in [("srgb", 128.0f32 / 255.0), ("linear", 0.215_860_5)] {
        let glb = exported(&doc, &materials, &dir.path().join("g.glb"), &ao_flags("cubes", &["--ao-strength", "0.6", "--colors", colours]))?;
        let quad = top_quad(&glb, 0, 0);
        let k = glb.positions(0)[quad * 4..quad * 4 + 4].iter().position(|c| c[0] == 1.0 && c[2] == 1.0).unwrap();
        let c = &glb.colors(0)[quad * 4 + k];
        for ch in &c[..3] {
            assert_close(*ch, base * factor(0.6, 3), colours);
        }
    }
    Ok(())
}

/// Exposed area keyed by (normal, plane, material) of a shape.
fn exposed_area(size: [i64; 3], shape: Shape) -> BTreeMap<([i64; 3], i64, char), i64> {
    let filled = |p: [i64; 3]| {
        (0..3).all(|a| p[a] >= 0 && p[a] < size[a]) && shape(p[0] as u32, p[1] as u32, p[2] as u32) != '.'
    };
    let mut exposed = BTreeMap::new();
    for x in 0..size[0] {
        for y in 0..size[1] {
            for z in 0..size[2] {
                if !filled([x, y, z]) {
                    continue;
                }
                for axis in 0..3 {
                    for dir in [-1i64, 1] {
                        let mut next = [x, y, z];
                        next[axis] += dir;
                        if !filled(next) {
                            let mut normal = [0; 3];
                            normal[axis] = dir;
                            let plane = [x, y, z][axis] + i64::from(dir > 0);
                            *exposed.entry((normal, plane, shape(x as u32, y as u32, z as u32))).or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    exposed
}

/// The same keys from an export whose colours may be darkened (red has a
/// non-zero red channel, blue does not; strength stays below 1).
fn exported_area(glb: &Glb) -> BTreeMap<([i64; 3], i64, char), i64> {
    let (positions, normals, colours) = (glb.positions(0), glb.normals(0), glb.colors(0));
    let mut area = BTreeMap::new();
    for quad in 0..positions.len() / 4 {
        let corners = &positions[quad * 4..quad * 4 + 4];
        let n = &normals[quad * 4];
        let normal = [n[0] as i64, n[1] as i64, n[2] as i64];
        let axis = (0..3).find(|a| normal[*a] != 0).unwrap();
        let extent = |a: usize| {
            corners.iter().map(|c| c[a]).fold(f32::MIN, f32::max) - corners.iter().map(|c| c[a]).fold(f32::MAX, f32::min)
        };
        let size: f32 = (0..3).filter(|a| *a != axis).map(extent).product();
        let colour = if colours[quad * 4][0] > 0.0 { 'r' } else { 'b' };
        *area.entry((normal, corners[0][axis] as i64, colour)).or_default() += size.round() as i64;
    }
    area
}

#[test]
fn merged_ao_keeps_the_exposed_area() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "irregular.mcvox", &single_part([5, 4, 6], 4, irregular))?;
    let ao = exported(&doc, &materials, &dir.path().join("ao.glb"), &ao_flags("merged", &[]))?;
    let off = exported(&doc, &materials, &dir.path().join("off.glb"), &["--mesh", "merged"])?;
    assert_eq!(exported_area(&ao), exposed_area([5, 4, 6], irregular));
    assert_eq!(exported_area(&off), exposed_area([5, 4, 6], irregular));
    // Faces only merge within one level, so never fewer quads than without AO.
    assert!(ao.vertex_count() >= off.vertex_count());
    Ok(())
}

#[test]
fn merged_slab_splits_only_around_the_foot_of_a_voxel() -> TestResult {
    let (dir, materials) = workspace()?;
    let top_quads = |glb: &Glb| {
        let normals = glb.normals(0);
        (0..normals.len() / 4).filter(|q| normals[q * 4] == vec![0.0, 1.0, 0.0]).collect::<Vec<_>>()
    };

    let slab = document_at(&dir, "slab.mcvox", &single_part([4, 1, 4], 4, |_, _, _| 'r'))?;
    let glb = exported(&slab, &materials, &dir.path().join("slab.glb"), &ao_flags("merged", &[]))?;
    assert_eq!(top_quads(&glb).len(), 1);
    assert_eq!(glb.vertex_count(), 24);

    let foot = |x: u32, y: u32, z: u32| if y == 0 || (x, y, z) == (1, 1, 1) { 'r' } else { '.' };
    let doc = document_at(&dir, "foot.mcvox", &single_part([4, 2, 4], 4, foot))?;
    let glb = exported(&doc, &materials, &dir.path().join("foot.glb"), &ao_flags("merged", &[]))?;
    let (positions, colours) = (glb.positions(0), glb.colors(0));
    let tops = top_quads(&glb);
    let bounds = |q: &usize| {
        let corners = &positions[q * 4..q * 4 + 4];
        let min = |a: usize| corners.iter().map(|c| c[a]).fold(f32::MAX, f32::min);
        let max = |a: usize| corners.iter().map(|c| c[a]).fold(f32::MIN, f32::max);
        (min(0), min(2), max(0), max(2), corners[0][1])
    };
    // The eight slab voxels around the foot keep lone 1x1 top quads.
    for x in 0..3 {
        for z in 0..3 {
            if (x, z) != (1, 1) {
                let (x, z) = (x as f32, z as f32);
                assert!(tops.iter().any(|q| bounds(q) == (x, z, x + 1.0, z + 1.0, 1.0)), "({x}, {z}) merged");
            }
        }
    }
    // Eight ring quads, the foot's own top, and the level-0 L merged into two.
    assert_eq!(tops.len(), 8 + 1 + 2);
    for q in &tops {
        let (x0, z0, x1, z1, _) = bounds(q);
        if (x1 - x0) * (z1 - z0) > 1.0 {
            assert!(colours[q * 4..q * 4 + 4].iter().all(|c| c == &vec![1.0, 0.0, 0.0, 1.0]));
        }
    }
    assert_eq!(exported_area(&glb), exposed_area([4, 2, 4], foot));
    Ok(())
}

#[test]
fn ao_strength_zero_keeps_the_material_colours() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "irregular.mcvox", &single_part([5, 4, 6], 4, irregular))?;
    // Cubes: same vertices in the same order, so positions and colours are
    // identical; only the triangle diagonals may differ.
    let off = exported(&doc, &materials, &dir.path().join("off.glb"), &[])?;
    let zero = exported(&doc, &materials, &dir.path().join("zero.glb"), &["--ao", "vertex", "--ao-strength", "0"])?;
    assert_eq!(off.colors(0), zero.colors(0));
    assert_eq!(off.positions(0), zero.positions(0));
    // Merged: the geometry differs (faces no longer merge across levels), so
    // only the colours can be compared: each is a plain material colour.
    let zero = exported(&doc, &materials, &dir.path().join("zero_m.glb"), &["--mesh", "merged", "--ao", "vertex", "--ao-strength", "0"])?;
    let off = exported(&doc, &materials, &dir.path().join("off_m.glb"), &["--mesh", "merged"])?;
    let palette = off.colors(0);
    assert!(zero.colors(0).iter().all(|c| palette.contains(c)));
    Ok(())
}

#[test]
fn ao_strength_out_of_range_is_refused_without_writing() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "two.mcvox", TWO_PARTS)?;
    for ext in ["glb", "obj", "vox"] {
        for bad in ["-0.1", "1.5", "2", "nan", "inf"] {
            for extra in [&["--ao", "vertex"][..], &[][..], &["--ao", "off"][..]] {
                let out = dir.path().join(format!("bad.{ext}"));
                let mut flags = extra.to_vec();
                flags.extend_from_slice(&["--ao-strength", bad]);
                let run = export(&doc, &materials, &out, &flags)?;
                assert_eq!(run.code, ExitCode::Defective, "{ext} {bad} {extra:?}");
                assert!(run.out.is_empty());
                assert!(run.err.contains("field `ao-strength`"), "stderr: {}", run.err);
                assert!(run.err.contains("outside 0.0 to 1.0"), "stderr: {}", run.err);
                assert!(!out.exists(), "{} was written", out.display());
            }
        }
    }
    for edge in ["0", "1", "0.0", "1.0"] {
        let run = export(&doc, &materials, &dir.path().join("edge.glb"), &["--ao", "vertex", "--ao-strength", edge])?;
        assert_eq!(run.code, ExitCode::Success, "{edge}: {}", run.err);
    }
    Ok(())
}

#[test]
fn ao_options_are_refused_for_other_formats() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "two.mcvox", TWO_PARTS)?;
    let options: [(&[&str], &str); 2] = [(&["--ao", "vertex"], "ao"), (&["--ao-strength", "0.7"], "ao-strength")];
    for ext in ["obj", "vox"] {
        for (option, field) in options {
            let out = dir.path().join(format!("refused.{ext}"));
            let run = export(&doc, &materials, &out, option)?;
            assert_eq!(run.code, ExitCode::Defective, "{ext} {option:?}");
            assert!(run.out.is_empty());
            assert!(run.err.contains("applies only to .glb"), "stderr: {}", run.err);
            assert!(run.err.contains(&format!("field `{field}`")), "stderr: {}", run.err);
            assert!(!out.exists(), "{} was written", out.display());
        }
        // Defaults spelled out are still accepted.
        let out = dir.path().join(format!("fine.{ext}"));
        let run = export(&doc, &materials, &out, &["--ao", "off", "--ao-strength", "0.5"])?;
        assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
    }
    Ok(())
}

#[test]
fn ao_output_is_deterministic() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "irregular.mcvox", &single_part([5, 4, 6], 4, irregular))?;
    for mode in MODES {
        let flags = ao_flags(mode, &["--ao-strength", "0.7", "--unit", "block", "--colors", "linear"]);
        let (first, second) = (dir.path().join("first.glb"), dir.path().join("second.glb"));
        exported(&doc, &materials, &first, &flags)?;
        exported(&doc, &materials, &second, &flags)?;
        assert_eq!(fs::read(&first)?, fs::read(&second)?, "{mode}");
    }
    Ok(())
}

#[test]
fn ao_triangles_face_outwards_including_flipped_quads() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(
        &dir,
        "multi.mcvox",
        &multi_part(
            4,
            &[
                ("body", [5, 4, 6], None, irregular as Shape),
                ("arm", [3, 4, 3], Some(("body", [5, 0, 0])), irregular_arm as Shape),
            ],
        ),
    )?;
    for mode in MODES {
        let glb = exported(&doc, &materials, &dir.path().join("ao.glb"), &ao_flags(mode, &["--ao-strength", "1"]))?;
        let mut flipped = 0;
        for mesh in 0..glb.mesh_count() {
            let (positions, normals, indices) = (glb.positions(mesh), glb.normals(mesh), glb.indices(mesh));
            assert_eq!(indices.len(), positions.len() / 4 * 6);
            for (quad, tris) in indices.chunks(6).enumerate() {
                if tris[0] == quad * 4 + 1 {
                    flipped += 1;
                }
                for t in tris.chunks(3) {
                    assert!(t.iter().all(|i| i / 4 == quad), "triangle leaves its quad");
                    let p: Vec<Vec<f64>> = t.iter().map(|i| positions[*i].iter().map(|c| f64::from(*c)).collect()).collect();
                    let e1: Vec<f64> = (0..3).map(|a| p[1][a] - p[0][a]).collect();
                    let e2: Vec<f64> = (0..3).map(|a| p[2][a] - p[0][a]).collect();
                    let cross = [
                        e1[1] * e2[2] - e1[2] * e2[1],
                        e1[2] * e2[0] - e1[0] * e2[2],
                        e1[0] * e2[1] - e1[1] * e2[0],
                    ];
                    let n = &normals[t[0]];
                    let dot = cross[0] * f64::from(n[0]) + cross[1] * f64::from(n[1]) + cross[2] * f64::from(n[2]);
                    assert!(dot > 0.0, "{mode} mesh {mesh} quad {quad}: triangle {t:?} faces inwards");
                }
            }
        }
        assert!(flipped > 0, "{mode}: the model has no flipped quad");
    }
    Ok(())
}
