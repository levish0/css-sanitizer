mod common;

use std::cell::RefCell;

use common::{declaration, declaration_css, style_policy, stylesheet_css};
use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::properties::custom::TokenOrValue;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::rules::font_face::FontFaceProperty;
use css_sanitizer::{
    CssPolicy, DescriptorContext, DescriptorKind, DynamicValueRef, FontFaceDescriptorKind,
    ImportContext, ImportDecision, NodeDecision, PropertyContext, PropertyLocation, ResourceRef,
    ResourceSyntax, ResourceUse, RuleContext, RuleKind, StrictPolicy, ValueContext, ValueDecision,
};

#[test]
fn deny_by_default_policy_removes_everything() {
    struct EmptyPolicy;
    impl CssPolicy for EmptyPolicy {}

    assert!(declaration_css("color: red", &EmptyPolicy).is_empty());
    assert!(stylesheet_css(".card { color: red }", &EmptyPolicy).is_empty());
}

#[test]
fn image_set_and_spec_resource_functions_are_guarded() {
    for input in [
        r#"background-image: image-set(url("https://example.test/a.png") 1x)"#,
        r#"background-image: src("https://example.test/a.png")"#,
        r#"background-image: SRC("https://example.test/a.png")"#,
        r#"background-image: s\72 c("https://example.test/a.png")"#,
        r#"background-image: image("https://example.test/a.png")"#,
        r#"--asset: -webkit-image-set("https://example.test/a.png" 1x)"#,
    ] {
        let css = declaration_css(
            input,
            &StrictPolicy::new().allow_properties(&["background-image", "--asset"]),
        );
        assert!(css.is_empty(), "resource survived: {input} -> {css}");
    }
}

