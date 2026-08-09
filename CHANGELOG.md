# Changelog

All notable changes to css-sanitizer will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-08-09

### Changed (breaking)

- `NodeAction::Skip` no longer bypasses descendant sanitization or an enabled engine value/resource guard. It remains in the API for compatibility and now has the same traversal semantics as `Continue` after the policy hook returns.
- Generic functions in raw/unparsed values now fail closed. `StrictPolicy::allow_functions()` can explicitly admit named non-resource functions; legacy `expression()` remains unconditionally denied.
- Updated the public AST dependency to `lightningcss 1.0.0-alpha.72`. AST API signatures and the benchmark parser options were adapted to the upstream lifetime change.

### Added

- Added the shared deny-by-default `check_resource(ResourceRef, ValueContext)` hook and public `ResourceKind`/`ResourceRef` types. It covers typed/raw `url()`, CSS Values `src()`, string-based `image()`/`image-set()`, dynamic resource references, and `@import` URLs.
- Added `check_function()` and `StrictPolicy::allow_functions()` for explicit handling of generic functions that `lightningcss` leaves in raw/unparsed values.
- Added deny-by-default normalization hooks for case/escape variants of `var()` and `env()` that alpha.72 leaves as generic functions.
- Added `@position-try` rule and declaration traversal for the rule variant introduced by `lightningcss` alpha.72.

### Fixed

- Closed semantic resource bypasses where `src("...")`, `image("...")`, string-based `image-set()`, and case/escape variants such as `URL("...")` were represented as generic functions rather than typed URLs by `lightningcss`.
- Reserved generic `VAR()`/`ENV()` variants for the existing variable/environment permissions and treated them recursively as dynamic resources inside image functions, preventing `allow_functions()` from bypassing resource checks.
- Treat unresolved generic functions inside generic `image()`/`image-set()` syntax as dynamic resource candidates, covering CSS Values 5 `if()` and dashed custom functions without a future function-name allowlist gap.
- Kept `@import` resource decisions inside the same engine-enforced guard as declaration resources. `@namespace` URI identifiers are no longer incorrectly treated as fetches.
- Unknown/custom at-rule categories cannot be enabled through `StrictPolicy::allow_rules()`.

### Notes

- `StrictPolicy::allow_url()` now admits every recognized resource kind. Custom policies should use `check_resource()` for origin-, scheme-, or kind-specific decisions.
- Existing custom `check_url()` overrides remain authoritative for typed URLs. Remove or delegate that override when migrating all resource decisions to `check_resource()`.
- Existing custom policies must implement the new unparsed variable/environment hooks if they want to preserve case/escape variants of `var()`/`env()`; the default behavior is safe removal.
- `ResourceRef::value` is decoded but unresolved and may be relative. `ResourceKind::Url` also represents image-set options that lightningcss normalizes into typed URL images; `ResourceKind::ImageSet` covers generic/unparsed string and dynamic forms.
- Dashed custom function allowlist entries are case-sensitive; standard CSS function entries remain ASCII case-insensitive.
- `allow_functions()` trusts the computed result of explicitly allowed arbitrary-substitution functions outside recognized resource wrappers, just as `allow_var()` trusts values supplied through external cascade boundaries. External `@function` definitions are not resolved by this crate.
- No input-size or parser-resource limit was added; `max_depth` continues to bound only sanitizer traversal.

## [0.3.0] - 2026-06-26

### Changed (breaking)

- **`CssSanitizationPolicy` is now deny-by-default.** Every hook that admits content (`visit_rule`, `visit_selector_list` and the selector hooks, `visit_property` and the per-location property hooks, and the descriptor hooks `visit_font_face_property`/`visit_font_palette_values_property`/`visit_view_transition_property`) now defaults to `NodeAction::Drop`. An empty policy removes everything; a policy must explicitly allow what it wants to keep. Hooks that only control descent into an already-allowed rule (`visit_page_rule`, `visit_counter_style_rule`, `visit_viewport_rule`, `visit_font_feature_values_rule`, page-margin and font-feature sub-rule hooks) still default to `NodeAction::Continue`. Migration: forgetting a hook now over-removes instead of leaking; use `StrictPolicy` or override the hooks you need.

### Added

