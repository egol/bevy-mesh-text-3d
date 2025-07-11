use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use crate::offset::BevelRings;
use crate::MeshTextError;
use lyon::path::Path;
use lyon::tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};

// Constants for mesh generation
const MAX_REASONABLE_COORD: f32 = 1000.0;
const DEGENERATE_TRIANGLE_THRESHOLD: f32 = 1e-6;
const NORMAL_LENGTH_TOLERANCE: f32 = 0.01;
const TESSELLATION_TOLERANCE: f32 = 0.25;
const FALLBACK_TESSELLATION_TOLERANCE: f32 = 0.5;

/// Resample a contour to have a specific number of vertices
/// This ensures all rings have matching vertex counts for proper bridging
fn resample_contour(contour: &crate::offset::Contour, target_count: usize) -> crate::offset::Contour {
    if contour.vertices.len() == target_count || contour.vertices.len() < 3 {
        return contour.clone();
    }
    
    let mut resampled_vertices = Vec::with_capacity(target_count);
    let source_count = contour.vertices.len();
    
    // Calculate the total perimeter
    let mut total_length = 0.0;
    let mut segment_lengths = Vec::with_capacity(source_count);
    
    for i in 0..source_count {
        let current = contour.vertices[i];
        let next_idx = if contour.is_closed {
            (i + 1) % source_count
        } else if i == source_count - 1 {
            break; // Don't include the last segment for open contours
        } else {
            i + 1
        };
        let next = contour.vertices[next_idx];
        let length = current.distance(next);
        segment_lengths.push(length);
        total_length += length;
    }
    
    if total_length < 1e-6 {
        // Degenerate contour, just duplicate the first vertex
        let first_vertex = contour.vertices[0];
        return crate::offset::Contour {
            vertices: vec![first_vertex; target_count],
            is_closed: contour.is_closed,
        };
    }
    
    // Resample at regular intervals along the perimeter
    let target_segment_length = total_length / target_count as f32;
    let mut current_distance = 0.0;
    let mut source_idx = 0;
    let mut segment_progress = 0.0;
    
    for _target_idx in 0..target_count {
        let target_distance = current_distance;
        
        // Find which source segment contains this target distance
        let mut accumulated_length = 0.0;
        let mut found_segment = false;
        
        for (seg_idx, &seg_length) in segment_lengths.iter().enumerate() {
            if target_distance <= accumulated_length + seg_length + 1e-6 {
                // This segment contains our target point
                let segment_start = contour.vertices[seg_idx];
                let segment_end_idx = if contour.is_closed {
                    (seg_idx + 1) % source_count
                } else {
                    (seg_idx + 1).min(source_count - 1)
                };
                let segment_end = contour.vertices[segment_end_idx];
                
                // Interpolate along the segment
                let t = if seg_length > 1e-6 {
                    (target_distance - accumulated_length) / seg_length
                } else {
                    0.0
                };
                
                let interpolated_point = segment_start + t * (segment_end - segment_start);
                resampled_vertices.push(interpolated_point);
                found_segment = true;
                break;
            }
            accumulated_length += seg_length;
        }
        
        if !found_segment {
            // Fallback: use the last vertex
            resampled_vertices.push(contour.vertices[source_count - 1]);
        }
        
        current_distance += target_segment_length;
    }
    
    crate::offset::Contour {
        vertices: resampled_vertices,
        is_closed: contour.is_closed,
    }
}

/// Determine the optimal vertex count for resampling all rings
fn determine_optimal_vertex_count(rings: &[&crate::offset::Contour]) -> usize {
    if rings.is_empty() {
        return 4; // Minimum for a reasonable shape
    }
    
    // Use the maximum vertex count among all rings as the target
    // This preserves the most detail
    let max_count = rings.iter().map(|ring| ring.vertices.len()).max().unwrap_or(4);
    
    // Clamp to reasonable bounds
    max_count.max(4).min(256) // At least 4, at most 256 vertices
}

/// Complete beveled glyph geometry
#[derive(Debug, Clone)]
pub struct BeveledGlyphGeometry {
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
}

