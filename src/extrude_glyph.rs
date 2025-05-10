use bevy::{asset::RenderAssetUsages, prelude::*, render::mesh::PrimitiveTopology};
use cosmic_text::ttf_parser::Rect;
use cosmic_text::ttf_parser::{Face, GlyphId};
use lyon::{
    geom::point,
    path::PathEvent,
    tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers},
};

use crate::MeshTextError;

#[derive(Debug, Clone)]
pub struct ExtrudedGlyphGeometry {
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u16>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>, // Added UV coordinates for texture mapping
}

impl From<ExtrudedGlyphGeometry> for Mesh {
    fn from(value: ExtrudedGlyphGeometry) -> Self {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, value.vertices)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, value.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, value.uvs)
        .with_inserted_indices(bevy::render::mesh::Indices::U16(value.indices))
    }
}

pub fn tessalate_glyph(
    glyph_info: &cosmic_text::LayoutGlyph,
    bounding_box: Rect,
    face: Face,
    extrusion_depth: f32,
) -> Result<(ExtrudedGlyphGeometry, f32, f32), MeshTextError> {
    let units_per_em = face.units_per_em();
    // Scale factor to convert font units to layout units (e.g., based on font_size)
    let scale_factor = glyph_info.font_size / units_per_em as f32;

    let mut builder = crate::command_encoder::LyonCommandEncoder::new();
    face.outline_glyph(GlyphId(glyph_info.glyph_id), &mut builder)
        .ok_or(MeshTextError::PathBuildingFailed)?;
    let path = builder.build_path();

    // Calculate the center of the glyph using the font units bounding box
    // (font unit coordinates - these come directly from the font)
    let center_x = (bounding_box.x_min as f32 + bounding_box.x_max as f32) / 2.0;
    let center_y = (bounding_box.y_min as f32 + bounding_box.y_max as f32) / 2.0;

    let mut final_positions: Vec<Vec3> = Vec::new();
    let mut final_indices: Vec<u16> = Vec::new();
    let mut final_normals: Vec<Vec3> = Vec::new();
    let mut final_uvs: Vec<Vec2> = Vec::new();

    // Adjust front and back z positions
    let (front_z, back_z) = (0.0, extrusion_depth);

    let mut tessellator = FillTessellator::new();

    // 1. Tessellate front face (z=front_z)
    let mut front_geometry: VertexBuffers<Vec3, u16> = VertexBuffers::new();
    tessellator
        .tessellate_path(
            &path,
            &FillOptions::default(),
            &mut BuffersBuilder::new(&mut front_geometry, |vertex: FillVertex| Vec3 {
                // Subtract center to make rotation happen around the center of each glyph
                x: (vertex.position().x - center_x) * scale_factor,
                y: (vertex.position().y - center_y) * scale_factor,
                z: front_z,
            }),
        )
        .map_err(|_| MeshTextError::TessellationFailed)?;

    let front_v_offset = final_positions.len() as u16;
    for v_pos in &front_geometry.vertices {
        final_positions.push(*v_pos);
        final_normals.push(Vec3::NEG_Z); // Front face normal (0,0,-1)

        // Add basic UV mapping for front face based on normalized position
        // Find bounding box of the glyph for UV normalization
        let uv_x = (v_pos.x / (units_per_em as f32 * scale_factor) + 0.5) * 0.5 + 0.5;
        let uv_y = (v_pos.y / (units_per_em as f32 * scale_factor) + 0.5) * 0.5 + 0.5;
        final_uvs.push(Vec2::new(uv_x, uv_y));
    }
    for index in &front_geometry.indices {
        final_indices.push(front_v_offset + *index);
    }

    // 2. Tessellate back face (z=back_z)
    let mut back_geometry: VertexBuffers<Vec3, u16> = VertexBuffers::new();
    tessellator
        .tessellate_path(
            &path, // Tessellate the same path
            &FillOptions::default(),
            &mut BuffersBuilder::new(&mut back_geometry, |vertex: FillVertex| Vec3 {
                // Subtract center to make rotation happen around the center of each glyph
                x: (vertex.position().x - center_x) * scale_factor,
                y: (vertex.position().y - center_y) * scale_factor,
                z: back_z, // Shifted in Z
            }),
        )
        .map_err(|_| MeshTextError::TessellationFailed)?;

    let back_v_offset = final_positions.len() as u16;
    for v_pos in &back_geometry.vertices {
        final_positions.push(*v_pos);
        final_normals.push(Vec3::Z); // Back face normal (0,0,1)

        // Add UV mapping for back face - can be the same as front face
        // or mirrored depending on preference
        let uv_x = (v_pos.x / (units_per_em as f32 * scale_factor) + 0.5) * 0.5 + 0.5;
        let uv_y = (v_pos.y / (units_per_em as f32 * scale_factor) + 0.5) * 0.5 + 0.5;
        final_uvs.push(Vec2::new(uv_x, uv_y));
    }
    // Add back face indices with reversed winding for correct culling and normals
    for i in (0..back_geometry.indices.len()).step_by(3) {
        if i + 2 < back_geometry.indices.len() {
            // Ensure we have a full triangle
            final_indices.push(back_v_offset + back_geometry.indices[i + 2]);
            final_indices.push(back_v_offset + back_geometry.indices[i + 1]);
            final_indices.push(back_v_offset + back_geometry.indices[i]);
        }
    }

    // 3. Generate side faces by iterating over path segments
    let mut last_point_opt: Option<lyon::geom::Point<f32>> = None;
    let mut v_texture_offset = 0.0; // Tracks accumulated length for texture mapping

    for event in path.iter() {
        match event {
            PathEvent::Begin { at } => {
                last_point_opt = Some(at);
                // Reset texture coordinate offset at the start of each subpath
                v_texture_offset = 0.0;
            }
            PathEvent::Line { from, to } => {
                if last_point_opt.is_some() {
                    // For straight line segments, just add a quad directly
                    // Center the points using the same center values used for the front/back faces
                    let centered_from = point(from.x - center_x, from.y - center_y);
                    let centered_to = point(to.x - center_x, to.y - center_y);

                    add_side_quad(
                        &mut final_positions,
                        &mut final_indices,
                        &mut final_normals,
                        &mut final_uvs,
                        centered_from,
                        centered_to,
                        scale_factor,
                        extrusion_depth,
                        v_texture_offset,
                    );

                    // Update texture offset
                    let dx = to.x - from.x;
                    let dy = to.y - from.y;
                    v_texture_offset += (dx * dx + dy * dy).sqrt();
                }
                last_point_opt = Some(to);
            }
            PathEvent::End { last, first, close } => {
                // If the path is closed, connect the last point to the first point
                if close && last_point_opt.is_some() {
                    // Center the points using the same center values
                    let centered_last = point(last.x - center_x, last.y - center_y);
                    let centered_first = point(first.x - center_x, first.y - center_y);

                    add_side_quad(
                        &mut final_positions,
                        &mut final_indices,
                        &mut final_normals,
                        &mut final_uvs,
                        centered_last,
                        centered_first,
                        scale_factor,
                        extrusion_depth,
                        v_texture_offset,
                    );
                }

                // Reset for next potential sub-path
                last_point_opt = None;
                v_texture_offset = 0.0;
            }
            _ => panic!("We only have begin, end and lineTo events"),
        }
    }

    // Return the glyph dimensions for correct positioning
    Ok((
        ExtrudedGlyphGeometry {
            vertices: final_positions,
            indices: final_indices,
            normals: final_normals,
            uvs: final_uvs,
        },
        center_x * scale_factor,
        center_y * scale_factor,
    ))
}

