uniffi::setup_scaffolding!();

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum MathError {
    #[error("Parse failed: {msg}")]
    ParseFailed { msg: String },
    #[error("Render failed: {msg}")]
    RenderFailed { msg: String },
    #[error("IO failed: {msg}")]
    IoFailed { msg: String },
    #[error("Theme parse failed: {msg}")]
    ThemeParseFailed { msg: String },
}

// ── Checksum ────────────────────────────────────────────────────────────────

/// SHA256 of LaTeX source, truncated to 12 hex chars.
fn compute_checksum(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let result = hasher.finalize();
    hex_encode(&result[..6])
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Sanitise theme name for use in filenames: lowercase, spaces → hyphens.
fn sanitise_theme_name(name: &str) -> String {
    name.to_lowercase().replace(' ', "-")
}

// ── Theme mapping ───────────────────────────────────────────────────────────

/// Extract foreground and background RGBA colors from Mitosu theme JSON.
fn extract_colors(theme_json: &str) -> Result<([u8; 4], [u8; 4]), MathError> {
    let val: serde_json::Value = serde_json::from_str(theme_json)
        .map_err(|e| MathError::ThemeParseFailed { msg: e.to_string() })?;

    let colors = val.get("colors").and_then(|v| v.as_object());
    let is_dark = val
        .get("type")
        .and_then(|v| v.as_str())
        .map(|t| t == "dark")
        .unwrap_or(true);

    let get = |key: &str| -> Option<String> {
        colors
            .and_then(|c| c.get(key))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    // Foreground: math.foreground → markdownEditor.text → ui.text → default
    let fg_hex = get("math.foreground")
        .or_else(|| get("markdownEditor.text"))
        .or_else(|| get("ui.text"))
        .unwrap_or_else(|| {
            if is_dark {
                "#FFFFFF".to_string()
            } else {
                "#000000".to_string()
            }
        });

    // Background: math.background → transparent
    let bg = [255, 255, 255, 0]; // always transparent

    let fg = parse_hex_color(&fg_hex).unwrap_or(if is_dark {
        [255, 255, 255, 255]
    } else {
        [0, 0, 0, 255]
    });

    Ok((fg, bg))
}

/// Parse a hex color string (#RGB, #RRGGBB, #RRGGBBAA) into [R, G, B, A].
fn parse_hex_color(hex: &str) -> Option<[u8; 4]> {
    let hex = hex.trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some([r, g, b, 255])
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some([r, g, b, 255])
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some([r, g, b, a])
        }
        _ => None,
    }
}

// ── Metadata ────────────────────────────────────────────────────────────────

