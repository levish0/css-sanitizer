use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::selector::SelectorList;
use css_sanitizer::{
    CssSanitizationPolicy, NodeAction, PropertyContext, RuleContext, SelectorContext,
    clean_declaration_list_with_policy, clean_stylesheet_with_policy,
};

struct DemoPolicy;

impl DemoPolicy {
    // Only the property name and `!important` are decided here. Values such as
    // `url()`/`expression()` are handled by the engine-enforced value guard,
    // whose `check_*` hooks are deny-by-default and are not overridden below.
    fn allow_property(property: &Property<'_>, important: bool) -> bool {
        if important {
            return false;
        }

        let property_id = property.property_id();
        matches!(
            property_id.name(),
            "color" | "background-color" | "font-size"
        )
    }
}

impl CssSanitizationPolicy for DemoPolicy {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        match rule {
            CssRule::Style(_) => NodeAction::Continue,
            _ => NodeAction::Drop,
        }
    }

    fn visit_selector_list(
        &self,
        _selectors: &mut SelectorList<'_>,
        _ctx: SelectorContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    fn visit_property(&self, property: &mut Property<'_>, ctx: PropertyContext) -> NodeAction {
        if Self::allow_property(property, ctx.important) {
            NodeAction::Continue
        } else {
            NodeAction::Drop
        }
    }
}

fn main() {
    let inline_input =
        "color: red; position: fixed; background-image: url(evil.png); font-size: 14px";
    let inline_output = clean_declaration_list_with_policy(inline_input, &DemoPolicy);

    let stylesheet_input = r#"
        @import url("evil.css");
        .card {
            color: red;
            position: fixed;
            background-color: white !important;
            font-size: 14px;
        }
    "#;
    let stylesheet_output = clean_stylesheet_with_policy(stylesheet_input, &DemoPolicy);

    println!("Inline input:\n{inline_input}\n");
    println!("Inline output:\n{inline_output}\n");
    println!("Stylesheet input:\n{stylesheet_input}\n");
    println!("Stylesheet output:\n{stylesheet_output}");
}
