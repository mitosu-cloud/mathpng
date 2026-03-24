/// The intermediate representation — a tree of math constructs.
#[derive(Debug, Clone)]
pub enum MathExpr {
    /// A sequence of expressions laid out horizontally
    Row(Vec<MathExpr>),

    /// A single glyph (letter, digit, operator symbol)
    Glyph {
        codepoint: char,
        variant: MathVariant,
    },

    /// Fraction: numerator / denominator
    Fraction {
        numerator: Box<MathExpr>,
        denominator: Box<MathExpr>,
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
        index: Option<Box<MathExpr>>,
    },

    /// Stretchy delimiter pair: \left( ... \right)
    Delimited {
        open: Option<char>,
        close: Option<char>,
        content: Box<MathExpr>,
    },

    /// Big operator: \sum, \prod, \int, etc.
    BigOperator {
        symbol: char,
        above: Option<Box<MathExpr>>,
        below: Option<Box<MathExpr>>,
        limits: bool,
    },

    /// Accent over expression: \hat, \tilde, \vec, \bar, \dot, \ddot
    Accent {
        base: Box<MathExpr>,
        accent_char: char,
    },

    /// Explicit space in em units
    Space(f32),

    /// Text mode: \text{...}
    Text(String),

    /// Generic grouping (from {})
    Group(Vec<MathExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MathVariant {
    Normal,
    Italic,
    Bold,
    BoldItalic,
    Script,
    BoldScript,
    Fraktur,
    BoldFraktur,
    DoubleStruck,
    SansSerif,
    SansSerifBold,
    SansSerifItalic,
    Monospace,
}

/// Map a character to its Unicode Mathematical Alphanumeric Symbols variant.
pub fn map_variant(c: char, variant: MathVariant) -> char {
    match variant {
        MathVariant::Normal => c,
        MathVariant::Italic => map_italic(c),
        MathVariant::Bold => map_bold(c),
        MathVariant::BoldItalic => map_bold_italic(c),
        MathVariant::DoubleStruck => map_double_struck(c),
        MathVariant::Script => map_script(c),
        MathVariant::Fraktur => map_fraktur(c),
        _ => c,
    }
}

fn map_italic(c: char) -> char {
    if c.is_ascii_lowercase() {
        // Special case: 'h' maps to U+210E (Planck constant)
        if c == 'h' {
            return '\u{210E}';
        }
        char::from_u32(0x1D44E + (c as u32 - 'a' as u32)).unwrap_or(c)
    } else if c.is_ascii_uppercase() {
        char::from_u32(0x1D434 + (c as u32 - 'A' as u32)).unwrap_or(c)
    } else {
        c
    }
}

fn map_bold(c: char) -> char {
    if c.is_ascii_lowercase() {
        char::from_u32(0x1D41A + (c as u32 - 'a' as u32)).unwrap_or(c)
    } else if c.is_ascii_uppercase() {
        char::from_u32(0x1D400 + (c as u32 - 'A' as u32)).unwrap_or(c)
    } else if c.is_ascii_digit() {
        char::from_u32(0x1D7CE + (c as u32 - '0' as u32)).unwrap_or(c)
    } else {
        c
    }
}

fn map_bold_italic(c: char) -> char {
    if c.is_ascii_lowercase() {
        char::from_u32(0x1D482 + (c as u32 - 'a' as u32)).unwrap_or(c)
    } else if c.is_ascii_uppercase() {
        char::from_u32(0x1D468 + (c as u32 - 'A' as u32)).unwrap_or(c)
    } else {
        c
    }
}

fn map_double_struck(c: char) -> char {
    // Special cases for commonly used double-struck letters
    match c {
        'C' => '\u{2102}',
        'H' => '\u{210D}',
        'N' => '\u{2115}',
        'P' => '\u{2119}',
        'Q' => '\u{211A}',
        'R' => '\u{211D}',
        'Z' => '\u{2124}',
        _ if c.is_ascii_uppercase() => {
            char::from_u32(0x1D538 + (c as u32 - 'A' as u32)).unwrap_or(c)
        }
        _ if c.is_ascii_lowercase() => {
            char::from_u32(0x1D552 + (c as u32 - 'a' as u32)).unwrap_or(c)
        }
        _ if c.is_ascii_digit() => {
            char::from_u32(0x1D7D8 + (c as u32 - '0' as u32)).unwrap_or(c)
        }
        _ => c,
    }
}

fn map_script(c: char) -> char {
    // Special cases
    match c {
        'B' => '\u{212C}',
        'E' => '\u{2130}',
        'F' => '\u{2131}',
        'H' => '\u{210B}',
        'I' => '\u{2110}',
        'L' => '\u{2112}',
        'M' => '\u{2133}',
        'R' => '\u{211B}',
        'e' => '\u{212F}',
        'g' => '\u{210A}',
        'o' => '\u{2134}',
        _ if c.is_ascii_uppercase() => {
            char::from_u32(0x1D49C + (c as u32 - 'A' as u32)).unwrap_or(c)
        }
        _ if c.is_ascii_lowercase() => {
            char::from_u32(0x1D4B6 + (c as u32 - 'a' as u32)).unwrap_or(c)
        }
        _ => c,
    }
}

fn map_fraktur(c: char) -> char {
    match c {
        'C' => '\u{212D}',
        'H' => '\u{210C}',
        'I' => '\u{2111}',
        'R' => '\u{211C}',
        'Z' => '\u{2128}',
        _ if c.is_ascii_uppercase() => {
            char::from_u32(0x1D504 + (c as u32 - 'A' as u32)).unwrap_or(c)
        }
        _ if c.is_ascii_lowercase() => {
            char::from_u32(0x1D51E + (c as u32 - 'a' as u32)).unwrap_or(c)
        }
        _ => c,
    }
}
