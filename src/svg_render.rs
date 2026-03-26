use std::collections::HashMap;
use std::fmt::Write;

use skrifa::instance::LocationRef;
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::MetadataProvider;

use crate::font::MathFont;
use crate::layout::{LayoutContent, LayoutNode};
use crate::RenderOptions;

/// Pen that converts glyph outlines to SVG path `d` attribute commands.
struct SvgPen {
    d: String,
}

impl SvgPen {
    fn new() -> Self {
        Self { d: String::new() }
    }
}

fn fmt(v: f32) -> String {
    // Round to 2 decimal places, strip trailing zeros
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0');
    let s = s.trim_end_matches('.');
    s.to_string()
}

impl OutlinePen for SvgPen {
    fn move_to(&mut self, x: f32, y: f32) {
        write!(self.d, "M{} {}", fmt(x), fmt(y)).unwrap();
    }

    fn line_to(&mut self, x: f32, y: f32) {
        write!(self.d, "L{} {}", fmt(x), fmt(y)).unwrap();
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        write!(self.d, "Q{} {} {} {}", fmt(cx0), fmt(cy0), fmt(x), fmt(y)).unwrap();
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        write!(
            self.d,
            "C{} {} {} {} {} {}",
            fmt(cx0), fmt(cy0), fmt(cx1), fmt(cy1), fmt(x), fmt(y)
        )
        .unwrap();
    }

    fn close(&mut self) {
        self.d.push('Z');
    }
}

/// Collect all unique (glyph_id, font_size_px) pairs from a layout tree.
fn collect_glyphs(node: &LayoutNode, glyphs: &mut HashMap<(u16, u32), ()>) {
    match &node.content {
        LayoutContent::Glyph {
            glyph_id,
            font_size_px,
        } => {
            glyphs.insert((*glyph_id, font_size_px.to_bits()), ());
        }
        LayoutContent::HBox { children } | LayoutContent::VBox { children } => {
            for child in children {
                collect_glyphs(&child.node, glyphs);
            }
        }
        LayoutContent::Rule { .. } | LayoutContent::Kern => {}
    }
}

/// Extract the SVG path `d` attribute for a glyph at a given size.
fn glyph_path_data(font: &MathFont, glyph_id: u16, font_size_px: f32) -> Option<String> {
    let gid = skrifa::GlyphId::new(glyph_id as u32);
    let font_ref = font.font_ref();
    let outlines = font_ref.outline_glyphs();

    let outline = outlines.get(gid)?;
    let size = skrifa::instance::Size::new(font_size_px);
    let settings = DrawSettings::unhinted(size, LocationRef::default());

    let mut pen = SvgPen::new();
    outline.draw(settings, &mut pen).ok()?;

    if pen.d.is_empty() {
        None
    } else {
        Some(pen.d)
    }
}

fn color_to_svg(rgba: [u8; 4]) -> String {
    if rgba[3] == 255 {
        format!("#{:02x}{:02x}{:02x}", rgba[0], rgba[1], rgba[2])
    } else {
        let alpha = rgba[3] as f32 / 255.0;
        format!(
            "#{:02x}{:02x}{:02x}\" fill-opacity=\"{:.2}",
            rgba[0], rgba[1], rgba[2], alpha
        )
    }
}

/// Recursively emit SVG elements for a layout node.
fn render_svg_node(
    out: &mut String,
    node: &LayoutNode,
    x: f32,
    baseline_y: f32,
    defs: &HashMap<(u16, u32), String>,
) {
    match &node.content {
        LayoutContent::Glyph {
            glyph_id,
            font_size_px,
        } => {
            let key = (*glyph_id, font_size_px.to_bits());
            if let Some(def_id) = defs.get(&key) {
                // Y-flip: font outlines are Y-up, SVG is Y-down
                write!(
                    out,
                    "<use href=\"#{}\" transform=\"matrix(1 0 0 -1 {} {})\"/>",
                    def_id,
                    fmt(x),
                    fmt(baseline_y)
                )
                .unwrap();
            }
        }

        LayoutContent::HBox { children } | LayoutContent::VBox { children } => {
            for child in children {
                render_svg_node(
                    out,
                    &child.node,
                    x + child.x,
                    baseline_y - child.y,
                    defs,
                );
            }
        }

        LayoutContent::Rule { thickness } => {
            write!(
                out,
                r#"<rect x="{}" y="{}" width="{}" height="{}"/>"#,
                fmt(x),
                fmt(baseline_y - thickness / 2.0),
                fmt(node.width),
                fmt(*thickness)
            )
            .unwrap();
        }

        LayoutContent::Kern => {}
    }
}

/// Render a layout tree to an SVG string.
pub fn render_to_svg_string(
    font: &MathFont,
    layout: &LayoutNode,
    opts: &RenderOptions,
) -> String {
    let padding = opts.padding as f32;
    let img_width = layout.width + 2.0 * padding;
    let img_height = layout.height() + 2.0 * padding;

    // Collect unique glyphs and build defs
    let mut glyph_set = HashMap::new();
    collect_glyphs(layout, &mut glyph_set);

    let mut defs_map: HashMap<(u16, u32), String> = HashMap::new();
    let mut defs_svg = String::new();
    let mut glyph_idx = 0;

    for &(glyph_id, size_bits) in glyph_set.keys() {
        let font_size_px = f32::from_bits(size_bits);
        if let Some(path_d) = glyph_path_data(font, glyph_id, font_size_px) {
            let def_id = format!("g{}", glyph_idx);
            write!(
                defs_svg,
                r#"<path id="{}" d="{}"/>"#,
                def_id, path_d
            )
            .unwrap();
            defs_map.insert((glyph_id, size_bits), def_id);
            glyph_idx += 1;
        }
    }

    // Build SVG
    let mut svg = String::new();
    write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
        fmt(img_width),
        fmt(img_height),
        fmt(img_width),
        fmt(img_height)
    )
    .unwrap();

    // Defs
    if !defs_svg.is_empty() {
        write!(svg, "<defs>{}</defs>", defs_svg).unwrap();
    }

    // Background
    if opts.bg_color[3] > 0 {
        write!(
            svg,
            r#"<rect width="100%" height="100%" fill="{}"/>"#,
            color_to_svg(opts.bg_color)
        )
        .unwrap();
    }

    // Content group with foreground color
    write!(svg, r#"<g fill="{}">"#, color_to_svg(opts.fg_color)).unwrap();

    let origin_x = padding;
    let origin_y = padding + layout.ascent;

    render_svg_node(&mut svg, layout, origin_x, origin_y, &defs_map);

    svg.push_str("</g></svg>");
    svg
}
