use crate::text_glyphs::TextGlyphs;
use crate::{InputText, MeshTextError};
use crate::{MeshTextEntry, Parameters};
use bevy::prelude::*;
use cosmic_text::fontdb::{Database, Source};
use std::sync::Arc;
use cosmic_text::{FontSystem, Metrics};

pub struct MeshTextPlugin(f32);

impl MeshTextPlugin {
    pub fn new(text_scale_factor: f32) -> Self {
        Self(text_scale_factor)
    }
}

impl Plugin for MeshTextPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Settings {
            font_system: {
                // Load a single, embedded font into a custom font database so we avoid an expensive scan of the host system fonts.
                // NOTE: Replace the placeholder "Roboto-Bold.ttf" with the actual font you want to embed.
                let font_source = Source::Binary(Arc::new(include_bytes!("../assets/centurygothic_bold.ttf").to_vec()));

                let mut font_db = Database::new();
                font_db.load_font_source(font_source);

                // Initialise the FontSystem with a fixed locale and our prepared database
                FontSystem::new_with_locale_and_db(String::from("en-US"), font_db)
            },
            text_scale_factor: self.0,
        });
    }
}

pub fn generate_meshes<M: Asset>(
    text: InputText<M>,
    fonts: &mut ResMut<Settings>,
    params: Parameters,
    meshes: &mut ResMut<Assets<Mesh>>,
) -> Result<Vec<MeshTextEntry<M>>, MeshTextError> {
    if !text.is_valid() {
        error!("Invalid text input");
        return Err(MeshTextError::InvalidInput);
    }

    let (materials, spans, default_attrs) = match text {
        InputText::Simple {
            material,
            ref text,
            attrs,
        } => (vec![material], vec![(text.as_str(), attrs.clone())], attrs),
        InputText::Rich {
            materials,
            ref words,
            attrs,
        } => (
            materials,
            words
                .iter()
                .map(|w| w.as_str())
                .zip(attrs.iter())
                .enumerate()
                .map(|(i, (word, attr))| (word, attr.clone().metadata(i)))
                .collect(),
            attrs[0].clone(),
        ),
    };

    let default_metrics = Metrics {
        font_size: params.font_size,
        line_height: params.line_height,
    };

    let text_scale_factor = fonts.text_scale_factor;

    let mut tx = TextGlyphs::new(
        default_metrics,
        spans,
        &default_attrs,
        &mut fonts.font_system,
        params.alignment,
    );
    let (_width, _height) = tx.measure(params.max_width, params.max_height, &mut fonts.font_system);
    let processed_glyphs = tx.generate_mesh_glyphs(
        &mut fonts.font_system,
        params.extrusion_depth,
        meshes,
        &materials,
    );

    let mut meshes = Vec::new();

    for glyph_data in processed_glyphs {
        // Calculate the target world position for the glyph's visual center.
        // glyph_data.x, .y, .x_offset, .y_offset, .line_y are from CosmicText layout.
        // glyph_data.glyph_center_x_layout and .glyph_center_y_layout are the offsets
        // from the glyph's own layout origin to its visual center, in layout units.
        // (Layout units are scaled by font_size relative to font design units).

        let target_center_x_layout_units =
            glyph_data.x + glyph_data.x_offset + glyph_data.glyph_center_x_layout;

        // Calculate the Y position for the glyph's visual center in Bevy's Y-up world space.
        // 1. Sum CosmicText's Y-down layout components:
        //    line_y: baseline position (Y increases downwards from top of text buffer).
        //    glyph.y: glyph's offset from baseline (Y increases downwards if positive).
        //    glyph.y_offset: additional Y offset in the same system.
        let sum_y_components_layout_down = glyph_data.line_y + glyph_data.y + glyph_data.y_offset;

        // 2. Convert the sum to a Y-up Bevy coordinate. If CosmicText Y=0 (top) is Bevy Y=H,
        //    and CosmicText Y=H (bottom) is Bevy Y=0, this would be (H_text_block - sum_y_components_layout_down).
        //    Simpler: if mapping Cosmic Y=0 to Bevy Y=0 and flipping axis: Bevy_Y_up = -Cosmic_Y_down.
        let glyph_origin_y_layout_bevy_up = -sum_y_components_layout_down;

        // 3. Add the glyph's intrinsic Y-up center offset.
        //    glyph_data.glyph_center_y_layout is the Y-up distance from the glyph's font origin to its visual center.
        let target_center_y_layout_units_bevy_up =
            glyph_origin_y_layout_bevy_up + glyph_data.glyph_center_y_layout;

        let world_x = target_center_x_layout_units * text_scale_factor;
        let world_y = target_center_y_layout_units_bevy_up * text_scale_factor; // Use the new Y-up calculation

        meshes.push(MeshTextEntry {
            mesh: glyph_data.mesh,
            material: glyph_data.material,
            transform: Transform::from_xyz(world_x, world_y, 0.0)
                .with_scale(Vec3::splat(text_scale_factor)),
        });
    }

    Ok(meshes)
}

#[derive(Resource)]
pub struct Settings {
    pub font_system: FontSystem,
    pub text_scale_factor: f32,
}
