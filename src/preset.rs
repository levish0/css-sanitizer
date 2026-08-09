//! A ready-made strict allowlist policy.

use std::collections::{HashMap, HashSet};

use lightningcss::printer::PrinterOptions;
use lightningcss::properties::Property;
use lightningcss::properties::custom::{
    EnvironmentVariable, Function, Token, TokenOrValue, Variable,
};
use lightningcss::rules::CssRule;
use lightningcss::rules::font_face::FontFaceProperty;
use lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use lightningcss::rules::view_transition::ViewTransitionProperty;
use lightningcss::selector::SelectorList;
use lightningcss::stylesheet::ParserOptions;

use crate::policy::{
    CssSanitizationPolicy, DescriptorContext, NodeAction, PropertyContext, ResourceRef,
    SelectorContext, ValueAction, ValueContext,
};

/// A strict, allowlist-based [`CssSanitizationPolicy`].
///
/// Everything is removed unless explicitly allowed. Normal style rules, nesting
/// rules, and nested declaration blocks are always structurally permitted (their
/// declarations are still filtered); all other at-rules must be allowed by name.
/// Properties are allowed by name. Fetchable resources, `var()`, `env()`, and
/// generic functions in raw/unparsed values must be opted into separately.
/// `!important` declarations are dropped unless
/// [`allow_important`](Self::allow_important) is set.
///
/// ```
/// use css_sanitizer::{clean_stylesheet_with_policy, StrictPolicy};
///
/// let safe = clean_stylesheet_with_policy(
///     "@import url('evil.css'); .card { color: red; position: fixed }",
///     &StrictPolicy::new().allow_properties(&["color"]),
/// );
/// assert!(safe.contains("color"));
/// assert!(!safe.contains("@import"));
/// assert!(!safe.contains("position"));
/// ```
#[derive(Debug, Default, Clone)]
pub struct StrictPolicy {
    allowed_properties: HashSet<String>,
    allowed_rules: HashSet<String>,
    allowed_values: HashMap<String, HashSet<String>>,
    allowed_functions: HashSet<String>,
    allow_important: bool,
    allow_url: bool,
    allow_var: bool,
    allow_env: bool,
}

impl StrictPolicy {
    /// Creates a policy that allows nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allows the named properties (e.g. `"color"`).
    pub fn allow_properties(mut self, properties: &[&str]) -> Self {
        self.allowed_properties
            .extend(properties.iter().map(|p| p.to_string()));
        self
    }

    /// Allows the named at-rules (e.g. `"media"`, `"supports"`).
    pub fn allow_rules(mut self, rules: &[&str]) -> Self {
        self.allowed_rules
            .extend(rules.iter().map(|r| r.to_string()));
        self
    }

    /// Allows `!important` declarations.
    pub fn allow_important(mut self) -> Self {
        self.allow_important = true;
        self
    }

    /// Allows all recognized fetchable resources: typed/raw `url()`, `src()`,
    /// string-based `image()`/`image-set()`, and URLs in allowed `@import`
    /// rules. Use a custom policy's `check_resource` hook when origin- or
    /// scheme-specific decisions are required.
    pub fn allow_url(mut self) -> Self {
        self.allow_url = true;
        self
    }

    /// Allows `var()` references. Outside a recognized resource wrapper, this
    /// trusts the computed value supplied by the external cascade; the
    /// sanitizer does not resolve custom properties across stylesheet/DOM
    /// boundaries.
    pub fn allow_var(mut self) -> Self {
        self.allow_var = true;
        self
    }

    /// Allows `env()` references. Outside a recognized resource wrapper, this
    /// trusts the computed value supplied by the user-agent/environment.
    pub fn allow_env(mut self) -> Self {
        self.allow_env = true;
        self
    }

    /// Restricts an allowed property to a fixed set of serialized values.
    pub fn allow_values(mut self, property: &str, values: &[&str]) -> Self {
        let entry = self.allowed_values.entry(property.to_string()).or_default();
        entry.extend(values.iter().map(|v| v.to_string()));
        self
    }

    /// Allows generic functions found in raw/unparsed values. Known resource
    /// functions such as `src()` and `image()` are controlled by
    /// [`allow_url`](Self::allow_url) instead. Case/escape variants of `var()`
    /// and `env()` remain controlled by [`allow_var`](Self::allow_var) and
    /// [`allow_env`](Self::allow_env). Dashed custom function names are
    /// case-sensitive; other CSS function names are ASCII case-insensitive.
    ///
    /// Allowing an arbitrary-substitution function (for example a dashed custom
    /// function or `if()`) trusts its computed result when the function appears
    /// outside a recognized resource wrapper. An external `@function`
    /// definition cannot be resolved statically by this sanitizer.
    pub fn allow_functions(mut self, functions: &[&str]) -> Self {
        self.allowed_functions
            .extend(functions.iter().map(|function| {
                if function.starts_with("--") {
                    (*function).to_string()
                } else {
                    function.to_ascii_lowercase()
                }
            }));
        self
    }

