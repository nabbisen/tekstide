# REQ-FILE-004 — the paired baseline, with its control (RFC-026 slice C, review 462)

Three runs of `cargo test --release -p tekstide editor_typing_latency_under_a_watched_burst -- --ignored --nocapture`, each with a fresh `XDG_STATE_HOME` under `/dev/shm`, `TMPDIR=/dev/shm/tkrel`, and `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true` (RFC-057's build). Each run: the 100,000-line fixture, the character-at-the-start keystroke, and three conditions in one process (unwatched as the control, watched and idle, and watched during a 1,000-file burst at one file a millisecond), over **six rounds, one per order of the three conditions**, so position in the run is balanced exactly. 200 keystrokes for the control and idle conditions; the burst runs until it has settled (about 130 keystrokes during it).

The figure that matters is the **paired difference** within a round, against the control: everything else is held constant, so the difference is the code's.

**Load, before and after each run:** Run 1 1.42 0.80 0.33 → 1.66 0.93 0.39; Run 2 1.48 0.92 0.40 → 10.69 3.03 1.11; Run 3 10.69 3.03 1.11 → 6.96 2.95 1.15 (1-minute, against RFC-057's quiet 1.0 to 1.6).

**Run 2 had a load spike from another session during rounds 5 and 6.** It inflated both the control and the watched conditions by up to +5 ms in those rounds. A paired control cancels load only while load is steady across a round, so those two rounds are the noise, not the effect. The medians across six rounds are not moved by them.

## Run 1

```text
fixture: 100000 lines, 3307639 bytes; body font 14 px; layout width 880 px; keystroke: character at the start; budget NFR-PERF-003: p95 <= 16 ms, p99 <= 33 ms
round 1: control 9.591 ms; watched-idle -1.612 ms; watched-burst -1.617 ms
round 2: control 7.899 ms; watched-idle +0.163 ms; watched-burst +0.822 ms
round 3: control 7.815 ms; watched-idle +0.025 ms; watched-burst +0.075 ms
round 4: control 8.061 ms; watched-idle +0.473 ms; watched-burst +1.330 ms
round 5: control 7.856 ms; watched-idle +0.058 ms; watched-burst +0.136 ms
round 6: control 7.838 ms; watched-idle +0.142 ms; watched-burst +0.285 ms
median of the six rounds: watched-idle +0.142 ms, watched-burst +0.285 ms against the control
the control's p99 per round: [13.093, 7.945, 7.856, 8.154, 7.951, 7.882] ms
the burst's p99 per round: [8.031, 10.364, 8.032, 13.272, 8.176, 8.269] ms
watch, round 1: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.3 ms of delivery between keystrokes
watch, round 2: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.6 ms of delivery between keystrokes
watch, round 3: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.5 ms of delivery between keystrokes
watch, round 4: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.4 ms of delivery between keystrokes
watch, round 5: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.3 ms of delivery between keystrokes
watch, round 6: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.4 ms of delivery between keystrokes
```

## Run 2

```text
fixture: 100000 lines, 3307639 bytes; body font 14 px; layout width 880 px; keystroke: character at the start; budget NFR-PERF-003: p95 <= 16 ms, p99 <= 33 ms
round 1: control 8.059 ms; watched-idle +0.010 ms; watched-burst +0.104 ms
round 2: control 8.188 ms; watched-idle -0.039 ms; watched-burst +0.491 ms
round 3: control 7.997 ms; watched-idle +0.015 ms; watched-burst -0.002 ms
round 4: control 8.030 ms; watched-idle -0.150 ms; watched-burst +0.072 ms
round 5: control 16.961 ms; watched-idle -2.761 ms; watched-burst -3.140 ms
round 6: control 9.307 ms; watched-idle +4.017 ms; watched-burst +5.085 ms
median of the six rounds: watched-idle +0.010 ms, watched-burst +0.104 ms against the control
the control's p99 per round: [8.136, 8.579, 8.280, 8.250, 26.132, 10.571] ms
the burst's p99 per round: [8.644, 9.126, 8.100, 8.211, 14.047, 27.309] ms
watch, round 1: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.3 ms of delivery between keystrokes
watch, round 2: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.3 ms of delivery between keystrokes
watch, round 3: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.2 ms of delivery between keystrokes
watch, round 4: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.3 ms of delivery between keystrokes
watch, round 5: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.8 ms of delivery between keystrokes
watch, round 6: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 2.2 ms of delivery between keystrokes
```

## Run 3

```text
fixture: 100000 lines, 3307639 bytes; body font 14 px; layout width 880 px; keystroke: character at the start; budget NFR-PERF-003: p95 <= 16 ms, p99 <= 33 ms
round 1: control 8.687 ms; watched-idle -0.046 ms; watched-burst -0.279 ms
round 2: control 8.080 ms; watched-idle -0.038 ms; watched-burst +0.055 ms
round 3: control 8.209 ms; watched-idle -0.105 ms; watched-burst -0.060 ms
round 4: control 8.113 ms; watched-idle +0.245 ms; watched-burst +0.043 ms
round 5: control 8.036 ms; watched-idle -0.146 ms; watched-burst +0.113 ms
round 6: control 7.836 ms; watched-idle +0.045 ms; watched-burst +0.140 ms
median of the six rounds: watched-idle -0.038 ms, watched-burst +0.055 ms against the control
the control's p99 per round: [9.029, 8.131, 8.316, 8.173, 8.107, 7.893] ms
the burst's p99 per round: [8.593, 8.234, 8.236, 8.500, 8.297, 8.018] ms
watch, round 1: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.4 ms of delivery between keystrokes
watch, round 2: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.2 ms of delivery between keystrokes
watch, round 3: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.2 ms of delivery between keystrokes
watch, round 4: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.2 ms of delivery between keystrokes
watch, round 5: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.6 ms of delivery between keystrokes
watch, round 6: 1000 files written, 4003 notices, 3 explorer scans run, 3 applied, 1.4 ms of delivery between keystrokes
```

## Summary

| run | median paired difference, watched idle | median paired difference, watched during the burst |
| --- | ---: | ---: |
| Run 1 | +0.142 ms | +0.285 ms |
| Run 2 | +0.010 ms | +0.104 ms |
| Run 3 | −0.038 ms | +0.055 ms |

**Reading.** The medians are within ±0.3 ms, and the burst median is at or above the idle median in every run. That is a small, consistent-in-sign tendency, not a demonstrated effect: round-to-round spread in the paired differences is ±1 to 2 ms on a quiet machine and ±5 ms through a load spike. **This harness resolves effects of roughly half a millisecond at the median, and no better.** The post-D8 measurement must use the same protocol, and any change it reports has to be read against this noise floor.

**Limits, stated where the figures are quoted.** Painting and presenting are not measured. The runtime's event loop is emulated in-process, and keys go through `update`. Round 1 of run 1 and round 4 of run 1 each show a single p99 outlier (13 ms); they are in the record, not removed.

The earlier four-run record (`req-file-004-baseline.md`) had no control and is superseded by this one.
