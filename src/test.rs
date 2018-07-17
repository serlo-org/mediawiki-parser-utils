use util::{find_arg, extract_plain_text};

fn nop_pred<'s>(_: &'s [Element]) -> PredResult<'s> {
    Ok(())
}

const _SPEC: &str = include_str!("test_spec.yml");

#[derive(TemplateSpec)]
#[spec = "test_spec.yml"]
struct _Test;

