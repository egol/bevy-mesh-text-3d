use bevy::prelude::*;
use cosmic_text::Attrs;
use bevy::gizmos::gizmos::Gizmos;
use std::collections::HashMap;

// Import the debug tessellation function directly
use bevy_mesh_text_3d::extrude_glyph::tessellate_beveled_glyph_with_gizmos;
use bevy_mesh_text_3d::{InputText, MeshTextPlugin, Parameters, Settings, BevelParameters};

const CAMERA_VIEWPORT_HEIGHT: f32 = 950.0;
const TEXT_SCALE_MULTIPLIER: f32 = 4.0; // Use proper scale like working examples

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(MeshTextPlugin::new(
            (CAMERA_VIEWPORT_HEIGHT / 950.0) * TEXT_SCALE_MULTIPLIER,
        ))
        .add_systems(Update, keyboard_input)
        .add_systems(Update, rotate_camera)
        .add_systems(Update, visualize_glyph_processing)
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 1500.0,
            affects_lightmapped_meshes: false,
        })
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .insert_resource(GlyphVisualizationData::default())
        .add_systems(Startup, setup)
        .add_systems(Startup, spawn_debug_text)
        .run();
}

fn keyboard_input(keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }
    // Comment out space key exit to keep window open
    // if keys.just_pressed(KeyCode::Space) {
    //     std::process::exit(0);
    // }
}

fn rotate_camera(mut query: Query<&mut Transform, With<Camera>>, time: Res<Time>) {
    for mut transform in &mut query {
        // Rotate around the Y axis to see the 3D structure
        transform.rotate_around(Vec3::ZERO, Quat::from_rotation_y(0.5 * time.delta_secs()));
    }
}

#[derive(Component)]
struct BevelVisualization {
    bevel_params: BevelParameters,
    extrusion_depth: f32,
}

#[derive(Component)]
struct GlyphData {
    glyph_id: u16,
    font_size: f32,
    extrusion_depth: f32,
    bevel_params: Option<BevelParameters>,
}

// Resource to store glyph processing data for visualization
#[derive(Resource, Default)]
struct GlyphVisualizationData {
    pending_glyphs: HashMap<u16, GlyphVisualizationEntry>,
}

#[derive(Clone)]
struct GlyphVisualizationEntry {
    font_size: f32,
    extrusion_depth: f32,
    bevel_params: Option<BevelParameters>,
}

fn setup(mut commands: Commands) {
    // Position camera closer to see the glyph details
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 80.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Add a directional light for better visibility
    commands.spawn((
        DirectionalLight {
            color: Color::WHITE,
            illuminance: 10000.0,
            ..default()
        },
        Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn spawn_debug_text(
    mut commands: Commands,
    mut fonts: ResMut<Settings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut viz_data: ResMut<GlyphVisualizationData>,
) {
    // Create different materials for comparison
    let no_bevel_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.4, 0.8),
        metallic: 0.1,
        cull_mode: None,
        ..default()
    });

    let bevel_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.2, 0.2),
        metallic: 0.1,
        cull_mode: None,
        ..default()
    });

    println!("=== DEBUGGING SINGLE LETTER (SOLID) WITH GIZMOS ===");
    println!("Rendering single letter 'A' with bevel process visualization");
    println!("Colors:");
    println!("  White  - Original glyph outline");
    println!("  Cyan   - Front cap tessellation");
    println!("  Yellow - Extracted contours");
    println!("  Red    - Outer bevel ring");
    println!("  Green  - Intermediate bevel rings");
    println!("  Blue   - Inner bevel ring");
    println!("  Magenta - Final mesh wireframe");

    // Generate a single letter with various bevel settings
    let bevel_configs = vec![
        (None, "No Bevel", no_bevel_material, -50.0),
        (Some(BevelParameters {
            bevel_width: 2.0,
            bevel_segments: 3,
            profile_power: 1.0,
        }), "With Bevel", bevel_material, 50.0),
    ];

    for (bevel_params, label, material, x_offset) in bevel_configs {
        if let Some(ref params) = bevel_params {
            println!("Creating '{}' with bevel_width={}, bevel_segments={}", label, params.bevel_width, params.bevel_segments);
        } else {
            println!("Creating '{}'", label);
        }
        
        let text_meshes = bevy_mesh_text_3d::generate_meshes(
            InputText::Simple {
                text: "A".to_string(), // Single letter for debugging
                material: material.clone(),
                attrs: Attrs::new(),
            },
            &mut fonts,
            Parameters {
                extrusion_depth: 4.0, // Use proper extrusion depth like working examples
                font_size: 24.0,      // Use proper font size like working examples
                line_height: 28.0,
                alignment: None,
                max_width: None,
                max_height: None,
                bevel: bevel_params.clone(),
            },
            &mut meshes,
        );

        match text_meshes {
            Ok(meshes) => {
                println!("Successfully generated {} meshes for '{}'", meshes.len(), label);
                for (i, mesh) in meshes.into_iter().enumerate() {
                    println!("  Mesh {}: transform = {:?}", i, mesh.transform);
                    // Position text at center, override the calculated positions
                    let mut entity = commands.spawn((
                        Mesh3d(mesh.mesh),
                        MeshMaterial3d(mesh.material),
                        Transform::from_translation(Vec3::new(x_offset, 0.0, 0.0)),
                    ));
                    
                    // Add glyph data for visualization
                    let glyph_id = 'A' as u16; // Using 'A' as the glyph ID for simplicity
                    entity.insert(GlyphData {
                        glyph_id,
                        font_size: 24.0,
                        extrusion_depth: 4.0,
                        bevel_params: bevel_params.clone(),
                    });
                    
                    // Store glyph data for visualization
                    viz_data.pending_glyphs.insert(glyph_id, GlyphVisualizationEntry {
                        font_size: 24.0,
                        extrusion_depth: 4.0,
                        bevel_params: bevel_params.clone(),
                    });
                }
            }
            Err(e) => {
                error!("Failed to generate meshes for '{}': {}", label, e);
            }
        }
    }
}