/// Mesh validation parameters
#[derive(Debug)]
pub struct MeshValidation {
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub degenerate_triangles: usize,
    pub invalid_normals: usize,
    pub extreme_vertices: usize,
}

/// Build complete beveled mesh from tessellated front cap and bevel rings with improved geometry
pub fn build_beveled_mesh(
    front_cap_vertices: &[Vec3],
    front_cap_indices: &[u16],
    bevel_rings: &[BevelRings],
    extrusion_depth: f32,
    glyph_id: u16,
) -> Result<BeveledGlyphGeometry, MeshTextError> {
    // Use a simpler, more robust approach for bevel construction
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    // 1. Add front cap vertices
    for &vertex in front_cap_vertices {
        vertices.push(vertex);
    }
    
    // 2. Add front cap indices
    for &idx in front_cap_indices {
        indices.push(idx as u32);
    }
    
    // 3. Build bevel geometry with improved approach
    for bevel_ring in bevel_rings {
        build_improved_bevel_ring_geometry(
            &mut vertices,
            &mut indices,
            bevel_ring,
            extrusion_depth,
        )?;
    }
    
    // 4. Generate normals and UVs
    let normals = generate_smooth_normals(&vertices, &indices);
    let uvs = generate_uvs_for_beveled_mesh(&vertices, extrusion_depth);
    
    #[cfg(feature = "debug")]
    println!("Checkpoint E: Built beveled mesh for glyph {} with improved geometry - {} vertices, {} triangles", 
             glyph_id, vertices.len(), indices.len() / 3);
    
    Ok(BeveledGlyphGeometry {
        vertices,
        indices,
        normals,
        uvs,
    })
}

/// Build improved bevel ring geometry with proper topology and no gaps
fn build_improved_bevel_ring_geometry(
    vertices: &mut Vec<Vec3>,
    indices: &mut Vec<u32>,
    bevel_ring: &BevelRings,
    extrusion_depth: f32,
) -> Result<(), MeshTextError> {
    // Build ordered sequence of rings: outer -> intermediates -> inner -> outer_back
    let mut all_rings_refs = vec![&bevel_ring.outer_contour];
    all_rings_refs.extend(bevel_ring.rings.iter());
    all_rings_refs.push(&bevel_ring.inner_contour);
    
    // Add extra ring: copy of outer contour at z=extrusion_depth to bridge the gap
    all_rings_refs.push(&bevel_ring.outer_contour);
    
    #[cfg(feature = "debug")]
    println!("Building bevel geometry with {} rings (including back outer ring)", all_rings_refs.len());
    
    // Determine optimal vertex count for resampling
    let target_vertex_count = determine_optimal_vertex_count(&all_rings_refs);
    
    // Resample all rings to have matching vertex counts
    let mut resampled_rings = Vec::with_capacity(all_rings_refs.len());
    for ring_ref in all_rings_refs {
        let resampled = resample_contour(ring_ref, target_vertex_count);
        resampled_rings.push(resampled);
    }
    
    // Store vertex offset for each ring
    let mut ring_offsets = Vec::with_capacity(resampled_rings.len());
    
    // Add vertices for each ring at appropriate Z levels
    for (ring_idx, ring) in resampled_rings.iter().enumerate() {
        let ring_offset = vertices.len();
        ring_offsets.push(ring_offset);
        
        // Calculate Z offset for proper bevel slope
        let z_offset = if resampled_rings.len() <= 1 {
            0.0 // Single ring case (shouldn't happen with the extra ring)
        } else if ring_idx == resampled_rings.len() - 1 {
            // Last ring (outer contour copy) is at full extrusion depth
            extrusion_depth
        } else {
            // Progressive depth for bevel rings
            let bevel_ring_count = resampled_rings.len() - 1; // Exclude the last outer ring
            let t = ring_idx as f32 / (bevel_ring_count - 1) as f32;
            t * extrusion_depth
        };
        
        #[cfg(feature = "debug")]
        println!("Ring {} at Z={:.3} with {} vertices", ring_idx, z_offset, ring.vertices.len());
        
        // Add ring vertices
        for vertex in &ring.vertices {
            vertices.push(Vec3::new(vertex.x, vertex.y, z_offset));
        }
    }
    
    // Build triangles between consecutive rings to form bevel surface
    for ring_idx in 0..resampled_rings.len() - 1 {
        let current_ring = &resampled_rings[ring_idx];
        let next_ring = &resampled_rings[ring_idx + 1];
        
        // All rings now have the same vertex count, so no need to skip
        assert_eq!(current_ring.vertices.len(), next_ring.vertices.len(), 
                  "Ring {} vs {} vertex count mismatch after resampling", ring_idx, ring_idx + 1);
        
        let current_offset = ring_offsets[ring_idx] as u32;
        let next_offset = ring_offsets[ring_idx + 1] as u32;
        let vertex_count = current_ring.vertices.len();
        
        // Create triangles between rings with correct winding for outward-facing normals
        for i in 0..vertex_count {
            let next_i = if current_ring.is_closed { 
                (i + 1) % vertex_count 
            } else if i == vertex_count - 1 {
                continue; // Skip last edge for open contours
            } else {
                i + 1
            };
            
            let v0 = current_offset + i as u32;
            let v1 = current_offset + next_i as u32;
            let v2 = next_offset + next_i as u32;
            let v3 = next_offset + i as u32;
            
            // Create quad between rings with proper winding
            // First triangle of quad (v0, v1, v2)
            indices.push(v0);
            indices.push(v1);
            indices.push(v2);
            
            // Second triangle of quad (v0, v2, v3)
            indices.push(v0);
            indices.push(v2);
            indices.push(v3);
        }
    }
    
    // Add back cap triangulation for the last ring (outer contour at full depth)
    if let Some(last_offset) = ring_offsets.last() {
        add_back_cap_triangulation(indices, *last_offset as u32, target_vertex_count);
    }
    
    Ok(())
}

