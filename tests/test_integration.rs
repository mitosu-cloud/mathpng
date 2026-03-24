use mathpng::{render_to_png, RenderOptions};

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
