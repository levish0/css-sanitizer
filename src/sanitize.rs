use std::cell::Cell;

use crate::guard;
use crate::options::{ParseLimits, SanitizeOptions};
use crate::output::{SanitizeError, SanitizeOutput, SanitizeReport, SanitizedCss};
use crate::policy::{
    CssPolicy, DescriptorContext, DescriptorKind, FontFaceDescriptorKind,
    FontPaletteValuesDescriptorKind, ImportContext, ImportDecision, NodeDecision, PropertyContext,
    PropertyLocation, RuleContext, RuleKind, SelectorContext, SelectorLocation, ValueContext,
    ViewTransitionDescriptorKind,
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

fn serialize_declaration_block(block: &DeclarationBlock<'_>) -> Result<String, SanitizeError> {
    let mut output = String::new();
    let mut printer = Printer::new(&mut output, PrinterOptions::default());
    block
        .to_css(&mut printer)
        .map_err(|error| SanitizeError::Serialize(error.to_string()))?;
    Ok(output)
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

#[derive(Default)]
struct ReportCounters {
    dropped_rules: Cell<usize>,
    dropped_selector_lists: Cell<usize>,
    dropped_declarations: Cell<usize>,
    dropped_descriptors: Cell<usize>,
    rejected_values: Cell<usize>,
}

impl ReportCounters {
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
        }
    }
}

struct Engine<'p> {
    policy: &'p dyn CssPolicy,
    options: SanitizeOptions,
    report: ReportCounters,
}

impl<'p> Engine<'p> {
    fn new(policy: &'p dyn CssPolicy, options: SanitizeOptions) -> Self {
        Self {
            policy,
            options,
            report: ReportCounters::default(),
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
            ReportCounters::increment(&self.report.dropped_selector_lists);
        }
        keep
    }

    fn sanitize_property_vec(
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
                ReportCounters::increment(&self.report.dropped_declarations);
                return false;
            }

