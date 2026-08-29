use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::selector::SelectorList;
use css_sanitizer::{
    CssPolicy, NodeDecision, PropertyContext, RuleContext, RuleKind, SelectorContext,
    sanitize_declaration_list, sanitize_stylesheet,
};

struct ColorOnly;

impl CssPolicy for ColorOnly {
    fn rule(&self, _rule: &mut CssRule<'_>, context: RuleContext) -> NodeDecision {
        if context.kind == RuleKind::Style {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn selector(
        &self,
        _selectors: &mut SelectorList<'_>,
        _context: SelectorContext,
    ) -> NodeDecision {
        NodeDecision::Keep
    }

    fn property(&self, _property: &mut Property<'_>, context: PropertyContext<'_>) -> NodeDecision {
        if context.key.name() == "color" && !context.important {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let declarations =
        sanitize_declaration_list("color: rebeccapurple; position: fixed", &ColorOnly)?;
    println!("{}", declarations.css);
    println!(
        "dropped declarations: {}",
        declarations.report.dropped_declarations
    );

    let stylesheet = sanitize_stylesheet(
        ".card { color: rebeccapurple; position: fixed }",
        &ColorOnly,
    )?;
    println!("{}", stylesheet.css);
    println!(
        "HTML style text: {}",
        stylesheet.css.to_style_element_text()
    );

    Ok(())
}