// Helper function for adding side quads during extrusion
#[allow(clippy::too_many_arguments)]
fn add_side_quad(
    positions: &mut Vec<Vec3>,
    indices: &mut Vec<u16>,
    normals: &mut Vec<Vec3>,
    uvs: &mut Vec<Vec2>,
    p1_orig: lyon::geom::Point<f32>,
    p2_orig: lyon::geom::Point<f32>,
    scale: f32,
    depth: f32,
    v_texture_coord: f32, // Texture coordinate for mapping along the extrusion
) {
    let p1_front = Vec3::new(p1_orig.x * scale, p1_orig.y * scale, 0.0);
    let p2_front = Vec3::new(p2_orig.x * scale, p2_orig.y * scale, 0.0);
    let p1_back = Vec3::new(p1_orig.x * scale, p1_orig.y * scale, depth);
    let p2_back = Vec3::new(p2_orig.x * scale, p2_orig.y * scale, depth);

    let base_idx = positions.len() as u16;
    positions.extend_from_slice(&[p1_front, p2_front, p1_back, p2_back]);

    // Calculate side normal based on the 2D segment direction
    // Assuming CCW winding for outer contours, (p2_orig.x - p1_orig.x, p2_orig.y - p1_orig.y) is the tangent vector.
    // The outward normal is (tangent.y, -tangent.x).
    let dx = p2_orig.x - p1_orig.x;
    let dy = p2_orig.y - p1_orig.y;
    let side_normal = Vec3::new(dy, -dx, 0.0).normalize_or_zero();

    normals.extend_from_slice(&[side_normal, side_normal, side_normal, side_normal]);

    // Calculate texture coordinates
    // U coordinate will be based on position along the contour
    // Distance from p1 to p2 to calculate u texture coordinate
    let segment_length = ((p2_orig.x - p1_orig.x).powi(2) + (p2_orig.y - p1_orig.y).powi(2)).sqrt();
    // Use segment_length to normalize UV coordinates
    let u1 = 0.0; // Start of segment
    let u2 = segment_length * scale; // End of segment, scaled

    // V coordinate will be 0.0 at front face and 1.0 at back face
    let v1 = v_texture_coord; // Front face
    let v2 = v_texture_coord + 1.0; // Back face

    uvs.extend_from_slice(&[
        Vec2::new(u1, v1), // p1_front
        Vec2::new(u2, v1), // p2_front
        Vec2::new(u1, v2), // p1_back
        Vec2::new(u2, v2), // p2_back
    ]);

    // Quad vertices: p1_front, p2_front, p1_back, p2_back (indices base_idx, base_idx+1, base_idx+2, base_idx+3)
    // Tri 1: (p1_front, p2_front, p2_back) -> (base_idx+0, base_idx+1, base_idx+3)
    // Tri 2: (p1_front, p2_back, p1_back)  -> (base_idx+0, base_idx+3, base_idx+2)
    // This winding should make the normal (dy, -dx, 0) point outwards.
    indices.extend_from_slice(&[
        base_idx,
        base_idx + 1,
        base_idx + 3,
        base_idx,
        base_idx + 3,
        base_idx + 2,
    ]);
}
