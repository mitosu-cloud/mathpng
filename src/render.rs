use skrifa::instance::LocationRef;
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::MetadataProvider;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

use crate::font::MathFont;
use crate::math_expr::{map_variant, MathExpr, MathVariant};

/// A positioned, measured box in the layout tree.
/// All measurements in pixels.
#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub content: LayoutContent,
}

impl LayoutNode {
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}

#[derive(Debug, Clone)]
pub enum LayoutContent {
    Glyph {
        glyph_id: u16,
        font_size_px: f32,
    },
    HBox {
        children: Vec<PositionedNode>,
    },
    VBox {
        children: Vec<PositionedNode>,
    },
    Rule {
        thickness: f32,
    },
    Kern,
}

#[derive(Debug, Clone)]
pub struct PositionedNode {
    pub x: f32,
    pub y: f32, // positive = up from baseline
    pub node: LayoutNode,
}

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

pub struct Renderer<'a> {
    font: &'a MathFont,
    font_size_px: f32,
}

impl<'a> Renderer<'a> {
    pub fn new(font: &'a MathFont, font_size_px: f32) -> Self {
        Self { font, font_size_px }
    }

    fn scale(&self) -> f32 {
        self.font_size_px / self.font.units_per_em()
    }

    fn scaled(&self, font_units: f32) -> f32 {
        font_units * self.scale()
    }

    /// Layout a MathExpr tree into a LayoutNode tree.
    pub fn layout(&self, expr: &MathExpr, display_mode: bool) -> LayoutNode {
        self.layout_expr(expr, self.font_size_px, display_mode, false)
    }

    fn layout_expr(
        &self,
        expr: &MathExpr,
        font_size_px: f32,
        display_mode: bool,
        _cramped: bool,
    ) -> LayoutNode {
        match expr {
            MathExpr::Glyph {
                codepoint,
                variant,
            } => self.layout_glyph(*codepoint, *variant, font_size_px),

            MathExpr::Row(children) | MathExpr::Group(children) => {
                self.layout_hbox(children, font_size_px, display_mode)
            }

            MathExpr::Fraction {
                numerator,
                denominator,
            } => self.layout_fraction(numerator, denominator, font_size_px, display_mode),

            MathExpr::Scripts {
                base,
                superscript,
                subscript,
            } => self.layout_scripts(
                base,
                superscript.as_deref(),
                subscript.as_deref(),
                font_size_px,
                display_mode,
            ),

            MathExpr::Radical { radicand, index } => {
                self.layout_radical(radicand, index.as_deref(), font_size_px, display_mode)
            }

            MathExpr::Delimited {
                open,
                close,
                content,
            } => self.layout_delimited(*open, *close, content, font_size_px, display_mode),

            MathExpr::BigOperator {
                symbol,
                above,
                below,
                limits,
            } => self.layout_big_operator(
                *symbol,
                above.as_deref(),
                below.as_deref(),
                *limits,
                font_size_px,
                display_mode,
            ),

            MathExpr::Space(em) => {
                let width = em * font_size_px;
                LayoutNode {
                    width,
                    ascent: 0.0,
                    descent: 0.0,
                    content: LayoutContent::Kern,
                }
            }

            MathExpr::Text(s) => self.layout_text(s, font_size_px),

            MathExpr::Accent { base, accent_char } => {
                self.layout_accent(base, *accent_char, font_size_px, display_mode)
            }

        }
    }

    fn layout_glyph(
        &self,
        codepoint: char,
        variant: MathVariant,
        font_size_px: f32,
    ) -> LayoutNode {
        let mapped = map_variant(codepoint, variant);
        let font_ref = self.font.font_ref();

        let glyph_id = font_ref
            .charmap()
            .map(mapped)
            .or_else(|| font_ref.charmap().map(codepoint));

        if let Some(gid) = glyph_id {
            let size = skrifa::instance::Size::new(font_size_px);
            let glyph_metrics = font_ref.glyph_metrics(size, LocationRef::default());
            let metrics = font_ref.metrics(size, LocationRef::default());

            let advance = glyph_metrics.advance_width(gid).unwrap_or(0.0);
            let ascent = metrics.ascent;
            let descent = metrics.descent.abs();

            // Try to get tighter bounds from the glyph bounding box
            let (glyph_ascent, glyph_descent) = if let Some(bounds) = glyph_metrics.bounds(gid) {
                (bounds.y_max.max(0.0), (-bounds.y_min).max(0.0))
            } else {
                (ascent, descent)
            };

            LayoutNode {
                width: advance,
                ascent: glyph_ascent,
                descent: glyph_descent,
                content: LayoutContent::Glyph {
                    glyph_id: gid.to_u32() as u16,
                    font_size_px,
                },
            }
        } else {
            // Fallback: render as a space-width box
            LayoutNode {
                width: font_size_px * 0.5,
                ascent: font_size_px * 0.7,
                descent: 0.0,
                content: LayoutContent::Kern,
            }
        }
    }

