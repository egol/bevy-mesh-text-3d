use bevy::prelude::*;
use cavalier_contours::{
    pline_closed,
    polyline::{PlineSource, Polyline},
    shape_algorithms::{Shape, ShapeOffsetOptions},
};

#[derive(Resource)]
struct TestResults {
    original_shape: Option<Shape<f64>>,
    offset_shapes: Vec<Shape<f64>>,
    offset_loops: Vec<Polyline<f64>>,
    computed: bool,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, test_shape_offset_system)
        .add_systems(Update, draw_2d_visualization)
        .add_systems(Update, keyboard_input)
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
        .insert_resource(TestResults {
            original_shape: None,
            offset_shapes: Vec::new(),
            offset_loops: Vec::new(),
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
        Transform::from_xyz(100.0, 0.0, 500.0).looking_at(Vec3::ZERO, Vec3::Y),
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

fn test_shape_offset_system(mut test_results: ResMut<TestResults>) {
    if test_results.computed {
        return; // Test already completed
    }
    
    println!("=== TESTING SHAPE OFFSET SYSTEM (Direct Copy of Official Example) ===");
    
    // Create the exact same test polylines as the official example
    let plines = vec![
        // Main outer shape with bulges
        pline_closed![
            (100.0, 100.0, -0.5),
            (80.0, 90.0, 0.374794619217547),
            (210.0, 0.0, 0.0),
            (230.0, 0.0, 1.0),
            (320.0, 0.0, -0.5),
            (280.0, 0.0, 0.5),
            (390.0, 210.0, 0.0),
            (280.0, 120.0, 0.5),
        ],
        // Inner shape creating a hole
        pline_closed![
            (150.0, 50.0, 0.0),
            (150.0, 100.0, 0.0),
            (223.74732137849435, 142.16931273980475, 0.0),
            (199.491310072685, 52.51543504258919, 0.5),
        ],
        // Small inner circle
        pline_closed![
            (261.11232783167395, 35.79686193615828, -1.0),
            (250.0, 100.0, -1.0),
        ],
        // Another small shape
        pline_closed![
            (320.5065990423979, 76.14222955572362, -1.0),
            (320.2986109239592, 103.52378781211337, 0.0),
        ],
        // Complex shape with bulge
        pline_closed![
            (273.6131273938006, -13.968608715397636, -0.3),
            (256.61336060995995, -25.49387433156079, 0.0),
            (249.69820124026208, 27.234215862385582, 0.0),
        ],
    ];

    println!("Created {} polylines:", plines.len());
    for (i, pline) in plines.iter().enumerate() {
        println!("  Polyline {}: {} vertices, closed: {}", 
                 i, pline.vertex_count(), pline.is_closed());
    }

    // Create shape from polylines (exactly like official example)
    let shape = Shape::from_plines(plines.iter().cloned());
    println!("Created shape with {} CCW plines, {} CW plines", 
             shape.ccw_plines.len(), shape.cw_plines.len());

    // Test offset functionality
    let offset = 2.0;
    let max_offset_count = 25;
    let options = ShapeOffsetOptions::default();
    
    println!("\nTesting offset with distance: {}", offset);
    
    // Generate multiple offset shapes (like the official example)
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
    
    println!("Generated {} offset shapes", offset_shapes.len());

    // Extract individual polylines from offset shapes for additional visualization
    let mut offset_loops: Vec<Polyline<f64>> = Vec::new();
    for offset_shape in &offset_shapes {
        for pline in &offset_shape.ccw_plines {
            offset_loops.push(pline.polyline.clone());
        }
        for pline in &offset_shape.cw_plines {
            offset_loops.push(pline.polyline.clone());
        }
    }
    
    println!("Total offset polylines for visualization: {}", offset_loops.len());

    // Store results for visualization
    test_results.original_shape = Some(shape);
    test_results.offset_shapes = offset_shapes;
    test_results.offset_loops = offset_loops;
    test_results.computed = true;
    
    println!("\n✅ SHAPE OFFSET TEST COMPLETED SUCCESSFULLY!");
    println!("Press ESC to exit, view the 2D visualization to see offset results");
}

fn draw_2d_visualization(mut gizmos: Gizmos, test_results: Res<TestResults>) {
    if !test_results.computed {
        return; // Nothing to draw yet
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
    ];
    
    for (i, offset_shape) in test_results.offset_shapes.iter().enumerate() {
        let color = colors[i % colors.len()];
        let z_offset = 0.1 + (i as f32 * 0.05);
        
        for pline in &offset_shape.ccw_plines {
            draw_polyline(&mut gizmos, &pline.polyline, color, z_offset);
        }
        for pline in &offset_shape.cw_plines {
            draw_polyline(&mut gizmos, &pline.polyline, color, z_offset);
        }
    }
    
    // Draw offset loops in cyan (slightly transparent)
    for (i, offset_loop) in test_results.offset_loops.iter().enumerate() {
        let z_offset = 0.2 + (i as f32 * 0.01);
        draw_polyline(&mut gizmos, offset_loop, Color::srgb(0.0, 1.0, 1.0), z_offset);
    }
    
    // Draw coordinate system for reference
    let origin = Vec3::ZERO;
    gizmos.line(origin, origin + Vec3::X * 50.0, Color::srgb(1.0, 0.5, 0.5)); // Light red X-axis
    gizmos.line(origin, origin + Vec3::Y * 50.0, Color::srgb(0.5, 1.0, 0.5)); // Light green Y-axis
    gizmos.line(origin, origin + Vec3::Z * 10.0, Color::srgb(0.5, 0.5, 1.0)); // Light blue Z-axis
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