use lightningcss::properties::Property;
use lightningcss::properties::custom::{EnvironmentVariable, Function, TokenOrValue, Variable};
use lightningcss::rules::CssRule;
use lightningcss::rules::counter_style::CounterStyleRule;
use lightningcss::rules::font_face::FontFaceProperty;
use lightningcss::rules::font_feature_values::{FontFeatureSubrule, FontFeatureValuesRule};
use lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use lightningcss::rules::page::{PageMarginRule, PageRule};
use lightningcss::rules::view_transition::ViewTransitionProperty;
use lightningcss::rules::viewport::ViewportRule;
use lightningcss::selector::SelectorList;
use lightningcss::values::url::Url;

/// Controls how the sanitizer should handle the current node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NodeAction {
    /// Continue with the sanitizer's normal traversal for this node.
    Continue,
    /// Keep this node. Retained for API compatibility; the engine still runs
    /// its normal traversal and any enabled value/resource guards so this
    /// cannot itself be used to bypass sanitization.
    Skip,
    /// Drop this node entirely.
    Drop,
}

/// Identifies the CSS syntax that produced a potentially fetchable resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResourceKind {
    /// A typed or raw `url()` value. This also includes image-set options that
    /// lightningcss normalizes into typed URL images.
    Url,
    /// A CSS Values `src()` function.
    Src,
    /// A string or dynamic value inside `image()`.
    Image,
    /// A string or dynamic value inside a generic/unparsed `image-set()`.
    ImageSet,
    /// The URL in an `@import` rule.
    Import,
}

/// A potentially fetchable resource found by the engine.
///
/// `value` is `None` when the resource is dynamic (for example
/// `src(var(--font-url))`) or otherwise cannot be recovered statically.
///
/// A literal `value` is parser-decoded but otherwise unresolved: it may be
/// relative and is not guaranteed to be an absolute or normalized URL. A
/// policy enforcing origins should resolve it with a standards-compatible URL
/// parser against a trusted base URL owned by that policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResourceRef<'a> {
    /// The CSS construct that produced the resource reference.
    pub kind: ResourceKind,
    /// The statically known resource string, or `None` when it is dynamic or
    /// otherwise not statically recoverable.
    pub value: Option<&'a str>,
}

impl<'a> ResourceRef<'a> {
    /// Creates a resource reference with a statically known value.
    pub fn literal(kind: ResourceKind, value: &'a str) -> Self {
        Self {
            kind,
            value: Some(value),
        }
    }

    /// Creates a resource reference whose final URL is dynamic or otherwise not
    /// statically recoverable, including runtime substitution such as `var()`.
    pub fn dynamic(kind: ResourceKind) -> Self {
        Self { kind, value: None }
    }
}

/// Controls how the engine-enforced value guard should handle a single value
/// (a URL, variable reference, environment variable, or raw token) found inside
/// a declaration or descriptor that the structural policy decided to keep.
///
/// Unlike [`NodeAction`], these checks default to [`ValueAction::Deny`]: a policy
/// that does not explicitly allow a value kind causes the entire containing
/// declaration/descriptor (or containing resource-bearing rule) to be dropped.
/// This is what makes value-level
/// exfiltration vectors (e.g. `url()` smuggled into a `var()` fallback or an
/// `@font-face` `src`) impossible to leak by forgetting a hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValueAction {
    /// Allow this value (and continue scanning its children).
    Allow,
    /// Reject this value, dropping its declaration, descriptor, or
    /// resource-bearing rule.
    Deny,
}

/// Identifies where a value being checked by the guard lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValueLocation {
    /// A standalone declaration list, e.g. an inline `style=""` attribute.
    DeclarationList,
    /// A declaration inside a normal style rule.
    StyleRule,
    /// A declaration inside a nested declarations rule.
    NestedDeclarations,
    /// A declaration inside `@keyframes`.
    Keyframe,
    /// A declaration inside `@page`.
    Page,
    /// A declaration inside a page margin rule.
    PageMargin,
    /// A declaration inside `@counter-style`.
    CounterStyle,
    /// A declaration inside `@viewport`.
    Viewport,
    /// A declaration inside `@position-try`.
    PositionTry,
    /// An `@font-face` descriptor.
    FontFaceDescriptor,
    /// An `@font-palette-values` descriptor.
    FontPaletteValuesDescriptor,
    /// An `@view-transition` descriptor.
    ViewTransitionDescriptor,
    /// A declaration inside an `@container` style query condition.
    ContainerCondition,
    /// The `initial-value` of an `@property` rule.
    PropertyInitialValue,
    /// The URL in an `@import` rule.
    ImportRule,
}

/// Context for a value inspected by the engine-enforced value guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueContext {
    pub depth: usize,
    pub important: bool,
    pub location: ValueLocation,
}

/// Context for a CSS rule node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleContext {
    pub depth: usize,
}

/// Context for a selector list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectorContext {
    pub depth: usize,
}

/// Context for a normal CSS property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyContext {
    pub depth: usize,
    pub important: bool,
}

