use std::path::Path;
use std::fs::File;
use std::io::Write;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

use crate::fault::{Fault, Origin};
use crate::format::{FilledCell, Model, Part};
use crate::material::MaterialTable;
use crate::name::MaterialKey;

/// How each part's mesh is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum MeshMode {
    // Six faces per voxel, hidden ones included.
    #[default]
    Cubes,
    // Faces between filled voxels of the same part dropped, coplanar faces of
    // one material greedily merged into rectangles.
    Merged,
}

/// What one unit of vertex position stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum Unit {
    // One voxel.
    #[default]
    Voxel,
    // One block: positions divided by the model's `scale`.
    Block,
}

/// How material colours are written to `COLOR_0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum Colors {
    // sRGB bytes over 255, as declared.
    #[default]
    Srgb,
    // Converted to linear with the sRGB transfer function.
    Linear,
}

/// Whether ambient occlusion is baked into the vertex colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum Ao {
    // No occlusion: colours are the material's.
    #[default]
    Off,
    // Each face corner darkened by the voxels around it, within the same part.
    Vertex,
}

/// The occlusion strength when nobody says otherwise.
pub const DEFAULT_AO_STRENGTH: f32 = 0.5;

/// Options for [`write_glb_with`]. The default reproduces [`write_glb`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlbOptions {
    pub mesh: MeshMode,
    pub unit: Unit,
    pub colors: Colors,
    pub ao: Ao,
    /// 0.0..=1.0; read only when `ao` is [`Ao::Vertex`].
    pub ao_strength: f32,
}

impl Default for GlbOptions {
    fn default() -> Self {
        Self {
            mesh: MeshMode::default(),
            unit: Unit::default(),
            colors: Colors::default(),
            ao: Ao::default(),
            ao_strength: DEFAULT_AO_STRENGTH,
        }
    }
}

/// Refuses an occlusion strength outside 0.0..=1.0 (NaN included).
pub fn check_ao_strength(strength: f32, origin: Origin) -> Result<(), Fault> {
    if (0.0..=1.0).contains(&strength) {
        Ok(())
    } else {
        Err(Fault::about(
            origin,
            format!("`--ao-strength {strength}` is outside 0.0 to 1.0"),
        )
        .in_field("ao-strength"))
    }
}

/// Totals over every mesh written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlbStats {
    pub vertices: u64,
    pub triangles: u64,
}

/// Writes `model` as a binary glTF, one cube per voxel, in voxel units.
pub fn write_glb(
    model: &Model,
    materials: &MaterialTable,
    destination: &Path,
) -> Result<(), Fault> {
    write_glb_with(model, materials, destination, &GlbOptions::default()).map(|_| ())
}

