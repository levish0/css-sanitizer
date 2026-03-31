use lightningcss::properties::Property;
use lightningcss::rules::CssRule;
use lightningcss::rules::counter_style::CounterStyleRule;
use lightningcss::rules::font_face::FontFaceProperty;
use lightningcss::rules::font_feature_values::{FontFeatureSubrule, FontFeatureValuesRule};
use lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use lightningcss::rules::page::{PageMarginRule, PageRule};
use lightningcss::rules::view_transition::ViewTransitionProperty;
use lightningcss::rules::viewport::ViewportRule;
use lightningcss::selector::SelectorList;

/// Controls how the sanitizer should handle the current node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeAction {
    /// Continue with the sanitizer's normal traversal for this node.
    Continue,
    /// Keep this node, but skip any deeper sanitization for its children.
    Skip,
    /// Drop this node entirely.
    Drop,
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

/// Advanced policy hook interface for sanitizing lightningcss AST nodes.
///
/// The default implementation keeps everything and only recurses normally.
/// Implementors can inspect or mutate AST nodes directly and return
/// [`NodeAction::Drop`] to remove them or [`NodeAction::Skip`] to keep the node
/// while bypassing deeper sanitization for its children.
pub trait CssSanitizationPolicy {
    /// Called for every [`CssRule`](lightningcss::rules::CssRule).
    fn visit_rule(&self, _rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        NodeAction::Continue
    }

    /// Called for all selector lists unless a more specific selector hook is
    /// overridden.
    fn visit_selector_list(
        &self,
        _selectors: &mut SelectorList<'_>,
        _ctx: SelectorContext,
    ) -> NodeAction {
        NodeAction::Continue
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

    /// Called for all normal CSS declarations represented as
    /// [`Property`](lightningcss::properties::Property) unless a more specific
    /// property hook is overridden.
    fn visit_property(&self, _property: &mut Property<'_>, _ctx: PropertyContext) -> NodeAction {
        NodeAction::Continue
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

    /// Called for `@font-face` descriptors.
    fn visit_font_face_property(
        &self,
        _property: &mut FontFaceProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    /// Called for `@font-palette-values` descriptors.
    fn visit_font_palette_values_property(
        &self,
        _property: &mut FontPaletteValuesProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    /// Called for `@view-transition` descriptors.
    fn visit_view_transition_property(
        &self,
        _property: &mut ViewTransitionProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Continue
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
}
