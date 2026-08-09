use crate::guard;
use crate::options::SanitizeOptions;
use crate::policy::{
    CssSanitizationPolicy, DescriptorContext, NodeAction, PropertyContext, ResourceKind,
    ResourceRef, RuleContext, SelectorContext, ValueAction, ValueContext, ValueLocation,
};
use lightningcss::declaration::DeclarationBlock;
use lightningcss::printer::{Printer, PrinterOptions};
use lightningcss::rules::CssRule;
use lightningcss::rules::container::{ContainerCondition, StyleQuery};
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
    Scope,
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
    PositionTry,
}

impl PropertyLocation {
    fn value_location(self) -> ValueLocation {
        match self {
            PropertyLocation::DeclarationList => ValueLocation::DeclarationList,
            PropertyLocation::StyleRule => ValueLocation::StyleRule,
            PropertyLocation::NestedDeclarations => ValueLocation::NestedDeclarations,
            PropertyLocation::Keyframe => ValueLocation::Keyframe,
            PropertyLocation::Page => ValueLocation::Page,
            PropertyLocation::PageMargin => ValueLocation::PageMargin,
            PropertyLocation::CounterStyle => ValueLocation::CounterStyle,
            PropertyLocation::Viewport => ValueLocation::Viewport,
            PropertyLocation::PositionTry => ValueLocation::PositionTry,
        }
    }
}

fn serialize_declaration_block(block: &DeclarationBlock<'_>) -> Option<String> {
    let mut output = String::new();
    let mut printer = Printer::new(&mut output, PrinterOptions::default());
    block.to_css(&mut printer).ok()?;
    Some(output)
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
        CssRule::PositionTry(rule) => rule.declarations.is_empty(),
        CssRule::LayerBlock(rule) => rule.rules.0.is_empty(),
        CssRule::Container(rule) => rule.rules.0.is_empty(),
        CssRule::Scope(rule) => rule.rules.0.is_empty(),
        CssRule::StartingStyle(rule) => rule.rules.0.is_empty(),
        CssRule::ViewTransition(rule) => rule.properties.is_empty(),
        _ => false,
    }
}

/// Carries the policy and options through the recursive traversal.
struct Engine<'p> {
    policy: &'p dyn CssSanitizationPolicy,
    opts: SanitizeOptions,
}

impl<'p> Engine<'p> {
    fn sanitize_selector_list(
        &self,
        selectors: &mut SelectorList<'_>,
        location: SelectorLocation,
        depth: usize,
    ) -> bool {
        let ctx = SelectorContext { depth };
        let action = match location {
            SelectorLocation::StyleRule => self.policy.visit_style_rule_selectors(selectors, ctx),
            SelectorLocation::Nesting => self.policy.visit_nesting_selectors(selectors, ctx),
            SelectorLocation::Scope => self.policy.visit_scope_selectors(selectors, ctx),
        };

        !matches!(action, NodeAction::Drop)
    }

    fn sanitize_property_vec(
        &self,
        properties: &mut Vec<lightningcss::properties::Property<'_>>,
        location: PropertyLocation,
        depth: usize,
        important: bool,
    ) {
        let value_location = location.value_location();
        properties.retain_mut(|property| {
            let ctx = PropertyContext { depth, important };
            let action = match location {
                PropertyLocation::DeclarationList => {
                    self.policy.visit_declaration_list_property(property, ctx)
                }
                PropertyLocation::StyleRule => self.policy.visit_style_property(property, ctx),
                PropertyLocation::NestedDeclarations => self
                    .policy
                    .visit_nested_declarations_property(property, ctx),
                PropertyLocation::Keyframe => self.policy.visit_keyframe_property(property, ctx),
                PropertyLocation::Page => self.policy.visit_page_property(property, ctx),
                PropertyLocation::PageMargin => {
                    self.policy.visit_page_margin_property(property, ctx)
                }
                PropertyLocation::CounterStyle => {
                    self.policy.visit_counter_style_property(property, ctx)
                }
                PropertyLocation::Viewport => self.policy.visit_viewport_property(property, ctx),
                PropertyLocation::PositionTry => {
                    self.policy.visit_position_try_property(property, ctx)
                }
            };

            match action {
                NodeAction::Drop => false,
                NodeAction::Skip | NodeAction::Continue => {
                    !self.opts.enforce_value_guard
                        || guard::property_allowed(
                            property,
                            self.policy,
                            ValueContext {
                                depth,
                                important,
                                location: value_location,
                            },
                        )
                }
            }
        });
    }

