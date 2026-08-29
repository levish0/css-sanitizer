//! Security regressions for the typed 0.5 policy API.

mod common;

use std::cell::Cell;

use common::{NoGlobalSelectors, declaration_css, style_policy, stylesheet_css};
use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::properties::custom::TokenOrValue;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::{
    CssPolicy, NodeDecision, ParseLimits, PropertyContext, PropertyLocation, ResourceRef,
    RuleContext, RuleKind, SanitizeError, SanitizeOptions, SelectorContext, ValueContext,
    ValueDecision, sanitize_declaration_list_with_options, sanitize_stylesheet,
    sanitize_stylesheet_with_options,
};

#[test]
fn scope_start_and_end_selectors_are_both_policed() {
    for css in [
        "@scope (html) to (.limit) { .card { color: red } }",
        "@scope (.root) to (html) { .card { color: red } }",
    ] {
        assert!(stylesheet_css(css, &NoGlobalSelectors).is_empty(), "{css}");
    }

    let safe = stylesheet_css(
        "@scope (.root) to (.limit) { .card { color: red } }",
        &NoGlobalSelectors,
    );
    assert!(safe.contains("color"));
}

#[test]
fn traversal_depth_cap_fails_closed_after_parsing() {
    let mut input = String::new();
    for _ in 0..8 {
        input.push_str("@media all {");
    }
    input.push_str(".card { color: red }");
    for _ in 0..8 {
        input.push('}');
    }

    let policy = style_policy(&["color"]).allow_rules(&[RuleKind::Media]);
    let output = sanitize_stylesheet_with_options(
        &input,
        &policy,
        SanitizeOptions::default().with_max_traversal_depth(3),
    )
    .expect("stylesheet should sanitize");
    assert!(!output.css.as_str().contains("color"));
    assert!(output.report.dropped_rules > 0);
}

#[test]
fn parser_nesting_limit_rejects_known_stack_overflow_shape_before_parse() {
    let input = format!(
        "{}a{}{{color:red}}",
        ":is(".repeat(2_000),
        ")".repeat(2_000)
    );
    let error = sanitize_stylesheet(&input, &style_policy(&["color"]))
        .expect_err("pathological selector must be rejected");
    assert_eq!(error, SanitizeError::NestingTooDeep { max: 128 });
}

#[test]
fn parser_limit_scanner_ignores_delimiters_in_strings_comments_and_escapes() {
    let input = r#".card { content: "(({{"; --raw: \(; /* ((( {{{ */ color: red }"#;
    let policy = style_policy(&["content", "--raw", "color"]);
    let output = sanitize_stylesheet(input, &policy).expect("safe delimiters should parse");
    assert!(output.css.as_str().contains("color"));
}

#[test]
fn input_and_output_byte_limits_return_typed_errors() {
    let input_error = sanitize_declaration_list_with_options(
        "color: red",
        &css_sanitizer::StrictPolicy::new().allow_properties(&["color"]),
        SanitizeOptions::default()
            .with_parse_limits(ParseLimits::default().with_max_input_bytes(2)),
    )
    .expect_err("input limit must be enforced");
    assert_eq!(
        input_error,
        SanitizeError::InputTooLarge { actual: 10, max: 2 }
    );

    let output_error = sanitize_declaration_list_with_options(
        "color: red",
        &css_sanitizer::StrictPolicy::new().allow_properties(&["color"]),
        SanitizeOptions::default()
            .with_parse_limits(ParseLimits::default().with_max_output_bytes(2)),
    )
    .expect_err("output limit must be enforced");
    assert!(matches!(
        output_error,
        SanitizeError::OutputTooLarge { max: 2, .. }
    ));
}

#[test]
fn strict_parsing_surfaces_invalid_css_instead_of_recovering() {
    let error = sanitize_stylesheet_with_options(
        ".card { color: red; broken }",
        &style_policy(&["color"]),
        SanitizeOptions::default().with_strict_parsing(),
    )
    .expect_err("strict parsing should reject the malformed declaration");
    assert!(matches!(error, SanitizeError::Parse(_)));
}

#[test]
fn container_style_queries_do_not_bypass_resource_guard() {
    for css in [
        "@container style(background: url('https://example.test/a.png')) { .card { color: red } }",
        "@container not (style(background: url('https://example.test/a.png'))) { .card { color: red } }",
        "@container style(color: red) and style(background: url('https://example.test/a.png')) { .card { color: red } }",
    ] {
        let policy = style_policy(&["color", "background"]).allow_rules(&[RuleKind::Container]);
        let output = stylesheet_css(css, &policy);
        assert!(!output.contains("example.test"), "{css} -> {output}");
    }
}

#[test]
fn property_registration_initial_value_is_guarded() {
    let css = stylesheet_css(
        "@property --asset { syntax: '<image>'; inherits: false; initial-value: url('https://example.test/a.png') }",
        &css_sanitizer::StrictPolicy::new().allow_rules(&[RuleKind::PropertyRegistration]),
    );
    assert!(css.is_empty());
}

#[test]
fn dangerous_guard_opt_out_is_explicit_and_effective() {
    let output = sanitize_declaration_list_with_options(
        "background-image: url('image.png')",
        &css_sanitizer::StrictPolicy::new().allow_properties(&["background-image"]),
        SanitizeOptions::default().dangerously_disable_value_guard(),
    )
    .expect("declaration should sanitize");
    assert!(output.css.as_str().contains("url("));
}

