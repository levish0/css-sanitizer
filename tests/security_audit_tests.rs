mod common;

use common::StrictPolicy;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::rules::font_feature_values::FontFeatureValuesRule;
use css_sanitizer::lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use css_sanitizer::lightningcss::rules::view_transition::ViewTransitionProperty;
use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
use css_sanitizer::{
    CssSanitizationPolicy, DescriptorContext, NodeAction, RuleContext,
    clean_declaration_list_with_policy, clean_stylesheet_with_policy, sanitize_stylesheet_ast,
};

fn sanitize_parsed_stylesheet(input: &str, policy: &dyn CssSanitizationPolicy) -> String {
    let mut stylesheet =
        StyleSheet::parse(input, ParserOptions::default()).expect("stylesheet should parse");
    sanitize_stylesheet_ast(&mut stylesheet, policy);
    stylesheet
        .to_css(Default::default())
        .expect("stylesheet should serialize")
        .code
        .trim()
        .to_string()
}

struct DropSpecialDescriptorsPolicy;

impl CssSanitizationPolicy for DropSpecialDescriptorsPolicy {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        match rule {
            CssRule::FontPaletteValues(_)
            | CssRule::ViewTransition(_)
            | CssRule::FontFeatureValues(_) => NodeAction::Continue,
            _ => NodeAction::Drop,
        }
    }

    fn visit_font_palette_values_property(
        &self,
        _property: &mut FontPaletteValuesProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Drop
    }

    fn visit_view_transition_property(
        &self,
        _property: &mut ViewTransitionProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Drop
    }

    fn visit_font_feature_values_rule(
        &self,
        _rule: &mut FontFeatureValuesRule<'_>,
        _ctx: RuleContext,
    ) -> NodeAction {
        NodeAction::Drop
    }
}

#[test]
fn import_rule_requires_url_permission_even_when_rule_is_whitelisted() {
    let result = clean_stylesheet_with_policy(
        "@import url('https://evil.test/a.css');",
        &StrictPolicy::new().allow_rules(&["import"]),
    );
    assert_eq!(result, "");
}

#[test]
fn string_import_is_preserved_only_when_rule_and_url_are_allowed() {
    let result = clean_stylesheet_with_policy(
        "@import \"https://example.com/safe.css\";",
        &StrictPolicy::new().allow_rules(&["import"]).allow_url(),
    );
    assert!(result.contains("@import"));
}

#[test]
fn malformed_inline_css_does_not_escape_into_new_rules() {
    let result = clean_declaration_list_with_policy(
        "color: red; } .owned { background-image: url('https://evil.test/x.png') }",
        &StrictPolicy::new().allow_properties(&["color", "background-image"]),
    );
    assert_eq!(result, "color: red");
}

#[test]
fn image_set_with_nested_url_is_blocked_without_url_permission() {
    let result = clean_declaration_list_with_policy(
        "background-image: image-set(url('https://evil.test/x.png') 1x)",
        &StrictPolicy::new().allow_properties(&["background-image"]),
    );
    assert_eq!(result, "");
}

#[test]
fn wrapper_rules_recursively_sanitize_hidden_url_payloads() {
    let result = sanitize_parsed_stylesheet(
        r#"
        @supports (display: block) {
            .supports { background-image: url("https://evil.test/supports.png"); }
        }
        @container (min-width: 10px) {
            .container { background-image: url("https://evil.test/container.png"); }
        }
        @scope (.card) {
            .scope { background-image: url("https://evil.test/scope.png"); }
        }
        @layer audit {
            .layer { background-image: url("https://evil.test/layer.png"); }
        }
        @starting-style {
            .start { background-image: url("https://evil.test/starting-style.png"); }
        }
        "#,
        &StrictPolicy::new()
            .allow_rules(&[
                "supports",
                "container",
                "scope",
                "layer-block",
                "starting-style",
            ])
            .allow_properties(&["background-image"]),
    );

    assert_eq!(result, "");
}

#[test]
fn nesting_rules_are_recursively_sanitized_and_pruned_when_empty() {
    let result = sanitize_parsed_stylesheet(
        r#"
        .card {
            & .child {
                background-image: url("https://evil.test/nesting.png");
            }
        }
        "#,
        &StrictPolicy::new().allow_properties(&["background-image"]),
    );

    assert_eq!(result, "");
}

#[test]
fn page_margin_rules_strip_nested_urls_but_keep_safe_properties() {
    let result = sanitize_parsed_stylesheet(
        r#"
        @page {
            margin: 1cm;
            @top-left {
                color: red;
                background-image: url("https://evil.test/page.png");
            }
        }
        "#,
        &StrictPolicy::new()
            .allow_rules(&["page"])
            .allow_properties(&["margin", "color", "background-image"]),
    );

    assert!(result.contains("@page"));
    assert!(result.contains("margin"));
    assert!(result.contains("color"));
    assert!(!result.contains("url("));
}

#[test]
fn viewport_rules_filter_disallowed_properties() {
    let result = sanitize_parsed_stylesheet(
        "@viewport { zoom: 1; width: device-width; }",
        &StrictPolicy::new()
            .allow_rules(&["viewport"])
            .allow_properties(&["zoom"]),
    );

    assert!(result.contains("@viewport"));
    assert!(result.contains("zoom"));
    assert!(!result.contains("width"));
}

#[test]
fn font_palette_values_rule_drops_when_descriptor_policy_removes_everything() {
    let result = sanitize_parsed_stylesheet(
        "@font-palette-values --brand { base-palette: 1; override-colors: 0 red; }",
        &DropSpecialDescriptorsPolicy,
    );

    assert_eq!(result, "");
}

#[test]
fn view_transition_rule_drops_when_descriptor_policy_removes_everything() {
    let result = sanitize_parsed_stylesheet(
        "@view-transition { navigation: auto; }",
        &DropSpecialDescriptorsPolicy,
    );

    assert_eq!(result, "");
}

#[test]
fn font_feature_values_rule_hook_can_drop_the_entire_rule() {
    let result = sanitize_parsed_stylesheet(
        "@font-feature-values Test Sans { @styleset { alt-glyphs: 1; } }",
        &DropSpecialDescriptorsPolicy,
    );

    assert_eq!(result, "");
}
