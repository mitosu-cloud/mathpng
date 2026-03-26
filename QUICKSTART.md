# Quickstart

## Install

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
```

The binary will be at `target/release/mathpng`.

## CLI Usage

```
mathpng [OPTIONS] <LATEX> <OUTPUT>
```

The `<LATEX>` argument is a LaTeX math expression **without** surrounding `$` or `$$` delimiters.

## Examples

### Simple expression

```bash
mathpng "x^2 + y^2 = z^2" pythagorean.png
```

### Fraction

```bash
mathpng '\frac{a}{b}' fraction.png
```

### Quadratic formula

```bash
mathpng '\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}' quadratic.png
```

### Sum with limits

```bash
mathpng '\sum_{i=0}^{n} i^2' sum.png
```

### Integral

```bash
mathpng '\int_0^\infty e^{-x^2} dx' integral.png
```

### Square root

```bash
mathpng '\sqrt{x^2 + y^2}' magnitude.png
```

### Greek letters

```bash
mathpng '\alpha + \beta = \gamma' greek.png
```

## Options

### Font size

Set the font size in points (default: 20):

```bash
mathpng --font-size 32 'E = mc^2' einstein.png
```

### Scale factor

Control pixels-per-point for retina/HiDPI output (default: 2.0):

```bash
mathpng --scale 4.0 'E = mc^2' einstein_4x.png
```

### Foreground color

Set text color as a hex value (`RRGGBB` or `RRGGBBAA`):

```bash
mathpng --fg ff0000 'x^2' red.png
mathpng --fg 336699 '\frac{a}{b}' blue.png
```

### Background color

Set background color. Use `00` alpha for transparent (the default):

```bash
# White opaque background
mathpng --bg ffffffff 'x^2' white_bg.png

# Transparent background (default)
mathpng --bg ffffff00 'x^2' transparent.png

# Dark background with light text
mathpng --fg ffffff --bg 1a1a1aff '\sum_{i=1}^n i' dark.png
```

### Padding

Set padding around the expression in pixels (default: 8):

```bash
mathpng --padding 16 '\frac{a}{b}' padded.png
```

### Inline mode

Use inline (text-style) rendering instead of display mode:

```bash
mathpng --inline '\frac{a}{b}' inline_fraction.png
```

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
mathpng = { path = "." }
```

```rust
use mathpng::{render_to_png, RenderOptions};

fn main() {
    // Default options (display mode, black on transparent, 20pt @ 2x)
    let png = render_to_png(r"\frac{1}{2}", None).unwrap();
    std::fs::write("half.png", &png).unwrap();

    // Custom options
    let opts = RenderOptions {
        font_size_pt: 32.0,
        fg_color: [255, 0, 0, 255],       // red
        bg_color: [255, 255, 255, 255],    // white
        ..Default::default()
    };
    let png = render_to_png(r"\sum_{i=0}^{n} i^2", Some(opts)).unwrap();
    std::fs::write("sum.png", &png).unwrap();
}
```
