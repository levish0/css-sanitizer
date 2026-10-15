use css_sanitizer::{StrictPolicy, sanitize_declaration_list, sanitize_stylesheet};

#[test]
fn recovered_errors_are_visible_and_diagnostic_details_are_bounded() {
    let policy = StrictPolicy::new().allow_properties(&["color"]);
    let output = sanitize_declaration_list("color: red;\nbroken", &policy).unwrap();
    assert_eq!(output.css.as_str(), "color: red");
    assert_eq!(output.report.dropped_declarations, 0);
    assert_eq!(output.report.parse_warning_count, 1);
    assert_eq!(output.report.parse_warnings[0].line, Some(2));
    assert!(output.report.parse_warnings[0].column.is_some());
    assert!(!output.report.parse_warnings[0].message.is_empty());

    let output = sanitize_declaration_list(&"broken;".repeat(40), &policy).unwrap();
    assert!(output.css.is_empty());
    assert_eq!(output.report.parse_warning_count, 40);
    assert_eq!(output.report.parse_warnings.len(), 32);

    let output = sanitize_stylesheet(
        ".x { color: red; broken }",
        &policy.allow_unscoped_selectors(),
    )
    .unwrap();
    assert_eq!(output.report.parse_warning_count, 1);
}

#[test]
fn style_attribute_output_encodes_quotes_and_html_metacharacters() {
    let output = sanitize_declaration_list(
        r#"font-family: "a&b'c<d>""#,
        &StrictPolicy::new().allow_properties(&["font-family"]),
    )
    .unwrap();
    assert_eq!(
        output.css.to_style_attribute_value(),
        "font-family: &quot;a&amp;b&#39;c&lt;d&gt;&quot;"
    );
    assert!(output.css.as_str().contains("a&b'c<d>"));
}
