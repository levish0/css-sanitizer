mod common;

use common::{DropImportant, NoGlobalSelectors, declaration_css, stylesheet_css};
use css_sanitizer::lightningcss::declaration::DeclarationBlock;
use css_sanitizer::lightningcss::properties::{
    Property,
    custom::{Token, TokenList, TokenOrValue},
};
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::rules::font_feature_values::{
    FontFeatureSubrule, FontFeatureSubruleType,
};
use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
use css_sanitizer::{
    CssPolicy, NodeDecision, PropertyContext, ResourceUse, RuleContext, RuleKind, StrictPolicy,
    ValueContext, ValueDecision, sanitize_declaration_block_ast, sanitize_stylesheet_ast,
};

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

#[test]
fn constructed_tokens_cannot_hide_resources_until_serialization() {
    let mut block =
        DeclarationBlock::parse_string("background-image: unknown", ParserOptions::default())
            .unwrap();
    let Property::Unparsed(property) = &mut block.declarations[0] else {
        panic!()
    };
    property.value = TokenList(vec![
        TokenOrValue::Token(Token::Ident("url".into())),
        TokenOrValue::Token(Token::ParenthesisBlock),
        TokenOrValue::Token(Token::String("https://example.test/image.png".into())),
        TokenOrValue::Token(Token::CloseParenthesis),
    ]);

    let mut explicitly_allowed = block.clone();
    let policy = StrictPolicy::new().allow_properties(&["background-image"]);
    let report = sanitize_declaration_block_ast(&mut block, &policy);
    assert!(block.is_empty());
    assert_eq!(report.rejected_values, 1);

    sanitize_declaration_block_ast(
        &mut explicitly_allowed,
        &policy.dangerously_allow_unrestricted_resources(&[ResourceUse::Image]),
    );
    assert!(!explicitly_allowed.is_empty());
}

#[test]
fn raw_function_tokens_cannot_bypass_the_resource_hook_of_a_custom_policy() {
    struct AllowPropertiesAndTokens;
    impl CssPolicy for AllowPropertiesAndTokens {
        fn property(&self, _: &mut Property<'_>, _: PropertyContext<'_>) -> NodeDecision {
            NodeDecision::Keep
        }
        fn token(&self, _: &TokenOrValue<'_>, _: &ValueContext<'_>) -> ValueDecision {
            ValueDecision::Allow
        }
    }

    let mut block =
        DeclarationBlock::parse_string("background-image: unknown", ParserOptions::default())
            .unwrap();
    let Property::Unparsed(property) = &mut block.declarations[0] else {
        panic!()
    };
    property.value = TokenList(vec![
        TokenOrValue::Token(Token::Function("url".into())),
        TokenOrValue::Token(Token::String("https://example.test/image.png".into())),
        TokenOrValue::Token(Token::CloseParenthesis),
    ]);
    let report = sanitize_declaration_block_ast(&mut block, &AllowPropertiesAndTokens);
    assert!(block.is_empty());
    assert_eq!(report.rejected_values, 1);
}

#[test]
fn constructed_tokens_cannot_introduce_a_declaration_or_important_flag() {
    for suffix in [
        vec![
            Token::Semicolon,
            Token::Ident("position".into()),
            Token::Colon,
            Token::Ident("fixed".into()),
        ],
        vec![Token::Delim('!'), Token::Ident("important".into())],
    ] {
        let mut block =
            DeclarationBlock::parse_string("color: unknown", ParserOptions::default()).unwrap();
        let Property::Unparsed(property) = &mut block.declarations[0] else {
            panic!()
        };
        property.value = TokenList(
            std::iter::once(Token::Ident("red".into()))
                .chain(suffix)
                .map(TokenOrValue::Token)
                .collect(),
        );
        sanitize_declaration_block_ast(
            &mut block,
            &StrictPolicy::new().allow_properties(&["color"]),
        );
        assert!(block.is_empty());
    }
}
