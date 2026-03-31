use crate::policy::{
    CssSanitizationPolicy, DescriptorContext, NodeAction, PropertyContext, RuleContext,
    SelectorContext,
};
use lightningcss::declaration::DeclarationBlock;
use lightningcss::printer::{Printer, PrinterOptions};
use lightningcss::rules::CssRule;
use lightningcss::rules::font_face::FontFaceProperty;
use lightningcss::rules::font_feature_values::FontFeatureValuesRule;
use lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use lightningcss::rules::page::PageMarginRule;
use lightningcss::rules::view_transition::ViewTransitionProperty;
use lightningcss::selector::SelectorList;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use lightningcss::traits::ToCss;

#[derive(Clone, Copy)]
enum SelectorLocation {
    StyleRule,
    Nesting,
}

#[derive(Clone, Copy)]
enum PropertyLocation {
    DeclarationList,
    StyleRule,
    NestedDeclarations,
    Keyframe,
    Page,
    PageMargin,
    CounterStyle,
    Viewport,
}

fn serialize_declaration_block(block: &DeclarationBlock<'_>) -> Option<String> {
    let mut output = String::new();
    let mut printer = Printer::new(&mut output, PrinterOptions::default());
    block.to_css(&mut printer).ok()?;
    Some(output)
}

fn sanitize_selector_list(
    selectors: &mut SelectorList<'_>,
    policy: &dyn CssSanitizationPolicy,
    location: SelectorLocation,
    depth: usize,
) -> bool {
    let action = match location {
        SelectorLocation::StyleRule => {
            policy.visit_style_rule_selectors(selectors, SelectorContext { depth })
        }
        SelectorLocation::Nesting => {
            policy.visit_nesting_selectors(selectors, SelectorContext { depth })
        }
    };

    !matches!(action, NodeAction::Drop)
}

fn sanitize_property_vec(
    properties: &mut Vec<lightningcss::properties::Property<'_>>,
    policy: &dyn CssSanitizationPolicy,
    location: PropertyLocation,
    depth: usize,
    important: bool,
) {
    properties.retain_mut(|property| {
        let ctx = PropertyContext { depth, important };
        let action = match location {
            PropertyLocation::DeclarationList => {
                policy.visit_declaration_list_property(property, ctx)
            }
            PropertyLocation::StyleRule => policy.visit_style_property(property, ctx),
            PropertyLocation::NestedDeclarations => {
                policy.visit_nested_declarations_property(property, ctx)
            }
            PropertyLocation::Keyframe => policy.visit_keyframe_property(property, ctx),
            PropertyLocation::Page => policy.visit_page_property(property, ctx),
            PropertyLocation::PageMargin => policy.visit_page_margin_property(property, ctx),
            PropertyLocation::CounterStyle => policy.visit_counter_style_property(property, ctx),
            PropertyLocation::Viewport => policy.visit_viewport_property(property, ctx),
        };

        !matches!(action, NodeAction::Drop)
    });
}

fn sanitize_declaration_block_inner(
    block: &mut DeclarationBlock<'_>,
    policy: &dyn CssSanitizationPolicy,
    location: PropertyLocation,
    depth: usize,
) {
    sanitize_property_vec(&mut block.declarations, policy, location, depth, false);
    sanitize_property_vec(
        &mut block.important_declarations,
        policy,
        location,
        depth,
        true,
    );
}

fn sanitize_font_face_properties(
    properties: &mut Vec<FontFaceProperty<'_>>,
    policy: &dyn CssSanitizationPolicy,
    depth: usize,
) {
    properties.retain_mut(|property| {
        let action = policy.visit_font_face_property(property, DescriptorContext { depth });
        !matches!(action, NodeAction::Drop)
    });
}

fn sanitize_font_palette_values_properties(
    properties: &mut Vec<FontPaletteValuesProperty<'_>>,
    policy: &dyn CssSanitizationPolicy,
    depth: usize,
) {
    properties.retain_mut(|property| {
        let action =
            policy.visit_font_palette_values_property(property, DescriptorContext { depth });
        !matches!(action, NodeAction::Drop)
    });
}

fn sanitize_view_transition_properties(
    properties: &mut Vec<ViewTransitionProperty<'_>>,
    policy: &dyn CssSanitizationPolicy,
    depth: usize,
) {
    properties.retain_mut(|property| {
        let action = policy.visit_view_transition_property(property, DescriptorContext { depth });
        !matches!(action, NodeAction::Drop)
    });
}

fn sanitize_font_feature_values_subrules(
    rule: &mut FontFeatureValuesRule<'_>,
    policy: &dyn CssSanitizationPolicy,
    depth: usize,
) {
    rule.rules.retain(|_, subrule| {
        let ctx = RuleContext { depth };
        match policy.visit_font_feature_values_subrule(subrule, ctx) {
            NodeAction::Drop => false,
            NodeAction::Skip | NodeAction::Continue => !subrule.declarations.is_empty(),
        }
    });
}

