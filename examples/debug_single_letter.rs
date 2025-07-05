use bevy::{
    color::palettes::css::WHITE,
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::*,
    render::{
        RenderPlugin,
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
    },
};
use cosmic_text::Attrs;

use bevy_mesh_text_3d::{InputText, MeshTextPlugin, Parameters, Settings, generate_meshes, BevelParameters};

const CAMERA_VIEWPORT_HEIGHT: f32 = 950.0;
const TEXT_SCALE_MULTIPLIER: f32 = 10.0; // Much larger scale for debugging

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    features: WgpuFeatures::POLYGON_MODE_LINE,
                    ..default()
                }),
                ..default()
            }),
            WireframePlugin::default(),
        ))
        .insert_resource(WireframeConfig {
            global: true,
            default_color: WHITE.into(),
        })
        .add_plugins(MeshTextPlugin::new(
            (CAMERA_VIEWPORT_HEIGHT / 950.0) * TEXT_SCALE_MULTIPLIER,
        ))
        .add_systems(Update, keyboard_input)
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 1500.0,
            affects_lightmapped_meshes: false,
        })
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .add_systems(Startup, setup)
        .add_systems(Startup, spawn_debug_text)
        .run();
}

fn keyboard_input(keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::Space) {
        std::process::exit(0);
    }
}

fn setup(mut commands: Commands) {
    // Position camera to view a single large letter
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 200.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Add directional light for better visibility
    commands.spawn((
        DirectionalLight {
            color: Color::WHITE,
            illuminance: 15000.0,
            ..default()
        },
        Transform::from_xyz(5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn spawn_debug_text(
    mut commands: Commands,
    mut fonts: ResMut<Settings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Create a bright material for visibility
    let debug_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.2, 0.2),
        metallic: 0.1,
        cull_mode: None,
        ..default()
    });

    println!("=== DEBUGGING SINGLE LETTER ===");
    println!("Rendering single letter 'A' with large scale");

    // Generate a single letter with various bevel settings
    let bevel_configs = vec![
        (None, "No Bevel", 0.0),
        (Some(BevelParameters {
            bevel_width: 2.0,
            bevel_segments: 3,
            profile_power: 1.0,
        }), "With Bevel", 60.0),
    ];

    for (bevel_params, label, x_offset) in bevel_configs {
        if let Some(ref params) = bevel_params {
            println!("Creating '{}' with bevel_width={}, bevel_segments={}", label, params.bevel_width, params.bevel_segments);
        } else {
            println!("Creating '{}'", label);
        }
        
        let text_meshes = generate_meshes(
            InputText::Simple {
                text: "A".to_string(), // Single letter for debugging
                material: debug_material.clone(),
                attrs: Attrs::new(),
            },
            &mut fonts,
            Parameters {
                extrusion_depth: 10.0, // Large extrusion for visibility
                font_size: 48.0,       // Large font size
                line_height: 56.0,
                alignment: None,
                max_width: None,
                max_height: None,
                bevel: bevel_params,
            },
            &mut meshes,
        );

        match text_meshes {
            Ok(meshes) => {
                println!("Successfully generated {} meshes for '{}'", meshes.len(), label);
                for (i, mesh) in meshes.into_iter().enumerate() {
                    println!("  Mesh {}: transform = {:?}", i, mesh.transform);
                    commands.spawn((
                        Mesh3d(mesh.mesh),
                        MeshMaterial3d(mesh.material),
                        mesh.transform.with_translation(Vec3::new(
                            x_offset + mesh.transform.translation.x,
                            mesh.transform.translation.y,
                            mesh.transform.translation.z,
                        )),
                    ));
                }
            }
            Err(e) => {
                error!("Failed to generate meshes for '{}': {}", label, e);
            }
        }
    }
} 