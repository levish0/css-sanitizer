mod common;

use common::{declaration, declaration_css};
use css_sanitizer::lightningcss::declaration::DeclarationBlock;
use css_sanitizer::lightningcss::printer::PrinterOptions;
use css_sanitizer::lightningcss::traits::ToCss;
use css_sanitizer::{ResourceUse, StrictPolicy, sanitize_declaration_block_ast};

#[test]
fn allows_only_explicit_properties_and_reports_drops() {
    let output = declaration(
        "color: red; position: fixed",
        &StrictPolicy::new().allow_properties(&["color"]),
    );
    assert_eq!(output.css.as_str(), "color: red");
    assert_eq!(output.report.dropped_declarations, 1);
}

#[test]
fn standard_property_configuration_is_ascii_case_insensitive() {
    let css = declaration_css(
        "COLOR: red",
        &StrictPolicy::new().allow_properties(&["COLOR"]),
    );
    assert_eq!(css, "color: red");
}

#[test]
fn custom_property_configuration_is_case_sensitive() {
    let css = declaration_css(
        "--theme: red; --THEME: blue",
        &StrictPolicy::new().allow_properties(&["--theme"]),
    );
    assert_eq!(css, "--theme: red");
}

#[test]
fn important_is_a_separate_capability() {
    let denied = declaration_css(
        "color: red !important",
        &StrictPolicy::new().allow_properties(&["color"]),
    );
    assert!(denied.is_empty());

    let allowed = declaration_css(
        "color: red !important",
        &StrictPolicy::new()
            .allow_properties(&["color"])
            .allow_important(),
    );
    assert!(allowed.contains("!important"));
}

#[test]
fn image_resources_are_a_use_specific_capability() {
    let denied = declaration_css(
        "background-image: url('https://example.test/image.png')",
        &StrictPolicy::new().allow_properties(&["background-image"]),
    );
    assert!(denied.is_empty());

    let allowed = declaration_css(
        "background-image: url('https://example.test/image.png')",
        &StrictPolicy::new()
            .allow_properties(&["background-image"])
            .allow_resources(&[ResourceUse::Image]),
    );
    assert!(allowed.contains("example.test"));
}

#[test]
fn variables_do_not_implicitly_allow_resources_in_fallbacks() {
    let denied = declaration_css(
        "background-image: var(--image, url('https://example.test/fallback.png'))",
        &StrictPolicy::new()
            .allow_properties(&["background-image"])
            .allow_variables(),
    );
    assert!(denied.is_empty());

    let allowed = declaration_css(
        "background-image: var(--image, url('https://example.test/fallback.png'))",
        &StrictPolicy::new()
            .allow_properties(&["background-image"])
            .allow_variables()
            .allow_resources(&[ResourceUse::Image]),
    );
    assert!(allowed.contains("var("));
    assert!(allowed.contains("url("));
}

#[test]
fn allowed_values_are_compared_as_parsed_css() {
    let policy = StrictPolicy::new()
        .allow_properties(&["display"])
        .allow_values("DISPLAY", &["flex", "none"]);

    assert_eq!(declaration_css("display: flex", &policy), "display: flex");
    assert!(declaration_css("display: grid", &policy).is_empty());
}

#[test]
fn typed_calculation_remains_available_without_generic_function_permission() {
    let css = declaration_css(
        "width: calc(100px + 1rem)",
        &StrictPolicy::new().allow_properties(&["width"]),
    );
    assert!(css.contains("calc("));
}

#[test]
fn ast_entry_point_updates_the_existing_tree_and_returns_a_report() {
    let mut block = DeclarationBlock::parse_string(
        "color: red; position: fixed",
        css_sanitizer::lightningcss::stylesheet::ParserOptions::default(),
    )
    .expect("declaration block should parse");

    let report = sanitize_declaration_block_ast(
        &mut block,
        &StrictPolicy::new().allow_properties(&["color"]),
    );

    let css = block
        .to_css_string(PrinterOptions::default())
        .expect("declaration block should serialize");
    assert_eq!(css, "color: red");
    assert_eq!(report.dropped_declarations, 1);
}
