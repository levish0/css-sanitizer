use css_sanitizer::{
    ResourceUse, StrictPolicy, ValueDecision, sanitize_declaration_list, sanitize_stylesheet,
};
use url::Url;

fn cdn_policy() -> StrictPolicy {
    let trusted_origin = Url::parse("https://cdn.example.test").unwrap().origin();
    StrictPolicy::new()
        .allow_properties(&["background-image", "cursor"])
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
        })
}

#[test]
fn resource_validator_accepts_only_the_configured_parsed_origin() {
    for value in [
        "https://cdn.example.test/image.png",
        "HTTPS://CDN.EXAMPLE.TEST:443/image.png",
    ] {
        let css = format!("background-image: url('{value}')");
        assert!(
            !sanitize_declaration_list(&css, &cdn_policy())
                .unwrap()
                .css
                .is_empty()
        );
    }
    for value in [
        "https://other.example.test/image.png",
        "https://cdn.example.test.other.test/image.png",
        "https://cdn.example.test@other.test/image.png",
        "https://user:password@cdn.example.test/image.png",
        "https://cdn.example.test:444/image.png",
        "http://cdn.example.test/image.png",
        "//cdn.example.test/image.png",
        "/image.png",
        "data:image/svg+xml,example",
        "javascript:example",
    ] {
        let css = format!("background-image: url('{value}')");
        let output = sanitize_declaration_list(&css, &cdn_policy()).unwrap();
        assert!(output.css.is_empty(), "unexpectedly allowed {value}");
        assert_eq!(output.report.rejected_values, 1);
    }
}

#[test]
fn resource_validator_cannot_grant_other_uses_or_imports() {
    let policy = cdn_policy().allow_resources(&[ResourceUse::Image], |_, _| ValueDecision::Allow);
    assert!(
        sanitize_declaration_list("cursor: url('cursor.cur'), auto", &policy)
            .unwrap()
            .css
            .is_empty()
    );
    assert!(
        sanitize_stylesheet("@import 'https://cdn.example.test/theme.css';", &policy)
            .unwrap()
            .css
            .is_empty()
    );
    let no_resource_capability = StrictPolicy::new()
        .allow_properties(&["background-image"])
        .allow_resources(&[], |_, _| ValueDecision::Allow);
    assert!(
        sanitize_declaration_list(
            "background-image: url('image.png')",
            &no_resource_capability
        )
        .unwrap()
        .css
        .is_empty()
    );
}

#[test]
fn resource_validator_rejects_nested_and_dynamic_resource_references() {
    let policy = cdn_policy().allow_variables(&["background-image"]);
    for input in [
        "background-image: image-set(url('https://other.test/a.png') 1x)",
        "background-image: image-set('https://other.test/a.png' 1x)",
        "background-image: SRC('https://other.test/a.png')",
        "background-image: src(var(--image))",
    ] {
        assert!(
            sanitize_declaration_list(input, &policy)
                .unwrap()
                .css
                .is_empty(),
            "{input}"
        );
    }
}

#[test]
fn validators_are_independent_per_resource_use_and_policy_clone() {
    let image_policy = cdn_policy();
    let policy =
        image_policy
            .clone()
            .allow_resources(&[ResourceUse::Cursor], |resource, context| {
                assert_eq!(context.property.as_ref().unwrap().name(), "cursor");
                if resource.value == Some("https://cursor.example.test/arrow.cur") {
                    ValueDecision::Allow
                } else {
                    ValueDecision::Deny
                }
            });
    let input = "background-image: url('https://cdn.example.test/card.png'); cursor: url('https://cursor.example.test/arrow.cur'), auto";
    let output = sanitize_declaration_list(input, &policy).unwrap();
    assert_eq!(output.report.rejected_values, 0);
    assert!(output.css.as_str().contains("cursor:"));
    assert!(output.css.as_str().contains("background-image:"));
    assert_eq!(
        sanitize_declaration_list(input, &image_policy)
            .unwrap()
            .report
            .rejected_values,
        1
    );

    let replaced = policy.allow_resources(&[ResourceUse::Image], |_, _| ValueDecision::Deny);
    let output = sanitize_declaration_list(input, &replaced).unwrap();
    assert_eq!(output.report.rejected_values, 1);
    assert!(!output.css.as_str().contains("background-image:"));
    assert!(output.css.as_str().contains("cursor:"));
}
