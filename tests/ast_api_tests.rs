mod common;

use common::{DropImportant, DropImports, NoGlobalSelectors};
use css_sanitizer::lightningcss::rules::font_feature_values::{
    FontFeatureSubrule, FontFeatureSubruleType,
};
use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
use css_sanitizer::{
    CssSanitizationPolicy, NodeAction, RuleContext, clean_declaration_list_with_policy,
    clean_stylesheet_with_policy, sanitize_stylesheet_ast,
};

struct DropSwashSubrules;

impl CssSanitizationPolicy for DropSwashSubrules {
    fn visit_font_feature_values_subrule(
        &self,
        subrule: &mut FontFeatureSubrule<'_>,
        _ctx: RuleContext,
    ) -> NodeAction {
        if matches!(subrule.name, FontFeatureSubruleType::Swash) {
            NodeAction::Drop
        } else {
            NodeAction::Continue
        }
    }
}

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
    let result =
        clean_declaration_list_with_policy("color: red !important; width: 10px", &DropImportant);
    assert!(!result.contains("!important"));
    assert!(result.contains("width"));
}

#[test]
fn custom_policy_can_filter_font_feature_values_subrules() {
    let result = clean_stylesheet_with_policy(
        "@font-feature-values Demo { @styleset { alt: 1; } @swash { fancy: 2; } }",
        &DropSwashSubrules,
    );

    assert!(result.contains("@font-feature-values"));
    assert!(result.contains("@styleset"));
    assert!(!result.contains("@swash"));
}

#[test]
fn custom_policy_drops_empty_font_feature_values_rule_after_filtering_subrules() {
    let result = clean_stylesheet_with_policy(
        "@font-feature-values Demo { @swash { fancy: 2; } }",
        &DropSwashSubrules,
    );

    assert!(result.is_empty());
}
