mod common;

use std::cell::RefCell;

use common::StrictPolicy;
use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::properties::custom::{
    EnvironmentVariable, Token, TokenOrValue, Variable,
};
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::rules::font_feature_values::FontFeatureValuesRule;
use css_sanitizer::lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use css_sanitizer::lightningcss::rules::view_transition::ViewTransitionProperty;
use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
use css_sanitizer::lightningcss::values::url::Url;
use css_sanitizer::{
    CssSanitizationPolicy, DescriptorContext, NodeAction, PropertyContext, ResourceKind,
    ResourceRef, RuleContext, ValueAction, ValueContext, ValueLocation,
    clean_declaration_list_with_policy, clean_stylesheet_with_policy, sanitize_stylesheet_ast,
};

fn sanitize_parsed_stylesheet(input: &str, policy: &dyn CssSanitizationPolicy) -> String {
    let mut stylesheet =
        StyleSheet::parse(input, ParserOptions::default()).expect("stylesheet should parse");
    sanitize_stylesheet_ast(&mut stylesheet, policy);
    stylesheet
        .to_css(Default::default())
        .expect("stylesheet should serialize")
        .code
        .trim()
        .to_string()
}

struct DropSpecialDescriptorsPolicy;

impl CssSanitizationPolicy for DropSpecialDescriptorsPolicy {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        match rule {
            CssRule::FontPaletteValues(_)
            | CssRule::ViewTransition(_)
            | CssRule::FontFeatureValues(_) => NodeAction::Continue,
            _ => NodeAction::Drop,
        }
    }

    fn visit_font_palette_values_property(
        &self,
        _property: &mut FontPaletteValuesProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Drop
    }

    fn visit_view_transition_property(
        &self,
        _property: &mut ViewTransitionProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        NodeAction::Drop
    }

    fn visit_font_feature_values_rule(
        &self,
        _rule: &mut FontFeatureValuesRule<'_>,
        _ctx: RuleContext,
    ) -> NodeAction {
        NodeAction::Drop
    }
}

#[test]
fn import_rule_requires_url_permission_even_when_rule_is_whitelisted() {
    let result = clean_stylesheet_with_policy(
        "@import url('https://evil.test/a.css');",
        &StrictPolicy::new().allow_rules(&["import"]),
    );
    assert_eq!(result, "");
}

#[test]
fn string_import_is_preserved_only_when_rule_and_url_are_allowed() {
    let result = clean_stylesheet_with_policy(
        "@import \"https://example.com/safe.css\";",
        &StrictPolicy::new().allow_rules(&["import"]).allow_url(),
    );
    assert!(result.contains("@import"));
}

#[test]
fn malformed_inline_css_does_not_escape_into_new_rules() {
    let result = clean_declaration_list_with_policy(
        "color: red; } .owned { background-image: url('https://evil.test/x.png') }",
        &StrictPolicy::new().allow_properties(&["color", "background-image"]),
    );
    assert_eq!(result, "color: red");
}

#[test]
fn image_set_with_nested_url_is_blocked_without_url_permission() {
    let result = clean_declaration_list_with_policy(
        "background-image: image-set(url('https://evil.test/x.png') 1x)",
        &StrictPolicy::new().allow_properties(&["background-image"]),
    );
    assert_eq!(result, "");
}

#[test]
fn spec_resource_functions_are_blocked_without_url_permission() {
    for css in [
        r#"background-image: src("https://evil.test/src.png")"#,
        r#"background-image: SRC("https://evil.test/upper.png")"#,
        r#"background-image: s\72 c("https://evil.test/escaped.png")"#,
        r#"--asset: URL("https://evil.test/upper-url.png")"#,
        r#"--asset: u\72 l("https://evil.test/escaped-url.png")"#,
        r#"background-image: image("https://evil.test/image.png")"#,
        r#"background-image: image(ltr "https://evil.test/directed.png")"#,
        r#"--asset: image-set("https://evil.test/set.png" 1x)"#,
        r#"--asset: -webkit-image-set("https://evil.test/webkit.png" 1x)"#,
    ] {
        let result = clean_declaration_list_with_policy(
            css,
            &StrictPolicy::new().allow_properties(&["background-image", "--asset"]),
        );
        assert!(
            result.is_empty(),
            "resource survived: css={css:?} result={result:?}"
        );
    }
}

