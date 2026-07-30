# css-sanitizer

Policy-driven CSS sanitization on top of [lightningcss](https://lightningcss.dev/).

This crate exposes `lightningcss` directly and lets you sanitize rules, selectors,
properties, and descriptors through a custom policy trait. The policy interface is
**deny-by-default**: an empty policy removes everything, and the engine independently
enforces a value guard so exfiltration vectors such as `url()`, `var()`, and `env()`
cannot leak. Even through `@font-face` `src`, `image-set()`, `var()` fallbacks, or
tokens recovered from malformed input unless the policy opts into them. A built-in
`StrictPolicy` allowlist is provided as a safe starting point.

## Install

```toml
[dependencies]
css-sanitizer = "0.3.0"
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
value-guard hooks (`check_url`, `check_variable`, `check_environment_variable`,
`check_token`) default to `ValueAction::Deny`. Forgetting a hook fails safe by
over-removing rather than leaking content.

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
- selector lists on style-like rules, including `@scope` prelude (`scope_start`/`scope_end`) selectors
- normal properties and `!important` declarations
- descriptor-style nodes exposed by `lightningcss`
- `@container` style query conditions and `@property` `initial-value`
- a value guard over every kept declaration and descriptor that reaches `url()`, `var()`, `env()`, `image-set()` images, and raw tokens

Empty rules created by filtering are removed during traversal. Rules nested deeper than
`SanitizeOptions::max_depth` (default 256) are dropped to bound the sanitizer's recursion.

> **Note on deeply nested input:** `max_depth` bounds the sanitizer's own traversal, not
> lightningcss's parser, which recurses before sanitization runs. Pathologically nested
> untrusted input can overflow the stack during parsing; bound input size upstream if that
> is a concern.

## API surface

- `StrictPolicy`
- `CssSanitizationPolicy`
- `NodeAction`, `ValueAction`
- `RuleContext`, `SelectorContext`, `PropertyContext`, `DescriptorContext`, `ValueContext`, `ValueLocation`
- `SanitizeOptions`
- `sanitize_declaration_block_ast()` / `sanitize_declaration_block_ast_with_options()`
- `sanitize_stylesheet_ast()` / `sanitize_stylesheet_ast_with_options()`
- `clean_declaration_list_with_policy()` / `clean_declaration_list_with_policy_and_options()`
- `clean_stylesheet_with_policy()` / `clean_stylesheet_with_policy_and_options()`
- `pub use lightningcss`

## Security notes

- The policy is deny-by-default: anything not explicitly allowed is removed, so forgetting a hook fails safe.
- The engine-enforced value guard means `url()`, `var()`, and `env()` cannot leak unless the policy allows them, regardless of which structural hooks are overridden. This covers `@font-face` `src`, `image-set()`, `var()`/`env()` fallbacks, and raw `url()` tokens recovered from malformed input.
- Selector scoping, `@import`, remote URLs, and `!important` are policy decisions; `StrictPolicy` denies all of them unless opted in.
- `NodeAction::Skip` keeps a node but bypasses deeper sanitization (including the value guard) for its children; avoid it in a strict policy.
- `var(--x)` still cannot be resolved statically across external cascade boundaries unless your own policy or environment model provides that information.

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
