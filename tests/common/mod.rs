#![allow(dead_code)]

use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::selector::{Component, SelectorList};
use css_sanitizer::{
    CssPolicy, NodeDecision, PropertyContext, RuleContext, RuleKind, SanitizeOutput,
    SelectorContext, StrictPolicy, ValueContext, ValueDecision, sanitize_declaration_list,
    sanitize_stylesheet,
};

pub fn declaration(input: &str, policy: &dyn CssPolicy) -> SanitizeOutput {
    sanitize_declaration_list(input, policy).expect("declaration list should sanitize")
}

pub fn declaration_css(input: &str, policy: &dyn CssPolicy) -> String {
    declaration(input, policy).css.into_string()
}

pub fn stylesheet(input: &str, policy: &dyn CssPolicy) -> SanitizeOutput {
    sanitize_stylesheet(input, policy).expect("stylesheet should sanitize")
}

pub fn stylesheet_css(input: &str, policy: &dyn CssPolicy) -> String {
    stylesheet(input, policy).css.into_string()
}

pub fn style_policy(properties: &[&str]) -> StrictPolicy {
    StrictPolicy::new()
        .allow_unscoped_selectors()
        .allow_properties(properties)
}

/// Test-only policy that retains all typed style declarations but no resources
/// or dynamic substitutions.
pub struct NoGlobalSelectors;

impl CssPolicy for NoGlobalSelectors {
    fn rule(&self, _rule: &mut CssRule<'_>, context: RuleContext) -> NodeDecision {
        if matches!(context.kind, RuleKind::Style | RuleKind::Scope) {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn property(&self, _property: &mut Property<'_>, _ctx: PropertyContext<'_>) -> NodeDecision {
        NodeDecision::Keep
    }

    fn selector(&self, selectors: &mut SelectorList<'_>, _ctx: SelectorContext) -> NodeDecision {
        let has_html = selectors.0.iter().any(|selector| {
            selector.iter_raw_match_order().any(|component| {
                matches!(component, Component::LocalName(name) if name.lower_name.0 == "html")
            })
        });
        if has_html {
            NodeDecision::Drop
        } else {
            NodeDecision::Keep
        }
    }

    fn token(
        &self,
        _token: &css_sanitizer::lightningcss::properties::custom::TokenOrValue<'_>,
        _ctx: &ValueContext<'_>,
    ) -> ValueDecision {
        ValueDecision::Allow
    }
}

pub struct DropImportant;

impl CssPolicy for DropImportant {
    fn property(&self, _property: &mut Property<'_>, context: PropertyContext<'_>) -> NodeDecision {
        if context.important {
            NodeDecision::Drop
        } else {
            NodeDecision::Keep
        }
    }

    fn token(
        &self,
        _token: &css_sanitizer::lightningcss::properties::custom::TokenOrValue<'_>,
        _ctx: &ValueContext<'_>,
    ) -> ValueDecision {
        ValueDecision::Allow
    }
}
