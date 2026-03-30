use crate::policy::SanitizePolicy;
use lightningcss::declaration::DeclarationBlock;
use lightningcss::printer::{Printer, PrinterOptions};
use lightningcss::properties::custom::{TokenList, TokenOrValue};
use lightningcss::properties::Property;
use lightningcss::rules::CssRule;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use lightningcss::traits::ToCss;
use lightningcss::values::url::Url;
use lightningcss::visit_types;
use lightningcss::visitor::{Visit, VisitTypes, Visitor};
use std::convert::Infallible;

/// Visitor that detects whether a Property contains any url() references.
struct UrlDetector {
    found: bool,
}

impl UrlDetector {
    fn new() -> Self {
        Self { found: false }
    }

    /// Returns true if the given property contains any url() values.
    fn has_url(prop: &Property<'_>) -> bool {
        let mut detector = Self::new();
        let mut prop_clone = prop.clone();
        let _ = prop_clone.visit(&mut detector);
        detector.found
    }
}

impl<'i> Visitor<'i> for UrlDetector {
    type Error = Infallible;

    fn visit_types(&self) -> VisitTypes {
        visit_types!(URLS)
    }

    fn visit_url(&mut self, _url: &mut Url<'i>) -> Result<(), Self::Error> {
        self.found = true;
        Ok(())
    }
}

/// Dangerous function names that must be blocked regardless of context.
const DANGEROUS_FUNCTIONS: &[&str] = &["expression"];

/// Checks if an Unparsed property's token list contains dangerous functions.
fn has_dangerous_function(tokens: &TokenList<'_>) -> bool {
    for token in &tokens.0 {
        if let TokenOrValue::Function(func) = token {
            let name = func.name.as_ref().to_ascii_lowercase();
            if DANGEROUS_FUNCTIONS.iter().any(|d| name == *d) {
                return true;
            }
            if has_dangerous_function(&func.arguments) {
                return true;
            }
        }
    }
    false
}

/// Serializes a Property's value (without the property name) to a string.
fn serialize_value(prop: &Property<'_>, important: bool) -> Option<String> {
    let full = prop
        .to_css_string(important, PrinterOptions::default())
        .ok()?;
    // Serialized form: "property-name: value" or "property-name: value !important"
    let colon_pos = full.find(':')?;
    let value = full[colon_pos + 1..].trim();
    // Strip !important suffix for value comparison
    let value = value
        .strip_suffix("!important")
        .map(|v| v.trim())
        .unwrap_or(value);
    Some(value.to_ascii_lowercase())
}

/// Checks if a single property is allowed by the policy.
fn is_property_allowed(prop: &Property<'_>, policy: &SanitizePolicy<'_>, important: bool) -> bool {
    let id = prop.property_id();
    let name = id.name();

    // 1. Property name must be in the allowlist
    if !policy.is_property_allowed(name) {
        return false;
    }

    // 2. Block dangerous functions in unparsed properties (expression(), etc.)
    if let Property::Unparsed(unparsed) = prop {
        if has_dangerous_function(&unparsed.value) {
            return false;
        }
    }

    // 3. Block url() via AST visitor
    if !policy.allow_urls && UrlDetector::has_url(prop) {
        return false;
    }

    // 3. Check value restrictions via serialization (if any)
    if let Some(allowed_values) = policy.property_values.get(name) {
        let Some(value) = serialize_value(prop, important) else {
            return false;
        };
        if !allowed_values.contains(value.as_str()) {
            return false;
        }
    }

    true
}

fn serialize_declaration_block(block: &DeclarationBlock<'_>) -> String {
    let mut output = String::new();
    let mut printer = Printer::new(&mut output, PrinterOptions::default());
    let _ = block.to_css(&mut printer);
    output
}

/// Sanitizes an inline style declaration list.
pub(crate) fn clean_declaration_list(input: &str, policy: &SanitizePolicy<'_>) -> String {
    let options = ParserOptions {
        error_recovery: true,
        ..ParserOptions::default()
    };

    let Ok(block) = DeclarationBlock::parse_string(input, options) else {
        return String::new();
    };

    let mut filtered = DeclarationBlock::new();

    for prop in &block.declarations {
        if is_property_allowed(prop, policy, false) {
            filtered.declarations.push(prop.clone());
        }
    }

    for prop in &block.important_declarations {
        if is_property_allowed(prop, policy, true) {
            filtered.important_declarations.push(prop.clone());
        }
    }

    if filtered.len() == 0 {
        return String::new();
    }

    serialize_declaration_block(&filtered)
}

