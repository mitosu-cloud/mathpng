use skrifa::instance::LocationRef;
use skrifa::metrics::GlyphMetrics;
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::MathRenderError;

const FONT_DATA: &[u8] = include_bytes!("../fonts/STIXTwoMath-Regular.otf");

pub struct MathFont {
    font_data: &'static [u8],
    units_per_em: f32,
}

impl MathFont {
    pub fn load() -> Result<Self, MathRenderError> {
        let font = FontRef::new(FONT_DATA)
            .map_err(|e| MathRenderError::Font(format!("Failed to load font: {e}")))?;
        let upem = font.head().map_err(|e| MathRenderError::Font(e.to_string()))?
            .units_per_em() as f32;
        Ok(Self {
            font_data: FONT_DATA,
            units_per_em: upem,
        })
    }

    pub fn font_ref(&self) -> FontRef<'_> {
        FontRef::new(self.font_data).unwrap()
    }

    pub fn units_per_em(&self) -> f32 {
        self.units_per_em
    }

    /// Map a character to a glyph ID.
    pub fn glyph_id(&self, c: char) -> Option<GlyphId> {
        let font = self.font_ref();
        let charmap = font.charmap();
        charmap.map(c)
    }

    /// Get glyph metrics at a given ppem size.
    pub fn glyph_metrics(&self, font_size_px: f32) -> GlyphMetrics<'_> {
        let font = self.font_ref();
        let size = skrifa::instance::Size::new(font_size_px);
        font.glyph_metrics(size, LocationRef::default())
    }

    /// Get font-level metrics (ascent, descent) at a given ppem size.
    pub fn metrics(&self, font_size_px: f32) -> skrifa::metrics::Metrics {
        let font = self.font_ref();
        let size = skrifa::instance::Size::new(font_size_px);
        font.metrics(size, LocationRef::default())
    }
}
