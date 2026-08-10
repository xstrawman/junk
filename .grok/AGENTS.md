# Junk project agent rules

## REVIEW TEAM gate (mandatory)

Before telling the user work is complete, shippable, installable, or “fixed”:

1. Write `REVIEW TEAM/inbox/CHANGESET.md`
2. Run the **REVIEW TEAM** (agents in `REVIEW TEAM/agents/`) — prefer independent subagents
3. Require `REVIEW TEAM/outbox/FINAL.md` with `SHIP: YES`

Do **not** ask the user to first-pass review code quality or taste. The team decides style.
Only escalate true blockers (credentials, irreversible data loss, legal grey areas).

## Product invariants

- Public downloads path: **`Downloads/JUNK DRAWER`** (never obscure app-private dirs as primary)
- Scrap **Soar** packaging — do not reintroduce
- APK must handle YouTube/stream extract (NewPipe Extractor or better), multi-conn HTTP, magnets honestly
