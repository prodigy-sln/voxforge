//! `--max-extent`: raising the 64-voxel limit, opt-in.
//!
//! Without the option nothing changes, including the refusal at 65 voxels and
//! its wording. With it, parts and assembled models may reach the chosen limit
//! on every axis, and every command that loads a document honours it.

mod common;

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use common::TestResult;
use common::cli::{Filled, document_at, filled_in, invoke, material_file, materials_at};
use serde_json::Value;
use tempfile::TempDir;
use voxforge::inspect::ExitCode;

/// A directory holding red and blue materials.
fn workspace() -> Result<(TempDir, PathBuf), Box<dyn Error>> {
    let dir = TempDir::new()?;
    let materials = materials_at(
        &dir,
        "materials",
        &[
            ("blue.toml", material_file("test:blue", "#0000ff", "0.0")),
            ("red.toml", material_file("test:red", "#ff0000", "0.0")),
        ],
    )?;
    Ok((dir, materials))
}

const HEADER: &str = "schema = 1\nname = \"test:tall\"\nscale = 4\nslice = \"y\"\n";

/// The palette; the empty marker only where a grid spells it, since an entry no
/// grid uses is a defect `inspect` reports.
fn palette(with_empty: bool, with_blue: bool) -> String {
    let empty = if with_empty { "\".\" = \"empty\"\n" } else { "" };
    let blue = if with_blue { "\"b\" = \"test:blue\"\n" } else { "" };
    format!("\n[palette]\n{empty}\"r\" = \"test:red\"\n{blue}")
}

/// One solid `4 x height x 4` column of `ch`, as the layers of `part`.
fn column_layers(part: Option<&str>, height: u32, ch: char) -> String {
    let mut text = String::new();
    for y in 0..height {
        text.push_str("\n[[layers]]\n");
        if let Some(part) = part {
            text.push_str(&format!("part = \"{part}\"\n"));
        }
        text.push_str(&format!("y = {y}\ngrid = \"\"\"\n"));
        for _ in 0..4 {
            text.push_str(&format!("{ch}{ch}{ch}{ch}\n"));
        }
        text.push_str("\"\"\"\n");
    }
    text
}

/// Red legs of `lower` voxels with blue torso of `upper` voxels attached on top.
fn two_parts(lower: u32, upper: u32) -> String {
    let mut text = String::from(HEADER);
    text.push_str(&palette(false, true));
    text.push_str(&format!("\n[[parts]]\nname = \"legs\"\nsize = [4, {lower}, 4]\norigin = [0, 0, 0]\n"));
    text.push_str(&format!(
        "\n[[parts]]\nname = \"torso\"\nsize = [4, {upper}, 4]\norigin = [0, 0, 0]\nattach = {{ to = \"legs\", at = [0, {lower}, 0] }}\n"
    ));
    text.push_str(&column_layers(Some("legs"), lower, 'r'));
    text.push_str(&column_layers(Some("torso"), upper, 'b'));
    text
}

/// One part, `height` voxels tall, written with all its layers.
fn one_part(height: u32) -> String {
    let mut text = String::from(HEADER);
    text.push_str(&format!("size = [4, {height}, 4]\norigin = [0, 0, 0]\n"));
    text.push_str(&palette(false, false));
    text.push_str(&column_layers(None, height, 'r'));
    text
}

/// One part declaring `height` and no art, which the header pass refuses first.
fn declared_only(height: u32) -> String {
    format!("{HEADER}size = [4, {height}, 4]\norigin = [0, 0, 0]\n{}", palette(false, false))
}

fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn assert_refused(run: &common::cli::Invocation, expected: &str) {
    assert_eq!(run.code, ExitCode::Defective, "stdout: {}", run.out);
    assert_eq!(run.out, "");
    assert!(run.err.contains(expected), "stderr was: {}", run.err);
}

#[test]
fn default_refuses_a_part_declaring_sixty_five_with_the_unchanged_message() -> TestResult {
    let dir = TempDir::new()?;
    let doc = document_at(&dir, "m.mcvox", &declared_only(65))?;
    let run = invoke(&["inspect", path(&doc)])?;
    assert_refused(&run, "on axis y the declared size is 65, but the limit is 64 voxels");
    Ok(())
}

#[test]
fn default_refuses_an_assembled_model_of_sixty_five_with_the_unchanged_message() -> TestResult {
    let dir = TempDir::new()?;
    let doc = document_at(&dir, "m.mcvox", &two_parts(33, 32))?;
    let run = invoke(&["inspect", path(&doc)])?;
    assert_refused(&run, "the assembled model reaches 65 voxels on axis y, but the limit is 64 voxels on any axis");
    Ok(())
}

