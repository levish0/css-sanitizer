use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
use css_sanitizer::{rewrite_selector_classes, rewrite_stylesheet_selector_classes};

fn parse_stylesheet(input: &str) -> StyleSheet<'_, '_> {
    StyleSheet::parse(input, ParserOptions::default()).expect("stylesheet should parse")
}

fn serialize_stylesheet(stylesheet: &StyleSheet<'_, '_>) -> String {
    stylesheet
        .to_css(Default::default())
        .expect("stylesheet should serialize")
        .code
}

#[test]
fn rewrite_stylesheet_selector_classes_updates_style_rule_selectors() {
    let mut stylesheet = parse_stylesheet(".notice, div.notice > .child { color: red }");

    rewrite_stylesheet_selector_classes(&mut stylesheet, |name| {
        (name == "notice").then(|| "smc-a1b2c3".to_string())
    });

    let output = serialize_stylesheet(&stylesheet);
    assert!(output.contains(".smc-a1b2c3"));
    assert!(!output.contains(".notice"));
}

#[test]
fn rewrite_stylesheet_selector_classes_updates_nested_selector_functions() {
    let mut stylesheet = parse_stylesheet(
        ".card:is(.notice, .info):where(.notice):not(.notice):has(.notice):nth-child(2n of .notice) { color: red }",
    );

    rewrite_stylesheet_selector_classes(&mut stylesheet, |name| {
        (name == "notice").then(|| "smc-a1b2c3".to_string())
    });

    let output = serialize_stylesheet(&stylesheet);
    assert!(output.contains(".smc-a1b2c3"));
    assert!(!output.contains(".notice"));
    assert!(output.contains(":is("));
    assert!(output.contains(":has("));
    assert!(output.contains(":nth-child("));
}

#[test]
fn rewrite_stylesheet_selector_classes_updates_nesting_and_scope_selectors() {
    let mut stylesheet = parse_stylesheet(
        r#"
        @scope (.notice) to (.notice-end) {
            .notice {
                & .notice-child {
                    color: red;
                }
            }
        }
        "#,
    );

    rewrite_stylesheet_selector_classes(&mut stylesheet, |name| match name {
        "notice" => Some("smc-root".to_string()),
        "notice-end" => Some("smc-end".to_string()),
        "notice-child" => Some("smc-child".to_string()),
        _ => None,
    });

    let output = serialize_stylesheet(&stylesheet);
    assert!(output.contains("@scope (.smc-root) to (.smc-end)"));
    assert!(output.contains(".smc-root"));
    assert!(output.contains(".smc-child"));
    assert!(!output.contains(".notice"));
}

#[test]
fn rewrite_selector_classes_can_operate_on_selector_list_directly() {
    let mut stylesheet = parse_stylesheet(".notice:hover, .info { color: red }");
    let CssRule::Style(style_rule) = &mut stylesheet.rules.0[0] else {
        panic!("expected a style rule");
    };

    rewrite_selector_classes(&mut style_rule.selectors, |name| {
        (name == "notice").then(|| "smc-a1b2c3".to_string())
    });

    let output = serialize_stylesheet(&stylesheet);
    assert!(output.contains(".smc-a1b2c3:hover"));
    assert!(output.contains(".info"));
    assert!(!output.contains(".notice"));
}
