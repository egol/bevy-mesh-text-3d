use bevy::prelude::*;
use lyon::path::{Path, PathEvent};
use crate::MeshTextError;

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

/// Extract contours from a lyon path
pub fn extract_contours(path: &Path, scale_factor: f32, center_x: f32, center_y: f32) -> Vec<Contour> {
    let mut contours = Vec::new();
    let mut current_contour = Vec::new();
    let mut start_point;
    
    for event in path.iter() {
        match event {
            PathEvent::Begin { at } => {
                current_contour.clear();
                start_point = Vec2::new(
                    (at.x - center_x) * scale_factor,
                    (at.y - center_y) * scale_factor,
                );
                current_contour.push(start_point);
            }
            PathEvent::Line { from: _, to } => {
                let point = Vec2::new(
                    (to.x - center_x) * scale_factor,
                    (to.y - center_y) * scale_factor,
                );
                current_contour.push(point);
            }
            PathEvent::End { last: _, first: _, close } => {
                if close && current_contour.len() > 2 {
                    // Remove the duplicate start point if it's closed
                    if current_contour.first() == current_contour.last() {
                        current_contour.pop();
                    }
                    contours.push(Contour {
                        vertices: current_contour.clone(),
                        is_closed: true,
                    });
                } else if current_contour.len() > 1 {
                    contours.push(Contour {
                        vertices: current_contour.clone(),
                        is_closed: false,
                    });
                }
                current_contour.clear();
            }
            _ => {
                // For curves, we assume they've been flattened by lyon
                panic!("Unexpected curve event in flattened path");
            }
        }
    }
    
    contours
}

/// Compute inset loop for bevel rim using bisector math
pub fn compute_bevel_rings(
    contours: &[Contour],
    bevel_width: f32,
    bevel_segments: u32,
    profile_power: f32,
    _glyph_id: u16,
) -> Result<Vec<BevelRings>, MeshTextError> {
    let mut all_rings = Vec::new();
    
    for contour in contours {
        if contour.vertices.len() < 3 {
            continue; // Skip degenerate contours
        }
        
        let inset_contour = compute_inset_contour(&contour.vertices, bevel_width, contour.is_closed)?;
        
        // Generate intermediate rings for curved profile
        let mut rings = Vec::new();
        if bevel_segments > 1 {
            for i in 1..bevel_segments {
                let t = (i as f32 / bevel_segments as f32).powf(profile_power);
                let ring_contour = interpolate_contours(&contour.vertices, &inset_contour, t, contour.is_closed);
                rings.push(Contour {
                    vertices: ring_contour,
                    is_closed: contour.is_closed,
                });
            }
        }
        
        #[cfg(feature = "debug")]
        println!("Checkpoint C: Computed bevel rings for glyph {} - outer: {} verts, inner: {} verts, rings: {}", 
                 _glyph_id, contour.vertices.len(), inset_contour.len(), rings.len());
        
        all_rings.push(BevelRings {
            outer_contour: contour.clone(),
            inner_contour: Contour {
                vertices: inset_contour,
                is_closed: contour.is_closed,
            },
            rings,
        });
    }
    
    Ok(all_rings)
}

/// Find the intersection point of two infinite lines defined by points and directions
fn line_intersection(p1: Vec2, d1: Vec2, p2: Vec2, d2: Vec2) -> Option<Vec2> {
    let denominator = d1.x * d2.y - d1.y * d2.x;
    
    // Lines are parallel if denominator is zero
    if denominator.abs() < f32::EPSILON {
        return None;
    }
    
    let dp = p2 - p1;
    let t = (dp.x * d2.y - dp.y * d2.x) / denominator;
    
    Some(p1 + t * d1)
}

