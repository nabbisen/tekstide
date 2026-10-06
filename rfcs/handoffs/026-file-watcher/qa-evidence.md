# RFC-026 — QA evidence

## PR-026-A — the batching, proved against a simulated stream

**No dependency added, no watcher wired, no surface changed.** The slice adds `crates/tekstide-core/src/project/watch.rs` (the batcher), its tests in `project/watch/tests.rs`, and two `pub` lines in `project.rs`.

### What was built

`ScanBatcher` (`project/watch.rs`):

- `SCAN_WINDOW = 250 ms` — the window, a stated constant (D3). A window opens at the first change in a directory that has none open and closes exactly `SCAN_WINDOW` later. It is measured from the first change, not reset by each one, so a burst longer than the window yields one scan per window instead of starving until the burst ends.
- `GIT_SUBPROCESSES_PER_SCAN = 2` — the second count RFC-026 asks every batching claim to carry. One scan request costs the gate's configuration query and one `check-ignore` (measurement 8, review 431). The batcher runs no git itself; the count is the cost of each request it issues.
- `record(path, at)` — keys a change by its parent directory. Changes in an open window collapse into it; different directories never share one.
- `drain_due(now)` — returns each closed window as one `ScanRequest { directory, coalesced_events }`, in path order.

### The numbers

Both counts, for each claim, as the test prints them (`cargo test -p tekstide-core --lib project::watch -- --nocapture`):

| Stream | Scan requests | Git subprocesses |
| --- | --- | --- |
| 1,000 events into one directory, inside one window | **1** | **2** |
| 1,000 events into one directory, spread over five windows | **5** | **10** |
| Ablated — batching removed (below), the same 1,000-event burst | **1,000** | **2,000** |
| Ablated — batching removed, the five-window stream | **1,000** | **2,000** |

The burst is a real one, not a loop with nothing between events: its thousand changes are spread through the first half of the window, one every 125 µs, and the batcher is driven the way a real loop drives it (drain, then record, then drain at the end).

### Ablation

`rfcs/handoffs/ablate.sh` on a clean tree, replacing the batcher's `record` body with an unconditional, immediately-due insert:

```
self.pending.insert(directory, PendingScan { due: at, coalesced_events: 1 });
```

Every change now becomes its own scan. Four tests fail, including both counting tests — the burst issues **1,000** requests, the five-window stream **1,000** — which is the number the acceptance asked for. The boundary test and the different-directories test also fail, as they should. The file was restored by the script and the tree was clean afterwards.

### What the tests hold

- `a_burst_of_a_thousand_events_into_one_directory_yields_one_scan_request_per_window` — one request, all 1,000 changes answered by it, two git subprocesses.
- `sustained_churn_costs_one_scan_per_window_it_spans` — five requests across five windows, all changes answered, ten git subprocesses. Six windows of room, so the last window's own scan has time to close.
- `events_for_different_directories_do_not_silently_merge` — two directories, two requests, each answering its own 500 changes.
- `a_window_closes_exactly_one_window_after_the_change_that_opened_it` — not due a nanosecond early; due exactly at the close; a change arriving at the close opens the next window rather than joining the closing one.

### Not in this slice, and why it matters that it is not

No clock is read by any test: every instant is constructed, so the numbers do not depend on machine load. The batcher is not yet fed by anything. D4's evaluation of `notify` (slice B) is judged against these numbers, which is the point of building this first.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`, `rfc_docs_invariants` 16: clean. **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR`: `713 + 16 + 1049` = 1,778 passed, 0 failed, 0 entries left after each.** No intermittent.
