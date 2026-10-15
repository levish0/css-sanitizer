<div align="center">

# css-sanitizer

Policy-driven sanitization of untrusted CSS on top of [lightningcss](https://lightningcss.dev/).

[![crates.io: css-sanitizer](https://img.shields.io/crates/v/css-sanitizer.svg?label=css-sanitizer)](https://crates.io/crates/css-sanitizer)
[![Documentation](https://docs.rs/css-sanitizer/badge.svg)](https://docs.rs/css-sanitizer)
[![Downloads](https://img.shields.io/crates/d/css-sanitizer.svg)](https://crates.io/crates/css-sanitizer)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-1.93.0+-orange.svg?logo=rust)](https://www.rust-lang.org/)

[![Build](https://github.com/levish0/css-sanitizer/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/levish0/css-sanitizer/actions/workflows/build.yml)
[![Check](https://github.com/levish0/css-sanitizer/actions/workflows/check.yml/badge.svg?branch=main)](https://github.com/levish0/css-sanitizer/actions/workflows/check.yml)
[![Test](https://github.com/levish0/css-sanitizer/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/levish0/css-sanitizer/actions/workflows/test.yml)

</div>

---

The crate deliberately stays close to the upstream AST. Custom policies receive
`lightningcss::rules::CssRule`, `Property`, `SelectorList`, and descriptor types directly.
The crate adds the security semantics that a parser does not provide: deny-by-default
decisions, parent/value context, use-specific resource checks, traversal invariants,
input/output limits, diagnostics, and explicit HTML output encoding.

## Install

```toml
[dependencies]
css-sanitizer = "0.6.0"
```

Version 0.6 is a breaking API update. Resource permissions require a validator, variable
permissions name their allowed properties, and budgets live in `SanitizeLimits`.
There are no compatibility aliases; see [the changelog](CHANGELOG.md) for migration details.

## Declaration-list quick start

```rust
use css_sanitizer::{StrictPolicy, sanitize_declaration_list};

let output = sanitize_declaration_list(
    "color: red; position: fixed",
    &StrictPolicy::new().allow_properties(&["color"]),
)?;

assert_eq!(output.css.as_str(), "color: red");
assert_eq!(output.report.dropped_declarations, 1);
# Ok::<(), css_sanitizer::SanitizeError>(())
```

String entry points return `Result<SanitizeOutput, SanitizeError>`. `SanitizeOutput`
contains a `SanitizedCss` value and a `SanitizeReport`; parse, serialization, and budget
failures are no longer collapsed into an empty string.

Recovery-mode parser warnings are also reported: `parse_warning_count` includes every
upstream warning, while `parse_warnings` contains the first 32 messages with one-based
line/column locations. Dropped-node counts describe policy decisions, not parser recovery.
Use `SanitizeOptions::with_parse_mode(ParseMode::Strict)` when recovery is inappropriate.

## Stylesheet quick start

Selectors are a separate capability. `StrictPolicy` does not apply an untrusted
stylesheet globally unless the caller explicitly accepts that risk.

```rust
use css_sanitizer::{RuleKind, StrictPolicy, sanitize_stylesheet};

let policy = StrictPolicy::new()
    .allow_unscoped_selectors()
    .allow_properties(&["color"])
    .allow_rules(&[RuleKind::Media]);

let output = sanitize_stylesheet(
    "@media all { .card { color: red; position: fixed } }",
    &policy,
)?;

assert!(output.css.as_str().contains("@media"));
assert!(output.css.as_str().contains("color"));
assert!(!output.css.as_str().contains("position"));
# Ok::<(), css_sanitizer::SanitizeError>(())
```

`allow_unscoped_selectors()` means exactly what it says. It does not rewrite selectors or
isolate them from the host document.

## Custom policy

`StrictPolicy` is a safe convenience preset, not the extension boundary. Implement
`CssPolicy` for full control. Hooks receive the original upstream AST, so a policy can
inspect or rewrite nodes without converting them into a second CSS model.

```rust
use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::selector::SelectorList;
use css_sanitizer::{
    CssPolicy, NodeDecision, PropertyContext, RuleContext, RuleKind, SelectorContext,
    sanitize_stylesheet,
};

struct ColorOnly;

impl CssPolicy for ColorOnly {
    fn rule(&self, _rule: &mut CssRule<'_>, context: RuleContext) -> NodeDecision {
        if context.kind == RuleKind::Style {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }

    fn selector(
        &self,
        _selectors: &mut SelectorList<'_>,
        _context: SelectorContext,
    ) -> NodeDecision {
        NodeDecision::Keep
    }

    fn property(
        &self,
        _property: &mut Property<'_>,
        context: PropertyContext<'_>,
    ) -> NodeDecision {
        if context.key.name() == "color" && !context.important {
            NodeDecision::Keep
        } else {
            NodeDecision::Drop
        }
    }
}

let output = sanitize_stylesheet(
    ".card { color: red; position: fixed }",
    &ColorOnly,
)?;

assert!(output.css.as_str().contains("color"));
assert!(!output.css.as_str().contains("position"));
# Ok::<(), css_sanitizer::SanitizeError>(())
```

All hooks that retain authored content default to denial. A policy that implements nothing
removes everything. Structural permission does not bypass the engine value/resource guard.

## Why both upstream AST types and crate-specific types exist

The crate does not duplicate the lightningcss AST.

- `CssRule`, `Property`, selectors, and descriptor nodes remain upstream types.
- `RuleKind` is a small, storable tag for preset configuration and parent context.
  `RuleKind::of()` exhaustively matches the upstream `CssRule` enum, forcing a review when
  an upstream upgrade adds a rule variant.
- `PropertyContext`, `DescriptorContext`, and `ValueContext` describe where a retained
  value came from. This parent information is not available from a leaf-only visitor hook.
- `ResourceSyntax` describes how a resource was written; `ResourceUse` describes what the
  browser may use it for.
- `ImportDecision::AllowPassthrough` names a distinct high-risk operation whose remote
  contents are not sanitized.

Callers that only need parsing or transformation can use lightningcss directly. This crate
is for callers that need a reusable security policy and complete sanitizer traversal.

## Resource and dynamic-value policy

The guard recognizes typed and raw/case-escaped forms of:

- `url()`
- CSS Values `src()`
- `image()` and `image-set()`
- `var()` and `env()`, including nested fallbacks
- arbitrary/generic functions left unparsed by lightningcss
- resource tokens inside unknown at-rules
- descriptor resources such as `@font-face src`

`ResourceUse` distinguishes image, font source, cursor, list-style image, generated
content, mask image, filter reference, SVG paint server, and other uses. A custom policy
also receives the exact `PropertyId`, descriptor kind, rule kind, location, nesting depth,
and `!important` state.

`StrictPolicy` exposes separate capabilities:

- `allow_resources(&[ResourceUse::...], validator)` for explicit URL/context validation
- `allow_variables(&["color", "width"])` for property-specific variable trust
- `allow_environment_variables(&["padding-top"])` for property-specific environment values
- `allow_functions(&["..."])`
- typed font-face, font-palette, and view-transition descriptor allowlists
- `allow_local_fonts()`
- `allow_page_margin_rules()`
- `allow_font_feature_values_subrules()`

Standard CSS names are normalized ASCII-case-insensitively. Custom properties and dashed
custom function names remain case-sensitive.

Resource strings are parser-decoded but unresolved. They may be relative and
`ResourceRef::value` may be `None` for a dynamic reference. A scheme/origin policy must
resolve literals against its own trusted base URL, and a fetch layer must re-check redirect
destinations.

`allow_resources()` requires a validator for the specified uses. Validators for other uses
are preserved; calling it again for the same use replaces that use's validator. For example,
using the `url` crate
(`url = "2.5"` in the application's dependencies):

```rust
use css_sanitizer::{ResourceUse, StrictPolicy, ValueDecision, sanitize_declaration_list};
use url::Url;

let trusted_origin = Url::parse("https://cdn.example.test")?.origin();
let policy = StrictPolicy::new()
    .allow_properties(&["color", "background-image"])
    .allow_variables(&["color"])
    .allow_resources(&[ResourceUse::Image], move |resource, _context| {
        let Some(url) = resource.value.and_then(|value| Url::parse(value).ok()) else {
            return ValueDecision::Deny;
        };
        if url.origin() == trusted_origin && url.username().is_empty() && url.password().is_none() {
            ValueDecision::Allow
        } else {
            ValueDecision::Deny
        }
    });

let output = sanitize_declaration_list(
    "color: var(--text-color); background-image: url('https://cdn.example.test/card.png')",
    &policy,
)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

This example permits only absolute URLs with the configured HTTPS origin. Relative and
dynamic references are rejected. If a validator resolves relative URLs, its trusted base
must match the base used by the consuming document; validation does not rewrite the URL.
Validators never enable other resource uses or `@import`, and cannot inspect redirects.
`dangerously_allow_unrestricted_resources()` explicitly trusts every URL scheme, origin,
and dynamic reference for the specified uses. It is intended for trusted transformation
workflows, not for untrusted CSS.

Dynamic substitution cannot be resolved across cascade boundaries. Allowing `var()`,
`env()`, or an externally defined custom function trusts values supplied outside the
sanitized fragment. Use `ValueContext` to deny dynamic substitution in resource-capable
properties when that trust is not appropriate.
The variable and environment-variable builders only enable substitution in the named
properties. They do not enable the properties themselves, descriptor substitutions, or
resources in fallbacks. They do not resolve or validate values supplied by the host cascade.

## Rules and descriptors

The walker handles every `CssRule` variant in lightningcss 1.0.0-alpha.72, including style
and nested rules, media/supports/container/scope/layer wrappers, keyframes, page rules and
page margins, font rules, counter styles, viewport, position try, property registration,
starting style, and view transitions.

Known typed rules are recursively sanitized. Unknown at-rules may only be retained by a
custom `CssPolicy`, and both prelude and block token lists still cross the value guard.
`StrictPolicy` cannot enable unknown, custom, ignored, or import categories through
`allow_rules()`. The default `CssRule::Custom` payload is not inspectable/serializable by
this sanitizer and is always removed.

Name-defining rules such as `@property`, `@layer`, `@keyframes`, `@font-face`, and
`@counter-style` can affect host CSS. They are denied by the preset unless their typed rule
capability is explicitly enabled. This crate does not automatically namespace definitions
and references.

## Import is deliberately separate

A general resource permission never enables `@import`.

```rust
use css_sanitizer::{ResourceUse, StrictPolicy, ValueDecision};

let images_only = StrictPolicy::new()
    .allow_resources(&[ResourceUse::Image], |resource, _| {
        if resource.value == Some("https://cdn.example.test/logo.png") {
            ValueDecision::Allow
        } else {
            ValueDecision::Deny
        }
    });

let trusted_passthrough = StrictPolicy::new()
    .dangerously_allow_passthrough_imports();
```

Passthrough preserves the browser import unchanged. It does not fetch, inspect, limit, or
sanitize the imported stylesheet. A secure recursive-import implementation must live in a
fetching layer that controls origins, redirects, byte budgets, cycles, and relative URL
resolution.

## Parse and traversal limits

All budgets are configured together. String input budgets are enforced before calling
lightningcss, the rule budget during AST traversal, and the output budget after serialization:

- input: 1 MiB
- delimiter nesting: 32
- output: 1 MiB
- nested rule-list depth: 128 (root rules have depth 0)

```rust
use css_sanitizer::{ParseMode, SanitizeLimits, SanitizeOptions};

let options = SanitizeOptions::default()
    .with_limits(
        SanitizeLimits::default()
            .with_max_input_bytes(64 * 1024)
            .with_max_nesting_depth(16)
            .with_max_output_bytes(64 * 1024)
            .with_max_rule_depth(64),
    )
    .with_parse_mode(ParseMode::Strict);
```

The pre-parser guard uses the same CSS tokenizer as lightningcss, walks tokens iteratively,
and tracks matching delimiter kinds. Bad strings, bad URLs, CSS escapes, and mismatched
closing delimiters cannot hide nested blocks. The default depth of 32 is exercised with
known recursive shapes on a 1 MiB stack; increasing it requires validating the target
runtime's stack budget. This is an input guard, not a formal proof that an upstream parser has no
other resource-exhaustion path. lightningcss
[issue #1297](https://github.com/parcel-bundler/lightningcss/issues/1297) remains open for
alpha.72; high-availability systems should still consider process isolation for hostile
inputs.

Parser feature flags such as `ParserFlags::CUSTOM_MEDIA` can be enabled with
`SanitizeOptions::with_parser_flags()`. The default string API uses lightningcss error
recovery (`ParseMode::Recover`); `ParseMode::Strict` returns the first parse error instead.

`dangerously_disable_value_guard()` exists for trusted transformation workflows. It
disables the core resource/value invariant and should not be used for untrusted CSS.

## HTML output context

`SanitizedCss` is CSS text, not HTML source. Use `as_str()`/`into_string()` for CSSOM or
text-node APIs. Before interpolating into the raw-text contents of a `<style>` element, use:

```rust
# use css_sanitizer::{StrictPolicy, sanitize_stylesheet};
# let output = sanitize_stylesheet(
#     ".card { color: red }",
#     &StrictPolicy::new().allow_unscoped_selectors().allow_properties(&["color"]),
# )?;
let html_style_text = output.css.to_style_element_text();
# Ok::<(), css_sanitizer::SanitizeError>(())
```

The method removes case-insensitive literal HTML `</style` end-tag sequences while
preserving valid CSS. Prefer DOM text APIs such as `style.textContent` when available.

For declaration-list output interpolated into a **quoted** HTML `style` attribute, use
`output.css.to_style_attribute_value()`. It encodes quotes, ampersands, and angle brackets:

```rust
# use css_sanitizer::{StrictPolicy, sanitize_declaration_list};
let output = sanitize_declaration_list("color: red", &StrictPolicy::new().allow_properties(&["color"]))?;
let html = format!(r#"<span style="{}">Text</span>"#, output.css.to_style_attribute_value());
# Ok::<(), css_sanitizer::SanitizeError>(())
```

`SanitizedCss` does not implement `Display`: callers must choose the output context
explicitly. `as_str()` and `into_string()` return CSS without HTML encoding. Use these raw
CSS methods for `style.cssText`, `setAttribute("style", value)`, and other DOM APIs; do not
pass HTML-encoded output to those APIs or to an auto-escaping template a second time.

## AST entry points

Use the AST API when the caller needs custom lightningcss parser options or wants to
perform typed transformations before/after sanitization.

```rust
use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
use css_sanitizer::{StrictPolicy, sanitize_stylesheet_ast};

let mut stylesheet = StyleSheet::parse(
    ".card { color: blue; position: fixed }",
    ParserOptions::default(),
)?;

let report = sanitize_stylesheet_ast(
    &mut stylesheet,
    &StrictPolicy::new()
        .allow_unscoped_selectors()
        .allow_properties(&["color"]),
);

assert_eq!(report.dropped_declarations, 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The crate re-exports lightningcss as `css_sanitizer::lightningcss`.

Retained raw token lists are checked against their serialized CSS syntax so separate AST
tokens cannot combine into an unchecked resource function, declaration, or `!important`.
These checks use `SanitizeLimits` input and nesting budgets for the serialized value and fail closed. AST inputs still
need to obey upstream structural invariants, and the caller owns the original parse and
serialization budgets. Transformations made after sanitization require sanitization again.

## Public entry points

- `sanitize_declaration_list()` / `sanitize_declaration_list_with_options()`
- `sanitize_stylesheet()` / `sanitize_stylesheet_with_options()`
- `sanitize_declaration_block_ast()` / `sanitize_declaration_block_ast_with_options()`
- `sanitize_stylesheet_ast()` / `sanitize_stylesheet_ast_with_options()`

## Internal responsibilities

- `sanitize`: public string/AST entry points, parsing, diagnostics, and serialization.
- `ast_sanitizer`: structural policy application, recursive rule traversal, pruning, and counters.
- `value_guard`: resource, dynamic-value, and serialized-token validation.
- `limits`: iterative lexical input checks and serialized-output size checks.
- `policy`: the typed extension contract; `strict_policy`: the built-in allowlist implementation.
- `options`: parser mode and budgets; `output`: results, reports, errors, and HTML encoding.

## Development

```bash
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo bench --bench sanitize
cargo xtask publish-dry
```

`cargo xtask publish` performs the actual publication and should only be run deliberately.

## License

Apache-2.0
