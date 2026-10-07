# REQ-FILE-004 — the pre-D8 baseline (RFC-026 slice C)

Four runs of `cargo test --release -p tekstide editor_typing_latency_under_a_watched_burst -- --ignored --nocapture`, each with a fresh `XDG_STATE_HOME` under `/dev/shm`, `TMPDIR=/dev/shm/tkrel`, and `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true` (RFC-057's build, needed because the suite's `()` renderer exists only with debug assertions). Commit: 7086d81. rustc: rustc 1.99.0 (b940084d7 2026-09-28). CPU: AMD Ryzen 9 9950X 16-Core Processor, 32 threads.

**The machine was not quiet.** Load average across the four runs was 6.2 to 10.7 (1-minute), against RFC-057's quiet 1.0 to 1.6. The figures are therefore a baseline under other sessions' load, and say so wherever they are quoted.

## What is measured, and what is not

- The RFC-057 harness, in-process: each keystroke is `update`, then `view`, then the layout of the rows that changed, timed separately, on the 100,000-line fixture, typing a character at the start.
- The burst: 1,000 files written into the project root (always watched) at one a millisecond, from a thread of their own. The watcher's notices, drain ticks and explorer scan results are delivered to `update` between keystrokes in the order the runtime's subscriptions would deliver them, and each explorer scan runs on a worker thread. That delivery is timed apart from the keystroke, and reported as the work between keystrokes.
- **Not measured:** painting and presenting the frame (the limit RFC-057 disclosed); the iced event loop itself, since delivery is emulated in-process; real keyboard input; any window latency.

## The four runs

### Run 1

```text
fixture: 100000 lines, 3307639 bytes; body font 14 px; layout width 880 px; keystroke: character at the start
the project watched, nothing happening (60 keystrokes)
  stage           p50 ms     p95 ms     p99 ms
  update           6.813      7.010      7.049
  view             1.210      1.224      1.226
  layout           0.008      0.014      0.016
  SUM              8.029      8.228      8.271   (budget: p95 <= 16, p99 <= 33)
during a 1,000-file burst into the project root, one file a millisecond (131 keystrokes)
  stage           p50 ms     p95 ms     p99 ms
  update           6.831      6.973      7.016
  view             1.226      1.239      1.249
  layout           0.009      0.014      0.016
  SUM              8.067      8.205      8.249   (budget: p95 <= 16, p99 <= 33)
watch, during the burst: 1000 files written, 4003 notices delivered, 3 explorer scans run, 3 results applied; delivery (the work between keystrokes, not counted in the keystroke figures) 1.2 ms in total
```

### Run 2

```text
fixture: 100000 lines, 3307639 bytes; body font 14 px; layout width 880 px; keystroke: character at the start
the project watched, nothing happening (60 keystrokes)
  stage           p50 ms     p95 ms     p99 ms
  update           6.977      7.132      8.101
  view             1.249      1.270      1.320
  layout           0.009      0.019      0.023
  SUM              8.233      8.413      9.436   (budget: p95 <= 16, p99 <= 33)
during a 1,000-file burst into the project root, one file a millisecond (129 keystrokes)
  stage           p50 ms     p95 ms     p99 ms
  update           6.996      7.111      7.367
  view             1.249      1.280      1.292
  layout           0.009      0.018      0.031
  SUM              8.260      8.390      8.629   (budget: p95 <= 16, p99 <= 33)
watch, during the burst: 1000 files written, 4003 notices delivered, 3 explorer scans run, 3 results applied; delivery (the work between keystrokes, not counted in the keystroke figures) 1.2 ms in total
```

### Run 3

```text
fixture: 100000 lines, 3307639 bytes; body font 14 px; layout width 880 px; keystroke: character at the start
the project watched, nothing happening (60 keystrokes)
  stage           p50 ms     p95 ms     p99 ms
  update           6.749      6.898      6.985
  view             1.199      1.208      1.222
  layout           0.008      0.013      0.015
  SUM              7.959      8.099      8.199   (budget: p95 <= 16, p99 <= 33)
during a 1,000-file burst into the project root, one file a millisecond (131 keystrokes)
  stage           p50 ms     p95 ms     p99 ms
  update           6.803      6.976      7.204
  view             1.225      1.275      1.291
  layout           0.011      0.018      0.033
  SUM              8.036      8.262      8.503   (budget: p95 <= 16, p99 <= 33)
watch, during the burst: 1000 files written, 4003 notices delivered, 3 explorer scans run, 3 results applied; delivery (the work between keystrokes, not counted in the keystroke figures) 1.7 ms in total
```

### Run 4

```text
fixture: 100000 lines, 3307639 bytes; body font 14 px; layout width 880 px; keystroke: character at the start
the project watched, nothing happening (60 keystrokes)
  stage           p50 ms     p95 ms     p99 ms
  update           6.736      6.833      6.957
  view             1.202      1.212      1.218
  layout           0.008      0.012      0.017
  SUM              7.944      8.046      8.174   (budget: p95 <= 16, p99 <= 33)
during a 1,000-file burst into the project root, one file a millisecond (133 keystrokes)
  stage           p50 ms     p95 ms     p99 ms
  update           6.737      6.888      7.120
  view             1.224      1.240      1.249
  layout           0.010      0.015      0.016
  SUM              7.966      8.117      8.331   (budget: p95 <= 16, p99 <= 33)
watch, during the burst: 1000 files written, 4003 notices delivered, 3 explorer scans run, 3 results applied; delivery (the work between keystrokes, not counted in the keystroke figures) 1.4 ms in total
```

## Reading

- Watched and idle, p95 total: 8.05 to 8.41 ms across runs. Burst, p95 total: 8.12 to 8.39 ms. The difference is within the run-to-run spread, so no effect of the watcher is visible **before** D8, which is the point of a baseline.
- Budget: NFR-PERF-003, p95 at most 16 ms, p99 at most 33 ms. Both conditions sit well inside it at this load.
- This is **a baseline, not the requirement's evidence.** `REQ-FILE-004` says watching must not block editor input in the shipped system. The box is ticked by the measurement after D8, with both numbers reported.
