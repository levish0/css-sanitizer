use std::collections::{HashMap, HashSet};

use crate::policy::SanitizePolicy;
use crate::sanitize;

/// A configurable CSS sanitizer builder.
///
/// Uses an allowlist approach: only explicitly allowed CSS properties and values
/// are kept; everything else is removed.
///
/// By default, nothing is allowed. Use the builder methods to configure what
/// CSS is permitted.
#[derive(Debug, Clone)]
pub struct Builder<'a> {
    allowed_properties: HashSet<&'a str>,
    property_values: HashMap<&'a str, HashSet<&'a str>>,
    allow_urls: bool,
    allowed_at_rules: HashSet<&'a str>,
}

impl<'a> Default for Builder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Builder<'a> {
    /// Creates a new builder with an empty allowlist.
    /// All CSS properties will be stripped until you add them.
    pub fn new() -> Self {
        Builder {
            allowed_properties: HashSet::new(),
            property_values: HashMap::new(),
            allow_urls: false,
            allowed_at_rules: HashSet::new(),
        }
    }

    // ── Property allowlist ──────────────────────────────────────────

    /// Sets the full set of allowed CSS properties, replacing any previous list.
    pub fn allowed_properties(&mut self, props: HashSet<&'a str>) -> &mut Self {
        self.allowed_properties = props;
        self
    }

    /// Adds properties to the allowlist.
    pub fn add_allowed_properties<I, S>(&mut self, iter: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<&'a str>,
    {
        for prop in iter {
            self.allowed_properties.insert(prop.into());
        }
        self
    }

    /// Removes properties from the allowlist.
    pub fn rm_allowed_properties<I, S>(&mut self, iter: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<&'a str>,
    {
        for prop in iter {
            self.allowed_properties.remove(prop.into());
        }
        self
    }

    /// Returns a clone of the current allowed properties set.
    pub fn clone_allowed_properties(&self) -> HashSet<&'a str> {
        self.allowed_properties.clone()
    }

    // ── Property value restrictions ─────────────────────────────────

    /// Sets allowed values for a specific property.
    /// If a property has value restrictions, only those values will be accepted.
    /// Properties without value restrictions accept any value.
    pub fn property_values(&mut self, property: &'a str, values: HashSet<&'a str>) -> &mut Self {
        self.property_values.insert(property, values);
        self
    }

    /// Adds allowed values for a specific property.
    pub fn add_property_values<I, S>(&mut self, property: &'a str, iter: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<&'a str>,
    {
        let entry = self.property_values.entry(property).or_default();
        for val in iter {
            entry.insert(val.into());
        }
        self
    }

    /// Removes value restrictions for a specific property entirely.
    pub fn rm_property_values(&mut self, property: &'a str) -> &mut Self {
        self.property_values.remove(property);
        self
    }

    // ── URL control ─────────────────────────────────────────────────

    /// Sets whether `url()` values are allowed in CSS properties.
    /// Default: `false` (all url() values are stripped).
    pub fn allow_urls(&mut self, allow: bool) -> &mut Self {
        self.allow_urls = allow;
        self
    }

    // ── At-rule control (stylesheet mode) ───────────────────────────

    /// Sets the allowed at-rules for stylesheet sanitization.
    /// Only these at-rules will be kept; all others are removed.
    /// Example: `["media", "keyframes"]`
    pub fn allowed_at_rules(&mut self, rules: HashSet<&'a str>) -> &mut Self {
        self.allowed_at_rules = rules;
        self
    }

    /// Adds at-rules to the allowlist.
    pub fn add_allowed_at_rules<I, S>(&mut self, iter: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<&'a str>,
    {
        for rule in iter {
            self.allowed_at_rules.insert(rule.into());
        }
        self
    }

    /// Removes at-rules from the allowlist.
    pub fn rm_allowed_at_rules<I, S>(&mut self, iter: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<&'a str>,
    {
        for rule in iter {
            self.allowed_at_rules.remove(rule.into());
        }
        self
    }

    // ── Sanitize methods ────────────────────────────────────────────

    fn policy(&self) -> SanitizePolicy<'_> {
        SanitizePolicy {
            allowed_properties: &self.allowed_properties,
            property_values: &self.property_values,
            allow_urls: self.allow_urls,
            allowed_at_rules: &self.allowed_at_rules,
        }
    }

    /// Sanitizes an inline style declaration list.
    ///
    /// Input: `"color: red; position: fixed; display: flex"`
    /// Output (if color and display allowed): `"color: red; display: flex"`
    pub fn clean_declaration_list(&self, input: &str) -> String {
        sanitize::clean_declaration_list(input, &self.policy())
    }

    /// Sanitizes a full CSS stylesheet.
    ///
    /// Removes disallowed at-rules, filters properties within style rules,
    /// and removes rules that become empty after filtering.
    pub fn clean_stylesheet(&self, input: &str) -> String {
        sanitize::clean_stylesheet(input, &self.policy())
    }
}
