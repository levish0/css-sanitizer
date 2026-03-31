#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;

use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::properties::custom::{EnvironmentVariable, Function, Variable};
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::rules::font_face::FontFaceProperty;
use css_sanitizer::lightningcss::selector::{Component, SelectorList};
use css_sanitizer::lightningcss::stylesheet::ParserOptions;
use css_sanitizer::lightningcss::values::image::Image;
use css_sanitizer::lightningcss::values::url::Url;
use css_sanitizer::lightningcss::visitor::{Visit, VisitTypes, Visitor};
use css_sanitizer::{
    CssSanitizationPolicy, DescriptorContext, NodeAction, PropertyContext, RuleContext,
    SelectorContext,
};

#[derive(Debug, Default)]
struct PropertySecurityScan {
    has_expression: bool,
    has_url: bool,
    has_var: bool,
    has_env: bool,
}

impl PropertySecurityScan {
    fn inspect(property: &mut Property<'_>) -> Self {
        let mut scan = Self::default();
        property
            .visit(&mut scan)
            .expect("property security scan should not fail");
        scan
    }

    fn scan_image(&mut self, image: &Image<'_>) {
        match image {
            Image::Url(_) => {
                self.has_url = true;
            }
            Image::ImageSet(image_set) => {
                for option in &image_set.options {
                    self.scan_image(&option.image);
                }
            }
            Image::Gradient(_) | Image::None => {}
        }
    }
}

impl<'i> Visitor<'i> for PropertySecurityScan {
    type Error = Infallible;

    fn visit_types(&self) -> VisitTypes {
        VisitTypes::URLS
            | VisitTypes::IMAGES
            | VisitTypes::VARIABLES
            | VisitTypes::ENVIRONMENT_VARIABLES
            | VisitTypes::FUNCTIONS
    }

    fn visit_url(&mut self, _url: &mut Url<'i>) -> Result<(), Self::Error> {
        self.has_url = true;
        Ok(())
    }

    fn visit_image(&mut self, image: &mut Image<'i>) -> Result<(), Self::Error> {
        self.scan_image(image);
        image.visit_children(self)
    }

    fn visit_variable(&mut self, variable: &mut Variable<'i>) -> Result<(), Self::Error> {
        self.has_var = true;
        variable.visit_children(self)
    }

    fn visit_environment_variable(
        &mut self,
        environment_variable: &mut EnvironmentVariable<'i>,
    ) -> Result<(), Self::Error> {
        self.has_env = true;
        environment_variable.visit_children(self)
    }

    fn visit_function(&mut self, function: &mut Function<'i>) -> Result<(), Self::Error> {
        if function.name.0.eq_ignore_ascii_case("expression") {
            self.has_expression = true;
        }

        function.visit_children(self)
    }
}

#[derive(Default)]
pub struct StrictPolicy {
    allowed_properties: HashSet<&'static str>,
    allowed_rules: HashSet<&'static str>,
    allowed_values: HashMap<&'static str, HashSet<&'static str>>,
    allow_important: bool,
    allow_url: bool,
    allow_var: bool,
}