    fn layout_hbox(
        &self,
        exprs: &[MathExpr],
        font_size_px: f32,
        display_mode: bool,
    ) -> LayoutNode {
        let mut children = Vec::new();
        let mut x = 0.0f32;
        let mut max_ascent = 0.0f32;
        let mut max_descent = 0.0f32;

        for expr in exprs {
            let node = self.layout_expr(expr, font_size_px, display_mode, false);
            max_ascent = max_ascent.max(node.ascent);
            max_descent = max_descent.max(node.descent);
            children.push(PositionedNode {
                x,
                y: 0.0,
                node: node.clone(),
            });
            x += node.width;
        }

        LayoutNode {
            width: x,
            ascent: max_ascent,
            descent: max_descent,
            content: LayoutContent::HBox { children },
        }
    }

    fn layout_fraction(
        &self,
        numerator: &MathExpr,
        denominator: &MathExpr,
        font_size_px: f32,
        display_mode: bool,
    ) -> LayoutNode {
        let num_node = self.layout_expr(numerator, font_size_px, display_mode, false);
        let den_node = self.layout_expr(denominator, font_size_px, display_mode, true);

        // Use reasonable defaults based on font size for MATH table constants
        let rule_thickness = font_size_px * 0.06;
        let axis_height = font_size_px * 0.25;
        let num_gap = if display_mode {
            font_size_px * 0.15
        } else {
            font_size_px * 0.08
        };
        let den_gap = num_gap;

        let bar_y = axis_height;

        // Position numerator above the bar
        let num_baseline = bar_y + rule_thickness / 2.0 + num_gap + num_node.descent;
        // Position denominator below the bar
        let den_baseline = -(bar_y - (-(rule_thickness / 2.0) - den_gap - den_node.ascent)).abs();

        let width = num_node.width.max(den_node.width);
        let num_x = (width - num_node.width) / 2.0;
        let den_x = (width - den_node.width) / 2.0;

        let ascent = num_baseline + num_node.ascent;
        let descent = (-den_baseline) + den_node.descent;

        LayoutNode {
            width,
            ascent,
            descent,
            content: LayoutContent::VBox {
                children: vec![
                    PositionedNode {
                        x: num_x,
                        y: num_baseline,
                        node: num_node,
                    },
                    PositionedNode {
                        x: 0.0,
                        y: bar_y,
                        node: LayoutNode {
                            width,
                            ascent: rule_thickness / 2.0,
                            descent: rule_thickness / 2.0,
                            content: LayoutContent::Rule {
                                thickness: rule_thickness,
                            },
                        },
                    },
                    PositionedNode {
                        x: den_x,
                        y: den_baseline,
                        node: den_node,
                    },
                ],
            },
        }
    }

    fn layout_scripts(
        &self,
        base: &MathExpr,
        sup: Option<&MathExpr>,
        sub: Option<&MathExpr>,
        font_size_px: f32,
        display_mode: bool,
    ) -> LayoutNode {
        let base_node = self.layout_expr(base, font_size_px, display_mode, false);
        let script_size = font_size_px * 0.7; // script scale down

        let mut children = vec![PositionedNode {
            x: 0.0,
            y: 0.0,
            node: base_node.clone(),
        }];
        let mut total_width = base_node.width;
        let mut total_ascent = base_node.ascent;
        let mut total_descent = base_node.descent;

        if let Some(sup_expr) = sup {
            let sup_node = self.layout_expr(sup_expr, script_size, false, false);
            let shift_up = (font_size_px * 0.4).max(base_node.ascent * 0.6);
            let sup_x = base_node.width;

            children.push(PositionedNode {
                x: sup_x,
                y: shift_up,
                node: sup_node.clone(),
            });
            total_width = total_width.max(sup_x + sup_node.width);
            total_ascent = total_ascent.max(shift_up + sup_node.ascent);
        }

        if let Some(sub_expr) = sub {
            let sub_node = self.layout_expr(sub_expr, script_size, false, false);
            let shift_down = (font_size_px * 0.2).max(base_node.descent * 0.5);
            let sub_x = base_node.width;

            children.push(PositionedNode {
                x: sub_x,
                y: -shift_down,
                node: sub_node.clone(),
            });
            total_width = total_width.max(sub_x + sub_node.width);
            total_descent = total_descent.max(shift_down + sub_node.descent);
        }

        LayoutNode {
            width: total_width,
            ascent: total_ascent,
            descent: total_descent,
            content: LayoutContent::HBox { children },
        }
    }

