# `mathpng` — Pure-Rust LaTeX Math to PNG Renderer

## Project Goal

Build a Rust library crate (`mathpng`) that takes LaTeX math notation as input and produces a PNG image as output. The entire pipeline must be pure Rust — no C FFI, no JS engines, no system TeX installations.

## Pipeline Overview

```
LaTeX string ──► pulldown-latex ──► Layout Engine ──► tiny-skia Pixmap ──► PNG bytes
                   (parse)         (YOU BUILD THIS)    (rasterize)         (encode)
```

### Crate Dependencies

| Crate | Purpose | Role |
|---|---|---|
| `pulldown-latex` | Parse LaTeX math into event stream | Input parsing |
| `skrifa` (Google fontations) | Read OpenType fonts, MATH tables, glyph metrics | Font data |
| `read-fonts` (Google fontations) | Low-level font table access (dep of skrifa) | Font data |
| `tiny-skia` | Pure-Rust 2D rasterizer (Skia subset) | Rasterization |
| `png` | PNG encoding | Output |

**Font asset:** Bundle `STIXTwoMath-Regular.otf` (OFL license) as a compile-time asset via `include_bytes!`. This font has a complete OpenType MATH table. Latin Modern Math is an alternative. STIX Two is preferred for broader glyph coverage.

---

## Architecture

### Module Structure

```
mathpng/
├── Cargo.toml
├── fonts/
│   └── STIXTwoMath-Regular.otf          # bundled font asset (OFL license)
├── src/
│   ├── lib.rs                            # public API
│   ├── parse.rs                          # pulldown-latex event consumption
│   ├── math_expr.rs                      # intermediate representation (MathExpr tree)
│   ├── layout/
│   │   ├── mod.rs                        # layout engine entry point
│   │   ├── context.rs                    # LayoutContext (font metrics, MATH constants)
│   │   ├── node.rs                       # LayoutNode (positioned box tree)
│   │   ├── fraction.rs                   # fraction layout
│   │   ├── scripts.rs                    # superscript / subscript layout
│   │   ├── radical.rs                    # square root / nth root layout
│   │   ├── delimiters.rs                 # stretchy delimiters (parens, braces, brackets)
│   │   ├── matrix.rs                     # matrix / array layout
│   │   ├── accents.rs                    # hat, tilde, bar, vec, etc.
│   │   ├── operators.rs                  # big operators (sum, integral, product)
│   │   └── hbox.rs                       # horizontal box assembly
│   ├── font/
│   │   ├── mod.rs                        # font loading, glyph lookup
│   │   ├── math_table.rs                 # OpenType MATH table reader
│   │   └── glyph_variants.rs            # size variants & glyph assembly (stretchy)
│   ├── render/
│   │   ├── mod.rs                        # LayoutNode tree → tiny-skia draw calls
│   │   └── path.rs                       # glyph outline → tiny-skia Path conversion
│   └── png_encode.rs                     # Pixmap → PNG bytes
└── tests/
    ├── snapshots/                        # reference PNG images for regression
    ├── test_fractions.rs
    ├── test_scripts.rs
    ├── test_radicals.rs
    ├── test_delimiters.rs
    └── test_integration.rs
```

---

## Public API

```rust
/// Render options
pub struct RenderOptions {
    /// Font size in points (default: 20.0)
    pub font_size_pt: f32,
    /// Pixels per point (default: 2.0 for 144 DPI, i.e. 2x retina)
    pub scale: f32,
    /// RGBA foreground color (default: black [0, 0, 0, 255])
    pub fg_color: [u8; 4],
    /// RGBA background color (default: transparent [255, 255, 255, 0])
    pub bg_color: [u8; 4],
    /// Padding in pixels around the rendered expression
    pub padding: u32,
    /// Display mode (true) vs inline mode (false). Display mode centers
    /// and uses larger operators/limits positioning.
    pub display_mode: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            font_size_pt: 20.0,
            scale: 2.0,
            fg_color: [0, 0, 0, 255],
            bg_color: [255, 255, 255, 0],
            padding: 8,
            display_mode: true,
        }
    }
}

/// Render a LaTeX math string to PNG bytes.
///
/// Input should be the LaTeX body WITHOUT surrounding `$` or `$$` delimiters.
///
/// # Example
/// ```
/// let png_bytes = mathpng::render_to_png(r"\frac{1}{2} + \sqrt{x^2 + y^2}", None)?;
/// std::fs::write("equation.png", &png_bytes)?;
/// ```
pub fn render_to_png(latex: &str, options: Option<RenderOptions>) -> Result<Vec<u8>, MathRenderError>;

/// Render to a tiny_skia::Pixmap (useful if caller wants to composite further).
pub fn render_to_pixmap(latex: &str, options: Option<RenderOptions>) -> Result<tiny_skia::Pixmap, MathRenderError>;

