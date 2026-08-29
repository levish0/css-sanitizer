mod common;

use common::{style_policy, stylesheet, stylesheet_css};
use css_sanitizer::{
    FontFaceDescriptorKind, FontPaletteValuesDescriptorKind, ResourceUse, RuleKind, StrictPolicy,
    ViewTransitionDescriptorKind,
};

#[test]
fn stylesheet_requires_explicit_selector_capability() {
    let denied = stylesheet_css(
        ".card { color: red }",
        &StrictPolicy::new().allow_properties(&["color"]),
    );
    assert!(denied.is_empty());

    let allowed = stylesheet_css(".card { color: red }", &style_policy(&["color"]));
    assert!(allowed.contains(".card"));
    assert!(allowed.contains("color"));
}

#[test]
fn style_rules_filter_properties_and_prune_empty_rules() {
    let output = stylesheet(
        ".kept { color: red; position: fixed } .empty { position: fixed }",
        &style_policy(&["color"]),
    );
    assert!(output.css.as_str().contains(".kept"));
    assert!(!output.css.as_str().contains(".empty"));
    assert!(!output.css.as_str().contains("position"));
    assert_eq!(output.report.dropped_declarations, 2);
    assert!(output.report.dropped_rules >= 1);
}

#[test]
fn import_is_denied_even_when_resources_are_allowed() {
    let css = stylesheet_css(
        "@import url('https://example.test/theme.css'); .card { color: red }",
        &style_policy(&["color"]).allow_resources(&[ResourceUse::Image]),
    );
    assert!(!css.contains("@import"));
    assert!(css.contains("color"));
}

#[test]
fn import_requires_the_dangerous_passthrough_capability() {
    let css = stylesheet_css(
        "@import url('https://example.test/theme.css');",
        &StrictPolicy::new().dangerously_allow_passthrough_imports(),
    );
    assert!(css.contains("@import"));
}

#[test]
fn media_is_typed_and_recursively_sanitized() {
    let css = stylesheet_css(
        "@media (max-width: 768px) { .card { color: red; position: fixed } }",
        &style_policy(&["color"]).allow_rules(&[RuleKind::Media]),
    );
    assert!(css.contains("@media"));
    assert!(css.contains("color"));
    assert!(!css.contains("position"));
}

#[test]
fn empty_wrapper_rules_are_pruned() {
    let css = stylesheet_css(
        "@media all { .card { position: fixed } }",
        &style_policy(&["color"]).allow_rules(&[RuleKind::Media]),
    );
    assert!(css.is_empty());
}

#[test]
fn keyframes_use_the_shared_property_policy_and_resource_guard() {
    let css = stylesheet_css(
        "@keyframes fade { from { opacity: 0; background-image: url('https://example.test/a.png') } to { opacity: 1 } }",
        &StrictPolicy::new()
            .allow_rules(&[RuleKind::Keyframes])
            .allow_properties(&["opacity", "background-image"]),
    );
    assert!(css.contains("@keyframes"));
    assert!(css.contains("opacity"));
    assert!(!css.contains("url("));
}

#[test]
fn font_face_requires_rule_descriptor_and_resource_capabilities() {
    let policy = StrictPolicy::new()
        .allow_rules(&[RuleKind::FontFace])
        .allow_font_face_descriptors(&[
            FontFaceDescriptorKind::FontFamily,
            FontFaceDescriptorKind::Source,
        ])
        .allow_resources(&[ResourceUse::FontSource]);

    let css = stylesheet_css(
        "@font-face { font-family: Demo; src: local('Installed Font'), url('https://example.test/demo.woff2') }",
        &policy,
    );
    assert!(css.contains("@font-face"));
    assert!(css.contains("font-family"));
    assert!(css.contains("example.test"));
    assert!(!css.contains("local("));
}

#[test]
fn other_descriptor_rules_have_typed_preset_allowlists() {
    let palette = stylesheet_css(
        "@font-palette-values --demo { base-palette: 1; override-colors: 0 red }",
        &StrictPolicy::new()
            .allow_rules(&[RuleKind::FontPaletteValues])
            .allow_font_palette_values_descriptors(&[FontPaletteValuesDescriptorKind::BasePalette]),
    );
    assert!(palette.contains("base-palette"));
    assert!(!palette.contains("override-colors"));

    let transition = stylesheet_css(
        "@view-transition { navigation: auto; types: slide }",
        &StrictPolicy::new()
            .allow_rules(&[RuleKind::ViewTransition])
            .allow_view_transition_descriptors(&[ViewTransitionDescriptorKind::Navigation]),
    );
    assert!(transition.contains("navigation"));
    assert!(!transition.contains("types:"));
}

#[test]
fn nested_descriptor_like_rules_are_separate_capabilities() {
    let page = stylesheet_css(
        "@page { @top-left { color: red } }",
        &StrictPolicy::new()
            .allow_rules(&[RuleKind::Page])
            .allow_page_margin_rules()
            .allow_properties(&["color"]),
    );
    assert!(page.contains("@top-left"));

    let features = stylesheet_css(
        "@font-feature-values Demo { @styleset { alt: 1 } }",
        &StrictPolicy::new()
            .allow_rules(&[RuleKind::FontFeatureValues])
            .allow_font_feature_values_subrules(),
    );
    assert!(features.contains("@styleset"));
}
