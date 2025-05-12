# Bevy Mesh Text 3D

Generate extruded 3d text meshes for Bevy.

![media/animation.gif](media/animation.gif)

## Usage

For detailed usage, please check out the [simple](examples/simple.rs) or [complex](examples/complex.rs) example.

### Include the plugin

``` rs
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(MeshTextPlugin::new(
            2.0 // text scale factor
        ))
        ...
        .run();
}
```

### Have a `setup` to instantiate meshes

``` rs
fn spawn_text(
    mut commands: Commands,
    mut fonts: ResMut<Fonts>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Convert text into meshes
    let meshes = generate_meshes(
        InputText::Simple {
            text: "Hello, World!".to_string(),
            material: materials.add(StandardMaterial {
                base_color: Color::WHITE,
                // Cull Mode `None` is important
                cull_mode: None,
                ..default()
            }),
            attrs: Attrs::new(),
        },
        &mut fonts,
        Parameters {
            extrusion_depth: 2.5,
            font_size: 14.0,
            line_height: 16.0,
            alignment: None,
            max_width: None,
            max_height: None,
        },
        &mut meshes,
    )
    .unwrap();

    // Place the meshes
    for mesh in meshes {
        commands.spawn((
            Mesh3d(mesh.mesh),
            MeshMaterial3d(mesh.material),
            mesh.transform.with_translation(Vec3::new(
                -200.0 + mesh.transform.translation.x,
                mesh.transform.translation.y,
                mesh.transform.translation.z,
            )),
        ));
    }
}
```

### Rich Text

The `InputText::Rich` type allows you to create rich texts.
It consists out of a `Vec<>` of words, a `Vec<>` of materials and a `Vec<>` of `Attrs`. These vecs *need* to have the same length.

### Missing features

I'd have loved to also implement Bevel functionality, but I tried and failed to implement it. If someone wants to have a go at this, feel free.
