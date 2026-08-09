# 2026-08-09 lightningcss alpha.72 resource guard hardening

## Objective
- Adapt the sanitizer to the user-selected `lightningcss 1.0.0-alpha.72` and close semantic CSS resource-fetch paths that alpha.72 does not expose as typed `Url` AST nodes.
- Review the plan and completed implementation with multiple independent agents, while explicitly not adding input-size/token/rule resource limits.

## User-Approved Scope
- The user approved implementing the non-input-limit hardening after an independent plan review, followed by multi-agent revalidation.
- The user independently updated `Cargo.toml`/`Cargo.lock` to lightningcss alpha.72 and instructed agents to use that version. Preserve that change; the manifest uses `1.0.0-alpha.72` (not an exact `=` requirement) and the current lock resolves alpha.72.
- No package-version bump was requested. `css-sanitizer` remains `0.3.0`; the changes are recorded under `[Unreleased]` and are behaviorally breaking, so a future release should use a 0.4.x version unless the user decides otherwise.
- Explicitly excluded: new input byte/token/rule budgets. Existing `max_depth` behavior and parser-limit warning remain.

## Implementation Status
- Completed:
  - alpha.72 lifetime/API compatibility and `CssRule::PositionTry` traversal.
  - shared resource metadata/hook and semantic resource recognition.
  - fail-closed generic function handling, case/escape normalization gaps, Skip guard bypass removal, tests/docs/changelog.
  - independent plan, AST, API, test/doc, and adversarial browser/security reviews.
- Partial:
  - None within the approved scope.
- Not started:
  - Package 0.4.x version bump/release/publish.
  - Property-aware resolution of external custom properties/custom functions.

## Major Changes
- Files/modules:
  - `src/policy.rs`: added non-exhaustive `ResourceKind`/`ResourceRef`, `check_resource`, `check_function`, unparsed VAR/ENV hooks, `ValueLocation::{PositionTry, ImportRule}`, and `visit_position_try_property`. Documented unresolved URL/base-URL and legacy `check_url` precedence contracts.
  - `src/guard.rs`: recognizes raw/generic case-insensitive `url`, CSS Values `src`, string-based `image`/`image-set`/`-webkit-image-set`; rejects malformed/raw function tokens; reserves generic VAR/ENV variants; recursively checks fallbacks; treats unresolved functions inside generic image wrappers as dynamic resource candidates.
  - `src/sanitize.rs`: alpha.72 `StyleSheet<'_>` signatures, explicit `@import` resource check, `@position-try` property filtering/value guard/pruning, and `NodeAction::Skip` no longer bypasses descendant sanitization or an enabled guard.
  - `src/preset.rs`: `allow_functions`, resource-wide `allow_url`, fail-closed unknown/custom rules and generic functions, VAR/ENV normalization, raw function denial, dashed custom-function case-sensitive matching.
  - `src/lib.rs`/`src/options.rs`: public exports and updated security/guard documentation.
  - `benches/sanitize.rs`: alpha.72 `ParserOptions<'i>` compatibility.
  - `tests/security_audit_tests.rs`, `tests/regression_tests.rs`, `tests/oracle.rs`: semantic-resource metadata corpus, URL/SRC/IMAGE/IMAGE-SET/IMPORT attacks, VAR/ENV/if()/dashed custom-function cases, Skip regressions, PositionTry dedicated hooks, alpha.72 oracle corpus, and compatibility precedence.
  - `README.md`/`CHANGELOG.md`: guarantees, migration notes, trust boundaries, alpha.72, and `[Unreleased]` changes.
- API routes:
  - No HTTP/API routes. Public Rust policy API expanded as described above.
- DTO/repository/service/permission layers:
  - No application DTO/repository/service layers. Policy permission behavior changed: `allow_url()` covers all recognized resource kinds; `allow_functions()` covers explicitly named non-resource functions; dashed names are case-sensitive.
  - Existing custom `check_url()` overrides remain authoritative for typed URLs. New resource forms default-deny through `check_resource`; migrate old policies by delegating/removing `check_url` if centralized origin rules are desired.
