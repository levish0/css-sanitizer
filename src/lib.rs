//! # css-sanitizer
//!
//! Policy-driven CSS sanitization on top of `lightningcss`.
//!
//! This crate exposes `lightningcss` and lets you sanitize parsed CSS AST nodes
//! directly through [`CssSanitizationPolicy`]. There is no built-in safe preset:
//! the safety properties come from the policy you implement.
//!
//! Default trait methods are fail-open and return
//! [`NodeAction::Continue`]. A strict sanitizer must explicitly drop the rules,
//! selectors, properties, and descriptors it does not want to keep.
//!
//! ## String API
//!
//! ```rust
//! use css_sanitizer::{
//!     clean_stylesheet_with_policy, CssSanitizationPolicy, NodeAction, PropertyContext,
//!     RuleContext, RuleKind,
//! };
//!
//! struct StyleColorOnly;
//!
//! impl CssSanitizationPolicy for StyleColorOnly {
//!     fn visit_rule(
//!         &self,
//!         _rule: &mut css_sanitizer::lightningcss::rules::CssRule<'_>,
//!         ctx: RuleContext,
//!     ) -> NodeAction {
//!         match ctx.kind {
//!             RuleKind::Style => NodeAction::Continue,
//!             _ => NodeAction::Drop,
//!         }
//!     }
//!
//!     fn visit_property(
//!         &self,
//!         property: &mut css_sanitizer::lightningcss::properties::Property<'_>,
//!         _ctx: PropertyContext,
//!     ) -> NodeAction {
//!         if property.property_id().name() == "color" {
//!             NodeAction::Continue
//!         } else {
//!             NodeAction::Drop
//!         }
//!     }
//! }
//!
//! let safe = clean_stylesheet_with_policy(
//!     "@import url('evil.css'); .card { color: red; position: fixed }",
//!     &StyleColorOnly,
//! );
//!
//! assert!(!safe.contains("@import"));
//! assert!(safe.contains("color"));
//! assert!(!safe.contains("position"));
//! ```
//!
//! ## AST API
//!
//! ```rust
//! use css_sanitizer::{
//!     sanitize_stylesheet_ast, CssSanitizationPolicy, NodeAction, RuleContext, RuleKind,
//! };
//! use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
//!
//! struct NoImports;
//!
//! impl CssSanitizationPolicy for NoImports {
//!     fn visit_rule(
//!         &self,
//!         _rule: &mut css_sanitizer::lightningcss::rules::CssRule<'_>,
//!         ctx: RuleContext,
//!     ) -> NodeAction {
//!         if ctx.kind == RuleKind::Import {
//!             NodeAction::Drop
//!         } else {
//!             NodeAction::Continue
//!         }
//!     }
//! }
//!
//! let mut stylesheet =
//!     StyleSheet::parse("@import url('evil.css'); .card { color: blue }", ParserOptions::default())
//!         .expect("stylesheet should parse");
//!
//! sanitize_stylesheet_ast(&mut stylesheet, &NoImports);
//!
//! let output = stylesheet
//!     .to_css(Default::default())
//!     .expect("stylesheet should serialize")
//!     .code;
//! assert!(!output.contains("@import"));
//! assert!(output.contains(".card"));
//! ```

mod policy;
mod sanitize;

pub use lightningcss;
pub use policy::{
    CssSanitizationPolicy, DeclarationOwner, DescriptorContext, DescriptorOwner, NodeAction,
    PropertyContext, RuleContext, RuleKind, SelectorContext,
};
pub use sanitize::{
    clean_declaration_list_with_policy, clean_stylesheet_with_policy,
    sanitize_declaration_block_ast, sanitize_stylesheet_ast,
};