#[test]
fn an_explicit_sixty_four_is_the_default() -> TestResult {
    let dir = TempDir::new()?;
    let doc = document_at(&dir, "m.mcvox", &two_parts(33, 32))?;
    let run = invoke(&["inspect", path(&doc), "--max-extent", "64"])?;
    assert_refused(&run, "reaches 65 voxels on axis y, but the limit is 64 voxels on any axis");
    Ok(())
}

#[test]
fn default_refuses_eighty_voxels_in_every_command() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "m.mcvox", &two_parts(40, 40))?;
    let png = dir.path().join("p.png");
    let glb = dir.path().join("e.glb");
    let limit = "reaches 80 voxels on axis y, but the limit is 64 voxels on any axis";

    assert_refused(&invoke(&["inspect", path(&doc)])?, limit);
    assert_refused(&invoke(&["preview", path(&doc), "--out", path(&png), "--materials", path(&materials)])?, limit);
    assert_refused(&invoke(&["export", path(&doc), "--out", path(&glb), "--materials", path(&materials)])?, limit);
    assert!(!png.exists() && !glb.exists());
    Ok(())
}

#[test]
fn inspect_reads_an_eighty_voxel_model_when_the_limit_is_raised() -> TestResult {
    let (dir, _) = workspace()?;
    let doc = document_at(&dir, "m.mcvox", &two_parts(40, 40))?;
    let after = invoke(&["inspect", path(&doc), "--max-extent", "96"])?;
    assert_eq!(after.code, ExitCode::Success, "stderr: {}", after.err);
    assert_eq!(filled_in(&after.out), Filled::Voxels(2 * 4 * 4 * 40));
    assert!(after.out.contains("(3, 79, 3)"), "report: {}", after.out);

    let before = invoke(&["--max-extent", "96", "inspect", path(&doc)])?;
    assert_eq!(before.out, after.out);
    Ok(())
}

#[test]
fn preview_renders_an_eighty_voxel_model_when_the_limit_is_raised() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "m.mcvox", &two_parts(40, 40))?;
    let png = dir.path().join("p.png");
    let run = invoke(&[
        "preview", path(&doc), "--out", path(&png), "--materials", path(&materials),
        "--view", "front", "--pixels-per-voxel", "2", "--max-extent", "96",
    ])?;
    assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
    let image = image::load_from_memory(&fs::read(&png)?)?.to_rgba8();
    assert_eq!((image.width(), image.height()), (8, 160));
    let top = image.get_pixel(4, 2);
    let bottom = image.get_pixel(4, 157);
    assert!(top[2] > top[0] && top[3] == 255, "torso should be blue at the top: {top:?}");
    assert!(bottom[0] > bottom[2] && bottom[3] == 255, "legs should be red at the bottom: {bottom:?}");

    let sheet = dir.path().join("sheet.png");
    let run = invoke(&[
        "preview", path(&doc), "--out", path(&sheet), "--materials", path(&materials),
        "--pixels-per-voxel", "1", "--max-extent", "96",
    ])?;
    assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
    Ok(())
}

/// A model of two voxels at opposite corners of a cube of `edge`: `near` blue and
/// `far` red, in one part that declares the whole cube.
fn two_corners(edge: u32, near: [u32; 3], far: [u32; 3]) -> String {
    let mut text = String::from(HEADER);
    text.push_str(&format!("size = [{edge}, {edge}, {edge}]\norigin = [0, 0, 0]\n"));
    text.push_str(&palette(true, true));
    for (at, ch) in [(near, 'b'), (far, 'r')] {
        text.push_str(&format!("\n[[layers]]\ny = {}\ngrid = \"\"\"\n", at[1]));
        for z in 0..edge {
            let row: String = (0..edge).map(|x| if x == at[0] && z == at[2] { ch } else { '.' }).collect();
            text.push_str(&row);
            text.push('\n');
        }
        text.push_str("\"\"\"\n");
    }
    text
}

#[test]
fn every_corner_view_of_a_large_model_shows_the_voxel_nearest_the_viewer() -> TestResult {
    let (dir, materials) = workspace()?;
    let edge = 160;
    let last = edge - 1;
    // View name, and the direction signs it looks along: the nearest corner is
    // at the high end of every axis it looks down, and at the low end otherwise.
    let views: [(&str, [i32; 3]); 8] = [
        ("iso-fl", [1, -1, -1]),
        ("iso-fr", [-1, -1, -1]),
        ("iso-bl", [1, -1, 1]),
        ("iso-br", [-1, -1, 1]),
        ("iso-fl-under", [1, 1, -1]),
        ("iso-fr-under", [-1, 1, -1]),
        ("iso-bl-under", [1, 1, 1]),
        ("iso-br-under", [-1, 1, 1]),
    ];
    for (view, signs) in views {
        let near = signs.map(|s| if s < 0 { last } else { 0 });
        let far = signs.map(|s| if s < 0 { 0 } else { last });
        let doc = document_at(&dir, "corners.mcvox", &two_corners(edge, near, far))?;
        let png = dir.path().join("corners.png");
        let run = invoke(&[
            "preview", path(&doc), "--out", path(&png), "--materials", path(&materials),
            "--view", view, "--pixels-per-voxel", "1", "--max-extent", "256",
        ])?;
        assert_eq!(run.code, ExitCode::Success, "{view}: {}", run.err);
        let image = image::load_from_memory(&fs::read(&png)?)?.to_rgba8();
        let seen = |blue: bool| {
            image.pixels().any(|p| {
                p[3] == 255 && if blue { p[2] > p[0] } else { p[0] > p[2] }
            })
        };
        assert!(seen(true), "{view}: the nearest voxel is missing");
        assert!(!seen(false), "{view}: the far voxel shows through the near one");
    }
    Ok(())
}

