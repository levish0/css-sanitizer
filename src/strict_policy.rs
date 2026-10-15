//! Built-in safe allowlist policy.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

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

type ResourceValidator = dyn Fn(ResourceRef<'_>, &ValueContext<'_>) -> ValueDecision + Send + Sync;

#[derive(Clone)]
struct ResourcePermission {
    validator: Arc<ResourceValidator>,
}

impl fmt::Debug for ResourcePermission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResourcePermission(..)")
    }
}

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
    resource_permissions: HashMap<ResourceUse, ResourcePermission>,
    allowed_font_face_descriptors: HashSet<FontFaceDescriptorKind>,
    allowed_font_palette_values_descriptors: HashSet<FontPaletteValuesDescriptorKind>,
    allowed_view_transition_descriptors: HashSet<ViewTransitionDescriptorKind>,
    allowed_functions: HashSet<String>,
    allow_important: bool,
    allow_unscoped_selectors: bool,
    properties_allowing_variables: HashSet<String>,
    properties_allowing_environment_variables: HashSet<String>,
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
            resource_permissions: HashMap::new(),
            allowed_font_face_descriptors: HashSet::new(),
            allowed_font_palette_values_descriptors: HashSet::new(),
            allowed_view_transition_descriptors: HashSet::new(),
            allowed_functions: HashSet::new(),
            allow_important: false,
            allow_unscoped_selectors: false,
            properties_allowing_variables: HashSet::new(),
            properties_allowing_environment_variables: HashSet::new(),
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

    /// Allows the named resource uses only when the validator also allows them.
    /// URLs are decoded but unresolved; dynamic references have no literal value.
    /// Use a URL parser to check schemes and origins, and reject dynamic
    /// references when they cannot be validated. Each call replaces validators
    /// for the specified uses, leaving other resource permissions unchanged.
    ///
    /// This does not inspect values supplied by an external CSS variable or
    /// custom function, and never enables `@import`.
    pub fn allow_resources<F>(mut self, uses: &[ResourceUse], validator: F) -> Self
    where
        F: Fn(ResourceRef<'_>, &ValueContext<'_>) -> ValueDecision + Send + Sync + 'static,
    {
        let permission = ResourcePermission {
            validator: Arc::new(validator),
        };
        self.resource_permissions
            .extend(uses.iter().map(|use_kind| (*use_kind, permission.clone())));
        self
    }

    /// Trusts all URLs and dynamic resource references for the named uses.
    /// No URL scheme, origin, relative-reference, or destination check is made.
    /// Prefer [`Self::allow_resources`] with an explicit validator for untrusted CSS.
    /// This never enables `@import`.
    pub fn dangerously_allow_unrestricted_resources(self, uses: &[ResourceUse]) -> Self {
        self.allow_resources(uses, |_, _| ValueDecision::Allow)
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

    /// Allows CSS variables only in the named properties. This does not allow
    /// the properties themselves or enable variables in descriptors.
    /// Callers still trust the variables' external values in these properties;
    /// do not include resource-capable properties without that trust.
    pub fn allow_variables(mut self, properties: &[&str]) -> Self {
        self.properties_allowing_variables
            .extend(properties.iter().map(|name| canonical_property_name(name)));
        self
    }

    /// Trusts environment-variable substitutions only in the named properties.
    /// This does not allow the properties themselves or enable descriptors.
    /// Fallbacks are guarded, but external substituted values cannot be resolved.
    pub fn allow_environment_variables(mut self, properties: &[&str]) -> Self {
        self.properties_allowing_environment_variables
            .extend(properties.iter().map(|name| canonical_property_name(name)));
        self
    }

    /// Trusts the named generic functions, including values supplied by external
    /// custom functions. Resource functions still require resource permission.
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

    fn resource(&self, resource: ResourceRef<'_>, context: &ValueContext<'_>) -> ValueDecision {
        self.resource_permissions
            .get(&resource.use_kind)
            .map_or(ValueDecision::Deny, |permission| {
                (permission.validator)(resource, context)
            })
    }

    fn dynamic_value(
        &self,
        value: DynamicValueRef<'_, '_>,
        context: &ValueContext<'_>,
    ) -> ValueDecision {
        match value.kind() {
            DynamicValueKind::Variable
                if context.property.as_ref().is_some_and(|property| {
                    self.properties_allowing_variables.contains(property.name())
                }) =>
            {
                ValueDecision::Allow
            }
            DynamicValueKind::EnvironmentVariable
                if context.property.as_ref().is_some_and(|property| {
                    self.properties_allowing_environment_variables
                        .contains(property.name())
                }) =>
            {
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
