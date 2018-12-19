//! Utility transformations.

use crate::util::{extract_plain_text, find_arg, TexChecker, TexResult};
use mediawiki_parser::transformations::*;
use mediawiki_parser::*;

/// Convert list templates to mediawiki lists.
pub fn convert_template_list(root: Element) -> TResult {
    convert_template_list_rec(root, ())
}

fn convert_template_list_rec(mut root: Element, _settings: ()) -> TResult {
    if let Element::Template(ref mut template) = root {
        let template_name = extract_plain_text(&template.name).trim().to_lowercase();
        if ["list", "liste"].contains(&template_name.as_str()) {
            let mut list_content = vec![];

            let list_type = if let Some(&Element::TemplateArgument(ref arg)) =
                find_arg(&template.content, &["type".into()])
            {
                extract_plain_text(&arg.value).to_lowercase()
            } else {
                String::new()
            };

            let item_kind = match list_type.trim() {
                "ol" | "ordered" => ListItemKind::Ordered,
                "ul" | _ => ListItemKind::Unordered,
            };

            for child in template.content.drain(..) {
                if let Element::TemplateArgument(mut arg) = child {
                    if arg.name.starts_with("item") {
                        let li = Element::ListItem(ListItem {
                            position: arg.position,
                            content: arg.value,
                            kind: item_kind,
                            depth: 1,
                        });
                        list_content.push(li);

                    // a whole sublist only wrapped by the template,
                    // -> replace template by wrapped list
                    } else if arg.name.starts_with("list") {
                        if arg.value.is_empty() {
                            continue;
                        }
                        let sublist = arg.value.remove(0);
                        return recurse_inplace(&convert_template_list_rec, sublist, ());
                    }
                }
            }

            let list = Element::List(List {
                position: template.position.to_owned(),
                content: list_content,
            });
            return recurse_inplace(&convert_template_list_rec, list, ());
        }
    }
    recurse_inplace(&convert_template_list_rec, root, ())
}

/// Normalize math formulas with texvccheck
pub fn normalize_math_formulas(mut root: Element, checker: &TexChecker) -> TResult {
    if let Element::Formatted(ref mut formatted) = root {
        if formatted.markup == MarkupType::Math {
            match check_formula(&formatted.content, &formatted.position, checker) {
                e @ Element::Text(_) => {
                    formatted.content.clear();
                    formatted.content.push(e);
                }
                e => return Ok(e),
            }
        }
    }
    recurse_inplace(&normalize_math_formulas, root, checker)
}

/// Check a Tex formula, return normalized version or error
fn check_formula(content: &[Element], position: &Span, checker: &TexChecker) -> Element {
    if content.len() != 1 {
        return Element::Error(Error {
            message: "A formula must have exactly one content element!".into(),
            position: position.clone(),
        });
    }
    let checked_formula = match content[0] {
        Element::Text(ref text) => checker.check(&text.text),
        _ => {
            return Element::Error(Error {
                message: "A formula must only have text as content!".into(),
                position: position.clone(),
            })
        }
    };
    let cause = match checked_formula {
        TexResult::Ok(content) => {
            return Element::Text(Text {
                position: position.clone(),
                text: content,
            });
        }
        TexResult::UnknownFunction(func) => format!("unknown latex function `{}`!", func),
        TexResult::SyntaxError => "latex syntax error!".into(),
        TexResult::LexingError => "latex lexer error!".into(),
        TexResult::UnknownError => "unknown latex error!".into(),
    };

    Element::Error(Error {
        message: cause,
        position: position.clone(),
    })
}
