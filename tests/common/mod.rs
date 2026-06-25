#![allow(dead_code)]

use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::selector::{Component, SelectorList};
use css_sanitizer::{
    CssSanitizationPolicy, NodeAction, PropertyContext, RuleContext, SelectorContext,
};

/// The built-in strict allowlist policy, re-exported for tests.
pub use css_sanitizer::StrictPolicy;

/// In the deny-by-default model `StrictPolicy` already gates `url()`, `var()`,
/// and `env()` via its value-guard hooks, so the function-security tests reuse
/// it directly.
pub type FunctionSecurityPolicy = StrictPolicy;

/// Allows normal style rules and their declarations, but drops any rule whose
/// selector list references the global `html` element.
pub struct NoGlobalSelectors;

impl CssSanitizationPolicy for NoGlobalSelectors {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        if matches!(rule, CssRule::Style(_)) {
            NodeAction::Continue
        } else {
            NodeAction::Drop
        }
    }

    fn visit_property(&self, _property: &mut Property<'_>, _ctx: PropertyContext) -> NodeAction {
        NodeAction::Continue
    }

    fn visit_selector_list(
        &self,
        selectors: &mut SelectorList<'_>,
        _ctx: SelectorContext,
    ) -> NodeAction {
        let has_global_html_selector = selectors.0.iter().any(|selector| {
            selector.iter_raw_match_order().any(|component| {
                matches!(
                    component,
                    Component::LocalName(name) if name.lower_name.0 == "html"
                )
            })
        });

        if has_global_html_selector {
            NodeAction::Drop
        } else {
            NodeAction::Continue
        }
    }
}

/// Allows normal style rules and their declarations, but drops `@import`.
pub struct DropImports;

impl CssSanitizationPolicy for DropImports {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        match rule {
            CssRule::Import(_) => NodeAction::Drop,
            CssRule::Style(_) => NodeAction::Continue,
            _ => NodeAction::Drop,
        }
    }

    fn visit_property(&self, _property: &mut Property<'_>, _ctx: PropertyContext) -> NodeAction {
        NodeAction::Continue
    }

    fn visit_selector_list(
        &self,
        _selectors: &mut SelectorList<'_>,
        _ctx: SelectorContext,
    ) -> NodeAction {
        NodeAction::Continue
    }
}

/// Allows declarations, but drops `!important` ones.
pub struct DropImportant;

impl CssSanitizationPolicy for DropImportant {
    fn visit_property(&self, _property: &mut Property<'_>, ctx: PropertyContext) -> NodeAction {
        if ctx.important {
            NodeAction::Drop
        } else {
            NodeAction::Continue
        }
    }
}
