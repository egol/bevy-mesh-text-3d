use bevy::prelude::*;
use cosmic_text::{
    Align, Attrs, Buffer, FontSystem, Metrics, Shaping,
};

use bevy_mesh_text_3d::{
    glyph::{extract_glyph_outline, GlyphOutline},
    offset::{extract_contours, Contour, compute_bevel_rings, BevelRings, draw_contour_outline},
    mesh::build_mesh_from_bevel_rings,
    BevelParameters,
};

#[derive(Resource)]
struct BevelVisualizationResults {
    original_contours: Vec<Contour>,
    bevel_rings: Vec<BevelRings>,
    bevel_params: BevelParameters,
    computed: bool,
    mesh_generated: bool,
}

#[derive(Component)]
struct BevelMesh;

fn main() {
    println!("=== BEVEL VISUALIZATION EXAMPLE ===");
    println!("Visualizing individual bevel rings like the offset test");
    println!("Shows the progressive inward offset rings that create the bevel");
    println!("Now also generates actual meshes from the bevel rings!");
    println!("=====================================");
    
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, compute_bevel_visualization)
        .add_systems(Update, generate_mesh_from_bevel_rings)
        .add_systems(Update, draw_bevel_visualization)
        .add_systems(Update, (keyboard_input, rotate_camera))
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
        .insert_resource(BevelVisualizationResults {
            original_contours: Vec::new(),
            bevel_rings: Vec::new(),
            bevel_params: BevelParameters::default(),
            computed: false,
            mesh_generated: false,
        })
        .run();
}

fn keyboard_input(keys: Res<ButtonInput<KeyCode>>, mut viz_results: ResMut<BevelVisualizationResults>) {
    if keys.just_pressed(KeyCode::Escape) {
        println!("Exiting bevel visualization...");
        std::process::exit(0);
    }
    
    if keys.just_pressed(KeyCode::KeyM) {
        viz_results.mesh_generated = false; // Reset to regenerate mesh
        println!("Mesh regeneration requested");
    }
    
    if keys.just_pressed(KeyCode::KeyG) {
        println!("Gizmo visualization toggle (always on in this example)");
    }
}

fn rotate_camera(
    time: Res<Time>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    static mut PAUSED: bool = false;
    
    if keyboard_input.just_pressed(KeyCode::Space) {
        unsafe {
            PAUSED = !PAUSED;
            println!("Camera rotation {}", if PAUSED { "paused" } else { "resumed" });
        }
    }
    
    unsafe {
        if !PAUSED {
            const ORBIT_RADIUS: f32 = 120.0;
            const ORBIT_SPEED: f32 = 0.3;
            
            for mut transform in camera_query.iter_mut() {
                let elapsed = time.elapsed_secs() * ORBIT_SPEED;
                let x = elapsed.cos() * ORBIT_RADIUS;
                let z = elapsed.sin() * ORBIT_RADIUS;
                let y = 50.0; // Keep some height
                
                transform.translation = Vec3::new(x, y, z);
                transform.look_at(Vec3::ZERO, Vec3::Y);
            }
        }
    }
}

fn setup(mut commands: Commands) {
    // Setup camera to look at origin (3D view with slight angle)
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(50.0, 50.0, 120.0)
            .looking_at(Vec3::ZERO, Vec3::Y),
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

    println!("\n=== CONTROLS ===");
    println!("ESC - Exit");
    println!("Space - Pause/Resume camera rotation");
    println!("M - Regenerate mesh");
    println!("G - Toggle gizmo visualization (always on)");
    println!("Camera automatically orbits around the letter");
    println!("================");
}

