use crate::util::{extract_plain_text, find_arg};
use mwparser_utils_derive::template_spec;

const _SPEC: &str = include_str!("test_spec.yml");

fn nop_pred<'s>(_: &'s [Element]) -> PredResult<'s> {
    Ok(())
}

template_spec!("src/test_spec.yml");
