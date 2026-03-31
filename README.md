# css-sanitizer

Policy-driven CSS sanitization on top of [lightningcss](https://lightningcss.dev/).

This crate exposes `lightningcss` directly and lets you sanitize rules, selectors,
properties, and descriptors through a custom policy trait. It is an AST policy engine,
not a built-in preset sanitizer.

## Install

```toml
[dependencies]
css-sanitizer = "0.1.2"
```

## Example

```bash
cargo run --example sanitize_strings
```

## Core model

- `CssSanitizationPolicy` is the main extension point.
- `clean_declaration_list_with_policy()` and `clean_stylesheet_with_policy()` parse, sanitize, and serialize strings.
- `sanitize_declaration_block_ast()` and `sanitize_stylesheet_ast()` mutate parsed `lightningcss` ASTs in place.
- `lightningcss` is re-exported so callers can work against the same AST types.

Default trait methods are fail-open. If you want a strict sanitizer, your policy must
explicitly return `NodeAction::Drop` for anything you do not want to keep.

## Quick start

```rust
use css_sanitizer::{
    clean_stylesheet_with_policy, CssSanitizationPolicy, NodeAction, PropertyContext,
    RuleContext,
};
use css_sanitizer::lightningcss::rules::CssRule;

struct StyleColorOnly;

impl CssSanitizationPolicy for StyleColorOnly {
    fn visit_rule(
        &self,
        rule: &mut CssRule<'_>,
        _ctx: RuleContext,
    ) -> NodeAction {
        match rule {
            CssRule::Style(_) => NodeAction::Continue,
            _ => NodeAction::Drop,
        }
    }

    fn visit_property(
        &self,
        property: &mut css_sanitizer::lightningcss::properties::Property<'_>,
        _ctx: PropertyContext,
    ) -> NodeAction {
        if property.property_id().name() == "color" {
            NodeAction::Continue
        } else {
            NodeAction::Drop
        }
    }
}

let safe = clean_stylesheet_with_policy(
    "@import url('evil.css'); .card { color: red; position: fixed }",
    &StyleColorOnly,
);

assert!(!safe.contains("@import"));
assert!(safe.contains("color"));
assert!(!safe.contains("position"));
```

## In-place AST sanitization

```rust
use css_sanitizer::{
    sanitize_stylesheet_ast, CssSanitizationPolicy, NodeAction, RuleContext,
};
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};

struct NoImports;

impl CssSanitizationPolicy for NoImports {
    fn visit_rule(
        &self,
        rule: &mut CssRule<'_>,
        _ctx: RuleContext,
    ) -> NodeAction {
        if matches!(rule, CssRule::Import(_)) {
            NodeAction::Drop
        } else {
            NodeAction::Continue
        }
    }
}

let mut stylesheet =
    StyleSheet::parse("@import url('evil.css'); .card { color: blue }", ParserOptions::default())
        .expect("stylesheet should parse");

sanitize_stylesheet_ast(&mut stylesheet, &NoImports);

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
- selector lists on style-like rules
- normal properties and `!important` declarations
- descriptor-style nodes exposed by `lightningcss`

Empty rules created by filtering are removed during traversal.

## API surface

- `CssSanitizationPolicy`
- `NodeAction`
- `RuleContext`
- `SelectorContext`
- `PropertyContext`
- `DescriptorContext`
- `sanitize_declaration_block_ast()`
- `sanitize_stylesheet_ast()`
- `clean_declaration_list_with_policy()`
- `clean_stylesheet_with_policy()`
- `pub use lightningcss`

## Security notes

- This crate does not ship a safe default policy.
- Selector scoping, `@import`, remote URLs, `!important`, `var()`, and unknown rules are all policy decisions.
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
