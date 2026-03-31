# Changelog

All notable changes to css-sanitizer will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.4] - 2026-03-31

### Removed

- Removed the selector rewrite helper APIs and their supporting example and regression tests.

## [0.1.3] - 2026-03-31

### Added

- Added `rewrite_selector_classes()` and `rewrite_stylesheet_selector_classes()` to rewrite class selectors in parsed selector lists and stylesheets.
- Added regression coverage for class selector rewriting across nested selector functions, `:nth-child(... of S)`, nesting rules, and `@scope` selectors.
- Added a `rewrite_classes` example that demonstrates stylesheet-wide and selector-list class rewriting.

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