fn visualize_glyph_processing(
    mut gizmos: Gizmos,
    mut fonts: ResMut<Settings>,
    query: Query<(&GlyphData, &Transform)>,
    viz_data: Res<GlyphVisualizationData>,
) {
    // For each glyph entity, draw the actual glyph processing visualization
    for (glyph_data, transform) in query.iter() {
        // Transform gizmos to match entity position
        let translation = transform.translation;
        
        // Draw coordinate system at the entity position
        gizmos.line(
            translation,
            translation + Vec3::X * 10.0,
            Color::srgb(1.0, 0.0, 0.0), // Red X-axis
        );
        gizmos.line(
            translation,
            translation + Vec3::Y * 10.0,
            Color::srgb(0.0, 1.0, 0.0), // Green Y-axis
        );
        gizmos.line(
            translation,
            translation + Vec3::Z * 10.0,
            Color::srgb(0.0, 0.0, 1.0), // Blue Z-axis
        );
        
        // Try to access the real glyph processing if we have bevel parameters
        if let Some(bevel_params) = &glyph_data.bevel_params {
            println!("Visualizing glyph {} with bevel - segments: {}, width: {}, depth: {}", 
                     glyph_data.glyph_id, bevel_params.bevel_segments, bevel_params.bevel_width, glyph_data.extrusion_depth);
            
            // For now, we'll use the tessellation function to generate the glyph processing
            // and the debug features will automatically draw the gizmos
            // This will be called on the first glyph in the text that has beveling
            
            // Use the buffer from the font system to get the actual glyph
            let metrics = cosmic_text::Metrics {
                font_size: glyph_data.font_size,
                line_height: glyph_data.font_size * 1.2,
            };
            
            let mut buffer = cosmic_text::Buffer::new_empty(metrics);
            let attrs = Attrs::new();
            buffer.set_rich_text(
                &mut fonts.font_system,
                [("A", attrs.clone())],
                &attrs,
                cosmic_text::Shaping::Advanced,
                None,
            );
            
            // Shape the text to get the actual glyph
            buffer.shape_until_scroll(&mut fonts.font_system, false);
            
            // Get the first glyph from the buffer
            if let Some(run) = buffer.layout_runs().next() {
                if let Some(glyph) = run.glyphs.first() {
                    // Now we can call the tessellation function with gizmos
                    // The gizmos will be automatically drawn by the debug features
                    let result = tessellate_beveled_glyph_with_gizmos(
                        glyph,
                        &mut fonts.font_system,
                        glyph_data.extrusion_depth,
                        bevel_params,
                        Some(&mut gizmos),
                    );
                    
                    match result {
                        Ok((geometry, center_x, center_y)) => {
                            println!("Successfully visualized real glyph {} with {} vertices, center: ({}, {})", 
                                     glyph_data.glyph_id, geometry.vertices.len(), center_x, center_y);
                        }
                        Err(e) => {
                            println!("Failed to visualize real glyph {}: {:?}", glyph_data.glyph_id, e);
                        }
                    }
                } else {
                    println!("No glyph found in buffer for visualization");
                }
            } else {
                println!("No layout runs found in buffer for visualization");
            }
        } else {
            println!("Glyph {} has no bevel parameters for visualization", glyph_data.glyph_id);
        }
    }
} 