#[test]
fn spec_resource_functions_are_preserved_only_with_url_permission() {
    for css in [
        r#"background-image: src("https://safe.test/src.png")"#,
        r#"background-image: SRC("https://safe.test/upper.png")"#,
        r#"background-image: s\72 c("https://safe.test/escaped.png")"#,
        r#"--asset: URL("https://safe.test/upper-url.png")"#,
        r#"--asset: u\72 l("https://safe.test/escaped-url.png")"#,
        r#"background-image: image("https://safe.test/image.png")"#,
        r#"background-image: image(ltr "https://safe.test/directed.png")"#,
        r#"--asset: image-set("https://safe.test/set.png" 1x)"#,
        r#"--asset: -webkit-image-set("https://safe.test/webkit.png" 1x)"#,
    ] {
        let result = clean_declaration_list_with_policy(
            css,
            &StrictPolicy::new()
                .allow_properties(&["background-image", "--asset"])
                .allow_url(),
        );
        assert!(
            result.contains("safe.test"),
            "resource was not preserved: {result:?}"
        );
    }
}

#[test]
fn dynamic_src_requires_both_resource_and_variable_permission() {
    let css = "background-image: src(var(--asset))";

    let without_var = clean_declaration_list_with_policy(
        css,
        &StrictPolicy::new()
            .allow_properties(&["background-image"])
            .allow_url(),
    );
    assert!(without_var.is_empty(), "got: {without_var:?}");

    let allowed = clean_declaration_list_with_policy(
        css,
        &StrictPolicy::new()
            .allow_properties(&["background-image"])
            .allow_url()
            .allow_var(),
    );
    assert!(allowed.contains("src("), "got: {allowed:?}");
    assert!(allowed.contains("var("), "got: {allowed:?}");
}

#[test]
fn case_and_escape_variants_of_var_and_env_use_dedicated_permissions() {
    for (css, policy) in [
        (
            "width: VAR(--size, 10px)",
            StrictPolicy::new().allow_properties(&["width"]).allow_var(),
        ),
        (
            r"width: V\41 R(--size, 10px)",
            StrictPolicy::new().allow_properties(&["width"]).allow_var(),
        ),
        (
            "width: ENV(safe-area-inset-left, 10px)",
            StrictPolicy::new().allow_properties(&["width"]).allow_env(),
        ),
    ] {
        let result = clean_declaration_list_with_policy(css, &policy);
        assert!(
            !result.is_empty(),
            "dedicated permission failed: {result:?}"
        );
    }

    for (function, css) in [
        ("var", "width: VAR(--size, 10px)"),
        ("env", "width: ENV(safe-area-inset-left, 10px)"),
    ] {
        let result = clean_declaration_list_with_policy(
            css,
            &StrictPolicy::new()
                .allow_properties(&["width"])
                .allow_functions(&[function]),
        );
        assert!(result.is_empty(), "generic allowlist bypassed {function}");
    }

    for (css, policy) in [
        (
            r#"width: VAR(--size, URL("https://evil.test/var.png"))"#,
            StrictPolicy::new().allow_properties(&["width"]).allow_var(),
        ),
        (
            r#"width: ENV(css-sanitizer-missing, URL("https://evil.test/env.png"))"#,
            StrictPolicy::new().allow_properties(&["width"]).allow_env(),
        ),
    ] {
        let result = clean_declaration_list_with_policy(css, &policy);
        assert!(result.is_empty(), "fallback resource survived: {result:?}");
    }
}

#[test]
fn unparsed_var_and_env_inside_image_functions_require_resource_permission() {
    for css in [
        "background-image: image-set(VAR(--asset) 1x)",
        r#"background-image: image-set(ENV(css-sanitizer-missing, "https://evil.test/e.png") 1x)"#,
        "background-image: image(future(VAR(--asset)))",
    ] {
        let result = clean_declaration_list_with_policy(
            css,
            &StrictPolicy::new()
                .allow_properties(&["background-image"])
                .allow_functions(&["var", "env", "future"])
                .allow_var()
                .allow_env(),
        );
        assert!(result.is_empty(), "dynamic resource survived: {result:?}");
    }

    let allowed = clean_declaration_list_with_policy(
        "background-image: image-set(VAR(--asset) 1x)",
        &StrictPolicy::new()
            .allow_properties(&["background-image"])
            .allow_var()
            .allow_url(),
    );
    assert!(allowed.contains("VAR("), "got: {allowed:?}");
}