fn sanitize_page_margin_rules(
    rules: &mut Vec<PageMarginRule<'_>>,
    policy: &dyn CssSanitizationPolicy,
    depth: usize,
) {
    rules.retain_mut(|rule| {
        let ctx = RuleContext { depth };

        match policy.visit_page_margin_rule(rule, ctx) {
            NodeAction::Drop => false,
            NodeAction::Skip => !rule.declarations.is_empty(),
            NodeAction::Continue => {
                sanitize_declaration_block_inner(
                    &mut rule.declarations,
                    policy,
                    PropertyLocation::PageMargin,
                    depth + 1,
                );

                !rule.declarations.is_empty()
            }
        }
    });
}

fn is_rule_empty(rule: &CssRule<'_>) -> bool {
    match rule {
        CssRule::Style(rule) => {
            rule.selectors.0.is_empty() || (rule.declarations.is_empty() && rule.rules.0.is_empty())
        }
        CssRule::Media(rule) => rule.rules.0.is_empty(),
        CssRule::Keyframes(rule) => rule.keyframes.is_empty(),
        CssRule::FontFace(rule) => rule.properties.is_empty(),
        CssRule::FontPaletteValues(rule) => rule.properties.is_empty(),
        CssRule::FontFeatureValues(rule) => rule.rules.is_empty(),
        CssRule::Page(rule) => rule.declarations.is_empty() && rule.rules.is_empty(),
        CssRule::Supports(rule) => rule.rules.0.is_empty(),
        CssRule::CounterStyle(rule) => rule.declarations.is_empty(),
        CssRule::MozDocument(rule) => rule.rules.0.is_empty(),
        CssRule::Nesting(rule) => {
            rule.style.selectors.0.is_empty()
                || (rule.style.declarations.is_empty() && rule.style.rules.0.is_empty())
        }
        CssRule::NestedDeclarations(rule) => rule.declarations.is_empty(),
        CssRule::Viewport(rule) => rule.declarations.is_empty(),
        CssRule::LayerBlock(rule) => rule.rules.0.is_empty(),
        CssRule::Container(rule) => rule.rules.0.is_empty(),
        CssRule::Scope(rule) => rule.rules.0.is_empty(),
        CssRule::StartingStyle(rule) => rule.rules.0.is_empty(),
        CssRule::ViewTransition(rule) => rule.properties.is_empty(),
        _ => false,
    }
}

fn sanitize_rule_contents(
    rule: &mut CssRule<'_>,
    policy: &dyn CssSanitizationPolicy,
    ctx: RuleContext,
) -> bool {
    match rule {
        CssRule::Style(style_rule) => {
            if !sanitize_selector_list(
                &mut style_rule.selectors,
                policy,
                SelectorLocation::StyleRule,
                ctx.depth + 1,
            ) {
                return false;
            }

            sanitize_declaration_block_inner(
                &mut style_rule.declarations,
                policy,
                PropertyLocation::StyleRule,
                ctx.depth + 1,
            );
            sanitize_rule_list(&mut style_rule.rules.0, policy, ctx.depth + 1);
        }
        CssRule::Media(rule) => {
            sanitize_rule_list(&mut rule.rules.0, policy, ctx.depth + 1);
        }
        CssRule::Keyframes(rule) => {
            for keyframe in &mut rule.keyframes {
                sanitize_declaration_block_inner(
                    &mut keyframe.declarations,
                    policy,
                    PropertyLocation::Keyframe,
                    ctx.depth + 1,
                );
            }
            rule.keyframes
                .retain(|keyframe| !keyframe.declarations.is_empty());
        }
        CssRule::FontFace(rule) => {
            sanitize_font_face_properties(&mut rule.properties, policy, ctx.depth + 1);
        }
        CssRule::FontPaletteValues(rule) => {
            sanitize_font_palette_values_properties(&mut rule.properties, policy, ctx.depth + 1);
        }
        CssRule::FontFeatureValues(rule) => {
            match policy.visit_font_feature_values_rule(rule, ctx) {
                NodeAction::Drop => return false,
                NodeAction::Skip => {}
                NodeAction::Continue => {
                    sanitize_font_feature_values_subrules(rule, policy, ctx.depth + 1);
                }
            }
        }
        CssRule::Page(rule) => match policy.visit_page_rule(rule, ctx) {
            NodeAction::Drop => return false,
            NodeAction::Skip => {}
            NodeAction::Continue => {
                sanitize_declaration_block_inner(
                    &mut rule.declarations,
                    policy,
                    PropertyLocation::Page,
                    ctx.depth + 1,
                );
                sanitize_page_margin_rules(&mut rule.rules, policy, ctx.depth + 1);
            }
        },
        CssRule::Supports(rule) => {
            sanitize_rule_list(&mut rule.rules.0, policy, ctx.depth + 1);
        }
        CssRule::CounterStyle(rule) => match policy.visit_counter_style_rule(rule, ctx) {
            NodeAction::Drop => return false,
            NodeAction::Skip => {}
            NodeAction::Continue => {
                sanitize_declaration_block_inner(
                    &mut rule.declarations,
                    policy,
                    PropertyLocation::CounterStyle,
                    ctx.depth + 1,
                );
            }
        },
        CssRule::MozDocument(rule) => {
            sanitize_rule_list(&mut rule.rules.0, policy, ctx.depth + 1);
        }
        CssRule::Nesting(rule) => {
            if !sanitize_selector_list(
                &mut rule.style.selectors,
                policy,
                SelectorLocation::Nesting,
                ctx.depth + 1,
            ) {
                return false;
            }

            sanitize_rule_list(&mut rule.style.rules.0, policy, ctx.depth + 1);
            sanitize_declaration_block_inner(
                &mut rule.style.declarations,
                policy,
                PropertyLocation::StyleRule,
                ctx.depth + 1,
            );
        }
        CssRule::NestedDeclarations(rule) => {
            sanitize_declaration_block_inner(
                &mut rule.declarations,
                policy,
                PropertyLocation::NestedDeclarations,
                ctx.depth + 1,
            );
        }
        CssRule::Viewport(rule) => match policy.visit_viewport_rule(rule, ctx) {
            NodeAction::Drop => return false,
            NodeAction::Skip => {}
            NodeAction::Continue => {
                sanitize_declaration_block_inner(
                    &mut rule.declarations,
                    policy,
                    PropertyLocation::Viewport,
                    ctx.depth + 1,
                );
            }
        },
        CssRule::LayerBlock(rule) => sanitize_rule_list(&mut rule.rules.0, policy, ctx.depth + 1),
        CssRule::Container(rule) => sanitize_rule_list(&mut rule.rules.0, policy, ctx.depth + 1),
        CssRule::Scope(rule) => sanitize_rule_list(&mut rule.rules.0, policy, ctx.depth + 1),
        CssRule::StartingStyle(rule) => {
            sanitize_rule_list(&mut rule.rules.0, policy, ctx.depth + 1)
        }
        CssRule::ViewTransition(rule) => {
            sanitize_view_transition_properties(&mut rule.properties, policy, ctx.depth + 1);
        }
        CssRule::Import(_)
        | CssRule::Namespace(_)
        | CssRule::CustomMedia(_)
        | CssRule::LayerStatement(_)
        | CssRule::Property(_)
        | CssRule::Ignored
        | CssRule::Unknown(_)
        | CssRule::Custom(_) => {}
    }

    true
}

