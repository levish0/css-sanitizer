#[cfg(test)]
mod tests {
    use css_sanitizer::Builder;

    // ── Declaration list: basic property filtering ──────────────────

    #[test]
    fn allows_whitelisted_property() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_declaration_list("color: red");
        assert_eq!(result, "color: red");
    }

    #[test]
    fn strips_non_whitelisted_property() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_declaration_list("color: red; position: fixed");
        assert_eq!(result, "color: red");
    }

    #[test]
    fn strips_all_when_no_properties_allowed() {
        let result = Builder::new().clean_declaration_list("color: red; font-size: 14px");
        assert_eq!(result, "");
    }

    #[test]
    fn allows_multiple_properties() {
        let result = Builder::new()
            .add_allowed_properties(["color", "font-size"])
            .clean_declaration_list("color: red; font-size: 14px; position: fixed");
        assert_eq!(result, "color: red; font-size: 14px");
    }

    #[test]
    fn handles_empty_input() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_declaration_list("");
        assert_eq!(result, "");
    }

    #[test]
    fn handles_malformed_css() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_declaration_list("color red; ;;;; font-size:");
        // Should not panic, may return empty or partial
        assert!(!result.contains("font-size"));
    }

    // ── Declaration list: !important ────────────────────────────────

    #[test]
    fn preserves_important() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_declaration_list("color: red !important");
        assert!(result.contains("color"));
        assert!(result.contains("!important"));
    }

    // ── Declaration list: url() blocking ────────────────────────────

    #[test]
    fn blocks_url_by_default() {
        let result = Builder::new()
            .add_allowed_properties(["background-image"])
            .clean_declaration_list("background-image: url('http://evil.com/img.png')");
        assert_eq!(result, "");
    }

    #[test]
    fn allows_url_when_enabled() {
        let result = Builder::new()
            .add_allowed_properties(["background-image"])
            .allow_urls(true)
            .clean_declaration_list("background-image: url('http://example.com/img.png')");
        assert!(result.contains("url("));
    }

    #[test]
    fn blocks_expression() {
        let result = Builder::new()
            .add_allowed_properties(["width"])
            .clean_declaration_list("width: expression(document.body.clientWidth)");
        assert_eq!(result, "");
    }

    // ── Declaration list: value restrictions ─────────────────────────

    #[test]
    fn allows_restricted_value() {
        let result = Builder::new()
            .add_allowed_properties(["display"])
            .add_property_values("display", ["block", "inline", "flex", "none"])
            .clean_declaration_list("display: flex");
        assert_eq!(result, "display: flex");
    }

    #[test]
    fn blocks_restricted_value() {
        let result = Builder::new()
            .add_allowed_properties(["display"])
            .add_property_values("display", ["block", "inline", "flex", "none"])
            .clean_declaration_list("display: grid");
        assert_eq!(result, "");
    }

    #[test]
    fn unrestricted_property_allows_any_value() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_declaration_list("color: #ff0000");
        assert!(result.contains("color"));
    }

    // ── Stylesheet: basic rule filtering ────────────────────────────

    #[test]
    fn stylesheet_keeps_style_rules() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_stylesheet(".foo { color: red; }");
        assert!(result.contains(".foo"));
        assert!(result.contains("color"));
    }

    #[test]
    fn stylesheet_strips_disallowed_properties() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_stylesheet(".foo { color: red; position: fixed; }");
        assert!(result.contains("color"));
        assert!(!result.contains("position"));
    }

    #[test]
    fn stylesheet_removes_empty_rules() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_stylesheet(".foo { position: fixed; }");
        assert!(!result.contains(".foo"));
    }

    // ── Stylesheet: at-rule filtering ───────────────────────────────

    #[test]
    fn stylesheet_blocks_import() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_stylesheet("@import url('evil.css'); .foo { color: red; }");
        assert!(!result.contains("@import"));
        assert!(result.contains("color"));
    }

    #[test]
    fn stylesheet_blocks_font_face() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_stylesheet(
                "@font-face { font-family: Evil; src: url('evil.woff'); } .foo { color: red; }",
            );
        assert!(!result.contains("@font-face"));
    }

    #[test]
    fn stylesheet_allows_media_when_permitted() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .add_allowed_at_rules(["media"])
            .clean_stylesheet("@media (max-width: 768px) { .foo { color: red; } }");
        assert!(result.contains("@media"));
        assert!(result.contains("color"));
    }

    #[test]
    fn stylesheet_blocks_media_by_default() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_stylesheet("@media (max-width: 768px) { .foo { color: red; } }");
        assert!(!result.contains("@media"));
    }

    // ── Stylesheet: url() in stylesheet ─────────────────────────────

    #[test]
    fn stylesheet_blocks_url_in_properties() {
        let result = Builder::new()
            .add_allowed_properties(["background-image", "color"])
            .clean_stylesheet(".foo { background-image: url('evil.png'); color: red; }");
        assert!(!result.contains("url("));
        assert!(result.contains("color"));
    }

    // ── Security: XSS vectors ───────────────────────────────────────

    #[test]
    fn blocks_moz_binding() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_declaration_list("-moz-binding: url('http://evil.com/xbl')");
        assert_eq!(result, "");
    }

    #[test]
    fn blocks_expression_in_any_property() {
        let result = Builder::new()
            .add_allowed_properties(["width", "height"])
            .clean_declaration_list("width: expression(alert(1))");
        assert_eq!(result, "");
    }

    // ── Builder: add/remove ─────────────────────────────────────────

    #[test]
    fn builder_add_remove_properties() {
        let mut builder = Builder::new();
        builder.add_allowed_properties(["color", "font-size", "width"]);
        builder.rm_allowed_properties(["width"]);

        let result = builder.clean_declaration_list("color: red; width: 100px");
        assert!(result.contains("color"));
        assert!(!result.contains("width"));
    }

    #[test]
    fn builder_clone_properties() {
        let mut builder = Builder::new();
        builder.add_allowed_properties(["color", "font-size"]);
        let props = builder.clone_allowed_properties();
        assert!(props.contains("color"));
        assert!(props.contains("font-size"));
        assert_eq!(props.len(), 2);
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[test]
    fn handles_custom_properties() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_declaration_list("--custom-color: red; color: blue");
        assert!(!result.contains("--custom-color"));
        assert!(result.contains("color"));
    }

    #[test]
    fn handles_vendor_prefixed() {
        let result = Builder::new()
            .add_allowed_properties(["color"])
            .clean_declaration_list("-webkit-transform: rotate(45deg); color: red");
        assert!(!result.contains("-webkit-transform"));
        assert!(result.contains("color"));
    }
}