/// In-memory representation of metadata.json (preserves mermaid_diagrams)
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Metadata {
    #[serde(default)]
    mermaid_diagrams: HashMap<String, DiagramEntry>,
    #[serde(default)]
    math_equations: HashMap<String, EquationEntry>,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
struct DiagramEntry {
    files: HashMap<String, String>,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
struct EquationEntry {
    files: HashMap<String, String>,
}

fn metadata_path(note_folder: &Path) -> PathBuf {
    note_folder.join("metadata.json")
}

fn read_metadata(note_folder: &Path) -> Metadata {
    let path = metadata_path(note_folder);
    match fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Metadata::default(),
    }
}

fn write_metadata(note_folder: &Path, meta: &Metadata) -> Result<(), MathError> {
    let path = metadata_path(note_folder);
    let json = serde_json::to_string_pretty(meta)
        .map_err(|e| MathError::IoFailed { msg: e.to_string() })?;
    fs::write(&path, json).map_err(|e| MathError::IoFailed { msg: e.to_string() })?;
    Ok(())
}

// ── Public FFI functions ────────────────────────────────────────────────────

/// Primary render function:
/// 1. Parses the Mitosu theme JSON → extracts fg/bg colors
/// 2. Computes SHA256 checksum of LaTeX source
/// 3. Checks if output file already exists on disk (cache hit)
/// 4. If not, renders and writes to disk (SVG or PNG)
/// 5. Updates metadata.json
/// Returns the filename (not full path) of the output file.
///
/// `output_format` should be `"svg"` or `"png"`. Defaults to SVG for unknown values.
/// `display_mode` controls whether the equation is rendered in display (block) or inline mode.
#[uniffi::export]
pub fn render_math_for_note(
    latex_source: String,
    note_folder_path: String,
    theme_json: String,
    theme_name: String,
    font_size_pt: f32,
    scale: f32,
    display_mode: bool,
    output_format: String,
) -> Result<String, MathError> {
    let note_folder = Path::new(&note_folder_path);
    let checksum = compute_checksum(&latex_source);
    let safe_theme = sanitise_theme_name(&theme_name);
    let use_png = output_format.eq_ignore_ascii_case("png");
    let ext = if use_png { "png" } else { "svg" };
    let filename = format!("math_{}_{}.{}", checksum, safe_theme, ext);
    let output_path = note_folder.join(&filename);

    // Cache hit — file already exists
    if output_path.exists() {
        return Ok(filename);
    }

    // Extract colors from theme
    let (fg, bg) = extract_colors(&theme_json)?;

    let options = mathpng::RenderOptions {
        font_size_pt,
        scale,
        fg_color: fg,
        bg_color: bg,
        padding: 8,
        display_mode,
    };

    if use_png {
        let png_bytes = mathpng::render_to_png(&latex_source, Some(options))
            .map_err(|e| MathError::RenderFailed { msg: e.to_string() })?;
        fs::write(&output_path, &png_bytes)
            .map_err(|e| MathError::IoFailed { msg: e.to_string() })?;
    } else {
        let svg = mathpng::render_to_svg(&latex_source, Some(options))
            .map_err(|e| MathError::RenderFailed { msg: e.to_string() })?;
        fs::write(&output_path, &svg)
            .map_err(|e| MathError::IoFailed { msg: e.to_string() })?;
    }

    // Update metadata.json
    let mut meta = read_metadata(note_folder);
    let entry = meta.math_equations.entry(checksum.clone()).or_default();
    entry.files.insert(safe_theme, filename.clone());
    write_metadata(note_folder, &meta)?;

    Ok(filename)
}

/// Compute checksum only (for Swift-side cache checks without full render).
#[uniffi::export]
pub fn math_checksum(latex_source: String) -> String {
    compute_checksum(&latex_source)
}

/// Remove all math files whose checksum is NOT in `valid_checksums`.
/// Updates metadata.json. Returns list of deleted filenames.
#[uniffi::export]
pub fn cleanup_stale_math_files(
    note_folder_path: String,
    valid_checksums: Vec<String>,
) -> Result<Vec<String>, MathError> {
    let note_folder = Path::new(&note_folder_path);
    let mut meta = read_metadata(note_folder);
    let mut deleted: Vec<String> = Vec::new();

    // Collect checksums to remove from metadata
    let stale_checksums: Vec<String> = meta
        .math_equations
        .keys()
        .filter(|cs| !valid_checksums.contains(cs))
        .cloned()
        .collect();

    for cs in &stale_checksums {
        if let Some(entry) = meta.math_equations.remove(cs) {
            for (_theme, filename) in &entry.files {
                let path = note_folder.join(filename);
                if path.exists() {
                    let _ = fs::remove_file(&path);
                    deleted.push(filename.clone());
                }
            }
        }
    }

    // Also scan for untracked math files on disk
    if let Ok(entries) = fs::read_dir(note_folder) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("math_") && (name.ends_with(".png") || name.ends_with(".svg")) {
                // Extract checksum from filename: math_<checksum>_<theme>.(png|svg)
                if let Some(cs) = name
                    .strip_prefix("math_")
                    .and_then(|rest| rest.split('_').next())
                {
                    if !valid_checksums.contains(&cs.to_string()) && !deleted.contains(&name) {
                        let path = entry.path();
                        if path.exists() {
                            let _ = fs::remove_file(&path);
                            deleted.push(name);
                        }
                    }
                }
            }
        }
    }

    if !stale_checksums.is_empty() {
        write_metadata(note_folder, &meta)?;
    }

    Ok(deleted)
}

/// Read math metadata from a note folder's metadata.json.
/// Returns JSON string of the math_equations section (or empty object).
#[uniffi::export]
pub fn get_math_metadata(note_folder_path: String) -> Result<String, MathError> {
    let note_folder = Path::new(&note_folder_path);
    let meta = read_metadata(note_folder);
    serde_json::to_string(&meta.math_equations)
        .map_err(|e| MathError::IoFailed { msg: e.to_string() })
}