    fn sanitize_declaration_block_inner(
        &self,
        block: &mut DeclarationBlock<'_>,
        location: PropertyLocation,
        depth: usize,
    ) {
        self.sanitize_property_vec(&mut block.declarations, location, depth, false);
        self.sanitize_property_vec(&mut block.important_declarations, location, depth, true);
    }

    fn sanitize_font_face_properties(
        &self,
        properties: &mut Vec<FontFaceProperty<'_>>,
        depth: usize,
    ) {
        properties.retain_mut(|property| {
            let action = self
                .policy
                .visit_font_face_property(property, DescriptorContext { depth });
            self.descriptor_kept(action, || {
                guard::font_face_property_allowed(
                    property,
                    self.policy,
                    ValueContext {
                        depth,
                        important: false,
                        location: ValueLocation::FontFaceDescriptor,
                    },
                )
            })
        });
    }

    fn sanitize_font_palette_values_properties(
        &self,
        properties: &mut Vec<FontPaletteValuesProperty<'_>>,
        depth: usize,
    ) {
        properties.retain_mut(|property| {
            let action = self
                .policy
                .visit_font_palette_values_property(property, DescriptorContext { depth });
            self.descriptor_kept(action, || {
                guard::font_palette_values_property_allowed(
                    property,
                    self.policy,
                    ValueContext {
                        depth,
                        important: false,
                        location: ValueLocation::FontPaletteValuesDescriptor,
                    },
                )
            })
        });
    }

    fn sanitize_view_transition_properties(
        &self,
        properties: &mut Vec<ViewTransitionProperty<'_>>,
        depth: usize,
    ) {
        properties.retain_mut(|property| {
            let action = self
                .policy
                .visit_view_transition_property(property, DescriptorContext { depth });
            self.descriptor_kept(action, || {
                guard::view_transition_property_allowed(
                    property,
                    self.policy,
                    ValueContext {
                        depth,
                        important: false,
                        location: ValueLocation::ViewTransitionDescriptor,
                    },
                )
            })
        });
    }

    /// Shared keep/drop logic for descriptor properties. Engine value/resource
    /// guards apply equally to `Skip` and `Continue` when enabled.
    fn descriptor_kept(&self, action: NodeAction, value_guard: impl FnOnce() -> bool) -> bool {
        match action {
            NodeAction::Drop => false,
            NodeAction::Skip | NodeAction::Continue => {
                !self.opts.enforce_value_guard || value_guard()
            }
        }
    }

    fn sanitize_font_feature_values_subrules(
        &self,
        rule: &mut FontFeatureValuesRule<'_>,
        depth: usize,
    ) {
        rule.rules.retain(|_, subrule| {
            let ctx = RuleContext { depth };
            match self.policy.visit_font_feature_values_subrule(subrule, ctx) {
                NodeAction::Drop => false,
                NodeAction::Skip | NodeAction::Continue => !subrule.declarations.is_empty(),
            }
        });
    }

    fn sanitize_page_margin_rules(&self, rules: &mut Vec<PageMarginRule<'_>>, depth: usize) {
        rules.retain_mut(|rule| {
            let ctx = RuleContext { depth };

            match self.policy.visit_page_margin_rule(rule, ctx) {
                NodeAction::Drop => false,
                NodeAction::Skip | NodeAction::Continue => {
                    self.sanitize_declaration_block_inner(
                        &mut rule.declarations,
                        PropertyLocation::PageMargin,
                        depth + 1,
                    );

                    !rule.declarations.is_empty()
                }
            }
        });
    }