fn sanitize_rule_list(
    rules: &mut Vec<CssRule<'_>>,
    policy: &dyn CssSanitizationPolicy,
    depth: usize,
) {
    rules.retain_mut(|rule| {
        let ctx = RuleContext { depth };

        match policy.visit_rule(rule, ctx) {
            NodeAction::Drop => false,
            NodeAction::Skip => !is_rule_empty(rule),
            NodeAction::Continue => {
                sanitize_rule_contents(rule, policy, ctx) && !is_rule_empty(rule)
            }
        }
    });
}

/// Sanitizes a parsed declaration block in place.
pub fn sanitize_declaration_block_ast(
    block: &mut DeclarationBlock<'_>,
    policy: &dyn CssSanitizationPolicy,
) {
    sanitize_declaration_block_inner(block, policy, PropertyLocation::DeclarationList, 0);
}

/// Sanitizes a parsed stylesheet AST in place.
pub fn sanitize_stylesheet_ast(
    stylesheet: &mut StyleSheet<'_, '_>,
    policy: &dyn CssSanitizationPolicy,
) {
    sanitize_rule_list(&mut stylesheet.rules.0, policy, 0);
}

/// Parses and sanitizes an inline declaration list with a custom AST policy.
pub fn clean_declaration_list_with_policy(
    input: &str,
    policy: &dyn CssSanitizationPolicy,
) -> String {
    let options = ParserOptions {
        error_recovery: true,
        ..ParserOptions::default()
    };

    let Ok(mut block) = DeclarationBlock::parse_string(input, options) else {
        return String::new();
    };

    sanitize_declaration_block_ast(&mut block, policy);

    if block.is_empty() {
        return String::new();
    }

    serialize_declaration_block(&block).unwrap_or_default()
}

/// Parses and sanitizes a full stylesheet with a custom AST policy.
pub fn clean_stylesheet_with_policy(input: &str, policy: &dyn CssSanitizationPolicy) -> String {
    let options = ParserOptions {
        error_recovery: true,
        ..ParserOptions::default()
    };

    let Ok(mut stylesheet) = StyleSheet::parse(input, options) else {
        return String::new();
    };

    sanitize_stylesheet_ast(&mut stylesheet, policy);

    if stylesheet.rules.0.is_empty() {
        return String::new();
    }

    match stylesheet.to_css(PrinterOptions::default()) {
        Ok(result) => result.code,
        Err(_) => String::new(),
    }
}