/// Add back cap triangulation using fan method
fn add_back_cap_triangulation(indices: &mut Vec<u32>, offset: u32, vertex_count: usize) {
    if vertex_count < 3 {
        return;
    }
    
    // Simple fan triangulation from first vertex
    for i in 1..vertex_count - 1 {
        // Reverse winding for back-facing triangles
        indices.push(offset);
        indices.push(offset + i as u32 + 1);
        indices.push(offset + i as u32);
    }
}

/// Generate smooth normals using vertex averaging
fn generate_smooth_normals(vertices: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    let mut normals = vec![Vec3::ZERO; vertices.len()];
    
    // Accumulate face normals at vertices
    for triangle in indices.chunks(3) {
        if triangle.len() == 3 {
            let i0 = triangle[0] as usize;
            let i1 = triangle[1] as usize;
            let i2 = triangle[2] as usize;
            
            if i0 < vertices.len() && i1 < vertices.len() && i2 < vertices.len() {
                let v0 = vertices[i0];
                let v1 = vertices[i1];
                let v2 = vertices[i2];
                
                let edge1 = v1 - v0;
                let edge2 = v2 - v0;
                let face_normal = edge1.cross(edge2);
                
                // Accumulate at each vertex
                normals[i0] += face_normal;
                normals[i1] += face_normal;
                normals[i2] += face_normal;
            }
        }
    }
    
    // Normalize accumulated normals
    for normal in &mut normals {
        *normal = normal.normalize_or_zero();
    }
    
    normals
}

/// Generate UV coordinates for beveled mesh
fn generate_uvs_for_beveled_mesh(vertices: &[Vec3], extrusion_depth: f32) -> Vec<Vec2> {
    vertices.iter().map(|vertex| {
        let u = (vertex.x + 50.0) / 100.0;
        let v = if extrusion_depth > 0.0 {
            vertex.z / extrusion_depth
        } else {
            (vertex.y + 50.0) / 100.0
        };
        Vec2::new(u, v)
    }).collect()
}

