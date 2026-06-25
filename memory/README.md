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
- [2026-06-26 0.3.0 deny-by-default redesign](2026-06-26-0.3.0-deny-by-default.md) — deny-by-default policy + engine value guard + StrictPolicy + @scope/@container/@property/depth fixes (post-review).
