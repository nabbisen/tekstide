---
title: "GUI evidence prerequisites — what the 2026-11-11 testing-rules update needs before it can be followed"
status: "In progress 2026-11-11 — A, C and most of B landed; D investigated and reported infeasible without a scoped decision"
rfc_file: "none — infrastructure for `.git-exclude/rules/project-instructions-gui-common.md`"
created: "2026-11-11"
---

# GUI evidence prerequisites

## Why

The owner updated `.git-exclude/rules/project-instructions-gui-common.md` on 2026-11-11 with two
new sections (GUI Evidence and Automation Rules; Test Optimization Rules). Four things the rules
now ask for did not exist yet. This handoff is the durable record of building them; the start
file (`tekstide-start-gui-evidence-prerequisites.md`) is the instruction, this is what happened
against it.

**Not a rules change.** `.git-exclude/rules/project-instructions-gui-common.md` is the owner's
own file and is untouched by this slice.

## A. The launcher now isolates configuration, not only state

`rfcs/handoffs/launch-scratch-gui.sh` (moved from `.git-exclude/tools/`, which is gitignored and
therefore absent from a fresh clone — a mandatory script cannot live somewhere a fresh clone does
not have).

**What it does now that it did not before:**

1. Isolates `XDG_CONFIG_HOME` the same way it already isolated `XDG_STATE_HOME`: derived from the
   scratch root (`<scratch-root>/config`), refused if it resolves outside the scratch root or
   equal to the real one (`${XDG_CONFIG_HOME:-$HOME/.config}`). Read from
   `crates/tekstide-core/src/config/path.rs` and
   `crates/tekstide-core/src/project/recent/store.rs` directly: those two environment variables
   are the *only* two directories this app reads at all — no cache or data root exists to
   isolate, so none was added speculatively.
2. "One instance only": refuses to start if a `tekstide` process is already running (`pgrep -x`),
   and (since round 2 — see below) a `--kill <scratch-root>` mode *scoped to the instance this
   script started*, plus an explicit `--kill --by-name [binary]` for the old, unscoped teardown
   when that is genuinely what is wanted — the two halves the rule itself names, not only the
   refusal half the script already had.
