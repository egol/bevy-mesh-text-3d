use bevy::prelude::*;
use cosmic_text::{
    Align, Attrs, Buffer, FontSystem, Metrics, Shaping,
};
use cavalier_contours::{
    polyline::{PlineSource, Polyline},
    shape_algorithms::{Shape, ShapeOffsetOptions},
};

use bevy_mesh_text_3d::{
    glyph::{extract_glyph_outline, GlyphOutline},
    offset::{extract_contours, Contour},
    MeshTextError,
};

#[derive(Resource)]
struct GlyphTestResults {
    original_contours: Vec<Contour>,
    original_shape: Option<Shape<f64>>,
    offset_shapes: Vec<Shape<f64>>,
    computed: bool,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, test_glyph_offset_system)
        .add_systems(Update, draw_2d_visualization)
        .add_systems(Update, keyboard_input)
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
        .insert_resource(GlyphTestResults {
            original_contours: Vec::new(),
            original_shape: None,
            offset_shapes: Vec::new(),
            computed: false,
        })
        .run();
}

fn keyboard_input(keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }
}

fn setup(mut commands: Commands) {
    // Setup camera to look at origin (2D view)
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 150.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Add light
    commands.spawn((
        DirectionalLight {
            color: Color::WHITE,
            illuminance: 5000.0,
            ..default()
        },
        Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn test_glyph_offset_system(mut test_results: ResMut<GlyphTestResults>) {
    if test_results.computed {
        return; // Test already completed
    }
    
    println!("=== TESTING GLYPH OFFSET SYSTEM (Letter A) ===");
    
    // Create a font system
    let mut font_system = FontSystem::new();
    
    // Create a simple buffer with the letter "A"
    let metrics = Metrics::new(72.0, 72.0); // Large font size for better visibility
    let mut buffer = Buffer::new_empty(metrics);
    let attrs = Attrs::new();
    
    buffer.set_rich_text(
        &mut font_system,
        [("A", attrs.clone())],
        &attrs,
        Shaping::Advanced,
        Some(Align::Center),
    );
    
    // Set buffer size and shape
    buffer.set_size(&mut font_system, Some(200.0), Some(200.0));
    buffer.shape_until_scroll(&mut font_system, false);
    
    println!("Created text buffer for letter 'A'");
    
    // Extract glyph information
    let mut glyph_found = false;
    let mut glyph_outline: Option<GlyphOutline> = None;
    
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            if glyph.glyph_id != 0 { // Skip whitespace
                println!("Found glyph: ID={}, font_size={}, x={}, y={}", 
                         glyph.glyph_id, glyph.font_size, glyph.x, glyph.y);
                
                // Extract glyph outline
                match extract_glyph_outline(glyph, &mut font_system) {
                    Ok(outline) => {
                        println!("Successfully extracted glyph outline");
                        println!("  Bounding box: {:?}", outline.bounding_box);
                        println!("  Font size: {}", outline.font_size);
                        println!("  Units per em: {}", outline.units_per_em);
                        
                        glyph_outline = Some(outline);
                        glyph_found = true;
                        break;
                    }
                    Err(e) => {
                        println!("Failed to extract glyph outline: {:?}", e);
                    }
                }
            }
        }
        if glyph_found {
            break;
        }
    }
    
    let Some(outline) = glyph_outline else {
        println!("❌ No glyph outline found for letter 'A'");
        return;
    };
    
    // Convert Lyon path to contours with proper scaling
    // Scale from font units to reasonable world units
    let scale_factor = outline.font_size / outline.units_per_em as f32;
    
    // Center the glyph at origin
    let glyph_width = (outline.bounding_box.x_max - outline.bounding_box.x_min) as f32 * scale_factor;
    let glyph_height = (outline.bounding_box.y_max - outline.bounding_box.y_min) as f32 * scale_factor;
    let center_x = glyph_width / 2.0;
    let center_y = glyph_height / 2.0;
    
    println!("Glyph scaling: scale_factor={:.4}, size={}x{}", scale_factor, glyph_width, glyph_height);
    
    let contours = extract_contours(&outline.path, scale_factor, center_x, center_y);
    println!("Extracted {} contours from glyph path", contours.len());
    
    if contours.is_empty() {
        println!("❌ No contours extracted from glyph path");
        return;
    }
    
    // Convert contours to polylines
    let mut polylines = Vec::new();
    for (i, contour) in contours.iter().enumerate() {
        println!("Contour {}: {} vertices, closed: {}", 
                 i, contour.vertices.len(), contour.is_closed);
        
        match contour_to_polyline(contour) {
            Ok(polyline) => {
                println!("  → Converted to polyline: {} vertices", polyline.vertex_count());
                polylines.push(polyline);
            }
            Err(e) => {
                println!("  → Failed to convert contour to polyline: {:?}", e);
            }
        }
    }
    
    if polylines.is_empty() {
        println!("❌ No polylines created from contours");
        return;
    }
    
    // Create shape from polylines (exactly like the official example)
    let shape = Shape::from_plines(polylines.iter().cloned());
    println!("Created shape with {} CCW plines, {} CW plines", 
             shape.ccw_plines.len(), shape.cw_plines.len());
    
    // Test offset functionality using EXACT same approach as test_offset_only.rs
    let offset = 1.0;
    let max_offset_count = 25;
    let options = ShapeOffsetOptions::default();
    
    println!("\nTesting offset with distance: {}", offset);
    
    // Generate multiple offset shapes (EXACTLY like the official example)
    let mut offset_shapes = Vec::new();
    let mut curr_offset = shape.parallel_offset(offset, options);
    
    while !curr_offset.ccw_plines.is_empty() || !curr_offset.cw_plines.is_empty() {
        println!("Offset iteration {}: {} CCW plines, {} CW plines", 
                 offset_shapes.len(), curr_offset.ccw_plines.len(), curr_offset.cw_plines.len());
        offset_shapes.push(curr_offset);
        if offset_shapes.len() >= max_offset_count {
            break;
        }

        curr_offset = offset_shapes
            .last()
            .unwrap()
            .parallel_offset(offset, ShapeOffsetOptions::default());
    }
    
    println!("Generated {} offset shapes for letter 'A'", offset_shapes.len());
    
    // Store results for visualization
    test_results.original_contours = contours;
    test_results.original_shape = Some(shape);
    test_results.offset_shapes = offset_shapes;
    test_results.computed = true;
    
    println!("\n✅ GLYPH OFFSET TEST COMPLETED SUCCESSFULLY!");
    println!("Press ESC to exit, view the 2D visualization to see letter 'A' with offset results");
}

fn draw_2d_visualization(mut gizmos: Gizmos, test_results: Res<GlyphTestResults>) {
    if !test_results.computed {
        return; // Nothing to draw yet
    }
    
    // Draw original contours as lines (for comparison)
    for contour in &test_results.original_contours {
        let color = Color::srgb(0.5, 0.5, 0.5); // Gray for original
        draw_contour_outline(&mut gizmos, contour, color, -0.1);
    }
    
    // Draw original shape in white
    if let Some(original_shape) = &test_results.original_shape {
        for pline in &original_shape.ccw_plines {
            draw_polyline(&mut gizmos, &pline.polyline, Color::WHITE, 0.0);
        }
        for pline in &original_shape.cw_plines {
            draw_polyline(&mut gizmos, &pline.polyline, Color::WHITE, 0.0);
        }
    }
    
    // Draw offset shapes in different colors
    let colors = [
        Color::srgb(1.0, 0.0, 0.0), // Red
        Color::srgb(0.0, 1.0, 0.0), // Green  
        Color::srgb(0.0, 0.0, 1.0), // Blue
        Color::srgb(1.0, 1.0, 0.0), // Yellow
        Color::srgb(1.0, 0.0, 1.0), // Magenta
        Color::srgb(0.0, 1.0, 1.0), // Cyan
        Color::srgb(1.0, 0.5, 0.0), // Orange
        Color::srgb(0.5, 0.0, 1.0), // Purple
    ];
    
    for (i, offset_shape) in test_results.offset_shapes.iter().enumerate() {
        let color = colors[i % colors.len()];
        let z_offset = 0.1 + (i as f32 * 0.02);
        
        for pline in &offset_shape.ccw_plines {
            draw_polyline(&mut gizmos, &pline.polyline, color, z_offset);
        }
        for pline in &offset_shape.cw_plines {
            draw_polyline(&mut gizmos, &pline.polyline, color, z_offset);
        }
    }
    
    // Draw coordinate system for reference
    let origin = Vec3::ZERO;
    gizmos.line(origin, origin + Vec3::X * 20.0, Color::srgb(1.0, 0.5, 0.5)); // Light red X-axis
    gizmos.line(origin, origin + Vec3::Y * 20.0, Color::srgb(0.5, 1.0, 0.5)); // Light green Y-axis
    gizmos.line(origin, origin + Vec3::Z * 5.0, Color::srgb(0.5, 0.5, 1.0)); // Light blue Z-axis
}

fn draw_contour_outline(gizmos: &mut Gizmos, contour: &Contour, color: Color, z_offset: f32) {
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

fn draw_polyline(gizmos: &mut Gizmos, polyline: &Polyline<f64>, color: Color, z_offset: f32) {
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

fn approximate_arc(start_vertex: cavalier_contours::polyline::PlineVertex<f64>, end_vertex: cavalier_contours::polyline::PlineVertex<f64>, segments: usize) -> Vec<(f64, f64)> {
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

/// Convert a Contour to a cavalier_contours Polyline (simplified approach)
fn contour_to_polyline(contour: &Contour) -> Result<Polyline<f64>, MeshTextError> {
    use cavalier_contours::polyline::{PlineVertex, PlineSourceMut};
    
    if contour.vertices.len() < 3 {
        return Err(MeshTextError::InvalidContour);
    }
    
    // Remove duplicate vertices (cavalier_contours doesn't handle them)
    let mut vertices = contour.vertices.clone();
    let tolerance = 1e-4;
    
    // Remove consecutive duplicates
    let mut i = 0;
    while i < vertices.len() - 1 {
        if vertices[i].distance(vertices[i + 1]) < tolerance {
            vertices.remove(i + 1);
        } else {
            i += 1;
        }
    }
    
    // For closed contours, check if first and last are duplicates
    if contour.is_closed && vertices.len() > 3 {
        let first = vertices[0];
        let last = vertices[vertices.len() - 1];
        if first.distance(last) < tolerance {
            vertices.pop();
        }
    }
    
    if vertices.len() < 3 {
        return Err(MeshTextError::InvalidContour);
    }
    
    let mut polyline = Polyline::new();
    
    // Add vertices to the polyline
    for vertex in &vertices {
        // Convert to f64 and add with bulge = 0.0 (straight lines)
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

 