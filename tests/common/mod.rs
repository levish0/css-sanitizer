#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use css_sanitizer::lightningcss::printer::PrinterOptions;
use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::rules::font_face::FontFaceProperty;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::{
    CssSanitizationPolicy, DescriptorContext, NodeAction, PropertyContext, RuleContext,
    SelectorContext,
};

#[derive(Default)]
pub struct StrictPolicy {
    allowed_properties: HashSet<&'static str>,
    allowed_rules: HashSet<&'static str>,
    allowed_values: HashMap<&'static str, HashSet<String>>,
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
        entry.extend(values.iter().map(|value| value.to_ascii_lowercase()));
        self
    }

    fn property_css(property: &Property<'_>, important: bool) -> String {
        property
            .to_css_string(important, PrinterOptions::default())
            .unwrap_or_default()
            .to_ascii_lowercase()
    }

    fn property_value(property: &Property<'_>, important: bool) -> Option<String> {
        let css = Self::property_css(property, important);
        let (_, value) = css.split_once(':')?;
        let value = value.trim().trim_end_matches(';').trim();
        let value = value
            .strip_suffix("!important")
            .map(str::trim)
            .unwrap_or(value);
        Some(value.to_string())
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
        matches!(rule, CssRule::Style(_) | CssRule::Nesting(_) | CssRule::NestedDeclarations(_))
            || self.allowed_rules.contains(Self::rule_name(rule))
    }
}

impl CssSanitizationPolicy for StrictPolicy {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
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

        let css = Self::property_css(property, ctx.important);
        if css.contains("expression(") {
            return NodeAction::Drop;
        }
        if !self.allow_url && css.contains("url(") {
            return NodeAction::Drop;
        }
        if !self.allow_var && css.contains("var(") {
            return NodeAction::Drop;
        }

        if let Some(allowed_values) = self.allowed_values.get(name) {
            let Some(value) = Self::property_value(property, ctx.important) else {
                return NodeAction::Drop;
            };
            if !allowed_values.contains(value.as_str()) {
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

    fn property_css(property: &Property<'_>, important: bool) -> String {
        property
            .to_css_string(important, PrinterOptions::default())
            .unwrap_or_default()
            .to_ascii_lowercase()
    }
}

impl CssSanitizationPolicy for FunctionSecurityPolicy {
    fn visit_property(&self, property: &mut Property<'_>, ctx: PropertyContext) -> NodeAction {
        let property_id = property.property_id();
        let name = property_id.name();
        if !self.allowed_properties.contains(name) {
            return NodeAction::Drop;
        }

        let css = Self::property_css(property, ctx.important);
        if css.contains("expression(") {
            return NodeAction::Drop;
        }
        if !self.allow_url && css.contains("url(") {
            return NodeAction::Drop;
        }
        if !self.allow_var && css.contains("var(") {
            return NodeAction::Drop;
        }
        if !self.allow_env && css.contains("env(") {
            return NodeAction::Drop;
        }

        NodeAction::Continue
    }
}

pub struct NoGlobalSelectors;

impl CssSanitizationPolicy for NoGlobalSelectors {
    fn visit_selector_list(
        &self,
        selectors: &mut css_sanitizer::lightningcss::selector::SelectorList<'_>,
        _ctx: SelectorContext,
    ) -> NodeAction {
        if selectors.to_string().contains("html") {
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