impl StrictPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allow_properties(mut self, properties: &[&'static str]) -> Self {
        self.allowed_properties.extend(properties.iter().copied());
        self
    }

    pub fn allow_rules(mut self, rules: &[&'static str]) -> Self {
        self.allowed_rules.extend(rules.iter().copied());
        self
    }

    pub fn allow_important(mut self) -> Self {
        self.allow_important = true;
        self
    }

    pub fn allow_url(mut self) -> Self {
        self.allow_url = true;
        self
    }

    pub fn allow_var(mut self) -> Self {
        self.allow_var = true;
        self
    }

    pub fn allow_values(mut self, property: &'static str, values: &[&'static str]) -> Self {
        let entry = self.allowed_values.entry(property).or_default();
        entry.extend(values.iter().copied());
        self
    }

    fn matches_allowed_value(property: &Property<'_>, allowed_value: &'static str) -> bool {
        Property::parse_string(
            property.property_id(),
            allowed_value,
            ParserOptions::default(),
        )
        .map(|allowed_property| allowed_property == *property)
        .unwrap_or(false)
    }

    fn rule_name(rule: &CssRule<'_>) -> &'static str {
        match rule {
            CssRule::Media(_) => "media",
            CssRule::Import(_) => "import",
            CssRule::Style(_) => "style",
            CssRule::Keyframes(_) => "keyframes",
            CssRule::FontFace(_) => "font-face",
            CssRule::FontPaletteValues(_) => "font-palette-values",
            CssRule::FontFeatureValues(_) => "font-feature-values",
            CssRule::Page(_) => "page",
            CssRule::Supports(_) => "supports",
            CssRule::CounterStyle(_) => "counter-style",
            CssRule::Namespace(_) => "namespace",
            CssRule::MozDocument(_) => "moz-document",
            CssRule::Nesting(_) => "nesting",
            CssRule::NestedDeclarations(_) => "nested-declarations",
            CssRule::Viewport(_) => "viewport",
            CssRule::CustomMedia(_) => "custom-media",
            CssRule::LayerStatement(_) => "layer-statement",
            CssRule::LayerBlock(_) => "layer-block",
            CssRule::Property(_) => "property",
            CssRule::Container(_) => "container",
            CssRule::Scope(_) => "scope",
            CssRule::StartingStyle(_) => "starting-style",
            CssRule::ViewTransition(_) => "view-transition",
            CssRule::Ignored => "ignored",
            CssRule::Unknown(_) => "unknown",
            CssRule::Custom(_) => "custom",
        }
    }

    fn rule_allowed(&self, rule: &CssRule<'_>) -> bool {
        matches!(
            rule,
            CssRule::Style(_) | CssRule::Nesting(_) | CssRule::NestedDeclarations(_)
        ) || self.allowed_rules.contains(Self::rule_name(rule))
    }
}

impl CssSanitizationPolicy for StrictPolicy {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        if matches!(
            rule,
            CssRule::Import(_) | CssRule::Namespace(_) | CssRule::MozDocument(_)
        ) && !self.allow_url
        {
            return NodeAction::Drop;
        }

        if self.rule_allowed(rule) {
            NodeAction::Continue
        } else {
            NodeAction::Drop
        }
    }

    fn visit_property(&self, property: &mut Property<'_>, ctx: PropertyContext) -> NodeAction {
        if ctx.important && !self.allow_important {
            return NodeAction::Drop;
        }

        let property_id = property.property_id();
        let name = property_id.name();
        if !self.allowed_properties.contains(name) {
            return NodeAction::Drop;
        }

        let security_scan = PropertySecurityScan::inspect(property);
        if security_scan.has_expression {
            return NodeAction::Drop;
        }
        if security_scan.has_url && !self.allow_url {
            return NodeAction::Drop;
        }
        if security_scan.has_var && !self.allow_var {
            return NodeAction::Drop;
        }

        if let Some(allowed_values) = self.allowed_values.get(name) {
            if !allowed_values
                .iter()
                .copied()
                .any(|allowed_value| Self::matches_allowed_value(property, allowed_value))
            {
                return NodeAction::Drop;
            }
        }

        NodeAction::Continue
    }

    fn visit_font_face_property(
        &self,
        property: &mut FontFaceProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        if !self.allow_url && matches!(property, FontFaceProperty::Source(_)) {
            NodeAction::Drop
        } else {
            NodeAction::Continue
        }
    }
}

#[derive(Default)]
pub struct FunctionSecurityPolicy {
    allowed_properties: HashSet<&'static str>,
    allow_url: bool,
    allow_var: bool,
    allow_env: bool,
}

impl FunctionSecurityPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allow_properties(mut self, properties: &[&'static str]) -> Self {
        self.allowed_properties.extend(properties.iter().copied());
        self
    }

    pub fn allow_url(mut self) -> Self {
        self.allow_url = true;
        self
    }

    pub fn allow_var(mut self) -> Self {
        self.allow_var = true;
        self
    }

    pub fn allow_env(mut self) -> Self {
        self.allow_env = true;
        self
    }
}

impl CssSanitizationPolicy for FunctionSecurityPolicy {
    fn visit_property(&self, property: &mut Property<'_>, _ctx: PropertyContext) -> NodeAction {
        let property_id = property.property_id();
        let name = property_id.name();
        if !self.allowed_properties.contains(name) {
            return NodeAction::Drop;
        }

        let security_scan = PropertySecurityScan::inspect(property);
        if security_scan.has_expression {
            return NodeAction::Drop;
        }
        if security_scan.has_url && !self.allow_url {
            return NodeAction::Drop;
        }
        if security_scan.has_var && !self.allow_var {
            return NodeAction::Drop;
        }
        if security_scan.has_env && !self.allow_env {
            return NodeAction::Drop;
        }

        NodeAction::Continue
    }
}

pub struct NoGlobalSelectors;

impl CssSanitizationPolicy for NoGlobalSelectors {
    fn visit_selector_list(
        &self,
        selectors: &mut SelectorList<'_>,
        _ctx: SelectorContext,
    ) -> NodeAction {
        let has_global_html_selector = selectors.0.iter().any(|selector| {
            selector.iter_raw_match_order().any(|component| {
                matches!(
                    component,
                    Component::LocalName(name) if name.lower_name.0 == "html"
                )
            })
        });

        if has_global_html_selector {
            NodeAction::Drop
        } else {
            NodeAction::Continue
        }
    }
}

pub struct DropImports;

impl CssSanitizationPolicy for DropImports {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        if matches!(rule, CssRule::Import(_)) {
            NodeAction::Drop
        } else {
            NodeAction::Continue
        }
    }
}

pub struct DropImportant;

impl CssSanitizationPolicy for DropImportant {
    fn visit_property(&self, _property: &mut Property<'_>, ctx: PropertyContext) -> NodeAction {
        if ctx.important {
            NodeAction::Drop
        } else {
            NodeAction::Continue
        }
    }
}
