use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
use css_sanitizer::lightningcss::traits::ToCss;
use css_sanitizer::{rewrite_selector_classes, rewrite_stylesheet_selector_classes};

fn main() {
    let mut stylesheet = StyleSheet::parse(
        ".notice:is(.notice, .info) { color: red } @scope (.notice) { & .notice-child { color: blue } }",
        ParserOptions::default(),
    )
    .expect("stylesheet should parse");

    rewrite_stylesheet_selector_classes(&mut stylesheet, |name| match name {
        "notice" => Some("smc-a1b2c3".to_string()),
        "notice-child" => Some("smc-child".to_string()),
        _ => None,
    });

    let stylesheet_output = stylesheet
        .to_css(Default::default())
        .expect("stylesheet should serialize")
        .code;

    let CssRule::Style(style_rule) = &mut stylesheet.rules.0[0] else {
        panic!("expected first rule to be a style rule");
    };

    rewrite_selector_classes(&mut style_rule.selectors, |name| {
        (name == "info").then(|| "smc-info".to_string())
    });

    let selector_output = style_rule
        .selectors
        .0
        .iter()
        .map(|selector| {
            selector
                .to_css_string(Default::default())
                .expect("selector should serialize")
        })
        .collect::<Vec<_>>()
        .join(", ");

    println!("Rewritten stylesheet:\n{stylesheet_output}\n");
    println!("First selector list after direct rewrite:\n{selector_output}");
}
