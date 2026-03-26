use mathpng::{render_to_png, render_to_svg, RenderOptions};

#[test]
fn test_render_single_x() {
    let png = render_to_png("x", None).unwrap();
    assert!(!png.is_empty());
    // PNG magic bytes
    assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    std::fs::write("/tmp/mathpng_test_x.png", &png).unwrap();
}

#[test]
fn test_render_simple_expression() {
    let png = render_to_png("x + y", None).unwrap();
    assert!(!png.is_empty());
    assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    std::fs::write("/tmp/mathpng_test_xpy.png", &png).unwrap();
}

#[test]
fn test_render_fraction() {
    let png = render_to_png(r"\frac{a}{b}", None).unwrap();
    assert!(!png.is_empty());
    assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    std::fs::write("/tmp/mathpng_test_frac.png", &png).unwrap();
}

#[test]
fn test_render_superscript() {
    let png = render_to_png(r"x^2", None).unwrap();
    assert!(!png.is_empty());
    std::fs::write("/tmp/mathpng_test_x2.png", &png).unwrap();
}

#[test]
fn test_render_quadratic() {
    let png = render_to_png(r"\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}", None).unwrap();
    assert!(!png.is_empty());
    std::fs::write("/tmp/mathpng_test_quadratic.png", &png).unwrap();
}

#[test]
fn test_custom_colors() {
    let opts = RenderOptions {
        fg_color: [255, 0, 0, 255], // red text
        bg_color: [255, 255, 255, 255], // white background (opaque)
        ..Default::default()
    };
    let png = render_to_png("x", Some(opts)).unwrap();
    assert!(!png.is_empty());
    std::fs::write("/tmp/mathpng_test_red.png", &png).unwrap();
}

#[test]
fn test_sum_with_limits() {
    let png = render_to_png(r"\sum_{i=0}^{n} i^2", None).unwrap();
    assert!(!png.is_empty());
    std::fs::write("/tmp/mathpng_test_sum.png", &png).unwrap();
}

#[test]
fn test_sqrt() {
    let png = render_to_png(r"\sqrt{x^2 + y^2}", None).unwrap();
    assert!(!png.is_empty());
    std::fs::write("/tmp/mathpng_test_sqrt.png", &png).unwrap();
}

#[test]
fn test_transparent_background() {
    let opts = RenderOptions {
        bg_color: [0, 0, 0, 0], // fully transparent
        ..Default::default()
    };
    let png = render_to_png("x", Some(opts)).unwrap();
    assert!(!png.is_empty());
    std::fs::write("/tmp/mathpng_test_transparent.png", &png).unwrap();
}

// ── SVG tests ──────────────────────────────────────────────────────────

#[test]
fn test_svg_basic_structure() {
    let svg = render_to_svg("x", None).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("viewBox="));
    assert!(svg.ends_with("</svg>"));
    std::fs::write("/tmp/mathpng_test_x.svg", &svg).unwrap();
}

#[test]
fn test_svg_has_defs_and_use() {
    let svg = render_to_svg(r"\frac{a}{b}", None).unwrap();
    assert!(svg.contains("<defs>"));
    assert!(svg.contains("<use "));
    assert!(svg.contains("</defs>"));
    std::fs::write("/tmp/mathpng_test_frac.svg", &svg).unwrap();
}

#[test]
fn test_svg_fraction_has_rect() {
    let svg = render_to_svg(r"\frac{a}{b}", None).unwrap();
    // Fraction bar should be a <rect>
    assert!(svg.contains("<rect "));
}

#[test]
fn test_svg_foreground_color() {
    let opts = RenderOptions {
        fg_color: [255, 0, 0, 255],
        ..Default::default()
    };
    let svg = render_to_svg("x", Some(opts)).unwrap();
    assert!(svg.contains("#ff0000"));
    std::fs::write("/tmp/mathpng_test_red.svg", &svg).unwrap();
}

#[test]
fn test_svg_background_color() {
    let opts = RenderOptions {
        bg_color: [255, 255, 255, 255],
        ..Default::default()
    };
    let svg = render_to_svg("x", Some(opts)).unwrap();
    // Background rect with white fill
    assert!(svg.contains("#ffffff"));
}

#[test]
fn test_svg_complex_expression() {
    let svg = render_to_svg(r"\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}", None).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("<defs>"));
    // Should have multiple glyph defs
    assert!(svg.contains("id=\"g0\""));
    assert!(svg.contains("id=\"g1\""));
    std::fs::write("/tmp/mathpng_test_quadratic.svg", &svg).unwrap();
}
