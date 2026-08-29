//! Built-in safe allowlist policy.

use std::collections::{HashMap, HashSet};

use lightningcss::printer::PrinterOptions;
use lightningcss::properties::Property;
use lightningcss::properties::custom::{Token, TokenOrValue};
use lightningcss::rules::CssRule;
use lightningcss::rules::font_face::{FontFaceProperty, Source};
use lightningcss::rules::font_feature_values::FontFeatureSubrule;
use lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use lightningcss::rules::page::PageMarginRule;
use lightningcss::rules::view_transition::ViewTransitionProperty;
use lightningcss::selector::SelectorList;
use lightningcss::stylesheet::ParserOptions;

use crate::policy::{
    CssPolicy, DescriptorContext, DescriptorKind, DynamicValueKind, DynamicValueRef,
    FontFaceDescriptorKind, FontPaletteValuesDescriptorKind, ImportContext, ImportDecision,
    NodeDecision, PropertyContext, ResourceRef, ResourceUse, RuleContext, RuleKind,
    SelectorContext, ValueContext, ValueDecision, ViewTransitionDescriptorKind,
};

/// A safe-by-default convenience preset.
///
/// Complex policies should implement [`CssPolicy`] directly. This preset never
/// admits opaque rules and keeps unscoped selectors, passthrough imports,
/// resources, dynamic values, and local font probing behind distinct explicit
/// capabilities.
#[derive(Debug, Clone)]
pub struct StrictPolicy {
    allowed_properties: HashSet<String>,
    allowed_rules: HashSet<RuleKind>,
    allowed_values: HashMap<String, HashSet<String>>,
    allowed_resources: HashSet<ResourceUse>,
    allowed_font_face_descriptors: HashSet<FontFaceDescriptorKind>,
    allowed_font_palette_values_descriptors: HashSet<FontPaletteValuesDescriptorKind>,
    allowed_view_transition_descriptors: HashSet<ViewTransitionDescriptorKind>,
    allowed_functions: HashSet<String>,
    allow_important: bool,
    allow_unscoped_selectors: bool,
    allow_variables: bool,
    allow_environment_variables: bool,
    allow_local_fonts: bool,
    allow_page_margin_rules: bool,
    allow_font_feature_values_subrules: bool,
    allow_passthrough_imports: bool,
}

impl Default for StrictPolicy {
    fn default() -> Self {
        Self {
            allowed_properties: HashSet::new(),
            allowed_rules: HashSet::from([
                RuleKind::Style,
                RuleKind::Nesting,
                RuleKind::NestedDeclarations,
            ]),
            allowed_values: HashMap::new(),
            allowed_resources: HashSet::new(),
            allowed_font_face_descriptors: HashSet::new(),
            allowed_font_palette_values_descriptors: HashSet::new(),
            allowed_view_transition_descriptors: HashSet::new(),
            allowed_functions: HashSet::new(),
            allow_important: false,
            allow_unscoped_selectors: false,
            allow_variables: false,
            allow_environment_variables: false,
            allow_local_fonts: false,
            allow_page_margin_rules: false,
            allow_font_feature_values_subrules: false,
            allow_passthrough_imports: false,
        }
    }
}

impl StrictPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allow_properties(mut self, properties: &[&str]) -> Self {
        self.allowed_properties
            .extend(properties.iter().map(|name| canonical_property_name(name)));
        self
    }

    pub fn allow_rules(mut self, rules: &[RuleKind]) -> Self {
        self.allowed_rules
            .extend(rules.iter().copied().filter(|kind| {
                !matches!(
                    kind,
                    RuleKind::Import | RuleKind::Unknown | RuleKind::Custom | RuleKind::Ignored
                )
            }));
        self
    }

    pub fn allow_values(mut self, property: &str, values: &[&str]) -> Self {
        self.allowed_values
            .entry(canonical_property_name(property))
            .or_default()
            .extend(values.iter().map(|value| (*value).to_owned()));
        self
    }

    /// Allows parsed selectors without rewriting them into a caller-owned scope.
    pub fn allow_unscoped_selectors(mut self) -> Self {
        self.allow_unscoped_selectors = true;
        self
    }

    pub fn allow_resources(mut self, uses: &[ResourceUse]) -> Self {
        self.allowed_resources.extend(uses.iter().copied());
        self
    }

    pub fn allow_font_face_descriptors(mut self, descriptors: &[FontFaceDescriptorKind]) -> Self {
        self.allowed_font_face_descriptors
            .extend(descriptors.iter().copied());
        self
    }

    pub fn allow_font_palette_values_descriptors(
        mut self,
        descriptors: &[FontPaletteValuesDescriptorKind],
    ) -> Self {
        self.allowed_font_palette_values_descriptors
            .extend(descriptors.iter().copied());
        self
    }

    pub fn allow_view_transition_descriptors(
        mut self,
        descriptors: &[ViewTransitionDescriptorKind],
    ) -> Self {
        self.allowed_view_transition_descriptors
            .extend(descriptors.iter().copied());
        self
    }

    pub fn allow_page_margin_rules(mut self) -> Self {
        self.allow_page_margin_rules = true;
        self
    }

    pub fn allow_font_feature_values_subrules(mut self) -> Self {
        self.allow_font_feature_values_subrules = true;
        self
    }

    pub fn allow_local_fonts(mut self) -> Self {
        self.allow_local_fonts = true;
        self
    }

    pub fn allow_variables(mut self) -> Self {
        self.allow_variables = true;
        self
    }

    pub fn allow_environment_variables(mut self) -> Self {
        self.allow_environment_variables = true;
        self
    }

    pub fn allow_functions(mut self, functions: &[&str]) -> Self {
        self.allowed_functions
            .extend(functions.iter().map(|function| {
                if function.starts_with("--") {
                    (*function).to_owned()
                } else {
                    function.to_ascii_lowercase()
                }
            }));
        self
    }

    pub fn allow_important(mut self) -> Self {
        self.allow_important = true;
        self
    }

    /// Preserves `@import` without fetching or sanitizing the imported sheet.
    pub fn dangerously_allow_passthrough_imports(mut self) -> Self {
        self.allow_passthrough_imports = true;
        self
    }

    fn matches_allowed_value(property: &Property<'_>, allowed_value: &str) -> bool {
        let Ok(parsed) = Property::parse_string(
            property.property_id(),
            allowed_value,
            ParserOptions::default(),
        ) else {
            return false;
        };
        match (
            parsed.value_to_css_string(PrinterOptions::default()),
            property.value_to_css_string(PrinterOptions::default()),
        ) {
            (Ok(expected), Ok(actual)) => expected == actual,
            _ => false,
        }
    }
}

