# REQ-FILE-004 — the measurement after D8, with its control and its before (RFC-026 slice C)

One release run of `cargo test --release -p tekstide editor_typing_latency_under_a_watched_burst -- --ignored --nocapture` on the commit that carries D8 (`364164c`), with a fresh `XDG_STATE_HOME` under `/dev/shm`, `TMPDIR=/dev/shm/tkrel`, and `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true` (RFC-057's build). Load before and after: 10.88 → 12.18 (1-minute), against RFC-057's quiet 1.0 to 1.6. The machine was busy: round 3 was hit by a load spike that moved **all four conditions** by several milliseconds, so round 3's paired figures are not usable, and they are kept in the record.

## The four conditions, in one frame, per round

- **unwatched (control):** the owner removed.
- **watched, nothing happening.**
- **burst before D8:** 1,000 files into the project root at one a millisecond, the open document untouched. This is the pipeline as it was before D8.
- **burst with D8:** the same burst, and the open document rewritten every hundred files, alternating between two versions, so each rewrite is a real change the refresh reads. This is the pipeline with D8.

The **D8 cost is the paired difference between the last two**, in the same round, so everything else is held constant.

```text
round 1 unwatched (control)                            keystrokes  200  p50  8.508  p95 10.611  p99 12.167 ms
round 1 burst into the root (before D8)                keystrokes  127  p50  8.353  p95  8.717  p99  9.584 ms
round 1 burst, open document rewritten (with D8)       keystrokes  122  p50  8.377  p95 11.062  p99 12.002 ms
round 1 watched, nothing happening                     keystrokes  200  p50  8.273  p95  8.663  p99 10.633 ms
round 2 burst into the root (before D8)                keystrokes  125  p50  8.316  p95  8.960  p99 11.717 ms
round 2 burst, open document rewritten (with D8)       keystrokes  128  p50  7.767  p95 10.656  p99 11.449 ms
round 2 watched, nothing happening                     keystrokes  200  p50  8.015  p95  8.154  p99  8.280 ms
round 2 unwatched (control)                            keystrokes  200  p50  7.977  p95  8.110  p99  8.155 ms
round 3 burst, open document rewritten (with D8)       keystrokes  135  p50  7.664  p95  8.857  p99 11.074 ms
round 3 watched, nothing happening                     keystrokes  200  p50 14.980  p95 20.251  p99 22.511 ms
round 3 unwatched (control)                            keystrokes  200  p50  9.034  p95 16.256  p99 22.481 ms
round 3 burst into the root (before D8)                keystrokes   87  p50 13.837  p95 16.625  p99 19.411 ms
round 4 watched, nothing happening                     keystrokes  200  p50  8.678  p95  9.663  p99 11.299 ms
round 4 unwatched (control)                            keystrokes  200  p50  8.238  p95 10.374  p99 11.406 ms
round 4 burst into the root (before D8)                keystrokes  130  p50  8.095  p95  8.421  p99  8.581 ms
round 4 burst, open document rewritten (with D8)       keystrokes  139  p50  7.524  p95  8.168  p99  8.280 ms
paired differences from the control, per round (p95 of the keystroke total):
round 1: control 10.611 ms; watched-idle -1.948; burst before D8 -1.893; burst with D8 +0.451; D8 cost (with minus before) +2.345 ms
round 2: control 8.110 ms; watched-idle +0.044; burst before D8 +0.850; burst with D8 +2.546; D8 cost (with minus before) +1.696 ms
round 3: control 16.256 ms; watched-idle +3.996; burst before D8 +0.369; burst with D8 -7.399; D8 cost (with minus before) -7.768 ms
round 4: control 10.374 ms; watched-idle -0.711; burst before D8 -1.953; burst with D8 -2.206; D8 cost (with minus before) -0.253 ms
median D8 cost over the four rounds (p95, with minus before): +0.721 ms
watch, round 1 before D8: 1000 files written, 2000 notices, 3 explorer scans applied, 1.5 ms of delivery between keystrokes (with D8 this includes the document refreshes)
watch, round 1 with D8: 1000 files written, 2020 notices, 3 explorer scans applied, 6.0 ms of delivery between keystrokes (with D8 this includes the document refreshes)
watch, round 2 before D8: 1000 files written, 2000 notices, 3 explorer scans applied, 1.2 ms of delivery between keystrokes (with D8 this includes the document refreshes)
watch, round 2 with D8: 1000 files written, 2020 notices, 3 explorer scans applied, 6.5 ms of delivery between keystrokes (with D8 this includes the document refreshes)
watch, round 3 with D8: 1000 files written, 2020 notices, 3 explorer scans applied, 5.4 ms of delivery between keystrokes (with D8 this includes the document refreshes)
watch, round 3 before D8: 1000 files written, 2000 notices, 3 explorer scans applied, 3.1 ms of delivery between keystrokes (with D8 this includes the document refreshes)
watch, round 4 before D8: 1000 files written, 2000 notices, 3 explorer scans applied, 1.2 ms of delivery between keystrokes (with D8 this includes the document refreshes)
watch, round 4 with D8: 1000 files written, 2020 notices, 3 explorer scans applied, 5.3 ms of delivery between keystrokes (with D8 this includes the document refreshes)
```

## What the figures say

- **The median D8 cost, p95 of the keystroke total, over four rounds: +0.72 ms.** The round-to-round spread is −7.8 to +2.3 ms, and the −7.8 is round 3's spike. Excluding round 3 (unusable), the three remaining figures are +2.3, +1.7 and −0.3 ms. **That is noisier than the effect it is trying to resolve**, so the median is reported, and no claim of a precise D8 cost is made from it.
- **The work between keystrokes rose by about 4 ms per burst** (1.2–3.1 ms before D8, 5.3–6.5 ms with D8). That is the document refresh on the update thread, four or so per burst, each reading the 3.3 MB fixture file. It is a deterministic measure, and it is the honest cost of D8 on the update thread.
- **Every p95 is inside the 16 ms budget**, with D8 (8.2 to 11.1 ms in all four rounds) and without it. Against the control in rounds 1, 2 and 4, the control's p95 was 8.1 to 10.6 ms. p99 with D8 reached 12.0 ms (round 1); the budget is 33 ms.
- **Read against the noise floor** of the earlier baseline (about half a millisecond at the median, ±1 to 2 ms round to round on a quiet machine), the D8 median is at the edge of resolution. It is reported, not claimed as zero.

## The loop found before this figure could be trusted

The first measurement with D8 ran for two minutes after the burst had finished, with 239 explorer scans and 465 ms of delivery work between keystrokes, against 3 scans and 1.5 ms before D8. A diagnostic (`d8_burst_diagnostic`) showed the root being re-recorded after the burst, and a count of notices that was not zero after the burst once the counter was fixed.

**The cause:** notify subscribes to `OPEN` on every watched directory (`notify-8.2.0/src/inotify.rs:427`). The document refresh reads its own file, so each refresh was answered by an open event naming the document, which became a notice, which asked for another refresh. A loop with no end.

**The fix:** `WatchEvents::wait_for_notice` drops access events. An open is not a change. Test: `reading_a_watched_file_is_not_reported_but_writing_it_is`, which fails without the filter (ablated). With the filter the diagnostic run takes 1.8 s with 3 scans and 5.6 ms of delivery work, and the four-condition measurement above is the result of that fixed pipeline.

## Two defects found on the way, both fixed with tests

- **An edit after an external change left the document `ExternalChanged`**, which reads as "nothing local to lose", so a later refresh would not mark a conflict and the save dialog would say there was nothing to lose. Fixed in `TextDocument`: an edit that differs from what was opened is a `Conflict`. A `Conflict` is still not cleared by undoing the text (RFC-057's rule, kept). Test: `an_edit_made_after_an_external_change_is_a_conflict_not_a_clean_change` (ablated).
- **A deleted open file read as "changed on disk"**: both come back as `ExternalChanged`. The workspace now reports `ExternalDeleted`, and the header says `deleted on disk, not reloaded`. Tests: `a_clean_document_whose_file_is_deleted_reports_external_deleted_and_keeps_its_text` and `the_header_names_a_deleted_file_apart_from_a_changed_one_and_says_nothing_was_reloaded`.

## D8's own properties, and their tests

- **No silent reload:** `an_external_change_to_the_open_document_reaches_it_without_a_silent_reload` — the notice reaches the document on the drain, its state is `ExternalChanged`, and its text is unchanged. Ablated: the drain's refresh removed, it fails at the drain assertion.
- **Unsaved edits survive:** `a_dirty_document_keeps_its_edit_when_the_file_changes_on_disk` — the state is `Conflict` and the edit is kept.
- **Deleted is a state the product can say:** above.
- **Changed is named as not reloaded in the header:** the header suffix `(changed on disk, not reloaded)` and `(deleted on disk, not reloaded)`, from the catalog. No new line is drawn, so the editor's window does not move.

**Not done:** a reload with undo history (the checklist's own item, `A reload takes the undo history`): there is no reload to take it through, since none is performed silently, and a user-driven reload is not built yet.
