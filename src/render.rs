use bevy::prelude::*;

#[cfg(feature = "debug")]
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, DiagnosticsStore};

/// Debug rendering configuration
#[derive(Resource, Default)]
pub struct DebugRenderConfig {
    pub wireframe_enabled: bool,
    pub show_normals: bool,
    pub normal_length: f32,
}

impl DebugRenderConfig {
    pub fn new() -> Self {
        Self {
            wireframe_enabled: false,
            show_normals: false,
            normal_length: 0.05,
        }
    }
}

/// Plugin for debug rendering features
pub struct DebugRenderPlugin;

impl Plugin for DebugRenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DebugRenderConfig::new());
        
        #[cfg(feature = "debug")]
        {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default())
               .add_systems(Update, (debug_input_system, debug_display_system));
        }
    }
}

/// Handle debug input for toggling wireframe and other debug features
#[cfg(feature = "debug")]
fn debug_input_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut debug_config: ResMut<DebugRenderConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyD) {
        debug_config.wireframe_enabled = !debug_config.wireframe_enabled;
        
        println!("Debug wireframe: {}", debug_config.wireframe_enabled);
        
        // Toggle wireframe on all materials
        for material_component in query.iter() {
            if let Some(material) = materials.get_mut(&material_component.0) {
                material.cull_mode = if debug_config.wireframe_enabled {
                    None // Show both sides in wireframe
                } else {
                    Some(bevy::render::render_resource::Face::Back)
                };
            }
        }
    }
    
    if keyboard_input.just_pressed(KeyCode::KeyN) {
        debug_config.show_normals = !debug_config.show_normals;
        println!("Debug normals: {}", debug_config.show_normals);
    }
}

/// Display debug information
#[cfg(feature = "debug")]
fn debug_display_system(
    diagnostics: Res<DiagnosticsStore>,
    debug_config: Res<DebugRenderConfig>,
    mesh_query: Query<&Mesh3d>,
    meshes: Res<Assets<Mesh>>,
) {
    if debug_config.wireframe_enabled || debug_config.show_normals {
        // Print FPS
        if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(average) = fps.average() {
                println!("FPS: {:.1}", average);
            }
        }
        
        // Count meshes
        let mesh_count = mesh_query.iter().count();
        let mut total_vertices = 0;
        let mut total_triangles = 0;
        
        for mesh_component in mesh_query.iter() {
            if let Some(mesh) = meshes.get(&mesh_component.0) {
                if let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                    total_vertices += positions.len();
                }
                if let Some(indices) = mesh.indices() {
                    total_triangles += indices.len() / 3;
                }
            }
        }
        
        println!("Meshes: {}, Vertices: {}, Triangles: {}", 
                 mesh_count, total_vertices, total_triangles);
    }
}

/// Create a debug wireframe material
#[cfg(feature = "debug")]
pub fn create_debug_wireframe_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(0.0, 1.0, 0.0), // Green wireframe
        unlit: true,
        cull_mode: None,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }
}

/// Create a debug normal material
#[cfg(feature = "debug")]
pub fn create_debug_normal_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(1.0, 0.0, 0.0), // Red normals
        unlit: true,
        ..default()
    }
}

/// Generate normal visualization lines
#[cfg(feature = "debug")]
pub fn generate_normal_lines(
    vertices: &[Vec3],
    normals: &[Vec3],
    normal_length: f32,
) -> (Vec<Vec3>, Vec<u32>) {
    let mut line_vertices = Vec::new();
    let mut line_indices = Vec::new();
    
    for (i, (vertex, normal)) in vertices.iter().zip(normals.iter()).enumerate() {
        let start = *vertex;
        let end = *vertex + *normal * normal_length;
        
        line_vertices.push(start);
        line_vertices.push(end);
        
        let base_idx = (i * 2) as u32;
        line_indices.push(base_idx);
        line_indices.push(base_idx + 1);
    }
    
    (line_vertices, line_indices)
}

/// Create a mesh for visualizing normals as lines
#[cfg(feature = "debug")]
pub fn create_normal_visualization_mesh(
    vertices: &[Vec3],
    normals: &[Vec3],
    normal_length: f32,
) -> Mesh {
    let (line_vertices, line_indices) = generate_normal_lines(vertices, normals, normal_length);
    
    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::LineList,
        bevy::asset::RenderAssetUsages::RENDER_WORLD | bevy::asset::RenderAssetUsages::MAIN_WORLD,
    );
    
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, line_vertices);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(line_indices));
    
    mesh
}

/// System to spawn normal visualization entities
#[cfg(feature = "debug")]
pub fn spawn_normal_visualization(
    commands: &mut Commands,
    vertices: &[Vec3],
    normals: &[Vec3],
    normal_length: f32,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    transform: Transform,
) -> Entity {
    let normal_mesh = create_normal_visualization_mesh(vertices, normals, normal_length);
    let mesh_handle = meshes.add(normal_mesh);
    let material_handle = materials.add(create_debug_normal_material());
    
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material_handle),
        transform,
    )).id()
}

/// Debug component to mark entities for debug rendering
#[derive(Component)]
pub struct DebugRenderable {
    pub show_wireframe: bool,
    pub show_normals: bool,
}

impl Default for DebugRenderable {
    fn default() -> Self {
        Self {
            show_wireframe: false,
            show_normals: false,
        }
    }
}

/// System to update debug rendering for marked entities
#[cfg(feature = "debug")]
pub fn update_debug_rendering(
    debug_config: Res<DebugRenderConfig>,
    mut query: Query<(&mut DebugRenderable, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (mut debug_renderable, material_component) in query.iter_mut() {
        if let Some(material) = materials.get_mut(&material_component.0) {
            // Update wireframe mode
            if debug_config.wireframe_enabled != debug_renderable.show_wireframe {
                debug_renderable.show_wireframe = debug_config.wireframe_enabled;
                material.cull_mode = if debug_config.wireframe_enabled {
                    None
                } else {
                    Some(bevy::render::render_resource::Face::Back)
                };
            }
            
            // Update normal visualization
            if debug_config.show_normals != debug_renderable.show_normals {
                debug_renderable.show_normals = debug_config.show_normals;
                // Note: Normal visualization requires spawning separate entities
                // This would be handled by the text rendering system
            }
        }
    }
} 