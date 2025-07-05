use bevy::prelude::*;
use lyon::path::{Path, PathEvent};
use crate::MeshTextError;

// Import cavalier_contours for robust polygon offsetting
use cavalier_contours::polyline::{Polyline, PlineVertex, PlineSourceMut};
use cavalier_contours::shape_algorithms::{Shape, ShapeOffsetOptions};
use std::collections::HashSet;

/// Represents a polygon contour with vertices
#[derive(Debug, Clone)]
pub struct Contour {
    pub vertices: Vec<Vec2>,
    pub is_closed: bool,
}

/// Represents bevel rings with outer and inner contours
#[derive(Debug, Clone)]
pub struct BevelRings {
    pub outer_contour: Contour,
    pub inner_contour: Contour,
    pub rings: Vec<Contour>, // Intermediate rings for curved profiles
}

/// Tolerance for vertex deduplication
const VERTEX_TOLERANCE: f32 = 1e-4;

/// Remove duplicate vertices from a list of vertices (both consecutive and non-consecutive)
fn deduplicate_vertices(vertices: &mut Vec<Vec2>) {
    if vertices.len() < 2 {
        return;
    }
    
    // First pass: remove consecutive duplicates
    let mut i = 0;
    while i < vertices.len() - 1 {
        if vertices[i].distance(vertices[i + 1]) < VERTEX_TOLERANCE {
            vertices.remove(i + 1);
        } else {
            i += 1;
        }
    }
    
    // Second pass: remove any remaining duplicates (non-consecutive)
    let mut unique_vertices = Vec::new();
    let mut seen_positions = HashSet::new();
    
    for vertex in vertices.iter() {
        // Create a hash key based on rounded coordinates
        let key = (
            (vertex.x / VERTEX_TOLERANCE).round() as i32,
            (vertex.y / VERTEX_TOLERANCE).round() as i32,
        );
        
        if !seen_positions.contains(&key) {
            seen_positions.insert(key);
            unique_vertices.push(*vertex);
        }
    }
    
    *vertices = unique_vertices;
}

/// Comprehensive vertex cleanup for cavalier_contours compatibility
fn cleanup_vertices_for_offset(vertices: &mut Vec<Vec2>) {
    if vertices.len() < 3 {
        return;
    }
    
    // Remove duplicates
    deduplicate_vertices(vertices);
    
    // Ensure we have enough vertices
    if vertices.len() < 3 {
        return;
    }
    
    // For closed contours, ensure the last vertex doesn't duplicate the first
    if vertices.len() > 3 {
        let first = vertices[0];
        let last = vertices[vertices.len() - 1];
        if first.distance(last) < VERTEX_TOLERANCE {
            vertices.pop();
        }
    }
    
    // Remove any vertices that are too close to their neighbors
    let mut i = 0;
    while i < vertices.len() && vertices.len() > 3 {
        let current = vertices[i];
        let next_idx = (i + 1) % vertices.len();
        let next = vertices[next_idx];
        
        if current.distance(next) < VERTEX_TOLERANCE {
            // Remove the vertex that's closer to the centroid (keep the more "important" one)
            vertices.remove(if i < next_idx { i } else { next_idx });
            if i >= vertices.len() {
                break;
            }
        } else {
            i += 1;
        }
    }
}

