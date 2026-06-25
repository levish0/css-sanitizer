//! # css-sanitizer
//!
//! Policy-driven CSS sanitization on top of `lightningcss`.
//!
//! This crate exposes `lightningcss` and lets you sanitize parsed CSS AST nodes
//! directly through [`CssSanitizationPolicy`].
//!
//! The policy interface is **deny-by-default**: every hook that admits content
//! drops it unless the policy explicitly keeps it, and the engine independently
//! enforces a value guard so that exfiltration vectors such as `url()`, `var()`,
//! and `env()` can never leak — even through `@font-face` `src`, `var()`
//! fallbacks, or tokens recovered from malformed input — unless the policy opts
//! into them. Forgetting a hook fails safe by over-removing rather than leaking.
//!
//! Use [`StrictPolicy`] for a ready-made allowlist policy, or implement
//! [`CssSanitizationPolicy`] for full control.
//!
//! ## String API (built-in strict policy)
//!
//! ```rust
//! use css_sanitizer::{clean_stylesheet_with_policy, StrictPolicy};
//!
//! let safe = clean_stylesheet_with_policy(
//!     "@import url('evil.css'); .card { color: red; position: fixed }",
//!     &StrictPolicy::new().allow_properties(&["color"]),
//! );
//!
//! assert!(!safe.contains("@import"));
//! assert!(safe.contains("color"));
//! assert!(!safe.contains("position"));
//! ```
//!
//! ## Custom policy
//!
//! Because the trait is deny-by-default, a custom policy must allow each kind of
//! node it wants to keep — including selectors and value kinds.
//!
//! ```rust
//! use css_sanitizer::{
//!     clean_stylesheet_with_policy, CssSanitizationPolicy, NodeAction, PropertyContext,
//!     SelectorContext, ValueAction,
//! };
//! use css_sanitizer::lightningcss::properties::Property;
//! use css_sanitizer::lightningcss::rules::CssRule;
//! use css_sanitizer::lightningcss::selector::SelectorList;
//!
//! struct ColorOnly;
//!
//! impl CssSanitizationPolicy for ColorOnly {
//!     fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: css_sanitizer::RuleContext) -> NodeAction {
//!         match rule {
//!             CssRule::Style(_) => NodeAction::Continue,
//!             _ => NodeAction::Drop,
//!         }
//!     }
//!
//!     fn visit_selector_list(&self, _s: &mut SelectorList<'_>, _c: SelectorContext) -> NodeAction {
//!         NodeAction::Continue
//!     }
//!
//!     fn visit_property(&self, property: &mut Property<'_>, _ctx: PropertyContext) -> NodeAction {
//!         if property.property_id().name() == "color" {
//!             NodeAction::Continue
//!         } else {
//!             NodeAction::Drop
//!         }
//!     }
//! }
//!
//! let safe = clean_stylesheet_with_policy(".card { color: red; position: fixed }", &ColorOnly);
//! assert!(safe.contains("color"));
//! assert!(!safe.contains("position"));
//! ```
//!
//! ## AST API
//!
//! ```rust
//! use css_sanitizer::{sanitize_stylesheet_ast, StrictPolicy};
//! use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
//!
//! let mut stylesheet =
//!     StyleSheet::parse("@import url('evil.css'); .card { color: blue }", ParserOptions::default())
//!         .expect("stylesheet should parse");
//!
//! sanitize_stylesheet_ast(&mut stylesheet, &StrictPolicy::new().allow_properties(&["color"]));
//!
//! let output = stylesheet
//!     .to_css(Default::default())
//!     .expect("stylesheet should serialize")
//!     .code;
//! assert!(!output.contains("@import"));
//! assert!(output.contains(".card"));
//! ```

mod guard;
mod options;
mod policy;
mod preset;
mod sanitize;

pub use lightningcss;
pub use options::SanitizeOptions;
pub use policy::{
    CssSanitizationPolicy, DescriptorContext, NodeAction, PropertyContext, RuleContext,
    SelectorContext, ValueAction, ValueContext, ValueLocation,
};
pub use preset::StrictPolicy;
pub use sanitize::{
    clean_declaration_list_with_policy, clean_declaration_list_with_policy_and_options,
    clean_stylesheet_with_policy, clean_stylesheet_with_policy_and_options,
    sanitize_declaration_block_ast, sanitize_declaration_block_ast_with_options,
    sanitize_stylesheet_ast, sanitize_stylesheet_ast_with_options,
};
