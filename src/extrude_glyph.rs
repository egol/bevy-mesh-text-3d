use bevy::{asset::RenderAssetUsages, prelude::*, render::mesh::PrimitiveTopology};
use cosmic_text::ttf_parser::Rect;
use cosmic_text::ttf_parser::{Face, GlyphId};
use lyon::{
    geom::point,
    path::PathEvent,
    tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers},
};

use crate::MeshTextError;
use crate::BevelParameters;

// New beveling function using the modular approach with gizmo visualization
pub fn tessellate_beveled_glyph(
    glyph_info: &cosmic_text::LayoutGlyph,
    font_system: &mut cosmic_text::FontSystem,
    extrusion_depth: f32,
    bevel_params: &BevelParameters,
) -> Result<(ExtrudedGlyphGeometry, f32, f32), MeshTextError> {
    tessellate_beveled_glyph_with_gizmos(glyph_info, font_system, extrusion_depth, bevel_params, None)
}

// Beveling function with optional gizmo visualization
pub fn tessellate_beveled_glyph_with_gizmos(
    glyph_info: &cosmic_text::LayoutGlyph,
    font_system: &mut cosmic_text::FontSystem,
    extrusion_depth: f32,
    bevel_params: &BevelParameters,
    mut gizmos: Option<&mut Gizmos>,
) -> Result<(ExtrudedGlyphGeometry, f32, f32), MeshTextError> {
    #[cfg(feature = "debug")]
    if let Some(ref mut gizmos) = gizmos {
        // Draw coordinate system for reference
        let origin = Vec3::ZERO;
        gizmos.line(origin, origin + Vec3::X * 20.0, Color::srgb(1.0, 0.0, 0.0)); // Red X-axis
        gizmos.line(origin, origin + Vec3::Y * 20.0, Color::srgb(0.0, 1.0, 0.0)); // Green Y-axis
        gizmos.line(origin, origin + Vec3::Z * 20.0, Color::srgb(0.0, 0.0, 1.0)); // Blue Z-axis
    }

    // 1. Extract glyph outline
    let glyph_outline = crate::glyph::extract_glyph_outline(glyph_info, font_system)?;
    
    // #[cfg(feature = "debug")]
    // if let Some(ref mut gizmos) = gizmos {
    //     // Draw original glyph outline in white
    //     draw_glyph_outline_gizmo(gizmos, &glyph_outline.path, glyph_outline.font_size, glyph_outline.units_per_em, Color::WHITE);
    //     println!("Step 1: Drew original glyph outline for glyph {}", glyph_outline.glyph_id);
    // }
    
    // 2. Tessellate front cap
    let front_cap = crate::tess::tessellate_front_cap(
        &glyph_outline.path,
        glyph_outline.bounding_box,
        glyph_outline.font_size,
        glyph_outline.units_per_em,
        glyph_outline.glyph_id,
    )?;
    
    // #[cfg(feature = "debug")]
    // if let Some(ref mut gizmos) = gizmos {
    //     // Draw front cap tessellation in cyan
    //     draw_front_cap_gizmo(gizmos, &front_cap.vertices, &front_cap.indices, Color::srgb(0.0, 1.0, 1.0));
    //     println!("Step 2: Drew front cap tessellation with {} vertices, {} triangles", front_cap.vertices.len(), front_cap.indices.len() / 3);
    // }
    
    // 3. Extract contours for beveling
    let contours = crate::offset::extract_contours(
        &glyph_outline.path,
        front_cap.scale_factor,
        front_cap.center_x,
        front_cap.center_y,
    );
    
    #[cfg(feature = "debug")]
    if let Some(ref mut gizmos) = gizmos {
        // Draw extracted contours in yellow
        draw_contours_gizmo(gizmos, &contours, 0.0, Color::srgb(1.0, 1.0, 0.0));
        println!("Step 3: Drew {} extracted contours", contours.len());
    }
    
    // 4. Compute bevel rings
    let bevel_rings = crate::offset::compute_bevel_rings(
        &contours,
        bevel_params.bevel_width,
        bevel_params.bevel_segments as usize,
        bevel_params.profile_power,
        glyph_outline.glyph_id.into(),
    )?;
    
    #[cfg(feature = "debug")]
    if let Some(ref mut gizmos) = gizmos {
        // Draw bevel rings in different colors
        draw_bevel_rings_gizmo(gizmos, &bevel_rings, extrusion_depth);
        println!("Step 4: Drew {} bevel rings", bevel_rings.len());
    }
    
    // 5. Build complete beveled mesh
    let beveled_geometry = crate::mesh::build_beveled_mesh(
        &front_cap.vertices,
        &front_cap.indices,
        &bevel_rings,
        extrusion_depth,
        glyph_outline.glyph_id,
    )?;
    
    // #[cfg(feature = "debug")]
    // if let Some(ref mut gizmos) = gizmos {
    //     // Draw final mesh wireframe in magenta
    //     draw_mesh_wireframe_gizmo(gizmos, &beveled_geometry.vertices, &beveled_geometry.indices, Color::srgb(1.0, 0.0, 1.0));
    //     println!("Step 5: Drew final mesh wireframe with {} vertices, {} triangles", beveled_geometry.vertices.len(), beveled_geometry.indices.len() / 3);
    // }
    
    // 6. Validate mesh
    let _validation = crate::mesh::check_mesh(&beveled_geometry)?;
    
    #[cfg(feature = "debug")]
    {
        println!("Checkpoint F: Successfully created beveled glyph {} with {} vertices, {} triangles", 
                 glyph_outline.glyph_id, beveled_geometry.vertices.len(), beveled_geometry.indices.len() / 3);
    }
    
    // Convert to ExtrudedGlyphGeometry format for compatibility
    let extruded_geometry = ExtrudedGlyphGeometry {
        vertices: beveled_geometry.vertices,
        indices: beveled_geometry.indices,
        normals: beveled_geometry.normals,
        uvs: beveled_geometry.uvs,
    };
    
    Ok((
        extruded_geometry,
        front_cap.center_x,
        front_cap.center_y,
    ))
}

