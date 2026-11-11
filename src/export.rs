use std::path::Path;
use std::fs::File;
use std::io::{Write, BufWriter};

use dot_vox::{Color, DotVoxData, Model as VoxModel, Size, Voxel as VoxVoxel, Material as VoxMaterial, Dict};

use crate::fault::{Fault, Origin};
use crate::volume::Volume;
use crate::material::MaterialTable;

pub fn write_vox(
    volume: &Volume,
    materials: &MaterialTable,
    destination: &Path,
) -> Result<(), Fault> {
    let origin = Origin::new(destination);
    
    let extent = volume.extent();
    if extent.x > 256 || extent.y > 256 || extent.z > 256 {
        return Err(Fault::about(
            origin.clone(),
            format!("volume extent {}x{}x{} exceeds the .vox 256-cubed limit", extent.x, extent.y, extent.z)
        ));
    }

    // Build the palette. The volume has its own `palette` of MaterialKeys.
    // .vox palette has up to 255 colors (index 1-255).
    // Volume palette length is checked implicitly by u8 limits.
    let mut palette = Vec::new();
    let mut vox_materials = Vec::new();
    
    // .vox uses 1-based indexing for voxels, so index 0 in the palette is usually ignored,
    // but the palette array itself is 256 colors.
    // We will push the first color at index 0, but voxel indices will be `i + 1`.
    let filled = volume.filled();
    let vol_palette: Vec<_> = filled.iter()
        .map(|c| c.material.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    
    if vol_palette.len() > 255 {
        return Err(Fault::about(
            origin.clone(),
            format!("volume uses {} materials, exceeding the .vox 255-color limit", vol_palette.len())
        ));
    }
    
    let mut mat_map = std::collections::HashMap::new();
    for (i, key) in vol_palette.iter().enumerate() {
        mat_map.insert(key, i as u8 + 1); // 1-based index
        
        let mut r = 255;
        let mut g = 0;
        let mut b = 255;
        let a = 255;
        
        if let Some(mat) = materials.get(key) {
            r = mat.color.red;
            g = mat.color.green;
            b = mat.color.blue;
            if mat.emissive.fraction() > 0.0 {
                let mut props = Dict::new();
                props.insert("_type".to_string(), "_emit".to_string());
                props.insert("_emit".to_string(), format!("{}", mat.emissive.fraction()));
                vox_materials.push(VoxMaterial {
                    id: i as u32 + 1,
                    properties: props,
                });
            }
        }
        palette.push(Color { r, g, b, a });
    }
    
    // Fill remaining palette to 256
    while palette.len() < 256 {
        palette.push(Color { r: 0, g: 0, b: 0, a: 255 });
    }

    let mut voxels = Vec::new();
    for cell in &filled {
        if let Some(&index) = mat_map.get(&cell.material) {
            voxels.push(VoxVoxel {
                x: cell.position.x as u8,
                y: cell.position.z as u8, // MagicaVoxel uses Z as up, so map Y -> Z
                z: cell.position.y as u8,
                i: index - 1, // dot_vox uses 0-based for the i field, writes it as +1 internally? Wait, dot_vox i is 0-254 for index 1-255.
            });
        }
    }

    let model = VoxModel {
        size: Size {
            x: extent.x as u32,
            y: extent.z as u32,
            z: extent.y as u32,
        },
        voxels,
    };

    let data = DotVoxData {
        version: 150,
        models: vec![model],
        palette,
        materials: vox_materials,
        scenes: Vec::new(),
        layers: Vec::new(),
        index_map: Vec::new(),
    };

    let mut file = File::create(destination).map_err(|e| {
        Fault::about(origin.clone(), format!("could not create .vox file: {e}"))
    })?;
    data.write_vox(&mut file).map_err(|e| {
        Fault::about(origin, format!("could not write .vox data: {e}"))
    })?;

    Ok(())
}

pub fn write_obj(
    volume: &Volume,
    materials: &MaterialTable,
    destination: &Path,
) -> Result<(), Fault> {
    let origin = Origin::new(destination);
    
    let mtl_path = destination.with_extension("mtl");
    let mtl_name = mtl_path.file_name().unwrap().to_string_lossy();
    
    let mut obj_file = BufWriter::new(File::create(destination).map_err(|e| {
        Fault::about(origin.clone(), format!("could not create .obj file: {e}"))
    })?);
    
    let mut mtl_file = BufWriter::new(File::create(&mtl_path).map_err(|e| {
        Fault::about(origin.clone(), format!("could not create .mtl file: {e}"))
    })?);

    writeln!(obj_file, "mtllib {}", mtl_name).unwrap();

    let filled = volume.filled();
    let vol_palette: Vec<_> = filled.iter()
        .map(|c| c.material.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    for key in &vol_palette {
        let clean_name = key.as_str().replace(':', "_").replace('/', "_");
        writeln!(mtl_file, "newmtl {}", clean_name).unwrap();
        if let Some(mat) = materials.get(key) {
            writeln!(mtl_file, "Kd {} {} {}", mat.color.red as f32 / 255.0, mat.color.green as f32 / 255.0, mat.color.blue as f32 / 255.0).unwrap();
            if mat.emissive.fraction() > 0.0 {
                writeln!(mtl_file, "Ke {} {} {}", mat.color.red as f32 / 255.0 * mat.emissive.fraction(), mat.color.green as f32 / 255.0 * mat.emissive.fraction(), mat.color.blue as f32 / 255.0 * mat.emissive.fraction()).unwrap();
            }
        } else {
            writeln!(mtl_file, "Kd 1.0 0.0 1.0").unwrap();
        }
    }

    let mut vertex_offset = 1;
    for cell in &filled {
        let x = cell.position.x as f32;
        let y = cell.position.y as f32;
        let z = cell.position.z as f32;
        
        let p = [
            (x, y, z),
            (x + 1.0, y, z),
            (x + 1.0, y + 1.0, z),
            (x, y + 1.0, z),
            (x, y, z + 1.0),
            (x + 1.0, y, z + 1.0),
            (x + 1.0, y + 1.0, z + 1.0),
            (x, y + 1.0, z + 1.0),
        ];
        
        for (vx, vy, vz) in &p {
            writeln!(obj_file, "v {} {} {}", vx, vy, vz).unwrap();
        }
        
        let clean_name = cell.material.as_str().replace(':', "_").replace('/', "_");
        writeln!(obj_file, "usemtl {}", clean_name).unwrap();
        
        // Output faces (1-based indices)
        // Back (-z)
        writeln!(obj_file, "f {} {} {} {}", vertex_offset + 3, vertex_offset + 2, vertex_offset + 1, vertex_offset).unwrap();
        // Front (+z)
        writeln!(obj_file, "f {} {} {} {}", vertex_offset + 4, vertex_offset + 5, vertex_offset + 6, vertex_offset + 7).unwrap();
        // Bottom (-y)
        writeln!(obj_file, "f {} {} {} {}", vertex_offset, vertex_offset + 1, vertex_offset + 5, vertex_offset + 4).unwrap();
        // Top (+y)
        writeln!(obj_file, "f {} {} {} {}", vertex_offset + 7, vertex_offset + 6, vertex_offset + 2, vertex_offset + 3).unwrap();
        // Left (-x)
        writeln!(obj_file, "f {} {} {} {}", vertex_offset, vertex_offset + 4, vertex_offset + 7, vertex_offset + 3).unwrap();
        // Right (+x)
        writeln!(obj_file, "f {} {} {} {}", vertex_offset + 1, vertex_offset + 2, vertex_offset + 6, vertex_offset + 5).unwrap();

        vertex_offset += 8;
    }

    Ok(())
}