/// Extract contours from a lyon path
pub fn extract_contours(path: &Path, scale_factor: f32, center_x: f32, center_y: f32) -> Vec<Contour> {
    let mut contours = Vec::new();
    let mut current_vertices = Vec::new();
    let mut start_pos = Vec2::ZERO;
    
    for event in path.iter() {
        match event {
            PathEvent::Begin { at } => {
                current_vertices.clear();
                start_pos = Vec2::new(
                    (at.x * scale_factor) - center_x,
                    -((at.y * scale_factor) - center_y)
                );
                current_vertices.push(start_pos);
            }
            PathEvent::Line { from: _, to } => {
                let vertex = Vec2::new(
                    (to.x * scale_factor) - center_x,
                    -((to.y * scale_factor) - center_y)
                );
                current_vertices.push(vertex);
            }
            PathEvent::Quadratic { from: _, ctrl, to } => {
                // Approximate quadratic curve with multiple line segments
                let segments = 8;
                let from = current_vertices.last().copied().unwrap_or(Vec2::ZERO);
                let ctrl = Vec2::new(
                    (ctrl.x * scale_factor) - center_x,
                    -((ctrl.y * scale_factor) - center_y)
                );
                let to = Vec2::new(
                    (to.x * scale_factor) - center_x,
                    -((to.y * scale_factor) - center_y)
                );
                
                for i in 1..=segments {
                    let t = i as f32 / segments as f32;
                    let point = from * (1.0 - t) * (1.0 - t) + ctrl * 2.0 * (1.0 - t) * t + to * t * t;
                    current_vertices.push(point);
                }
            }
            PathEvent::Cubic { from: _, ctrl1, ctrl2, to } => {
                // Approximate cubic curve with multiple line segments
                let segments = 10;
                let from = current_vertices.last().copied().unwrap_or(Vec2::ZERO);
                let ctrl1 = Vec2::new(
                    (ctrl1.x * scale_factor) - center_x,
                    -((ctrl1.y * scale_factor) - center_y)
                );
                let ctrl2 = Vec2::new(
                    (ctrl2.x * scale_factor) - center_x,
                    -((ctrl2.y * scale_factor) - center_y)
                );
                let to = Vec2::new(
                    (to.x * scale_factor) - center_x,
                    -((to.y * scale_factor) - center_y)
                );
                
                for i in 1..=segments {
                    let t = i as f32 / segments as f32;
                    let point = from * (1.0 - t).powi(3) + 
                               ctrl1 * 3.0 * (1.0 - t).powi(2) * t +
                               ctrl2 * 3.0 * (1.0 - t) * t.powi(2) + 
                               to * t.powi(3);
                    current_vertices.push(point);
                }
            }
            PathEvent::End { close, .. } => {
                if current_vertices.len() >= 3 {
                    // Deduplicate vertices to prevent cavalier_contours issues
                    deduplicate_vertices(&mut current_vertices);
                    
                    // Close the contour if needed
                    if close {
                        if let Some(last) = current_vertices.last() {
                            if last.distance(start_pos) > VERTEX_TOLERANCE {
                                current_vertices.push(start_pos);
                            }
                        }
                    }
                    
                    contours.push(Contour {
                        vertices: current_vertices.clone(),
                        is_closed: close,
                    });
                }
                current_vertices.clear();
            }
        }
    }
    
    // Handle any remaining vertices
    if current_vertices.len() >= 3 {
        deduplicate_vertices(&mut current_vertices);
        contours.push(Contour {
            vertices: current_vertices,
            is_closed: false,
        });
    }
    
    contours
}

/// Convert a Contour to a cavalier_contours Polyline with proper cleanup
fn contour_to_polyline(contour: &Contour) -> Result<Polyline<f64>, MeshTextError> {
    let mut vertices = contour.vertices.clone();
    
    // Clean up vertices to prevent cavalier_contours issues
    cleanup_vertices_for_offset(&mut vertices);
    
    if vertices.len() < 3 {
        return Err(MeshTextError::InvalidContour);
    }
    
    let mut polyline = Polyline::new();
    
    // Add vertices to the polyline
    for vertex in &vertices {
        // Convert to f64 and add with bulge = 0.0 (no arcs for now)
        let pline_vertex = PlineVertex {
            x: vertex.x as f64,
            y: vertex.y as f64,
            bulge: 0.0,
        };
        polyline.add_vertex(pline_vertex);
    }
    
    // Set closed status
    polyline.set_is_closed(contour.is_closed);
    
    Ok(polyline)
}

/// Convert a cavalier_contours Polyline back to a Contour
fn polyline_to_contour(polyline: &Polyline<f64>) -> Contour {
    let mut vertices = Vec::new();
    
    for i in 0..polyline.vertex_data.len() {
        let vertex = &polyline.vertex_data[i];
        vertices.push(Vec2::new(vertex.x as f32, vertex.y as f32));
    }
    
    Contour {
        vertices,
        is_closed: polyline.is_closed,
    }
}

