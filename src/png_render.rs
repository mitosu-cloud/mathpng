use skrifa::instance::LocationRef;
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::MetadataProvider;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

use crate::font::MathFont;
use crate::layout::{LayoutContent, LayoutNode};

/// Pen that converts glyph outlines to tiny-skia PathBuilder commands.
struct SkiaPen<'a> {
    builder: &'a mut PathBuilder,
}

impl<'a> OutlinePen for SkiaPen<'a> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(x, y);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.builder.quad_to(cx0, cy0, x, y);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.builder.cubic_to(cx0, cy0, cx1, cy1, x, y);
    }

    fn close(&mut self) {
        self.builder.close();
    }
}

/// Render a LayoutNode tree to a tiny-skia Pixmap.
pub fn render_node(
    font: &MathFont,
    pixmap: &mut Pixmap,
    node: &LayoutNode,
    x: f32,
    baseline_y: f32,
    fg_color: [u8; 4],
) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(fg_color[0], fg_color[1], fg_color[2], fg_color[3]);
    paint.anti_alias = true;

    render_node_inner(font, pixmap, node, x, baseline_y, &paint);
}

fn render_node_inner(
    font: &MathFont,
    pixmap: &mut Pixmap,
    node: &LayoutNode,
    x: f32,
    baseline_y: f32,
    paint: &Paint,
) {
    match &node.content {
        LayoutContent::Glyph {
            glyph_id,
            font_size_px,
        } => {
            let gid = skrifa::GlyphId::new(*glyph_id as u32);
            let font_ref = font.font_ref();
            let outlines = font_ref.outline_glyphs();

            if let Some(outline) = outlines.get(gid) {
                let mut builder = PathBuilder::new();
                let size = skrifa::instance::Size::new(*font_size_px);
                let settings = DrawSettings::unhinted(size, LocationRef::default());

                let mut pen = SkiaPen {
                    builder: &mut builder,
                };
                let _ = outline.draw(settings, &mut pen);

                if let Some(path) = builder.finish() {
                    let transform = Transform::from_row(1.0, 0.0, 0.0, -1.0, x, baseline_y);
                    pixmap.fill_path(&path, paint, FillRule::Winding, transform, None);
                }
            }
        }

        LayoutContent::HBox { children } | LayoutContent::VBox { children } => {
            for child in children {
                render_node_inner(
                    font,
                    pixmap,
                    &child.node,
                    x + child.x,
                    baseline_y - child.y,
                    paint,
                );
            }
        }

        LayoutContent::Rule { thickness } => {
            let rect = tiny_skia::Rect::from_xywh(
                x,
                baseline_y - thickness / 2.0,
                node.width,
                *thickness,
            );
            if let Some(rect) = rect {
                pixmap.fill_rect(rect, paint, Transform::identity(), None);
            }
        }

        LayoutContent::Kern => {}
    }
}