    /// Runs the value guard over the declarations embedded in an `@container`
    /// style query condition. `Not`/`Operation` are `#[skip_type]` in
    /// lightningcss, so the tree is walked manually rather than via `Visit`.
    fn container_condition_allowed(
        &self,
        condition: &mut ContainerCondition<'_>,
        depth: usize,
    ) -> bool {
        match condition {
            ContainerCondition::Style(query) => self.style_query_allowed(query, depth),
            ContainerCondition::Not(inner) => self.container_condition_allowed(inner, depth),
            ContainerCondition::Operation { conditions, .. } => conditions
                .iter_mut()
                .all(|condition| self.container_condition_allowed(condition, depth)),
            ContainerCondition::Unknown(tokens) => {
                guard::token_list_allowed(tokens, self.policy, self.condition_value_ctx(depth))
            }
            ContainerCondition::Feature(_) | ContainerCondition::ScrollState(_) => true,
        }
    }

    fn style_query_allowed(&self, query: &mut StyleQuery<'_>, depth: usize) -> bool {
        match query {
            StyleQuery::Declaration(property) => {
                guard::property_allowed(property, self.policy, self.condition_value_ctx(depth))
            }
            StyleQuery::Not(inner) => self.style_query_allowed(inner, depth),
            StyleQuery::Operation { conditions, .. } => conditions
                .iter_mut()
                .all(|query| self.style_query_allowed(query, depth)),
            StyleQuery::Property(_) => true,
        }
    }

    fn condition_value_ctx(&self, depth: usize) -> ValueContext {
        ValueContext {
            depth,
            important: false,
            location: ValueLocation::ContainerCondition,
        }
    }

