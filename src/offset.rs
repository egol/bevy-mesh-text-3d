use bevy::prelude::*;
use lyon::path::{Path, PathEvent};
use crate::MeshTextError;

// Import cavalier_contours for robust polygon offsetting
use cavalier_contours::polyline::{Polyline, PlineVertex, PlineSource, PlineSourceMut};
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
pub fn contour_to_polyline(contour: &Contour) -> Result<Polyline<f64>, MeshTextError> {
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
pub fn polyline_to_contour(polyline: &Polyline<f64>) -> Contour {
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
    
    #[cfg(feature = "debug")]
    println!("Computing bevel rings for {} contours, bevel_width={}, segments={}", 
             contours.len(), bevel_width, bevel_segments);
    
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
    
    // Process each polyline as a separate shape (using the working pattern from test_glyph_offset.rs)
    let mut all_bevel_rings = Vec::new();
    
    for polyline in polylines.into_iter() {
        // Validate polyline before offset operations
        if polyline.vertex_data.len() < 3 {
            warn!("Skipping polyline with insufficient vertices: {}", polyline.vertex_data.len());
            continue;
        }
        
        // Create a shape from the single polyline (exactly like the working test)
        let shape = Shape::from_plines(std::iter::once(polyline.clone()));
        
        #[cfg(feature = "debug")]
        println!("Created shape with {} CCW plines, {} CW plines", 
                 shape.ccw_plines.len(), shape.cw_plines.len());
        
        // Generate progressive inward offsets (like the working test_glyph_offset.rs)
        let mut bevel_rings = Vec::new();
        // For n bevel segments, we need n+1 rings (outer + n intermediate/inner rings)
        let max_ring_count = bevel_segments.max(1) + 1;
        let options = ShapeOffsetOptions::default();
        
        // First ring is the original contour
        let original_contour = polyline_to_contour(&polyline);
        bevel_rings.push(original_contour.clone());
        
        // Generate inward offset shapes progressively
        // For bevel_segments = 1, we need to create one offset (2 rings total)
        // For bevel_segments = n, we need to create n offsets (n+1 rings total)
        let offset_step = bevel_width as f64 / bevel_segments as f64;
        let mut curr_offset = shape.parallel_offset(offset_step, options);
        
        while (!curr_offset.ccw_plines.is_empty() || !curr_offset.cw_plines.is_empty()) && bevel_rings.len() < max_ring_count {
            #[cfg(feature = "debug")]
            println!("Bevel ring {}: {} CCW plines, {} CW plines", 
                     bevel_rings.len(), curr_offset.ccw_plines.len(), curr_offset.cw_plines.len());
            
            // Convert offset results to contours
            for indexed_pline in curr_offset.ccw_plines.iter().chain(curr_offset.cw_plines.iter()) {
                bevel_rings.push(polyline_to_contour(&indexed_pline.polyline));
            }
            
            if bevel_rings.len() >= max_ring_count {
                break;
            }
            
            // Generate next offset with progressive stepping
            let current_step = (bevel_rings.len() as f64) * offset_step;
            curr_offset = shape.parallel_offset(current_step, ShapeOffsetOptions::default());
        }
        
        #[cfg(feature = "debug")]
        println!("Generated {} bevel rings total", bevel_rings.len());
        
        // Create BevelRings structure
        // For the new system, we'll use the rings array to store all progressive offsets
        let outer_contour = bevel_rings.first().cloned().unwrap_or(original_contour.clone());
        let inner_contour = bevel_rings.last().cloned().unwrap_or(original_contour.clone());
        
        // All intermediate rings (excluding first and last)
        let intermediate_rings = if bevel_rings.len() > 2 {
            bevel_rings[1..bevel_rings.len()-1].to_vec()
        } else {
            Vec::new()
        };
        
        all_bevel_rings.push(BevelRings {
            outer_contour,
            inner_contour,
            rings: intermediate_rings,
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

/// Approximate an arc defined by two polyline vertices with line segments
pub fn approximate_arc(
    start_vertex: PlineVertex<f64>, 
    end_vertex: PlineVertex<f64>, 
    segments: usize
) -> Vec<(f64, f64)> {
    let mut points = Vec::new();
    
    let start_x = start_vertex.x;
    let start_y = start_vertex.y;
    let end_x = end_vertex.x;
    let end_y = end_vertex.y;
    let bulge = start_vertex.bulge;
    
    // Calculate arc parameters from bulge
    let chord_len = ((end_x - start_x).powi(2) + (end_y - start_y).powi(2)).sqrt();
    let sagitta = chord_len * bulge.abs() / 2.0;
    let radius = (chord_len.powi(2) + 4.0 * sagitta.powi(2)) / (8.0 * sagitta);
    
    // Calculate center point
    let chord_mid_x = (start_x + end_x) / 2.0;
    let chord_mid_y = (start_y + end_y) / 2.0;
    
    let chord_dx = end_x - start_x;
    let chord_dy = end_y - start_y;
    
    let perp_dx = -chord_dy / chord_len;
    let perp_dy = chord_dx / chord_len;
    
    let center_offset = radius - sagitta;
    let center_offset = if bulge > 0.0 { center_offset } else { -center_offset };
    
    let center_x = chord_mid_x + perp_dx * center_offset;
    let center_y = chord_mid_y + perp_dy * center_offset;
    
    // Calculate start and end angles
    let start_angle = (start_y - center_y).atan2(start_x - center_x);
    let end_angle = (end_y - center_y).atan2(end_x - center_x);
    
    // Calculate sweep angle
    let mut sweep_angle = end_angle - start_angle;
    if bulge > 0.0 {
        if sweep_angle <= 0.0 {
            sweep_angle += 2.0 * std::f64::consts::PI;
        }
    } else {
        if sweep_angle >= 0.0 {
            sweep_angle -= 2.0 * std::f64::consts::PI;
        }
    }
    
    // Generate points along the arc
    points.push((start_x, start_y));
    
    for i in 1..segments {
        let t = i as f64 / segments as f64;
        let angle = start_angle + sweep_angle * t;
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();
        points.push((x, y));
    }
    
    points.push((end_x, end_y));
    points
}

/// Draw a polyline using Gizmos for debugging
pub fn draw_polyline(gizmos: &mut Gizmos, polyline: &Polyline<f64>, color: Color, z_offset: f32) {
    if polyline.vertex_count() < 2 {
        return;
    }
    
    for i in 0..polyline.vertex_count() {
        let current_vertex = polyline.at(i);
        let current_pos = Vec3::new(current_vertex.x as f32, current_vertex.y as f32, z_offset);
        
        // Determine next vertex index
        let next_i = if polyline.is_closed() {
            (i + 1) % polyline.vertex_count()
        } else if i == polyline.vertex_count() - 1 {
            continue; // Last vertex of open polyline
        } else {
            i + 1
        };
        
        let next_vertex = polyline.at(next_i);
        
        // Check if this is an arc (bulge != 0) or a line (bulge == 0)
        if current_vertex.bulge.abs() < 1e-10 {
            // Draw straight line
            let next_pos = Vec3::new(next_vertex.x as f32, next_vertex.y as f32, z_offset);
            gizmos.line(current_pos, next_pos, color);
        } else {
            // Draw arc approximation with line segments
            let segments = 16;
            let arc_points = approximate_arc(current_vertex, next_vertex, segments);
            
            for j in 0..arc_points.len() - 1 {
                let start_pos = Vec3::new(arc_points[j].0 as f32, arc_points[j].1 as f32, z_offset);
                let end_pos = Vec3::new(arc_points[j + 1].0 as f32, arc_points[j + 1].1 as f32, z_offset);
                gizmos.line(start_pos, end_pos, color);
            }
        }
    }
}

/// Draw contour outline using Gizmos for debugging
pub fn draw_contour_outline(gizmos: &mut Gizmos, contour: &Contour, color: Color, z_offset: f32) {
    if contour.vertices.len() < 2 {
        return;
    }
    
    // Draw contour as simple lines
    for i in 0..contour.vertices.len() {
        let current = contour.vertices[i];
        let next_i = if contour.is_closed {
            (i + 1) % contour.vertices.len()
        } else if i == contour.vertices.len() - 1 {
            continue; // Don't draw last segment for open contours
        } else {
            i + 1
        };
        
        let next = contour.vertices[next_i];
        let start_pos = Vec3::new(current.x, current.y, z_offset);
        let end_pos = Vec3::new(next.x, next.y, z_offset);
        gizmos.line(start_pos, end_pos, color);
    }
}

