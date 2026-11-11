use std::path::Path;
use std::fs::File;
use std::io::Write;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::fault::{Fault, Origin};
use crate::format::Model;
use crate::material::MaterialTable;

pub fn write_glb(
    model: &Model,
    materials: &MaterialTable,
    destination: &Path,
) -> Result<(), Fault> {
    let origin = Origin::new(destination);
    
    let mut bin = Vec::new();
    let mut accessors = Vec::new();
    let mut buffer_views = Vec::new();
    let mut meshes = Vec::new();
    let mut nodes = Vec::new();
    
    let mut part_to_node = HashMap::new();
    
    // First, map parts to node indices
    for (i, part) in model.parts.iter().enumerate() {
        part_to_node.insert(part.name.as_str().to_string(), i);
    }
    
    for part in &model.parts {
        let mut min_x = f32::MAX; let mut max_x = f32::MIN;
        let mut min_y = f32::MAX; let mut max_y = f32::MIN;
        let mut min_z = f32::MAX; let mut max_z = f32::MIN;
        
        let cells = part.filled_cells_in(part.default_state());
        
        let mut pos_data = Vec::new();
        let mut norm_data = Vec::new();
        let mut color_data = Vec::new();
        let mut idx_data = Vec::new();
        
        let mut vertex_count = 0u32;
        
        let o_x = part.origin.x as f32;
        let o_y = part.origin.y as f32;
        let o_z = part.origin.z as f32;
        
        for cell in &cells {
            let cx = cell.position.x as f32 - o_x;
            let cy = cell.position.y as f32 - o_y;
            let cz = cell.position.z as f32 - o_z;
            
            let mut r: f32 = 1.0; let mut g: f32 = 0.0; let mut b: f32 = 1.0; let mut a: f32 = 1.0;
            if let Some(mat) = materials.get(&cell.material) {
                r = mat.color.red as f32 / 255.0;
                g = mat.color.green as f32 / 255.0;
                b = mat.color.blue as f32 / 255.0;
            }
            
            // 6 faces * 4 vertices = 24 vertices per voxel
            let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
                // Front (+z)
                ([0.0, 0.0, 1.0], [[0.0,0.0,1.0], [1.0,0.0,1.0], [1.0,1.0,1.0], [0.0,1.0,1.0]]),
                // Back (-z)
                ([0.0, 0.0, -1.0], [[1.0,0.0,0.0], [0.0,0.0,0.0], [0.0,1.0,0.0], [1.0,1.0,0.0]]),
                // Top (+y)
                ([0.0, 1.0, 0.0], [[0.0,1.0,1.0], [1.0,1.0,1.0], [1.0,1.0,0.0], [0.0,1.0,0.0]]),
                // Bottom (-y)
                ([0.0, -1.0, 0.0], [[0.0,0.0,0.0], [1.0,0.0,0.0], [1.0,0.0,1.0], [0.0,0.0,1.0]]),
                // Right (+x)
                ([1.0, 0.0, 0.0], [[1.0,0.0,1.0], [1.0,0.0,0.0], [1.0,1.0,0.0], [1.0,1.0,1.0]]),
                // Left (-x)
                ([-1.0, 0.0, 0.0], [[0.0,0.0,0.0], [0.0,0.0,1.0], [0.0,1.0,1.0], [0.0,1.0,0.0]]),
            ];
            
            for (norm, positions) in faces.iter() {
                for p in positions {
                    let px = cx + p[0];
                    let py = cy + p[1];
                    let pz = cz + p[2];
                    
                    min_x = min_x.min(px); max_x = max_x.max(px);
                    min_y = min_y.min(py); max_y = max_y.max(py);
                    min_z = min_z.min(pz); max_z = max_z.max(pz);
                    
                    pos_data.extend_from_slice(&px.to_le_bytes());
                    pos_data.extend_from_slice(&py.to_le_bytes());
                    pos_data.extend_from_slice(&pz.to_le_bytes());
                    
                    norm_data.extend_from_slice(&norm[0].to_le_bytes());
                    norm_data.extend_from_slice(&norm[1].to_le_bytes());
                    norm_data.extend_from_slice(&norm[2].to_le_bytes());
                    
                    color_data.extend_from_slice(&r.to_le_bytes());
                    color_data.extend_from_slice(&g.to_le_bytes());
                    color_data.extend_from_slice(&b.to_le_bytes());
                    color_data.extend_from_slice(&a.to_le_bytes());
                }
                
                let v0 = vertex_count;
                let v1 = vertex_count + 1;
                let v2 = vertex_count + 2;
                let v3 = vertex_count + 3;
                
                idx_data.extend_from_slice(&v0.to_le_bytes());
                idx_data.extend_from_slice(&v1.to_le_bytes());
                idx_data.extend_from_slice(&v2.to_le_bytes());
                idx_data.extend_from_slice(&v0.to_le_bytes());
                idx_data.extend_from_slice(&v2.to_le_bytes());
                idx_data.extend_from_slice(&v3.to_le_bytes());
                
                vertex_count += 4;
            }
        }
        
        if vertex_count == 0 {
            min_x = 0.0; max_x = 0.0;
            min_y = 0.0; max_y = 0.0;
            min_z = 0.0; max_z = 0.0;
        }

        let mut push_buffer_view = |data: &[u8], target: u32| -> usize {
            let offset = bin.len();
            bin.extend_from_slice(data);
            // padding
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
            let length = data.len();
            buffer_views.push(json!({
                "buffer": 0,
                "byteOffset": offset,
                "byteLength": length,
                "target": target
            }));
            buffer_views.len() - 1
        };

        let pos_bv = push_buffer_view(&pos_data, 34962); // ARRAY_BUFFER
        let norm_bv = push_buffer_view(&norm_data, 34962);
        let color_bv = push_buffer_view(&color_data, 34962);
        let idx_bv = push_buffer_view(&idx_data, 34963); // ELEMENT_ARRAY_BUFFER
        
        accessors.push(json!({
            "bufferView": pos_bv,
            "componentType": 5126, // FLOAT
            "count": vertex_count,
            "type": "VEC3",
            "min": [min_x, min_y, min_z],
            "max": [max_x, max_y, max_z]
        }));
        let pos_acc = accessors.len() - 1;
        
        accessors.push(json!({
            "bufferView": norm_bv,
            "componentType": 5126,
            "count": vertex_count,
            "type": "VEC3"
        }));
        let norm_acc = accessors.len() - 1;
        
        accessors.push(json!({
            "bufferView": color_bv,
            "componentType": 5126,
            "count": vertex_count,
            "type": "VEC4"
        }));
        let color_acc = accessors.len() - 1;
        
        accessors.push(json!({
            "bufferView": idx_bv,
            "componentType": 5125, // UNSIGNED_INT
            "count": vertex_count / 4 * 6,
            "type": "SCALAR"
        }));
        let idx_acc = accessors.len() - 1;
        
        meshes.push(json!({
            "name": part.name.as_str(),
            "primitives": [{
                "attributes": {
                    "POSITION": pos_acc,
                    "NORMAL": norm_acc,
                    "COLOR_0": color_acc
                },
                "indices": idx_acc,
                "mode": 4 // TRIANGLES
            }]
        }));
        
        let mesh_idx = meshes.len() - 1;
        
        let mut translation = [0.0, 0.0, 0.0];
        let mut children = Vec::new();
        
        // Find children of this part
        for (i, other_part) in model.parts.iter().enumerate() {
            if let Some(attach) = &other_part.attach {
                if attach.to.as_str() == part.name.as_str() {
                    children.push(i);
                }
            }
        }
        
        if let Some(attach) = &part.attach {
            if let Some(parent_part) = model.part(attach.to.as_str()) {
                let px = parent_part.origin.x as f32;
                let py = parent_part.origin.y as f32;
                let pz = parent_part.origin.z as f32;
                
                let ax = attach.at.x as f32;
                let ay = attach.at.y as f32;
                let az = attach.at.z as f32;
                
                translation = [ax - px, ay - py, az - pz];
            }
        }
        
        let mut node = json!({
            "name": part.name.as_str(),
            "mesh": mesh_idx,
            "translation": translation,
        });
        
        if !children.is_empty() {
            node.as_object_mut().unwrap().insert("children".to_string(), json!(children));
        }
        
        nodes.push(node);
    }
    
    // Find root nodes (nodes without parents)
    let mut root_nodes = Vec::new();
    for (i, part) in model.parts.iter().enumerate() {
        if part.attach.is_none() {
            root_nodes.push(i);
        }
    }
    
    let gltf_json = json!({
        "asset": {
            "version": "2.0",
            "generator": "voxforge"
        },
        "scene": 0,
        "scenes": [{
            "nodes": root_nodes
        }],
        "nodes": nodes,
        "meshes": meshes,
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [{
            "byteLength": bin.len()
        }]
    });
    
    let mut json_str = serde_json::to_string(&gltf_json).unwrap();
    while json_str.len() % 4 != 0 {
        json_str.push(' ');
    }
    let json_bytes = json_str.as_bytes();
    
    let total_length = 12 + 8 + json_bytes.len() as u32 + 8 + bin.len() as u32;
    
    let mut file = File::create(destination).map_err(|e| Fault::about(origin.clone(), format!("could not create .glb file: {e}")))?;
    
    // GLB Header
    file.write_all(b"glTF").unwrap();
    file.write_all(&2u32.to_le_bytes()).unwrap();
    file.write_all(&total_length.to_le_bytes()).unwrap();
    
    // JSON Chunk
    file.write_all(&(json_bytes.len() as u32).to_le_bytes()).unwrap();
    file.write_all(b"JSON").unwrap();
    file.write_all(json_bytes).unwrap();
    
    // BIN Chunk
    file.write_all(&(bin.len() as u32).to_le_bytes()).unwrap();
    file.write_all(b"BIN\0").unwrap();
    file.write_all(&bin).unwrap();
    
    Ok(())
}
