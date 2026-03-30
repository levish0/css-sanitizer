mod common;

use common::StrictPolicy;
use css_sanitizer::{clean_stylesheet_with_policy, RuleKind};

#[test]
fn stylesheet_keeps_style_rules() {
    let result = clean_stylesheet_with_policy(
        ".foo { color: red; }",
        &StrictPolicy::new().allow_properties(&["color"]),
    );
    assert!(result.contains(".foo"));
    assert!(result.contains("color"));
}

#[test]
fn stylesheet_strips_disallowed_properties() {
    let result = clean_stylesheet_with_policy(
        ".foo { color: red; position: fixed; }",
        &StrictPolicy::new().allow_properties(&["color"]),
    );
    assert!(result.contains("color"));
    assert!(!result.contains("position"));
}

#[test]
fn stylesheet_removes_empty_rules() {
    let result = clean_stylesheet_with_policy(
        ".foo { position: fixed; }",
        &StrictPolicy::new().allow_properties(&["color"]),
    );
    assert!(!result.contains(".foo"));
}

#[test]
fn stylesheet_blocks_import_by_default() {
    let result = clean_stylesheet_with_policy(
        "@import url('evil.css'); .foo { color: red; }",
        &StrictPolicy::new().allow_properties(&["color"]),
    );
    assert!(!result.contains("@import"));
    assert!(result.contains("color"));
}

#[test]
fn stylesheet_allows_import_only_with_rule_and_url() {
    let result = clean_stylesheet_with_policy(
        "@import url('https://example.com/style.css');",
        &StrictPolicy::new()
            .allow_rules(&[RuleKind::Import])
            .allow_url(),
    );
    assert!(result.contains("@import"));
}

#[test]
fn stylesheet_blocks_media_by_default() {
    let result = clean_stylesheet_with_policy(
        "@media (max-width: 768px) { .foo { color: red; } }",
        &StrictPolicy::new().allow_properties(&["color"]),
    );
    assert!(!result.contains("@media"));
}

#[test]
fn stylesheet_allows_media_when_permitted() {
    let result = clean_stylesheet_with_policy(
        "@media (max-width: 768px) { .foo { color: red; } }",
        &StrictPolicy::new()
            .allow_properties(&["color"])
            .allow_rules(&[RuleKind::Media]),
    );
    assert!(result.contains("@media"));
    assert!(result.contains("color"));
}

#[test]
fn stylesheet_font_face_strips_src_without_url_permission() {
    let result = clean_stylesheet_with_policy(
        "@font-face { font-family: Evil; src: url('evil.woff'); }",
        &StrictPolicy::new().allow_rules(&[RuleKind::FontFace]),
    );
    assert!(result.contains("@font-face"));
    assert!(result.contains("font-family"));
    assert!(!result.contains("src:"));
    assert!(!result.contains("url("));
}

#[test]
fn stylesheet_filters_keyframes_when_allowed() {
    let result = clean_stylesheet_with_policy(
        "@keyframes fade { from { opacity: 0; background-image: url('evil.png'); } to { opacity: 1; } }",
        &StrictPolicy::new()
            .allow_rules(&[RuleKind::Keyframes])
            .allow_properties(&["opacity", "background-image"]),
    );
    assert!(result.contains("@keyframes"));
    assert!(result.contains("opacity"));
    assert!(!result.contains("url("));
}

#[test]
fn stylesheet_removes_empty_media_after_filtering() {
    let result = clean_stylesheet_with_policy(
        "@media (max-width: 768px) { .foo { position: fixed; } }",
        &StrictPolicy::new()
            .allow_properties(&["color"])
            .allow_rules(&[RuleKind::Media]),
    );
    assert!(!result.contains("@media"));
}
