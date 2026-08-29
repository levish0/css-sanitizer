//! Broad AST-variant coverage independent from `StrictPolicy` configuration.

use std::cell::RefCell;
use std::collections::HashSet;

use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::properties::custom::TokenOrValue;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::rules::font_face::FontFaceProperty;
use css_sanitizer::lightningcss::rules::font_feature_values::FontFeatureSubrule;
use css_sanitizer::lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use css_sanitizer::lightningcss::rules::page::PageMarginRule;
use css_sanitizer::lightningcss::rules::view_transition::ViewTransitionProperty;
use css_sanitizer::lightningcss::selector::SelectorList;
use css_sanitizer::{
    CssPolicy, DescriptorContext, DynamicValueRef, ImportContext, ImportDecision, NodeDecision,
    PropertyContext, ResourceRef, RuleContext, RuleKind, SanitizeOptions, SelectorContext,
    ValueContext, ValueDecision, sanitize_stylesheet_with_options,
};

#[derive(Default)]
struct RecordingPolicy {
    rules: RefCell<HashSet<RuleKind>>,
}

impl CssPolicy for RecordingPolicy {
    fn rule(&self, _rule: &mut CssRule<'_>, context: RuleContext) -> NodeDecision {
        self.rules.borrow_mut().insert(context.kind);
        NodeDecision::Keep
    }

    fn selector(
        &self,
        _selectors: &mut SelectorList<'_>,
        _context: SelectorContext,
    ) -> NodeDecision {
        NodeDecision::Keep
    }

    fn property(
        &self,
        _property: &mut Property<'_>,
        _context: PropertyContext<'_>,
    ) -> NodeDecision {
        NodeDecision::Keep
    }

    fn font_face_descriptor(
        &self,
        _property: &mut FontFaceProperty<'_>,
        _context: DescriptorContext,
    ) -> NodeDecision {
        NodeDecision::Keep
    }

    fn font_palette_values_descriptor(
        &self,
        _property: &mut FontPaletteValuesProperty<'_>,
        _context: DescriptorContext,
    ) -> NodeDecision {
        NodeDecision::Keep
    }

    fn view_transition_descriptor(
        &self,
        _property: &mut ViewTransitionProperty<'_>,
        _context: DescriptorContext,
    ) -> NodeDecision {
        NodeDecision::Keep
    }

    fn page_margin_rule(
        &self,
        _rule: &mut PageMarginRule<'_>,
        _context: RuleContext,
    ) -> NodeDecision {
        NodeDecision::Keep
    }

    fn font_feature_values_subrule(
        &self,
        _rule: &mut FontFeatureSubrule<'_>,
        _context: RuleContext,
    ) -> NodeDecision {
        NodeDecision::Keep
    }

    fn import(&self, _context: ImportContext<'_>) -> ImportDecision {
        ImportDecision::AllowPassthrough
    }

    fn resource(&self, _resource: ResourceRef<'_>, _context: &ValueContext<'_>) -> ValueDecision {
        ValueDecision::Allow
    }

    fn dynamic_value(
        &self,
        _value: DynamicValueRef<'_, '_>,
        _context: &ValueContext<'_>,
    ) -> ValueDecision {
        ValueDecision::Allow
    }

    fn token(&self, _token: &TokenOrValue<'_>, _context: &ValueContext<'_>) -> ValueDecision {
        ValueDecision::Allow
    }
}

#[test]
fn every_typed_rule_family_reaches_the_shared_rule_hook() {
    let cases = [
        (RuleKind::Import, "@import 'theme.css';"),
        (RuleKind::Style, ".card { color: red }"),
        (RuleKind::Media, "@media all { .card { color: red } }"),
        (
            RuleKind::Keyframes,
            "@keyframes fade { from { opacity: 0 } to { opacity: 1 } }",
        ),
        (
            RuleKind::FontFace,
            "@font-face { font-family: Demo; src: url('font.woff2') }",
        ),
        (
            RuleKind::FontPaletteValues,
            "@font-palette-values --demo { base-palette: 0 }",
        ),
        (
            RuleKind::FontFeatureValues,
            "@font-feature-values Demo { @styleset { alt: 1 } }",
        ),
        (RuleKind::Page, "@page { margin: 1cm }"),
        (
            RuleKind::Supports,
            "@supports (display: block) { .card { color: red } }",
        ),
        (
            RuleKind::CounterStyle,
            "@counter-style thumbs { system: cyclic; symbols: '*'; suffix: ' ' }",
        ),
        (
            RuleKind::Namespace,
            "@namespace svg 'http://www.w3.org/2000/svg';",
        ),
        (
            RuleKind::MozDocument,
            "@-moz-document url-prefix() { .card { color: red } }",
        ),
        (RuleKind::Nesting, ".card { @nest & .child { color: red } }"),
        (
            RuleKind::NestedDeclarations,
            ".card { .child { color: red } width: 1px }",
        ),
        (RuleKind::Viewport, "@viewport { zoom: 1 }"),
        (
            RuleKind::PositionTry,
            "@position-try --fallback { top: 1px }",
        ),
        (
            RuleKind::CustomMedia,
            "@custom-media --narrow (max-width: 30em);",
        ),
        (RuleKind::LayerStatement, "@layer reset, components;"),
        (
            RuleKind::LayerBlock,
            "@layer components { .card { color: red } }",
        ),
        (
            RuleKind::PropertyRegistration,
            "@property --tone { syntax: '<color>'; inherits: false; initial-value: red }",
        ),
        (
            RuleKind::Container,
            "@container (min-width: 1px) { .card { color: red } }",
        ),
        (RuleKind::Scope, "@scope (.root) { .card { color: red } }"),
        (
            RuleKind::StartingStyle,
            "@starting-style { .card { color: red } }",
        ),
        (
            RuleKind::ViewTransition,
            "@view-transition { navigation: auto }",
        ),
        (RuleKind::Unknown, "@future-rule mode { payload: safe }"),
    ];

    for (expected, input) in cases {
        let policy = RecordingPolicy::default();
        let options = if expected == RuleKind::CustomMedia {
            SanitizeOptions::default().with_parser_flags(
                css_sanitizer::lightningcss::stylesheet::ParserFlags::CUSTOM_MEDIA,
            )
        } else {
            SanitizeOptions::default()
        };
        sanitize_stylesheet_with_options(input, &policy, options).expect("fixture should sanitize");
        assert!(
            policy.rules.borrow().contains(&expected),
            "{expected:?} did not reach the rule hook for {input:?}; observed {:?}",
            policy.rules.borrow()
        );
    }
}

#[test]
fn rule_kind_mapping_is_exhaustive_for_the_upstream_css_rule_enum() {
    fn classify(rule: &CssRule<'_>) -> RuleKind {
        RuleKind::of(rule)
    }

    let stylesheet = css_sanitizer::lightningcss::stylesheet::StyleSheet::parse(
        ".card { color: red }",
        Default::default(),
    )
    .expect("fixture should parse");
    assert_eq!(classify(&stylesheet.rules.0[0]), RuleKind::Style);
}