/// Validate mesh geometry
pub fn check_mesh(geometry: &BeveledGlyphGeometry) -> Result<MeshValidation, MeshTextError> {
    let vertex_count = geometry.vertices.len();
    let triangle_count = geometry.indices.len() / 3;
    
    // Check that we have valid triangles
    if geometry.indices.len() % 3 != 0 {
        return Err(MeshTextError::InvalidMesh("Index count not divisible by 3".to_string()));
    }
    
    // Check that all arrays have the same length
    if geometry.vertices.len() != geometry.normals.len() || 
       geometry.vertices.len() != geometry.uvs.len() {
        return Err(MeshTextError::InvalidMesh("Vertex attribute arrays have different lengths".to_string()));
    }
    
    // Check for extreme vertex coordinates that indicate geometry issues
    let mut extreme_vertices = 0;
    
    for vertex in &geometry.vertices {
        if vertex.x.abs() > MAX_REASONABLE_COORD || 
           vertex.y.abs() > MAX_REASONABLE_COORD ||
           vertex.z.abs() > MAX_REASONABLE_COORD {
            extreme_vertices += 1;
        }
        
        // Check for NaN or infinite values
        if !vertex.x.is_finite() || !vertex.y.is_finite() || !vertex.z.is_finite() {
            return Err(MeshTextError::InvalidMesh("Vertex contains NaN or infinite values".to_string()));
        }
    }
    
    // Count degenerate triangles (area ≈ 0)
    let mut degenerate_triangles = 0;
    for i in (0..geometry.indices.len()).step_by(3) {
        if i + 2 < geometry.indices.len() {
            let a = geometry.vertices[geometry.indices[i] as usize];
            let b = geometry.vertices[geometry.indices[i + 1] as usize];
            let c = geometry.vertices[geometry.indices[i + 2] as usize];
            
            let area = (b - a).cross(c - a).length();
            if area < DEGENERATE_TRIANGLE_THRESHOLD {
                degenerate_triangles += 1;
            }
        }
    }
    
    // Check normal lengths (should be unit vectors within tolerance)
    let mut invalid_normals = 0;
    for normal in &geometry.normals {
        let length = normal.length();
        if (length - 1.0).abs() > NORMAL_LENGTH_TOLERANCE {
            invalid_normals += 1;
        }
        
        // Check for NaN or infinite normals
        if !normal.x.is_finite() || !normal.y.is_finite() || !normal.z.is_finite() {
            return Err(MeshTextError::InvalidMesh("Normal contains NaN or infinite values".to_string()));
        }
    }
    
    let validation = MeshValidation {
        vertex_count,
        triangle_count,
        degenerate_triangles,
        invalid_normals,
        extreme_vertices,
    };
    
    #[cfg(feature = "debug")]
    println!("Mesh validation: {} vertices, {} triangles, {} degenerate, {} invalid normals, {} extreme vertices",
             validation.vertex_count, validation.triangle_count, 
             validation.degenerate_triangles, validation.invalid_normals,
             validation.extreme_vertices);
    
    // Fail if we have significant issues
    if degenerate_triangles > triangle_count / 10 {
        return Err(MeshTextError::InvalidMesh("Too many degenerate triangles".to_string()));
    }
    
    if invalid_normals > vertex_count / 10 {
        return Err(MeshTextError::InvalidMesh("Too many invalid normals".to_string()));
    }
    
    if validation.extreme_vertices > vertex_count / 20 {
        return Err(MeshTextError::InvalidMesh("Too many extreme vertex coordinates".to_string()));
    }
    
    Ok(validation)
}

/// Convert to Bevy mesh
impl From<BeveledGlyphGeometry> for bevy::render::mesh::Mesh {
    fn from(geometry: BeveledGlyphGeometry) -> Self {
        let mut mesh = bevy::render::mesh::Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
        
        mesh.insert_attribute(bevy::render::mesh::Mesh::ATTRIBUTE_POSITION, geometry.vertices);
        mesh.insert_attribute(bevy::render::mesh::Mesh::ATTRIBUTE_NORMAL, geometry.normals);
        mesh.insert_attribute(bevy::render::mesh::Mesh::ATTRIBUTE_UV_0, geometry.uvs);
        mesh.insert_indices(Indices::U32(geometry.indices));
        
        mesh
    }
}