/// Filters declarations within a DeclarationBlock, modifying it in place.
fn filter_declarations(block: &mut DeclarationBlock<'_>, policy: &SanitizePolicy<'_>) {
    block
        .declarations
        .retain(|prop| is_property_allowed(prop, policy, false));
    block
        .important_declarations
        .retain(|prop| is_property_allowed(prop, policy, true));
}

/// Checks if a CssRule is an allowed at-rule.
fn is_at_rule_allowed(rule: &CssRule<'_>, policy: &SanitizePolicy<'_>) -> bool {
    match rule {
        CssRule::Style(_) => true,
        CssRule::Media(_) => policy.allowed_at_rules.contains("media"),
        CssRule::Import(_) => policy.allowed_at_rules.contains("import"),
        CssRule::FontFace(_) => policy.allowed_at_rules.contains("font-face"),
        CssRule::Keyframes(_) => policy.allowed_at_rules.contains("keyframes"),
        CssRule::Supports(_) => policy.allowed_at_rules.contains("supports"),
        CssRule::Container(_) => policy.allowed_at_rules.contains("container"),
        CssRule::LayerStatement(_) | CssRule::LayerBlock(_) => {
            policy.allowed_at_rules.contains("layer")
        }
        CssRule::Namespace(_) => policy.allowed_at_rules.contains("namespace"),
        CssRule::Page(_) => policy.allowed_at_rules.contains("page"),
        CssRule::Property(_) => policy.allowed_at_rules.contains("property"),
        CssRule::CounterStyle(_) => policy.allowed_at_rules.contains("counter-style"),
        CssRule::Scope(_) => policy.allowed_at_rules.contains("scope"),
        CssRule::StartingStyle(_) => policy.allowed_at_rules.contains("starting-style"),
        CssRule::Nesting(_) => true,
        CssRule::Ignored => true,
        _ => false,
    }
}

/// Recursively sanitizes rules within a CssRuleList.
fn sanitize_rules(rules: &mut Vec<CssRule<'_>>, policy: &SanitizePolicy<'_>) {
    rules.retain(|rule| is_at_rule_allowed(rule, policy));

    for rule in rules.iter_mut() {
        match rule {
            CssRule::Style(style_rule) => {
                filter_declarations(&mut style_rule.declarations, policy);
                sanitize_rules(&mut style_rule.rules.0, policy);
            }
            CssRule::Media(media_rule) => {
                sanitize_rules(&mut media_rule.rules.0, policy);
            }
            CssRule::Supports(supports_rule) => {
                sanitize_rules(&mut supports_rule.rules.0, policy);
            }
            CssRule::Container(container_rule) => {
                sanitize_rules(&mut container_rule.rules.0, policy);
            }
            CssRule::LayerBlock(layer_rule) => {
                sanitize_rules(&mut layer_rule.rules.0, policy);
            }
            CssRule::Scope(scope_rule) => {
                sanitize_rules(&mut scope_rule.rules.0, policy);
            }
            CssRule::StartingStyle(starting_rule) => {
                sanitize_rules(&mut starting_rule.rules.0, policy);
            }
            CssRule::Nesting(nesting_rule) => {
                sanitize_rules(&mut nesting_rule.style.rules.0, policy);
                filter_declarations(&mut nesting_rule.style.declarations, policy);
            }
            _ => {}
        }
    }

    rules.retain(|rule| match rule {
        CssRule::Style(style_rule) => {
            style_rule.declarations.len() > 0 || !style_rule.rules.0.is_empty()
        }
        _ => true,
    });
}

/// Sanitizes a full CSS stylesheet.
pub(crate) fn clean_stylesheet(input: &str, policy: &SanitizePolicy<'_>) -> String {
    let options = ParserOptions {
        error_recovery: true,
        ..ParserOptions::default()
    };

    let Ok(mut stylesheet) = StyleSheet::parse(input, options) else {
        return String::new();
    };

    sanitize_rules(&mut stylesheet.rules.0, policy);

    match stylesheet.to_css(PrinterOptions::default()) {
        Ok(result) => result.code,
        Err(_) => String::new(),
    }
}