#[cfg(feature = "debug")]
fn draw_glyph_outline_gizmo(gizmos: &mut Gizmos, path: &lyon::path::Path, font_size: f32, units_per_em: u16, color: Color) {
    let scale_factor = font_size / units_per_em as f32;
    let mut last_point: Option<lyon::geom::Point<f32>> = None;
    
    for event in path.iter() {
        match event {
            lyon::path::PathEvent::Begin { at } => {
                last_point = Some(at);
            }
            lyon::path::PathEvent::Line { from: _, to } => {
                if let Some(from) = last_point {
                    let from_3d = Vec3::new(from.x * scale_factor, from.y * scale_factor, 0.0);
                    let to_3d = Vec3::new(to.x * scale_factor, to.y * scale_factor, 0.0);
                    gizmos.line(from_3d, to_3d, color);
                }
                last_point = Some(to);
            }
            lyon::path::PathEvent::End { last, first, close } => {
                if close && last_point.is_some() {
                    let last_3d = Vec3::new(last.x * scale_factor, last.y * scale_factor, 0.0);
                    let first_3d = Vec3::new(first.x * scale_factor, first.y * scale_factor, 0.0);
                    gizmos.line(last_3d, first_3d, color);
                }
                last_point = None;
            }
            _ => {}
        }
    }
}

#[cfg(feature = "debug")]
fn draw_front_cap_gizmo(gizmos: &mut Gizmos, vertices: &[Vec3], indices: &[u16], color: Color) {
    // Draw triangle edges
    for triangle in indices.chunks(3) {
        if triangle.len() == 3 {
            let v0 = vertices[triangle[0] as usize];
            let v1 = vertices[triangle[1] as usize];
            let v2 = vertices[triangle[2] as usize];
            
            gizmos.line(v0, v1, color);
            gizmos.line(v1, v2, color);
            gizmos.line(v2, v0, color);
        }
    }
}

#[cfg(feature = "debug")]
fn draw_contours_gizmo(gizmos: &mut Gizmos, contours: &[crate::offset::Contour], z_offset: f32, color: Color) {
    for contour in contours {
        for i in 0..contour.vertices.len() {
            let current = Vec3::new(contour.vertices[i].x, contour.vertices[i].y, z_offset);
            let next_idx = if contour.is_closed {
                (i + 1) % contour.vertices.len()
            } else if i == contour.vertices.len() - 1 {
                continue;
            } else {
                i + 1
            };
            let next = Vec3::new(contour.vertices[next_idx].x, contour.vertices[next_idx].y, z_offset);
            gizmos.line(current, next, color);
        }
    }
}