/// Unit cube faces: normal and four corners, counter-clockwise from outside.
const FACES: [([f32; 3], [[f32; 3]; 4]); 6] = [
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

/// Normal axis, in-plane (u, v) axes and direction (+1/-1) of each entry in [`FACES`].
const FACE_AXES: [(usize, usize, usize, i32); 6] = [
    (2, 0, 1, 1),
    (2, 0, 1, -1),
    (1, 0, 2, 1),
    (1, 0, 2, -1),
    (0, 2, 1, 1),
    (0, 2, 1, -1),
];

/// One part's vertex streams under construction.
struct PartMesh {
    divisor: f32,
    min: [f32; 3],
    max: [f32; 3],
    pos_data: Vec<u8>,
    norm_data: Vec<u8>,
    color_data: Vec<u8>,
    idx_data: Vec<u8>,
    vertex_count: u32,
}

impl PartMesh {
    fn new(divisor: f32) -> Self {
        Self {
            divisor,
            min: [f32::MAX; 3],
            max: [f32::MIN; 3],
            pos_data: Vec::new(),
            norm_data: Vec::new(),
            color_data: Vec::new(),
            idx_data: Vec::new(),
            vertex_count: 0,
        }
    }

    /// Appends one quad; `corners` are in voxel units relative to the part origin.
    /// `flip` splits it along corners 1-3 instead of 0-2.
    fn quad(&mut self, norm: &[f32; 3], corners: &[[f32; 3]; 4], colors: [[f32; 4]; 4], flip: bool) {
        for (p, rgba) in corners.iter().zip(colors) {
            let pos = [p[0] / self.divisor, p[1] / self.divisor, p[2] / self.divisor];
            for axis in 0..3 {
                self.min[axis] = self.min[axis].min(pos[axis]);
                self.max[axis] = self.max[axis].max(pos[axis]);
                self.pos_data.extend_from_slice(&pos[axis].to_le_bytes());
            }
            for n in norm {
                self.norm_data.extend_from_slice(&n.to_le_bytes());
            }
            for c in rgba {
                self.color_data.extend_from_slice(&c.to_le_bytes());
            }
        }
        let v = self.vertex_count;
        let order = if flip { [v + 1, v + 2, v + 3, v + 1, v + 3, v] } else { [v, v + 1, v + 2, v, v + 2, v + 3] };
        for i in order {
            self.idx_data.extend_from_slice(&i.to_le_bytes());
        }
        self.vertex_count += 4;
    }
}

/// sRGB-encoded channel in 0..=1 to linear.
#[must_use]
pub fn srgb_to_linear(c: f32) -> f32 {
    let c = f64::from(c);
    let linear = if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    };
    linear as f32
}

fn color_of(materials: &MaterialTable, key: &MaterialKey, colors: Colors) -> [f32; 4] {
    let mut rgba = [1.0, 0.0, 1.0, 1.0];
    if let Some(mat) = materials.get(key) {
        rgba[0] = mat.color.red as f32 / 255.0;
        rgba[1] = mat.color.green as f32 / 255.0;
        rgba[2] = mat.color.blue as f32 / 255.0;
    }
    if colors == Colors::Linear {
        for c in &mut rgba[..3] {
            *c = srgb_to_linear(*c);
        }
    }
    rgba
}

/// Filled voxels of one part by position; 0 is empty, and so is anything outside.
struct Grid {
    dims: [usize; 3],
    slots: Vec<u16>,
}

impl Grid {
    fn new(cells: &[FilledCell], slot: impl Fn(&FilledCell) -> u16) -> Self {
        let dims = [
            cells.iter().map(|c| c.position.x).max().unwrap_or(0) as usize + 1,
            cells.iter().map(|c| c.position.y).max().unwrap_or(0) as usize + 1,
            cells.iter().map(|c| c.position.z).max().unwrap_or(0) as usize + 1,
        ];
        let mut grid = Self { dims, slots: vec![0u16; dims[0] * dims[1] * dims[2]] };
        for cell in cells {
            let p = [cell.position.x as usize, cell.position.y as usize, cell.position.z as usize];
            let at = grid.index(p);
            grid.slots[at] = slot(cell);
        }
        grid
    }

    fn index(&self, p: [usize; 3]) -> usize {
        p[0] + p[1] * self.dims[0] + p[2] * self.dims[0] * self.dims[1]
    }

    fn at(&self, p: [i64; 3]) -> u16 {
        if (0..3).any(|a| p[a] < 0 || p[a] >= self.dims[a] as i64) {
            return 0;
        }
        self.slots[self.index([p[0] as usize, p[1] as usize, p[2] as usize])]
    }
}

/// Occlusion level 0..=3 at each corner of `face` of the voxel `cell`, in
/// [`FACES`] corner order. Per corner: the two edge neighbours and the diagonal
/// one in the layer in front of the face; both edges filled counts as 3.
fn ao_levels(grid: &Grid, cell: [i64; 3], face: usize) -> [u8; 4] {
    let (n, u, v, dir) = FACE_AXES[face];
    let mut front = cell;
    front[n] += i64::from(dir);
    FACES[face].1.map(|t| {
        let su = if t[u] > 0.5 { 1 } else { -1 };
        let sv = if t[v] > 0.5 { 1 } else { -1 };
        let filled = |du: i64, dv: i64| {
            let mut p = front;
            p[u] += du;
            p[v] += dv;
            grid.at(p) != 0
        };
        let (side1, side2, corner) = (filled(su, 0), filled(0, sv), filled(su, sv));
        if side1 && side2 { 3 } else { u8::from(side1) + u8::from(side2) + u8::from(corner) }
    })
}