#[derive(Default)]
struct PositionContextRecorder {
    property_called: Cell<bool>,
    resource_location: Cell<Option<PropertyLocation>>,
}

impl CssPolicy for PositionContextRecorder {
    fn rule(&self, _rule: &mut CssRule<'_>, context: RuleContext) -> NodeDecision {
        if context.kind == RuleKind::PositionTry {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn property(&self, _property: &mut Property<'_>, context: PropertyContext<'_>) -> NodeDecision {
        self.property_called
            .set(context.location == PropertyLocation::PositionTry);
        NodeDecision::Keep
    }

    fn resource(&self, _resource: ResourceRef<'_>, context: &ValueContext<'_>) -> ValueDecision {
        self.resource_location.set(Some(context.location));
        ValueDecision::Allow
    }

    fn token(&self, _token: &TokenOrValue<'_>, _context: &ValueContext<'_>) -> ValueDecision {
        ValueDecision::Allow
    }
}

#[test]
fn position_try_uses_typed_property_and_value_context() {
    let policy = PositionContextRecorder::default();
    let css = stylesheet_css(
        "@position-try --fallback { background-image: url('fallback.png') }",
        &policy,
    );
    assert!(policy.property_called.get());
    assert_eq!(
        policy.resource_location.get(),
        Some(PropertyLocation::PositionTry)
    );
    assert!(css.contains("fallback.png"));
}

struct OpaqueRulePolicy {
    allow_resources: bool,
}

impl CssPolicy for OpaqueRulePolicy {
    fn rule(&self, _rule: &mut CssRule<'_>, context: RuleContext) -> NodeDecision {
        if context.kind == RuleKind::Unknown {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn resource(&self, _resource: ResourceRef<'_>, _context: &ValueContext<'_>) -> ValueDecision {
        if self.allow_resources {
            ValueDecision::Allow
        } else {
            ValueDecision::Deny
        }
    }

    fn token(&self, _token: &TokenOrValue<'_>, _context: &ValueContext<'_>) -> ValueDecision {
        ValueDecision::Allow
    }
}

#[test]
fn unknown_at_rule_payload_always_crosses_the_value_guard() {
    let safe = stylesheet_css(
        "@future-rule mode { payload: safe }",
        &OpaqueRulePolicy {
            allow_resources: false,
        },
    );
    assert!(safe.contains("@future-rule"));

    let denied = stylesheet_css(
        "@future-rule url('https://example.test/a') { payload: safe }",
        &OpaqueRulePolicy {
            allow_resources: false,
        },
    );
    assert!(denied.is_empty());

    let explicitly_allowed = stylesheet_css(
        "@future-rule url('https://example.test/a') { payload: safe }",
        &OpaqueRulePolicy {
            allow_resources: true,
        },
    );
    assert!(explicitly_allowed.contains("example.test"));
}

#[test]
fn strict_preset_cannot_enable_opaque_rules() {
    let css = stylesheet_css(
        "@future-rule mode { payload: safe }",
        &css_sanitizer::StrictPolicy::new().allow_rules(&[RuleKind::Unknown]),
    );
    assert!(css.is_empty());
}

#[test]
fn style_element_output_does_not_contain_an_html_end_tag() {
    let output = sanitize_stylesheet(
        r#".card { font-family: "</StYlE><script>ignored()</script>" }"#,
        &style_policy(&["font-family"]),
    )
    .expect("stylesheet should sanitize");
    let html_text = output.css.to_style_element_text();
    assert!(!html_text.to_ascii_lowercase().contains("</style"));
    assert!(html_text.contains(r"<\/StYlE"));
    sanitize_stylesheet(&html_text, &style_policy(&["font-family"]))
        .expect("HTML-safe output must remain valid CSS");
}

#[test]
fn malformed_declaration_list_cannot_escape_into_a_rule() {
    let css = declaration_css(
        "color: red; } .owned { background-image: url('https://example.test/a.png') }",
        &css_sanitizer::StrictPolicy::new().allow_properties(&["color", "background-image"]),
    );
    assert_eq!(css, "color: red");
}

#[test]
fn selector_policy_is_independent_from_property_and_resource_permissions() {
    struct DropEverySelector;

    impl CssPolicy for DropEverySelector {
        fn rule(&self, _rule: &mut CssRule<'_>, context: RuleContext) -> NodeDecision {
            if context.kind == RuleKind::Style {
                NodeDecision::Keep
            } else {
                NodeDecision::Drop
            }
        }

        fn selector(
            &self,
            _selectors: &mut css_sanitizer::lightningcss::selector::SelectorList<'_>,
            _context: SelectorContext,
        ) -> NodeDecision {
            NodeDecision::Drop
        }

        fn property(
            &self,
            _property: &mut Property<'_>,
            _context: PropertyContext<'_>,
        ) -> NodeDecision {
            NodeDecision::Keep
        }

        fn resource(
            &self,
            _resource: ResourceRef<'_>,
            _context: &ValueContext<'_>,
        ) -> ValueDecision {
            ValueDecision::Allow
        }
    }

    let css = stylesheet_css(
        "[data-secret] { background-image: url('https://example.test/a.png') }",
        &DropEverySelector,
    );
    assert!(css.is_empty());
}
