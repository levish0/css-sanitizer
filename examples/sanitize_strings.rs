use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::{
    clean_declaration_list_with_policy, clean_stylesheet_with_policy, CssSanitizationPolicy,
    NodeAction, PropertyContext, RuleContext,
};

struct DemoPolicy;

impl DemoPolicy {
    fn allow_property(property: &Property<'_>, important: bool) -> bool {
        if important {
            return false;
        }

        let property_id = property.property_id();
        let name = property_id.name();
        if !matches!(name, "color" | "background-color" | "font-size") {
            return false;
        }

        let css = property
            .to_css_string(important, Default::default())
            .unwrap_or_default()
            .to_ascii_lowercase();

        !css.contains("url(") && !css.contains("expression(")
    }
}

impl CssSanitizationPolicy for DemoPolicy {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        match rule {
            CssRule::Style(_) => NodeAction::Continue,
            _ => NodeAction::Drop,
        }
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
