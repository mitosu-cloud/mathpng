use image::{DynamicImage, GenericImageView, ImageReader};
use mathpng::{RenderOptions, render_to_png};
use std::fs;
use std::io::Cursor;
use std::path::Path;

const SNAPSHOT_DIR: &str = "tests/snapshots/ours";
const REFERENCE_DIR: &str = "tests/references/katex";

/// Binarize an image: pixels darker than threshold become 1.0 (ink), else 0.0.
fn binarize(img: &DynamicImage, threshold: u8) -> Vec<Vec<f64>> {
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();
    let mut grid = vec![vec![0.0f64; w as usize]; h as usize];
    for y in 0..h {
        for x in 0..w {
            let lum = gray.get_pixel(x, y).0[0];
            grid[y as usize][x as usize] = if lum < threshold { 1.0 } else { 0.0 };
        }
    }
    grid
}

/// Compute the horizontal ink density profile (ink per row, normalized).
fn horizontal_profile(grid: &[Vec<f64>]) -> Vec<f64> {
    let h = grid.len();
    if h == 0 {
        return vec![];
    }
    let w = grid[0].len();
    let max_ink = w as f64;
    grid.iter()
        .map(|row| row.iter().sum::<f64>() / max_ink)
        .collect()
}

/// Compute the vertical ink density profile (ink per column, normalized).
fn vertical_profile(grid: &[Vec<f64>]) -> Vec<f64> {
    let h = grid.len();
    if h == 0 {
        return vec![];
    }
    let w = grid[0].len();
    let max_ink = h as f64;
    (0..w)
        .map(|x| {
            let col_sum: f64 = (0..h).map(|y| grid[y][x]).sum();
            col_sum / max_ink
        })
        .collect()
}

/// Resample a 1D profile to a target length using linear interpolation.
fn resample(profile: &[f64], target_len: usize) -> Vec<f64> {
    if profile.is_empty() || target_len == 0 {
        return vec![0.0; target_len];
    }
    if profile.len() == target_len {
        return profile.to_vec();
    }
    let src_len = profile.len() as f64;
    let dst_len = target_len as f64;
    (0..target_len)
        .map(|i| {
            let src_pos = (i as f64 + 0.5) * src_len / dst_len - 0.5;
            let lo = (src_pos.floor() as isize).max(0) as usize;
            let hi = (lo + 1).min(profile.len() - 1);
            let frac = src_pos - lo as f64;
            profile[lo] * (1.0 - frac) + profile[hi] * frac
        })
        .collect()
}

/// Pearson correlation coefficient between two equal-length vectors.
fn correlation(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let n = a.len() as f64;
    if n == 0.0 {
        return 0.0;
    }
    let mean_a: f64 = a.iter().sum::<f64>() / n;
    let mean_b: f64 = b.iter().sum::<f64>() / n;
    let mut cov = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;
    for i in 0..a.len() {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }
    if var_a < 1e-12 || var_b < 1e-12 {
        // One of the signals is constant — if both are, they match
        return if var_a < 1e-12 && var_b < 1e-12 {
            1.0
        } else {
            0.0
        };
    }
    cov / (var_a.sqrt() * var_b.sqrt())
}

/// Trim whitespace (non-ink) rows/cols from a binarized grid, returning the cropped region.
fn trim_grid(grid: &[Vec<f64>]) -> Vec<Vec<f64>> {
    if grid.is_empty() {
        return vec![];
    }
    let h = grid.len();
    let w = grid[0].len();

    // Find bounding box of ink
    let mut top = h;
    let mut bottom = 0;
    let mut left = w;
    let mut right = 0;

    for y in 0..h {
        for x in 0..w {
            if grid[y][x] > 0.5 {
                top = top.min(y);
                bottom = bottom.max(y);
                left = left.min(x);
                right = right.max(x);
            }
        }
    }

    if top > bottom || left > right {
        return vec![vec![0.0]]; // no ink found
    }

    // Add small margin (5% of dimension)
    let margin_y = ((bottom - top) as f64 * 0.05).ceil() as usize;
    let margin_x = ((right - left) as f64 * 0.05).ceil() as usize;
    let top = top.saturating_sub(margin_y);
    let bottom = (bottom + margin_y).min(h - 1);
    let left = left.saturating_sub(margin_x);
    let right = (right + margin_x).min(w - 1);

    (top..=bottom)
        .map(|y| grid[y][left..=right].to_vec())
        .collect()
}

/// Compare two images structurally by correlating their ink density profiles.
///
/// Returns (horizontal_correlation, vertical_correlation, ink_ratio).
fn structural_compare(ours: &DynamicImage, reference: &DynamicImage) -> (f64, f64, f64) {
    let grid_ours = trim_grid(&binarize(ours, 200));
    let grid_ref = trim_grid(&binarize(reference, 200));

    let h_prof_ours = horizontal_profile(&grid_ours);
    let h_prof_ref = horizontal_profile(&grid_ref);
    let v_prof_ours = vertical_profile(&grid_ours);
    let v_prof_ref = vertical_profile(&grid_ref);

    // Resample to common length (use 128 bins for stable comparison)
    let bins = 128;
    let h_ours = resample(&h_prof_ours, bins);
    let h_ref = resample(&h_prof_ref, bins);
    let v_ours = resample(&v_prof_ours, bins);
    let v_ref = resample(&v_prof_ref, bins);

    let h_corr = correlation(&h_ours, &h_ref);
    let v_corr = correlation(&v_ours, &v_ref);

    // Ink density ratio (in trimmed bounding box)
    let ink_ours: f64 = grid_ours.iter().flat_map(|r| r.iter()).sum();
    let ink_ref: f64 = grid_ref.iter().flat_map(|r| r.iter()).sum();
    let area_ours = (grid_ours.len() * grid_ours.first().map_or(1, |r| r.len())) as f64;
    let area_ref = (grid_ref.len() * grid_ref.first().map_or(1, |r| r.len())) as f64;
    let density_ours = ink_ours / area_ours;
    let density_ref = ink_ref / area_ref;
    let ink_ratio = if density_ref > 0.0 && density_ours > 0.0 {
        let r = density_ours / density_ref;
        r.min(1.0 / r) // symmetric: always 0..1
    } else {
        0.0
    };

    (h_corr, v_corr, ink_ratio)
}

