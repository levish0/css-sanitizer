//! Applies structural policy hooks, validates retained values, and prunes the CSS AST.

use std::cell::Cell;

use crate::options::SanitizeOptions;
use crate::output::SanitizeReport;
use crate::policy::{
    CssPolicy, DescriptorContext, DescriptorKind, FontFaceDescriptorKind,
    FontPaletteValuesDescriptorKind, ImportContext, ImportDecision, NodeDecision, PropertyContext,
    PropertyLocation, RuleContext, RuleKind, SelectorContext, SelectorLocation, ValueContext,
    ViewTransitionDescriptorKind,
};
use crate::value_guard;
use lightningcss::declaration::DeclarationBlock;
use lightningcss::rules::CssRule;
use lightningcss::rules::container::{ContainerCondition, StyleQuery};
use lightningcss::rules::font_face::FontFaceProperty;
use lightningcss::rules::font_feature_values::FontFeatureValuesRule;
use lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use lightningcss::rules::page::PageMarginRule;
use lightningcss::rules::view_transition::ViewTransitionProperty;
use lightningcss::selector::SelectorList;
use lightningcss::stylesheet::StyleSheet;

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

#[derive(Default)]
struct SanitizationCounters {
    dropped_rules: Cell<usize>,
    dropped_selector_lists: Cell<usize>,
    dropped_declarations: Cell<usize>,
    dropped_descriptors: Cell<usize>,
    rejected_values: Cell<usize>,
}

impl SanitizationCounters {
    fn increment(cell: &Cell<usize>) {
        cell.set(cell.get() + 1);
    }

    fn finish(&self) -> SanitizeReport {
        SanitizeReport {
            dropped_rules: self.dropped_rules.get(),
            dropped_selector_lists: self.dropped_selector_lists.get(),
            dropped_declarations: self.dropped_declarations.get(),
            dropped_descriptors: self.dropped_descriptors.get(),
            rejected_values: self.rejected_values.get(),
            ..SanitizeReport::default()
        }
    }
}

pub(crate) struct AstSanitizer<'p> {
    policy: &'p dyn CssPolicy,
    options: SanitizeOptions,
    counters: SanitizationCounters,
}

impl<'p> AstSanitizer<'p> {
    pub(crate) fn new(policy: &'p dyn CssPolicy, options: SanitizeOptions) -> Self {
        Self {
            policy,
            options,
            counters: SanitizationCounters::default(),
        }
    }

    fn sanitize_selector_list(
        &self,
        selectors: &mut SelectorList<'_>,
        rule: RuleKind,
        location: SelectorLocation,
        depth: usize,
    ) -> bool {
        let keep = matches!(
            self.policy.selector(
                selectors,
                SelectorContext {
                    rule,
                    location,
                    depth,
                },
            ),
            NodeDecision::Keep
        );
        if !keep {
            SanitizationCounters::increment(&self.counters.dropped_selector_lists);
        }
        keep
    }

    fn sanitize_properties(
        &self,
        properties: &mut Vec<lightningcss::properties::Property<'_>>,
        location: PropertyLocation,
        rule: Option<RuleKind>,
        depth: usize,
        important: bool,
    ) {
        properties.retain_mut(|property| {
            let key = property.property_id();
            let decision = self.policy.property(
                property,
                PropertyContext {
                    key: key.clone(),
                    rule,
                    location,
                    depth,
                    important,
                },
            );
            if matches!(decision, NodeDecision::Drop) {
                SanitizationCounters::increment(&self.counters.dropped_declarations);
                return false;
            }

            // Structural policies may rewrite the property in place. Value and
            // resource decisions must observe the post-policy property kind.
            let key = property.property_id();
            let allowed = !self.options.enforce_value_guard
                || value_guard::property_allowed(
                    property,
                    self.policy,
                    ValueContext {
                        property: Some(key),
                        descriptor: None,
                        rule,
                        location,
                        depth,
                        important,
                    },
                    self.options.limits,
                );
            if !allowed {
                SanitizationCounters::increment(&self.counters.dropped_declarations);
                SanitizationCounters::increment(&self.counters.rejected_values);
            }
            allowed
        });
    }

