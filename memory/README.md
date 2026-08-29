# Memory

This folder stores milestone snapshots for agent handoff. Use it after a user-approved scope is completed so the next agent can resume without reconstructing the full conversation.

## When To Update
- After a requested version/changelog milestone is finished.
- After a major domain layer is implemented or removed.
- After server/worker bridge behavior, permissions, migrations, or API surface changes.
- Before the final response for a completed milestone.

## Required Shape
Use `memory/TEMPLATE.md`. Keep entries concise but complete enough to resume implementation.

## Current Entries
- [2026-08-30 0.5.0 typed policy redesign](2026-08-30-0.5.0-typed-policy.md) — clean breaking policy API, semantic resource/import controls, parser and HTML output guards, full validation, and successful publish dry-run (not published).
- [2026-08-09 0.4.0 release version](2026-08-09-0.4.0-release-version.md) — breaking version bump, README/changelog/lock alignment, full tests, and successful crates.io publish dry-run (not published).
- [2026-08-09 lightningcss alpha.72 resource guard hardening](2026-08-09-lightningcss-alpha72-resource-guard.md) — alpha.72 compatibility, semantic resource guard, URL/VAR/ENV/arbitrary-substitution hardening, and multi-agent/browser revalidation (no input limits).
- [2026-06-26 0.3.0 deny-by-default redesign](2026-06-26-0.3.0-deny-by-default.md) — deny-by-default policy + engine value guard + StrictPolicy + @scope/@container/@property/depth fixes (post-review).
