# css-sanitizer

A Rust library for sanitizing untrusted CSS through an allowlist policy. Powered by [lightningcss](https://github.com/nickel-org/nickel.rs) for robust CSS parsing and AST manipulation.

Think of it as what [ammonia](https://github.com/rust-ammonia/ammonia) does for HTML, but for CSS: parse the input, keep only what you explicitly allow, strip everything else.

## Features

- **Allowlist-based** — nothing passes unless explicitly permitted
- **Inline styles** — sanitize `style=""` declaration lists
- **Full stylesheets** — sanitize `<style>` blocks with rule-level and at-rule filtering
- **Value restrictions** — limit specific properties to a set of allowed values (e.g., `display` → `block | flex | none`)
- **URL blocking** — strip all `url()` references by default (opt-in to allow)
- **XSS protection** — blocks `expression()`, `-moz-binding`, and other legacy attack vectors via AST inspection
- **lightningcss-powered** — no regex hacks; everything is parsed, filtered, and serialized through a proper CSS parser

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
css-sanitizer = "0.1"
```

### Sanitize inline styles

```rust
use css_sanitizer::Builder;

let safe = Builder::new()
    .add_allowed_properties(["color", "font-size", "display"])
    .add_property_values("display", ["block", "inline", "flex", "none"])
    .clean_declaration_list("color: red; position: fixed; display: flex");

assert_eq!(safe, "color: red; display: flex");
```

### Sanitize full stylesheets

```rust
use css_sanitizer::Builder;

let safe = Builder::new()
    .add_allowed_properties(["color", "background-color"])
    .add_allowed_at_rules(["media"])
    .clean_stylesheet(r#"
        @import url('evil.css');
        @font-face { font-family: Evil; src: url('evil.woff'); }
        @media (max-width: 768px) {
            .card { color: red; position: fixed; }
        }
        .header { background-color: #fff; z-index: 9999; }
    "#);

// @import and @font-face are removed
// position and z-index are stripped
// @media is kept (explicitly allowed)
// color and background-color pass through
```

## API

### `Builder`

The main entry point. Uses the builder pattern to configure your sanitization policy.

```rust
use css_sanitizer::Builder;

let mut builder = Builder::new();

// Allow specific CSS properties
builder.add_allowed_properties(["color", "font-size", "margin", "padding"]);

// Remove a property from the allowlist
builder.rm_allowed_properties(["padding"]);

// Restrict values for specific properties
builder.add_property_values("display", ["block", "inline", "flex", "none"]);

// Allow url() values (blocked by default)
builder.allow_urls(true);

// Allow specific at-rules in stylesheet mode
builder.add_allowed_at_rules(["media", "keyframes"]);

// Sanitize
let safe_inline = builder.clean_declaration_list("color: red; position: fixed");
let safe_sheet = builder.clean_stylesheet(".cls { color: red; position: fixed }");
```

### Property Allowlist

Only properties in the allowlist survive sanitization. Everything else is silently removed.

```rust
use css_sanitizer::Builder;

let safe = Builder::new()
    .add_allowed_properties(["color"])
    .clean_declaration_list("color: red; position: fixed; z-index: 999");

assert_eq!(safe, "color: red");
```

### Value Restrictions

For properties where only certain values should be allowed (like `display`, `overflow`, `text-align`), use `add_property_values`:

```rust
use css_sanitizer::Builder;

let safe = Builder::new()
    .add_allowed_properties(["display"])
    .add_property_values("display", ["block", "inline", "flex", "none"])
    .clean_declaration_list("display: grid");

assert_eq!(safe, ""); // grid is not in the allowed values
```

Properties **without** value restrictions accept any value:

```rust
use css_sanitizer::Builder;

let safe = Builder::new()
    .add_allowed_properties(["color"])
    .clean_declaration_list("color: #ff0000");

assert_eq!(safe, "color: #f00"); // any color value is fine
```

### URL Blocking

By default, any property containing `url()` is removed. This prevents loading external resources.

```rust
use css_sanitizer::Builder;

let safe = Builder::new()
    .add_allowed_properties(["background-image"])
    .clean_declaration_list("background-image: url('http://evil.com/tracker.png')");

assert_eq!(safe, ""); // url() blocked by default
```

Enable URLs when you trust the source or have additional validation:

```rust
use css_sanitizer::Builder;

let safe = Builder::new()
    .add_allowed_properties(["background-image"])
    .allow_urls(true)
    .clean_declaration_list("background-image: url('https://cdn.example.com/bg.png')");

assert!(safe.contains("url(")); // allowed
```

### At-Rule Filtering (Stylesheet Mode)

When sanitizing full stylesheets, at-rules (`@media`, `@import`, `@font-face`, etc.) are blocked by default. Explicitly allow the ones you need:

```rust
use css_sanitizer::Builder;

let safe = Builder::new()
    .add_allowed_properties(["color"])
    .add_allowed_at_rules(["media"])  // only @media allowed
    .clean_stylesheet(r#"
        @import url('evil.css');
        @media (max-width: 768px) { .foo { color: red; } }
    "#);

// @import is removed, @media is kept
assert!(!safe.contains("@import"));
assert!(safe.contains("@media"));
```

### XSS Vector Protection

The sanitizer blocks known CSS-based attack vectors through AST-level inspection:

- **`expression()`** — IE legacy scripting in CSS values
- **`-moz-binding`** — Firefox XBL binding injection
- **`url()`** — external resource loading (blocked by default)
- **Disallowed at-rules** — `@import`, `@font-face`, etc. (blocked by default)

All detection is done via the lightningcss AST (visitor pattern for URLs, token list inspection for dangerous functions) — not string matching or regex.

## How It Works

1. **Parse** — CSS input is parsed into an AST by lightningcss with error recovery enabled
2. **Filter properties** — each declaration is checked:
   - Is the property name in the allowlist?
   - Does the value contain dangerous functions (`expression()`, etc.)?
   - Does the value contain `url()` (checked via lightningcss visitor)?
   - If value restrictions exist for this property, does the serialized value match?
3. **Filter rules** (stylesheet mode) — at-rules are checked against the at-rule allowlist; style rules with no surviving declarations are removed
4. **Serialize** — the filtered AST is serialized back to a CSS string

## Publishing

```bash
# Dry-run (validate without uploading)
cargo xtask publish-dry

# Publish to crates.io
cargo xtask publish
```

## License

Apache-2.0