3. Prints the pid of the instance it is about to launch (`$$`, since `exec` replaces the shell
   without forking, so the launched process keeps the script's own pid), and records it (with the
   binary's name) in `<scratch-root>/launcher.pid` for the scoped `--kill` to read back — the focus
   check before
   every keystroke is specified in terms of it.
4. Prints the one consequence of isolating `XDG_CONFIG_HOME` that the start file asked to be met
   before a confusing result rather than after: `crates/tekstide-core/src/runtime/git.rs`
   forwards `HOME` and `XDG_CONFIG_HOME` into every `git` subprocess Tekstide spawns. `HOME` stays
   real here, so `~/.gitconfig` still applies; a git identity kept at
   `$XDG_CONFIG_HOME/git/config` instead will not be seen by git run from inside this session.

**Backend is selectable since round 2 — see below.** Round 1 left the script forcing X11
unconditionally, reasoning that the start file's own "what to build" list did not ask for it.
Review 670 found that reasoning wrong: the move into the mandatory path is what turns a
limitation into a blocker, since the rules file's own Testing Guidelines say keyboard tests run
on the Wayland backend with `wtype` (`xdotool`'s keys do not reach this app under Wayland), and
RFC-076's own existing keyboard evidence used exactly that. Fixed; see the round-2 section.

**Evidence, per the start file's own ask: recorded state, not a picture.**

- *The three refusals, each ablated and firing its own message.* Pointed the scratch root so the
  resolved `XDG_STATE_HOME` equals the real one, so the resolved `XDG_CONFIG_HOME` equals the real
  one, and so `XDG_STATE_HOME` resolves (via a pre-planted symlink) outside the intended scratch
  root — three separate runs, three separate `REFUSING:` messages, each naming the exact resolved
  path and which check it tripped. A guard that has never refused is not a guard; these did.
- *The "one instance" precondition and `--kill`, both exercised against the real binary.* Launched
  one real instance, confirmed a second launch attempt refuses by name while the first is still
  running, then tore the first down with `--kill` and confirmed `pgrep -x tekstide` finds nothing
  afterward.
- *A distinctive scratch config, shown read verbatim by the real production parser, not assumed
  from the launcher's own correctness.* Wrote a `config.toml` with colours and font sizes that
  could not be mistaken for a default (`accent`/`border_focused` `#DEAD42`; `body_size 11`,
  `heading_size 29`, `status_size 13` — none of them 16/20/16) into a scratch `XDG_CONFIG_HOME`,
  then called `tekstide_core::config::ConfigStore::load` — the exact function the real app calls
  at boot — directly against that file, via a temporary test probe added to
  `crates/tekstide-core/src/config/tests/load.rs`, run, and reverted (the file is back to its
  committed content; `git status --porcelain` on it is empty). Output: every distinctive value
  parsed verbatim (`Accent`/`BorderFocused` `r:0.8706 g:0.6784 b:0.2588` = `#DEAD42`; `body_size:
  Some(11.0)`, `heading_size: Some(29.0)`, `status_size: Some(13.0)`), `fallbacks: []`. Separately,
  launched the real binary through the real script against a scratch root carrying the same
  config and read `/proc/<pid>/environ` of the running process: `XDG_STATE_HOME` and
  `XDG_CONFIG_HOME` both point at the scratch paths, `WAYLAND_DISPLAY` is absent (X11, as
  printed), and the pid matched exactly what the script had printed before `exec`.

**A finding, not a fix, and not required of this slice: AT-SPI is not actually available for this
app.** The existing Testing Guidelines name AT-SPI actions (`do_action`, `grab_focus`,
`EditableText.set_text_contents`) as the sanctioned non-keystroke interaction mechanism. Checked
directly rather than assumed: `python3`'s `gi.repository.Atspi`, querying the real AT-SPI desktop
registry while a real, running tekstide instance's pid was known, found **no application
registered under that pid at all** — only `xdg-desktop-portal-gtk` and `org.gnome.Nautilus`
appeared. This workspace's `iced` dependency enables `["tokio", "advanced"]` only; no
accessibility feature is requested, and grepping both crates for `accessibility`/`a11y` found
nothing. So AT-SPI genuinely cannot drive or inspect this app today, regardless of how carefully
it is queried — a fact worth recording for whoever next reaches for it, not a defect introduced
by this slice.

## B. The tier-2 harness, one property landed on one surface, ablated

**The shared harness.** `crates/tekstide/src/theme/text_extent.rs` (test-only,
`#[cfg(test)]`, alongside the existing `theme::contrast`): `wrapped_extent`/`unwrapped_extent`,
replacing `shell/tests.rs`'s own one-surface `wrapped_height` closure. Fixes the three
measurement bugs the start file named, each checked against the pinned sources directly rather
than taken on faith:

| | the old closure | production (`theme::text`) | fixed to |
|---|---|---|---|
| `line_height` | `LineHeight::Relative(1.0)` | `LineHeight::default()` = `Relative(1.3)` | calls `LineHeight::default()` |
| `shaping` | `Shaping::Basic` | `Shaping::default()` = `Auto` (this workspace's `iced` features are `["tokio", "advanced"]`, neither `advanced-shaping` nor `basic-shaping`) | calls `Shaping::default()` |
| `font` | `Font::DEFAULT` | `theme::ui_font()` (the *configured* family) | calls `super::ui_font()` |

**Why calling the same function, not copying today's value.** If iced's own defaults or this
workspace's `iced` feature flags ever change, both the harness and production move together, by
construction. What that cannot catch is `theme::text` itself gaining an override later —
`theme_text_overrides_nothing_the_harness_assumes_is_default` (`theme/text_extent/tests.rs`)
closes that gap by reading `theme.rs`'s own shipped source (the same technique
`theme::tests::the_theme_button_applies_the_theme_style` already uses) and failing if `text`'s
body ever calls `.line_height(..)`/`.shaping(..)`/`.wrapping(..)`.

**The harness is ablated against the bug it replaces, not merely documented as fixing it.**
`wrapped_extent_is_measurably_taller_than_the_relative_1_0_bug_would_report` constructs both the
corrected measurement and the exact buggy shape side by side, over the same multi-line content,
and asserts the corrected one measures taller — proving the fix is in effect, not merely present
in a doc comment. Two non-vacuity sanity tests (wrapped and unwrapped paths, mirroring
`change_review_decision_controls_measured_layout_cost`'s own "a real sentence must not measure
0px" check) guard against a regression to the null-renderer's always-zero shape.

**One property, landed on one surface, ablated before any widening.**
`every_advertised_binding_fits_its_own_help_modal_column` (`shell/tests.rs`): the Help modal
(`help_modal_view`) renders every keybinding with `.wrapping(Wrapping::None)` inside a column
sized by `help_modal_binding_column_px` — `HELP_MODAL_BINDING_COLUMN_EMS * font_size_body()`, a
crude ems multiplier, not a real measurement of the longest binding. That is exactly the shape of
defect "nothing overflows" exists to catch: a guessed constant, not falsified against real text.
Measured with the corrected harness, at `Theme::default().font_size_body()`, against every
binding `keyboard_help_lines` actually advertises on this host. **Passes today** (every binding
measures under the column). **Ablated before being trusted**: temporarily set
`HELP_MODAL_BINDING_COLUMN_EMS` to `1.0`, re-ran, and the property failed, naming every one of the
26 overflowing bindings with its own measured width against the (now too-small) column —
`"Ctrl+Shift+V": measured 87.91px > column 16.00px"` and 25 more. Reverted; `md5sum
crates/tekstide/src/shell.rs` afterward matches the pre-ablation value exactly
(`4abbf483ebe64c7346edd8bf546ca909`).

**Deliberately not widened in this slice.** "Land one property on one surface, ablated, before
widening" — this is one property (binding-column fit), one surface (the Help modal), at one point
in the matrix (default font size, default locale, the one shipped theme, the host's own current
window size). The full matrix the start file names as a working assumption (five interface-scale
steps; font sizes `{8.0, defaults 16/20/16, 32.0}`; both locales `en`/`pl`; one theme; a named
window-width set with the narrowest supported width stated explicitly) is **not built here** —
widening it, and picking the narrowest supported width, is this follow-up's own next round, not
folded into the same commit as the harness that makes it possible.

**Contrast stays at tier 1, as a recorded reading rather than a silent choice.** The rule lists
*contrast* among its tier-2 examples. This project already proves contrast arithmetically at tier
1 — `derived_contrast_pairs`'s own AA test and PR-076-D3's AAA test, both pure functions of the
shipped palette, no rendering involved — and the rule's own "cheapest level that can prove it"
places it there, not at tier 2. No code changes from this; recorded so a future reader does not
wonder why no tier-2 contrast property exists.

**A duplication finding, not fixed here.** Two *other* existing, already-correct hand-rolled
copies of this same `Text`-via-`GraphicsParagraph` measurement pattern were found while locating
`wrapped_height`: `shell/tests/editor_column_width.rs`'s `real_width` and
`shell/tests/help_modal_glyph_coverage.rs`'s `shaped_glyph_ids` (both already call
`LineHeight::default()`/`Shaping::default()` correctly, unlike the one closure the start file
named). Not consolidated onto `text_extent` here — the start file's own ask was specifically to
lift `wrapped_height`, and folding in two more already-correct call sites is a separate,
small refactor left for whoever next touches either file, named here so it is not rediscovered as
new duplication.

## C. Two small, independent pieces

**The release-checklist row.** `rfcs/handoffs/release-checklist.md`'s "Run It" section gained a
box for "every expensive, normally-skipped real-UI pass still runs once, on the release commit" —
the per-release half of the rule's "don't repeat an expensive pass on unchanged code" optimization,
which only stays safe if something forces the skipped pass to run at least once. **Checked rather
than assumed: no pass in this project is currently skip-gated on an unchanged-code path filter** —
`ci.yml`, `release.yml` and `docs.yml` each run unconditionally, by their own stated "no `paths:`
filter" design, and Post-Publish Verification's own install/download checks already run
unconditionally every release. The box exists for the day a tier-2 matrix pass (or any other) is
built with that optimization, so it does not skip itself into irrelevance from the day it ships.

**The fixtures folder.** `rfcs/handoffs/fixtures/`, created, with a `README.md` stating the
checksum convention (`checksums.sha256`, one `sha256sum`-style line per fixture, a comment above
each naming where it came from and how to reproduce it) and `.gitignore` entries so the fixture
files themselves are never committed while the README and the checksum manifest are. Empty on a
fresh clone, by design — nothing needed one yet.

## D. The fixed window size at launch — investigated, reported infeasible without a scoped decision

The rule asks for a window size **obtained at launch** — through the app's own setting (ruled out
by the start file: a product change, refused), or inside a nested compositor sized for the
purpose. `main.rs` sets no window size at all; under this tiling host, the layout decides it.

**Tried, in order, on the real desktop, carefully:**

1. **Xephyr / Xnest** (a nested, fixed-size X server — the natural target for this launcher, since
   it already forces the X11 backend): neither installed.
2. **Xvfb** (a headless, fixed-size X framebuffer — not literally a "nested compositor" but would
   answer the same need for X11-backend tests): not installed either.
3. **sway / weston** (an alternative nested Wayland compositor): neither installed.
4. **niri's own nested (`winit`) backend** — documented, supported (`niri --help`'s own
   `--session` flag: "do not set when running as a nested window"), and genuinely tried: spawned
   with its own `XDG_CONFIG_HOME` and a config requesting `output "winit" { mode "1280x720" }`.
   It ran cleanly, isolated, with its own Wayland socket, its own IPC socket, and — a real bonus —
   its own embedded XWayland (`:2`), which would have covered both backends from one nested
   instance had this worked. **It ignored the requested mode.** `niri msg outputs` against the
   nested instance's own socket reported `1250x1373`, not `1280x720` — a size matching this
   *host* session's own current tiling layout at the moment it was spawned, not the config's
   request. So the nested compositor's own window size is dictated by the **host's** tiling
   decisions, exactly the problem this was meant to escape, unless the host also floats/resizes
   that one outer window to a chosen size before anything launches inside it. Killed and cleaned
   up immediately after measuring this (`pgrep` for the nested process and its Wayland socket both
   confirmed gone).