#[cfg(feature = "debug")]
fn draw_bevel_rings_gizmo(gizmos: &mut Gizmos, bevel_rings: &[crate::offset::BevelRings], extrusion_depth: f32) {
    for (ring_set_idx, bevel_ring) in bevel_rings.iter().enumerate() {
        let mut all_rings = vec![&bevel_ring.outer_contour];
        all_rings.extend(bevel_ring.rings.iter());
        all_rings.push(&bevel_ring.inner_contour);
        
        for (ring_idx, ring) in all_rings.iter().enumerate() {
            let z_offset = if ring_idx == 0 {
                0.0 // Outer ring at front
            } else if ring_idx == all_rings.len() - 1 {
                extrusion_depth // Inner ring at back
            } else {
                // Intermediate rings interpolated
                let t = ring_idx as f32 / (all_rings.len() - 1) as f32;
                t * extrusion_depth
            };
            
            // Different colors for different ring depths
            let color = match ring_idx {
                0 => Color::srgb(1.0, 0.0, 0.0),      // Red for outer ring
                idx if idx == all_rings.len() - 1 => Color::srgb(0.0, 0.0, 1.0), // Blue for inner ring
                _ => Color::srgb(0.0, 1.0, 0.0),      // Green for intermediate rings
            };
            
            draw_contours_gizmo(gizmos, &[(*ring).clone()], z_offset, color);
        }
    }
}