    fn layout_radical(
        &self,
        radicand: &MathExpr,
        _index: Option<&MathExpr>,
        font_size_px: f32,
        display_mode: bool,
    ) -> LayoutNode {
        let radicand_node = self.layout_expr(radicand, font_size_px, display_mode, true);

        let rule_thickness = font_size_px * 0.06;
        let vertical_gap = font_size_px * 0.1;
        let extra_ascender = font_size_px * 0.05;

        // Layout the radical (√) surd symbol
        let surd_glyph = self.layout_glyph('√', MathVariant::Normal, font_size_px);
        let surd_width = surd_glyph.width;

        let overbar_y = radicand_node.ascent + vertical_gap + rule_thickness;
        let total_ascent = overbar_y + extra_ascender;

        let total_width = surd_width + radicand_node.width;

        LayoutNode {
            width: total_width,
            ascent: total_ascent,
            descent: radicand_node.descent,
            content: LayoutContent::HBox {
                children: vec![
                    // Surd symbol
                    PositionedNode {
                        x: 0.0,
                        y: 0.0,
                        node: surd_glyph,
                    },
                    // Overbar
                    PositionedNode {
                        x: surd_width,
                        y: overbar_y,
                        node: LayoutNode {
                            width: radicand_node.width,
                            ascent: rule_thickness / 2.0,
                            descent: rule_thickness / 2.0,
                            content: LayoutContent::Rule {
                                thickness: rule_thickness,
                            },
                        },
                    },
                    // Radicand
                    PositionedNode {
                        x: surd_width,
                        y: 0.0,
                        node: radicand_node,
                    },
                ],
            },
        }
    }

    fn layout_delimited(
        &self,
        open: Option<char>,
        close: Option<char>,
        content: &MathExpr,
        font_size_px: f32,
        display_mode: bool,
    ) -> LayoutNode {
        let content_node = self.layout_expr(content, font_size_px, display_mode, false);

        let mut children = Vec::new();
        let mut x = 0.0;

        if let Some(open_char) = open {
            let delim = self.layout_glyph(open_char, MathVariant::Normal, font_size_px);
            children.push(PositionedNode {
                x,
                y: 0.0,
                node: delim.clone(),
            });
            x += delim.width;
        }

        children.push(PositionedNode {
            x,
            y: 0.0,
            node: content_node.clone(),
        });
        x += content_node.width;

        if let Some(close_char) = close {
            let delim = self.layout_glyph(close_char, MathVariant::Normal, font_size_px);
            children.push(PositionedNode {
                x,
                y: 0.0,
                node: delim.clone(),
            });
            x += delim.width;
        }

        let max_ascent = children
            .iter()
            .map(|c| c.y + c.node.ascent)
            .fold(0.0_f32, f32::max);
        let max_descent = children
            .iter()
            .map(|c| (-c.y) + c.node.descent)
            .fold(0.0_f32, f32::max);

        LayoutNode {
            width: x,
            ascent: max_ascent,
            descent: max_descent,
            content: LayoutContent::HBox { children },
        }
    }