/// Per-corner colours darkened by `levels`, and whether the quad's diagonal flips.
///
/// The factor `1 - strength * level / 3` multiplies the RGB values as written
/// to `COLOR_0`, in whichever space `--colors` chose. With `linear` that is
/// the physically meaningful space; with `srgb` it is a plain scale of the
/// declared colour. Alpha is untouched.
///
/// The diagonal runs through the corner pair with the larger level sum, so the
/// interpolation between the darker corners is not stretched across a lighter one.
fn shade(base: [f32; 4], levels: [u8; 4], strength: f32) -> ([[f32; 4]; 4], bool) {
    let colors = levels.map(|level| {
        let factor = 1.0 - strength * f32::from(level) / 3.0;
        [base[0] * factor, base[1] * factor, base[2] * factor, base[3]]
    });
    (colors, levels[0] + levels[2] < levels[1] + levels[3])
}

fn emit_cubes(
    mesh: &mut PartMesh,
    part: &Part,
    cells: &[FilledCell],
    materials: &MaterialTable,
    colors: Colors,
    ao: Option<f32>,
) {
    let o = [part.origin.x as f32, part.origin.y as f32, part.origin.z as f32];
    let grid = ao.map(|_| Grid::new(cells, |_| 1));
    for cell in cells {
        let c = [
            cell.position.x as f32 - o[0],
            cell.position.y as f32 - o[1],
            cell.position.z as f32 - o[2],
        ];
        let rgba = color_of(materials, &cell.material, colors);
        let at = [i64::from(cell.position.x), i64::from(cell.position.y), i64::from(cell.position.z)];
        for (face, (norm, positions)) in FACES.iter().enumerate() {
            let corners = positions.map(|p| [c[0] + p[0], c[1] + p[1], c[2] + p[2]]);
            let (shaded, flip) = match (&grid, ao) {
                (Some(grid), Some(strength)) => shade(rgba, ao_levels(grid, at, face), strength),
                _ => ([rgba; 4], false),
            };
            mesh.quad(norm, &corners, shaded, flip);
        }
    }
}

/// Marks a mask cell whose corners differ in occlusion: it is never merged.
const UNMERGEABLE: u32 = 1 << 31;