/// Error type
#[derive(Debug, thiserror::Error)]
pub enum MathRenderError {
    #[error("LaTeX parse error: {0}")]
    Parse(String),
    #[error("Layout error: {0}")]
    Layout(String),
    #[error("Font error: {0}")]
    Font(String),
    #[error("Render error: {0}")]
    Render(String),
}
```

---

## Phase 1: Intermediate Representation (`math_expr.rs`)

Convert the `pulldown-latex` event stream into a tree. The events from `pulldown-latex` are modeled after pulldown-cmark: `Event::Begin(GroupType)`, `Event::End`, `Event::Content(Content)`, etc.

```rust
/// The intermediate representation — a tree of math constructs.
/// This sits between parsing and layout.
#[derive(Debug, Clone)]
pub enum MathExpr {
    /// A sequence of expressions laid out horizontally
    Row(Vec<MathExpr>),

    /// A single glyph (letter, digit, operator symbol)
    Glyph {
        codepoint: char,
        /// Math variant: normal, italic, bold, bold-italic, script, fraktur, etc.
        variant: MathVariant,
    },

    /// Fraction: numerator / denominator
    Fraction {
        numerator: Box<MathExpr>,
        denominator: Box<MathExpr>,
        /// Line thickness override (None = default from font MATH table)
        line_thickness: Option<f32>,
    },

    /// Superscript and/or subscript
    Scripts {
        base: Box<MathExpr>,
        superscript: Option<Box<MathExpr>>,
        subscript: Option<Box<MathExpr>>,
    },

    /// Square root or nth root
    Radical {
        radicand: Box<MathExpr>,
        index: Option<Box<MathExpr>>,  // nth root index
    },

    /// Stretchy delimiter pair: \left( ... \right)
    Delimited {
        open: Option<char>,    // None for \left.
        close: Option<char>,   // None for \right.
        content: Box<MathExpr>,
    },

    /// Big operator: \sum, \prod, \int, etc.
    BigOperator {
        symbol: char,
        above: Option<Box<MathExpr>>,  // limits above (display) or superscript (inline)
        below: Option<Box<MathExpr>>,  // limits below (display) or subscript (inline)
        limits: bool,                   // true if limits positioning (above/below)
    },

    /// Matrix / array
    Matrix {
        rows: Vec<Vec<MathExpr>>,
        delimiters: Option<(char, char)>,  // e.g., ('[', ']') for bmatrix
    },

    /// Accent over expression: \hat, \tilde, \vec, \bar, \dot, \ddot
    Accent {
        base: Box<MathExpr>,
        accent_char: char,   // the combining accent (e.g., U+0302 for hat)
    },

    /// Explicit space: \quad, \,, \;, \! etc.
    Space(f32),  // in em units

    /// Text mode: \text{...}
    Text(String),

