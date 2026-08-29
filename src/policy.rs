use lightningcss::properties::custom::{EnvironmentVariable, Function, TokenOrValue, Variable};
use lightningcss::properties::{Property, PropertyId};
use lightningcss::rules::CssRule;
use lightningcss::rules::font_face::FontFaceProperty;
use lightningcss::rules::font_feature_values::FontFeatureSubrule;
use lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use lightningcss::rules::page::PageMarginRule;
use lightningcss::rules::view_transition::ViewTransitionProperty;
use lightningcss::selector::SelectorList;

/// Controls whether a structural CSS node is retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NodeDecision {
    Keep,
    Drop,
}

/// Controls whether a value-level construct is retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValueDecision {
    Allow,
    Deny,
}

/// Identifies a stylesheet rule without relying on internal string names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RuleKind {
    Media,
    Import,
    Style,
    Keyframes,
    FontFace,
    FontPaletteValues,
    FontFeatureValues,
    Page,
    Supports,
    CounterStyle,
    Namespace,
    MozDocument,
    Nesting,
    NestedDeclarations,
    Viewport,
    PositionTry,
    CustomMedia,
    LayerStatement,
    LayerBlock,
    PropertyRegistration,
    Container,
    Scope,
    StartingStyle,
    ViewTransition,
    Ignored,
    Unknown,
    Custom,
}