/// Build a complete mesh directly from bevel rings, including tessellated caps
pub fn build_mesh_from_bevel_rings(
    bevel_rings: &[BevelRings],
    extrusion_depth: f32,
    glyph_id: u16,
) -> Result<BeveledGlyphGeometry, MeshTextError> {
    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();
    
    // First, build all bevel ring geometry to establish vertex layout
    let mut front_cap_boundary_vertices = Vec::new();
    let mut back_cap_boundary_vertices = Vec::new();
    
    for bevel_ring in bevel_rings {
        let bevel_start_idx = all_vertices.len() as u32;
        
        // Build bevel ring geometry and track boundary vertices
        let (front_boundary, back_boundary) = build_bevel_ring_geometry_with_boundaries(
            &mut all_vertices,
            &mut all_indices,
            bevel_ring,
            extrusion_depth,
            bevel_start_idx,
        )?;
        
        front_cap_boundary_vertices.extend(front_boundary);
        back_cap_boundary_vertices.extend(back_boundary);
    }
    
    // Now tessellate caps with proper boundary connections
    tessellate_and_connect_caps(
        &mut all_vertices,
        &mut all_indices,
        bevel_rings,
        &front_cap_boundary_vertices,
        &back_cap_boundary_vertices,
        extrusion_depth,
    )?;
    
    // Generate normals and UVs
    let normals = generate_smooth_normals(&all_vertices, &all_indices);
    let uvs = generate_uvs_for_beveled_mesh(&all_vertices, extrusion_depth);
    
    #[cfg(feature = "debug")]
    println!("Built complete mesh from {} bevel rings for glyph {} - {} vertices, {} triangles", 
             bevel_rings.len(), glyph_id, all_vertices.len(), all_indices.len() / 3);
    
    Ok(BeveledGlyphGeometry {
        vertices: all_vertices,
        indices: all_indices,
        normals,
        uvs,
    })
}

/// Build bevel ring geometry and return boundary vertex indices
fn build_bevel_ring_geometry_with_boundaries(
    vertices: &mut Vec<Vec3>,
    indices: &mut Vec<u32>,
    bevel_ring: &BevelRings,
    extrusion_depth: f32,
    base_vertex_offset: u32,
) -> Result<(Vec<u32>, Vec<u32>), MeshTextError> {
    // Build ordered sequence of rings: outer -> intermediates -> inner
    let mut all_rings = vec![&bevel_ring.outer_contour];
    all_rings.extend(bevel_ring.rings.iter());
    all_rings.push(&bevel_ring.inner_contour);
    
    if all_rings.len() < 2 {
        return Err(MeshTextError::InvalidInput);
    }
    
    // Store vertex offset for each ring
    let mut ring_offsets = Vec::new();
    
    // Add vertices for each ring at progressively deeper Z levels
    for (ring_idx, ring) in all_rings.iter().enumerate() {
        let ring_offset = vertices.len() - base_vertex_offset as usize;
        ring_offsets.push(ring_offset);
        
        // Calculate Z offset for proper bevel slope
        let z_offset = if all_rings.len() == 1 {
            0.0
        } else {
            let t = ring_idx as f32 / (all_rings.len() - 1) as f32;
            t * extrusion_depth
        };
        
        // Add ring vertices
        for vertex in &ring.vertices {
            vertices.push(Vec3::new(vertex.x, vertex.y, z_offset));
        }
    }
    
    // Track boundary vertices for cap tessellation
    let front_boundary: Vec<u32> = (base_vertex_offset + ring_offsets[0] as u32..
                                   base_vertex_offset + ring_offsets[0] as u32 + all_rings[0].vertices.len() as u32)
                                   .collect();
    let back_boundary: Vec<u32> = (base_vertex_offset + *ring_offsets.last().unwrap() as u32..
                                  base_vertex_offset + *ring_offsets.last().unwrap() as u32 + all_rings.last().unwrap().vertices.len() as u32)
                                  .collect();
    
    // Build triangles between consecutive rings
    for ring_idx in 0..all_rings.len() - 1 {
        let current_ring = &all_rings[ring_idx];
        let next_ring = &all_rings[ring_idx + 1];
        
        // Skip if rings have incompatible vertex counts
        if current_ring.vertices.len() != next_ring.vertices.len() {
            continue;
        }
        
        let current_offset = base_vertex_offset + ring_offsets[ring_idx] as u32;
        let next_offset = base_vertex_offset + ring_offsets[ring_idx + 1] as u32;
        let vertex_count = current_ring.vertices.len();
        
        // Create triangles between rings
        for i in 0..vertex_count {
            let next_i = if current_ring.is_closed { 
                (i + 1) % vertex_count 
            } else if i == vertex_count - 1 {
                continue;
            } else {
                i + 1
            };
            
            let v0 = current_offset + i as u32;
            let v1 = current_offset + next_i as u32;
            let v2 = next_offset + next_i as u32;
            let v3 = next_offset + i as u32;
            
            // Create quad between rings with proper winding
            indices.push(v0);
            indices.push(v1);
            indices.push(v2);
            
            indices.push(v0);
            indices.push(v2);
            indices.push(v3);
        }
    }
    
    Ok((front_boundary, back_boundary))
}