fn glb_json_and_bin(path: &Path) -> Result<(Value, Vec<u8>), Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let json_len = word(12);
    let json: Value = serde_json::from_slice(&bytes[20..20 + json_len])?;
    let bin_start = 20 + json_len + 8;
    Ok((json, bytes[bin_start..bin_start + word(20 + json_len)].to_vec()))
}

/// The lowest and highest vertex of the whole scene, node translations applied.
fn glb_span(path: &Path) -> Result<([f32; 3], [f32; 3]), Box<dyn Error>> {
    let (json, bin) = glb_json_and_bin(path)?;
    let mut low = [f32::MAX; 3];
    let mut high = [f32::MIN; 3];
    for node in json["nodes"].as_array().unwrap() {
        let mesh = node["mesh"].as_u64().unwrap() as usize;
        let accessor = json["meshes"][mesh]["primitives"][0]["attributes"]["POSITION"].as_u64().unwrap() as usize;
        let acc = &json["accessors"][accessor];
        let view = &json["bufferViews"][acc["bufferView"].as_u64().unwrap() as usize];
        let offset = view["byteOffset"].as_u64().unwrap() as usize;
        let shift: Vec<f32> = node["translation"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
        for v in 0..acc["count"].as_u64().unwrap() as usize {
            for axis in 0..3 {
                let at = offset + (v * 3 + axis) * 4;
                let value = f32::from_le_bytes(bin[at..at + 4].try_into().unwrap()) + shift[axis];
                low[axis] = low[axis].min(value);
                high[axis] = high[axis].max(value);
            }
        }
    }
    Ok((low, high))
}

#[test]
fn export_writes_an_eighty_voxel_model_when_the_limit_is_raised() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "m.mcvox", &two_parts(40, 40))?;
    for mesh in ["cubes", "merged"] {
        let glb = dir.path().join(format!("{mesh}.glb"));
        let run = invoke(&[
            "export", path(&doc), "--out", path(&glb), "--materials", path(&materials),
            "--mesh", mesh, "--max-extent", "96",
        ])?;
        assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
        let (low, high) = glb_span(&glb)?;
        assert_eq!((low, high), ([0.0, 0.0, 0.0], [4.0, 80.0, 4.0]), "{mesh}");
    }
    let blocks = dir.path().join("blocks.glb");
    let run = invoke(&[
        "export", path(&doc), "--out", path(&blocks), "--materials", path(&materials),
        "--unit", "block", "--max-extent", "96",
    ])?;
    assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
    assert_eq!(glb_span(&blocks)?, ([0.0, 0.0, 0.0], [1.0, 20.0, 1.0]));
    Ok(())
}

#[test]
fn export_to_vox_and_obj_keeps_every_voxel_of_a_large_model() -> TestResult {
    let (dir, materials) = workspace()?;
    let doc = document_at(&dir, "m.mcvox", &two_parts(40, 40))?;
    let vox = dir.path().join("m.vox");
    let obj = dir.path().join("m.obj");
    for out in [&vox, &obj] {
        let run = invoke(&[
            "export", path(&doc), "--out", path(out), "--materials", path(&materials), "--max-extent", "96",
        ])?;
        assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
    }
    let data = dot_vox::load_bytes(&fs::read(&vox)?).map_err(|e| e.to_string())?;
    let model = &data.models[0];
    assert_eq!((model.size.x, model.size.y, model.size.z), (4, 4, 80));
    assert_eq!(model.voxels.len(), 2 * 4 * 4 * 40);
    assert_eq!(model.voxels.iter().map(|v| v.z).max(), Some(79));
    let vertices = fs::read_to_string(&obj)?.lines().filter(|l| l.starts_with("v ")).count();
    assert_eq!(vertices, 8 * 2 * 4 * 4 * 40);
    Ok(())
}

