mod font;
mod math_expr;
mod parse;
mod render;

use render::Renderer;

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
    /// Display mode (true) vs inline mode (false)
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

/// Render a LaTeX math string to PNG bytes.
///
/// Input should be the LaTeX body WITHOUT surrounding `$` or `$$` delimiters.
///
/// # Example
/// ```no_run
/// let png_bytes = mathpng::render_to_png(r"\frac{1}{2} + \sqrt{x^2 + y^2}", None).unwrap();
/// std::fs::write("equation.png", &png_bytes).unwrap();
/// ```
pub fn render_to_png(latex: &str, options: Option<RenderOptions>) -> Result<Vec<u8>, MathRenderError> {
    let pixmap = render_to_pixmap(latex, options)?;
    pixmap
        .encode_png()
        .map_err(|e| MathRenderError::Render(e.to_string()))
}

/// Render to a tiny_skia::Pixmap (useful if caller wants to composite further).
pub fn render_to_pixmap(
    latex: &str,
    options: Option<RenderOptions>,
) -> Result<tiny_skia::Pixmap, MathRenderError> {
    let opts = options.unwrap_or_default();
    let font_size_px = opts.font_size_pt * opts.scale;

    // 1. Parse LaTeX to MathExpr tree
    let expr = parse::parse_latex(latex)?;

    // 2. Load font and create renderer
    let math_font = font::MathFont::load()?;
    let renderer = Renderer::new(&math_font, font_size_px);

    // 3. Layout
    let layout = renderer.layout(&expr, opts.display_mode);

    // 4. Render to pixmap
    let img_width = (layout.width + 2.0 * opts.padding as f32).ceil() as u32;
    let img_height = (layout.height() + 2.0 * opts.padding as f32).ceil() as u32;
    let img_width = img_width.max(1);
    let img_height = img_height.max(1);

    let mut pixmap = tiny_skia::Pixmap::new(img_width, img_height)
        .ok_or_else(|| MathRenderError::Render("Failed to create pixmap".into()))?;

    // Fill background
    let bg = tiny_skia::Color::from_rgba8(
        opts.bg_color[0],
        opts.bg_color[1],
        opts.bg_color[2],
        opts.bg_color[3],
    );
    pixmap.fill(bg);

    // Render
    let origin_x = opts.padding as f32;
    let origin_y = opts.padding as f32 + layout.ascent;

    renderer.render_node(
        &mut pixmap,
        &layout,
        origin_x,
        origin_y,
        opts.fg_color,
    );

    Ok(pixmap)
}