fn compute_bevel_visualization(mut viz_results: ResMut<BevelVisualizationResults>) {
    if viz_results.computed {
        return; // Already computed
    }
    
    println!("\n=== COMPUTING BEVEL VISUALIZATION (Letter A) ===");
    
    // Create a font system
    let mut font_system = FontSystem::new();
    
    // Create a simple buffer with the letter "A"
    let metrics = Metrics::new(72.0, 72.0);
    let mut buffer = Buffer::new_empty(metrics);
    let attrs = Attrs::new();
    
    buffer.set_rich_text(
        &mut font_system,
        [("B", attrs.clone())],
        &attrs,
        Shaping::Advanced,
        Some(Align::Center),
    );
    
    // Set buffer size and shape
    buffer.set_size(&mut font_system, Some(200.0), Some(200.0));
    buffer.shape_until_scroll(&mut font_system, false);
    
    // Extract glyph information
    let mut glyph_found = false;
    let mut glyph_outline: Option<GlyphOutline> = None;
    
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            if glyph.glyph_id != 0 {
                println!("Found glyph: ID={}, font_size={}", glyph.glyph_id, glyph.font_size);
                
                match extract_glyph_outline(glyph, &mut font_system) {
                    Ok(outline) => {
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
        println!("❌ No glyph outline found");
        return;
    };
    
    // Extract contours with proper scaling
    let scale_factor = outline.font_size / outline.units_per_em as f32;
    let glyph_width = (outline.bounding_box.x_max - outline.bounding_box.x_min) as f32 * scale_factor;
    let glyph_height = (outline.bounding_box.y_max - outline.bounding_box.y_min) as f32 * scale_factor;
    let center_x = glyph_width / 2.0;
    let center_y = glyph_height / 2.0;
    
    let contours = extract_contours(&outline.path, scale_factor, center_x, center_y);
    println!("Extracted {} contours from glyph", contours.len());
    
    if contours.is_empty() {
        println!("❌ No contours extracted");
        return;
    }
    
    // Test different bevel configurations
    let bevel_configs = vec![
        BevelParameters {
            bevel_width: 1.5,
            bevel_segments: 1,
            profile_power: 1.0,
        },
    ];
    
    for bevel_params in bevel_configs {
        println!("\n=== Testing Bevel: width={}, segments={}, power={} ===", 
                 bevel_params.bevel_width, bevel_params.bevel_segments, bevel_params.profile_power);
        
        // Compute bevel rings
        match compute_bevel_rings(
            &contours,
            bevel_params.bevel_width,
            bevel_params.bevel_segments as usize,
            bevel_params.profile_power,
            outline.glyph_id.into(),
        ) {
            Ok(bevel_rings) => {
                println!("✅ Generated {} bevel ring sets", bevel_rings.len());
                
                // Store results for visualization
                viz_results.original_contours = contours.clone();
                viz_results.bevel_rings = bevel_rings;
                viz_results.bevel_params = bevel_params;
                viz_results.computed = true;
                
                println!("✅ BEVEL VISUALIZATION COMPUTED SUCCESSFULLY!");
                break;
            }
            Err(e) => {
                println!("❌ Failed to compute bevel rings: {}", e);
            }
        }
    }
}

fn generate_mesh_from_bevel_rings(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut viz_results: ResMut<BevelVisualizationResults>,
    existing_meshes: Query<Entity, With<BevelMesh>>,
) {
    if !viz_results.computed || viz_results.mesh_generated {
        return;
    }
    
    println!("\n=== GENERATING MESH FROM BEVEL RINGS ===");
    
    // Remove existing mesh entities
    for entity in existing_meshes.iter() {
        commands.entity(entity).despawn();
    }
    
    // Generate mesh from bevel rings
    match build_mesh_from_bevel_rings(
        &viz_results.bevel_rings,
        10.0, // extrusion_depth
        0, // glyph_id
    ) {
        Ok(beveled_geometry) => {
            println!("✅ Generated mesh with {} vertices, {} triangles", 
                     beveled_geometry.vertices.len(), beveled_geometry.indices.len() / 3);
            
            // Convert to Bevy mesh
            let mesh: Mesh = beveled_geometry.into();
            let mesh_handle = meshes.add(mesh);
            
            // Create a material for the mesh
            let material = materials.add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.7, 0.6),
                metallic: 0.0,
                perceptual_roughness: 0.8,
                ..default()
            });
            
            // Spawn the mesh entity
            commands.spawn((
                Mesh3d(mesh_handle),
                MeshMaterial3d(material),
                Transform::from_xyz(-50.0, 0.0, 0.0), // Offset to the side
                BevelMesh,
            ));
            
            viz_results.mesh_generated = true;
            println!("✅ MESH GENERATED AND SPAWNED SUCCESSFULLY!");
        }
        Err(e) => {
            println!("❌ Failed to generate mesh: {}", e);
        }
    }
}