            // Structural policies may rewrite the property in place. Value and
            // resource decisions must observe the post-policy property kind.
            let key = property.property_id();
            let allowed = !self.options.enforce_value_guard
                || guard::property_allowed(
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
                );
            if !allowed {
                ReportCounters::increment(&self.report.dropped_declarations);
                ReportCounters::increment(&self.report.rejected_values);
            }
            allowed
        });
    }

    fn sanitize_declaration_block_inner(
        &self,
        block: &mut DeclarationBlock<'_>,
        location: PropertyLocation,
        rule: Option<RuleKind>,
        depth: usize,
    ) {
        self.sanitize_property_vec(&mut block.declarations, location, rule, depth, false);
        self.sanitize_property_vec(
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
                    guard::font_face_property_allowed(property, self.policy, context)
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
                    guard::font_palette_values_property_allowed(property, self.policy, context)
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
                    guard::view_transition_property_allowed(property, self.policy, context)
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
            ReportCounters::increment(&self.report.dropped_descriptors);
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
            ReportCounters::increment(&self.report.dropped_descriptors);
            ReportCounters::increment(&self.report.rejected_values);
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
                ReportCounters::increment(&self.report.dropped_rules);
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
                ReportCounters::increment(&self.report.dropped_rules);
                return false;
            }

            self.sanitize_declaration_block_inner(
                &mut rule.declarations,
                PropertyLocation::PageMargin,
                Some(RuleKind::Page),
                depth + 1,
            );
            let keep = !rule.declarations.is_empty();
            if !keep {
                ReportCounters::increment(&self.report.dropped_rules);
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
            ContainerCondition::Unknown(tokens) => {
                guard::token_list_allowed(tokens, self.policy, self.condition_value_context(depth))
            }
            ContainerCondition::Feature(_) | ContainerCondition::ScrollState(_) => true,
        }
    }

    fn style_query_allowed(&self, query: &mut StyleQuery<'_>, depth: usize) -> bool {
        match query {
            StyleQuery::Declaration(property) => {
                let mut context = self.condition_value_context(depth);
                context.property = Some(property.property_id());
                guard::property_allowed(property, self.policy, context)
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
        let prelude_allowed =
            guard::token_list_allowed(&mut rule.prelude, self.policy, context.clone());
        let block_allowed = rule
            .block
            .as_mut()
            .is_none_or(|block| guard::token_list_allowed(block, self.policy, context));
        if !prelude_allowed || !block_allowed {
            ReportCounters::increment(&self.report.rejected_values);
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
                self.sanitize_declaration_block_inner(
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
                    self.sanitize_declaration_block_inner(
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
                self.sanitize_declaration_block_inner(
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
            CssRule::CounterStyle(rule) => self.sanitize_declaration_block_inner(
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
                self.sanitize_declaration_block_inner(
                    &mut rule.style.declarations,
                    PropertyLocation::StyleRule,
                    Some(RuleKind::Nesting),
                    context.depth + 1,
                );
            }
            CssRule::NestedDeclarations(rule) => self.sanitize_declaration_block_inner(
                &mut rule.declarations,
                PropertyLocation::NestedDeclarations,
                Some(RuleKind::NestedDeclarations),
                context.depth + 1,
            ),
            CssRule::Viewport(rule) => self.sanitize_declaration_block_inner(
                &mut rule.declarations,
                PropertyLocation::Viewport,
                Some(RuleKind::Viewport),
                context.depth + 1,
            ),
            CssRule::PositionTry(rule) => self.sanitize_declaration_block_inner(
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
                    ReportCounters::increment(&self.report.rejected_values);
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
                    && !guard::parsed_component_allowed(
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
                    )
                {
                    ReportCounters::increment(&self.report.rejected_values);
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
        if depth > self.options.max_traversal_depth {
            self.report
                .dropped_rules
                .set(self.report.dropped_rules.get() + rules.len());
            rules.clear();
            return;
        }

        rules.retain_mut(|rule| {
            let context = RuleContext {
                kind: RuleKind::of(rule),
                depth,
            };
            if !matches!(self.policy.rule(rule, context.clone()), NodeDecision::Keep) {
                ReportCounters::increment(&self.report.dropped_rules);
                return false;
            }
            let keep = self.sanitize_rule_contents(rule, context) && !is_rule_empty(rule);
            if !keep {
                ReportCounters::increment(&self.report.dropped_rules);
            }
            keep
        });
    }

    fn report(&self) -> SanitizeReport {
        self.report.finish()
    }
}

pub fn sanitize_declaration_block_ast(
    block: &mut DeclarationBlock<'_>,
    policy: &dyn CssPolicy,
) -> SanitizeReport {
    sanitize_declaration_block_ast_with_options(block, policy, SanitizeOptions::default())
}

pub fn sanitize_declaration_block_ast_with_options(
    block: &mut DeclarationBlock<'_>,
    policy: &dyn CssPolicy,
    options: SanitizeOptions,
) -> SanitizeReport {
    let engine = Engine::new(policy, options);
    engine.sanitize_declaration_block_inner(block, PropertyLocation::DeclarationList, None, 0);
    engine.report()
}

pub fn sanitize_stylesheet_ast(
    stylesheet: &mut StyleSheet<'_>,
    policy: &dyn CssPolicy,
) -> SanitizeReport {
    sanitize_stylesheet_ast_with_options(stylesheet, policy, SanitizeOptions::default())
}

pub fn sanitize_stylesheet_ast_with_options(
    stylesheet: &mut StyleSheet<'_>,
    policy: &dyn CssPolicy,
    options: SanitizeOptions,
) -> SanitizeReport {
    let engine = Engine::new(policy, options);
    engine.sanitize_rule_list(&mut stylesheet.rules.0, 0);
    engine.report()
}

pub fn sanitize_declaration_list(
    input: &str,
    policy: &dyn CssPolicy,
) -> Result<SanitizeOutput, SanitizeError> {
    sanitize_declaration_list_with_options(input, policy, SanitizeOptions::default())
}

pub fn sanitize_declaration_list_with_options(
    input: &str,
    policy: &dyn CssPolicy,
    options: SanitizeOptions,
) -> Result<SanitizeOutput, SanitizeError> {
    enforce_parse_limits(input, options.parse_limits)?;
    let parse_limits = options.parse_limits;
    let parser_options = ParserOptions {
        error_recovery: options.error_recovery,
        flags: options.parser_flags.clone(),
        ..ParserOptions::default()
    };
    let mut block = DeclarationBlock::parse_string(input, parser_options)
        .map_err(|error| SanitizeError::Parse(error.to_string()))?;
    let report = sanitize_declaration_block_ast_with_options(&mut block, policy, options);
    let css = if block.is_empty() {
        String::new()
    } else {
        serialize_declaration_block(&block)?
    };
    validate_output_size(&css, parse_limits)?;
    Ok(SanitizeOutput {
        css: SanitizedCss::new(css),
        report,
    })
}

pub fn sanitize_stylesheet(
    input: &str,
    policy: &dyn CssPolicy,
) -> Result<SanitizeOutput, SanitizeError> {
    sanitize_stylesheet_with_options(input, policy, SanitizeOptions::default())
}

pub fn sanitize_stylesheet_with_options(
    input: &str,
    policy: &dyn CssPolicy,
    options: SanitizeOptions,
) -> Result<SanitizeOutput, SanitizeError> {
    enforce_parse_limits(input, options.parse_limits)?;
    let parse_limits = options.parse_limits;
    let parser_options = ParserOptions {
        error_recovery: options.error_recovery,
        flags: options.parser_flags.clone(),
        ..ParserOptions::default()
    };
    let mut stylesheet = StyleSheet::parse(input, parser_options)
        .map_err(|error| SanitizeError::Parse(error.to_string()))?;
    let report = sanitize_stylesheet_ast_with_options(&mut stylesheet, policy, options);
    let css = if stylesheet.rules.0.is_empty() {
        String::new()
    } else {
        stylesheet
            .to_css(PrinterOptions::default())
            .map_err(|error| SanitizeError::Serialize(error.to_string()))?
            .code
    };
    validate_output_size(&css, parse_limits)?;
    Ok(SanitizeOutput {
        css: SanitizedCss::new(css),
        report,
    })
}

fn validate_output_size(output: &str, limits: ParseLimits) -> Result<(), SanitizeError> {
    if output.len() > limits.max_output_bytes {
        return Err(SanitizeError::OutputTooLarge {
            actual: output.len(),
            max: limits.max_output_bytes,
        });
    }
    Ok(())
}

fn enforce_parse_limits(input: &str, limits: ParseLimits) -> Result<(), SanitizeError> {
    if input.len() > limits.max_input_bytes {
        return Err(SanitizeError::InputTooLarge {
            actual: input.len(),
            max: limits.max_input_bytes,
        });
    }

    #[derive(Clone, Copy)]
    enum State {
        Normal,
        SingleQuoted,
        DoubleQuoted,
        Comment,
    }

    let bytes = input.as_bytes();
    let mut state = State::Normal;
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        match state {
            State::Normal => match byte {
                b'/' if bytes.get(index + 1) == Some(&b'*') => {
                    state = State::Comment;
                    index += 1;
                }
                b'\'' => state = State::SingleQuoted,
                b'"' => state = State::DoubleQuoted,
                b'\\' => index += usize::from(index + 1 < bytes.len()),
                b'(' | b'[' | b'{' => {
                    depth += 1;
                    if depth > limits.max_nesting_depth {
                        return Err(SanitizeError::NestingTooDeep {
                            max: limits.max_nesting_depth,
                        });
                    }
                }
                b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                _ => {}
            },
            State::SingleQuoted => match byte {
                b'\\' => index += usize::from(index + 1 < bytes.len()),
                b'\'' => state = State::Normal,
                _ => {}
            },
            State::DoubleQuoted => match byte {
                b'\\' => index += usize::from(index + 1 < bytes.len()),
                b'"' => state = State::Normal,
                _ => {}
            },
            State::Comment => {
                if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    state = State::Normal;
                    index += 1;
                }
            }
        }
        index += 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_limit_ignores_delimiters_in_strings_and_comments() {
        let input = r#".x { content: \"((((\"; /* {{{{ */ color: red }"#;
        assert!(enforce_parse_limits(input, ParseLimits::default()).is_ok());
    }

    #[test]
    fn parse_limit_rejects_known_recursive_parser_shapes() {
        let inputs = [
            format!("{}a{}{{color:red}}", ":is(".repeat(129), ")".repeat(129)),
            format!("{}a{}{{color:red}}", ":not(".repeat(129), ")".repeat(129)),
            format!("{}color:red{}", "a{".repeat(129), "}".repeat(129)),
            format!(
                "{}a{{color:red}}{}",
                "@media all{".repeat(129),
                "}".repeat(129)
            ),
            format!("width:{}1px{}", "calc(".repeat(129), ")".repeat(129)),
            format!("width:{}1px{}", "min(".repeat(129), ")".repeat(129)),
        ];

        for input in inputs {
            assert_eq!(
                enforce_parse_limits(&input, ParseLimits::default()),
                Err(SanitizeError::NestingTooDeep { max: 128 })
            );
        }
    }
}
