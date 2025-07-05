use cosmic_text::ttf_parser as ttf;
use cosmic_text::{FontSystem, LayoutGlyph};
use lyon::path::Path;
use crate::MeshTextError;

/// Represents a glyph outline extracted from a font
#[derive(Debug, Clone)]
pub struct GlyphOutline {
    pub path: Path,
    pub bounding_box: ttf::Rect,
    pub glyph_id: u16,
    pub font_size: f32,
    pub units_per_em: u16,
}

/// Extract glyph outline using cosmic-text's ttf-parser
pub fn extract_glyph_outline(
    glyph_info: &LayoutGlyph,
    font_system: &mut FontSystem,
) -> Result<GlyphOutline, MeshTextError> {
    font_system.db().with_face_data(glyph_info.font_id, |font_bytes, font_index| {
        let face = ttf::Face::parse(font_bytes, font_index)
            .map_err(|_| MeshTextError::FontParseFailed)?;
        
        let glyph_id = ttf::GlyphId(glyph_info.glyph_id);
        let bounding_box = face.glyph_bounding_box(glyph_id)
            .ok_or(MeshTextError::GlyphNotFound)?;
        
        let mut builder = crate::command_encoder::LyonCommandEncoder::new();
        let outline_result = face.outline_glyph(glyph_id, &mut builder);
        
        if outline_result.is_none() {
            return Err(MeshTextError::PathBuildingFailed);
        }
        
        let path = builder.build_path();
        
        // Check if the path is empty
        if path.iter().next().is_none() {
            #[cfg(feature = "debug")]
            println!("Empty path for glyph {}", glyph_info.glyph_id);
            return Err(MeshTextError::PathBuildingFailed);
        }
        
        #[cfg(feature = "debug")]
        println!("Checkpoint A: Extracted glyph {} with {} curves", 
                 glyph_info.glyph_id, path.iter().count());
        
        Ok(GlyphOutline {
            path,
            bounding_box,
            glyph_id: glyph_info.glyph_id,
            font_size: glyph_info.font_size,
            units_per_em: face.units_per_em(),
        })
    }).ok_or(MeshTextError::FontParseFailed)?
} 