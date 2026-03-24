use pulldown_latex::{Event, Parser, Storage};

use crate::math_expr::{MathExpr, MathVariant};
use crate::MathRenderError;

/// Parse a LaTeX math string into a MathExpr tree.
pub fn parse_latex(input: &str) -> Result<MathExpr, MathRenderError> {
    let storage = Storage::new();
    let parser = Parser::new(input, &storage);
    let events: Vec<Event<'_>> = parser
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| MathRenderError::Parse(format!("{e}")))?;

    let mut cursor = EventCursor::new(&events);
    let exprs = cursor.parse_elements_until_end()?;

    Ok(if exprs.len() == 1 {
        exprs.into_iter().next().unwrap()
    } else {
        MathExpr::Row(exprs)
    })
}

struct EventCursor<'a> {
    events: &'a [Event<'a>],
    pos: usize,
}

impl<'a> EventCursor<'a> {
    fn new(events: &'a [Event<'a>]) -> Self {
        Self { events, pos: 0 }
    }

    fn peek(&self) -> Option<&'a Event<'a>> {
        self.events.get(self.pos)
    }

    fn next(&mut self) -> Option<&'a Event<'a>> {
        let event = self.events.get(self.pos);
        if event.is_some() {
            self.pos += 1;
        }
        event
    }

    fn at_end(&self) -> bool {
        self.pos >= self.events.len()
    }

    /// Parse elements until we run out of events.
    fn parse_elements_until_end(&mut self) -> Result<Vec<MathExpr>, MathRenderError> {
        let mut exprs = Vec::new();
        while !self.at_end() {
            if let Some(Event::End) = self.peek() {
                break;
            }
            exprs.push(self.parse_one_element()?);
        }
        Ok(exprs)
    }

    /// Parse a single logical element from the event stream.
    /// In pulldown-latex's prefix notation:
    /// - Visual/Script events consume the next N elements as operands
    /// - Begin/End groups count as one element
    /// - Content is one element
    fn parse_one_element(&mut self) -> Result<MathExpr, MathRenderError> {
        let event = self.next().ok_or_else(|| {
            MathRenderError::Parse("Unexpected end of event stream".into())
        })?;

        match event {
            Event::Content(content) => self.parse_content(content),
            Event::Begin(grouping) => self.parse_group(grouping),
            Event::Visual(visual) => self.parse_visual(visual),
            Event::Script { ty, position } => self.parse_script(*ty, *position),
            Event::Space { width, height: _ } => {
                let em = width
                    .as_ref()
                    .map(|d| d.value)
                    .unwrap_or(0.0);
                // Convert common units to approximate em
                Ok(MathExpr::Space(em))
            }
            Event::StateChange(sc) => {
                self.parse_state_change(sc)
            }
            Event::End => {
                // Should not normally reach here
                Ok(MathExpr::Space(0.0))
            }
            Event::EnvironmentFlow(_) => {
                // Skip environment flow events for now
                Ok(MathExpr::Space(0.0))
            }
        }
    }

    fn parse_content(
        &self,
        content: &pulldown_latex::event::Content<'a>,
    ) -> Result<MathExpr, MathRenderError> {
        use pulldown_latex::event::Content;

        match content {
            Content::Ordinary { content: c, .. } => {
                let variant = default_variant(*c);
                Ok(MathExpr::Glyph {
                    codepoint: *c,
                    variant,
                })
            }
            Content::Number(s) => {
                let glyphs: Vec<MathExpr> = s
                    .chars()
                    .map(|c| MathExpr::Glyph {
                        codepoint: c,
                        variant: MathVariant::Normal,
                    })
                    .collect();
                Ok(if glyphs.len() == 1 {
                    glyphs.into_iter().next().unwrap()
                } else {
                    MathExpr::Row(glyphs)
                })
            }
            Content::Text(s) => Ok(MathExpr::Text(s.to_string())),
            Content::Function(s) => {
                // Render function names like \sin, \cos as upright text
                Ok(MathExpr::Text(s.to_string()))
            }
            Content::LargeOp { content: c, .. } => {
                // Large operators like \sum, \prod, \int
                Ok(MathExpr::Glyph {
                    codepoint: *c,
                    variant: MathVariant::Normal,
                })
            }
            Content::BinaryOp { content: c, .. } => Ok(MathExpr::Glyph {
                codepoint: *c,
                variant: MathVariant::Normal,
            }),
            Content::Relation { content: rel, .. } => {
                // Use the public encode_utf8_to_buf method to extract char(s)
                let mut buf = [0u8; 8];
                let bytes = rel.encode_utf8_to_buf(&mut buf);
                let s = std::str::from_utf8(bytes).unwrap_or("=");
                let glyphs: Vec<MathExpr> = s
                    .chars()
                    .map(|ch| MathExpr::Glyph {
                        codepoint: ch,
                        variant: MathVariant::Normal,
                    })
                    .collect();
                Ok(if glyphs.len() == 1 {
                    glyphs.into_iter().next().unwrap()
                } else if glyphs.is_empty() {
                    MathExpr::Glyph {
                        codepoint: '=',
                        variant: MathVariant::Normal,
                    }
                } else {
                    MathExpr::Row(glyphs)
                })
            }
            Content::Delimiter { content: c, .. } => Ok(MathExpr::Glyph {
                codepoint: *c,
                variant: MathVariant::Normal,
            }),
            Content::Punctuation(c) => Ok(MathExpr::Glyph {
                codepoint: *c,
                variant: MathVariant::Normal,
            }),
        }
    }

    fn parse_group(
        &mut self,
        grouping: &pulldown_latex::event::Grouping,
    ) -> Result<MathExpr, MathRenderError> {
        use pulldown_latex::event::Grouping;

        match grouping {
            Grouping::LeftRight(open, close) => {
                let children = self.parse_elements_until_end()?;
                self.expect_end()?;
                let content = if children.len() == 1 {
                    children.into_iter().next().unwrap()
                } else {
                    MathExpr::Row(children)
                };
                Ok(MathExpr::Delimited {
                    open: *open,
                    close: *close,
                    content: Box::new(content),
                })
            }
            Grouping::Normal => {
                let children = self.parse_elements_until_end()?;
                self.expect_end()?;
                Ok(if children.len() == 1 {
                    children.into_iter().next().unwrap()
                } else {
                    MathExpr::Group(children)
                })
            }
            _ => {
                // For other groupings (matrix, array, etc.), collect children
                let children = self.parse_elements_until_end()?;
                self.expect_end()?;
                Ok(MathExpr::Group(children))
            }
        }
    }

    fn parse_visual(
        &mut self,
        visual: &pulldown_latex::event::Visual,
    ) -> Result<MathExpr, MathRenderError> {
        use pulldown_latex::event::Visual;

        match visual {
            Visual::Fraction(_thickness) => {
                let numerator = self.parse_one_element()?;
                let denominator = self.parse_one_element()?;
                Ok(MathExpr::Fraction {
                    numerator: Box::new(numerator),
                    denominator: Box::new(denominator),
                })
            }
            Visual::SquareRoot => {
                let radicand = self.parse_one_element()?;
                Ok(MathExpr::Radical {
                    radicand: Box::new(radicand),
                    index: None,
                })
            }
            Visual::Root => {
                let radicand = self.parse_one_element()?;
                let index = self.parse_one_element()?;
                Ok(MathExpr::Radical {
                    radicand: Box::new(radicand),
                    index: Some(Box::new(index)),
                })
            }
            Visual::Negation => {
                // Just parse the negated element as-is for now
                self.parse_one_element()
            }
        }
    }

    fn parse_script(
        &mut self,
        ty: pulldown_latex::event::ScriptType,
        position: pulldown_latex::event::ScriptPosition,
    ) -> Result<MathExpr, MathRenderError> {
        use pulldown_latex::event::{ScriptPosition, ScriptType};

        match ty {
            ScriptType::Superscript => {
                let base = self.parse_one_element()?;
                let sup = self.parse_one_element()?;

                // Check if this is a large operator with movable limits
                if matches!(position, ScriptPosition::AboveBelow | ScriptPosition::Movable) {
                    if let MathExpr::Glyph { codepoint, .. } = &base {
                        if is_large_operator(*codepoint) {
                            return Ok(MathExpr::BigOperator {
                                symbol: *codepoint,
                                above: Some(Box::new(sup)),
                                below: None,
                                limits: matches!(position, ScriptPosition::AboveBelow | ScriptPosition::Movable),
                            });
                        }
                    }
                }

                Ok(MathExpr::Scripts {
                    base: Box::new(base),
                    superscript: Some(Box::new(sup)),
                    subscript: None,
                })
            }
            ScriptType::Subscript => {
                let base = self.parse_one_element()?;
                let sub = self.parse_one_element()?;

                if matches!(position, ScriptPosition::AboveBelow | ScriptPosition::Movable) {
                    if let MathExpr::Glyph { codepoint, .. } = &base {
                        if is_large_operator(*codepoint) {
                            return Ok(MathExpr::BigOperator {
                                symbol: *codepoint,
                                above: None,
                                below: Some(Box::new(sub)),
                                limits: true,
                            });
                        }
                    }
                }

                Ok(MathExpr::Scripts {
                    base: Box::new(base),
                    superscript: None,
                    subscript: Some(Box::new(sub)),
                })
            }
            ScriptType::SubSuperscript => {
                let base = self.parse_one_element()?;
                let sub = self.parse_one_element()?;
                let sup = self.parse_one_element()?;

                if matches!(position, ScriptPosition::AboveBelow | ScriptPosition::Movable) {
                    if let MathExpr::Glyph { codepoint, .. } = &base {
                        if is_large_operator(*codepoint) {
                            return Ok(MathExpr::BigOperator {
                                symbol: *codepoint,
                                above: Some(Box::new(sup)),
                                below: Some(Box::new(sub)),
                                limits: true,
                            });
                        }
                    }
                }

                Ok(MathExpr::Scripts {
                    base: Box::new(base),
                    superscript: Some(Box::new(sup)),
                    subscript: Some(Box::new(sub)),
                })
            }
        }
    }

    fn parse_state_change(
        &mut self,
        sc: &pulldown_latex::event::StateChange,
    ) -> Result<MathExpr, MathRenderError> {
        use pulldown_latex::event::StateChange;

        match sc {
            StateChange::Font(Some(_font)) => {
                // Font changes affect the next element — for now just pass through
                // TODO: track font state and apply to subsequent glyphs
                Ok(MathExpr::Space(0.0))
            }
            StateChange::Font(None) => Ok(MathExpr::Space(0.0)),
            _ => Ok(MathExpr::Space(0.0)),
        }
    }

    fn expect_end(&mut self) -> Result<(), MathRenderError> {
        match self.next() {
            Some(Event::End) => Ok(()),
            Some(other) => Err(MathRenderError::Parse(format!(
                "Expected End event, got {other:?}"
            ))),
            None => Err(MathRenderError::Parse("Expected End event, got EOF".into())),
        }
    }
}

/// Determine the default math variant for a character.
/// In TeX math mode, single Latin letters are italic; digits and operators are upright.
fn default_variant(c: char) -> MathVariant {
    if c.is_ascii_alphabetic() {
        MathVariant::Italic
    } else {
        MathVariant::Normal
    }
}

/// Check if a character is a large operator (sum, product, integral, etc.)
fn is_large_operator(c: char) -> bool {
    matches!(
        c,
        '\u{2211}' // ∑
        | '\u{220F}' // ∏
        | '\u{2210}' // ∐
        | '\u{222B}' // ∫
        | '\u{222C}' // ∬
        | '\u{222D}' // ∭
        | '\u{222E}' // ∮
        | '\u{22C0}' // ⋀
        | '\u{22C1}' // ⋁
        | '\u{22C2}' // ⋂
        | '\u{22C3}' // ⋃
        | '\u{2A00}' // ⨀
        | '\u{2A01}' // ⨁
        | '\u{2A02}' // ⨂
    )
}