/// Hidden faces dropped, then coplanar faces of one material greedily merged.
/// With occlusion, only faces of one uniform level (all four corners equal) merge.
fn emit_merged(
    mesh: &mut PartMesh,
    part: &Part,
    cells: &[FilledCell],
    materials: &MaterialTable,
    colors: Colors,
    ao: Option<f32>,
) {
    if cells.is_empty() {
        return;
    }
    // Material slots in key order, so output never depends on hash order.
    let keys: BTreeMap<&MaterialKey, u16> = {
        let mut sorted: Vec<&MaterialKey> = cells.iter().map(|c| &c.material).collect();
        sorted.sort();
        sorted.dedup();
        sorted.into_iter().zip(1u16..).collect()
    };
    let palette: Vec<[f32; 4]> = keys.keys().map(|k| color_of(materials, k, colors)).collect();

    let grid = Grid::new(cells, |c| keys[&c.material]);
    let dims = grid.dims;

    let o = [part.origin.x as f32, part.origin.y as f32, part.origin.z as f32];
    for (face, (norm, template)) in FACES.iter().enumerate() {
        let (n, u, v, dir) = FACE_AXES[face];
        let (du, dv) = (dims[u], dims[v]);
        let mut mask = vec![0u32; du * dv];
        let mut levels = vec![[0u8; 4]; du * dv];
        for d in 0..dims[n] {
            for j in 0..dv {
                for i in 0..du {
                    let mut p = [0i64; 3];
                    p[n] = d as i64;
                    p[u] = i as i64;
                    p[v] = j as i64;
                    let here = grid.at(p);
                    let cell = p;
                    p[n] += i64::from(dir);
                    mask[i + j * du] = if here != 0 && grid.at(p) == 0 {
                        let mut key = u32::from(here);
                        if ao.is_some() {
                            let lv = ao_levels(&grid, cell, face);
                            levels[i + j * du] = lv;
                            if lv.iter().all(|l| *l == lv[0]) {
                                key |= (u32::from(lv[0]) + 1) << 16;
                            } else {
                                key |= UNMERGEABLE;
                            }
                        }
                        key
                    } else {
                        0
                    };
                }
            }
            for j in 0..dv {
                let mut i = 0;
                while i < du {
                    let m = mask[i + j * du];
                    if m == 0 {
                        i += 1;
                        continue;
                    }
                    let single = m & UNMERGEABLE != 0;
                    let mut w = 1;
                    while !single && i + w < du && mask[i + w + j * du] == m {
                        w += 1;
                    }
                    let mut h = 1;
                    'grow: while !single && j + h < dv {
                        for k in 0..w {
                            if mask[i + k + (j + h) * du] != m {
                                break 'grow;
                            }
                        }
                        h += 1;
                    }
                    for jj in j..j + h {
                        for k in 0..w {
                            mask[i + k + jj * du] = 0;
                        }
                    }
                    let corners = template.map(|t| {
                        let mut q = [0f32; 3];
                        q[n] = d as f32 + t[n];
                        q[u] = i as f32 + t[u] * w as f32;
                        q[v] = j as f32 + t[v] * h as f32;
                        [q[0] - o[0], q[1] - o[1], q[2] - o[2]]
                    });
                    let base = palette[usize::from((m & 0xFFFF) as u16) - 1];
                    let (shaded, flip) = match ao {
                        Some(strength) => shade(base, levels[i + j * du], strength),
                        None => ([base; 4], false),
                    };
                    mesh.quad(norm, &corners, shaded, flip);
                    i += w;
                }
            }
        }
    }
}

/// Writes `model` as a binary glTF under `options`, and reports its totals.
pub fn write_glb_with(
    model: &Model,
    materials: &MaterialTable,
    destination: &Path,
    options: &GlbOptions,
) -> Result<GlbStats, Fault> {
    let origin = Origin::new(destination);
    check_ao_strength(options.ao_strength, origin.clone())?;
    let ao = (options.ao == Ao::Vertex).then_some(options.ao_strength);
    let divisor = match options.unit {
        Unit::Voxel => 1.0,
        Unit::Block => model.scale.get() as f32,
    };
    let mut stats = GlbStats::default();

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
        let cells = part.filled_cells_in(part.default_state());
        let mut mesh = PartMesh::new(divisor);
        match options.mesh {
            MeshMode::Cubes => emit_cubes(&mut mesh, part, &cells, materials, options.colors, ao),
            MeshMode::Merged => emit_merged(&mut mesh, part, &cells, materials, options.colors, ao),
        }
        let vertex_count = mesh.vertex_count;
        stats.vertices += u64::from(vertex_count);
        stats.triangles += u64::from(vertex_count / 2);
        let [mut min_x, mut min_y, mut min_z] = mesh.min;
        let [mut max_x, mut max_y, mut max_z] = mesh.max;

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

        let pos_bv = push_buffer_view(&mesh.pos_data, 34962); // ARRAY_BUFFER
        let norm_bv = push_buffer_view(&mesh.norm_data, 34962);
        let color_bv = push_buffer_view(&mesh.color_data, 34962);
        let idx_bv = push_buffer_view(&mesh.idx_data, 34963); // ELEMENT_ARRAY_BUFFER

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

                translation = [(ax - px) / divisor, (ay - py) / divisor, (az - pz) / divisor];
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

    let gltf_json: Value = json!({
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

    Ok(stats)
}
