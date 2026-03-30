use crate::policy::{
    CssSanitizationPolicy, DeclarationOwner, DescriptorContext, DescriptorOwner, NodeAction,
    PropertyContext, RuleContext, RuleKind, SelectorContext,
};
use lightningcss::declaration::DeclarationBlock;
use lightningcss::printer::{Printer, PrinterOptions};
use lightningcss::rules::font_face::FontFaceProperty;
use lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use lightningcss::rules::page::PageMarginRule;
use lightningcss::rules::view_transition::ViewTransitionProperty;
use lightningcss::rules::CssRule;
use lightningcss::selector::SelectorList;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use lightningcss::traits::ToCss;

fn rule_kind(rule: &CssRule<'_>) -> RuleKind {
    match rule {
        CssRule::Media(_) => RuleKind::Media,
        CssRule::Import(_) => RuleKind::Import,
        CssRule::Style(_) => RuleKind::Style,
        CssRule::Keyframes(_) => RuleKind::Keyframes,
        CssRule::FontFace(_) => RuleKind::FontFace,
        CssRule::FontPaletteValues(_) => RuleKind::FontPaletteValues,
        CssRule::FontFeatureValues(_) => RuleKind::FontFeatureValues,
        CssRule::Page(_) => RuleKind::Page,
        CssRule::Supports(_) => RuleKind::Supports,
        CssRule::CounterStyle(_) => RuleKind::CounterStyle,
        CssRule::Namespace(_) => RuleKind::Namespace,
        CssRule::MozDocument(_) => RuleKind::MozDocument,
        CssRule::Nesting(_) => RuleKind::Nesting,
        CssRule::NestedDeclarations(_) => RuleKind::NestedDeclarations,
        CssRule::Viewport(_) => RuleKind::Viewport,
        CssRule::CustomMedia(_) => RuleKind::CustomMedia,
        CssRule::LayerStatement(_) => RuleKind::LayerStatement,
        CssRule::LayerBlock(_) => RuleKind::LayerBlock,
        CssRule::Property(_) => RuleKind::Property,
        CssRule::Container(_) => RuleKind::Container,
        CssRule::Scope(_) => RuleKind::Scope,
        CssRule::StartingStyle(_) => RuleKind::StartingStyle,
        CssRule::ViewTransition(_) => RuleKind::ViewTransition,
        CssRule::Ignored => RuleKind::Ignored,
        CssRule::Unknown(_) => RuleKind::Unknown,
        CssRule::Custom(_) => RuleKind::Custom,
    }
}

fn serialize_declaration_block(block: &DeclarationBlock<'_>) -> String {
    let mut output = String::new();
    let mut printer = Printer::new(&mut output, PrinterOptions::default());
    let _ = block.to_css(&mut printer);
    output
}

fn sanitize_selector_list(
    selectors: &mut SelectorList<'_>,
    policy: &dyn CssSanitizationPolicy,
    parent_rule: RuleKind,
    depth: usize,
) -> bool {
    !matches!(
        policy.visit_selector_list(selectors, SelectorContext { parent_rule, depth }),
        NodeAction::Drop
    )
}

fn sanitize_property_vec(
    properties: &mut Vec<lightningcss::properties::Property<'_>>,
    policy: &dyn CssSanitizationPolicy,
    owner: DeclarationOwner,
    parent_rule: Option<RuleKind>,
    depth: usize,
    important: bool,
) {
    let mut filtered = Vec::with_capacity(properties.len());
    for mut property in std::mem::take(properties) {
        let action = policy.visit_property(
            &mut property,
            PropertyContext {
                owner,
                parent_rule,
                depth,
                important,
            },
        );

        if !matches!(action, NodeAction::Drop) {
            filtered.push(property);
        }
    }
    *properties = filtered;
}

fn sanitize_declaration_block_inner(
    block: &mut DeclarationBlock<'_>,
    policy: &dyn CssSanitizationPolicy,
    owner: DeclarationOwner,
    parent_rule: Option<RuleKind>,
    depth: usize,
) {
    sanitize_property_vec(&mut block.declarations, policy, owner, parent_rule, depth, false);
    sanitize_property_vec(
        &mut block.important_declarations,
        policy,
        owner,
        parent_rule,
        depth,
        true,
    );
}

