# REVIEW TEAM

**Hard gate:** Nothing is shown to the human as “done / ship / install / PR ready”
until this team has **pre-approved** the change set.

The human does **not** do first-pass review. Agents do. Agents **must** make
stylistic product choices when requirements are ambiguous — no “TBD, ask user”
for taste (colors, copy, layout density, error tone). Ask the user only for
true blockers (secrets, destructive ops, legal scope).

## Roster

| Agent | File | Mandate |
|-------|------|---------|
| **ARCHITECT** | `agents/01-architect.md` | Structure, boundaries, one-product coherence |
| **ANDROID UX** | `agents/02-android-ux.md` | Cabinet UI, permissions, JUNK DRAWER path clarity |
| **DOWNLOAD ENGINE** | `agents/03-download-engine.md` | Multi-conn, YouTube, magnet honesty, speed |
| **SECURITY** | `agents/04-security.md` | Storage, network, injection, secrets |
| **STYLIST** | `agents/05-stylist.md` | 90s arcade taste; not afraid to decide |
| **QA GATE** | `agents/06-qa-gate.md` | Build/install proof; final APPROVE / REJECT |

## Protocol

See `PROTOCOL.md`. Summary:

1. Implementing agent finishes work + runs builds/tests it can.  
2. Implementing agent writes `REVIEW TEAM/inbox/CHANGESET.md` (what/why/paths).  
3. Spawn **all six** reviewers (parallel OK for 01–05; **06 last**).  
4. Each writes `REVIEW TEAM/outbox/<role>.md` with `VERDICT: APPROVE|REJECT`.  
5. QA GATE only **APPROVE** if 01–05 all APPROVE (or documented override).  
6. `REVIEW TEAM/outbox/FINAL.md` must say `SHIP: YES` before telling the human.  

## Stylistic courage

Reviewers **pick** when taste is open:

- Prefer neon cabinet denser HUD over sparse “Material bland.”  
- Prefer showing full `…/Download/JUNK DRAWER` path always.  
- Prefer honest magnet failures over fake progress.  
- Prefer progressive YouTube streams on phone (no silent no-op).  

## Local path

```
REVIEW TEAM/
  README.md
  PROTOCOL.md
  agents/
  inbox/
  outbox/
  history/
```
