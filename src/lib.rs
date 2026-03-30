//! # css-sanitizer
//!
//! A CSS sanitizer that filters untrusted CSS through an allowlist policy.
//! Parse the input with lightningcss, keep only what you explicitly allow,
//! strip everything else.
//!
//! ## Usage
//!
//! ```rust
//! use css_sanitizer::Builder;
//! use std::collections::{HashSet, HashMap};
//!
//! // Sanitize inline styles (declaration lists)
//! let safe = Builder::new()
//!     .add_allowed_properties(["color", "font-size", "display"])
//!     .add_property_values("display", ["block", "inline", "flex", "none"])
//!     .clean_declaration_list("color: red; position: fixed; display: flex");
//! assert_eq!(safe, "color: red; display: flex");
//!
//! // Sanitize full stylesheets
//! let safe = Builder::new()
//!     .add_allowed_properties(["color", "background-color"])
//!     .add_allowed_at_rules(["media"])
//!     .clean_stylesheet(".cls { color: red; position: fixed }");
//! assert_eq!(safe, ".cls {\n  color: red;\n}\n");
//! ```

mod builder;
mod policy;
mod sanitize;

pub use builder::Builder;