    fn sanitize_rule_contents(&self, rule: &mut CssRule<'_>, ctx: RuleContext) -> bool {
        match rule {
            CssRule::Style(style_rule) => {
                if !self.sanitize_selector_list(
                    &mut style_rule.selectors,
                    SelectorLocation::StyleRule,
                    ctx.depth + 1,
                ) {
                    return false;
                }

                self.sanitize_declaration_block_inner(
                    &mut style_rule.declarations,
                    PropertyLocation::StyleRule,
                    ctx.depth + 1,
                );
                self.sanitize_rule_list(&mut style_rule.rules.0, ctx.depth + 1);
            }
            CssRule::Media(rule) => {
                self.sanitize_rule_list(&mut rule.rules.0, ctx.depth + 1);
            }
            CssRule::Keyframes(rule) => {
                for keyframe in &mut rule.keyframes {
                    self.sanitize_declaration_block_inner(
                        &mut keyframe.declarations,
                        PropertyLocation::Keyframe,
                        ctx.depth + 1,
                    );
                }
                rule.keyframes
                    .retain(|keyframe| !keyframe.declarations.is_empty());
            }
            CssRule::FontFace(rule) => {
                self.sanitize_font_face_properties(&mut rule.properties, ctx.depth + 1);
            }
            CssRule::FontPaletteValues(rule) => {
                self.sanitize_font_palette_values_properties(&mut rule.properties, ctx.depth + 1);
            }
            CssRule::FontFeatureValues(rule) => {
                match self.policy.visit_font_feature_values_rule(rule, ctx) {
                    NodeAction::Drop => return false,
                    NodeAction::Skip | NodeAction::Continue => {
                        self.sanitize_font_feature_values_subrules(rule, ctx.depth + 1);
                    }
                }
            }
            CssRule::Page(rule) => match self.policy.visit_page_rule(rule, ctx) {
                NodeAction::Drop => return false,
                NodeAction::Skip | NodeAction::Continue => {
                    self.sanitize_declaration_block_inner(
                        &mut rule.declarations,
                        PropertyLocation::Page,
                        ctx.depth + 1,
                    );
                    self.sanitize_page_margin_rules(&mut rule.rules, ctx.depth + 1);
                }
            },
            CssRule::Supports(rule) => {
                self.sanitize_rule_list(&mut rule.rules.0, ctx.depth + 1);
            }
            CssRule::CounterStyle(rule) => match self.policy.visit_counter_style_rule(rule, ctx) {
                NodeAction::Drop => return false,
                NodeAction::Skip | NodeAction::Continue => {
                    self.sanitize_declaration_block_inner(
                        &mut rule.declarations,
                        PropertyLocation::CounterStyle,
                        ctx.depth + 1,
                    );
                }
            },
            CssRule::MozDocument(rule) => {
                self.sanitize_rule_list(&mut rule.rules.0, ctx.depth + 1);
            }
            CssRule::Nesting(rule) => {
                if !self.sanitize_selector_list(
                    &mut rule.style.selectors,
                    SelectorLocation::Nesting,
                    ctx.depth + 1,
                ) {
                    return false;
                }

                self.sanitize_rule_list(&mut rule.style.rules.0, ctx.depth + 1);
                self.sanitize_declaration_block_inner(
                    &mut rule.style.declarations,
                    PropertyLocation::StyleRule,
                    ctx.depth + 1,
                );
            }
            CssRule::NestedDeclarations(rule) => {
                self.sanitize_declaration_block_inner(
                    &mut rule.declarations,
                    PropertyLocation::NestedDeclarations,
                    ctx.depth + 1,
                );
            }
            CssRule::Viewport(rule) => match self.policy.visit_viewport_rule(rule, ctx) {
                NodeAction::Drop => return false,
                NodeAction::Skip | NodeAction::Continue => {
                    self.sanitize_declaration_block_inner(
                        &mut rule.declarations,
                        PropertyLocation::Viewport,
                        ctx.depth + 1,
                    );
                }
            },
            CssRule::PositionTry(rule) => {
                self.sanitize_declaration_block_inner(
                    &mut rule.declarations,
                    PropertyLocation::PositionTry,
                    ctx.depth + 1,
                );
            }
            CssRule::LayerBlock(rule) => self.sanitize_rule_list(&mut rule.rules.0, ctx.depth + 1),
            CssRule::Container(rule) => {
                // The `@container` prelude can embed declarations inside style
                // queries (`style(prop: value)`); guard their values and drop
                // the whole rule if any is rejected.
                if self.opts.enforce_value_guard
                    && let Some(condition) = &mut rule.condition
                    && !self.container_condition_allowed(condition, ctx.depth + 1)
                {
                    return false;
                }
                self.sanitize_rule_list(&mut rule.rules.0, ctx.depth + 1);
            }
            CssRule::Scope(rule) => {
                if let Some(scope_start) = &mut rule.scope_start
                    && !self.sanitize_selector_list(
                        scope_start,
                        SelectorLocation::Scope,
                        ctx.depth + 1,
                    )
                {
                    return false;
                }
                if let Some(scope_end) = &mut rule.scope_end
                    && !self.sanitize_selector_list(
                        scope_end,
                        SelectorLocation::Scope,
                        ctx.depth + 1,
                    )
                {
                    return false;
                }

                self.sanitize_rule_list(&mut rule.rules.0, ctx.depth + 1);
            }
            CssRule::StartingStyle(rule) => {
                self.sanitize_rule_list(&mut rule.rules.0, ctx.depth + 1)
            }
            CssRule::ViewTransition(rule) => {
                self.sanitize_view_transition_properties(&mut rule.properties, ctx.depth + 1);
            }
            CssRule::Property(rule) => {
                // `@property` registers a custom property whose `initial-value`
                // can carry a url that is later fetched via `var()`. The field is
                // `#[skip_visit]`, so guard it explicitly.
                if self.opts.enforce_value_guard
                    && let Some(initial_value) = &mut rule.initial_value
                    && !guard::parsed_component_allowed(
                        initial_value,
                        self.policy,
                        ValueContext {
                            depth: ctx.depth + 1,
                            important: false,
                            location: ValueLocation::PropertyInitialValue,
                        },
                    )
                {
                    return false;
                }
            }
            CssRule::Import(rule) => {
                if self.opts.enforce_value_guard
                    && matches!(
                        self.policy.check_resource(
                            ResourceRef::literal(ResourceKind::Import, rule.url.as_ref()),
                            ValueContext {
                                depth: ctx.depth + 1,
                                important: false,
                                location: ValueLocation::ImportRule,
                            },
                        ),
                        ValueAction::Deny
                    )
                {
                    return false;
                }
            }
            CssRule::Namespace(_)
            | CssRule::CustomMedia(_)
            | CssRule::LayerStatement(_)
            | CssRule::Ignored
            | CssRule::Unknown(_)
            | CssRule::Custom(_) => {}
        }

        true
    }