    fn matches_allowed_value(property: &Property<'_>, allowed_value: &str) -> bool {
        // Compare normalized serializations so the parsed allowlist value (which
        // borrows from `allowed_value`) need not share the AST's lifetime.
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
            CssRule::PositionTry(_) => "position-try",
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
        if matches!(
            rule,
            CssRule::Ignored | CssRule::Unknown(_) | CssRule::Custom(_)
        ) {
            return false;
        }

        matches!(
            rule,
            CssRule::Style(_) | CssRule::Nesting(_) | CssRule::NestedDeclarations(_)
        ) || self.allowed_rules.contains(Self::rule_name(rule))
    }
}

impl CssSanitizationPolicy for StrictPolicy {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: crate::policy::RuleContext) -> NodeAction {
        if self.rule_allowed(rule) {
            NodeAction::Continue
        } else {
            NodeAction::Drop
        }
    }

    fn visit_selector_list(
        &self,
        _selectors: &mut SelectorList<'_>,
        _ctx: SelectorContext,
    ) -> NodeAction {
        // This preset filters by rule/property/value, not by selector.
        NodeAction::Continue
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

        if let Some(allowed_values) = self.allowed_values.get(name)
            && !allowed_values
                .iter()
                .any(|allowed_value| Self::matches_allowed_value(property, allowed_value))
        {
            return NodeAction::Drop;
        }

        NodeAction::Continue
    }

    fn visit_font_face_property(
        &self,
        _property: &mut FontFaceProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        // The value guard enforces url permission on `src`.
        NodeAction::Continue
    }

    fn visit_font_palette_values_property(
        &self,
        _property: &mut FontPaletteValuesProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    fn visit_view_transition_property(
        &self,
        _property: &mut ViewTransitionProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    fn check_resource(&self, _resource: ResourceRef<'_>, _ctx: ValueContext) -> ValueAction {
        if self.allow_url {
            ValueAction::Allow
        } else {
            ValueAction::Deny
        }
    }

    fn check_function(&self, function: &Function<'_>, _ctx: ValueContext) -> ValueAction {
        if function.name.0.eq_ignore_ascii_case("expression") {
            return ValueAction::Deny;
        }

        let name = function.name.0.as_ref();
        let allowed = if name.starts_with("--") {
            self.allowed_functions.contains(name)
        } else {
            self.allowed_functions.contains(&name.to_ascii_lowercase())
        };

        if allowed {
            ValueAction::Allow
        } else {
            ValueAction::Deny
        }
    }

    fn check_variable(&self, _variable: &Variable<'_>, _ctx: ValueContext) -> ValueAction {
        if self.allow_var {
            ValueAction::Allow
        } else {
            ValueAction::Deny
        }
    }

    fn check_environment_variable(
        &self,
        _env: &EnvironmentVariable<'_>,
        _ctx: ValueContext,
    ) -> ValueAction {
        if self.allow_env {
            ValueAction::Allow
        } else {
            ValueAction::Deny
        }
    }

    fn check_unparsed_variable(&self, _function: &Function<'_>, _ctx: ValueContext) -> ValueAction {
        if self.allow_var {
            ValueAction::Allow
        } else {
            ValueAction::Deny
        }
    }

    fn check_unparsed_environment_variable(
        &self,
        _function: &Function<'_>,
        _ctx: ValueContext,
    ) -> ValueAction {
        if self.allow_env {
            ValueAction::Allow
        } else {
            ValueAction::Deny
        }
    }

    fn check_token(&self, token: &TokenOrValue<'_>, _ctx: ValueContext) -> ValueAction {
        match token {
            TokenOrValue::Token(Token::BadUrl(_) | Token::Function(_)) => ValueAction::Deny,
            // Legacy IE `expression()` is never legitimate; always reject.
            TokenOrValue::Function(f) if f.name.0.eq_ignore_ascii_case("expression") => {
                ValueAction::Deny
            }
            // Other tokens carry no fetch capability on their own; nested values
            // (urls, vars, env) are still checked as the guard recurses.
            _ => ValueAction::Allow,
        }
    }
}