#[test]
fn unresolved_functions_inside_image_syntax_require_resource_permission() {
    for css in [
        "background-image: image-set(--asset() 1x)",
        "background-image: image(--asset())",
        r#"background-image: image-set(if(media(width >= 0px): "https://evil.test/if.png"; else: "fallback.png") 1x)"#,
    ] {
        let result = clean_declaration_list_with_policy(
            css,
            &StrictPolicy::new()
                .allow_properties(&["background-image"])
                .allow_functions(&["--asset", "if", "media"]),
        );
        assert!(
            result.is_empty(),
            "unresolved resource survived: {result:?}"
        );
    }

    let allowed = clean_declaration_list_with_policy(
        "background-image: image-set(--asset() 1x)",
        &StrictPolicy::new()
            .allow_properties(&["background-image"])
            .allow_functions(&["--asset"])
            .allow_url(),
    );
    assert!(allowed.contains("--asset("), "got: {allowed:?}");
}

#[test]
fn dashed_custom_function_allowlist_is_case_sensitive() {
    let wrong_case = clean_declaration_list_with_policy(
        "width: --SAFE()",
        &StrictPolicy::new()
            .allow_properties(&["width"])
            .allow_functions(&["--safe"]),
    );
    assert!(wrong_case.is_empty(), "got: {wrong_case:?}");

    let exact_case = clean_declaration_list_with_policy(
        "width: --SAFE()",
        &StrictPolicy::new()
            .allow_properties(&["width"])
            .allow_functions(&["--SAFE"]),
    );
    assert!(exact_case.contains("--SAFE("), "got: {exact_case:?}");
}

#[derive(Default)]
struct ResourceRecorder {
    seen: RefCell<Vec<(ResourceKind, Option<String>, ValueLocation)>>,
}

impl CssSanitizationPolicy for ResourceRecorder {
    fn visit_property(&self, _property: &mut Property<'_>, _ctx: PropertyContext) -> NodeAction {
        NodeAction::Continue
    }

    fn check_resource(&self, resource: ResourceRef<'_>, ctx: ValueContext) -> ValueAction {
        self.seen.borrow_mut().push((
            resource.kind,
            resource.value.map(str::to_owned),
            ctx.location,
        ));
        ValueAction::Allow
    }

    fn check_variable(&self, _variable: &Variable<'_>, _ctx: ValueContext) -> ValueAction {
        ValueAction::Allow
    }

    fn check_environment_variable(
        &self,
        _env: &EnvironmentVariable<'_>,
        _ctx: ValueContext,
    ) -> ValueAction {
        ValueAction::Allow
    }

    fn check_unparsed_variable(
        &self,
        _function: &css_sanitizer::lightningcss::properties::custom::Function<'_>,
        _ctx: ValueContext,
    ) -> ValueAction {
        ValueAction::Allow
    }

    fn check_unparsed_environment_variable(
        &self,
        _function: &css_sanitizer::lightningcss::properties::custom::Function<'_>,
        _ctx: ValueContext,
    ) -> ValueAction {
        ValueAction::Allow
    }

    fn check_token(&self, _token: &TokenOrValue<'_>, _ctx: ValueContext) -> ValueAction {
        ValueAction::Allow
    }
}

#[test]
fn semantic_resource_variants_reach_the_shared_hook_with_exact_metadata() {
    let recorder = ResourceRecorder::default();
    let result = clean_declaration_list_with_policy(
        r#"
            background-image: image-set("typed-set.png" 1x);
            --upper: SRC("upper.png");
            --escaped: s\72 c("escaped.png");
            --upper-url: URL("upper-url.png");
            --escaped-url: u\72 l("escaped-url.png");
            --webkit: -webkit-image-set("webkit.png" 1x);
            --dynamic-image: image(var(--asset));
            --dynamic-set: image-set(env(asset));
        "#,
        &recorder,
    );

    assert!(!result.is_empty(), "got: {result:?}");
    assert_eq!(
        recorder.seen.into_inner(),
        vec![
            (
                ResourceKind::Url,
                Some("typed-set.png".into()),
                ValueLocation::DeclarationList,
            ),
            (
                ResourceKind::Src,
                Some("upper.png".into()),
                ValueLocation::DeclarationList,
            ),
            (
                ResourceKind::Src,
                Some("escaped.png".into()),
                ValueLocation::DeclarationList,
            ),
            (
                ResourceKind::Url,
                Some("upper-url.png".into()),
                ValueLocation::DeclarationList,
            ),
            (
                ResourceKind::Url,
                Some("escaped-url.png".into()),
                ValueLocation::DeclarationList,
            ),
            (
                ResourceKind::ImageSet,
                Some("webkit.png".into()),
                ValueLocation::DeclarationList,
            ),
            (ResourceKind::Image, None, ValueLocation::DeclarationList,),
            (ResourceKind::ImageSet, None, ValueLocation::DeclarationList,),
        ]
    );
}