    fn sanitize_rule_list(&self, rules: &mut Vec<CssRule<'_>>, depth: usize) {
        // Fail-closed depth cap: drop any subtree nested deeper than configured
        // to bound recursion against stack-overflow denial of service.
        if depth > self.opts.max_depth {
            rules.clear();
            return;
        }

        rules.retain_mut(|rule| {
            let ctx = RuleContext { depth };

            match self.policy.visit_rule(rule, ctx) {
                NodeAction::Drop => false,
                NodeAction::Skip | NodeAction::Continue => {
                    self.sanitize_rule_contents(rule, ctx) && !is_rule_empty(rule)
                }
            }
        });
    }
}

/// Sanitizes a parsed declaration block in place using default options.
pub fn sanitize_declaration_block_ast(
    block: &mut DeclarationBlock<'_>,
    policy: &dyn CssSanitizationPolicy,
) {
    sanitize_declaration_block_ast_with_options(block, policy, SanitizeOptions::default());
}

/// Sanitizes a parsed declaration block in place with custom options.
pub fn sanitize_declaration_block_ast_with_options(
    block: &mut DeclarationBlock<'_>,
    policy: &dyn CssSanitizationPolicy,
    options: SanitizeOptions,
) {
    let engine = Engine {
        policy,
        opts: options,
    };
    engine.sanitize_declaration_block_inner(block, PropertyLocation::DeclarationList, 0);
}

/// Sanitizes a parsed stylesheet AST in place using default options.
pub fn sanitize_stylesheet_ast(
    stylesheet: &mut StyleSheet<'_>,
    policy: &dyn CssSanitizationPolicy,
) {
    sanitize_stylesheet_ast_with_options(stylesheet, policy, SanitizeOptions::default());
}

/// Sanitizes a parsed stylesheet AST in place with custom options.
pub fn sanitize_stylesheet_ast_with_options(
    stylesheet: &mut StyleSheet<'_>,
    policy: &dyn CssSanitizationPolicy,
    options: SanitizeOptions,
) {
    let engine = Engine {
        policy,
        opts: options,
    };
    engine.sanitize_rule_list(&mut stylesheet.rules.0, 0);
}

/// Parses and sanitizes an inline declaration list with a custom AST policy.
pub fn clean_declaration_list_with_policy(
    input: &str,
    policy: &dyn CssSanitizationPolicy,
) -> String {
    clean_declaration_list_with_policy_and_options(input, policy, SanitizeOptions::default())
}

/// Parses and sanitizes an inline declaration list with a custom AST policy and
/// options.
pub fn clean_declaration_list_with_policy_and_options(
    input: &str,
    policy: &dyn CssSanitizationPolicy,
    options: SanitizeOptions,
) -> String {
    let parser_options = ParserOptions {
        error_recovery: true,
        ..ParserOptions::default()
    };

    let Ok(mut block) = DeclarationBlock::parse_string(input, parser_options) else {
        return String::new();
    };

    sanitize_declaration_block_ast_with_options(&mut block, policy, options);

    if block.is_empty() {
        return String::new();
    }

    serialize_declaration_block(&block).unwrap_or_default()
}

/// Parses and sanitizes a full stylesheet with a custom AST policy.
pub fn clean_stylesheet_with_policy(input: &str, policy: &dyn CssSanitizationPolicy) -> String {
    clean_stylesheet_with_policy_and_options(input, policy, SanitizeOptions::default())
}

/// Parses and sanitizes a full stylesheet with a custom AST policy and options.
pub fn clean_stylesheet_with_policy_and_options(
    input: &str,
    policy: &dyn CssSanitizationPolicy,
    options: SanitizeOptions,
) -> String {
    let parser_options = ParserOptions {
        error_recovery: true,
        ..ParserOptions::default()
    };

    let Ok(mut stylesheet) = StyleSheet::parse(input, parser_options) else {
        return String::new();
    };

    sanitize_stylesheet_ast_with_options(&mut stylesheet, policy, options);

    if stylesheet.rules.0.is_empty() {
        return String::new();
    }

    match stylesheet.to_css(PrinterOptions::default()) {
        Ok(result) => result.code,
        Err(_) => String::new(),
    }
}