- **`StrictPolicy`** — a built-in allowlist policy (`allow_properties`, `allow_rules`, `allow_values`, `allow_important`, `allow_url`, `allow_var`, `allow_env`) as a vetted safe entry point.
- **Engine-enforced value guard.** New deny-by-default hooks `check_url`, `check_variable`, `check_environment_variable`, and `check_token` are run by the engine (via lightningcss's `Visit` traversal) over every kept declaration and descriptor. Value-level exfiltration vectors — including `@font-face` `src`, `image-set()`, `var()`/`env()` fallbacks, and raw `url()` tokens recovered from malformed input — can no longer leak even if a structural hook is omitted.
- **`SanitizeOptions`** (`max_depth`, `enforce_value_guard`) plus `*_with_options` variants of all four entry points (`sanitize_stylesheet_ast_with_options`, `sanitize_declaration_block_ast_with_options`, `clean_stylesheet_with_policy_and_options`, `clean_declaration_list_with_policy_and_options`).
- **`visit_scope_selectors`** hook and `ValueAction`/`ValueContext`/`ValueLocation` types.
- Differential completeness test that uses lightningcss's `Visit` traversal as an oracle, plus regression tests for the fixes below.

### Fixed

- **`@scope` prelude selectors** (`scope_start`/`scope_end`) are now sanitized through the selector hooks; previously they bypassed all selector policies.
- **`@container` style query conditions** (`style(prop: value)`, including nested `not()`/`and`/`or`) are now value-guarded; previously a `url()` embedded in a container condition bypassed the value guard.
- **`@property` `initial-value`** is now value-guarded; previously a `url()` registered as a custom property's initial value (later fetched via `var()`) bypassed the value guard. The field is `#[skip_visit]` in lightningcss, so it is handled explicitly.
- **Descriptor value bypass**: value checks now apply uniformly to descriptors, so a url-blocking policy also covers `@font-face` `src`.
- **Unbounded recursion**: a configurable depth cap (`max_depth`, default 256) drops overly nested rules fail-closed to bound the sanitizer's own recursion. Note that lightningcss's parser recurses before the sanitizer and is not bound by this cap; bound untrusted input size upstream.

### Notes

- `NodeAction` and `ValueAction` are now `#[non_exhaustive]`.
- `ValueLocation` gains `ContainerCondition` and `PropertyInitialValue` variants.

## [0.1.4] - 2026-03-31

### Removed

- Removed the selector rewrite helper APIs and their supporting example and regression tests.

## [0.1.2] - 2026-03-31

### Added

- Added a Criterion benchmark suite covering declaration-list sanitization, stylesheet sanitization, AST API parse-and-sanitize runs, and `lightningcss` round-trip baselines.
- Added CI coverage that exercises the benchmark target in test mode so benchmark code keeps compiling and running.

### Changed

- Documented local benchmarking workflow and the synthetic benchmark fixture strategy in the README.
- Kept benchmark execution in CI at smoke-test level rather than using noisy GitHub-hosted runner timings as a performance gate.

## [0.1.1] - 2026-03-31

### Added

- Added `visit_font_feature_values_subrule()` so policies can filter `@font-feature-values` sub-rules directly.
- Added regression coverage for `@font-feature-values` sub-rule filtering and empty-rule pruning.

### Changed

- Sanitization now walks `@font-feature-values` sub-rules when `visit_font_feature_values_rule()` returns `NodeAction::Continue`.
- `clean_declaration_list_with_policy()` now returns an empty string when declaration serialization fails, matching stylesheet sanitization behavior.
- Internal filtering now uses in-place `retain` and `retain_mut` passes instead of rebuilding intermediate vectors.
- Updated README walker documentation to explicitly include `@font-feature-values` sub-rules.

## [0.1.0]

### Added

- Added an AST-first sanitization API built directly on top of `lightningcss`.
- Added public policy hooks for rules, selector lists, properties, and descriptor-style nodes through `CssSanitizationPolicy`.
- Added in-place AST sanitization entry points: `sanitize_declaration_block_ast()` and `sanitize_stylesheet_ast()`.
- Added string-based sanitization entry points that run custom policies: `clean_declaration_list_with_policy()` and `clean_stylesheet_with_policy()`.
- Added selector, declaration, stylesheet, and nested-function security regression coverage to the test suite.

### Changed

- Repositioned the crate as a policy engine rather than a builder-based allowlist sanitizer.
- Re-exported `lightningcss` so callers can implement policies against the same AST types used by the walker.
- Updated crate-level docs and README to describe the AST-first usage model and the fail-open default trait behavior.
- Reorganized integration tests into focused files for declaration policy, stylesheet policy, AST hook behavior, and function security.
- Pinned `lightningcss` to `=1.0.0-alpha.71` because the public API now exposes upstream AST types directly.

### Removed

- Removed the legacy `Builder` API and its allowlist-based compatibility layer.