/// Context for descriptor-style properties that are not represented as
/// [`Property`](lightningcss::properties::Property).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DescriptorContext {
    pub depth: usize,
}

/// Policy hook interface for sanitizing lightningcss AST nodes.
///
/// This trait is **deny-by-default**: every hook that admits content
/// (rules, selectors, declarations, descriptors) returns [`NodeAction::Drop`]
/// by default, and every value-guard hook returns [`ValueAction::Deny`] by
/// default. An empty policy therefore removes everything; a useful policy
/// explicitly allows the rules, selectors, properties, descriptors, and value
/// kinds it wants to keep. Forgetting a hook fails safe (over-removes) rather
/// than leaking content.
///
/// Hooks that merely control whether to *descend* into an already-allowed rule
/// (e.g. [`visit_page_rule`](Self::visit_page_rule)) default to
/// [`NodeAction::Continue`], because the content they expose is itself
/// deny-gated downstream. They are only reached after [`visit_rule`](Self::visit_rule)
/// has already allowed the enclosing rule.
///
/// Implementors can inspect or mutate AST nodes directly. [`NodeAction::Skip`]
/// is retained for compatibility but does not bypass engine traversal or the
/// enabled value/resource guards.
///
/// See [`StrictPolicy`](crate::StrictPolicy) for a ready-made allowlist policy.
pub trait CssSanitizationPolicy {
    /// Called for every [`CssRule`](lightningcss::rules::CssRule). Defaults to
    /// [`NodeAction::Drop`].
    fn visit_rule(&self, _rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        NodeAction::Drop
    }

    /// Called for all selector lists unless a more specific selector hook is
    /// overridden. Defaults to [`NodeAction::Drop`].
    fn visit_selector_list(
        &self,
        _selectors: &mut SelectorList<'_>,
        _ctx: SelectorContext,
    ) -> NodeAction {
        NodeAction::Drop
    }

    /// Called for selectors on normal style rules.
    fn visit_style_rule_selectors(
        &self,
        selectors: &mut SelectorList<'_>,
        ctx: SelectorContext,
    ) -> NodeAction {
        self.visit_selector_list(selectors, ctx)
    }

    /// Called for selectors on nesting rules.
    fn visit_nesting_selectors(
        &self,
        selectors: &mut SelectorList<'_>,
        ctx: SelectorContext,
    ) -> NodeAction {
        self.visit_selector_list(selectors, ctx)
    }

    /// Called for the `<scope-start>` and `<scope-end>` selector lists in the
    /// prelude of an `@scope` rule.
    fn visit_scope_selectors(
        &self,
        selectors: &mut SelectorList<'_>,
        ctx: SelectorContext,
    ) -> NodeAction {
        self.visit_selector_list(selectors, ctx)
    }

    /// Called for all normal CSS declarations represented as
    /// [`Property`](lightningcss::properties::Property) unless a more specific
    /// property hook is overridden. Defaults to [`NodeAction::Drop`].
    fn visit_property(&self, _property: &mut Property<'_>, _ctx: PropertyContext) -> NodeAction {
        NodeAction::Drop
    }

    /// Called for parsed declaration lists such as `style=""` content.
    fn visit_declaration_list_property(
        &self,
        property: &mut Property<'_>,
        ctx: PropertyContext,
    ) -> NodeAction {
        self.visit_property(property, ctx)
    }

    /// Called for declarations inside normal style rules.
    fn visit_style_property(
        &self,
        property: &mut Property<'_>,
        ctx: PropertyContext,
    ) -> NodeAction {
        self.visit_property(property, ctx)
    }

    /// Called for declarations inside nested declarations rules.
    fn visit_nested_declarations_property(
        &self,
        property: &mut Property<'_>,
        ctx: PropertyContext,
    ) -> NodeAction {
        self.visit_property(property, ctx)
    }

    /// Called for declarations inside keyframes.
    fn visit_keyframe_property(
        &self,
        property: &mut Property<'_>,
        ctx: PropertyContext,
    ) -> NodeAction {
        self.visit_property(property, ctx)
    }

    /// Called for declarations inside `@page`.
    fn visit_page_property(&self, property: &mut Property<'_>, ctx: PropertyContext) -> NodeAction {
        self.visit_property(property, ctx)
    }

    /// Called for declarations inside page margin rules.
    fn visit_page_margin_property(
        &self,
        property: &mut Property<'_>,
        ctx: PropertyContext,
    ) -> NodeAction {
        self.visit_property(property, ctx)
    }

    /// Called for declarations inside `@counter-style`.
    fn visit_counter_style_property(
        &self,
        property: &mut Property<'_>,
        ctx: PropertyContext,
    ) -> NodeAction {
        self.visit_property(property, ctx)
    }

    /// Called for declarations inside `@viewport`.
    fn visit_viewport_property(
        &self,
        property: &mut Property<'_>,
        ctx: PropertyContext,
    ) -> NodeAction {
        self.visit_property(property, ctx)
    }

    /// Called for declarations inside `@position-try`.
    fn visit_position_try_property(
        &self,
        property: &mut Property<'_>,
        ctx: PropertyContext,
    ) -> NodeAction {
        self.visit_property(property, ctx)
    }

