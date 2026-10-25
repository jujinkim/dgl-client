# Official hook contract fixtures

Synthetic metadata shaped from the official sources linked in `contracts.json`,
reviewed 2026-10-25. No account, prompt, tool output or transcript is used.
Codex 0.163.0 and 0.164.0 share the observer's configured event routes. These
fixtures verify parsing and neutral output, not receipt from an authenticated CLI.

AGY uses named definitions and direct handlers for non-tool events. Its required
PreToolUse `allow` delegates to default policy; it does not auto-approve. The empty
Stop decision leaves the original stop unchanged. Grok's observer ignores idle
and task completion notifications: only Stop contributes a Response.