    /// Generic grouping (from {})
    Group(Vec<MathExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MathVariant {
    Normal,
    Italic,       // default for single latin letters
    Bold,
    BoldItalic,
    Script,       // \mathcal
    BoldScript,
    Fraktur,
    BoldFraktur,
    DoubleStruck, // \mathbb
    SansSerif,
    SansSerifBold,
    SansSerifItalic,
    Monospace,
}
```

### Mapping `pulldown-latex` Events to `MathExpr`

Build a recursive descent consumer over the event stream. Key mappings:

| pulldown-latex event | MathExpr |
|---|---|
| `Begin(Fraction)` ... `End(Fraction)` | `Fraction { numerator, denominator }` |
| `Begin(Superscript)` / `Begin(Subscript)` | Attach to preceding expr as `Scripts` |
| `Begin(Sqrt)` | `Radical` |
| `Begin(Left)` ... `End(Right)` | `Delimited` |
| `Content(Glyph { .. })` | `Glyph` |
| `Content(Operator { .. })` | `Glyph` or `BigOperator` depending on char |
| `Begin(Array)` | `Matrix` |
| `Content(Space { .. })` | `Space` |

Use a stack-based builder: push frames on `Begin`, pop and assemble on `End`.

---

## Phase 2: Font Module (`font/`)

### Loading the Font

```rust
use skrifa::{FontRef, MetadataProvider, raw::TableProvider};

pub struct MathFont {
    font_data: &'static [u8],  // from include_bytes!
    font: FontRef<'static>,
    math_constants: MathConstants,
    units_per_em: f32,
}

// Embed the font at compile time
const FONT_DATA: &[u8] = include_bytes!("../fonts/STIXTwoMath-Regular.otf");
```

### OpenType MATH Table Constants

The MATH table's `MathConstants` subtable provides ~60 values. The critical ones for layout:

```rust
pub struct MathConstants {
    // -- General --
    pub axis_height: f32,                        // height of the math axis (center of fraction bar, minus sign, etc.)

    // -- Fractions --
    pub fraction_numerator_shift_up: f32,        // display style
    pub fraction_numerator_display_style_shift_up: f32,
    pub fraction_denominator_shift_down: f32,
    pub fraction_denominator_display_style_shift_down: f32,
    pub fraction_rule_thickness: f32,
    pub fraction_num_display_style_gap_min: f32,
    pub fraction_denom_display_style_gap_min: f32,

    // -- Scripts --
    pub superscript_shift_up: f32,
    pub superscript_shift_up_cramped: f32,
    pub subscript_shift_down: f32,
    pub sub_superscript_gap_min: f32,
    pub superscript_bottom_min: f32,
    pub subscript_top_max: f32,
    pub script_percent_scale_down: f32,          // e.g., 70%
    pub script_script_percent_scale_down: f32,   // e.g., 50%

    // -- Radicals --
    pub radical_vertical_gap: f32,
    pub radical_display_style_vertical_gap: f32,
    pub radical_rule_thickness: f32,
    pub radical_extra_ascender: f32,
    pub radical_kern_before_degree: f32,
    pub radical_kern_after_degree: f32,
    pub radical_degree_bottom_raise_percent: f32,

    // -- Limits / Big Operators --
    pub upper_limit_gap_min: f32,
    pub upper_limit_baseline_rise_min: f32,
    pub lower_limit_gap_min: f32,
    pub lower_limit_baseline_drop_min: f32,

    // -- Stretchy --
    pub stretch_stack_top_shift_up: f32,
    pub stretch_stack_bottom_shift_down: f32,
    pub stretch_stack_gap_min: f32,

    // -- Accents --
    pub accent_base_height: f32,                 // if base is shorter than this, don't shift accent
    pub flattened_accent_base_height: f32,

    // -- Delimiters --
    pub delimited_sub_formula_min_height: f32,
}
```

**Reading these from skrifa:**

```rust
use skrifa::raw::tables::math::Math;

fn read_math_constants(font: &FontRef) -> MathConstants {
    let math_table = font.math().expect("Font has no MATH table");
    let constants = math_table.math_constants().expect("No MathConstants");

    MathConstants {
        axis_height: constants.axis_height().value as f32,
        fraction_rule_thickness: constants.fraction_rule_thickness().value as f32,
        superscript_shift_up: constants.superscript_shift_up().value as f32,
        // ... read all constants similarly
        // Values are in font design units; divide by units_per_em and multiply
        // by font_size_px at layout time.
    }
}
```

### Glyph Metrics

For each glyph you need:
- **Advance width** — how far to move the cursor after placing this glyph
- **Bounding box** (xMin, yMin, xMax, yMax) — for computing content extents
- **Italic correction** — extra space to add when a superscript follows an italic glyph

```rust
pub struct GlyphMetrics {
    pub glyph_id: u16,
    pub advance_width: f32,       // in font units
    pub lsb: f32,                 // left side bearing
    pub bbox: (f32, f32, f32, f32), // (x_min, y_min, x_max, y_max)
    pub italic_correction: f32,    // from MATH table MathGlyphInfo
}
```

### Glyph Variants and Assembly (for Stretchy Constructs)

The MATH table provides two mechanisms for stretchy glyphs:

1. **Size variants**: A list of progressively larger pre-drawn glyphs (e.g., 5 sizes of parenthesis)
2. **Glyph assembly**: Instructions to build an arbitrarily tall/wide glyph from parts (top, bottom, middle, repeater)

```rust
/// Get the best size variant for a delimiter at a target height.
pub fn get_size_variant(
    font: &MathFont,
    codepoint: char,
    target_height: f32,  // in font units
    vertical: bool,      // true for vertical stretching
) -> StretchResult {
    // 1. Look up MathVariants table for this glyph
    // 2. Iterate size variants, pick smallest that exceeds target_height
    // 3. If none large enough, fall back to glyph assembly
}

pub enum StretchResult {
    /// A single pre-drawn glyph variant
    Variant(u16),  // glyph_id
    /// Assembly instructions for building the stretchy glyph
    Assembly(GlyphAssembly),
}

pub struct GlyphAssembly {
    pub parts: Vec<AssemblyPart>,
    pub italic_correction: f32,
}

pub struct AssemblyPart {
    pub glyph_id: u16,
    pub start_connector_length: f32,
    pub end_connector_length: f32,
    pub full_advance: f32,
    pub is_extender: bool,  // true = this part repeats to fill space
}
```

### Math Variant Glyph Mapping

For `\mathbb`, `\mathcal`, etc., map the ASCII codepoint to the Unicode Mathematical Alphanumeric Symbols block:

```rust
fn map_variant(c: char, variant: MathVariant) -> char {
    match variant {
        MathVariant::Italic => {
            if c.is_ascii_lowercase() {
                // U+1D44E offset for math italic lowercase
                // Special case: 'h' maps to U+210E (Planck constant)
                char::from_u32(0x1D44E + (c as u32 - 'a' as u32)).unwrap()
            } else if c.is_ascii_uppercase() {
                char::from_u32(0x1D434 + (c as u32 - 'A' as u32)).unwrap()
            } else { c }
        }
        MathVariant::Bold => { /* U+1D400 block */ }
        MathVariant::DoubleStruck => { /* U+1D538 block, with exceptions for C,H,N,P,Q,R,Z */ }
        MathVariant::Script => { /* U+1D49C block, with exceptions */ }
        MathVariant::Fraktur => { /* U+1D504 block */ }
        // ... etc
        _ => c,
    }
}
```

---

## Phase 3: Layout Engine (`layout/`)

### Core Data Types

```rust
/// A positioned, measured box in the layout tree.
/// All measurements in pixels (font units × font_size_px / units_per_em).
#[derive(Debug, Clone)]
pub struct LayoutNode {
    /// Width of this box
    pub width: f32,
    /// Height above the baseline
    pub ascent: f32,
    /// Depth below the baseline
    pub descent: f32,
    /// Content of this box
    pub content: LayoutContent,
}

impl LayoutNode {
    pub fn height(&self) -> f32 { self.ascent + self.descent }
}

#[derive(Debug, Clone)]
pub enum LayoutContent {
    /// A single glyph positioned at the origin of this box
    Glyph {
        glyph_id: u16,
        font_size_px: f32,
    },

    /// A horizontal list of positioned children
    HBox {
        children: Vec<PositionedNode>,
    },

    /// A vertical list of positioned children (for fractions, limits)
    VBox {
        children: Vec<PositionedNode>,
    },

    /// A horizontal rule (fraction bar, radical overbar)
    Rule {
        thickness: f32,
    },

    /// Kern (invisible spacing)
    Kern,

    /// Glyph assembly (stretchy delimiter built from parts)
    Assembly {
        parts: Vec<(u16, f32, f32)>, // (glyph_id, x_offset, y_offset) per part
        font_size_px: f32,
    },
}

/// A child node with its position relative to the parent box origin.
/// The origin of a box is its left edge at the baseline.
#[derive(Debug, Clone)]
pub struct PositionedNode {
    pub x: f32,
    pub y: f32,  // positive = up from baseline, negative = down
    pub node: LayoutNode,
}
```

### Layout Context

```rust
pub struct LayoutContext<'a> {
    pub font: &'a MathFont,
    pub font_size_px: f32,
    pub display_mode: bool,
    pub cramped: bool,        // true inside denominators, under radicals, etc.
    pub script_level: u8,     // 0 = normal, 1 = script, 2 = scriptscript
}

impl<'a> LayoutContext<'a> {
    /// Get a font-unit value scaled to pixels at current font size.
    pub fn scaled(&self, font_units: f32) -> f32 {
        font_units * self.font_size_px / self.font.units_per_em
    }