/// Tessellate caps and connect them to boundary vertices
fn tessellate_and_connect_caps(
    vertices: &mut Vec<Vec3>,
    indices: &mut Vec<u32>,
    bevel_rings: &[BevelRings],
    front_boundary_vertices: &[u32],
    back_boundary_vertices: &[u32],
    extrusion_depth: f32,
) -> Result<(), MeshTextError> {
    // Group contours for tessellation
    let outer_contours: Vec<&crate::offset::Contour> = bevel_rings.iter()
        .map(|ring| &ring.outer_contour)
        .collect();
    let inner_contours: Vec<&crate::offset::Contour> = bevel_rings.iter()
        .map(|ring| &ring.inner_contour)
        .collect();
    
    // Tessellate front cap (but only the interior, since boundary vertices already exist)
    let front_cap = tessellate_contours_as_face_with_holes(&outer_contours, 0.0)?;
    tessellate_cap_interior_and_connect_to_boundary(
        vertices,
        indices,
        &front_cap,
        front_boundary_vertices,
        &outer_contours,
        0.0,
        false, // front face - normal winding
    )?;
    
    // Tessellate back cap (but only the interior, since boundary vertices already exist)
    let back_cap = tessellate_contours_as_face_with_holes(&inner_contours, extrusion_depth)?;
    tessellate_cap_interior_and_connect_to_boundary(
        vertices,
        indices,
        &back_cap,
        back_boundary_vertices,
        &inner_contours,
        extrusion_depth,
        true, // back face - reverse winding
    )?;
    
    Ok(())
}

/// Tessellate cap interior and connect to existing boundary vertices
fn tessellate_cap_interior_and_connect_to_boundary(
    vertices: &mut Vec<Vec3>,
    indices: &mut Vec<u32>,
    cap_geometry: &CapGeometry,
    boundary_vertices: &[u32],
    contours: &[&crate::offset::Contour],
    z_offset: f32,
    reverse_winding: bool,
) -> Result<(), MeshTextError> {
    // Simple approach: just use the tessellated cap geometry
    // In a more sophisticated implementation, we would:
    // 1. Identify which tessellated vertices are on the boundary
    // 2. Map them to existing boundary vertices
    // 3. Only add interior vertices
    // For now, we'll use the tessellated geometry as-is
    
    let vertex_offset = vertices.len() as u32;
    vertices.extend(cap_geometry.vertices.iter().cloned());
    
    for triangle in cap_geometry.indices.chunks(3) {
        if triangle.len() == 3 {
            if reverse_winding {
                // Reverse winding for back face
                indices.push(vertex_offset + triangle[0] as u32);
                indices.push(vertex_offset + triangle[2] as u32);
                indices.push(vertex_offset + triangle[1] as u32);
            } else {
                // Normal winding for front face
                indices.push(vertex_offset + triangle[0] as u32);
                indices.push(vertex_offset + triangle[1] as u32);
                indices.push(vertex_offset + triangle[2] as u32);
            }
        }
    }
    
    Ok(())
}

