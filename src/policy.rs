use std::collections::{HashMap, HashSet};

/// Resolved sanitization policy.
#[derive(Debug, Clone)]
pub(crate) struct SanitizePolicy<'a> {
    pub allowed_properties: &'a HashSet<&'a str>,
    pub property_values: &'a HashMap<&'a str, HashSet<&'a str>>,
    pub allow_urls: bool,
    pub allowed_at_rules: &'a HashSet<&'a str>,
}

impl<'a> SanitizePolicy<'a> {
    pub fn is_property_allowed(&self, name: &str) -> bool {
        self.allowed_properties.contains(name)
    }
}