fn sanitize_font_face_properties(
    properties: &mut Vec<FontFaceProperty<'_>>,
    policy: &dyn CssSanitizationPolicy,
    depth: usize,
) {
    let mut filtered = Vec::with_capacity(properties.len());
    for mut property in std::mem::take(properties) {
        let action = policy.visit_font_face_property(
            &mut property,
            DescriptorContext {
                owner: DescriptorOwner::FontFace,
                parent_rule: Some(RuleKind::FontFace),
                depth,
            },
        );

        if !matches!(action, NodeAction::Drop) {
            filtered.push(property);
        }
    }
    *properties = filtered;
}

fn sanitize_font_palette_values_properties(
    properties: &mut Vec<FontPaletteValuesProperty<'_>>,
    policy: &dyn CssSanitizationPolicy,
    depth: usize,
) {
    let mut filtered = Vec::with_capacity(properties.len());
    for mut property in std::mem::take(properties) {
        let action = policy.visit_font_palette_values_property(
            &mut property,
            DescriptorContext {
                owner: DescriptorOwner::FontPaletteValues,
                parent_rule: Some(RuleKind::FontPaletteValues),
                depth,
            },
        );

        if !matches!(action, NodeAction::Drop) {
            filtered.push(property);
        }
    }
    *properties = filtered;
}

fn sanitize_view_transition_properties(
    properties: &mut Vec<ViewTransitionProperty<'_>>,
    policy: &dyn CssSanitizationPolicy,
    depth: usize,
) {
    let mut filtered = Vec::with_capacity(properties.len());
    for mut property in std::mem::take(properties) {
        let action = policy.visit_view_transition_property(
            &mut property,
            DescriptorContext {
                owner: DescriptorOwner::ViewTransition,
                parent_rule: Some(RuleKind::ViewTransition),
                depth,
            },
        );

        if !matches!(action, NodeAction::Drop) {
            filtered.push(property);
        }
    }
    *properties = filtered;
}