/// Compute inset contour by offsetting each edge inward and finding intersections
fn compute_inset_contour(vertices: &[Vec2], bevel_width: f32, is_closed: bool) -> Result<Vec<Vec2>, MeshTextError> {
    if vertices.len() < 3 {
        return Err(MeshTextError::InvalidContour);
    }
    
    let mut inset_vertices = Vec::new();
    let len = vertices.len();
    
    // For each vertex, find the intersection of the two adjacent inset edges
    for i in 0..len {
        let prev_idx = if i == 0 {
            if is_closed { len - 1 } else { 0 }
        } else {
            i - 1
        };
        let next_idx = if i == len - 1 {
            if is_closed { 0 } else { len - 1 }
        } else {
            i + 1
        };
        
        let p_prev = vertices[prev_idx];
        let p_curr = vertices[i];
        let p_next = vertices[next_idx];
        
        // Handle boundary vertices for open contours
        if !is_closed && (i == 0 || i == len - 1) {
            let edge = if i == 0 { p_next - p_curr } else { p_curr - p_prev };
            let edge_len = edge.length();
            if edge_len > f32::EPSILON {
                // Inward normal (perpendicular to edge, pointing inward)
                let normal = Vec2::new(-edge.y, edge.x) / edge_len;
                inset_vertices.push(p_curr + normal * bevel_width);
            } else {
                inset_vertices.push(p_curr);
            }
            continue;
        }
        
        // Get the two edges
        let edge1 = p_curr - p_prev;
        let edge2 = p_next - p_curr;
        
        let edge1_len = edge1.length();
        let edge2_len = edge2.length();
        
        // Handle degenerate edges
        if edge1_len < f32::EPSILON || edge2_len < f32::EPSILON {
            inset_vertices.push(p_curr);
            continue;
        }
        
        // Calculate inward normals for both edges
        let normal1 = Vec2::new(-edge1.y, edge1.x) / edge1_len;
        let normal2 = Vec2::new(-edge2.y, edge2.x) / edge2_len;
        
        // Create the offset lines (infinite lines through the offset edge points)
        let offset_p1 = p_prev + normal1 * bevel_width;
        let offset_p2 = p_curr + normal1 * bevel_width;
        let offset_p3 = p_curr + normal2 * bevel_width;
        let offset_p4 = p_next + normal2 * bevel_width;
        
        // Find intersection of the two offset lines
        let line1_dir = offset_p2 - offset_p1; // Direction of first offset line
        let line2_dir = offset_p4 - offset_p3; // Direction of second offset line
        
        if let Some(intersection) = line_intersection(offset_p1, line1_dir, offset_p3, line2_dir) {
            // Check if the intersection is reasonable (not too far from original vertex)
            let distance_from_original = (intersection - p_curr).length();
            let max_reasonable_distance = bevel_width * 10.0; // Reasonable upper bound
            
            if distance_from_original <= max_reasonable_distance {
                inset_vertices.push(intersection);
            } else {
                // Fallback: use bisector method for extreme cases
                let bisector = (normal1 + normal2).normalize_or_zero();
                let bisector_length = if bisector.length() > f32::EPSILON {
                    // Calculate the distance along the bisector
                    let cos_half_angle = normal1.dot(normal2).max(-1.0).min(1.0);
                    let sin_half_angle = ((1.0 - cos_half_angle) / 2.0).sqrt();
                    if sin_half_angle > 0.1 {
                        bevel_width / sin_half_angle
                    } else {
                        bevel_width * 2.0 // Fallback for very sharp angles
                    }
                } else {
                    bevel_width
                };
                inset_vertices.push(p_curr + bisector * bisector_length);
            }
        } else {
            // Lines are parallel - use simple offset
            inset_vertices.push(p_curr + normal1 * bevel_width);
        }
    }
    
    // Validate the result
    if inset_vertices.len() != vertices.len() {
        return Err(MeshTextError::InvalidContour);
    }
    
    Ok(inset_vertices)
}

/// Interpolate between outer and inner contours for profile curves
fn interpolate_contours(outer: &[Vec2], inner: &[Vec2], t: f32, _is_closed: bool) -> Vec<Vec2> {
    assert_eq!(outer.len(), inner.len());
    
    outer.iter()
        .zip(inner.iter())
        .map(|(o, i)| o.lerp(*i, t))
        .collect()
}

/// Export contours as SVG for debugging (only available with debug feature)
#[cfg(feature = "debug")]
pub fn export_contours_svg(contours: &[Contour], filename: &str) -> Result<(), std::io::Error> {
    use std::fs::File;
    use std::io::Write;
    
    let mut file = File::create(filename)?;
    writeln!(file, r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-50 -50 100 100">"#)?;
    
    for (i, contour) in contours.iter().enumerate() {
        if contour.vertices.is_empty() {
            continue;
        }
        
        write!(file, r#"<polyline points=""#)?;
        for vertex in &contour.vertices {
            write!(file, "{},{} ", vertex.x, -vertex.y)?; // Flip Y for SVG
        }
        if contour.is_closed {
            let first = &contour.vertices[0];
            write!(file, "{},{} ", first.x, -first.y)?;
        }
        writeln!(file, r#"" fill="none" stroke="hsl({}, 70%, 50%)" stroke-width="0.5"/>"#, i * 60)?;
    }
    
    writeln!(file, "</svg>")?;
    Ok(())
} 