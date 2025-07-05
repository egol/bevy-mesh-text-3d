use bevy::prelude::*;
use cosmic_text::{Attrs, Weight, Family, Align};

use bevy_mesh_text_3d::{InputText, MeshTextPlugin, Parameters, Settings, generate_meshes, BevelParameters};

#[cfg(feature = "debug")]
use bevy_mesh_text_3d::render::DebugRenderable;

const CAMERA_VIEWPORT_HEIGHT: f32 = 950.0;
const TEXT_SCALE_MULTIPLIER: f32 = 3.0;

fn main() {
    let mut app = App::new();
    
    // Enable debug features when the debug feature is enabled
    #[cfg(feature = "debug")]
    {
        app.add_plugins((
            DefaultPlugins,
            MeshTextPlugin::new(1.0),
        ));
    }
    
    #[cfg(not(feature = "debug"))]
    {
        app.add_plugins((
            DefaultPlugins,
            MeshTextPlugin::new(1.0),
        ));
    }
    
    app.add_systems(Startup, setup)
       .add_systems(Update, (update_bevel_parameters, camera_controls))
       .run();
}

#[derive(Component)]
struct BevelDemo {
    current_segments: u32,
    current_width: f32,
    current_power: f32,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut settings: ResMut<Settings>,
) {
    // Create a basic material
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.7, 0.6),
        metallic: 0.1,
        ..default()
    });

    // Create different bevel configurations to demonstrate the system
    let bevel_configs = vec![
        // No bevel (original extrusion)
        (None, "No Bevel", Vec3::new(0.0, 2.0, 0.0)),
        // Simple bevel
        (Some(BevelParameters {
            bevel_width: 0.2,
            bevel_segments: 1,
            profile_power: 1.0,
        }), "Simple Bevel", Vec3::new(0.0, 0.0, 0.0)),
        // Rounded bevel
        (Some(BevelParameters {
            bevel_width: 0.3,
            bevel_segments: 4,
            profile_power: 2.0,
        }), "Rounded Bevel", Vec3::new(0.0, -2.0, 0.0)),
        // Complex bevel
        (Some(BevelParameters {
            bevel_width: 0.15,
            bevel_segments: 6,
            profile_power: 1.5,
        }), "Complex Bevel", Vec3::new(0.0, -4.0, 0.0)),
    ];

    for (bevel_params, text, position) in bevel_configs {
        let input_text = InputText::Simple {
            text: text.to_string(),
            material: material.clone(),
            attrs: Attrs::new()
                .family(Family::SansSerif)
                .weight(Weight::BOLD),
        };

        let parameters = Parameters {
            font_size: 72.0,
            line_height: 72.0,
            extrusion_depth: 0.5,
            alignment: Some(Align::Center),
            max_width: None,
            max_height: None,
            bevel: bevel_params,
        };

        match generate_meshes(input_text, &mut settings, parameters, &mut meshes) {
            Ok(mesh_entries) => {
                let mut text_entity = commands.spawn(Transform::from_translation(position));

                #[cfg(feature = "debug")]
                {
                    // Add debug renderable component
                    text_entity.insert(DebugRenderable::default());
                }

                for entry in mesh_entries {
                    text_entity.with_children(|parent| {
                        parent.spawn((
                            Mesh3d(entry.mesh),
                            MeshMaterial3d(entry.material),
                            entry.transform,
                        ));
                    });
                }
            }
            Err(e) => error!("Failed to generate mesh: {:?}", e),
        }
    }

    // Add interactive demo
    commands.spawn(BevelDemo {
        current_segments: 1,
        current_width: 0.2,
        current_power: 1.0,
    });

    // Setup camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 8.0)
            .looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Add lighting
    commands.spawn((
        DirectionalLight {
            color: Color::WHITE,
            illuminance: 10000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,
            -std::f32::consts::FRAC_PI_4,
            0.0,
        )),
    ));

    // Add ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 500.0,
        affects_lightmapped_meshes: false,
    });

    #[cfg(feature = "debug")]
    {
        println!("=== BEVEL DEMO ===");
        println!("Controls:");
        println!("  D - Toggle debug wireframe");
        println!("  N - Toggle normal visualization");
        println!("  Arrow Keys - Adjust bevel parameters");
        println!("  WASD - Move camera");
        println!("  Mouse - Look around");
        println!("==================");
    }
}

fn update_bevel_parameters(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut bevel_demo: Query<&mut BevelDemo>,
    _commands: Commands,
    _meshes: ResMut<Assets<Mesh>>,
    _materials: ResMut<Assets<StandardMaterial>>,
    _settings: ResMut<Settings>,
) {
    if let Ok(mut demo) = bevel_demo.single_mut() {
        let mut changed = false;

        if keyboard_input.just_pressed(KeyCode::ArrowUp) {
            demo.current_segments = (demo.current_segments + 1).min(8);
            changed = true;
        }
        if keyboard_input.just_pressed(KeyCode::ArrowDown) {
            demo.current_segments = (demo.current_segments.saturating_sub(1)).max(1);
            changed = true;
        }
        if keyboard_input.just_pressed(KeyCode::ArrowLeft) {
            demo.current_width = (demo.current_width - 0.05).max(0.05);
            changed = true;
        }
        if keyboard_input.just_pressed(KeyCode::ArrowRight) {
            demo.current_width = (demo.current_width + 0.05).min(0.5);
            changed = true;
        }

        if changed {
            #[cfg(feature = "debug")]
            {
                println!("Bevel params: width={:.2}, segments={}, power={:.1}", 
                         demo.current_width, demo.current_segments, demo.current_power);
            }
            
            // This would normally recreate the interactive demo mesh
            // For now, just print the parameters
        }
    }
}

fn camera_controls(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
    time: Res<Time>,
) {
    if let Ok(mut transform) = camera_query.single_mut() {
        let mut movement = Vec3::ZERO;
        let speed = 5.0 * time.delta_secs();

        if keyboard_input.pressed(KeyCode::KeyW) {
            movement += *transform.forward();
        }
        if keyboard_input.pressed(KeyCode::KeyS) {
            movement += *transform.back();
        }
        if keyboard_input.pressed(KeyCode::KeyA) {
            movement += *transform.left();
        }
        if keyboard_input.pressed(KeyCode::KeyD) {
            movement += *transform.right();
        }

        transform.translation += movement * speed;
    }
} 