fn draw_bevel_visualization(mut gizmos: Gizmos, viz_results: Res<BevelVisualizationResults>) {
    if !viz_results.computed {
        return; // Nothing to draw yet
    }
    
    // Draw original contours in gray (baseline)
    for contour in &viz_results.original_contours {
        let color = Color::srgb(0.5, 0.5, 0.5);
        draw_contour_outline(&mut gizmos, contour, color, 0.0);
    }
    
    // Color palette for bevel rings
    let colors = [
        Color::srgb(1.0, 0.0, 0.0), // Red
        Color::srgb(0.0, 1.0, 0.0), // Green  
        Color::srgb(0.0, 0.0, 1.0), // Blue
        Color::srgb(1.0, 1.0, 0.0), // Yellow
        Color::srgb(1.0, 0.0, 1.0), // Magenta
        Color::srgb(0.0, 1.0, 1.0), // Cyan
        Color::srgb(1.0, 0.5, 0.0), // Orange
        Color::srgb(0.5, 0.0, 1.0), // Purple
        Color::srgb(0.5, 1.0, 0.5), // Light green
        Color::srgb(1.0, 0.5, 0.5), // Light red
    ];
    
    // Draw each bevel ring set
    for (_ring_set_idx, bevel_ring) in viz_results.bevel_rings.iter().enumerate() {
        
        // Collect all rings in order: outer -> intermediates -> inner
        let mut all_rings = vec![&bevel_ring.outer_contour];
        all_rings.extend(bevel_ring.rings.iter());
        all_rings.push(&bevel_ring.inner_contour);
        
        // Draw each ring with different colors and Z offsets
        for (ring_idx, ring) in all_rings.iter().enumerate() {
            let color = colors[ring_idx % colors.len()];
            let z_offset = ring_idx as f32 * 2.0; // Progressive Z offset for depth
            
            draw_contour_outline(&mut gizmos, ring, color, z_offset);
            
            // Also draw as filled outline for better visualization
            if ring.vertices.len() >= 3 {
                for i in 0..ring.vertices.len() {
                    let current = ring.vertices[i];
                    let next = ring.vertices[(i + 1) % ring.vertices.len()];
                    
                    let start = Vec3::new(current.x, current.y, z_offset);
                    let end = Vec3::new(next.x, next.y, z_offset);
                    
                    gizmos.line(start, end, color);
                }
            }
        }
    }
    
    // Draw coordinate system for reference
    let origin = Vec3::ZERO;
    gizmos.line(origin, origin + Vec3::X * 30.0, Color::srgb(1.0, 0.3, 0.3)); // Red X-axis
    gizmos.line(origin, origin + Vec3::Y * 30.0, Color::srgb(0.3, 1.0, 0.3)); // Green Y-axis
    gizmos.line(origin, origin + Vec3::Z * 20.0, Color::srgb(0.3, 0.3, 1.0)); // Blue Z-axis
    
    // Draw labels
    let label_offset = Vec3::new(35.0, 0.0, 0.0);
    gizmos.line(origin + label_offset, origin + label_offset + Vec3::X * 5.0, Color::WHITE);
    
    // Draw separation line between gizmo and mesh visualization
    let sep_x = -25.0;
    gizmos.line(
        Vec3::new(sep_x, -30.0, 0.0),
        Vec3::new(sep_x, 30.0, 0.0),
        Color::srgb(0.8, 0.8, 0.8),
    );
} 