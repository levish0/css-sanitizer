//! Typed, policy-driven CSS sanitization on top of `lightningcss`.
//!
//! The engine is deny-by-default at both structural and value levels. A custom
//! [`CssPolicy`] receives typed rule, selector, property, descriptor, resource,
//! and dynamic-value context. [`StrictPolicy`] is only a convenience preset; it
//! is not required for full policy control.
//!
//! String entry points return [`Result`] plus a [`SanitizeReport`]. Parsing is
//! preceded by byte and nesting limits so pathological input is rejected before
//! recursive upstream parser paths run. [`SanitizedCss`] is CSS text, not HTML;
//! use [`SanitizedCss::to_style_element_text`] before interpolating into an HTML
//! `<style>` raw-text context, or [`SanitizedCss::to_style_attribute_value`] for a
//! quoted HTML `style` attribute containing a declaration list.
//!
//! ```rust
//! use css_sanitizer::{sanitize_declaration_list, StrictPolicy};
//!
//! let output = sanitize_declaration_list(
//!     "color: red; position: fixed",
//!     &StrictPolicy::new().allow_properties(&["color"]),
//! ).unwrap();
//!
//! assert!(output.css.as_str().contains("color"));
//! assert!(!output.css.as_str().contains("position"));
//! ```

mod ast_sanitizer;
mod limits;
mod options;
mod output;
mod policy;
mod sanitize;
mod strict_policy;
mod value_guard;

pub use lightningcss;
pub use options::{ParseMode, SanitizeLimits, SanitizeOptions};
pub use output::{ParseWarning, SanitizeError, SanitizeOutput, SanitizeReport, SanitizedCss};
pub use policy::{
    CssPolicy, DescriptorContext, DescriptorKind, DynamicValueKind, DynamicValueRef,
    FontFaceDescriptorKind, FontPaletteValuesDescriptorKind, ImportContext, ImportDecision,
    NodeDecision, PropertyContext, PropertyLocation, ResourceRef, ResourceSyntax, ResourceUse,
    RuleContext, RuleKind, SelectorContext, SelectorLocation, ValueContext, ValueDecision,
    ViewTransitionDescriptorKind,
};
pub use sanitize::{
    sanitize_declaration_block_ast, sanitize_declaration_block_ast_with_options,
    sanitize_declaration_list, sanitize_declaration_list_with_options, sanitize_stylesheet,
    sanitize_stylesheet_ast, sanitize_stylesheet_ast_with_options,
    sanitize_stylesheet_with_options,
};
pub use strict_policy::StrictPolicy;