- Worker/background jobs:
  - None. No worker or background-job behavior changed.
- Migrations/entities/constants:
  - No database migrations/entities. New public enums/contexts are `ResourceKind` and `ValueLocation` variants; `ResourceRef` is non-exhaustive.

## Validation
- Commands run:
  - `cargo test --workspace --all-targets`
  - `cargo test --workspace --doc`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps`
  - `cargo fmt --all -- --check`
  - `git diff --check`
  - `cargo tree -i lightningcss`
- Result:
  - All 87 integration tests passed; all-target benchmark smoke cases passed.
  - All 5 doctests passed.
  - Clippy, rustdoc warnings-as-errors, formatting, and diff checks passed.
  - Dependency resolution reports `lightningcss v1.0.0-alpha.72`.
  - Independent agents reported no remaining blocking AST/API/test/doc issue under the documented trust model.
  - Adversarial review used Chrome 151 to confirm that uppercase/escaped VAR/ENV, CSS Values 5 `if()`, and dashed custom functions can compute image-set URL fetches, then confirmed the final sanitizer blocks those cases inside recognized resource wrappers without resource permission.
- Skipped checks:
  - Full performance benchmark timing (`cargo bench`) was not run; benchmark targets ran in smoke/test mode.
  - No publish, package version bump, commit, or deployment.

## Remaining Work
- Known gaps:
  - `SanitizeOptions::with_value_guard(false)` intentionally disables all value/resource protection.
  - `StrictPolicy::allow_url()` is a broad allow-all-resources permission. Use a custom `check_resource` policy for scheme/origin/kind restrictions and resolve relative values against a trusted base URL.
  - `allow_var()`, `allow_env()`, and explicitly allowed arbitrary-substitution functions trust computed results supplied outside the sanitized fragment. Direct `background-image: var(--asset)` or `background-image: --asset()` cannot be resolved without cascade/DOM/external `@function` state. Inside recognized `src()`/`image()`/`image-set()` wrappers, dynamic results additionally require resource permission.
  - Selector scoping and conditional-request side channels after URLs are explicitly allowed remain caller policy concerns.
  - No new input-size/resource limit was added by user decision; lightningcss parsing still occurs before sanitizer `max_depth` traversal limits.
  - The manifest requirement is not an exact alpha.72 pin, although the current lock resolves alpha.72. A future dependency resolution/upgrade needs another AST/resource audit.
- Risks:
  - `NodeAction::Skip` now behaves like `Continue` for traversal and may over-remove content in older policies that relied on unchecked subtree preservation. This is documented as breaking.
  - Generic unresolved functions inside raw/custom image syntax are deliberately over-removed without resource permission, even if they eventually produce a non-fetching image.
- Suggested next entry points:
  - Decide and apply the 0.4.x version/changelog/release milestone.
  - If a stronger cross-stylesheet guarantee is required, design a property-aware environment model for custom-property/custom-function results rather than a brittle URL-capable-property list.
  - Consider exact-pinning the alpha dependency if the user approves changing their manifest requirement.

## Notes For Next Agent
- Do not revert or rewrite the user's alpha.72 dependency change. `Cargo.toml`/`Cargo.lock` are currently clean relative to HEAD.
- Preserve the explicit decision not to add input limits unless the user reopens that scope.
- Keep generic resource-function recognition independent of lightningcss typed URL traversal; the Visit-based oracle alone cannot detect semantic functions that upstream leaves generic.
- Do not treat `@namespace` URI identifiers or `@-moz-document` conditions as network fetches. `@import` is explicitly resource-gated.
- Current safety contract is fail-closed for syntactically identifiable resources; external cascade/custom-function results are trusted only when their corresponding policy permissions are explicitly enabled.