**Not attempted: floating or resizing the nested compositor's own outer window.** This project's
standing rule is never to float or resize a window on this shared desktop, because doing so
exposes other sessions' content in whatever is captured next. A nested compositor's own window,
resized once while it is still empty (before anything renders inside it), would not itself expose
another session's content the way resizing *tekstide's own* window does — but the existing rule
is written without that exception, and this slice's own boundaries ("do not add an app-side
window-size setting... if you disagree, raise it as a proposal") extend the same caution to not
quietly carving an exception out of a different standing rule either. Not done here; named as the
one concrete thing that would unblock this path if the owner wants it.

**Not attempted: installing Xephyr or Xvfb.** A system package install on a shared machine other
sessions depend on is the kind of decision this project's own standing rules ask to be raised
rather than made unilaterally, even though neither package is large or unusual.

**Conclusion.** The nested-compositor route is real and well-supported by niri itself, but does
not currently deliver a chosen size on this host without one of the two actions named above, and
neither is mine to decide. Current practice — recording the size `niri msg -j windows` reports,
stated as observed rather than assumed — continues until one of those is authorized. This is
reported rather than quietly worked around, per the start file's own instruction for exactly this
outcome.

## Round 2, 2026-11-11 — review 670's three required changes, each ablated against the reviewer's own probe

