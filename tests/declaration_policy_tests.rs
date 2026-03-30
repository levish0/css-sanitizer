mod common;

use common::StrictPolicy;
use css_sanitizer::lightningcss::declaration::DeclarationBlock;
use css_sanitizer::lightningcss::printer::PrinterOptions;
use css_sanitizer::lightningcss::traits::ToCss;
use css_sanitizer::{
    clean_declaration_list_with_policy, sanitize_declaration_block_ast,
};

#[test]
fn allows_whitelisted_property() {
    let result = clean_declaration_list_with_policy(
        "color: red",
        &StrictPolicy::new().allow_properties(&["color"]),
    );
    assert_eq!(result, "color: red");
}

#[test]
fn strips_non_whitelisted_property() {
    let result = clean_declaration_list_with_policy(
        "color: red; position: fixed",
        &StrictPolicy::new().allow_properties(&["color"]),
    );
    assert_eq!(result, "color: red");
}

#[test]
fn blocks_important_by_default() {
    let result = clean_declaration_list_with_policy(
        "color: red !important",
        &StrictPolicy::new().allow_properties(&["color"]),
    );
    assert_eq!(result, "");
}

#[test]
fn allows_important_when_enabled() {
    let result = clean_declaration_list_with_policy(
        "color: red !important",
        &StrictPolicy::new().allow_properties(&["color"]).allow_important(),
    );
    assert!(result.contains("!important"));
}

#[test]
fn blocks_url_by_default() {
    let result = clean_declaration_list_with_policy(
        "background-image: url('http://evil.com/img.png')",
        &StrictPolicy::new().allow_properties(&["background-image"]),
    );
    assert_eq!(result, "");
}

#[test]
fn allows_url_when_enabled() {
    let result = clean_declaration_list_with_policy(
        "background-image: url('http://example.com/img.png')",
        &StrictPolicy::new()
            .allow_properties(&["background-image"])
            .allow_url(),
    );
    assert!(result.contains("url("));
}

#[test]
fn blocks_var_by_default() {
    let result = clean_declaration_list_with_policy(
        "color: var(--theme-color)",
        &StrictPolicy::new().allow_properties(&["color"]),
    );
    assert_eq!(result, "");
}

#[test]
fn allows_var_when_enabled() {
    let result = clean_declaration_list_with_policy(
        "color: var(--theme-color)",
        &StrictPolicy::new().allow_properties(&["color"]).allow_var(),
    );
    assert!(result.contains("var("));
}

#[test]
fn blocks_var_with_url_fallback_without_url_permission() {
    let result = clean_declaration_list_with_policy(
        "background-image: var(--bg, url(evil.png))",
        &StrictPolicy::new()
            .allow_properties(&["background-image"])
            .allow_var(),
    );
    assert_eq!(result, "");
}

#[test]
fn blocks_expression() {
    let result = clean_declaration_list_with_policy(
        "width: expression(document.body.clientWidth)",
        &StrictPolicy::new().allow_properties(&["width"]),
    );
    assert_eq!(result, "");
}

#[test]
fn allows_restricted_value() {
    let result = clean_declaration_list_with_policy(
        "display: flex",
        &StrictPolicy::new()
            .allow_properties(&["display"])
            .allow_values("display", &["block", "inline", "flex", "none"]),
    );
    assert_eq!(result, "display: flex");
}

#[test]
fn blocks_restricted_value() {
    let result = clean_declaration_list_with_policy(
        "display: grid",
        &StrictPolicy::new()
            .allow_properties(&["display"])
            .allow_values("display", &["block", "inline", "flex", "none"]),
    );
    assert_eq!(result, "");
}

#[test]
fn calc_passes_without_special_function_allowlist() {
    let result = clean_declaration_list_with_policy(
        "width: calc(100px + 1rem)",
        &StrictPolicy::new().allow_properties(&["width"]),
    );
    assert!(result.contains("calc("));
}

#[test]
fn sanitize_declaration_block_ast_updates_existing_ast() {
    let mut block = DeclarationBlock::parse_string(
        "color: red; position: fixed",
        css_sanitizer::lightningcss::stylesheet::ParserOptions::default(),
    )
    .expect("declaration block should parse");

    sanitize_declaration_block_ast(&mut block, &StrictPolicy::new().allow_properties(&["color"]));

    let result = block
        .to_css_string(PrinterOptions::default())
        .expect("block should serialize");
    assert_eq!(result, "color: red");
}
