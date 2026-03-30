mod common;

use common::{DropImportant, DropImports, NoGlobalSelectors};
use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
use css_sanitizer::{
    clean_declaration_list_with_policy, clean_stylesheet_with_policy, sanitize_stylesheet_ast,
};

#[test]
fn custom_policy_can_drop_selector_lists() {
    let result = clean_stylesheet_with_policy(
        "html { color: red } .card { color: blue }",
        &NoGlobalSelectors,
    );
    assert!(!result.contains("html"));
    assert!(result.contains(".card"));
}

#[test]
fn custom_policy_can_drop_rules_on_parsed_ast() {
    let mut stylesheet = StyleSheet::parse(
        "@import url('evil.css'); .card { color: blue }",
        ParserOptions::default(),
    )
    .expect("stylesheet should parse");

    sanitize_stylesheet_ast(&mut stylesheet, &DropImports);

    let result = stylesheet
        .to_css(Default::default())
        .expect("stylesheet should serialize")
        .code;
    assert!(!result.contains("@import"));
    assert!(result.contains(".card"));
}

#[test]
fn custom_policy_can_drop_important_declarations() {
    let result = clean_declaration_list_with_policy(
        "color: red !important; width: 10px",
        &DropImportant,
    );
    assert!(!result.contains("!important"));
    assert!(result.contains("width"));
}