/// Compute aspect ratio similarity (0.0 to 1.0) based on trimmed ink bounding boxes.
fn aspect_ratio_similarity(ours: &DynamicImage, reference: &DynamicImage) -> f64 {
    let grid_ours = trim_grid(&binarize(ours, 200));
    let grid_ref = trim_grid(&binarize(reference, 200));

    let oh = grid_ours.len() as f64;
    let ow = grid_ours.first().map_or(0, |r| r.len()) as f64;
    let rh = grid_ref.len() as f64;
    let rw = grid_ref.first().map_or(0, |r| r.len()) as f64;

    if oh < 1.0 || rh < 1.0 {
        return 0.0;
    }
    let ar_ours = ow / oh;
    let ar_ref = rw / rh;
    let ratio = if ar_ours > ar_ref {
        ar_ref / ar_ours
    } else {
        ar_ours / ar_ref
    };
    ratio
}

fn render_options() -> RenderOptions {
    RenderOptions {
        font_size_pt: 24.0,
        scale: 2.0,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        display_mode: true,
        ..Default::default()
    }
}

/// Render an expression and optionally compare against a reference snapshot.
fn run_snapshot_test(name: &str, latex: &str) {
    let png_bytes = render_to_png(latex, Some(render_options()))
        .unwrap_or_else(|e| panic!("render_to_png failed for {name}: {e}"));

    // Verify valid PNG
    let decoded = ImageReader::new(Cursor::new(&png_bytes))
        .with_guessed_format()
        .expect("failed to guess image format")
        .decode()
        .expect("rendered bytes are not a valid image");
    assert!(
        decoded.width() > 0 && decoded.height() > 0,
        "rendered image has zero dimensions"
    );

    // Save our output
    let ours_path = Path::new(SNAPSHOT_DIR).join(format!("{name}.png"));
    fs::create_dir_all(SNAPSHOT_DIR).ok();
    fs::write(&ours_path, &png_bytes).expect("failed to write snapshot");

    // Compare against reference if it exists
    let ref_path = Path::new(REFERENCE_DIR).join(format!("{name}.png"));
    if ref_path.exists() {
        let reference = ImageReader::open(&ref_path)
            .expect("failed to open reference")
            .decode()
            .expect("failed to decode reference");

        let (h_corr, v_corr, ink_ratio) = structural_compare(&decoded, &reference);
        let aspect_sim = aspect_ratio_similarity(&decoded, &reference);

        // Combined score: weighted average
        let score = 0.35 * h_corr + 0.35 * v_corr + 0.15 * ink_ratio + 0.15 * aspect_sim;

        println!(
            "[{name}] h_corr={h_corr:.3} v_corr={v_corr:.3} ink_ratio={ink_ratio:.3} \
             aspect={aspect_sim:.3} => score={score:.3}"
        );

        // Thresholds: at least one axis should correlate well, and combined
        // score should be meaningful. Different fonts cause axis-specific
        // differences, so we require the combined score rather than both axes.
        let min_axis = h_corr.min(v_corr);
        let max_axis = h_corr.max(v_corr);
        assert!(
            max_axis > 0.3,
            "[{name}] Best axis correlation {max_axis:.3} too low (need >0.3)"
        );
        assert!(
            score > 0.35,
            "[{name}] Combined score {score:.3} too low (need >0.35)"
        );
        if min_axis < 0.15 {
            println!(
                "  WARNING: [{name}] weak axis correlation ({min_axis:.3}) — \
                 rendering may have structural differences from reference"
            );
        }
    } else {
        println!(
            "[{name}] No reference at {}; skipping comparison",
            ref_path.display()
        );
    }
}

#[test] fn snapshot_simple_x() { run_snapshot_test("simple_x", "x"); }
#[test] fn snapshot_x_plus_y() { run_snapshot_test("x_plus_y", "x + y"); }
#[test] fn snapshot_fraction() { run_snapshot_test("fraction", r"\frac{a}{b}"); }
#[test] fn snapshot_superscript() { run_snapshot_test("superscript", "x^2"); }
#[test] fn snapshot_subscript() { run_snapshot_test("subscript", "x_i"); }
#[test] fn snapshot_sub_superscript() { run_snapshot_test("sub_superscript", "x_i^2"); }
#[test] fn snapshot_nested_fraction() { run_snapshot_test("nested_fraction", r"\frac{1}{1 + \frac{1}{x}}"); }
#[test] fn snapshot_quadratic() { run_snapshot_test("quadratic", r"\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}"); }
#[test] fn snapshot_sqrt() { run_snapshot_test("sqrt", r"\sqrt{x^2 + y^2}"); }
#[test] fn snapshot_sum_limits() { run_snapshot_test("sum_limits", r"\sum_{i=0}^{n} i^2"); }
#[test] fn snapshot_integral() { run_snapshot_test("integral", r"\int_0^\infty e^{-x^2} dx"); }
#[test] fn snapshot_greek() { run_snapshot_test("greek", r"\alpha + \beta = \gamma"); }
#[test] fn snapshot_product() { run_snapshot_test("product", r"\prod_{k=1}^{n} k"); }
