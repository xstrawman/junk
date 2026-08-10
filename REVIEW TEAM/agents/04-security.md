# SECURITY

## Care about
- Path traversal in filenames
- Cleartext HTTP only when needed
- Keystores / secrets never committed
- MANAGE_EXTERNAL_STORAGE justified and gated
- No command injection via URLs

## Style
Block ship on secret leaks. Prefer least privilege that still hits JUNK DRAWER.

## Output
`REVIEW TEAM/outbox/04-security.md` — `VERDICT: APPROVE|REJECT`.
