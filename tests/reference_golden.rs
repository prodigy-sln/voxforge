//! The reference model's contact sheet against the one a human signed off.
//!
//! The golden is evidence that today's render still produces the reviewed
//! picture, not that the orientation convention is right: re-shooting it is a
//! request for a new review, never a repair for a red run.
//!
//! Compared pixel for pixel after decoding rather than byte for byte, so a
//! change in the PNG encoder's output does not fail it while any changed pixel,
//! alpha included, does. Rendered at 24 pixels per voxel, the reviewed scale.

mod common;

use std::error::Error;
use std::num::NonZeroU32;

use common::TestResult;
use common::cli::repository_path;
use voxforge::format::load_document;
use voxforge::material::load_materials;
use voxforge::render::contact_sheet;
use voxforge::volume::{StateSelection, assemble};

const SIGNED_OFF_SCALE: NonZeroU32 = match NonZeroU32::new(24) {
    Some(scale) => scale,
    None => NonZeroU32::MIN,
};

/// How today's sheet compares with the signed one.
#[derive(Debug, PartialEq)]
enum Golden {
    AsSigned,
    Resized {
        signed: (u32, u32),
        rendered: (u32, u32),
    },
    PixelMoved {
        column: u32,
        row: u32,
        signed: [u8; 4],
        rendered: [u8; 4],
    },
}

fn against_the_signed_sheet() -> Result<Golden, Box<dyn Error>> {
    let signed = image::open(repository_path(
        "tests/goldens/reference-asymmetric-sheet.png",
    ))?
    .into_rgba8();

    let model = load_document(&repository_path(
        "tests/fixtures/content/models/reference-asymmetric.mcvox",
    ))?;
    let materials = load_materials(&repository_path("tests/fixtures/content/materials"))?;
    model.bind_materials(&materials)?;
    let volume = assemble(&model, &StateSelection::default())?;
    let sheet = contact_sheet(&volume, &materials, SIGNED_OFF_SCALE);
    let rendered = sheet.image();

    if (signed.width(), signed.height()) != (rendered.width(), rendered.height()) {
        return Ok(Golden::Resized {
            signed: signed.dimensions(),
            rendered: (rendered.width(), rendered.height()),
        });
    }
    for (column, row, pixel) in signed.enumerate_pixels() {
        let here = rendered
            .pixel(column, row)
            .ok_or_else(|| format!("the render has no pixel at column {column}, row {row}"))?;
        let there = [here.red, here.green, here.blue, here.alpha];
        if pixel.0 != there {
            return Ok(Golden::PixelMoved {
                column,
                row,
                signed: pixel.0,
                rendered: there,
            });
        }
    }
    Ok(Golden::AsSigned)
}

#[test]
fn the_reference_models_contact_sheet_still_matches_the_one_that_was_signed_off() -> TestResult {
    assert_eq!(
        against_the_signed_sheet()?,
        Golden::AsSigned,
        "a difference here is this build disagreeing with the reviewed picture — re-shooting the golden is a request for a new review, never a repair"
    );
    Ok(())
}
