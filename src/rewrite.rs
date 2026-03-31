use lightningcss::rules::CssRule;
use lightningcss::selector::{Component, Selector, SelectorList};
use lightningcss::stylesheet::StyleSheet;
use parcel_selectors::parser::NthOfSelectorData;

type RewriteFn<'a> = dyn FnMut(&str) -> Option<String> + 'a;

fn rewrite_selector_slice(selectors: &mut [Selector<'_>], rewrite: &mut RewriteFn<'_>) {
    for selector in selectors {
        rewrite_selector(selector, rewrite);
    }
}

fn rewrite_selector(selector: &mut Selector<'_>, rewrite: &mut RewriteFn<'_>) {
    for component in selector.iter_mut_raw_match_order() {
        rewrite_component(component, rewrite);
    }
}

fn rewrite_component(component: &mut Component<'_>, rewrite: &mut RewriteFn<'_>) {
    match component {
        Component::Class(name) => {
            if let Some(rewritten) = rewrite(name.0.as_ref()) {
                *name = rewritten.into();
            }
        }
        Component::Negation(selectors)
        | Component::Where(selectors)
        | Component::Is(selectors)
        | Component::Any(_, selectors)
        | Component::Has(selectors) => rewrite_selector_slice(selectors, rewrite),
        Component::Slotted(selector) => rewrite_selector(selector, rewrite),
        Component::Host(Some(selector)) => rewrite_selector(selector, rewrite),
        Component::NthOf(nth) => {
            let nth_data = *nth.nth_data();
            let mut selectors = nth.clone_selectors();
            rewrite_selector_slice(&mut selectors, rewrite);
            *nth = NthOfSelectorData::new(nth_data, selectors);
        }
        Component::Host(None)
        | Component::Combinator(_)
        | Component::ExplicitUniversalType
        | Component::ExplicitNoNamespace
        | Component::ExplicitAnyNamespace
        | Component::Namespace(_, _)
        | Component::DefaultNamespace(_)
        | Component::ID(_)
        | Component::LocalName(_)
        | Component::AttributeInNoNamespaceExists { .. }
        | Component::AttributeInNoNamespace { .. }
        | Component::AttributeOther(_)
        | Component::Root
        | Component::Empty
        | Component::Scope
        | Component::Nth(_)
        | Component::NonTSPseudoClass(_)
        | Component::Part(_)
        | Component::PseudoElement(_)
        | Component::Nesting => {}
    }
}

/// Rewrites class selectors in a selector list in place.
///
/// The callback receives each class name without a leading `.`. Returning
/// `Some` replaces the class name, and returning `None` keeps it unchanged.
pub fn rewrite_selector_classes(
    selectors: &mut SelectorList<'_>,
    mut rewrite: impl FnMut(&str) -> Option<String>,
) {
    rewrite_selector_slice(&mut selectors.0, &mut rewrite);
}

fn rewrite_rule_list_classes(rules: &mut Vec<CssRule<'_>>, rewrite: &mut RewriteFn<'_>) {
    for rule in rules {
        match rule {
            CssRule::Style(style_rule) => {
                rewrite_selector_classes(&mut style_rule.selectors, &mut *rewrite);
                rewrite_rule_list_classes(&mut style_rule.rules.0, rewrite);
            }
            CssRule::Media(rule) => rewrite_rule_list_classes(&mut rule.rules.0, rewrite),
            CssRule::Supports(rule) => rewrite_rule_list_classes(&mut rule.rules.0, rewrite),
            CssRule::MozDocument(rule) => rewrite_rule_list_classes(&mut rule.rules.0, rewrite),
            CssRule::LayerBlock(rule) => rewrite_rule_list_classes(&mut rule.rules.0, rewrite),
            CssRule::Container(rule) => rewrite_rule_list_classes(&mut rule.rules.0, rewrite),
            CssRule::StartingStyle(rule) => rewrite_rule_list_classes(&mut rule.rules.0, rewrite),
            CssRule::Scope(rule) => {
                if let Some(scope_start) = &mut rule.scope_start {
                    rewrite_selector_classes(scope_start, &mut *rewrite);
                }
                if let Some(scope_end) = &mut rule.scope_end {
                    rewrite_selector_classes(scope_end, &mut *rewrite);
                }
                rewrite_rule_list_classes(&mut rule.rules.0, rewrite);
            }
            CssRule::Nesting(rule) => {
                rewrite_selector_classes(&mut rule.style.selectors, &mut *rewrite);
                rewrite_rule_list_classes(&mut rule.style.rules.0, rewrite);
            }
            CssRule::Import(_)
            | CssRule::Keyframes(_)
            | CssRule::FontFace(_)
            | CssRule::FontPaletteValues(_)
            | CssRule::FontFeatureValues(_)
            | CssRule::Page(_)
            | CssRule::CounterStyle(_)
            | CssRule::Namespace(_)
            | CssRule::NestedDeclarations(_)
            | CssRule::Viewport(_)
            | CssRule::CustomMedia(_)
            | CssRule::LayerStatement(_)
            | CssRule::Property(_)
            | CssRule::ViewTransition(_)
            | CssRule::Ignored
            | CssRule::Unknown(_)
            | CssRule::Custom(_) => {}
        }
    }
}

/// Rewrites class selectors across a parsed stylesheet AST in place.
///
/// This walks style rule selectors, nesting selectors, nested selector
/// functions such as `:is(...)` and `:where(...)`, and `@scope` selector
/// lists. The callback receives each class name without a leading `.` and may
/// return a replacement.
pub fn rewrite_stylesheet_selector_classes(
    stylesheet: &mut StyleSheet<'_, '_>,
    mut rewrite: impl FnMut(&str) -> Option<String>,
) {
    rewrite_rule_list_classes(&mut stylesheet.rules.0, &mut rewrite);
}
