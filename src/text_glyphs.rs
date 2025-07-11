use bevy::prelude::*;
use cosmic_text::{
    Align, Attrs, Buffer, FontSystem, Metrics, Shaping,
    ttf_parser::{Face, GlyphId},
};
use std::collections::HashMap;

use crate::MeshGlyph;
use crate::extrude_glyph::{tessalate_glyph, tessellate_beveled_glyph};
use crate::BevelParameters;

pub struct TextGlyphs {
    buffer: cosmic_text::Buffer,
}

impl TextGlyphs {
    pub fn new<'r, 's, I>(
        metrics: Metrics,
        spans: I,
        default_attrs: &Attrs,
        font_system: &mut FontSystem,
        alignment: Option<Align>,
    ) -> Self
    where
        I: IntoIterator<Item = (&'s str, Attrs<'r>)>,
    {
        let mut buffer = Buffer::new_empty(metrics);
        buffer.set_rich_text(
            font_system,
            spans,
            default_attrs,
            Shaping::Advanced,
            alignment,
        );
        Self { buffer }
    }

    pub fn measure(
        &mut self,
        width_opt: Option<f32>,
        height_opt: Option<f32>,
        font_system: &mut FontSystem,
    ) -> (f32, f32) {
        self.buffer.set_size(font_system, width_opt, height_opt);

        // Compute layout
        self.buffer.shape_until_scroll(font_system, false);

        // Determine measured size of text
        let (width, total_lines) = self
            .buffer
            .layout_runs()
            .fold((0.0, 0usize), |(width, total_lines), run| {
                (run.line_w.max(width), total_lines + 1)
            });
        let height = total_lines as f32 * self.buffer.metrics().line_height;

        (width, height)
    }

    pub fn generate_mesh_glyphs<M: Asset>(
        &self,
        font_system: &mut FontSystem,
        extrusion_depth: f32,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &[Handle<M>],
        bevel_params: Option<&BevelParameters>,
    ) -> Vec<MeshGlyph<M>> {
        let mut mesh_map: HashMap<(u16, bool), (Handle<Mesh>, f32, f32)> = HashMap::new();
        let mut processed_glyphs = Vec::new();
        let mut cache_hits = 0;
        let mut cache_builds = 0;
        
        for run in self.buffer.layout_runs() {
            for glyph in run.glyphs {
                let use_beveling = bevel_params.is_some();
                let cache_key = (glyph.glyph_id, use_beveling);
                
                let Some((geometry, center_x_layout, center_y_layout)) = mesh_map
                    .get(&cache_key)
                    .map(|(mesh, center_x_layout, center_y_layout)| {
                        cache_hits += 1;
                        (mesh.clone(), *center_x_layout, *center_y_layout)
                    })
                    .or_else(|| {
                        cache_builds += 1;
                        
                        let tessellation_result = if let Some(bevel_params) = bevel_params {
                            #[cfg(feature = "debug")]
                            println!("Attempting beveled tessellation for glyph {}", glyph.glyph_id);
                            
                            match tessellate_beveled_glyph(glyph, font_system, extrusion_depth, bevel_params) {
                                Ok(result) => {
                                    #[cfg(feature = "debug")]
                                    println!("Beveled tessellation succeeded for glyph {}", glyph.glyph_id);
                                    Some(result)
                                }
                                Err(e) => {
                                    #[cfg(feature = "debug")]
                                    println!("Beveled tessellation failed for glyph {}: {:?}, falling back to original", glyph.glyph_id, e);
                                    
                                    // Fallback to original tessellation method
                                    font_system
                                        .db()
                                        .with_face_data(glyph.font_id, |file, _| {
                                            let Ok(face) = Face::parse(file, 0) else {
                                                error!("Failed to parse font");
                                                return None;
                                            };
                                            
                                            // Check if the glyph has a bounding box (space characters don't)
                                            let Some(bb) = face.glyph_bounding_box(GlyphId(glyph.glyph_id)) else {
                                                return None;
                                            };
                                            
                                            match tessalate_glyph(glyph, bb, face, extrusion_depth) {
                                                Ok(n) => Some(n),
                                                Err(e) => {
                                                    error!("Failed to tessalate glyph {}: {}", glyph.glyph_id, e);
                                                    None
                                                }
                                            }
                                        })
                                        .flatten()
                                }
                            }
                        } else {
                            #[cfg(feature = "debug")]
                            println!("Using original tessellation for glyph {}", glyph.glyph_id);
                            
                            // Use original tessellation method
                            font_system
                                .db()
                                .with_face_data(glyph.font_id, |file, _| {
                                    let Ok(face) = Face::parse(file, 0) else {
                                        error!("Failed to parse font");
                                        return None;
                                    };
                                    
                                    // Check if the glyph has a bounding box (space characters don't)
                                    let Some(bb) = face.glyph_bounding_box(GlyphId(glyph.glyph_id)) else {
                                        return None;
                                    };
                                    
                                    match tessalate_glyph(glyph, bb, face, extrusion_depth) {
                                        Ok(n) => Some(n),
                                        Err(e) => {
                                            error!("Failed to tessalate glyph {}: {}", glyph.glyph_id, e);
                                            None
                                        }
                                    }
                                })
                                .flatten()
                        };
                        
                        tessellation_result
                            .map(|(geometry, center_x_layout, center_y_layout)| {
                                (meshes.add(geometry), center_x_layout, center_y_layout)
                            })
                    })
                else {
                    continue;
                };
                
                mesh_map
                    .entry(cache_key)
                    .or_insert_with(|| (geometry.clone(), center_x_layout, center_y_layout));

                let material = materials
                    .get(glyph.metadata)
                    .unwrap_or_else(|| &materials[0])
                    .clone();

                processed_glyphs.push(MeshGlyph {
                    glyph_id: glyph.glyph_id,
                    font_id: Some(glyph.font_id),
                    x: glyph.x,
                    y: glyph.y,
                    x_offset: glyph.x_offset,
                    y_offset: glyph.y_offset,
                    line_y: run.line_y,
                    glyph_center_x_layout: center_x_layout,
                    glyph_center_y_layout: center_y_layout,
                    height: glyph.font_size,
                    mesh: geometry,
                    material,
                });
            }
        }
        
        #[cfg(feature = "debug")]
        println!("Checkpoint F: Cache stats - {} hits, {} builds", cache_hits, cache_builds);
        
        processed_glyphs
    }
}
