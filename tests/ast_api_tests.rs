mod common;

use common::{DropImportant, NoGlobalSelectors, declaration_css, stylesheet_css};
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::rules::font_feature_values::{
    FontFeatureSubrule, FontFeatureSubruleType,
};
use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
use css_sanitizer::{CssPolicy, NodeDecision, RuleContext, RuleKind, sanitize_stylesheet_ast};

struct DropSwashSubrules;

impl CssPolicy for DropSwashSubrules {
    fn rule(&self, _rule: &mut CssRule<'_>, context: RuleContext) -> NodeDecision {
        if context.kind == RuleKind::FontFeatureValues {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn font_feature_values_subrule(
        &self,
        subrule: &mut FontFeatureSubrule<'_>,
        _ctx: RuleContext,
    ) -> NodeDecision {
        if matches!(subrule.name, FontFeatureSubruleType::Swash) {
            NodeDecision::Drop
        } else {
            NodeDecision::Keep
        }
    }
}

#[test]
fn custom_policy_can_drop_selector_lists() {
    let css = stylesheet_css(
        "html { color: red } .card { color: blue }",
        &NoGlobalSelectors,
    );
    assert!(!css.contains("html"));
    assert!(css.contains(".card"));
}

#[test]
fn custom_policy_can_filter_important_declarations() {
    let css = declaration_css("color: red !important; width: 10px", &DropImportant);
    assert!(!css.contains("!important"));
    assert!(css.contains("width"));
}

#[test]
fn ast_policy_receives_typed_rule_kinds() {
    let mut stylesheet = StyleSheet::parse(
        "@import url('https://example.test/a.css'); .card { color: blue }",
        ParserOptions::default(),
    )
    .expect("stylesheet should parse");

    let report = sanitize_stylesheet_ast(&mut stylesheet, &NoGlobalSelectors);
    let css = stylesheet
        .to_css(Default::default())
        .expect("stylesheet should serialize")
        .code;
    assert!(!css.contains("@import"));
    assert!(css.contains(".card"));
    assert_eq!(report.dropped_rules, 1);
}

#[test]
fn custom_policy_can_filter_font_feature_subrules() {
    let css = stylesheet_css(
        "@font-feature-values Demo { @styleset { alt: 1; } @swash { fancy: 2; } }",
        &DropSwashSubrules,
    );
    assert!(css.contains("@styleset"));
    assert!(!css.contains("@swash"));

    let empty = stylesheet_css(
        "@font-feature-values Demo { @swash { fancy: 2; } }",
        &DropSwashSubrules,
    );
    assert!(empty.is_empty());
}