#[test]
fn a_part_above_the_chosen_limit_is_refused_even_when_the_model_would_fit() -> TestResult {
    let dir = TempDir::new()?;
    let doc = document_at(&dir, "m.mcvox", &declared_only(97))?;
    let run = invoke(&["inspect", path(&doc), "--max-extent", "96"])?;
    assert_refused(&run, "on axis y the declared size is 97, but the limit is 96 voxels");

    let doc = document_at(&dir, "part.mcvox", &declared_only(100))?;
    let run = invoke(&["inspect", path(&doc), "--max-extent", "96"])?;
    assert_refused(&run, "the declared size is 100, but the limit is 96 voxels");
    Ok(())
}

#[test]
fn a_part_at_the_chosen_limit_loads() -> TestResult {
    let dir = TempDir::new()?;
    let doc = document_at(&dir, "m.mcvox", &one_part(96))?;
    let run = invoke(&["inspect", path(&doc), "--max-extent", "96"])?;
    assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
    assert_eq!(filled_in(&run.out), Filled::Voxels(4 * 4 * 96));
    Ok(())
}

#[test]
fn an_assembled_model_above_the_chosen_limit_is_refused() -> TestResult {
    let dir = TempDir::new()?;
    let doc = document_at(&dir, "m.mcvox", &two_parts(60, 60))?;
    let run = invoke(&["inspect", path(&doc), "--max-extent", "96"])?;
    assert_refused(&run, "the assembled model reaches 120 voxels on axis y, but the limit is 96 voxels on any axis");
    Ok(())
}

#[test]
fn values_outside_sixty_four_to_two_hundred_fifty_six_are_refused() -> TestResult {
    let dir = TempDir::new()?;
    let doc = document_at(&dir, "m.mcvox", &one_part(4))?;
    for bad in ["63", "0", "-5", "257", "1000", "99999999999"] {
        let run = invoke(&["inspect", path(&doc), "--max-extent", bad])?;
        assert_refused(&run, &format!("`--max-extent {bad}` is out of range: the limit must be between 64 and 256 voxels"));
    }
    let run = invoke(&["inspect", path(&doc), "--max-extent", "many"])?;
    assert_eq!(run.code, ExitCode::Defective);
    assert_eq!(run.out, "");
    assert!(run.err.contains("--max-extent"), "stderr was: {}", run.err);
    for good in ["64", "256"] {
        let run = invoke(&["inspect", path(&doc), "--max-extent", good])?;
        assert_eq!(run.code, ExitCode::Success, "{good}: {}", run.err);
    }
    Ok(())
}

#[test]
fn the_largest_limit_accepts_two_hundred_fifty_six_voxels() -> TestResult {
    let dir = TempDir::new()?;
    let doc = document_at(&dir, "m.mcvox", &declared_only(256))?;
    // No art: refused for that, which is only reached once the declared size passed.
    let run = invoke(&["inspect", path(&doc), "--max-extent", "256"])?;
    assert_eq!(run.code, ExitCode::Defective);
    assert!(!run.err.contains("limit is"), "stderr was: {}", run.err);
    let doc = document_at(&dir, "m2.mcvox", &declared_only(257))?;
    let run = invoke(&["inspect", path(&doc), "--max-extent", "256"])?;
    assert_refused(&run, "the declared size is 257, but the limit is 256 voxels");
    Ok(())
}

#[test]
fn texture_honours_the_option_for_a_block_larger_than_sixty_four() -> TestResult {
    let (dir, materials) = workspace()?;
    let edge = 80;
    let mut text = format!(
        "schema = 1\nname = \"test:cube\"\nscale = {edge}\nsize = [{edge}, {edge}, {edge}]\norigin = [0, 0, 0]\nslice = \"y\"\n\n[palette]\n\".\" = \"empty\"\n\"r\" = \"test:red\"\n"
    );
    let row = "r".repeat(edge);
    for y in 0..edge {
        text.push_str(&format!("\n[[layers]]\ny = {y}\ngrid = \"\"\"\n"));
        for _ in 0..edge {
            text.push_str(&row);
            text.push('\n');
        }
        text.push_str("\"\"\"\n");
    }
    let doc = document_at(&dir, "cube.mcvox", &text)?;
    let out = dir.path().join("faces");
    fs::create_dir_all(&out)?;
    let args = [
        "texture", path(&doc), "--out", path(&out), "--materials", path(&materials), "--face", "front",
        "--pixels-per-voxel", "1",
    ];
    assert_refused(&invoke(&args)?, "on axis x the declared size is 80, but the limit is 64 voxels");
    let raised = [&args[..], &["--max-extent", "96"]].concat();
    let run = invoke(&raised)?;
    assert_eq!(run.code, ExitCode::Success, "stderr: {}", run.err);
    let image = image::load_from_memory(&fs::read(out.join("front.png"))?)?;
    assert_eq!((image.width(), image.height()), (80, 80));
    Ok(())
}
