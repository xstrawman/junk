# DOWNLOAD ENGINE review
VERDICT: APPROVE

## Scope (final re-review)

- `android/app/src/main/java/dev/xstrawman/junk/download/MultiConnDownloader.kt`
  - `multiDownload` (segment loop, retry, global `done`)
  - `downloadRange` (Range write + credit)
- Claim under test: on exception mid-segment, `segDone` is rolled back via `getAndSet(0)` and `done.addAndGet(-rolled)` **before** retry; next attempt starts `Range` from `from` again and credits **only** new writes.

## Claim verification — **PASS**

### 1. Exception mid-segment → rollback before retry

In `multiDownload` each segment owns a private `AtomicLong segDone` (starts at 0). `downloadRange` credits **both** global `done` and `segDone` on every successful write:

```kotlin
done.addAndGet(add)
segDone.addAndGet(add)
```

Order is **write then credit** (`raf.write` → `pos +=` → addAndGet), so a throw on write never leaves phantom credit.

Catch path (runs before the next `while` iteration / next `downloadRange` call):

```kotlin
} catch (e: Exception) {
    val rolled = segDone.getAndSet(0)
    if (rolled > 0) done.addAndGet(-rolled)
    if (attempt >= 4) {
        err.set(true)
        errMsg = e.message
    }
}
```

- `getAndSet(0)` atomically captures this attempt’s credit and clears segment credit.
- `done.addAndGet(-rolled)` removes only that amount from the global counter (other segments untouched).
- Rollback is complete **before** the next attempt; no path can re-enter `downloadRange` with leftover `segDone`.

Covers: short read (`error("short read …")` after partial body), IO/read failures mid-stream, non-206/HTTP errors after partial body, any other `Exception` after some credits.

### 2. Next attempt starts Range from `from` again

Segment bounds are fixed once:

```kotlin
val from = i * chunk
val to = if (i == n - 1) total - 1 else (i + 1) * chunk - 1
```

Every call (including retries) is:

```kotlin
downloadRange(url, partFile, from, to, done, active, segDone)
// → header "Range: bytes=$from-$to"
// → raf.seek(from)
```

There is no mid-segment resume offset. Failed partial file bytes in `[from, to]` are overwritten on the next full-range attempt. Matches the chosen strategy: **rollback + full segment restart** (not keep-credit + resume).

### 3. Credits only new writes

After catch:

| counter   | state |
|-----------|--------|
| `segDone` | `0` |
| `done`    | prior global total minus this segment’s failed attempt |

Retry only runs `addAndGet` for bytes actually written on that attempt. No double-count of the failed partial. On success, segment contributes exactly `to - from + 1` net once (modulo cancel race below).

### Prior REJECT #1 (Range retry double-count) — **FIXED**

Earlier defect: return value / outer `credited` never updated on throw, so catch rollback was a no-op and retry re-added the same bytes.

Current design matches the prescribed minimal fix from the prior review:

- per-segment `AtomicLong` updated **inside** `downloadRange` on each write;
- catch always sees attempt credit via `segDone`, not via a successful assignment;
- **rollback** + restart `bytes=from-to` (not resume).

Invariant holds under concurrent segments: each job rolls back only its own `segDone`; `done` ops are `AtomicLong` add/sub.

## Other prior rejects (still fixed)

| # | Issue | Status |
|---|--------|--------|
| 2 | Unknown length → fake 100% in `singleDownload` | **PASS** — `bytesTotal = 0` when unknown; phase `downloading-unknown-size` |
| 3 | `total < connections` → `chunk=0` | **PASS** — `n = min(connections,16).coerceAtMost(total).coerceAtLeast(1)`; `from > to` guard |
| 4 | `audioUrl` never downloaded | **PASS** (not re-broken in this file; prior extract path fix stands outside this re-review focus) |

## Residual notes (non-blocking)

1. **Cancel mid-segment does not throw.** Loop exits on `cancel.get()`; short-read check is gated `&& !cancel.get()`, so `downloadRange` returns “ok” with partial credit still in `done`/`segDone`. Outer `multiDownload` then `error("cancelled")` and caller deletes `.junk.part`. Progress may look partial until cancel surfaces — acceptable; not a retry double-count.
2. **Ticker vs rollback:** UI can briefly show a higher `done` until catch runs; `coerceIn(0, total)` clamps display. Internal accounting ends correct after rollback.
3. **Servers that ignore `Range`:** multi path still assumes honor when probe said ranges OK; unchanged product risk, not this retry claim.
4. **Max 4 attempts** then `err` + sibling segments stop starting new retries; `awaitAll` + `error(errMsg)` — fine.

## Must-fix before re-review

_(none)_

## Stylistic decisions (APPROVE)

- Keep **rollback + full-range restart** for segment retries (simpler than mid-segment resume; correct with `segDone`).
- Keep write-then-credit ordering.
- Keep unknown-size honesty in `singleDownload`.
- Keep `n` capped by `total` so multi-conn never invents empty ranges.

## Summary

`multiDownload` / `downloadRange` now implement exception-safe segment accounting: mid-segment failure rolls back only that segment’s credit via `segDone.getAndSet(0)` + `done.addAndGet(-rolled)` before any retry; the next attempt always re-requests `bytes=from-to`, seeks `from`, and credits only newly written bytes. Prior double-count REJECT is closed. **APPROVE** download engine for ship on this invariant.
