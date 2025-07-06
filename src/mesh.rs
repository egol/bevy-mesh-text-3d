use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use crate::offset::BevelRings;
use crate::MeshTextError;

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

/// Build improved bevel ring geometry with proper topology
fn build_improved_bevel_ring_geometry(
    vertices: &mut Vec<Vec3>,
    indices: &mut Vec<u32>,
    bevel_ring: &BevelRings,
    extrusion_depth: f32,
) -> Result<(), MeshTextError> {
    // Build ordered sequence of rings: outer -> intermediates -> inner
    let mut all_rings = vec![&bevel_ring.outer_contour];
    all_rings.extend(bevel_ring.rings.iter());
    all_rings.push(&bevel_ring.inner_contour);
    
    #[cfg(feature = "debug")]
    println!("Building bevel geometry with {} rings", all_rings.len());
    
    // Store vertex offset for each ring
    let mut ring_offsets = Vec::new();
    
    // Add vertices for each ring at progressively deeper Z levels
    for (ring_idx, ring) in all_rings.iter().enumerate() {
        let ring_offset = vertices.len();
        ring_offsets.push(ring_offset);
        
        // Calculate Z offset for proper bevel slope
        let z_offset = if all_rings.len() == 1 {
            0.0 // Single ring case
        } else {
            let t = ring_idx as f32 / (all_rings.len() - 1) as f32;
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
    for ring_idx in 0..all_rings.len() - 1 {
        let current_ring = &all_rings[ring_idx];
        let next_ring = &all_rings[ring_idx + 1];
        
        // Skip if rings have different vertex counts - this can happen with complex offsets
        if current_ring.vertices.len() != next_ring.vertices.len() {
            #[cfg(feature = "debug")]
            println!("Skipping ring connection {}->{} due to vertex count mismatch ({} vs {})", 
                     ring_idx, ring_idx + 1, current_ring.vertices.len(), next_ring.vertices.len());
            continue;
        }
        
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
    
    // Add back cap triangulation for the innermost (deepest) ring
    if let Some(inner_ring) = all_rings.last() {
        if inner_ring.vertices.len() >= 3 {
            let inner_offset = ring_offsets.last().unwrap();
            add_back_cap_triangulation(indices, *inner_offset as u32, inner_ring.vertices.len());
        }
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
    let max_reasonable_coord = 1000.0; // Reasonable bound for glyph coordinates
    
    for vertex in &geometry.vertices {
        if vertex.x.abs() > max_reasonable_coord || 
           vertex.y.abs() > max_reasonable_coord ||
           vertex.z.abs() > max_reasonable_coord {
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
            if area < 1e-6 {
                degenerate_triangles += 1;
            }
        }
    }
    
    // Check normal lengths (should be unit vectors within 1%)
    let mut invalid_normals = 0;
    for normal in &geometry.normals {
        let length = normal.length();
        if (length - 1.0).abs() > 0.01 {
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