impl RuleKind {
    pub fn of(rule: &CssRule<'_>) -> Self {
        match rule {
            CssRule::Media(_) => Self::Media,
            CssRule::Import(_) => Self::Import,
            CssRule::Style(_) => Self::Style,
            CssRule::Keyframes(_) => Self::Keyframes,
            CssRule::FontFace(_) => Self::FontFace,
            CssRule::FontPaletteValues(_) => Self::FontPaletteValues,
            CssRule::FontFeatureValues(_) => Self::FontFeatureValues,
            CssRule::Page(_) => Self::Page,
            CssRule::Supports(_) => Self::Supports,
            CssRule::CounterStyle(_) => Self::CounterStyle,
            CssRule::Namespace(_) => Self::Namespace,
            CssRule::MozDocument(_) => Self::MozDocument,
            CssRule::Nesting(_) => Self::Nesting,
            CssRule::NestedDeclarations(_) => Self::NestedDeclarations,
            CssRule::Viewport(_) => Self::Viewport,
            CssRule::PositionTry(_) => Self::PositionTry,
            CssRule::CustomMedia(_) => Self::CustomMedia,
            CssRule::LayerStatement(_) => Self::LayerStatement,
            CssRule::LayerBlock(_) => Self::LayerBlock,
            CssRule::Property(_) => Self::PropertyRegistration,
            CssRule::Container(_) => Self::Container,
            CssRule::Scope(_) => Self::Scope,
            CssRule::StartingStyle(_) => Self::StartingStyle,
            CssRule::ViewTransition(_) => Self::ViewTransition,
            CssRule::Ignored => Self::Ignored,
            CssRule::Unknown(_) => Self::Unknown,
            CssRule::Custom(_) => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectorLocation {
    StyleRule,
    Nesting,
    ScopeStart,
    ScopeEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PropertyLocation {
    DeclarationList,
    StyleRule,
    NestedDeclarations,
    Keyframe,
    Page,
    PageMargin,
    CounterStyle,
    Viewport,
    PositionTry,
    ContainerCondition,
    PropertyInitialValue,
    OpaqueAtRule,
    FontFaceDescriptor,
    FontPaletteValuesDescriptor,
    ViewTransitionDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FontFaceDescriptorKind {
    Source,
    FontFamily,
    FontStyle,
    FontWeight,
    FontStretch,
    UnicodeRange,
    Custom,
}

impl FontFaceDescriptorKind {
    pub fn of(property: &FontFaceProperty<'_>) -> Self {
        match property {
            FontFaceProperty::Source(_) => Self::Source,
            FontFaceProperty::FontFamily(_) => Self::FontFamily,
            FontFaceProperty::FontStyle(_) => Self::FontStyle,
            FontFaceProperty::FontWeight(_) => Self::FontWeight,
            FontFaceProperty::FontStretch(_) => Self::FontStretch,
            FontFaceProperty::UnicodeRange(_) => Self::UnicodeRange,
            FontFaceProperty::Custom(_) => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FontPaletteValuesDescriptorKind {
    FontFamily,
    BasePalette,
    OverrideColors,
    Custom,
}

impl FontPaletteValuesDescriptorKind {
    pub fn of(property: &FontPaletteValuesProperty<'_>) -> Self {
        match property {
            FontPaletteValuesProperty::FontFamily(_) => Self::FontFamily,
            FontPaletteValuesProperty::BasePalette(_) => Self::BasePalette,
            FontPaletteValuesProperty::OverrideColors(_) => Self::OverrideColors,
            FontPaletteValuesProperty::Custom(_) => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ViewTransitionDescriptorKind {
    Navigation,
    Types,
    Custom,
}

impl ViewTransitionDescriptorKind {
    pub fn of(property: &ViewTransitionProperty<'_>) -> Self {
        match property {
            ViewTransitionProperty::Navigation(_) => Self::Navigation,
            ViewTransitionProperty::Types(_) => Self::Types,
            ViewTransitionProperty::Custom(_) => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DescriptorKind {
    FontFace(FontFaceDescriptorKind),
    FontPaletteValues(FontPaletteValuesDescriptorKind),
    ViewTransition(ViewTransitionDescriptorKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleContext {
    pub kind: RuleKind,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorContext {
    pub rule: RuleKind,
    pub location: SelectorLocation,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyContext<'i> {
    pub key: PropertyId<'i>,
    pub rule: Option<RuleKind>,
    pub location: PropertyLocation,
    pub depth: usize,
    pub important: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorContext {
    pub rule: RuleKind,
    pub kind: DescriptorKind,
    pub depth: usize,
}

/// Full typed context supplied to value and resource decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueContext<'i> {
    pub property: Option<PropertyId<'i>>,
    pub descriptor: Option<DescriptorKind>,
    pub rule: Option<RuleKind>,
    pub location: PropertyLocation,
    pub depth: usize,
    pub important: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResourceSyntax {
    Url,
    Src,
    Image,
    ImageSet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResourceUse {
    Image,
    FontSource,
    Cursor,
    ListStyleImage,
    Content,
    MaskImage,
    FilterReference,
    SvgPaintServer,
    Other,
}

impl ResourceUse {
    pub fn from_value_context(ctx: &ValueContext<'_>) -> Self {
        if matches!(
            ctx.descriptor,
            Some(DescriptorKind::FontFace(FontFaceDescriptorKind::Source))
        ) {
            return Self::FontSource;
        }

        let Some(property) = &ctx.property else {
            return Self::Other;
        };
        let name = property.name();
        if name.starts_with("--") {
            Self::Other
        } else if name.eq_ignore_ascii_case("cursor") {
            Self::Cursor
        } else if matches!(name, "list-style" | "list-style-image") {
            Self::ListStyleImage
        } else if name.eq_ignore_ascii_case("content") {
            Self::Content
        } else if name.contains("mask") {
            Self::MaskImage
        } else if name.ends_with("filter") || name == "clip-path" {
            Self::FilterReference
        } else if matches!(name, "fill" | "stroke") {
            Self::SvgPaintServer
        } else if name.contains("image")
            || name.starts_with("background")
            || name.starts_with("border-image")
        {
            Self::Image
        } else {
            Self::Other
        }
    }
}

/// A syntactically identifiable resource reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResourceRef<'a> {
    pub syntax: ResourceSyntax,
    pub use_kind: ResourceUse,
    pub value: Option<&'a str>,
}

impl<'a> ResourceRef<'a> {
    pub fn literal(syntax: ResourceSyntax, use_kind: ResourceUse, value: &'a str) -> Self {
        Self {
            syntax,
            use_kind,
            value: Some(value),
        }
    }

    pub fn dynamic(syntax: ResourceSyntax, use_kind: ResourceUse) -> Self {
        Self {
            syntax,
            use_kind,
            value: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DynamicValueKind {
    Variable,
    EnvironmentVariable,
    Function,
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum DynamicValueRef<'a, 'i> {
    Variable(&'a Variable<'i>),
    EnvironmentVariable(&'a EnvironmentVariable<'i>),
    UnparsedVariable(&'a Function<'i>),
    UnparsedEnvironmentVariable(&'a Function<'i>),
    Function(&'a Function<'i>),
}

impl DynamicValueRef<'_, '_> {
    pub fn kind(&self) -> DynamicValueKind {
        match self {
            Self::Variable(_) | Self::UnparsedVariable(_) => DynamicValueKind::Variable,
            Self::EnvironmentVariable(_) | Self::UnparsedEnvironmentVariable(_) => {
                DynamicValueKind::EnvironmentVariable
            }
            Self::Function(_) => DynamicValueKind::Function,
        }
    }

    pub fn function_name(&self) -> Option<&str> {
        match self {
            Self::UnparsedVariable(function)
            | Self::UnparsedEnvironmentVariable(function)
            | Self::Function(function) => Some(function.name.0.as_ref()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportContext<'a> {
    pub url: &'a str,
    pub depth: usize,
}

/// `AllowPassthrough` preserves the browser import unchanged. The imported
/// stylesheet is not fetched or sanitized by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImportDecision {
    Deny,
    AllowPassthrough,
}

/// Policy interface for typed, deny-by-default CSS sanitization.
///
/// Every method that can retain authored content defaults to denial. Policies
/// may inspect and mutate the upstream AST, but opaque token lists are still
/// passed through the engine value guard before they can be retained.
pub trait CssPolicy {
    fn rule(&self, _rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeDecision {
        NodeDecision::Drop
    }

    fn selector(&self, _selectors: &mut SelectorList<'_>, _ctx: SelectorContext) -> NodeDecision {
        NodeDecision::Drop
    }

    fn property(&self, _property: &mut Property<'_>, _ctx: PropertyContext<'_>) -> NodeDecision {
        NodeDecision::Drop
    }

    fn font_face_descriptor(
        &self,
        _property: &mut FontFaceProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeDecision {
        NodeDecision::Drop
    }

    fn font_palette_values_descriptor(
        &self,
        _property: &mut FontPaletteValuesProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeDecision {
        NodeDecision::Drop
    }

    fn view_transition_descriptor(
        &self,
        _property: &mut ViewTransitionProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeDecision {
        NodeDecision::Drop
    }

    fn page_margin_rule(&self, _rule: &mut PageMarginRule<'_>, _ctx: RuleContext) -> NodeDecision {
        NodeDecision::Drop
    }

    fn font_feature_values_subrule(
        &self,
        _rule: &mut FontFeatureSubrule<'_>,
        _ctx: RuleContext,
    ) -> NodeDecision {
        NodeDecision::Drop
    }

    fn import(&self, _ctx: ImportContext<'_>) -> ImportDecision {
        ImportDecision::Deny
    }

    fn resource(&self, _resource: ResourceRef<'_>, _ctx: &ValueContext<'_>) -> ValueDecision {
        ValueDecision::Deny
    }

    fn dynamic_value(
        &self,
        _value: DynamicValueRef<'_, '_>,
        _ctx: &ValueContext<'_>,
    ) -> ValueDecision {
        ValueDecision::Deny
    }

    fn token(&self, _token: &TokenOrValue<'_>, _ctx: &ValueContext<'_>) -> ValueDecision {
        ValueDecision::Deny
    }
}
