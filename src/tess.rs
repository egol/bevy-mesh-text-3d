use bevy::prelude::*;
use lyon::{
    path::Path,
    tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers},
};
use crate::MeshTextError;

/// Tessellated geometry for the front cap of a glyph
#[derive(Debug, Clone)]
pub struct TessellatedGeometry {
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u16>,
    pub scale_factor: f32,
    pub center_x: f32,
    pub center_y: f32,
}

/// Tessellate the front cap of a glyph using Lyon
pub fn tessellate_front_cap(
    path: &Path,
    bounding_box: cosmic_text::ttf_parser::Rect,
    font_size: f32,
    units_per_em: u16,
    glyph_id: u16,
) -> Result<TessellatedGeometry, MeshTextError> {
    let scale_factor = font_size / units_per_em as f32;
    
    // Calculate the center of the glyph using the font units bounding box
    let center_x = (bounding_box.x_min as f32 + bounding_box.x_max as f32) / 2.0;
    let center_y = (bounding_box.y_min as f32 + bounding_box.y_max as f32) / 2.0;
    
    let tolerance = 0.25; // ¼ unit in font space
    let mut options = FillOptions::default();
    options.tolerance = tolerance;
    
    let mut geometry: VertexBuffers<Vec3, u16> = VertexBuffers::new();
    let mut tessellator = FillTessellator::new();
    
    // Try tessellation with fallbacks
    let result = try_tessellation_with_fallbacks(
        &mut tessellator,
        path,
        center_x,
        center_y,
        scale_factor,
        &mut geometry,
        glyph_id,
    );
    
    if result.is_err() {
        return Err(MeshTextError::TessellationFailed);
    }
    
    #[cfg(feature = "debug")]
    println!("Checkpoint B: Tessellated glyph {} - {} vertices, {} indices", 
             glyph_id, geometry.vertices.len(), geometry.indices.len());
    
    Ok(TessellatedGeometry {
        vertices: geometry.vertices,
        indices: geometry.indices,
        scale_factor,
        center_x,
        center_y,
    })
}

fn try_tessellation_with_fallbacks(
    tessellator: &mut FillTessellator,
    path: &Path,
    center_x: f32,
    center_y: f32,
    scale_factor: f32,
    geometry: &mut VertexBuffers<Vec3, u16>,
    _glyph_id: u16,
) -> Result<(), MeshTextError> {
    let front_z = 0.0;
    
    // First attempt: Normal tessellation with default options
    let mut options = FillOptions::default();
    options.tolerance = 0.25;
    
    let result = tessellator.tessellate_path(
        path,
        &options,
        &mut BuffersBuilder::new(geometry, |vertex: FillVertex| Vec3 {
            x: (vertex.position().x - center_x) * scale_factor,
            y: (vertex.position().y - center_y) * scale_factor,
            z: front_z,
        }),
    );
    
    if result.is_ok() {
        return Ok(());
    }
    
    #[cfg(feature = "debug")]
    println!("Normal tessellation failed for glyph {}, trying with higher tolerance", _glyph_id);
    
    // Second attempt: Higher tolerance
    geometry.vertices.clear();
    geometry.indices.clear();
    options.tolerance = 0.5;
    
    let result = tessellator.tessellate_path(
        path,
        &options,
        &mut BuffersBuilder::new(geometry, |vertex: FillVertex| Vec3 {
            x: (vertex.position().x - center_x) * scale_factor,
            y: (vertex.position().y - center_y) * scale_factor,
            z: front_z,
        }),
    );
    
    if result.is_ok() {
        return Ok(());
    }
    
    #[cfg(feature = "debug")]
    println!("High tolerance tessellation failed for glyph {}, trying non-zero fill rule", _glyph_id);
    
    // Third attempt: Non-zero fill rule
    geometry.vertices.clear();
    geometry.indices.clear();
    options.fill_rule = lyon::tessellation::FillRule::NonZero;
    
    let result = tessellator.tessellate_path(
        path,
        &options,
        &mut BuffersBuilder::new(geometry, |vertex: FillVertex| Vec3 {
            x: (vertex.position().x - center_x) * scale_factor,
            y: (vertex.position().y - center_y) * scale_factor,
            z: front_z,
        }),
    );
    
    result.map_err(|_| MeshTextError::TessellationFailed)
} 