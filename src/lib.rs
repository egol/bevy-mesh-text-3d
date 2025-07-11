use bevy::{
    asset::{Asset, Handle},
    render::mesh::Mesh,
    transform::components::Transform,
};
pub use cosmic_text::{
    Align, Attrs, CacheKeyFlags, CacheMetrics, Color, Family, Feature, FeatureTag, FontFeatures,
    LetterSpacing, Stretch, Style, Weight, fontdb::ID,
};

pub mod command_encoder;
pub mod extrude_glyph;
pub mod mesh_text_plugin;
pub mod text_glyphs;

// New modules for beveling system
pub mod glyph;
pub mod tess;
pub mod offset;
pub mod mesh;
pub mod render;

pub use mesh_text_plugin::*;
pub use extrude_glyph::{tessalate_glyph, tessellate_beveled_glyph};

// Export additional utilities for advanced usage
pub use offset::{contour_to_polyline, polyline_to_contour, approximate_arc, draw_polyline, draw_contour_outline, BevelRings};
pub use glyph::extract_glyph_outline;
pub use mesh::{build_mesh_from_bevel_rings, BeveledGlyphGeometry};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MeshTextError {
    #[error("Tessellation process failed")]
    TessellationFailed,

    #[error("Path Building Failed")]
    PathBuildingFailed,

    #[error("The input provided was invalid")]
    InvalidInput,
    
    #[error("Font parsing failed")]
    FontParseFailed,
    
    #[error("Glyph not found")]
    GlyphNotFound,
    
    #[error("Invalid contour")]
    InvalidContour,
    
    #[error("Invalid mesh: {0}")]
    InvalidMesh(String),
}

/// Parameters for beveling
#[derive(Debug, Clone)]
pub struct BevelParameters {
    /// Width of the bevel
    pub bevel_width: f32,
    /// Number of segments for curved profile (≥1)
    pub bevel_segments: u32,
    /// Profile power for curve shape (1=linear, 2=rounded)
    pub profile_power: f32,
}

impl Default for BevelParameters {
    fn default() -> Self {
        Self {
            bevel_width: 0.1,
            bevel_segments: 1,
            profile_power: 1.0,
        }
    }
}

/// A extruded glyph mesh.
#[derive(Debug)]
pub struct MeshGlyph<M: Asset> {
    pub glyph_id: u16,
    pub font_id: Option<ID>,
    pub x: f32,
    pub y: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub line_y: f32,
    pub glyph_center_x_layout: f32,
    pub glyph_center_y_layout: f32,
    pub height: f32,
    pub mesh: Handle<Mesh>,
    pub material: Handle<M>,
}

/// The text input for glyph mesh generation.
pub enum InputText<'a, M: Asset> {
    /// A simple text with a string and a single material
    Simple {
        text: String,
        material: Handle<M>,
        attrs: Attrs<'a>,
    },
    /// A rich text with a vector of words and materials.
    /// The three Vecs must be the same length.
    Rich {
        words: Vec<String>,
        materials: Vec<Handle<M>>,
        attrs: Vec<Attrs<'a>>,
    },
}

impl<M: Asset> InputText<'_, M> {
    pub fn is_valid(&self) -> bool {
        match self {
            InputText::Simple { text, .. } => !text.is_empty(),
            InputText::Rich {
                words,
                materials,
                attrs,
            } => !words.is_empty() && words.len() == materials.len() && attrs.len() == words.len(),
        }
    }
}

/// A extruded glyph mesh including the transform and material.
pub struct MeshTextEntry<M: Asset> {
    /// The mesh handle of the glyph. Similar glyphes share the same mesh handle
    pub mesh: Handle<Mesh>,
    /// The transform for this glyph in the provided sentence / text
    pub transform: Transform,
    /// The material of this glyph
    pub material: Handle<M>,
}

pub struct Parameters {
    /// Extrusion depth
    pub extrusion_depth: f32,
    /// Font size
    pub font_size: f32,
    /// Line height
    pub line_height: f32,
    /// Alignment
    pub alignment: Option<Align>,
    /// Maximum width of the textbox. Beyond this width, the text will wrap.
    pub max_width: Option<f32>,
    /// Maximum height of the textbox.
    pub max_height: Option<f32>,
    /// Bevel parameters
    pub bevel: Option<BevelParameters>,
}