/// Compute bevel rings using cavalier_contours Shape API
pub fn compute_bevel_rings(
    contours: &[Contour],
    bevel_width: f32,
    bevel_segments: usize,
    profile_power: f32,
    _glyph_id: usize,
) -> Result<Vec<BevelRings>, MeshTextError> {
    if contours.is_empty() {
        return Ok(Vec::new());
    }
    
    // Convert contours to polylines
    let mut polylines = Vec::new();
    for (i, contour) in contours.iter().enumerate() {
        match contour_to_polyline(contour) {
            Ok(polyline) => {
                polylines.push(polyline);
            }
            Err(e) => {
                println!("DEBUG: Failed to convert contour {} to polyline: {:?}", i, e);
                continue;
            }
        }
    }
    
    if polylines.is_empty() {
        return Ok(Vec::new());
    }
    
    // Process each polyline as a separate shape (like the official example)
    let mut all_bevel_rings = Vec::new();
    
    for polyline in polylines.into_iter() {
        // Validate polyline before offset operations
        if polyline.vertex_data.len() < 3 {
            warn!("Skipping polyline with insufficient vertices: {}", polyline.vertex_data.len());
            continue;
        }
        
        // Create a shape from the single polyline
        let shape = Shape::from_plines(std::iter::once(polyline.clone()));
        
        // Create outer offset (positive)
        let outer_offset = shape.parallel_offset(bevel_width as f64, ShapeOffsetOptions::default());
        
        // Create inner offset (negative)
        let inner_offset = shape.parallel_offset(-(bevel_width as f64), ShapeOffsetOptions::default());
        
        // Convert outer offset results to contours
        let mut outer_contours = Vec::new();
        for indexed_pline in outer_offset.ccw_plines.iter().chain(outer_offset.cw_plines.iter()) {
            outer_contours.push(polyline_to_contour(&indexed_pline.polyline));
        }
        
        // Convert inner offset results to contours
        let mut inner_contours = Vec::new();
        for indexed_pline in inner_offset.ccw_plines.iter().chain(inner_offset.cw_plines.iter()) {
            inner_contours.push(polyline_to_contour(&indexed_pline.polyline));
        }
        
        // Generate intermediate rings for curved profiles
        let mut rings = Vec::new();
        
        if bevel_segments > 2 {
            for i in 1..bevel_segments {
                let t = i as f32 / bevel_segments as f32;
                // Apply profile power for curved transitions
                let profile_t = t.powf(profile_power);
                let offset_distance = bevel_width * (1.0 - profile_t);
                
                let ring_offset = shape.parallel_offset(offset_distance as f64, ShapeOffsetOptions::default());
                
                for indexed_pline in ring_offset.ccw_plines.iter().chain(ring_offset.cw_plines.iter()) {
                    rings.push(polyline_to_contour(&indexed_pline.polyline));
                }
            }
        }
        
        // Create bevel rings from the results
        // Use the original contour as fallback if no offsets were generated
        let original_contour = polyline_to_contour(&polyline);
        
        let outer_contour = outer_contours.first()
            .cloned()
            .unwrap_or_else(|| original_contour.clone());
        
        let inner_contour = inner_contours.first()
            .cloned()
            .unwrap_or_else(|| original_contour.clone());
        
        all_bevel_rings.push(BevelRings {
            outer_contour,
            inner_contour,
            rings,
        });
    }
    
    Ok(all_bevel_rings)
}

/// Calculate offset normal for a point on the contour
pub fn calculate_offset_normal(
    vertices: &[Vec2],
    index: usize,
    _offset_distance: f32,
) -> Vec2 {
    let len = vertices.len();
    if len < 2 {
        return Vec2::Y; // Default normal
    }
    
    let current = vertices[index];
    let prev = vertices[if index == 0 { len - 1 } else { index - 1 }];
    let next = vertices[(index + 1) % len];
    
    // Calculate edge vectors
    let edge1 = (current - prev).normalize_or_zero();
    let edge2 = (next - current).normalize_or_zero();
    
    // Calculate normals (perpendicular to edges)
    let normal1 = Vec2::new(-edge1.y, edge1.x);
    let normal2 = Vec2::new(-edge2.y, edge2.x);
    
    // Average the normals for smoother offset
    let avg_normal = (normal1 + normal2).normalize_or_zero();
    
    // If normalization failed, use a fallback
    if avg_normal.length() < 1e-6 {
        Vec2::new(-edge1.y, edge1.x).normalize_or_zero()
    } else {
        avg_normal
    }
}