    /// Derive a context for script-level content (superscripts, subscripts).
    pub fn for_script(&self) -> LayoutContext<'a> {
        let scale = if self.script_level == 0 {
            self.font.math_constants.script_percent_scale_down / 100.0
        } else {
            self.font.math_constants.script_script_percent_scale_down / 100.0
        };
        LayoutContext {
            font_size_px: self.font_size_px * scale,
            script_level: (self.script_level + 1).min(2),
            cramped: self.cramped,
            display_mode: false,
            ..*self
        }
    }

    /// Derive a cramped context (used in denominators, under radicals).
    pub fn cramped(&self) -> LayoutContext<'a> {
        LayoutContext {
            cramped: true,
            ..*self
        }
    }
}
```

### Layout Algorithm — Main Dispatch

```rust
/// Top-level layout function: MathExpr → LayoutNode
pub fn layout(expr: &MathExpr, ctx: &LayoutContext) -> LayoutNode {
    match expr {
        MathExpr::Row(children) => layout_hbox(children, ctx),
        MathExpr::Glyph { codepoint, variant } => layout_glyph(*codepoint, *variant, ctx),
        MathExpr::Fraction { numerator, denominator, line_thickness } =>
            layout_fraction(numerator, denominator, *line_thickness, ctx),
        MathExpr::Scripts { base, superscript, subscript } =>
            layout_scripts(base, superscript.as_deref(), subscript.as_deref(), ctx),
        MathExpr::Radical { radicand, index } =>
            layout_radical(radicand, index.as_deref(), ctx),
        MathExpr::Delimited { open, close, content } =>
            layout_delimited(*open, *close, content, ctx),
        MathExpr::BigOperator { symbol, above, below, limits } =>
            layout_big_operator(*symbol, above.as_deref(), below.as_deref(), *limits, ctx),
        MathExpr::Matrix { rows, delimiters } =>
            layout_matrix(rows, *delimiters, ctx),
        MathExpr::Accent { base, accent_char } =>
            layout_accent(base, *accent_char, ctx),
        MathExpr::Space(em) => layout_space(*em, ctx),
        MathExpr::Text(s) => layout_text(s, ctx),
        MathExpr::Group(children) => layout_hbox(children, ctx),
    }
}
```

### Layout Algorithm — Fractions (Detailed Example)

This is the most illustrative layout function. All others follow the same pattern of reading MATH table constants and positioning children.

```rust
pub fn layout_fraction(
    numerator: &MathExpr,
    denominator: &MathExpr,
    line_thickness: Option<f32>,
    ctx: &LayoutContext,
) -> LayoutNode {
    let mc = &ctx.font.math_constants;

    // 1. Layout children with appropriate style changes
    let num_node = layout(numerator, &ctx.cramped());  // numerator is NOT cramped but let's simplify
    let den_node = layout(denominator, &ctx.cramped()); // denominator IS cramped

    // 2. Read MATH constants (scaled to pixels)
    let rule_thickness = line_thickness
        .unwrap_or_else(|| ctx.scaled(mc.fraction_rule_thickness));
    let axis = ctx.scaled(mc.axis_height);

    let (num_shift, den_shift, min_num_gap, min_den_gap) = if ctx.display_mode {
        (
            ctx.scaled(mc.fraction_numerator_display_style_shift_up),
            ctx.scaled(mc.fraction_denominator_display_style_shift_down),
            ctx.scaled(mc.fraction_num_display_style_gap_min),
            ctx.scaled(mc.fraction_denom_display_style_gap_min),
        )
    } else {
        (
            ctx.scaled(mc.fraction_numerator_shift_up),
            ctx.scaled(mc.fraction_denominator_shift_down),
            rule_thickness, // use rule thickness as min gap in inline mode
            rule_thickness,
        )
    };

    // 3. Compute vertical positions
    // The fraction bar sits at the math axis.
    // num_shift is from baseline to baseline of numerator.
    // den_shift is from baseline to baseline of denominator.
    let bar_y = axis; // center of the bar

    // Numerator baseline: above the bar
    let mut num_baseline = bar_y + rule_thickness / 2.0 + min_num_gap + num_node.descent;
    num_baseline = num_baseline.max(num_shift);

    // Denominator baseline: below the bar
    let mut den_baseline = bar_y - rule_thickness / 2.0 - min_den_gap - den_node.ascent;
    den_baseline = den_baseline.min(-den_shift); // den_baseline is negative (below baseline)

    // 4. Compute horizontal centering
    let width = num_node.width.max(den_node.width);
    let num_x = (width - num_node.width) / 2.0;
    let den_x = (width - den_node.width) / 2.0;

    // 5. Assemble VBox
    let ascent = num_baseline + num_node.ascent;
    let descent = -(den_baseline - den_node.descent); // make positive

    LayoutNode {
        width,
        ascent,
        descent,
        content: LayoutContent::VBox {
            children: vec![
                // Numerator
                PositionedNode { x: num_x, y: num_baseline, node: num_node },
                // Fraction bar
                PositionedNode {
                    x: 0.0,
                    y: bar_y,
                    node: LayoutNode {
                        width,
                        ascent: rule_thickness / 2.0,
                        descent: rule_thickness / 2.0,
                        content: LayoutContent::Rule { thickness: rule_thickness },
                    },
                },
                // Denominator
                PositionedNode { x: den_x, y: den_baseline, node: den_node },
            ],
        },
    }
}
```

### Layout Algorithm — Scripts (Superscript / Subscript)

```rust
pub fn layout_scripts(
    base: &MathExpr,
    sup: Option<&MathExpr>,
    sub: Option<&MathExpr>,
    ctx: &LayoutContext,
) -> LayoutNode {
    let mc = &ctx.font.math_constants;
    let base_node = layout(base, ctx);
    let script_ctx = ctx.for_script();

    let sup_node = sup.map(|s| layout(s, &script_ctx));
    let sub_node = sub.map(|s| layout(s, &script_ctx));

    // Get italic correction from base (for superscript horizontal shift)
    let italic_correction = get_italic_correction(base, ctx);

    let mut children = vec![
        PositionedNode { x: 0.0, y: 0.0, node: base_node.clone() },
    ];
    let mut total_width = base_node.width;
    let mut total_ascent = base_node.ascent;
    let mut total_descent = base_node.descent;

    if let Some(sup_n) = sup_node {
        let shift_up = ctx.scaled(if ctx.cramped {
            mc.superscript_shift_up_cramped
        } else {
            mc.superscript_shift_up
        });
        // Ensure superscript bottom doesn't drop below superscript_bottom_min
        let sup_bottom_min = ctx.scaled(mc.superscript_bottom_min);
        let sup_y = shift_up.max(base_node.ascent - sup_n.descent)
            .max(sup_bottom_min + sup_n.descent);

        let sup_x = base_node.width + italic_correction;

        children.push(PositionedNode { x: sup_x, y: sup_y, node: sup_n.clone() });
        total_width = total_width.max(sup_x + sup_n.width);
        total_ascent = total_ascent.max(sup_y + sup_n.ascent);
    }

    if let Some(sub_n) = sub_node {
        let shift_down = ctx.scaled(mc.subscript_shift_down);
        let sub_top_max = ctx.scaled(mc.subscript_top_max);
        let sub_y = -(shift_down.max(base_node.descent + sub_n.ascent)
            .max(sub_n.ascent - sub_top_max));

        let sub_x = base_node.width;

        // If both sup and sub, enforce minimum gap
        // (adjust sub_y if needed so gap >= sub_superscript_gap_min)

        children.push(PositionedNode { x: sub_x, y: sub_y, node: sub_n.clone() });
        total_width = total_width.max(sub_x + sub_n.width);
        total_descent = total_descent.max(-(sub_y) + sub_n.descent);
    }

    LayoutNode {
        width: total_width,
        ascent: total_ascent,
        descent: total_descent,
        content: LayoutContent::HBox { children },
    }
}
```

### Layout Algorithm — Radicals

```rust
pub fn layout_radical(
    radicand: &MathExpr,
    index: Option<&MathExpr>,
    ctx: &LayoutContext,
) -> LayoutNode {
    let mc = &ctx.font.math_constants;
    let radicand_node = layout(radicand, &ctx.cramped());

    let rule_thickness = ctx.scaled(mc.radical_rule_thickness);
    let vertical_gap = ctx.scaled(if ctx.display_mode {
        mc.radical_display_style_vertical_gap
    } else {
        mc.radical_vertical_gap
    });
    let extra_ascender = ctx.scaled(mc.radical_extra_ascender);

    // Total height the radical surd must cover
    let content_height = radicand_node.ascent + radicand_node.descent;
    let clearance = content_height + vertical_gap + rule_thickness + extra_ascender;

    // Get the radical surd glyph (stretchy) to match this height
    let surd = get_size_variant(ctx.font, '√', clearance, true);
    let surd_width = get_surd_advance(ctx.font, &surd);

    // The overbar extends across the radicand
    let overbar_y = radicand_node.ascent + vertical_gap + rule_thickness;

    // If there's an index (nth root), position it
    let (index_node, index_x, index_y, index_width) = if let Some(idx) = index {
        let ss_ctx = ctx.for_script().for_script(); // scriptscript level
        let idx_node = layout(idx, &ss_ctx);
        let kern_before = ctx.scaled(mc.radical_kern_before_degree);
        let kern_after = ctx.scaled(mc.radical_kern_after_degree);
        let raise = mc.radical_degree_bottom_raise_percent / 100.0;
        let idx_y = raise * clearance; // raise the index
        (Some(idx_node.clone()), kern_before, idx_y, idx_node.width + kern_after)
    } else {
        (None, 0.0, 0.0, 0.0)
    };

    let total_width = index_width + surd_width + radicand_node.width;

    // ... assemble: index + surd glyph + overbar + radicand into an HBox/VBox combo
    todo!("Assemble positioned nodes")
}
```

### Layout Algorithm — Stretchy Delimiters

```rust
pub fn layout_delimited(
    open: Option<char>,
    close: Option<char>,
    content: &MathExpr,
    ctx: &LayoutContext,
) -> LayoutNode {
    let content_node = layout(content, ctx);
    let mc = &ctx.font.math_constants;

    // Target height for delimiters: at least as tall as content
    let target_height = (content_node.ascent + content_node.descent)
        .max(ctx.scaled(mc.delimited_sub_formula_min_height));

    let mut children = Vec::new();
    let mut x = 0.0;

    // Open delimiter
    if let Some(open_char) = open {
        let delim_node = make_stretchy_delimiter(ctx, open_char, target_height);
        children.push(PositionedNode { x, y: 0.0, node: delim_node.clone() });
        x += delim_node.width;
    }

    // Content
    children.push(PositionedNode { x, y: 0.0, node: content_node.clone() });
    x += content_node.width;

    // Close delimiter
    if let Some(close_char) = close {
        let delim_node = make_stretchy_delimiter(ctx, close_char, target_height);
        children.push(PositionedNode { x, y: 0.0, node: delim_node.clone() });
        x += delim_node.width;
    }

    LayoutNode {
        width: x,
        ascent: children.iter().map(|c| c.y + c.node.ascent).fold(0.0_f32, f32::max),
        descent: children.iter().map(|c| -(c.y) + c.node.descent).fold(0.0_f32, f32::max),
        content: LayoutContent::HBox { children },
    }
}
```

### Layout Algorithm — Big Operators

```rust
pub fn layout_big_operator(
    symbol: char,
    above: Option<&MathExpr>,
    below: Option<&MathExpr>,
    limits: bool,
    ctx: &LayoutContext,
) -> LayoutNode {
    // In display mode with limits=true, place above/below centered over/under the operator
    // In inline mode or limits=false, treat as scripts (superscript/subscript)

    if ctx.display_mode && limits {
        // Get a display-size variant of the operator
        let op_node = layout_glyph_display_variant(symbol, ctx);

        let mc = &ctx.font.math_constants;
        let mut children = vec![];
        let mut total_ascent = op_node.ascent;
        let mut total_descent = op_node.descent;
        let mut max_width = op_node.width;

        let above_node = above.map(|a| layout(a, &ctx.for_script()));
        let below_node = below.map(|b| layout(b, &ctx.for_script()));

        if let Some(ref a) = above_node { max_width = max_width.max(a.width); }
        if let Some(ref b) = below_node { max_width = max_width.max(b.width); }

        // Center everything horizontally, stack vertically with gaps from MATH table
        // ... position above_node above op_node with upper_limit_gap_min
        // ... position below_node below op_node with lower_limit_gap_min

        todo!("Assemble VBox with centered children")
    } else {
        // Treat as scripts
        let base = MathExpr::Glyph { codepoint: symbol, variant: MathVariant::Normal };
        layout_scripts(&base, above.map(|a| a), below.map(|b| b), ctx)
    }
}
```

### Layout Algorithm — HBox (Horizontal List)

```rust
pub fn layout_hbox(exprs: &[MathExpr], ctx: &LayoutContext) -> LayoutNode {
    let mut children = Vec::new();
    let mut x = 0.0;
    let mut max_ascent: f32 = 0.0;
    let mut max_descent: f32 = 0.0;

    for expr in exprs {
        let node = layout(expr, ctx);
        max_ascent = max_ascent.max(node.ascent);
        max_descent = max_descent.max(node.descent);
        children.push(PositionedNode { x, y: 0.0, node: node.clone() });
        x += node.width;
    }

    LayoutNode {
        width: x,
        ascent: max_ascent,
        descent: max_descent,
        content: LayoutContent::HBox { children },
    }
}
```

---

## Phase 4: Rendering (`render/`)

### Glyph Outline → tiny-skia Path

```rust
use tiny_skia::{Pixmap, Paint, Transform, PathBuilder, FillRule};
use skrifa::outline::{DrawSettings, OutlinePen};

