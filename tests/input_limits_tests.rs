use css_sanitizer::{
    SanitizeError, SanitizeLimits, SanitizeOptions, StrictPolicy,
    sanitize_declaration_list_with_options, sanitize_stylesheet, sanitize_stylesheet_with_options,
};

fn assert_nesting_rejected(input: &str, max: usize) {
    let result = sanitize_stylesheet_with_options(
        input,
        &StrictPolicy::new(),
        SanitizeOptions::default()
            .with_limits(SanitizeLimits::default().with_max_nesting_depth(max)),
    );
    assert_eq!(result.unwrap_err(), SanitizeError::NestingTooDeep { max });
}

#[test]
fn bad_strings_end_at_every_css_newline_before_nesting_is_counted() {
    for quote in ['\'', '"'] {
        for newline in ["\n", "\r", "\r\n", "\u{000c}"] {
            let input = format!(
                ".x{{content:{quote}{newline};}}{}a{{color:red}}{}",
                "@media all{".repeat(4),
                "}".repeat(4),
            );
            assert_nesting_rejected(&input, 2);
        }
    }
}

#[test]
fn mismatched_closing_delimiters_do_not_reduce_real_nesting() {
    for opener in ["@media all { ]; ", "@media all { ); ", ":is(]", ":is(}"] {
        assert_nesting_rejected(&format!("{}a{{color:red}}", opener.repeat(4)), 2);
    }
}

#[test]
fn bad_url_quotes_do_not_hide_following_blocks() {
    assert_nesting_rejected(
        ".x{background:url(bad\"url)} @media all{@media all{a{color:red}}}",
        2,
    );
}

#[test]
fn default_limits_reject_the_previously_crashing_input() {
    let input = format!(
        ".x{{content:\"\n;}}{}a{{color:red}}{}",
        "@media all{".repeat(2_000),
        "}".repeat(2_000),
    );
    assert_eq!(
        sanitize_stylesheet(&input, &StrictPolicy::new()).unwrap_err(),
        SanitizeError::NestingTooDeep { max: 32 },
    );
}

#[test]
fn default_budget_handles_known_recursive_shapes_on_a_one_mib_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let max = SanitizeLimits::default().max_nesting_depth;
            for input in [
                format!("{}a{}{{color:red}}", ":is(".repeat(max), ")".repeat(max)),
                format!("{}a{}{{color:red}}", ":not(".repeat(max), ")".repeat(max)),
                format!("{}color:red{}", "a{".repeat(max), "}".repeat(max)),
                format!(
                    "{}a{{color:red}}{}",
                    "@media all{".repeat(max - 1),
                    "}".repeat(max - 1)
                ),
                format!(
                    "{}a{{color:red}}{}",
                    "@supports (display:grid){".repeat(max - 1),
                    "}".repeat(max - 1)
                ),
            ] {
                sanitize_stylesheet(&input, &StrictPolicy::new()).unwrap();
            }
            for function in ["calc(", "min("] {
                let input = format!("width: {}1px{}", function.repeat(max), ")".repeat(max));
                sanitize_declaration_list_with_options(
                    &input,
                    &StrictPolicy::new(),
                    SanitizeOptions::default(),
                )
                .unwrap();
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn declaration_entry_point_uses_the_same_lexical_guard() {
    let result = sanitize_declaration_list_with_options(
        "content: \"\n; width: calc(calc(calc(1px)))",
        &StrictPolicy::new(),
        SanitizeOptions::default().with_limits(SanitizeLimits::default().with_max_nesting_depth(2)),
    );
    assert_eq!(
        result.unwrap_err(),
        SanitizeError::NestingTooDeep { max: 2 }
    );
}