impl CssPolicy for StrictPolicy {
    fn rule(&self, _rule: &mut CssRule<'_>, context: RuleContext) -> NodeDecision {
        if context.kind == RuleKind::Import && self.allow_passthrough_imports {
            return NodeDecision::Keep;
        }
        if self.allowed_rules.contains(&context.kind) {
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
        if self.allow_unscoped_selectors {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn property(&self, property: &mut Property<'_>, context: PropertyContext<'_>) -> NodeDecision {
        if context.important && !self.allow_important {
            return NodeDecision::Drop;
        }
        let name = context.key.name();
        if !self.allowed_properties.contains(name) {
            return NodeDecision::Drop;
        }
        if let Some(allowed_values) = self.allowed_values.get(name)
            && !allowed_values
                .iter()
                .any(|allowed| Self::matches_allowed_value(property, allowed))
        {
            return NodeDecision::Drop;
        }
        NodeDecision::Keep
    }

    fn font_face_descriptor(
        &self,
        property: &mut FontFaceProperty<'_>,
        context: DescriptorContext,
    ) -> NodeDecision {
        let DescriptorKind::FontFace(kind) = context.kind else {
            return NodeDecision::Drop;
        };
        if !self.allowed_font_face_descriptors.contains(&kind) {
            return NodeDecision::Drop;
        }
        if let FontFaceProperty::Source(sources) = property
            && !self.allow_local_fonts
        {
            sources.retain(|source| !matches!(source, Source::Local(_)));
            if sources.is_empty() {
                return NodeDecision::Drop;
            }
        }
        NodeDecision::Keep
    }

    fn font_palette_values_descriptor(
        &self,
        _property: &mut FontPaletteValuesProperty<'_>,
        context: DescriptorContext,
    ) -> NodeDecision {
        let DescriptorKind::FontPaletteValues(kind) = context.kind else {
            return NodeDecision::Drop;
        };
        if self.allowed_font_palette_values_descriptors.contains(&kind) {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn view_transition_descriptor(
        &self,
        _property: &mut ViewTransitionProperty<'_>,
        context: DescriptorContext,
    ) -> NodeDecision {
        let DescriptorKind::ViewTransition(kind) = context.kind else {
            return NodeDecision::Drop;
        };
        if self.allowed_view_transition_descriptors.contains(&kind) {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn page_margin_rule(
        &self,
        _rule: &mut PageMarginRule<'_>,
        _context: RuleContext,
    ) -> NodeDecision {
        if self.allow_page_margin_rules {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn font_feature_values_subrule(
        &self,
        _rule: &mut FontFeatureSubrule<'_>,
        _context: RuleContext,
    ) -> NodeDecision {
        if self.allow_font_feature_values_subrules {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn import(&self, _context: ImportContext<'_>) -> ImportDecision {
        if self.allow_passthrough_imports {
            ImportDecision::AllowPassthrough
        } else {
            ImportDecision::Deny
        }
    }

    fn resource(&self, resource: ResourceRef<'_>, _context: &ValueContext<'_>) -> ValueDecision {
        if self.allowed_resources.contains(&resource.use_kind) {
            ValueDecision::Allow
        } else {
            ValueDecision::Deny
        }
    }

    fn dynamic_value(
        &self,
        value: DynamicValueRef<'_, '_>,
        _context: &ValueContext<'_>,
    ) -> ValueDecision {
        match value.kind() {
            DynamicValueKind::Variable if self.allow_variables => ValueDecision::Allow,
            DynamicValueKind::EnvironmentVariable if self.allow_environment_variables => {
                ValueDecision::Allow
            }
            DynamicValueKind::Function => {
                let Some(name) = value.function_name() else {
                    return ValueDecision::Deny;
                };
                if name.eq_ignore_ascii_case("expression") {
                    return ValueDecision::Deny;
                }
                let allowed = if name.starts_with("--") {
                    self.allowed_functions.contains(name)
                } else {
                    self.allowed_functions.contains(&name.to_ascii_lowercase())
                };
                if allowed {
                    ValueDecision::Allow
                } else {
                    ValueDecision::Deny
                }
            }
            _ => ValueDecision::Deny,
        }
    }

    fn token(&self, token: &TokenOrValue<'_>, _context: &ValueContext<'_>) -> ValueDecision {
        match token {
            TokenOrValue::Token(Token::BadUrl(_) | Token::Function(_)) => ValueDecision::Deny,
            TokenOrValue::Function(function)
                if function.name.0.eq_ignore_ascii_case("expression") =>
            {
                ValueDecision::Deny
            }
            _ => ValueDecision::Allow,
        }
    }
}

fn canonical_property_name(name: &str) -> String {
    if name.starts_with("--") {
        name.to_owned()
    } else {
        name.to_ascii_lowercase()
    }
}