    fn sanitize_declaration_block_contents(
        &self,
        block: &mut DeclarationBlock<'_>,
        location: PropertyLocation,
        rule: Option<RuleKind>,
        depth: usize,
    ) {
        self.sanitize_properties(&mut block.declarations, location, rule, depth, false);
        self.sanitize_properties(
            &mut block.important_declarations,
            location,
            rule,
            depth,
            true,
        );
    }

    fn sanitize_font_face_properties<'i>(
        &self,
        properties: &mut Vec<FontFaceProperty<'i>>,
        depth: usize,
    ) {
        properties.retain_mut(|property| {
            let descriptor = DescriptorKind::FontFace(FontFaceDescriptorKind::of(property));
            let decision = self.policy.font_face_descriptor(
                property,
                DescriptorContext {
                    rule: RuleKind::FontFace,
                    kind: descriptor,
                    depth,
                },
            );
            let descriptor = DescriptorKind::FontFace(FontFaceDescriptorKind::of(property));
            self.descriptor_allowed(
                property,
                decision,
                descriptor,
                depth,
                |property, context| {
                    value_guard::font_face_property_allowed(
                        property,
                        self.policy,
                        context,
                        self.options.limits,
                    )
                },
            )
        });
    }

    fn sanitize_font_palette_values_properties<'i>(
        &self,
        properties: &mut Vec<FontPaletteValuesProperty<'i>>,
        depth: usize,
    ) {
        properties.retain_mut(|property| {
            let descriptor =
                DescriptorKind::FontPaletteValues(FontPaletteValuesDescriptorKind::of(property));
            let decision = self.policy.font_palette_values_descriptor(
                property,
                DescriptorContext {
                    rule: RuleKind::FontPaletteValues,
                    kind: descriptor,
                    depth,
                },
            );
            let descriptor =
                DescriptorKind::FontPaletteValues(FontPaletteValuesDescriptorKind::of(property));
            self.descriptor_allowed(
                property,
                decision,
                descriptor,
                depth,
                |property, context| {
                    value_guard::font_palette_values_property_allowed(
                        property,
                        self.policy,
                        context,
                        self.options.limits,
                    )
                },
            )
        });
    }

    fn sanitize_view_transition_properties<'i>(
        &self,
        properties: &mut Vec<ViewTransitionProperty<'i>>,
        depth: usize,
    ) {
        properties.retain_mut(|property| {
            let descriptor =
                DescriptorKind::ViewTransition(ViewTransitionDescriptorKind::of(property));
            let decision = self.policy.view_transition_descriptor(
                property,
                DescriptorContext {
                    rule: RuleKind::ViewTransition,
                    kind: descriptor,
                    depth,
                },
            );
            let descriptor =
                DescriptorKind::ViewTransition(ViewTransitionDescriptorKind::of(property));
            self.descriptor_allowed(
                property,
                decision,
                descriptor,
                depth,
                |property, context| {
                    value_guard::view_transition_property_allowed(
                        property,
                        self.policy,
                        context,
                        self.options.limits,
                    )
                },
            )
        });
    }

    fn descriptor_allowed<'i, T>(
        &self,
        property: &mut T,
        decision: NodeDecision,
        descriptor: DescriptorKind,
        depth: usize,
        value_guard: impl FnOnce(&mut T, ValueContext<'i>) -> bool,
    ) -> bool {
        if matches!(decision, NodeDecision::Drop) {
            SanitizationCounters::increment(&self.counters.dropped_descriptors);
            return false;
        }
        let allowed = !self.options.enforce_value_guard
            || value_guard(
                property,
                ValueContext {
                    property: None,
                    descriptor: Some(descriptor),
                    rule: Some(match descriptor {
                        DescriptorKind::FontFace(_) => RuleKind::FontFace,
                        DescriptorKind::FontPaletteValues(_) => RuleKind::FontPaletteValues,
                        DescriptorKind::ViewTransition(_) => RuleKind::ViewTransition,
                    }),
                    location: match descriptor {
                        DescriptorKind::FontFace(_) => PropertyLocation::FontFaceDescriptor,
                        DescriptorKind::FontPaletteValues(_) => {
                            PropertyLocation::FontPaletteValuesDescriptor
                        }
                        DescriptorKind::ViewTransition(_) => {
                            PropertyLocation::ViewTransitionDescriptor
                        }
                    },
                    depth,
                    important: false,
                },
            );
        if !allowed {
            SanitizationCounters::increment(&self.counters.dropped_descriptors);
            SanitizationCounters::increment(&self.counters.rejected_values);
        }
        allowed
    }

    fn sanitize_font_feature_values_subrules(
        &self,
        rule: &mut FontFeatureValuesRule<'_>,
        depth: usize,
    ) {
        rule.rules.retain(|_, subrule| {
            let keep = matches!(
                self.policy.font_feature_values_subrule(
                    subrule,
                    RuleContext {
                        kind: RuleKind::FontFeatureValues,
                        depth,
                    },
                ),
                NodeDecision::Keep
            ) && !subrule.declarations.is_empty();
            if !keep {
                SanitizationCounters::increment(&self.counters.dropped_rules);
            }
            keep
        });
    }

    fn sanitize_page_margin_rules(&self, rules: &mut Vec<PageMarginRule<'_>>, depth: usize) {
        rules.retain_mut(|rule| {
            if !matches!(
                self.policy.page_margin_rule(
                    rule,
                    RuleContext {
                        kind: RuleKind::Page,
                        depth,
                    },
                ),
                NodeDecision::Keep
            ) {
                SanitizationCounters::increment(&self.counters.dropped_rules);
                return false;
            }

            self.sanitize_declaration_block_contents(
                &mut rule.declarations,
                PropertyLocation::PageMargin,
                Some(RuleKind::Page),
                depth + 1,
            );
            let keep = !rule.declarations.is_empty();
            if !keep {
                SanitizationCounters::increment(&self.counters.dropped_rules);
            }
            keep
        });
    }

    fn condition_value_context(&self, depth: usize) -> ValueContext<'static> {
        ValueContext {
            property: None,
            descriptor: None,
            rule: Some(RuleKind::Container),
            location: PropertyLocation::ContainerCondition,
            depth,
            important: false,
        }
    }

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
            ContainerCondition::Unknown(tokens) => value_guard::token_list_allowed(
                tokens,
                self.policy,
                self.condition_value_context(depth),
                self.options.limits,
            ),
            ContainerCondition::Feature(_) | ContainerCondition::ScrollState(_) => true,
        }
    }

    fn style_query_allowed(&self, query: &mut StyleQuery<'_>, depth: usize) -> bool {
        match query {
            StyleQuery::Declaration(property) => {
                let mut context = self.condition_value_context(depth);
                context.property = Some(property.property_id());
                value_guard::property_allowed(property, self.policy, context, self.options.limits)
            }
            StyleQuery::Not(inner) => self.style_query_allowed(inner, depth),
            StyleQuery::Operation { conditions, .. } => conditions
                .iter_mut()
                .all(|condition| self.style_query_allowed(condition, depth)),
            StyleQuery::Property(_) => true,
        }
    }

    fn opaque_rule_allowed(
        &self,
        rule: &mut lightningcss::rules::unknown::UnknownAtRule<'_>,
        depth: usize,
    ) -> bool {
        if !self.options.enforce_value_guard {
            return true;
        }
        let context = ValueContext {
            property: None,
            descriptor: None,
            rule: Some(RuleKind::Unknown),
            location: PropertyLocation::OpaqueAtRule,
            depth,
            important: false,
        };
        let prelude_allowed = value_guard::token_list_allowed(
            &mut rule.prelude,
            self.policy,
            context.clone(),
            self.options.limits,
        );
        let block_allowed = rule.block.as_mut().is_none_or(|block| {
            value_guard::token_list_allowed(block, self.policy, context, self.options.limits)
        });
        if !prelude_allowed || !block_allowed {
            SanitizationCounters::increment(&self.counters.rejected_values);
        }
        prelude_allowed && block_allowed
    }

    fn sanitize_rule_contents(&self, rule: &mut CssRule<'_>, context: RuleContext) -> bool {
        match rule {
            CssRule::Style(rule) => {
                if !self.sanitize_selector_list(
                    &mut rule.selectors,
                    RuleKind::Style,
                    SelectorLocation::StyleRule,
                    context.depth + 1,
                ) {
                    return false;
                }
                self.sanitize_declaration_block_contents(
                    &mut rule.declarations,
                    PropertyLocation::StyleRule,
                    Some(RuleKind::Style),
                    context.depth + 1,
                );
                self.sanitize_rule_list(&mut rule.rules.0, context.depth + 1);
            }
            CssRule::Media(rule) => self.sanitize_rule_list(&mut rule.rules.0, context.depth + 1),
            CssRule::Keyframes(rule) => {
                for keyframe in &mut rule.keyframes {
                    self.sanitize_declaration_block_contents(
                        &mut keyframe.declarations,
                        PropertyLocation::Keyframe,
                        Some(RuleKind::Keyframes),
                        context.depth + 1,
                    );
                }
                rule.keyframes
                    .retain(|keyframe| !keyframe.declarations.is_empty());
            }
            CssRule::FontFace(rule) => {
                self.sanitize_font_face_properties(&mut rule.properties, context.depth + 1)
            }
            CssRule::FontPaletteValues(rule) => self
                .sanitize_font_palette_values_properties(&mut rule.properties, context.depth + 1),
            CssRule::FontFeatureValues(rule) => {
                self.sanitize_font_feature_values_subrules(rule, context.depth + 1)
            }
            CssRule::Page(rule) => {
                self.sanitize_declaration_block_contents(
                    &mut rule.declarations,
                    PropertyLocation::Page,
                    Some(RuleKind::Page),
                    context.depth + 1,
                );
                self.sanitize_page_margin_rules(&mut rule.rules, context.depth + 1);
            }
            CssRule::Supports(rule) => {
                self.sanitize_rule_list(&mut rule.rules.0, context.depth + 1)
            }
            CssRule::CounterStyle(rule) => self.sanitize_declaration_block_contents(
                &mut rule.declarations,
                PropertyLocation::CounterStyle,
                Some(RuleKind::CounterStyle),
                context.depth + 1,
            ),
            CssRule::MozDocument(rule) => {
                self.sanitize_rule_list(&mut rule.rules.0, context.depth + 1)
            }
            CssRule::Nesting(rule) => {
                if !self.sanitize_selector_list(
                    &mut rule.style.selectors,
                    RuleKind::Nesting,
                    SelectorLocation::Nesting,
                    context.depth + 1,
                ) {
                    return false;
                }
                self.sanitize_rule_list(&mut rule.style.rules.0, context.depth + 1);
                self.sanitize_declaration_block_contents(
                    &mut rule.style.declarations,
                    PropertyLocation::StyleRule,
                    Some(RuleKind::Nesting),
                    context.depth + 1,
                );
            }
            CssRule::NestedDeclarations(rule) => self.sanitize_declaration_block_contents(
                &mut rule.declarations,
                PropertyLocation::NestedDeclarations,
                Some(RuleKind::NestedDeclarations),
                context.depth + 1,
            ),
            CssRule::Viewport(rule) => self.sanitize_declaration_block_contents(
                &mut rule.declarations,
                PropertyLocation::Viewport,
                Some(RuleKind::Viewport),
                context.depth + 1,
            ),
            CssRule::PositionTry(rule) => self.sanitize_declaration_block_contents(
                &mut rule.declarations,
                PropertyLocation::PositionTry,
                Some(RuleKind::PositionTry),
                context.depth + 1,
            ),
            CssRule::LayerBlock(rule) => {
                self.sanitize_rule_list(&mut rule.rules.0, context.depth + 1)
            }
            CssRule::Container(rule) => {
                if self.options.enforce_value_guard
                    && let Some(condition) = &mut rule.condition
                    && !self.container_condition_allowed(condition, context.depth + 1)
                {
                    SanitizationCounters::increment(&self.counters.rejected_values);
                    return false;
                }
                self.sanitize_rule_list(&mut rule.rules.0, context.depth + 1);
            }
            CssRule::Scope(rule) => {
                if let Some(start) = &mut rule.scope_start
                    && !self.sanitize_selector_list(
                        start,
                        RuleKind::Scope,
                        SelectorLocation::ScopeStart,
                        context.depth + 1,
                    )
                {
                    return false;
                }
                if let Some(end) = &mut rule.scope_end
                    && !self.sanitize_selector_list(
                        end,
                        RuleKind::Scope,
                        SelectorLocation::ScopeEnd,
                        context.depth + 1,
                    )
                {
                    return false;
                }
                self.sanitize_rule_list(&mut rule.rules.0, context.depth + 1);
            }
            CssRule::StartingStyle(rule) => {
                self.sanitize_rule_list(&mut rule.rules.0, context.depth + 1)
            }
            CssRule::ViewTransition(rule) => {
                self.sanitize_view_transition_properties(&mut rule.properties, context.depth + 1)
            }
            CssRule::Property(rule) => {
                if self.options.enforce_value_guard
                    && let Some(initial_value) = &mut rule.initial_value
                    && !value_guard::parsed_component_allowed(
                        initial_value,
                        self.policy,
                        ValueContext {
                            property: None,
                            descriptor: None,
                            rule: Some(RuleKind::PropertyRegistration),
                            location: PropertyLocation::PropertyInitialValue,
                            depth: context.depth + 1,
                            important: false,
                        },
                        self.options.limits,
                    )
                {
                    SanitizationCounters::increment(&self.counters.rejected_values);
                    return false;
                }
            }
            CssRule::Import(rule) => {
                return matches!(
                    self.policy.import(ImportContext {
                        url: rule.url.as_ref(),
                        depth: context.depth + 1,
                    }),
                    ImportDecision::AllowPassthrough
                );
            }
            CssRule::Unknown(rule) => {
                return self.opaque_rule_allowed(rule, context.depth + 1);
            }
            // The public sanitizer accepts lightningcss's default at-rule type.
            // A `Custom(DefaultAtRule)` has no inspectable payload and cannot be
            // serialized successfully, so it is never retained.
            CssRule::Custom(_) => return false,
            CssRule::Namespace(_)
            | CssRule::CustomMedia(_)
            | CssRule::LayerStatement(_)
            | CssRule::Ignored => {}
        }
        true
    }

    fn sanitize_rule_list(&self, rules: &mut Vec<CssRule<'_>>, depth: usize) {
        if depth > self.options.limits.max_rule_depth {
            self.counters
                .dropped_rules
                .set(self.counters.dropped_rules.get() + rules.len());
            rules.clear();
            return;
        }

        rules.retain_mut(|rule| {
            let context = RuleContext {
                kind: RuleKind::of(rule),
                depth,
            };
            if !matches!(self.policy.rule(rule, context.clone()), NodeDecision::Keep) {
                SanitizationCounters::increment(&self.counters.dropped_rules);
                return false;
            }
            let keep = self.sanitize_rule_contents(rule, context) && !is_rule_empty(rule);
            if !keep {
                SanitizationCounters::increment(&self.counters.dropped_rules);
            }
            keep
        });
    }

    pub(crate) fn sanitize_declaration_block(
        self,
        block: &mut DeclarationBlock<'_>,
    ) -> SanitizeReport {
        self.sanitize_declaration_block_contents(block, PropertyLocation::DeclarationList, None, 0);
        self.counters.finish()
    }

    pub(crate) fn sanitize_stylesheet(self, stylesheet: &mut StyleSheet<'_>) -> SanitizeReport {
        self.sanitize_rule_list(&mut stylesheet.rules.0, 0);
        self.counters.finish()
    }
}