#[cfg(feature = "debug")]
fn draw_mesh_wireframe_gizmo(gizmos: &mut Gizmos, vertices: &[Vec3], indices: &[u32], color: Color) {
    // Draw triangle edges
    for triangle in indices.chunks(3) {
        if triangle.len() == 3 {
            let v0 = vertices[triangle[0] as usize];
            let v1 = vertices[triangle[1] as usize];
            let v2 = vertices[triangle[2] as usize];
            
            gizmos.line(v0, v1, color);
            gizmos.line(v1, v2, color);
            gizmos.line(v2, v0, color);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtrudedGlyphGeometry {
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
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
        .with_inserted_indices(bevy::render::mesh::Indices::U32(value.indices))
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
    let outline_result = face.outline_glyph(GlyphId(glyph_info.glyph_id), &mut builder);
    
    outline_result.ok_or(MeshTextError::PathBuildingFailed)?;
    let path = builder.build_path();

    // Check if the path is empty
    if path.iter().next().is_none() {
        warn!("Empty path for glyph {}", glyph_info.glyph_id);
        return Err(MeshTextError::PathBuildingFailed);
    }

    // Calculate the center of the glyph using the font units bounding box
    // (font unit coordinates - these come directly from the font)
    let center_x = (bounding_box.x_min as f32 + bounding_box.x_max as f32) / 2.0;
    let center_y = (bounding_box.y_min as f32 + bounding_box.y_max as f32) / 2.0;

    let mut final_positions: Vec<Vec3> = Vec::new();
    let mut final_indices: Vec<u32> = Vec::new();
    let mut final_normals: Vec<Vec3> = Vec::new();
    let mut final_uvs: Vec<Vec2> = Vec::new();

    // Adjust front and back z positions
    let (front_z, back_z) = (0.0, extrusion_depth);

    let mut tessellator = FillTessellator::new();

    // Try tessellation with different approaches
    let tessellation_result = try_tessellation_with_fallbacks(
        &mut tessellator,
        &path,
        center_x,
        center_y,
        scale_factor,
        front_z,
        back_z,
        glyph_info.glyph_id,
    );

    let (front_geometry, back_geometry) = match tessellation_result {
        Ok(result) => result,
        Err(e) => {
            error!("All tessellation attempts failed for glyph {}: {:?}", glyph_info.glyph_id, e);
            return Err(e);
        }
    };

    // Process front face
    let front_v_offset = final_positions.len() as u32;
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
        final_indices.push(front_v_offset + *index as u32);
    }

    // Process back face
    let back_v_offset = final_positions.len() as u32;
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
            final_indices.push(back_v_offset + back_geometry.indices[i + 2] as u32);
            final_indices.push(back_v_offset + back_geometry.indices[i + 1] as u32);
            final_indices.push(back_v_offset + back_geometry.indices[i] as u32);
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

fn try_tessellation_with_fallbacks(
    tessellator: &mut FillTessellator,
    path: &lyon::path::Path,
    center_x: f32,
    center_y: f32,
    scale_factor: f32,
    front_z: f32,
    back_z: f32,
    glyph_id: u16,
) -> Result<(VertexBuffers<Vec3, u16>, VertexBuffers<Vec3, u16>), MeshTextError> {
    // First attempt: Normal tessellation with default options
    let result = try_tessellation_with_options(
        tessellator,
        path,
        center_x,
        center_y,
        scale_factor,
        front_z,
        back_z,
        &FillOptions::default(),
    );
    
    if result.is_ok() {
        return result;
    }
    
    warn!("Normal tessellation failed for glyph {}, trying with tolerance", glyph_id);
    
    // Second attempt: Use higher tolerance
    let mut options = FillOptions::default();
    options.tolerance = 0.5; // Much higher tolerance
    
    let result = try_tessellation_with_options(
        tessellator,
        path,
        center_x,
        center_y,
        scale_factor,
        front_z,
        back_z,
        &options,
    );
    
    if result.is_ok() {
        return result;
    }
    
    warn!("High tolerance tessellation failed for glyph {}, trying non-zero fill rule", glyph_id);
    
    // Third attempt: Use non-zero fill rule
    let mut options = FillOptions::default();
    options.fill_rule = lyon::tessellation::FillRule::NonZero;
    
    let result = try_tessellation_with_options(
        tessellator,
        path,
        center_x,
        center_y,
        scale_factor,
        front_z,
        back_z,
        &options,
    );
    
    if result.is_ok() {
        return result;
    }
    
    error!("All tessellation attempts failed for glyph {}", glyph_id);
    Err(MeshTextError::TessellationFailed)
}

fn try_tessellation_with_options(
    tessellator: &mut FillTessellator,
    path: &lyon::path::Path,
    center_x: f32,
    center_y: f32,
    scale_factor: f32,
    front_z: f32,
    back_z: f32,
    options: &FillOptions,
) -> Result<(VertexBuffers<Vec3, u16>, VertexBuffers<Vec3, u16>), MeshTextError> {
    // 1. Tessellate front face (z=front_z)
    let mut front_geometry: VertexBuffers<Vec3, u16> = VertexBuffers::new();
    tessellator
        .tessellate_path(
            path,
            options,
            &mut BuffersBuilder::new(&mut front_geometry, |vertex: FillVertex| Vec3 {
                // Subtract center to make rotation happen around the center of each glyph
                x: (vertex.position().x - center_x) * scale_factor,
                y: (vertex.position().y - center_y) * scale_factor,
                z: front_z,
            }),
        )
        .map_err(|e| {
            debug!("Front face tessellation failed: {:?}", e);
            MeshTextError::TessellationFailed
        })?;

    // 2. Tessellate back face (z=back_z)
    let mut back_geometry: VertexBuffers<Vec3, u16> = VertexBuffers::new();
    tessellator
        .tessellate_path(
            path, // Tessellate the same path
            options,
            &mut BuffersBuilder::new(&mut back_geometry, |vertex: FillVertex| Vec3 {
                // Subtract center to make rotation happen around the center of each glyph
                x: (vertex.position().x - center_x) * scale_factor,
                y: (vertex.position().y - center_y) * scale_factor,
                z: back_z, // Shifted in Z
            }),
        )
        .map_err(|e| {
            debug!("Back face tessellation failed: {:?}", e);
            MeshTextError::TessellationFailed
        })?;

    Ok((front_geometry, back_geometry))
}

// Helper function for adding side quads during extrusion
#[allow(clippy::too_many_arguments)]
fn add_side_quad(
    positions: &mut Vec<Vec3>,
    indices: &mut Vec<u32>,
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

    let base_idx = positions.len() as u32;
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
