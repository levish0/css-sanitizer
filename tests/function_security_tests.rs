mod common;

use common::declaration_css;
use css_sanitizer::{ResourceUse, StrictPolicy};

#[test]
fn deeply_nested_resource_in_variable_fallback_is_denied() {
    let css = declaration_css(
        "width: calc(1px + var(--x, calc(2px + var(--y, url('https://example.test/a.png')))))",
        &StrictPolicy::new()
            .allow_properties(&["width"])
            .allow_variables(),
    );
    assert!(css.is_empty());
}

#[test]
fn url_spellings_are_checked_case_insensitively() {
    for input in [
        "background-image: url('a.png')",
        "background-image: URL('a.png')",
        r"background-image: u\72 l('a.png')",
    ] {
        let css = declaration_css(
            input,
            &StrictPolicy::new().allow_properties(&["background-image"]),
        );
        assert!(css.is_empty(), "resource spelling survived: {input}");
    }
}

#[test]
fn var_and_env_permissions_are_distinct_from_generic_functions() {
    let generic_only = StrictPolicy::new()
        .allow_properties(&["width"])
        .allow_functions(&["var", "env"]);
    assert!(declaration_css("width: VAR(--size, 1px)", &generic_only).is_empty());
    assert!(declaration_css("width: ENV(safe-area-inset-left, 1px)", &generic_only).is_empty());

    let variables = StrictPolicy::new()
        .allow_properties(&["width"])
        .allow_variables();
    assert!(declaration_css("width: VAR(--size, 1px)", &variables).contains("VAR("));

    let environment = StrictPolicy::new()
        .allow_properties(&["width"])
        .allow_environment_variables();
    assert!(
        declaration_css("width: ENV(safe-area-inset-left, 1px)", &environment).contains("ENV(")
    );
}

#[test]
fn unresolved_image_function_requires_both_dynamic_and_resource_permissions() {
    let input = "background-image: image-set(--asset() 1x)";
    let dynamic_only = StrictPolicy::new()
        .allow_properties(&["background-image"])
        .allow_functions(&["--asset"]);
    assert!(declaration_css(input, &dynamic_only).is_empty());

    let allowed = dynamic_only.allow_resources(&[ResourceUse::Image]);
    assert!(declaration_css(input, &allowed).contains("--asset("));
}

#[test]
fn legacy_expression_is_always_denied() {
    let policy = StrictPolicy::new()
        .allow_properties(&["width"])
        .allow_functions(&["expression", "alert"]);
    assert!(declaration_css("width: expression(alert(1))", &policy).is_empty());
}

#[test]
fn unknown_functions_are_fail_closed_and_custom_names_are_case_sensitive() {
    let denied = StrictPolicy::new().allow_properties(&["width"]);
    assert!(declaration_css("width: future-size(1px)", &denied).is_empty());

    let allowed = StrictPolicy::new()
        .allow_properties(&["width"])
        .allow_functions(&["future-size", "--Exact"]);
    assert!(declaration_css("width: future-size(1px)", &allowed).contains("future-size("));
    assert!(declaration_css("width: --exact()", &allowed).is_empty());
    assert!(declaration_css("width: --Exact()", &allowed).contains("--Exact("));
}

#[test]
fn resource_functions_cannot_be_enabled_as_generic_functions() {
    for (name, spelling) in [
        ("url", "URL"),
        ("src", "SRC"),
        ("image", "image"),
        ("image-set", "image-set"),
    ] {
        let css = format!("--asset: {spelling}('https://example.test/a')");
        let policy = StrictPolicy::new()
            .allow_properties(&["--asset"])
            .allow_functions(&[name]);
        assert!(declaration_css(&css, &policy).is_empty(), "{name} escaped");
    }
}
