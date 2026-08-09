# css-sanitizer

Policy-driven CSS sanitization on top of [lightningcss](https://lightningcss.dev/).

This crate exposes `lightningcss` directly and lets you sanitize rules, selectors,
properties, and descriptors through a custom policy trait. The policy interface is
**deny-by-default**: an empty policy removes everything, and the engine independently
enforces a value/resource guard so exfiltration vectors such as `url()`, `src()`,
string-based image functions, `@import`, `var()`, and `env()` cannot leak unless the
policy opts into them. Generic functions in raw/unparsed values also fail closed
unless explicitly allowed. A built-in `StrictPolicy` allowlist is provided as a
safe starting point.

## Install

```toml
[dependencies]
css-sanitizer = "0.4.0"
```

## Example

```bash
cargo run --example sanitize_strings
```

## Core model

- `StrictPolicy` is the built-in allowlist policy; use it as a safe default.
- `CssSanitizationPolicy` is the extension point for custom policies.
- `clean_declaration_list_with_policy()` and `clean_stylesheet_with_policy()` parse, sanitize, and serialize strings.
- `sanitize_declaration_block_ast()` and `sanitize_stylesheet_ast()` mutate parsed `lightningcss` ASTs in place.
- The `*_with_options` variants accept a `SanitizeOptions` (recursion depth cap, value-guard toggle).
- `lightningcss` is re-exported so callers can work against the same AST types.

Every hook that admits content is **deny-by-default** (`NodeAction::Drop`), and the
value-guard hooks (`check_resource`, `check_url`, `check_function`, variable and
environment-variable hooks, and `check_token`) default to `ValueAction::Deny`.
Forgetting a hook fails safe by over-removing rather than leaking content.

## Quick start (built-in strict policy)

```rust
use css_sanitizer::{clean_stylesheet_with_policy, StrictPolicy};

let safe = clean_stylesheet_with_policy(
    "@import url('evil.css'); .card { color: red; position: fixed }",
    &StrictPolicy::new().allow_properties(&["color"]),
);

assert!(!safe.contains("@import"));
assert!(safe.contains("color"));
assert!(!safe.contains("position"));
```

## Custom policy

Because the trait is deny-by-default, a custom policy must allow each kind of node it
wants to keep — including selectors.

```rust
use css_sanitizer::{
    clean_stylesheet_with_policy, CssSanitizationPolicy, NodeAction, PropertyContext,
    RuleContext, SelectorContext,
};
use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::selector::SelectorList;

struct ColorOnly;

impl CssSanitizationPolicy for ColorOnly {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        match rule {
            CssRule::Style(_) => NodeAction::Continue,
            _ => NodeAction::Drop,
        }
    }

    fn visit_selector_list(&self, _s: &mut SelectorList<'_>, _c: SelectorContext) -> NodeAction {
        NodeAction::Continue
    }

    fn visit_property(&self, property: &mut Property<'_>, _ctx: PropertyContext) -> NodeAction {
        if property.property_id().name() == "color" {
            NodeAction::Continue
        } else {
            NodeAction::Drop
        }
    }
}

let safe = clean_stylesheet_with_policy(".card { color: red; position: fixed }", &ColorOnly);
assert!(safe.contains("color"));
assert!(!safe.contains("position"));
```

## In-place AST sanitization

```rust
use css_sanitizer::{sanitize_stylesheet_ast, StrictPolicy};
use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};

let mut stylesheet =
    StyleSheet::parse("@import url('evil.css'); .card { color: blue }", ParserOptions::default())
        .expect("stylesheet should parse");

sanitize_stylesheet_ast(&mut stylesheet, &StrictPolicy::new().allow_properties(&["color"]));

let output = stylesheet
    .to_css(Default::default())
    .expect("stylesheet should serialize")
    .code;

assert!(!output.contains("@import"));
assert!(output.contains(".card"));
```

## What the sanitizer walks

The built-in walker already handles:

- full stylesheet rule lists
- nested style rules
- `@media`, `@supports`, `@container`, `@scope`, `@starting-style`
- `@keyframes`
- `@font-face`
- `@font-palette-values`
- `@font-feature-values` and its sub-rules
- `@page` and page margin rules
- `@counter-style`
- `@viewport`
- `@position-try`
- selector lists on style-like rules, including `@scope` prelude (`scope_start`/`scope_end`) selectors
- normal properties and `!important` declarations
- descriptor-style nodes exposed by `lightningcss`
- `@container` style query conditions and `@property` `initial-value`
- a value/resource guard over every kept declaration and descriptor that reaches `url()`, `src()`, string-based `image()`/`image-set()`, `var()`, `env()`, generic functions, and raw tokens
- explicit resource checks for URLs in kept `@import` rules

Empty rules created by filtering are removed during traversal. Rules nested deeper than
`SanitizeOptions::max_depth` (default 256) are dropped to bound the sanitizer's recursion.

> **Note on deeply nested input:** `max_depth` bounds the sanitizer's own traversal, not
> lightningcss's parser, which recurses before sanitization runs. Pathologically nested
> untrusted input can overflow the stack during parsing; bound input size upstream if that
> is a concern.

## API surface

- `StrictPolicy`
- `CssSanitizationPolicy`
- `NodeAction`, `ValueAction`, `ResourceKind`, `ResourceRef`
- `RuleContext`, `SelectorContext`, `PropertyContext`, `DescriptorContext`, `ValueContext`, `ValueLocation`
- `SanitizeOptions`
- `sanitize_declaration_block_ast()` / `sanitize_declaration_block_ast_with_options()`
- `sanitize_stylesheet_ast()` / `sanitize_stylesheet_ast_with_options()`
- `clean_declaration_list_with_policy()` / `clean_declaration_list_with_policy_and_options()`
- `clean_stylesheet_with_policy()` / `clean_stylesheet_with_policy_and_options()`
- `pub use lightningcss`

## Security notes

- The policy is deny-by-default: anything not explicitly allowed is removed, so forgetting a hook fails safe.
- The engine-enforced guard means syntactically identifiable fetchable resources cannot leak unless the policy allows them, regardless of which structural hooks are overridden. It covers typed/raw `url()`, CSS Values `src()`, string-based `image()`/`image-set()`, `@import`, and dynamic references inside recognized resource wrappers, plus nested `var()`/`env()` fallbacks.
- `StrictPolicy::allow_url()` is intentionally broad: it admits all recognized resource kinds. Implement `check_resource` in a custom policy for scheme, origin, or resource-kind restrictions.
- For compatibility, a custom `check_url()` override remains authoritative for typed `url()` values. New custom policies should centralize resource decisions in `check_resource()`; existing policies can delegate their `check_url()` implementation to it when migrating.
- Generic functions in raw/unparsed values are denied by default. `StrictPolicy::allow_functions()` admits named non-resource functions; known resource functions remain controlled by `allow_url()`.
- Any unresolved generic function nested inside generic `image()`/`image-set()` syntax is treated as a dynamic resource candidate. This covers current and future arbitrary-substitution functions such as `if()` and dashed custom functions; both the function and resource permissions are required.
- Dashed custom function names in `allow_functions()` are matched case-sensitively, while standard CSS function names are matched ASCII case-insensitively.
- `allow_functions()` trusts the computed result of an explicitly allowed arbitrary-substitution function when it appears outside a recognized resource wrapper. For example, an external `@function --asset()` can make `background-image: --asset()` resolve to a URL without a syntactic URL in the sanitized fragment. Only allow such functions when their external definitions/results are trusted.
- Case/escape variants of `var()` and `env()` that lightningcss leaves generic are reserved and still use their dedicated deny-by-default hooks; they cannot be enabled through `allow_functions()`.
- Existing custom policies that want to preserve those case/escape variants must implement `check_unparsed_variable()` and `check_unparsed_environment_variable()` in addition to their typed variable hooks; otherwise the variants are safely removed.
- Selector scoping and `!important` are policy decisions. `StrictPolicy` allows parsed selectors by default and denies `!important` unless opted in, so callers that need selector isolation must implement selector hooks or scope CSS outside this crate.
- `NodeAction::Skip` is retained for API compatibility, but it no longer bypasses recursive sanitization or an enabled value/resource guard. `SanitizeOptions::with_value_guard(false)` is the explicit, dangerous opt-out.
- `ResourceRef::value` is parser-decoded but unresolved and may be relative; `None` also covers resources that cannot be recovered statically. Origin-restricting custom policies must resolve literals against their own trusted base URL. `ResourceKind::Url` also includes image-set options that lightningcss normalizes to typed URLs.
- `var(--x)` and externally defined arbitrary-substitution/custom functions cannot be resolved statically across cascade and stylesheet boundaries unless your own policy or environment model provides that information.

## Publishing

```bash
cargo xtask publish-dry
cargo xtask publish
```

## Benchmarking

```bash
cargo bench --bench sanitize
```

The Criterion benchmark suite measures:

- declaration-list parse + sanitize + serialize
- stylesheet parse + sanitize + serialize
- stylesheet AST API parse + sanitize
- `lightningcss` parse/serialize round-trips as a baseline next to sanitizer runs

The built-in fixtures are synthetic but intentionally stress nested rules, descriptor rules,
URLs, `var()`, and pruning behavior. If you later want real-world corpora, prefer fetching
official distributed CSS from upstream projects during benchmarking rather than vendoring
large third-party CSS blobs into this repository.

## License

Apache-2.0
