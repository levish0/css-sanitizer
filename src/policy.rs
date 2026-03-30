use lightningcss::properties::Property;
use lightningcss::rules::counter_style::CounterStyleRule;
use lightningcss::rules::font_face::FontFaceProperty;
use lightningcss::rules::font_feature_values::FontFeatureValuesRule;
use lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use lightningcss::rules::page::{PageMarginRule, PageRule};
use lightningcss::rules::view_transition::ViewTransitionProperty;
use lightningcss::rules::viewport::ViewportRule;
use lightningcss::rules::CssRule;
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

/// High-level kinds of CSS rules exposed by lightningcss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleKind {
    Stylesheet,
    Media,
    Import,
    Style,
    Keyframes,
    FontFace,
    FontPaletteValues,
    FontFeatureValues,
    Page,
    PageMargin,
    Supports,
    CounterStyle,
    Namespace,
    MozDocument,
    Nesting,
    NestedDeclarations,
    Viewport,
    CustomMedia,
    LayerStatement,
    LayerBlock,
    Property,
    Container,
    Scope,
    StartingStyle,
    ViewTransition,
    Ignored,
    Unknown,
    Custom,
}

/// Where a normal CSS declaration/property is being sanitized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationOwner {
    DeclarationList,
    StyleRule,
    NestedDeclarations,
    Keyframe,
    Page,
    PageMargin,
    CounterStyle,
    Viewport,
}

/// Where a descriptor-style property is being sanitized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorOwner {
    FontFace,
    FontPaletteValues,
    ViewTransition,
}

/// Context for a CSS rule node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleContext {
    pub kind: RuleKind,
    pub parent: Option<RuleKind>,
    pub depth: usize,
}

/// Context for a selector list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectorContext {
    pub parent_rule: RuleKind,
    pub depth: usize,
}

/// Context for a normal CSS property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyContext {
    pub owner: DeclarationOwner,
    pub parent_rule: Option<RuleKind>,
    pub depth: usize,
    pub important: bool,
}

/// Context for descriptor-style properties that are not represented as
/// [`Property`](lightningcss::properties::Property).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DescriptorContext {
    pub owner: DescriptorOwner,
    pub parent_rule: Option<RuleKind>,
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

    /// Called for selector lists on style-like rules.
    fn visit_selector_list(
        &self,
        _selectors: &mut SelectorList<'_>,
        _ctx: SelectorContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    /// Called for normal CSS declarations represented as
    /// [`Property`](lightningcss::properties::Property).
    fn visit_property(&self, _property: &mut Property<'_>, _ctx: PropertyContext) -> NodeAction {
        NodeAction::Continue
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
    ///
    /// These rules have their own internal structure in lightningcss, so the
    /// advanced hook gets access to the full AST node directly.
    fn visit_font_feature_values_rule(
        &self,
        _rule: &mut FontFeatureValuesRule<'_>,
        _ctx: RuleContext,
    ) -> NodeAction {
        NodeAction::Continue
    }
}