/// A pen that converts glyph outlines to tiny-skia PathBuilder commands.
struct SkiaPen {
    builder: PathBuilder,
    scale: f32,
    x_offset: f32,
    y_offset: f32,
}

impl OutlinePen for SkiaPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(
            x * self.scale + self.x_offset,
            -y * self.scale + self.y_offset,  // flip Y: font coords are Y-up, skia is Y-down
        );
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(
            x * self.scale + self.x_offset,
            -y * self.scale + self.y_offset,
        );
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.builder.quad_to(
            cx * self.scale + self.x_offset,
            -cy * self.scale + self.y_offset,
            x * self.scale + self.x_offset,
            -y * self.scale + self.y_offset,
        );
    }

    fn curve_to(&mut self, cx1: f32, cy1: f32, cx2: f32, cy2: f32, x: f32, y: f32) {
        self.builder.cubic_to(
            cx1 * self.scale + self.x_offset,
            -cy1 * self.scale + self.y_offset,
            cx2 * self.scale + self.x_offset,
            -cy2 * self.scale + self.y_offset,
            x * self.scale + self.x_offset,
            -y * self.scale + self.y_offset,
        );
    }

    fn close(&mut self) {
        self.builder.close();
    }
}
```

### Tree Walking Renderer

```rust
pub fn render_layout(
    pixmap: &mut Pixmap,
    node: &LayoutNode,
    x: f32,           // absolute x position
    baseline_y: f32,   // absolute y position of the baseline (in pixel coords, Y-down)
    font: &MathFont,
    paint: &Paint,
) {
    match &node.content {
        LayoutContent::Glyph { glyph_id, font_size_px } => {
            let scale = font_size_px / font.units_per_em;
            // Draw glyph outline at (x, baseline_y)
            let path = build_glyph_path(font, *glyph_id, scale, x, baseline_y);
            if let Some(path) = path {
                pixmap.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
            }
        }

        LayoutContent::HBox { children } | LayoutContent::VBox { children } => {
            for child in children {
                render_layout(
                    pixmap,
                    &child.node,
                    x + child.x,
                    baseline_y - child.y,  // child.y is positive-up, convert to Y-down
                    font,
                    paint,
                );
            }
        }

        LayoutContent::Rule { thickness } => {
            // Draw a filled rectangle for fraction bars, radical overbars
            let rect = tiny_skia::Rect::from_xywh(x, baseline_y - thickness / 2.0, node.width, *thickness);
            if let Some(rect) = rect {
                pixmap.fill_rect(rect, paint, Transform::identity(), None);
            }
        }

        LayoutContent::Assembly { parts, font_size_px } => {
            let scale = font_size_px / font.units_per_em;
            for (glyph_id, gx, gy) in parts {
                let path = build_glyph_path(font, *glyph_id, scale, x + gx, baseline_y - gy);
                if let Some(path) = path {
                    pixmap.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
                }
            }
        }

        LayoutContent::Kern => { /* invisible, nothing to draw */ }
    }
}
```

### Top-Level Render Function

```rust
pub fn render_to_png(latex: &str, options: Option<RenderOptions>) -> Result<Vec<u8>, MathRenderError> {
    let opts = options.unwrap_or_default();

    // 1. Parse
    let expr = parse::parse_latex(latex)?;

    // 2. Layout
    let font = MathFont::load()?;
    let ctx = LayoutContext {
        font: &font,
        font_size_px: opts.font_size_pt * opts.scale,  // scale up for retina
        display_mode: opts.display_mode,
        cramped: false,
        script_level: 0,
    };
    let layout_tree = layout::layout(&expr, &ctx);

    // 3. Create pixmap
    let img_width = (layout_tree.width + 2.0 * opts.padding as f32).ceil() as u32;
    let img_height = (layout_tree.height() + 2.0 * opts.padding as f32).ceil() as u32;
    let mut pixmap = Pixmap::new(img_width, img_height)
        .ok_or_else(|| MathRenderError::Render("Failed to create pixmap".into()))?;

    // Fill background
    let bg = tiny_skia::Color::from_rgba8(opts.bg_color[0], opts.bg_color[1], opts.bg_color[2], opts.bg_color[3]);
    pixmap.fill(bg);

    // 4. Render
    let mut paint = Paint::default();
    paint.set_color_rgba8(opts.fg_color[0], opts.fg_color[1], opts.fg_color[2], opts.fg_color[3]);
    paint.anti_alias = true;

    let origin_x = opts.padding as f32;
    let origin_y = opts.padding as f32 + layout_tree.ascent;  // baseline position

    render::render_layout(&mut pixmap, &layout_tree, origin_x, origin_y, &font, &paint);

    // 5. Encode PNG
    Ok(pixmap.encode_png().map_err(|e| MathRenderError::Render(e.to_string()))?)
}
```

---

## Phase 5: Testing Strategy

### Snapshot Testing

Use `insta` or a simple custom harness:

1. Render known expressions to PNG
2. Compare against checked-in reference images (pixel diff with tolerance)
3. Key test cases:
   - Simple glyph: `x`
   - Fraction: `\frac{a}{b}`
   - Nested fraction: `\frac{1}{1 + \frac{1}{x}}`
   - Scripts: `x^2`, `x_i`, `x_i^2`
   - Radical: `\sqrt{x}`, `\sqrt[3]{x}`
   - Delimiters: `\left(\frac{a}{b}\right)`
   - Big operators: `\sum_{i=0}^{n} i^2`
   - Integral: `\int_0^\infty e^{-x^2} dx`
   - Matrix: `\begin{pmatrix} a & b \\ c & d \end{pmatrix}`
   - Complex: `\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}` (quadratic formula)
   - Greek: `\alpha + \beta = \gamma`
   - Accents: `\hat{x}`, `\vec{v}`, `\bar{z}`

### Unit Tests

- `parse.rs`: Verify MathExpr tree structure for known inputs
- `font/math_table.rs`: Verify MATH constant values against known values for STIX Two
- `layout/fraction.rs`: Verify bounding box dimensions for simple fractions
- Each layout module gets targeted tests

---

## Implementation Order

**Milestone 1 — Vertical slice (single glyph rendering):**
- [ ] Font loading with skrifa, reading basic glyph metrics
- [ ] Glyph outline → tiny-skia path
- [ ] Render a single character to PNG
- **Deliverable:** `render_to_png("x", None)` produces a PNG of italic x

**Milestone 2 — Horizontal layout:**
- [ ] pulldown-latex integration, parse simple expressions
- [ ] MathExpr IR, Row/Glyph variants only
- [ ] HBox layout
- [ ] Math variant mapping (italic for variables, normal for operators)
- **Deliverable:** `render_to_png("x + y = z", None)` works

**Milestone 3 — Fractions and scripts:**
- [ ] Read MATH table constants
- [ ] Fraction layout
- [ ] Superscript/subscript layout with script scaling
- **Deliverable:** `render_to_png(r"\frac{x^2}{y_i}", None)` works

**Milestone 4 — Radicals and delimiters:**
- [ ] Glyph size variants from MATH table
- [ ] Glyph assembly (stretchy construction)
- [ ] Radical layout
- [ ] Stretchy delimiter layout
- **Deliverable:** `render_to_png(r"\sqrt{\frac{a}{b}}", None)` works

**Milestone 5 — Big operators and limits:**
- [ ] Display-size operator variants
- [ ] Limits positioning (above/below)
- [ ] Integral, sum, product
- **Deliverable:** `render_to_png(r"\sum_{i=0}^{n} x_i", None)` works

**Milestone 6 — Matrices and accents:**
- [ ] Matrix/array layout with column alignment
- [ ] Accent positioning
- **Deliverable:** Full quadratic formula, matrices, accented variables

**Milestone 7 — Polish:**
- [ ] Snapshot test suite
- [ ] Edge cases (empty groups, deeply nested expressions)
- [ ] Error handling and graceful fallbacks for unsupported commands
- [ ] Documentation

---

## Key References

- **OpenType MATH table specification:** https://learn.microsoft.com/en-us/typography/opentype/spec/math
- **MathML Core spec:** https://www.w3.org/TR/mathml-core/ (layout algorithm descriptions are useful even if we're not producing MathML)
- **TeX by Topic (Victor Eijkhout):** Free PDF, chapters 11-12 cover math typesetting rules
- **The TeXbook (Knuth):** Appendix G has the definitive math layout algorithm
- **KaTeX source code:** Good reference for which MATH table constants to use where
- **pulldown-latex docs:** https://docs.rs/pulldown-latex
- **skrifa / fontations docs:** https://docs.rs/skrifa
- **tiny-skia docs:** https://docs.rs/tiny-skia
- **STIX Two Math font:** https://github.com/stipub/stixfonts (OFL license)

---

## Cargo.toml

```toml
[package]
name = "mathpng"
version = "0.1.0"
edition = "2021"
description = "Pure-Rust LaTeX math to PNG renderer"
license = "MIT OR Apache-2.0"

[dependencies]
pulldown-latex = "0.6"         # LaTeX parser (check latest version)
skrifa = "0.26"                # OpenType font reading (check latest version)
read-fonts = "0.24"            # low-level font tables (check latest version)
tiny-skia = "0.11"             # 2D rasterizer
png = "0.17"                   # PNG encoding (note: tiny-skia has encode_png() built in)
thiserror = "2"                # error derive

[dev-dependencies]
insta = "1"                    # snapshot testing
```

**Note:** Pin actual versions after checking crates.io. The `skrifa` / `read-fonts` APIs evolve; check `fontations` GitHub for current MATH table access patterns.
