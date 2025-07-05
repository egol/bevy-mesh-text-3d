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

use bevy_mesh_text_3d::{generate_meshes, BevelParameters, InputText, MeshTextPlugin, Parameters, Settings};

const CAMERA_VIEWPORT_HEIGHT: f32 = 950.0;
// This factor controls the overall size of text in the world
// Adjust this to make your text appear at the desired size
const TEXT_SCALE_MULTIPLIER: f32 = 4.0;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    // WARN this is a native only feature. It will not work with webgl or webgpu
                    features: WgpuFeatures::POLYGON_MODE_LINE,
                    ..default()
                }),
                ..default()
            }),
            // You need to add this plugin to enable wireframe rendering
            WireframePlugin::default(),
        ))
        .insert_resource(WireframeConfig {
            // The global wireframe config enables drawing of wireframes on every mesh,
            // except those with `NoWireframe`. Meshes with `Wireframe` will always have a wireframe,
            // regardless of the global configuration.
            global: false,
            // Controls the default color of all wireframes. Used as the default color for global wireframes.
            // Can be changed per mesh using the `WireframeColor` component.
            default_color: WHITE.into(),
        })
        .add_plugins(MeshTextPlugin::new(
            (CAMERA_VIEWPORT_HEIGHT / 950.0) * TEXT_SCALE_MULTIPLIER,
        ))
        .add_systems(Update, keyboard_input)
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 800.0,
            affects_lightmapped_meshes: false,
        })
        .add_systems(Startup, setup)
        .add_systems(Startup, spawn_text)
        .add_systems(Update, rotate_text) // Add the rotation system
        .run();
}

fn keyboard_input(keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::Space) {
        std::process::exit(0);
    }
}

/// Component to mark text that should rotate
#[derive(Component)]
pub struct RotatingText {
    pub speed: f32,
}

impl Default for RotatingText {
    fn default() -> Self {
        Self { speed: 0.5 }
    }
}

fn rotate_text(mut query: Query<(&mut Transform, &RotatingText)>, time: Res<Time>) {
    for (mut transform, rotating) in &mut query {
        // Rotate around the Y axis
        transform.rotate_y(rotating.speed * time.delta_secs());
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 100.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn spawn_text(
    mut commands: Commands,
    mut fonts: ResMut<Settings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let meshes = generate_meshes(
        InputText::Simple {
            text: "Hello, World!".to_string(),
            material: materials.add(StandardMaterial {
                base_color: Color::WHITE,
                cull_mode: None,
                ..default()
            }),
            attrs: Attrs::new(),
        },
        &mut fonts,
        Parameters {
            extrusion_depth: 2.5,
            font_size: 72.0,
            line_height: 80.0,
            alignment: None,
            max_width: None,
            max_height: None,
            bevel: Some(BevelParameters {
                bevel_width: 0.7,
                bevel_segments: 3,
                profile_power: 1.0,
            }),
        },
        &mut meshes,
    )
    .unwrap();

    for mesh in meshes {
        commands.spawn((
            Mesh3d(mesh.mesh),
            MeshMaterial3d(mesh.material),
            mesh.transform.with_translation(Vec3::new(
                mesh.transform.translation.x,
                mesh.transform.translation.y,
                mesh.transform.translation.z,
            )),
            RotatingText::default(),
        ));
    }
}