**Required 1 — the agreement guards caught a substring, not a shape.**
`theme_text_overrides_nothing_the_harness_assumes_is_default` and the pre-existing
`theme::tests::the_theme_button_applies_the_theme_style` both asserted `flat.contains(expected)`,
which a chained call survives (`.contains` is still true once something is appended). Review 670
proved it with a probe appending `.line_height(2.0)` to `theme::text`'s body: the guard stayed
green while production drew every label at a line height the harness did not know about. Both
tests now extract the function's own `{ ... }` body by bracket depth (a small `function_body`
helper, duplicated once per test module rather than shared across two sibling modules for three
lines of reuse) and assert **equality** with the expected expression — nothing chained after it.
Re-ran the reviewer's exact probe against both: `theme::text` with `.line_height(2.0)` appended,
and `theme::button` with `.padding(5)` appended (the same hole, in the sibling test, fixed in the
same pass rather than filed separately, per the review's own ask). Both failed, naming the exact
mismatched body text; both reverted, `theme.rs` md5 `e814acf2…` afterward — identical to both the
reviewer's and this round's own pre-ablation value.

**Required 2 — the mandatory launcher can now produce a Wayland session, not only X11.**
`--backend=x11` (default, unchanged behaviour) or `--backend=wayland` (`WAYLAND_DISPLAY` left as
inherited rather than unset, for `wtype`-driven keyboard tests), printed explicitly either way.
Tested both: `--backend=wayland` launches, prints "backend: Wayland...", and tears down cleanly
via the new scoped `--kill` below; the default path is unchanged and re-verified against the same
three refusal ablations from round 1 (all three still fire, identical messages).

**Required 3 — `--kill` is now scoped to the instance this script started, with the unscoped form
named and explicit.** The launch now writes `<scratch-root>/launcher.pid` (`<pid> <binary-name>`,
written just before `exec`). `--kill <scratch-root>` reads it, confirms `/proc/<pid>/comm` still
names that exact binary (guarding against pid reuse after the original process already exited),
kills only that pid, and removes the pid file. `--kill --by-name [binary]` is the old, unscoped
`pkill -x`, kept as an explicit opt-in with a printed warning ("this stops every matching process
on the machine... other agents' sessions can be running the same name"), using the same
`basename`-derived name the launch pre-flight uses — closing the second defect review 670 found
(the pre-flight and the old bare `--kill` disagreeing on where the name came from).

**All three exercised against the real binary, not only reasoned about:**

- Launch, then a second launch attempt while the first runs: refuses, naming the new scoped-kill
  command in its own message.
- Scoped `--kill <scratch-root>`: killed exactly the recorded pid, confirmed by `pgrep -x tekstide`
  finding nothing afterward and the pid file being removed.
- `--kill --by-name`: printed its own warning, then killed by name, confirmed empty via `pgrep`.
- The pid-reuse guard: killed the launched process directly (bypassing the script, simulating it
  having already exited on its own), then called the scoped `--kill` against the now-stale pid
  file — it correctly reported "pid ... is not running as 'tekstide' any more -- nothing to kill"
  rather than risking a different process that happened to reuse the pid.
- `--kill <scratch-root>` against a scratch root nothing was ever launched against: reports
  plainly that there is nothing to do, exit 0.

**Gate, re-run after all three fixes.** `cargo fmt --check`, `clippy -D warnings`,
`RUSTDOCFLAGS="-D warnings" cargo doc` — all clean. Full workspace suite, `--no-fail-fast`, short
`TMPDIR`: unchanged counts from round 1 (`tekstide` 799/0/8, `rfc_docs_invariants` 28/0,
`tekstide-core` 1190/0/4) — the Required-1 fix changed two existing tests' own assertions, not
the count. `pgrep -x tekstide` empty before and after every real launch in this round.

**Taken to the owner, not acted on here, per the review's own framing:**

- **D is not closed; a real route exists and was found by the reviewer, not tried here.** A niri
  `window-rule` matching this app's `app-id` (or the nested compositor's own outer window) with a
  `default-column-width`/`default-window-height` can give a window its size **at map time** —
  neither floating nor resizing a running window, so it does not need the exception round 1's own
  write-up declined to carve out of the standing rule. This edits the owner's own live compositor
  configuration, so it is the owner's to authorise, not this slice's to do. Left open; nothing
  further spent on it per the review's own instruction ("don't spend on it until then").
- **The Testing Guidelines' AT-SPI sentence is now a real conflict, not only a gap.** Round 1
  found AT-SPI unavailable for this app (no accessibility feature in `iced`'s `Cargo.toml`); review
  670 found the consequence sharper than "unavailable" — as written, the guideline would forbid
  keystrokes for any non-keyboard test and point at a mechanism this app cannot provide at all,
  which blocks every mouse/interaction evidence path. That file is the owner's; the review is
  raising it with them directly, so no action is taken here.

## Round 3, 2026-11-11 — the one remaining required fix, and the owner's two decisions

**Required 4 — `--backend=wayland` now refuses rather than claims a backend it did not get.**
Review 670 round 2 found that requesting `--backend=wayland` with `WAYLAND_DISPLAY` unset or
empty printed "backend: Wayland" and proceeded anyway, landing a false backend claim in whatever
evidence sidecar quoted it. Fixed with the same discipline the two XDG guards already have: a
`REFUSING:` line and exit `1` when `--backend=wayland` is requested and `WAYLAND_DISPLAY` is
unset or empty. Ablated: ran `--backend=wayland` with `WAYLAND_DISPLAY` explicitly unset against
`/bin/true` (so nothing launches even if the refusal failed to fire) — refused correctly, naming
the exact reason. Re-ran the positive case (`WAYLAND_DISPLAY` set, a real launch and a scoped
`--kill` teardown) and the three original XDG refusal ablations — all unaffected.

**The owner's two decisions, both applied where they are mine to apply:**

1. **The Testing Guidelines' AT-SPI sentence is amended by the owner, in their own file** — an
   ordered three-part rule (AT-SPI unavailable, named with cause; the keyboard as the primary
   route since it usually exists by this app's own design; coordinate clicks via `xdotool` on X11
   only where no keyboard route exists, from a measured layout, never on a destructive control,
   always under "assert, then continue"). Read there, not duplicated here; this handoff's own job
   is to follow it, not restate it.
2. **D is authorised, scoped to the nested compositor's own window, never to `tekstide`'s own
   `app-id`** — a niri `window-rule` the owner will add to their own config themselves (not edited
   here). **Confirmed, on a nested instance spawned and torn down for this purpose alone, exactly
   as asked**: the nested compositor's own window reports `app_id: "niri"` on the host's own
   `niri msg -j windows` — matching the owner's own expectation (`^niri$`) exactly, not merely
   assumed. The nested instance and its Wayland socket were both confirmed gone immediately after
   this one check; no other window's details from that listing are recorded here or anywhere else
   (the host's `windows` list also names other sessions' own real window titles, out of scope for
   this handoff and not reproduced).

**D's implementation is deferred to its own round, per the owner's own explicit discretion** ("if
[the owner's config change] has not landed when you are ready, land Required 4, push it, and
treat D as its own round rather than holding a green slice") — whether it has landed was not
checked (reading the host's own compositor config file directly was outside this round's scope),
so this slice closes with Required 4 and leaves D for the next one.

**Gate, re-run after the fix.** `cargo fmt --check`, `clippy -D warnings`, `RUSTDOCFLAGS="-D
warnings" cargo doc` — all clean. Full workspace suite, `--no-fail-fast`, short `TMPDIR`:
unchanged counts (`tekstide` 799/0/8, `rfc_docs_invariants` 28/0, `tekstide-core` 1190/0/4) — this
round touched only the launcher script, no Rust tests. `pgrep -x tekstide` empty before and after
every real launch in this round, including the app-id confirmation's own nested-niri probe.
