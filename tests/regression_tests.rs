//! Regressions for the deny-by-default redesign.

mod common;

use std::cell::Cell;

use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::rules::font_face::FontFaceProperty;
use css_sanitizer::lightningcss::selector::{Component, SelectorList};
use css_sanitizer::{
    CssSanitizationPolicy, DescriptorContext, NodeAction, PropertyContext, RuleContext,
    SanitizeOptions, SelectorContext, StrictPolicy, ValueAction, ValueContext, ValueLocation,
    clean_declaration_list_with_policy, clean_declaration_list_with_policy_and_options,
    clean_stylesheet_with_policy, clean_stylesheet_with_policy_and_options,
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
fn container_nested_condition_url_is_denied() {
    // `not()` and `and`/`or` operations are `#[skip_type]` in lightningcss, so the
    // condition tree is walked manually; verify urls nested inside them are caught.
    for css in [
        "@container not (style(background: url(https://evil.test/c.png))) { .a { color: red } }",
        "@container style(color: red) and style(background: url(https://evil.test/c.png)) { .a { color: red } }",
    ] {
        let result = clean_stylesheet_with_policy(
            css,
            &StrictPolicy::new()
                .allow_rules(&["container"])
                .allow_properties(&["color", "background"]),
        );
        assert!(!result.contains("evil"), "css={css} got: {result:?}");
    }
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

struct SkipCannotBypassGuard;

impl CssSanitizationPolicy for SkipCannotBypassGuard {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        if matches!(
            rule,
            CssRule::Style(_) | CssRule::FontFace(_) | CssRule::Import(_)
        ) {
            NodeAction::Skip
        } else {
            NodeAction::Drop
        }
    }

    fn visit_selector_list(
        &self,
        _selectors: &mut SelectorList<'_>,
        _ctx: SelectorContext,
    ) -> NodeAction {
        NodeAction::Skip
    }

    fn visit_property(
        &self,
        _property: &mut css_sanitizer::lightningcss::properties::Property<'_>,
        _ctx: PropertyContext,
    ) -> NodeAction {
        NodeAction::Skip
    }

    fn visit_font_face_property(
        &self,
        _property: &mut FontFaceProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Skip
    }
}

#[test]
fn skip_cannot_bypass_property_resource_guard() {
    let result = clean_stylesheet_with_policy(
        ".a { color: red; background-image: url('https://evil.test/a.png') }",
        &SkipCannotBypassGuard,
    );

    assert!(result.contains("color"), "got: {result:?}");
    assert!(!result.contains("evil.test"), "got: {result:?}");
    assert!(!result.contains("url("), "got: {result:?}");
}

#[test]
fn skip_cannot_bypass_descriptor_resource_guard() {
    let result = clean_stylesheet_with_policy(
        "@font-face { font-family: Test; src: url('https://evil.test/a.woff2') }",
        &SkipCannotBypassGuard,
    );

    assert!(result.contains("font-family"), "got: {result:?}");
    assert!(!result.contains("evil.test"), "got: {result:?}");
    assert!(!result.contains("url("), "got: {result:?}");
}

#[test]
fn skip_cannot_bypass_import_resource_guard() {
    let result = clean_stylesheet_with_policy(
        "@import 'https://evil.test/theme.css';",
        &SkipCannotBypassGuard,
    );

    assert!(result.is_empty(), "got: {result:?}");
}

#[test]
fn position_try_declarations_are_filtered_and_resource_guarded() {
    let result = clean_stylesheet_with_policy(
        "@position-try --fallback { top: 10px; background-image: url('https://evil.test/p.png') }",
        &StrictPolicy::new()
            .allow_rules(&["position-try"])
            .allow_properties(&["top", "background-image"]),
    );

    assert!(result.contains("@position-try"), "got: {result:?}");
    assert!(result.contains("top"), "got: {result:?}");
    assert!(!result.contains("evil.test"), "got: {result:?}");
}

#[derive(Default)]
struct PositionTryHookPolicy {
    property_hook_called: Cell<bool>,
    resource_location: Cell<Option<ValueLocation>>,
}

impl CssSanitizationPolicy for PositionTryHookPolicy {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        if matches!(rule, CssRule::PositionTry(_)) {
            NodeAction::Continue
        } else {
            NodeAction::Drop
        }
    }

    fn visit_property(
        &self,
        _property: &mut css_sanitizer::lightningcss::properties::Property<'_>,
        _ctx: PropertyContext,
    ) -> NodeAction {
        NodeAction::Drop
    }

    fn visit_position_try_property(
        &self,
        _property: &mut css_sanitizer::lightningcss::properties::Property<'_>,
        _ctx: PropertyContext,
    ) -> NodeAction {
        self.property_hook_called.set(true);
        NodeAction::Continue
    }

    fn check_resource(
        &self,
        _resource: css_sanitizer::ResourceRef<'_>,
        ctx: ValueContext,
    ) -> ValueAction {
        self.resource_location.set(Some(ctx.location));
        ValueAction::Allow
    }
}

#[test]
fn position_try_uses_dedicated_property_hook_and_value_location() {
    let policy = PositionTryHookPolicy::default();
    let result = clean_stylesheet_with_policy(
        "@position-try --fallback { background-image: url('fallback.png') }",
        &policy,
    );

    assert!(policy.property_hook_called.get());
    assert_eq!(
        policy.resource_location.get(),
        Some(ValueLocation::PositionTry)
    );
    assert!(result.contains("fallback.png"), "got: {result:?}");
}

#[test]
fn empty_position_try_rule_is_pruned() {
    let result = clean_stylesheet_with_policy(
        "@position-try --fallback { background-image: url('https://evil.test/p.png') }",
        &StrictPolicy::new()
            .allow_rules(&["position-try"])
            .allow_properties(&["background-image"]),
    );

    assert!(result.is_empty(), "got: {result:?}");
}

#[test]
fn strict_policy_never_allows_unknown_at_rules_by_category() {
    let result = clean_stylesheet_with_policy(
        "@future-resource 'https://evil.test/x' { payload: yes }",
        &StrictPolicy::new().allow_rules(&["unknown"]),
    );

    assert!(result.is_empty(), "got: {result:?}");
}