    fn layout_big_operator(
        &self,
        symbol: char,
        above: Option<&MathExpr>,
        below: Option<&MathExpr>,
        limits: bool,
        font_size_px: f32,
        display_mode: bool,
    ) -> LayoutNode {
        let op_node = self.layout_glyph(symbol, MathVariant::Normal, font_size_px);

        if display_mode && limits {
            let script_size = font_size_px * 0.7;
            let gap = font_size_px * 0.12;

            let above_node = above.map(|a| self.layout_expr(a, script_size, false, false));
            let below_node = below.map(|b| self.layout_expr(b, script_size, false, false));

            let mut max_width = op_node.width;
            if let Some(ref a) = above_node {
                max_width = max_width.max(a.width);
            }
            if let Some(ref b) = below_node {
                max_width = max_width.max(b.width);
            }

            let mut children = Vec::new();
            let mut total_ascent = op_node.ascent;
            let mut total_descent = op_node.descent;

            let op_x = (max_width - op_node.width) / 2.0;
            children.push(PositionedNode {
                x: op_x,
                y: 0.0,
                node: op_node.clone(),
            });

            if let Some(a) = above_node {
                let above_y = op_node.ascent + gap + a.descent;
                let above_x = (max_width - a.width) / 2.0;
                total_ascent = above_y + a.ascent;
                children.push(PositionedNode {
                    x: above_x,
                    y: above_y,
                    node: a,
                });
            }

            if let Some(b) = below_node {
                let below_y = -(op_node.descent + gap + b.ascent);
                let below_x = (max_width - b.width) / 2.0;
                total_descent = (-below_y) + b.descent;
                children.push(PositionedNode {
                    x: below_x,
                    y: below_y,
                    node: b,
                });
            }

            LayoutNode {
                width: max_width,
                ascent: total_ascent,
                descent: total_descent,
                content: LayoutContent::VBox { children },
            }
        } else {
            // Treat as scripts
            let base = MathExpr::Glyph {
                codepoint: symbol,
                variant: MathVariant::Normal,
            };
            self.layout_scripts(&base, above, below, font_size_px, display_mode)
        }
    }

    fn layout_text(&self, text: &str, font_size_px: f32) -> LayoutNode {
        let exprs: Vec<MathExpr> = text
            .chars()
            .map(|c| MathExpr::Glyph {
                codepoint: c,
                variant: MathVariant::Normal,
            })
            .collect();
        self.layout_hbox(&exprs, font_size_px, false)
    }

    fn layout_accent(
        &self,
        base: &MathExpr,
        accent_char: char,
        font_size_px: f32,
        display_mode: bool,
    ) -> LayoutNode {
        let base_node = self.layout_expr(base, font_size_px, display_mode, false);
        let accent_node = self.layout_glyph(accent_char, MathVariant::Normal, font_size_px);

        let accent_x = (base_node.width - accent_node.width) / 2.0;
        let accent_y = base_node.ascent + font_size_px * 0.05;

        let total_ascent = accent_y + accent_node.ascent;

        LayoutNode {
            width: base_node.width.max(accent_node.width),
            ascent: total_ascent,
            descent: base_node.descent,
            content: LayoutContent::VBox {
                children: vec![
                    PositionedNode {
                        x: accent_x.max(0.0),
                        y: accent_y,
                        node: accent_node,
                    },
                    PositionedNode {
                        x: 0.0,
                        y: 0.0,
                        node: base_node,
                    },
                ],
            },
        }
    }

    /// Render a LayoutNode tree to a pixmap.
    pub fn render_node(
        &self,
        pixmap: &mut Pixmap,
        node: &LayoutNode,
        x: f32,
        baseline_y: f32,
        fg_color: [u8; 4],
    ) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(fg_color[0], fg_color[1], fg_color[2], fg_color[3]);
        paint.anti_alias = true;

        self.render_node_inner(pixmap, node, x, baseline_y, &paint);
    }

    fn render_node_inner(
        &self,
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
                let font_ref = self.font.font_ref();
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
                        // Font coordinates: Y-up. Pixmap: Y-down.
                        // Apply Y-flip and translate to position.
                        let transform =
                            Transform::from_row(1.0, 0.0, 0.0, -1.0, x, baseline_y);
                        pixmap.fill_path(&path, paint, FillRule::Winding, transform, None);
                    }
                }
            }

            LayoutContent::HBox { children } | LayoutContent::VBox { children } => {
                for child in children {
                    self.render_node_inner(
                        pixmap,
                        &child.node,
                        x + child.x,
                        baseline_y - child.y, // child.y positive = up, convert to Y-down
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

            LayoutContent::Kern => { /* invisible spacing, nothing to draw */ }
        }
    }
}
