use css_sanitizer::{ResourceUse, StrictPolicy, ValueDecision, sanitize_declaration_list};
use url::Url;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let trusted_origin = Url::parse("https://cdn.example.test")?.origin();
    let policy = StrictPolicy::new()
        .allow_properties(&["color", "background-image"])
        .allow_variables(&["color"])
        .allow_resources(&[ResourceUse::Image], move |resource, _context| {
            let Some(url) = resource.value.and_then(|value| Url::parse(value).ok()) else {
                return ValueDecision::Deny;
            };
            if url.origin() == trusted_origin
                && url.username().is_empty()
                && url.password().is_none()
            {
                ValueDecision::Allow
            } else {
                ValueDecision::Deny
            }
        });

    let output = sanitize_declaration_list(
        "color: var(--text-color); background-image: url('https://cdn.example.test/card.png'); broken",
        &policy,
    )?;
    println!("CSS: {}", output.css.as_str());
    println!(
        "HTML style attribute: {}",
        output.css.to_style_attribute_value()
    );
    for warning in &output.report.parse_warnings {
        println!(
            "Parser warning at {:?}:{:?}: {}",
            warning.line, warning.column, warning.message
        );
    }
    Ok(())
}
