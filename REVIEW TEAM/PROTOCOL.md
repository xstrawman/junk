# REVIEW TEAM Protocol

## When required

Any of: APK change, download path/engine, public install artifact, GitHub push
of user-facing behavior, packaging removal, “it works now” claims.

## Inbox format (`inbox/CHANGESET.md`)

```markdown
# Changeset
- Summary:
- Goals / user pain fixed:
- Paths touched:
- Build commands run + results:
- Known risks:
- Stylistic choices already made (do not re-ask user):
```

## Outbox format (each agent)

```markdown
# <ROLE> review
VERDICT: APPROVE | REJECT

## Findings
- …

## Stylistic decisions (if APPROVE)
- …

## Must-fix before re-review (if REJECT)
- …
```

## FINAL.md (QA GATE only)

```markdown
SHIP: YES | NO
Summary:
Blocking issues:
```

## Agent invocation (parent agent)

Use **six** independent review passes (subagents preferred). Do not self-approve.
Do not show the human a success narrative until `SHIP: YES`.

If REJECT: fix, new changeset, full re-run (or delta re-run of failed roles + QA).
