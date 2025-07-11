use bevy::prelude::*;
use cosmic_text::{
    Align, Attrs, Buffer, FontSystem, Metrics, Shaping,
};

use bevy_mesh_text_3d::{
    glyph::{extract_glyph_outline, GlyphOutline},
    offset::{extract_contours, compute_bevel_rings},
    mesh::build_mesh_from_bevel_rings,
    BevelParameters,
};

fn main() {
    println!("=== MESH FROM BEVEL RINGS EXAMPLE ===");
    println!("Demonstrates converting bevel rings directly into a mesh");
    println!("=====================================");
    
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, create_mesh_from_bevel_rings)
        .add_systems(Update, (keyboard_input, rotate_camera))
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .run();
}

fn keyboard_input(keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::Escape) {
        println!("Exiting...");
        std::process::exit(0);
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
            const ORBIT_RADIUS: f32 = 80.0;
            const ORBIT_SPEED: f32 = 0.5;
            
            for mut transform in camera_query.iter_mut() {
                let elapsed = time.elapsed_secs() * ORBIT_SPEED;
                let x = elapsed.cos() * ORBIT_RADIUS;
                let z = elapsed.sin() * ORBIT_RADIUS;
                let y = 30.0;
                
                transform.translation = Vec3::new(x, y, z);
                transform.look_at(Vec3::ZERO, Vec3::Y);
            }
        }
    }
}

fn setup(mut commands: Commands) {
    // Setup camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(30.0, 30.0, 80.0)
            .looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Add light
    commands.spawn((
        DirectionalLight {
            color: Color::WHITE,
            illuminance: 8000.0,
            ..default()
        },
        Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Add ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 0.3,
        affects_lightmapped_meshes: false,
    });

    println!("\n=== CONTROLS ===");
    println!("ESC - Exit");
    println!("Space - Pause/Resume camera rotation");
    println!("================");
}

fn create_mesh_from_bevel_rings(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    static mut MESH_CREATED: bool = false;
    
    unsafe {
        if MESH_CREATED {
            return;
        }
        MESH_CREATED = true;
    }
    
    println!("\n=== CREATING MESH FROM BEVEL RINGS ===");
    
    // 1. Extract glyph outline for letter "B"
    let mut font_system = FontSystem::new();
    let metrics = Metrics::new(80.0, 80.0);
    let mut buffer = Buffer::new_empty(metrics);
    let attrs = Attrs::new();
    
    buffer.set_rich_text(
        &mut font_system,
        [("B", attrs.clone())],
        &attrs,
        Shaping::Advanced,
        Some(Align::Center),
    );
    
    buffer.set_size(&mut font_system, Some(200.0), Some(200.0));
    buffer.shape_until_scroll(&mut font_system, false);
    
    // 2. Extract glyph information
    let mut glyph_outline: Option<GlyphOutline> = None;
    
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            if glyph.glyph_id != 0 {
                println!("Found glyph: ID={}, font_size={}", glyph.glyph_id, glyph.font_size);
                
                match extract_glyph_outline(glyph, &mut font_system) {
                    Ok(outline) => {
                        glyph_outline = Some(outline);
                        break;
                    }
                    Err(e) => {
                        println!("Failed to extract glyph outline: {:?}", e);
                    }
                }
            }
        }
        if glyph_outline.is_some() {
            break;
        }
    }
    
    let Some(outline) = glyph_outline else {
        println!("❌ No glyph outline found");
        return;
    };
    
    // 3. Extract contours
    let scale_factor = outline.font_size / outline.units_per_em as f32;
    let glyph_width = (outline.bounding_box.x_max - outline.bounding_box.x_min) as f32 * scale_factor;
    let glyph_height = (outline.bounding_box.y_max - outline.bounding_box.y_min) as f32 * scale_factor;
    let center_x = glyph_width / 2.0;
    let center_y = glyph_height / 2.0;
    
    let contours = extract_contours(&outline.path, scale_factor, center_x, center_y);
    println!("Extracted {} contours from glyph", contours.len());
    
    // 4. Compute bevel rings
    let bevel_params = BevelParameters {
        bevel_width: 2.0,
        bevel_segments: 4,
        profile_power: 1.5,
    };
    
    let bevel_rings = match compute_bevel_rings(
        &contours,
        bevel_params.bevel_width,
        bevel_params.bevel_segments as usize,
        bevel_params.profile_power,
        outline.glyph_id.into(),
    ) {
        Ok(rings) => {
            println!("✅ Generated {} bevel ring sets", rings.len());
            rings
        }
        Err(e) => {
            println!("❌ Failed to compute bevel rings: {}", e);
            return;
        }
    };
    
    // 5. Build mesh from bevel rings
    let extrusion_depth = 8.0;
    let beveled_geometry = match build_mesh_from_bevel_rings(
        &bevel_rings,
        extrusion_depth,
        outline.glyph_id,
    ) {
        Ok(geometry) => {
            println!("✅ Generated mesh with {} vertices, {} triangles", 
                     geometry.vertices.len(), geometry.indices.len() / 3);
            geometry
        }
        Err(e) => {
            println!("❌ Failed to build mesh: {}", e);
            return;
        }
    };
    
    // 6. Convert to Bevy mesh and spawn
    let mesh: Mesh = beveled_geometry.into();
    let mesh_handle = meshes.add(mesh);
    
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.6, 0.9),
        metallic: 0.1,
        perceptual_roughness: 0.3,
        ..default()
    });
    
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    
    println!("✅ MESH CREATED AND SPAWNED SUCCESSFULLY!");
    println!("The 3D letter 'B' with bevel is now displayed!");
} 