    /// Called for `@font-face` descriptors. Defaults to [`NodeAction::Drop`].
    fn visit_font_face_property(
        &self,
        _property: &mut FontFaceProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Drop
    }

    /// Called for `@font-palette-values` descriptors. Defaults to
    /// [`NodeAction::Drop`].
    fn visit_font_palette_values_property(
        &self,
        _property: &mut FontPaletteValuesProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Drop
    }

    /// Called for `@view-transition` descriptors. Defaults to
    /// [`NodeAction::Drop`].
    fn visit_view_transition_property(
        &self,
        _property: &mut ViewTransitionProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Drop
    }

    /// Called for `@page` rules before declaration filtering.
    fn visit_page_rule(&self, _rule: &mut PageRule<'_>, _ctx: RuleContext) -> NodeAction {
        NodeAction::Continue
    }

    /// Called for page margin rules nested inside `@page`.
    fn visit_page_margin_rule(
        &self,
        _rule: &mut PageMarginRule<'_>,
        _ctx: RuleContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    /// Called for `@counter-style` rules before declaration filtering.
    fn visit_counter_style_rule(
        &self,
        _rule: &mut CounterStyleRule<'_>,
        _ctx: RuleContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    /// Called for `@viewport` rules before declaration filtering.
    fn visit_viewport_rule(&self, _rule: &mut ViewportRule<'_>, _ctx: RuleContext) -> NodeAction {
        NodeAction::Continue
    }

    /// Called for `@font-feature-values` rules.
    fn visit_font_feature_values_rule(
        &self,
        _rule: &mut FontFeatureValuesRule<'_>,
        _ctx: RuleContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    /// Called for sub-rules nested inside `@font-feature-values`.
    fn visit_font_feature_values_subrule(
        &self,
        _subrule: &mut FontFeatureSubrule<'_>,
        _ctx: RuleContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    /// Central engine-enforced guard for potentially fetchable resources,
    /// including raw URL forms, `src()`, string-based image functions, and
    /// `@import`. The default [`check_url`](Self::check_url) implementation also
    /// routes typed URLs here. Dynamic references have [`ResourceRef::value`]
    /// set to `None`. Defaults to [`ValueAction::Deny`].
    fn check_resource(&self, _resource: ResourceRef<'_>, _ctx: ValueContext) -> ValueAction {
        ValueAction::Deny
    }

    /// Compatibility hook for typed `url()` values. The default implementation
    /// routes through [`check_resource`](Self::check_resource). An existing
    /// policy that overrides `check_url` keeps that behavior; delegate to or
    /// remove the override to centralize typed URLs in `check_resource`.
    fn check_url(&self, url: &Url<'_>, ctx: ValueContext) -> ValueAction {
        self.check_resource(
            ResourceRef::literal(ResourceKind::Url, url.url.as_ref()),
            ctx,
        )
    }

    /// Engine-enforced guard: called for every `var()` reference found inside a
    /// kept declaration or descriptor. Defaults to [`ValueAction::Deny`].
    fn check_variable(&self, _variable: &Variable<'_>, _ctx: ValueContext) -> ValueAction {
        ValueAction::Deny
    }

    /// Engine-enforced guard: called for every `env()` reference found inside a
    /// kept declaration or descriptor. Defaults to [`ValueAction::Deny`].
    fn check_environment_variable(
        &self,
        _env: &EnvironmentVariable<'_>,
        _ctx: ValueContext,
    ) -> ValueAction {
        ValueAction::Deny
    }

    /// Engine-enforced guard for a case/escape variant of `var()` that
    /// lightningcss left as a generic function rather than a typed
    /// [`Variable`]. Defaults to deny. Its arguments are still scanned after
    /// this hook allows it.
    fn check_unparsed_variable(&self, _function: &Function<'_>, _ctx: ValueContext) -> ValueAction {
        ValueAction::Deny
    }

    /// Engine-enforced guard for a case/escape variant of `env()` that
    /// lightningcss left as a generic function rather than a typed
    /// [`EnvironmentVariable`]. Defaults to deny. Its arguments are still
    /// scanned after this hook allows it.
    fn check_unparsed_environment_variable(
        &self,
        _function: &Function<'_>,
        _ctx: ValueContext,
    ) -> ValueAction {
        ValueAction::Deny
    }

    /// Engine-enforced guard for generic functions in raw/unparsed values.
    /// Functions with resource or dynamic-substitution semantics are routed
    /// through their dedicated hooks instead. Defaults to deny so future CSS
    /// functions cannot become a fail-open path.
    fn check_function(&self, _function: &Function<'_>, _ctx: ValueContext) -> ValueAction {
        ValueAction::Deny
    }

    /// Engine-enforced guard for raw tokens left after more-specific resource,
    /// function, variable, and environment-variable routing. This hook is the
    /// final backstop against value smuggling through `error_recovery`.
    /// Defaults to [`ValueAction::Deny`].
    fn check_token(&self, _token: &TokenOrValue<'_>, _ctx: ValueContext) -> ValueAction {
        ValueAction::Deny
    }
}