#[test]
fn unknown_functions_are_fail_closed_unless_explicitly_allowed() {
    let denied = clean_declaration_list_with_policy(
        "width: future-size(10px)",
        &StrictPolicy::new().allow_properties(&["width"]),
    );
    assert!(denied.is_empty(), "got: {denied:?}");

    let allowed = clean_declaration_list_with_policy(
        "width: future-size(10px)",
        &StrictPolicy::new()
            .allow_properties(&["width"])
            .allow_functions(&["future-size"]),
    );
    assert!(allowed.contains("future-size("), "got: {allowed:?}");
}

#[test]
fn resource_functions_cannot_be_enabled_through_generic_function_allowlist() {
    for (function, spelling) in [
        ("url", "URL"),
        ("src", "src"),
        ("image", "image"),
        ("image-set", "image-set"),
        ("-webkit-image-set", "-webkit-image-set"),
    ] {
        let css = format!("--asset: {spelling}(\"https://evil.test/asset\")");
        let result = clean_declaration_list_with_policy(
            &css,
            &StrictPolicy::new()
                .allow_properties(&["--asset"])
                .allow_functions(&[function]),
        );

        assert!(result.is_empty(), "resource survived: {result:?}");
    }
}

#[test]
fn strict_policy_denies_unstructured_raw_function_tokens() {
    let policy = StrictPolicy::new();
    let action = policy.check_token(
        &TokenOrValue::Token(Token::Function("future".into())),
        ValueContext {
            depth: 0,
            important: false,
            location: ValueLocation::DeclarationList,
        },
    );

    assert_eq!(action, ValueAction::Deny);
}

#[test]
fn legacy_expression_is_denied_even_when_named_in_function_allowlist() {
    let result = clean_declaration_list_with_policy(
        "width: expression(alert(1))",
        &StrictPolicy::new()
            .allow_properties(&["width"])
            .allow_functions(&["expression", "alert"]),
    );

    assert!(result.is_empty(), "got: {result:?}");
}

#[test]
fn ordinary_strings_and_local_fonts_are_not_resources() {
    let content = clean_declaration_list_with_policy(
        r#"content: "https://not-a-fetch.test/value""#,
        &StrictPolicy::new().allow_properties(&["content"]),
    );
    assert!(content.contains("not-a-fetch.test"), "got: {content:?}");

    let font = clean_stylesheet_with_policy(
        r#"@font-face { font-family: Test; src: local("Arial") }"#,
        &StrictPolicy::new().allow_rules(&["font-face"]),
    );
    assert!(font.contains("local("), "got: {font:?}");
}

struct SafeImportOnly;

impl CssSanitizationPolicy for SafeImportOnly {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        if matches!(rule, CssRule::Import(_)) {
            NodeAction::Continue
        } else {
            NodeAction::Drop
        }
    }

    fn check_resource(&self, resource: ResourceRef<'_>, ctx: ValueContext) -> ValueAction {
        if matches!(resource.kind, ResourceKind::Import)
            && resource.value == Some("https://safe.test/theme.css")
            && matches!(ctx.location, ValueLocation::ImportRule)
        {
            ValueAction::Allow
        } else {
            ValueAction::Deny
        }
    }
}