fn sanitize_page_margin_rules(
    rules: &mut Vec<PageMarginRule<'_>>,
    policy: &dyn CssSanitizationPolicy,
    depth: usize,
) {
    let mut filtered = Vec::with_capacity(rules.len());
    for mut rule in std::mem::take(rules) {
        let ctx = RuleContext {
            kind: RuleKind::PageMargin,
            parent: Some(RuleKind::Page),
            depth,
        };

        match policy.visit_page_margin_rule(&mut rule, ctx) {
            NodeAction::Drop => {}
            NodeAction::Skip => {
                if !rule.declarations.is_empty() {
                    filtered.push(rule);
                }
            }
            NodeAction::Continue => {
                sanitize_declaration_block_inner(
                    &mut rule.declarations,
                    policy,
                    DeclarationOwner::PageMargin,
                    Some(RuleKind::PageMargin),
                    depth + 1,
                );

                if !rule.declarations.is_empty() {
                    filtered.push(rule);
                }
            }
        }
    }

    *rules = filtered;
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
            if !sanitize_selector_list(&mut style_rule.selectors, policy, RuleKind::Style, ctx.depth + 1)
            {
                return false;
            }

            sanitize_declaration_block_inner(
                &mut style_rule.declarations,
                policy,
                DeclarationOwner::StyleRule,
                Some(RuleKind::Style),
                ctx.depth + 1,
            );
            sanitize_rule_list(&mut style_rule.rules.0, policy, Some(RuleKind::Style), ctx.depth + 1);
        }
        CssRule::Media(rule) => {
            sanitize_rule_list(&mut rule.rules.0, policy, Some(RuleKind::Media), ctx.depth + 1);
        }
        CssRule::Keyframes(rule) => {
            for keyframe in &mut rule.keyframes {
                sanitize_declaration_block_inner(
                    &mut keyframe.declarations,
                    policy,
                    DeclarationOwner::Keyframe,
                    Some(RuleKind::Keyframes),
                    ctx.depth + 1,
                );
            }
            rule.keyframes.retain(|keyframe| !keyframe.declarations.is_empty());
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
                NodeAction::Skip | NodeAction::Continue => {}
            }
        }
        CssRule::Page(rule) => match policy.visit_page_rule(rule, ctx) {
            NodeAction::Drop => return false,
            NodeAction::Skip => {}
            NodeAction::Continue => {
                sanitize_declaration_block_inner(
                    &mut rule.declarations,
                    policy,
                    DeclarationOwner::Page,
                    Some(RuleKind::Page),
                    ctx.depth + 1,
                );
                sanitize_page_margin_rules(&mut rule.rules, policy, ctx.depth + 1);
            }
        },
        CssRule::Supports(rule) => {
            sanitize_rule_list(
                &mut rule.rules.0,
                policy,
                Some(RuleKind::Supports),
                ctx.depth + 1,
            );
        }
        CssRule::CounterStyle(rule) => match policy.visit_counter_style_rule(rule, ctx) {
            NodeAction::Drop => return false,
            NodeAction::Skip => {}
            NodeAction::Continue => {
                sanitize_declaration_block_inner(
                    &mut rule.declarations,
                    policy,
                    DeclarationOwner::CounterStyle,
                    Some(RuleKind::CounterStyle),
                    ctx.depth + 1,
                );
            }
        },
        CssRule::MozDocument(rule) => {
            sanitize_rule_list(
                &mut rule.rules.0,
                policy,
                Some(RuleKind::MozDocument),
                ctx.depth + 1,
            );
        }
        CssRule::Nesting(rule) => {
            if !sanitize_selector_list(
                &mut rule.style.selectors,
                policy,
                RuleKind::Nesting,
                ctx.depth + 1,
            ) {
                return false;
            }

            sanitize_rule_list(
                &mut rule.style.rules.0,
                policy,
                Some(RuleKind::Nesting),
                ctx.depth + 1,
            );
            sanitize_declaration_block_inner(
                &mut rule.style.declarations,
                policy,
                DeclarationOwner::StyleRule,
                Some(RuleKind::Nesting),
                ctx.depth + 1,
            );
        }
        CssRule::NestedDeclarations(rule) => {
            sanitize_declaration_block_inner(
                &mut rule.declarations,
                policy,
                DeclarationOwner::NestedDeclarations,
                Some(RuleKind::NestedDeclarations),
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
                    DeclarationOwner::Viewport,
                    Some(RuleKind::Viewport),
                    ctx.depth + 1,
                );
            }
        },
        CssRule::LayerBlock(rule) => {
            sanitize_rule_list(
                &mut rule.rules.0,
                policy,
                Some(RuleKind::LayerBlock),
                ctx.depth + 1,
            );
        }
        CssRule::Container(rule) => {
            sanitize_rule_list(
                &mut rule.rules.0,
                policy,
                Some(RuleKind::Container),
                ctx.depth + 1,
            );
        }
        CssRule::Scope(rule) => {
            sanitize_rule_list(
                &mut rule.rules.0,
                policy,
                Some(RuleKind::Scope),
                ctx.depth + 1,
            );
        }
        CssRule::StartingStyle(rule) => {
            sanitize_rule_list(
                &mut rule.rules.0,
                policy,
                Some(RuleKind::StartingStyle),
                ctx.depth + 1,
            );
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
    parent: Option<RuleKind>,
    depth: usize,
) {
    let mut filtered = Vec::with_capacity(rules.len());

    for mut rule in std::mem::take(rules) {
        let ctx = RuleContext {
            kind: rule_kind(&rule),
            parent,
            depth,
        };

        match policy.visit_rule(&mut rule, ctx) {
            NodeAction::Drop => {}
            NodeAction::Skip => {
                if !is_rule_empty(&rule) {
                    filtered.push(rule);
                }
            }
            NodeAction::Continue => {
                if sanitize_rule_contents(&mut rule, policy, ctx) && !is_rule_empty(&rule) {
                    filtered.push(rule);
                }
            }
        }
    }

    *rules = filtered;
}

/// Sanitizes a parsed declaration block in place.
pub fn sanitize_declaration_block_ast(
    block: &mut DeclarationBlock<'_>,
    policy: &dyn CssSanitizationPolicy,
) {
    sanitize_declaration_block_inner(block, policy, DeclarationOwner::DeclarationList, None, 0);
}

/// Sanitizes a parsed stylesheet AST in place.
pub fn sanitize_stylesheet_ast(
    stylesheet: &mut StyleSheet<'_, '_>,
    policy: &dyn CssSanitizationPolicy,
) {
    sanitize_rule_list(
        &mut stylesheet.rules.0,
        policy,
        Some(RuleKind::Stylesheet),
        0,
    );
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

    serialize_declaration_block(&block)
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
