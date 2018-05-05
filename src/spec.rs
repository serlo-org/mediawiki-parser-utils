//! The template specification for "Mathe-für-Nicht-Freaks".

use util::*;
use std::io;
use mediawiki_parser::*;

#[cfg(debug_assertions)]
const _SPEC: &'static str = include_str!("templates.yml");

#[derive(TemplateSpec)]
#[spec = "templates.yml"]
struct _DummySpec;

/// A list of elements only contains one math-tag.
pub fn is_math_tag(elems: &[Element]) -> PredResult {
    if elems.len() != 1 {
        return Err(PredError {
            tree: None,
            cause: "There is none or more than one element in this math tag!".into()
        });
    }
    if let Some(&Element::Formatted(ref fmt)) = elems.first() {
        if fmt.markup != MarkupType::Math {
            return Err(PredError {
                tree: elems.first(),
                cause: "This is not math-formatted!".into()
            })
        };
        Ok(())
    } else {
        Err(PredError {
            tree: elems.first(),
            cause: "This is not math-formatted text!".into()
        })
    }
}

/// Paragraphs or Text without any formatting or special contents.
pub fn is_plain_text(elems: &[Element]) -> PredResult {
    fn shallow(elements: &[Element]) -> PredResult {
        for elem in elements {
            let allowed = match *elem {
                Element::Paragraph(_)
                | Element::Text(_) => true,
                _ => false
            };
            if !allowed {
                return Err(PredError {
                    tree: Some(elem),
                    cause: "This markup is not allowed in plain text!".into(),
                });
            }
        }
        Ok(())
    }
    always(elems, &shallow)
}

fn get_template_spec(template: &Template) -> Result<TemplateSpec, PredError> {
    let name = extract_plain_text(&template.name);
    if let Some(spec) = spec_of(&name) {
        Ok(spec)
    } else {
        Err(PredError {
            tree: None,
            cause: format!("\"{}\" has no specification!", &name)
        })
    }
}
/// Certain block elements are allowed in theorems.
pub fn is_theorem_paragraph(elems: &[Element]) -> PredResult {
    fn shallow(elems: &[Element]) -> PredResult {
        for elem in elems {
            match *elem {
                Element::Template(ref template) => {
                    let spec = get_template_spec(template)?;
                    // allowed templates
                    if let Some(parsed) = parse_template(template) {
                        match parsed {
                            KnownTemplate::ProofStep { .. } => continue,
                            _ => (),
                        };
                    }
                    if spec.format != Format::Inline {
                        return Err(PredError {
                            tree: Some(elem),
                            cause: format!("\"{}\" is not an inline template!",
                                &extract_plain_text(&template.name))
                        })
                    }
                },
                Element::Gallery(_)
                | Element::Heading(_)
                | Element::Table(_)
                | Element::TableRow(_)
                | Element::TableCell(_)
                | Element::InternalReference(_)
                => return Err(PredError {
                    tree: Some(elem),
                    cause: "This markup is not allowed in proofs!".into()
                }),
                _ => (),
            }
        }
        Ok(())
    };
    always(elems, &shallow)
}

/// Pragraphs with only formatted text content (no block content).
pub fn is_text_only_paragraph(elems: &[Element]) -> PredResult {
    fn shallow(elements: &[Element]) -> PredResult {
        for elem in elements {
            match *elem {
                Element::Template(ref template) => {
                    let spec = get_template_spec(template)?;
                    if spec.format != Format::Inline {
                        return Err(PredError {
                            tree: Some(elem),
                            cause: format!("\"{}\" is not an inline template!",
                                &extract_plain_text(&template.name))
                        })
                    }
                },
                Element::Gallery(_)
                | Element::Heading(_)
                | Element::Table(_)
                | Element::TableRow(_)
                | Element::TableCell(_)
                | Element::InternalReference(_)
                => return Err(PredError {
                    tree: Some(elem),
                    cause: "This markup is not text-only!".into()
                }),
                _ => (),
            }
        }
        Ok(())
    };
    always(elems, &shallow)
}