#[test]
fn import_uses_the_shared_resource_policy() {
    let safe =
        clean_stylesheet_with_policy(r#"@import "https://safe.test/theme.css";"#, &SafeImportOnly);
    assert!(safe.contains("@import"), "got: {safe:?}");

    let evil = clean_stylesheet_with_policy(
        "@import url('https://evil.test/theme.css');",
        &SafeImportOnly,
    );
    assert!(evil.is_empty(), "got: {evil:?}");
}

struct LegacyTypedUrlPolicy;

impl CssSanitizationPolicy for LegacyTypedUrlPolicy {
    fn visit_property(&self, _property: &mut Property<'_>, _ctx: PropertyContext) -> NodeAction {
        NodeAction::Continue
    }

    fn check_url(&self, _url: &Url<'_>, _ctx: ValueContext) -> ValueAction {
        ValueAction::Allow
    }

    fn check_resource(&self, _resource: ResourceRef<'_>, _ctx: ValueContext) -> ValueAction {
        ValueAction::Deny
    }
}

#[test]
fn legacy_check_url_override_remains_authoritative_for_typed_urls_only() {
    let typed = clean_declaration_list_with_policy(
        "background-image: url('legacy.png')",
        &LegacyTypedUrlPolicy,
    );
    assert!(typed.contains("legacy.png"), "got: {typed:?}");

    let generic =
        clean_declaration_list_with_policy("--asset: URL('blocked.png')", &LegacyTypedUrlPolicy);
    assert!(generic.is_empty(), "got: {generic:?}");
}

#[test]
fn namespace_identifier_is_not_treated_as_a_fetchable_resource() {
    let result = clean_stylesheet_with_policy(
        r#"@namespace svg "http://www.w3.org/2000/svg";"#,
        &StrictPolicy::new().allow_rules(&["namespace"]),
    );
    assert!(result.contains("@namespace"), "got: {result:?}");
}

#[test]
fn wrapper_rules_recursively_sanitize_hidden_url_payloads() {
    let result = sanitize_parsed_stylesheet(
        r#"
        @supports (display: block) {
            .supports { background-image: url("https://evil.test/supports.png"); }
        }
        @container (min-width: 10px) {
            .container { background-image: url("https://evil.test/container.png"); }
        }
        @scope (.card) {
            .scope { background-image: url("https://evil.test/scope.png"); }
        }
        @layer audit {
            .layer { background-image: url("https://evil.test/layer.png"); }
        }
        @starting-style {
            .start { background-image: url("https://evil.test/starting-style.png"); }
        }
        "#,
        &StrictPolicy::new()
            .allow_rules(&[
                "supports",
                "container",
                "scope",
                "layer-block",
                "starting-style",
            ])
            .allow_properties(&["background-image"]),
    );

    assert_eq!(result, "");
}

#[test]
fn nesting_rules_are_recursively_sanitized_and_pruned_when_empty() {
    let result = sanitize_parsed_stylesheet(
        r#"
        .card {
            & .child {
                background-image: url("https://evil.test/nesting.png");
            }
        }
        "#,
        &StrictPolicy::new().allow_properties(&["background-image"]),
    );

    assert_eq!(result, "");
}

#[test]
fn page_margin_rules_strip_nested_urls_but_keep_safe_properties() {
    let result = sanitize_parsed_stylesheet(
        r#"
        @page {
            margin: 1cm;
            @top-left {
                color: red;
                background-image: url("https://evil.test/page.png");
            }
        }
        "#,
        &StrictPolicy::new()
            .allow_rules(&["page"])
            .allow_properties(&["margin", "color", "background-image"]),
    );

    assert!(result.contains("@page"));
    assert!(result.contains("margin"));
    assert!(result.contains("color"));
    assert!(!result.contains("url("));
}

#[test]
fn viewport_rules_filter_disallowed_properties() {
    let result = sanitize_parsed_stylesheet(
        "@viewport { zoom: 1; width: device-width; }",
        &StrictPolicy::new()
            .allow_rules(&["viewport"])
            .allow_properties(&["zoom"]),
    );

    assert!(result.contains("@viewport"));
    assert!(result.contains("zoom"));
    assert!(!result.contains("width"));
}

#[test]
fn font_palette_values_rule_drops_when_descriptor_policy_removes_everything() {
    let result = sanitize_parsed_stylesheet(
        "@font-palette-values --brand { base-palette: 1; override-colors: 0 red; }",
        &DropSpecialDescriptorsPolicy,
    );

    assert_eq!(result, "");
}

#[test]
fn view_transition_rule_drops_when_descriptor_policy_removes_everything() {
    let result = sanitize_parsed_stylesheet(
        "@view-transition { navigation: auto; }",
        &DropSpecialDescriptorsPolicy,
    );

    assert_eq!(result, "");
}

#[test]
fn font_feature_values_rule_hook_can_drop_the_entire_rule() {
    let result = sanitize_parsed_stylesheet(
        "@font-feature-values Test Sans { @styleset { alt-glyphs: 1; } }",
        &DropSpecialDescriptorsPolicy,
    );

    assert_eq!(result, "");
}