#[test]
fn dynamic_resource_wrappers_require_dynamic_and_resource_permissions() {
    let input = "background-image: src(var(--asset))";

    let resource_only = StrictPolicy::new()
        .allow_properties(&["background-image"])
        .allow_resources(&[ResourceUse::Image]);
    assert!(declaration_css(input, &resource_only).is_empty());

    let dynamic_only = StrictPolicy::new()
        .allow_properties(&["background-image"])
        .allow_variables();
    assert!(declaration_css(input, &dynamic_only).is_empty());

    let both = dynamic_only.allow_resources(&[ResourceUse::Image]);
    assert!(declaration_css(input, &both).contains("var("));
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SeenResource {
    syntax: ResourceSyntax,
    use_kind: ResourceUse,
    value: Option<String>,
    location: PropertyLocation,
    property: Option<String>,
    descriptor: Option<DescriptorKind>,
}

#[derive(Default)]
struct ResourceRecorder {
    seen: RefCell<Vec<SeenResource>>,
}

impl CssPolicy for ResourceRecorder {
    fn rule(&self, _rule: &mut CssRule<'_>, context: RuleContext) -> NodeDecision {
        if context.kind == RuleKind::FontFace {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn property(
        &self,
        _property: &mut Property<'_>,
        _context: PropertyContext<'_>,
    ) -> NodeDecision {
        NodeDecision::Keep
    }

    fn font_face_descriptor(
        &self,
        _property: &mut FontFaceProperty<'_>,
        _context: DescriptorContext,
    ) -> NodeDecision {
        NodeDecision::Keep
    }

    fn resource(&self, resource: ResourceRef<'_>, context: &ValueContext<'_>) -> ValueDecision {
        self.seen.borrow_mut().push(SeenResource {
            syntax: resource.syntax,
            use_kind: resource.use_kind,
            value: resource.value.map(str::to_owned),
            location: context.location,
            property: context.property.as_ref().map(|key| key.name().to_owned()),
            descriptor: context.descriptor,
        });
        ValueDecision::Allow
    }

    fn dynamic_value(
        &self,
        _value: DynamicValueRef<'_, '_>,
        _context: &ValueContext<'_>,
    ) -> ValueDecision {
        ValueDecision::Allow
    }

    fn token(&self, _token: &TokenOrValue<'_>, _context: &ValueContext<'_>) -> ValueDecision {
        ValueDecision::Allow
    }
}

#[test]
fn resource_hook_receives_syntax_use_property_and_location() {
    let recorder = ResourceRecorder::default();
    let css = declaration_css(
        r#"
            background-image: url("image.png");
            cursor: url("cursor.cur"), auto;
            list-style-image: url("marker.svg");
            list-style: url("marker-short.svg") disc;
            content: url("content.png");
            mask-image: url("mask.svg");
            filter: url("filter.svg#blur");
            backdrop-filter: url("backdrop.svg#blur");
            fill: url("paint.svg#gradient");
            --asset: SRC("generic.bin");
            --background-image: url("custom-token.png");
        "#,
        &recorder,
    );
    assert!(!css.is_empty());

    let seen = recorder.seen.borrow();
    assert!(seen.iter().any(|item| {
        item.syntax == ResourceSyntax::Url
            && item.use_kind == ResourceUse::Image
            && item.property.as_deref() == Some("background-image")
            && item.location == PropertyLocation::DeclarationList
    }));
    assert!(seen.iter().any(|item| item.use_kind == ResourceUse::Cursor));
    assert!(
        seen.iter()
            .any(|item| item.use_kind == ResourceUse::ListStyleImage)
    );
    assert!(
        seen.iter()
            .any(|item| item.use_kind == ResourceUse::Content)
    );
    assert!(
        seen.iter()
            .any(|item| item.use_kind == ResourceUse::MaskImage)
    );
    assert!(
        seen.iter()
            .any(|item| item.use_kind == ResourceUse::FilterReference)
    );
    assert!(
        seen.iter()
            .any(|item| item.use_kind == ResourceUse::SvgPaintServer)
    );
    assert!(seen.iter().any(|item| {
        item.syntax == ResourceSyntax::Src
            && item.use_kind == ResourceUse::Other
            && item.value.as_deref() == Some("generic.bin")
    }));
    assert!(seen.iter().any(|item| {
        item.use_kind == ResourceUse::Other && item.value.as_deref() == Some("custom-token.png")
    }));
}

#[test]
fn font_source_context_is_distinct_from_image_context() {
    let recorder = ResourceRecorder::default();
    let css = stylesheet_css("@font-face { src: url('font.woff2') }", &recorder);
    assert!(css.contains("font.woff2"));

    let seen = recorder.seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].use_kind, ResourceUse::FontSource);
    assert_eq!(
        seen[0].descriptor,
        Some(DescriptorKind::FontFace(FontFaceDescriptorKind::Source))
    );
}

#[test]
fn resource_use_permissions_do_not_bleed_between_css_features() {
    let policy = StrictPolicy::new()
        .allow_properties(&["background-image", "cursor"])
        .allow_resources(&[ResourceUse::Image]);
    let css = declaration_css(
        "background-image: url('image.png'); cursor: url('cursor.cur'), auto",
        &policy,
    );
    assert!(css.contains("image.png"));
    assert!(!css.contains("cursor.cur"));
}

#[test]
fn local_font_access_is_separate_from_font_urls() {
    let base = StrictPolicy::new()
        .allow_rules(&[RuleKind::FontFace])
        .allow_font_face_descriptors(&[
            FontFaceDescriptorKind::FontFamily,
            FontFaceDescriptorKind::Source,
        ])
        .allow_resources(&[ResourceUse::FontSource]);

    let denied = stylesheet_css(
        "@font-face { font-family: Demo; src: local('Installed Font') }",
        &base,
    );
    assert!(denied.contains("font-family"));
    assert!(!denied.contains("local("));

    let allowed = stylesheet_css(
        "@font-face { font-family: Demo; src: local('Installed Font') }",
        &base.allow_local_fonts(),
    );
    assert!(allowed.contains("local("));
}

struct HostRestrictedImport;

impl CssPolicy for HostRestrictedImport {
    fn rule(&self, _rule: &mut CssRule<'_>, context: RuleContext) -> NodeDecision {
        if context.kind == RuleKind::Import {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn import(&self, context: ImportContext<'_>) -> ImportDecision {
        if context.url == "https://static.example.test/theme.css" {
            ImportDecision::AllowPassthrough
        } else {
            ImportDecision::Deny
        }
    }

    fn resource(&self, _resource: ResourceRef<'_>, _context: &ValueContext<'_>) -> ValueDecision {
        panic!("imports must not be routed through the general resource hook")
    }
}

#[test]
fn import_has_a_dedicated_policy_boundary() {
    let safe = stylesheet_css(
        "@import 'https://static.example.test/theme.css';",
        &HostRestrictedImport,
    );
    assert!(safe.contains("@import"));

    let denied = stylesheet_css(
        "@import 'https://other.example.test/theme.css';",
        &HostRestrictedImport,
    );
    assert!(denied.is_empty());
}

#[test]
fn import_passthrough_does_not_claim_to_sanitize_remote_contents() {
    let css = stylesheet_css(
        "@import 'https://example.test/theme.css';",
        &StrictPolicy::new().dangerously_allow_passthrough_imports(),
    );
    assert!(css.contains("@import"));
}

struct PagePolicy;

impl CssPolicy for PagePolicy {
    fn rule(&self, _rule: &mut CssRule<'_>, context: RuleContext) -> NodeDecision {
        if context.kind == RuleKind::Page {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn page_margin_rule(
        &self,
        _rule: &mut css_sanitizer::lightningcss::rules::page::PageMarginRule<'_>,
        _context: RuleContext,
    ) -> NodeDecision {
        NodeDecision::Keep
    }

    fn property(&self, _property: &mut Property<'_>, context: PropertyContext<'_>) -> NodeDecision {
        if matches!(context.key.name(), "margin" | "color" | "background-image") {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn token(&self, _token: &TokenOrValue<'_>, _context: &ValueContext<'_>) -> ValueDecision {
        ValueDecision::Allow
    }
}

#[test]
fn page_margin_subrules_are_independently_controllable_and_guarded() {
    let css = stylesheet_css(
        r#"
        @page {
            margin: 1cm;
            @top-left {
                color: red;
                background-image: url("https://example.test/page.png");
            }
        }
        "#,
        &PagePolicy,
    );
    assert!(css.contains("@page"));
    assert!(css.contains("margin"));
    assert!(css.contains("color"));
    assert!(!css.contains("url("));
}

#[test]
fn wrapper_rules_recursively_apply_selector_property_and_resource_policy() {
    for (kind, input) in [
        (
            RuleKind::Supports,
            "@supports (display: block) { .card { background-image: url('a.png') } }",
        ),
        (
            RuleKind::Container,
            "@container (min-width: 10px) { .card { background-image: url('a.png') } }",
        ),
        (
            RuleKind::Scope,
            "@scope (.root) { .card { background-image: url('a.png') } }",
        ),
        (
            RuleKind::LayerBlock,
            "@layer component { .card { background-image: url('a.png') } }",
        ),
        (
            RuleKind::StartingStyle,
            "@starting-style { .card { background-image: url('a.png') } }",
        ),
    ] {
        let policy = style_policy(&["background-image"]).allow_rules(&[kind]);
        let css = stylesheet_css(input, &policy);
        assert!(css.is_empty(), "{kind:?} retained guarded content: {css}");
    }
}

#[test]
fn rejection_is_visible_in_the_report() {
    let output = declaration(
        "background-image: url('a.png')",
        &StrictPolicy::new().allow_properties(&["background-image"]),
    );
    assert!(output.css.is_empty());
    assert_eq!(output.report.dropped_declarations, 1);
    assert_eq!(output.report.rejected_values, 1);
}
