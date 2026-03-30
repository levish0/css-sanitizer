mod common;

use common::FunctionSecurityPolicy;
use css_sanitizer::clean_declaration_list_with_policy;

#[test]
fn var_with_url_fallback_is_blocked_when_url_is_not_allowed() {
    let result = clean_declaration_list_with_policy(
        "background-image: var(--theme-bg, url(evil.png))",
        &FunctionSecurityPolicy::new()
            .allow_properties(&["background-image"])
            .allow_var(),
    );
    assert!(result.is_empty());
}

#[test]
fn var_with_url_fallback_is_allowed_when_both_are_allowed() {
    let result = clean_declaration_list_with_policy(
        "background-image: var(--theme-bg, url(good.png))",
        &FunctionSecurityPolicy::new()
            .allow_properties(&["background-image"])
            .allow_var()
            .allow_url(),
    );
    assert!(result.contains("var("));
    assert!(result.contains("url("));
}

#[test]
fn deeply_nested_url_in_var_fallback_is_blocked() {
    let result = clean_declaration_list_with_policy(
        "width: calc(1px + var(--x, calc(2px + var(--y, url(evil.png)))))",
        &FunctionSecurityPolicy::new()
            .allow_properties(&["width"])
            .allow_var(),
    );
    assert!(result.is_empty());
}

#[test]
fn url_inside_other_allowed_function_is_blocked_without_url_permission() {
    let result = clean_declaration_list_with_policy(
        "width: min(100px, url(bad.png))",
        &FunctionSecurityPolicy::new().allow_properties(&["width"]),
    );
    assert!(result.is_empty());
}

#[test]
fn url_forms_are_blocked_case_insensitively() {
    for css in [
        "background-image: url('test.png')",
        "background-image: URL(test.png)",
        "background: Url(test.png)",
    ] {
        let result = clean_declaration_list_with_policy(
            css,
            &FunctionSecurityPolicy::new().allow_properties(&["background-image", "background"]),
        );
        assert!(result.is_empty(), "{css} should be blocked");
    }
}

#[test]
fn env_with_url_fallback_is_blocked_without_url_permission() {
    let result = clean_declaration_list_with_policy(
        "padding: env(--safe-area-inset-left, url(bad.png))",
        &FunctionSecurityPolicy::new()
            .allow_properties(&["padding"])
            .allow_env(),
    );
    assert!(result.is_empty());
}

#[test]
fn var_without_fallback_is_allowed_when_var_is_allowed() {
    let result = clean_declaration_list_with_policy(
        "color: var(--theme-color)",
        &FunctionSecurityPolicy::new()
            .allow_properties(&["color"])
            .allow_var(),
    );
    assert!(result.contains("var("));
}

#[test]
fn var_with_expression_fallback_is_blocked() {
    let result = clean_declaration_list_with_policy(
        "width: var(--spacing, expression(1+1))",
        &FunctionSecurityPolicy::new()
            .allow_properties(&["width"])
            .allow_var(),
    );
    assert!(result.is_empty());
}
