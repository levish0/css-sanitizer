//! Regressions for the deny-by-default redesign.

mod common;

use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::selector::{Component, SelectorList};
use css_sanitizer::{
    CssSanitizationPolicy, NodeAction, PropertyContext, RuleContext, SanitizeOptions,
    SelectorContext, StrictPolicy, ValueAction, ValueContext, clean_declaration_list_with_policy,
    clean_declaration_list_with_policy_and_options, clean_stylesheet_with_policy,
    clean_stylesheet_with_policy_and_options,
};

/// Allows `@scope`/style rules and all declarations, but drops any selector list
/// that references the `html` element — including `@scope` prelude selectors.
struct DropHtmlSelectors;

impl CssSanitizationPolicy for DropHtmlSelectors {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        match rule {
            CssRule::Scope(_) | CssRule::Style(_) => NodeAction::Continue,
            _ => NodeAction::Drop,
        }
    }

    fn visit_property(
        &self,
        _property: &mut css_sanitizer::lightningcss::properties::Property<'_>,
        _ctx: PropertyContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    fn visit_selector_list(
        &self,
        selectors: &mut SelectorList<'_>,
        _ctx: SelectorContext,
    ) -> NodeAction {
        let has_html = selectors.0.iter().any(|selector| {
            selector.iter_raw_match_order().any(|component| {
                matches!(component, Component::LocalName(name) if name.lower_name.0 == "html")
            })
        });

        if has_html {
            NodeAction::Drop
        } else {
            NodeAction::Continue
        }
    }

    fn check_url(
        &self,
        _url: &css_sanitizer::lightningcss::values::url::Url<'_>,
        _ctx: ValueContext,
    ) -> ValueAction {
        ValueAction::Allow
    }
}

#[test]
fn scope_start_selector_is_policed() {
    // The `html` in the `@scope` prelude must be subject to the selector hook;
    // before the fix it was never visited and the rule survived.
    let result = clean_stylesheet_with_policy(
        "@scope (html) to (.x) { .a { color: red } }",
        &DropHtmlSelectors,
    );
    assert!(result.is_empty(), "got: {result:?}");
}

#[test]
fn scope_end_selector_is_policed() {
    let result = clean_stylesheet_with_policy(
        "@scope (.ok) to (html) { .a { color: red } }",
        &DropHtmlSelectors,
    );
    assert!(result.is_empty(), "got: {result:?}");
}

#[test]
fn scope_with_safe_prelude_is_kept() {
    let result = clean_stylesheet_with_policy(
        "@scope (.ok) to (.limit) { .a { color: red } }",
        &DropHtmlSelectors,
    );
    assert!(result.contains("color"), "got: {result:?}");
}

#[test]
fn font_face_src_url_is_denied_by_default() {
    let result = clean_stylesheet_with_policy(
        "@font-face { font-family: x; src: url('https://evil.test/f.woff2') }",
        &StrictPolicy::new().allow_rules(&["font-face"]),
    );
    assert!(!result.contains("url("), "got: {result:?}");
    assert!(result.contains("font-family"), "got: {result:?}");
}

#[test]
fn font_face_src_url_is_kept_when_url_allowed() {
    let result = clean_stylesheet_with_policy(
        "@font-face { src: url('https://example.com/f.woff2') }",
        &StrictPolicy::new().allow_rules(&["font-face"]).allow_url(),
    );
    assert!(result.contains("url("), "got: {result:?}");
}

#[test]
fn custom_property_url_is_denied_by_default() {
    let result = clean_declaration_list_with_policy(
        "--brand: url('https://evil.test/x.png')",
        &StrictPolicy::new().allow_properties(&["--brand"]),
    );
    assert!(result.is_empty(), "got: {result:?}");
}

#[test]
fn depth_cap_drops_overly_nested_rules() {
    let mut input = String::new();
    for _ in 0..8 {
        input.push_str("@media all{");
    }
    input.push_str(".x{color:red}");
    for _ in 0..8 {
        input.push('}');
    }

    let policy = StrictPolicy::new()
        .allow_rules(&["media"])
        .allow_properties(&["color"]);

    // Default depth keeps the whole thing.
    let full = clean_stylesheet_with_policy(&input, &policy);
    assert!(full.contains("color"), "got: {full:?}");

    // A small cap drops the deeply nested content (fail-closed).
    let capped = clean_stylesheet_with_policy_and_options(
        &input,
        &policy,
        SanitizeOptions::default().with_max_depth(3),
    );
    assert!(!capped.contains("color"), "got: {capped:?}");
}

#[test]
fn container_style_query_url_is_denied_by_default() {
    // `@container style(background: url(...))` embeds a declaration in the
    // prelude; before the fix its url bypassed the value guard entirely.
    let result = clean_stylesheet_with_policy(
        "@container style(background: url(https://evil.test/c.png)) { .a { color: red } }",
        &StrictPolicy::new()
            .allow_rules(&["container"])
            .allow_properties(&["color", "background"]),
    );
    assert!(!result.contains("url("), "got: {result:?}");
    assert!(!result.contains("evil"), "got: {result:?}");
}

#[test]
fn container_style_query_is_kept_when_url_allowed() {
    let result = clean_stylesheet_with_policy(
        "@container style(background: url(https://example.com/c.png)) { .a { color: red } }",
        &StrictPolicy::new()
            .allow_rules(&["container"])
            .allow_properties(&["color", "background"])
            .allow_url(),
    );
    assert!(result.contains("color"), "got: {result:?}");
}

#[test]
fn property_initial_value_url_is_denied_by_default() {
    // `@property` `initial-value` can carry a url later fetched via `var()`.
    let result = clean_stylesheet_with_policy(
        "@property --x { syntax: '<image>'; inherits: false; initial-value: url(https://evil.test/p.png) }",
        &StrictPolicy::new().allow_rules(&["property"]),
    );
    assert!(!result.contains("url("), "got: {result:?}");
    assert!(!result.contains("evil"), "got: {result:?}");
}

#[test]
fn raw_unquoted_url_token_is_denied_by_default() {
    // Exercises the `check_token` backstop for raw url tokens.
    let result = clean_declaration_list_with_policy(
        "--brand: url(evil.png)",
        &StrictPolicy::new().allow_properties(&["--brand"]),
    );
    assert!(result.is_empty(), "got: {result:?}");
}

#[test]
fn env_is_kept_when_env_allowed() {
    let result = clean_declaration_list_with_policy(
        "padding: env(safe-area-inset-left)",
        &StrictPolicy::new()
            .allow_properties(&["padding"])
            .allow_env(),
    );
    assert!(result.contains("env("), "got: {result:?}");
}

#[test]
fn depth_cap_applies_to_nested_style_rules() {
    let mut input = String::new();
    for i in 0..8 {
        input.push_str(&format!(".l{i} {{ "));
    }
    input.push_str("color: red");
    for _ in 0..8 {
        input.push_str(" }");
    }

    let policy = StrictPolicy::new().allow_properties(&["color"]);

    let capped = clean_stylesheet_with_policy_and_options(
        &input,
        &policy,
        SanitizeOptions::default().with_max_depth(3),
    );
    assert!(!capped.contains("color"), "got: {capped:?}");
}

#[test]
fn value_guard_can_be_disabled() {
    let result = clean_declaration_list_with_policy_and_options(
        "background-image: url('x.png')",
        &StrictPolicy::new().allow_properties(&["background-image"]),
        SanitizeOptions::default().with_value_guard(false),
    );
    assert!(result.contains("url("), "got: {result:?}");
}
