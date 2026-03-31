use std::collections::BTreeSet;

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

#[test]
fn rewrite_stylesheet_selector_classes_updates_shadow_dom_selector_functions() {
    let mut stylesheet =
        parse_stylesheet(":host(.notice), :host, ::slotted(.notice) { color: red }");

    rewrite_stylesheet_selector_classes(&mut stylesheet, |name| {
        (name == "notice").then(|| "smc-shadow".to_string())
    });

    let output = serialize_stylesheet(&stylesheet);
    assert!(output.contains(":host(.smc-shadow)"));
    assert!(output.contains("::slotted(.smc-shadow)"));
    assert!(output.contains(":host,"));
    assert!(!output.contains(".notice"));
}

#[test]
fn rewrite_stylesheet_selector_classes_recurses_through_wrapper_rules() {
    let mut stylesheet = parse_stylesheet(
        r#"
        @media (min-width: 10px) {
            .media-target { color: red; }
        }
        @supports (display: grid) {
            .supports-target { color: red; }
        }
        @container card (min-width: 10px) {
            .container-target { color: red; }
        }
        @layer demo {
            .layer-target { color: red; }
        }
        @starting-style {
            .start-target { color: red; }
        }
        @scope (.scope-root) to (.scope-end) {
            .scope-target { color: red; }
        }
        "#,
    );

    let mut seen = Vec::new();
    rewrite_stylesheet_selector_classes(&mut stylesheet, |name| {
        seen.push(name.to_string());
        Some(format!("smc-{name}"))
    });

    let output = serialize_stylesheet(&stylesheet);
    for original in [
        "media-target",
        "supports-target",
        "container-target",
        "layer-target",
        "start-target",
        "scope-root",
        "scope-end",
        "scope-target",
    ] {
        assert!(!output.contains(&format!(".{original}")));
        assert!(output.contains(&format!(".smc-{original}")));
    }

    let seen = seen.into_iter().collect::<BTreeSet<_>>();
    let expected = [
        "container-target",
        "layer-target",
        "media-target",
        "scope-end",
        "scope-root",
        "scope-target",
        "start-target",
        "supports-target",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    assert_eq!(seen, expected);
}

#[test]
fn rewrite_selector_classes_visits_nested_classes_in_direct_api() {
    let mut stylesheet = parse_stylesheet(
        ":host(.hosted), ::slotted(.slot), .card:is(.notice, .info):where(.notice):not(.warning):has(.notice):nth-child(2n of .notice) { color: red }",
    );
    let CssRule::Style(style_rule) = &mut stylesheet.rules.0[0] else {
        panic!("expected a style rule");
    };

    let mut seen = Vec::new();
    rewrite_selector_classes(&mut style_rule.selectors, |name| {
        seen.push(name.to_string());
        Some(format!("smc-{name}"))
    });

    let output = serialize_stylesheet(&stylesheet);
    for original in ["card", "hosted", "slot", "notice", "info", "warning"] {
        assert!(!output.contains(&format!(".{original}")));
        assert!(output.contains(&format!(".smc-{original}")));
    }

    let seen = seen.into_iter().collect::<BTreeSet<_>>();
    let expected = ["card", "hosted", "slot", "notice", "info", "warning"]
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    assert_eq!(seen, expected);
}
