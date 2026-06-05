# Agent Instructions
- Do not modify files unless the user explicitly approves the specific change.
- Before proposing edits, explain the problem, affected files, and at least one implementation option.
- If a fix is needed, wait for user confirmation before applying patches.
- Analysis, tests, and read-only commands are allowed unless the user says otherwise.
- Do not touch, revert, format, or otherwise modify unrelated files or user changes outside the current task scope.
- If a file has changes you did not intentionally make, do not revert or "clean up" those changes; ask the user before touching it.

## Memory Workflow
- When a user-approved milestone is completed, especially after instructions such as "bump the version", "write the changelog", or "this scope is done", update the `memory/` folder before the final response.
- Use `memory/YYYY-MM-DD-<version-or-scope>.md` for milestone snapshots. Keep `memory/README.md` as the workflow index and `memory/TEMPLATE.md` as the required structure.
- A milestone memory document must include: objective, user-approved scope/interactions, implementation status, major changed files/modules, API/service/repository/permission behavior, worker/background-job behavior, validation commands/results, remaining gaps, and follow-up entry points.
- Record facts that help the next agent resume work without rereading the whole conversation. Do not store secrets, tokens, private credentials, or irrelevant chat history.
- If a milestone changes server/worker integration, permissions, migrations, or version/changelog state, explicitly call that out in the memory document.
- If validation was skipped or failed, record exactly what was not verified and why.
- Do not edit unrelated memory entries while documenting a new milestone. If an older entry is wrong, create a short correction note or ask the user before rewriting it.
