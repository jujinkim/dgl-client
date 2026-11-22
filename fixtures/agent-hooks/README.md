# Official hook contract fixtures

Synthetic metadata shaped from the official sources linked in `contracts.json`,
reviewed 2026-10-25. No account, prompt, tool output or transcript is used.
Codex 0.163.0 and 0.164.0 share the observer's configured event routes. These
fixtures verify parsing and neutral output, not receipt from an authenticated CLI.

AGY uses named definitions and direct handlers for non-tool events. Its required
PreToolUse `allow` delegates to default policy; it does not auto-approve. The empty
Stop decision leaves the original stop unchanged. Grok's observer ignores idle
and task completion notifications: only Stop contributes a Response.

`status-payloads.json` holds synthetic inputs for detailed status collection:
hook payloads (Claude `effort`/`PostModelSwitch`/`StopFailure`, Codex
`PostModelRequest`, AGY `modelName`, Grok `StopFailure`), an AGY statusline
payload, a Claude Code mod status file and Grok `signals.json`/`usage.json`.
Values listed under `secrets` stand for emails, titles, paths, prompts and raw
ids; the observer tests prove none of them reaches the status table or the poll
reply. They verify the whitelist and mapping, not receipt from a real CLI.
