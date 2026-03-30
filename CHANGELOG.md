# Changelog

All notable changes to css-sanitizer will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