/// Tessellate multiple contours as a single face with holes using Lyon
fn tessellate_contours_as_face_with_holes(
    contours: &[&crate::offset::Contour], 
    z_offset: f32
) -> Result<CapGeometry, MeshTextError> {
    if contours.is_empty() {
        return Err(MeshTextError::InvalidInput);
    }
    
    // Determine which contours are outer (CCW winding) and which are holes (CW winding)
    let mut outer_contours = Vec::new();
    let mut hole_contours = Vec::new();
    
    for contour in contours {
        if contour.vertices.len() < 3 {
            continue;
        }
        
        // Calculate signed area to determine winding order
        let signed_area = calculate_signed_area(&contour.vertices);
        
        if signed_area > 0.0 {
            // Counter-clockwise (positive area) = outer boundary
            outer_contours.push(*contour);
        } else {
            // Clockwise (negative area) = hole
            hole_contours.push(*contour);
        }
    }
    
    // If no outer contours, treat all as outer
    if outer_contours.is_empty() {
        outer_contours = contours.to_vec();
        hole_contours.clear();
    }
    
    // Create a Lyon path with outer contours and holes
    let mut path_builder = Path::builder();
    
    // Add outer contours
    for contour in &outer_contours {
        add_contour_to_path_builder(&mut path_builder, contour, false)?;
    }
    
    // Add holes (reverse their winding)
    for contour in &hole_contours {
        add_contour_to_path_builder(&mut path_builder, contour, true)?;
    }
    
    let path = path_builder.build();
    
    // Tessellate the path with holes
    let mut tessellator = FillTessellator::new();
    let mut geometry: VertexBuffers<Vec3, u16> = VertexBuffers::new();
    
    let mut options = FillOptions::default();
    options.tolerance = TESSELLATION_TOLERANCE;
    options.fill_rule = lyon::tessellation::FillRule::EvenOdd; // Better for handling holes
    
    let result = tessellator.tessellate_path(
        &path,
        &options,
        &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| Vec3 {
            x: vertex.position().x,
            y: vertex.position().y,
            z: z_offset,
        }),
    );
    
    if result.is_err() {
        // Try with higher tolerance
        geometry.vertices.clear();
        geometry.indices.clear();
        options.tolerance = FALLBACK_TESSELLATION_TOLERANCE;
        
        tessellator.tessellate_path(
            &path,
            &options,
            &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| Vec3 {
                x: vertex.position().x,
                y: vertex.position().y,
                z: z_offset,
            }),
        ).map_err(|_| MeshTextError::TessellationFailed)?;
    }
    
    #[cfg(feature = "debug")]
    println!("Tessellated face with {} outer contours and {} holes - {} vertices, {} triangles", 
             outer_contours.len(), hole_contours.len(), geometry.vertices.len(), geometry.indices.len() / 3);
    
    Ok(CapGeometry {
        vertices: geometry.vertices,
        indices: geometry.indices,
    })
}

/// Calculate signed area of a polygon to determine winding order
fn calculate_signed_area(vertices: &[Vec2]) -> f32 {
    if vertices.len() < 3 {
        return 0.0;
    }
    
    let mut area = 0.0;
    let n = vertices.len();
    
    for i in 0..n {
        let j = (i + 1) % n;
        area += vertices[i].x * vertices[j].y;
        area -= vertices[j].x * vertices[i].y;
    }
    
    area / 2.0
}

/// Add a contour to the path builder
fn add_contour_to_path_builder(
    path_builder: &mut lyon::path::path::Builder,
    contour: &crate::offset::Contour,
    reverse_winding: bool,
) -> Result<(), MeshTextError> {
    if contour.vertices.len() < 3 {
        return Err(MeshTextError::InvalidContour);
    }
    
    let vertices = if reverse_winding {
        // Reverse the order of vertices to flip winding
        contour.vertices.iter().rev().collect::<Vec<_>>()
    } else {
        contour.vertices.iter().collect::<Vec<_>>()
    };
    
    // Start the path
    let first_vertex = vertices[0];
    path_builder.begin(lyon::geom::point(first_vertex.x, first_vertex.y));
    
    // Add remaining vertices
    for vertex in vertices.iter().skip(1) {
        path_builder.line_to(lyon::geom::point(vertex.x, vertex.y));
    }
    
    // Close the path if it's a closed contour
    if contour.is_closed {
        path_builder.close();
    } else {
        path_builder.end(false);
    }
    
    Ok(())
}

/// Geometry for a tessellated cap
#[derive(Debug, Clone)]
struct CapGeometry {
    vertices: Vec<Vec3>,
    indices: Vec<u16>,
} 