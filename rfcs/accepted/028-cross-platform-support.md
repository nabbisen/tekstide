# RFC-028: Cross-Platform Support — macOS

Status: **Accepted by the human owner 2026-10-15.** D1–D9 as written; **the Run It question
answered by the acceptance itself** — see *Decided on acceptance*. `0.38.0` in the authorised
schedule. Proposed 2026-10-14. Scoped against what the current code actually assumes, not a
hypothetical platform matrix — found by a four-part investigation (process/PTY, IPC/sockets,
filesystem/paths/watcher, GUI/clipboard/packaging) whose load-bearing claims were independently
re-derived against vendored dependency sources, not taken on report alone. Returned twice before
this draft: review 566 found D5's inventory incomplete (re-scoped against a full
`libc::`/`/proc`/`SYS_` sweep); review 567 found the keybinding layer missing entirely (D9) and no
acceptance story for a platform nobody here can run Run It on. `0.37.0` was first of M14; this is
the second. Reserved 2026-09-24 from the requirements/roadmap/GUI audit of 2026-09-23.

## Summary

**Tekstide is Linux-only today, and not by a thin margin.** Every subsystem that touches a
process, a PTY, or an IPC channel is unconditionally Unix-coded with no `cfg` gate at all — code
that will not *compile* on a non-Linux target, not merely behave differently on one. Clipboard,
fonts, and locale resolution are the opposite: already fully cross-platform, entirely through
dependencies (`iced`, `cosmic-text`, `sys-locale`) Tekstide never had to write platform-specific
code for. **The one real GUI-layer gap is keybindings** (D9): a Tekstide-authored, Ctrl-shaped
rules table with zero handling of the Command/⌘ modifier anywhere, found only on a second review
pass — the primary interaction surface of a GUI-first product, not a dependency's problem to have
already solved. This RFC closes the gap for **macOS only**. Windows is reserved as its own future
RFC (D1) — not because the individual primitives are harder, but because Windows has no design to
port *to*: no process groups, no POSIX signals, no `AF_UNIX`. macOS has all three, so RFC-043's
containment model (session leader → enumerate members → escalate) ports as the same model with
different system calls underneath, which is a materially different, and materially smaller, kind
of work.

## What the current code actually assumes

**Every `libc::`/`/proc`/`SYS_*` symbol in production code, swept mechanically** (`grep -rhno
'libc::[A-Za-z_0-9]*'` plus a search for raw syscalls and `/proc` paths), **sorted into portable
and not:**

| Mechanism | Site | Portable to macOS as-is? | macOS equivalent if not |
| --- | --- | --- | --- |
| `/proc/<pid>/stat` session-member enumeration | `runtime/terminal/launch.rs:808-879`, `termination.rs` | No | `sysctl(KERN_PROC, KERN_PROC_PGRP, …)` or `libproc` |
| `libc::syscall(SYS_close_range, …)` | `runtime/terminal/launch.rs:1152` | No | `closefrom(3)` — simpler, no raw-syscall fallback needed |
| `libc::eventfd`/`EFD_CLOEXEC` | `runtime/terminal/reader.rs:650,674` | **No — the symbol does not exist on Apple targets at all** (confirmed against the `libc` crate's own source tree: no `eventfd` in any Apple/BSD module) | self-pipe or kqueue `EVFILT_USER` |
| `libc::syscall(SYS_gettid)` | `runtime/terminal/reader.rs:484` | **No — `SYS_gettid` is undefined for Apple targets** (confirmed the same way; defined per-architecture under `linux_like` only) | `pthread_threadid_np`, different return type, no raw-syscall form |
| `SO_PEERCRED`/`libc::ucred` peer-credential check | `approval/channel.rs:1106-1120` | No | `LOCAL_PEERCRED`/`xucred` — different option level (`SOL_LOCAL`, not `SOL_SOCKET`) and a `cr_version` field that must be checked against `XUCRED_VERSION` |
| `/proc/self/fd/<fd>` TOCTOU-safe bind path | `approval/channel.rs:170,402` | No (`/proc` does not exist on macOS) | `/dev/fd/<fd>` is the plausible BSD-heritage analogue — **must be verified on a real macOS machine that `bind()` through it is still TOCTOU-safe (the same open file, not a re-open by name) before this RFC commits to it; this is an open question, not a decision** |
| `/proc/meminfo` | `measurement.rs:506` | No, already self-labelled "Linux-only... fails silently elsewhere" | Out of this RFC's scope — memory-pressure measurement is not required for correctness on macOS |
| `libc::ioctl(fd, libc::TIOCSCTTY, 0)` | `runtime/terminal/launch.rs:1133` | **Measured, PR-028-A, 2026-10-15: no — a type mismatch, not a missing symbol.** `TIOCSCTTY` itself exists on macOS; `ioctl`'s own `request` parameter is `u64` there against `u32` on Linux. Never caught by the original symbol-presence sweep, since the symbol genuinely exists on both platforms | `libc::TIOCSCTTY.into()` (rustc's own suggested fix) — mechanically plausible, functional correctness on real hardware not yet verified |
| `kill`, `SIGHUP`/`SIGKILL`/`SIGTERM`, `fcntl`, `poll`/`pollfd`/`POLLIN`, `TIOCSWINSZ`/`winsize`, `setsid`, `waitpid`, `openat`/`O_NOFOLLOW`/`O_DIRECTORY`/`O_CLOEXEC`, `geteuid`, `recv` w/ `MSG_PEEK`\|`MSG_DONTWAIT`, `sockaddr_un` | throughout `runtime/terminal/`, `approval/channel.rs`, `project_lock/` | **Yes, unchanged** | — |
| `posix_openpt`/`grantpt`/`unlockpt`/`ptsname_r` with `O_CLOEXEC` requested atomically at each `open()` (review 579's fix, replacing `openpty`'s own non-atomic two-step `fcntl` — see D5) | `runtime/terminal/pty.rs` | **Unverified — not assumed.** The function names match POSIX on both platforms, but the property that matters is not presence, it is whether macOS's `posix_openpt` actually *honours* `O_CLOEXEC` the same way Linux's `/dev/ptmx` open does; it has not always | Must be verified on the real `macos-latest` runner with the same concurrent-fork test Linux now has (`no_concurrently_forked_sibling_inherits_a_pty_master_fd`), not inferred from the flag being accepted without error |

**`std::fs::File::try_lock()`** (`project_lock.rs`, `audit/store.rs`'s creation-section guard,
`transcript/{writer,loading}.rs`) **is already cross-platform** — stabilized in Rust 1.89, which
`tekstide-core`'s own `rust-version` already requires. No porting work needed for the lock
primitive itself; the `#[cfg(unix)]`-gated permission-bits calls around it (`DirBuilderExt::mode`,
`PermissionsExt::from_mode`) already degrade gracefully (compile fine elsewhere, just skip
restricting permissions) and need no change for macOS specifically, since macOS is `cfg(unix)`.

**Filesystem paths**: `ConfigPathProvider` (`crates/tekstide-core/src/config/path.rs`) already has
real, tested `macos_default()`/`macos_from_env()` and `windows_default()`/`windows_from_env()`
constructors (lines 48-76) — **dead code**, since no `#[cfg(target_os = ..)]` dispatcher exists
anywhere and every call site picks `linux_default()` unconditionally (confirmed: a repo-wide grep
for `cfg(target_os` returns exactly one hit, a doc comment explaining why no dispatcher exists).
`AppStatePathProvider` (`project/recent/store.rs`) is **asymmetric** — it has no macOS or Windows
constructor at all, a gap this RFC closes from scratch rather than wires up.

**The file watcher has a latent, verified compile-time bug, unrelated to any new code this RFC
would write**: root `Cargo.toml`'s `notify = { version = "8.2.0", default-features = false }`
disables `notify`'s own `default = ["macos_fsevent"]`. Traced directly in the vendored source
(`notify-8.2.0`): `src/lib.rs:204` gates `pub mod fsevent` on `cfg(all(target_os = "macos",
not(feature = "macos_kqueue")))` — **not** on `macos_fsevent` — so with neither feature enabled,
`fsevent.rs` still compiles on macOS and its line 21 unconditionally does `use fsevent_sys as fs;`,
while `fsevent-sys` (`optional = true`) is pulled in **only** by the disabled `macos_fsevent`
feature. Net effect: `error: can't find crate for fsevent_sys` on any macOS build today, with
nothing in this RFC's own code touched. Not a reason to patch the manifest standalone (D2) — there
is no way to verify the fix works without a macOS build to run it against, and this machine has
none — but it is fixed in this RFC's first slice, where the macOS CI job can actually prove it.

**GUI, clipboard, fonts, locale: nothing to do.** Clipboard goes through `iced::clipboard::read`;
fonts through `cosmic-text`/`fontdb` via `iced_graphics`; locale resolution through `sys-locale`.
Grepped every call site in `crates/tekstide/src` and found zero Tekstide-authored OS-specific code
in any of the three. (The missing Wayland `app_id` is real but tracked separately in
`future-work.md` and is Linux-desktop-specific, not a macOS blocker.)

## Decisions

- **D1. This RFC covers macOS only. Windows is reserved as its own future RFC.** The distinction
  is not "fewer primitives to replace" — `eventfd` alone is a full parallel implementation on
  macOS too. It is that **macOS keeps the design and Windows has none to keep**: sessions, process
  groups, POSIX signals and `AF_UNIX` all exist on macOS, so RFC-043's containment model (session
  leader → enumerate members → escalate) ports as the same model with different system calls.
  Windows has no process groups and no POSIX signals, so there is no model to port — it needs a
  containment design built against Job Objects from first principles, which is an RFC's worth of
  design work before any code, not a slice inside this one.
- **D2. The `notify` feature-gate bug is fixed in this RFC's own first slice, alongside the macOS
  CI job that can prove it — not landed standalone.** Add `macos_kqueue` (not `macos_fsevent`):
  `kqueue` needs no `FSEvents`-style elevated privilege, and RFC-026 D4's own non-recursive
  per-directory-watch design never needed `FSEvents`' native recursive-tree behaviour. The feature
  is inert on Linux/Windows (it only un-gates code behind `cfg(target_os = "macos")`). Carry the
  `cfg`-chain reasoning into the manifest as a comment, the same way the AUR `PKGBUILD`'s `CFLAGS`
  strip documents its own reason — a bare feature name is exactly what a later cleanup deletes.
- **D3. A platform-dispatch convention is established before any macOS logic lands, and it is one
  convention, not one per type.** `ConfigPathProvider` and `AppStatePathProvider` both get a
  single dispatch point (e.g. `default_for_host()`); `AppStatePathProvider::macos_default()` is
  written from scratch, with the same test pattern its sibling already uses (`_from_env` pure,
  `_default` reading real env vars). Two `cfg` shapes already exist in this tree —
  `audit/recovery.rs`'s whole-function `#[cfg(unix)]`/`#[cfg(not(unix))]` split, and
  `project_lock.rs`'s narrower in-function `#[cfg(unix)]` blocks with `#[cfg_attr(not(unix),
  allow(unused_mut))]` — this RFC picks the whole-function shape for its own three-way splits
  (D4/D5 below, `linux`/`macos`/other, deliberately not a blanket `cfg(unix)`, which would
  silently claim the BSDs and Solaris too) and states that choice once, as the convention: adding a
  future provider without a host dispatch is a review-gate failure, not a style preference.

  **The convention also has to say which existing `linux_*` names are a dispatch point and which
  are not, since the tree has more of them than the two providers above.** Read all six:

  - **`LinuxTerminalRuntime`** (`runtime/terminal/launch.rs:26`, threaded through `app.rs`,
    `project/session.rs`, named in `transcript/writer.rs`'s docs) names the concrete type D5
    replaces mechanisms *inside*. Landing macOS code in a type called `Linux…` is exactly the
    label-that-means-nothing this decision exists to abolish, one layer down. **Renamed to
    `TerminalRuntime`, not `UnixTerminalRuntime`** — required correction, review 568: `Unix` claims
    the BSDs and Solaris too, the same over-claim D3's and D4's own three-way `linux`/`macos`/other
    splits exist to avoid, just in a type name rather than a `cfg`. No platform adjective at all is
    the honest name for a type whose actual coverage is whatever its internal `cfg`s say — and it
    sits correctly beside the module's other exports (`TerminalRuntimeEvent`,
    `TerminalRuntimeError`, `TerminalLaunchError`), with no existing `TerminalRuntime` anywhere in
    the tree to collide with. **This decision owns the rename; PR-028-B performs it** — the
    dispatch-convention slice, where the convention is being written anyway and before D5 starts
    editing the type's internals. Not a trait split: Windows is out of scope (D1), so there is no
    second concrete implementation to abstract over yet, and introducing a trait for one
    implementation would be exactly the kind of abstraction this project's own convention avoids
    building ahead of a real second caller.
  - **Five `linux_mvp()` constructors** (`navigation.rs:348`, `content/open.rs:15`,
    `project/diff.rs:96`, `project/root/explorer.rs:28`, `transcript/reader.rs:106`) hold byte
    limits, directory-ignore lists and keybinding rules — **nothing platform-specific**, confirmed
    by reading all five. "linux" there names the *Linux MVP profile*, not a platform. They are
    **not** wired into the host dispatch this decision establishes, and a reader must be able to
    tell the two kinds apart without opening every file — so this RFC states the distinction once,
    here, rather than leaving it to be rediscovered. One exception: `navigation.rs:348`'s
    `KeybindingPolicy::linux_mvp()` is also the exact table D9 below has to make platform-aware.
    D9's own slice is where this one constructor graduates from "legacy label, pure data" to a real
    dispatch point — consistent with this convention, not an exception to it, since D9 gives it the
    same host-dispatch shape D3 requires of anything that actually varies by platform.
  - **A third kind, found at review 585: `AiCliProfile::claude_code_linux_default()`
    (`agent/profile.rs:79`) is neither of the two above.** Unlike the five `linux_mvp()` labels, it
    is genuinely platform-coupled, not a mislabelled profile name: `claude_code_from_env`
    (`:83-91`) hardcodes `/usr/local/bin` and `/usr/bin` as reviewed system lookup paths -- verified
    directly against the source, not assumed from the name -- which is exactly D8's own
    AI-CLI-bare-command-`PATH`-lookup concern, one level up from the path-literal list D8 already
    names. **Not dispatched here.** Recorded as a known future dispatch point with its own
    resolution deferred to PR-028-F (D8), alongside the `/opt/homebrew/bin` addition that section
    already scopes, so the sweep this decision performs is complete rather than a fourth
    platform-named constructor turning up later as a label nobody re-checked.
- **D4. The approval channel's two Linux-specific mechanisms get macOS-specific replacements, not a
  shared abstraction.** `SO_PEERCRED` → `LOCAL_PEERCRED`/`xucred` (option level `SOL_LOCAL`, not
  `SOL_SOCKET`; `peer_uid` only reads `cred.uid`, so `xucred`'s `cr_uid` suffices and macOS's
  separate `LOCAL_PEERPID` is not needed; `xucred`'s own `cr_version` must be checked against
  `XUCRED_VERSION` before trusting the rest of the struct — an unchecked version field on a
  security boundary is exactly the shape this codebase already catches in review, so it is built in
  from the start, not added after). The `/proc/self/fd` TOCTOU bind path's macOS replacement is an
  **open question with a named venue, not an abstract "real machine"**: there is no macOS hardware
  on this project outside PR-028-A's own `macos-latest` CI runner, so that runner is where it gets
  answered. The check: a test that binds through `/dev/fd/<fd>` (the plausible BSD-heritage
  analogue) and asserts it is the same open file rather than a re-open by name — comparing
  `fstat`'s `st_dev`/`st_ino` before and after, or a write-then-read round trip, not merely that the
  path resolves. `/dev/fd` is written into this RFC only once that assertion passes on the runner;
  until then it is a hypothesis, not D4's decision.
- **D5. Process/PTY: six Linux-only mechanisms, re-scoped against a full sweep rather than the
  three files expected to hold them (review 566's own correction).** Four replaced in-kind
  (`/proc` enumeration → `sysctl`/`libproc`; `SYS_close_range` → `closefrom`; `eventfd` → self-pipe
  or kqueue `EVFILT_USER`; `SYS_gettid` → `pthread_threadid_np`), one already portable and only
  needing its link target confirmed (`openpty`: `libutil` on glibc, `libSystem` on macOS — check,
  don't infer), one out of scope (`/proc/meminfo`, already self-labelled). **Correction, review
  579: `openpty` is no longer the call site at all.** A fork/exec race in the Linux PTY path (a
  sibling thread's `fork` landing in the window between fd creation and a follow-up
  `fcntl(FD_CLOEXEC)`, demonstrated directly — 7,470 leaked descriptors across 46,143 checks against
  236 forked children) was fixed by replacing `libc::openpty` with `posix_openpt`/`grantpt`/
  `unlockpt`/`open(ptsname_r(...))`, requesting `O_CLOEXEC` atomically at each `open()` call instead
  of as a separate step. **"Portable" was the wrong property to have scoped `openpty` on;
  atomicity of `CLOEXEC` is the one that matters, and it is not a given across platforms**: macOS's
  own `posix_openpt` has not always honoured `O_CLOEXEC` the same way Linux's `/dev/ptmx` open does.
  PR-028-D must *verify* this on the real `macos-latest` runner — the same discipline D4's `/dev/fd`
  question already uses — via a test structured like `tekstide-core`'s own
  `no_concurrently_forked_sibling_inherits_a_pty_master_fd` (a bounded, wall-clock-capped
  concurrent-fork check, not a static read of the flag), not inferred from the flag being accepted
  without error. If macOS's `posix_openpt` silently ignores `O_CLOEXEC`, the macOS port needs its
  own, separate close-on-exec application immediately after `open()` returns, accepting a narrower
  version of the exact race just fixed on Linux as a known, stated gap rather than an undiscovered
  one. Everything else in this
  subsystem — `kill`, the three termination signals, `fcntl`, `poll`, the `ioctl` calls, `setsid`,
  `waitpid`, the `openat` TOCTOU chain, `geteuid`, `recv` with `MSG_PEEK`\|`MSG_DONTWAIT`,
  `sockaddr_un` sizing — is POSIX and lands unchanged.
- **D6. Clipboard, fonts, and locale resolution need no GUI-layer work.** All three are already
  fully cross-platform through dependencies; stated explicitly so this is not rediscovered as a
  gap. **Narrower than "no GUI-layer work at all"** — corrected by D9, below, which is real
  GUI-layer scope this decision does not cover.
- **D7. The macOS CI job is added early and non-blocking, not at the end once everything already
  compiles.** A red macOS job here is not noise — it is a live, continuous measurement of distance
  from compiling, replacing this scoping document's one-time snapshot, and it catches the next
  `eventfd`-shaped gap the moment someone writes it rather than at the next audit. Flipped to
  blocking in the slice that makes it green, the same advisory-then-blocking shape `ci.yml` already
  used for its own first run and RFC-072's quarantine job already uses for a different artefact.
  `install.sh:36-37`'s refusal message names RFC-028 by number ("no released build for '$os' yet
  (Linux only -- see RFC-028)") and goes stale the day macOS ships, so updating it is this RFC's
  own job, in the slice that ships the first macOS artifact. **`aarch64-apple-darwin` only for the
  first macOS release** — a second (Intel) artifact doubles the release surface (build, checksum,
  installer branch, post-publish check) for a shrinking minority of machines; adding it later is
  additive, and if the owner wants it, that is one more matrix entry at that point, not a reason to
  hold this one.
- **D8. Four hardcoded Unix-only path literals, three of them no-ops for macOS and one real product
  question.** `/bin/sh` (terminal-pane launch) and `"/"` (browse-dir fallback) are portable as
  literals; `/proc/meminfo` is out of scope per D5's table. The one that matters:
  `config/profile.rs:126-127`'s AI-CLI bare-command `PATH` lookup list (`/usr/local/bin`,
  `/usr/bin`) assumes a shell-launched process. **GUI-launched macOS apps do not inherit the user's
  interactive shell `PATH` at all** — the normal way a user reaches this product on that platform —
  so the lookup fails for exactly the common case, not an edge one. **Recommendation: add
  `/opt/homebrew/bin` (Apple Silicon Homebrew) to the reviewed list as this RFC's own scope**,
  since it is the same class of fix as the two existing entries. **Reading the user's login shell's
  own `PATH` (spawning `$SHELL -ilc 'echo $PATH'` or equivalent) is explicitly deferred to
  `future-work.md`, not decided here** — it is a materially bigger behavioural change (a subprocess
  spawn at launch or boot, with its own timeout/failure/security questions on a boundary this
  project treats carefully) and deserves its own slice-level scrutiny rather than riding in on a
  path-literal list.
- **D9. Keybindings are Tekstide-authored data in Linux/Windows convention, with zero handling of
  the Command/⌘ modifier anywhere — required review correction, review 567: this was missing from
  the first draft entirely.** `KeybindingPolicy::linux_mvp()` (`navigation.rs:348`) is a literal
  rules table (`"Ctrl+Shift+P"`, `"Ctrl+Alt+P"`, the full `Ctrl+Alt+<letter>` set this crate mints),
  matched by 7 call sites across `shell.rs`, `shell/run_report.rs`, `input.rs` and
  `input/terminal_surface.rs` (`key.modifiers.control()`/`press.modifiers.control()` — corrected
  from the review's own count of 8, read line by line rather than counted: the eighth hit its own
  `grep -rn "control()" | wc -l` matched was a **test function name**,
  `activating_a_row_whose_file_has_changed_surfaces_the_reload_control()` — a pattern match inside
  an identifier, not a call site. The 7 named here are read, not counted. **No dependency
  translates this**: grepped the whole tree for `logo()`, `Modifiers::LOGO` and `.command()` and
  found zero hits — the Command/⌘ modifier is not handled in any form. `winit` reports ⌘ as `LOGO`,
  not `control()`, so a macOS build with this table unchanged requires the physical Control key for
  every shortcut (not what a Mac user presses) and makes `Ctrl+click` collide with the platform's
  own secondary-click convention. 15 strings in `locales/en.ftl` name `Ctrl` directly; 99 lines
  across `README.md` and `docs/src` combined do too (8 + 91, both counted directly).

  **This is the primary interaction surface of a GUI-first product, not polish, and it stays inside
  RFC-028 as its own slice before the first artifact ships** — "users must not be able to
  misunderstand" applies hardest at first contact. Bounded scope: one modifier abstraction (the
  rules table stops holding a literal, OS-agnostic modifier name and gains a platform-resolved
  one — this is the slice `KeybindingPolicy::linux_mvp()` graduates into a real D3 dispatch point,
  per that decision's own note), the 7 call sites, the 15 catalog strings, and the documentation
  sweep `release-checklist.md`'s own Book Against The Changelog section already knows how to run.
  **Also checked in this slice**: whether any binding collides with a macOS system shortcut once
  translated to ⌘ (`⌘Q`, `⌘W`, `⌘H`, `⌘M`, `⌘Space` are all claimed by the OS) — a real finding if
  one exists, and cheaper to learn here than from a user. If the bounded scope above prices out
  larger once started, it is split deliberately at that point rather than discovered mid-slice.

## Non-goals

- **Windows support of any kind** — D1; its own future RFC.
- **Intel macOS artifacts** — D7; additive later if the owner wants it.
- **The `locales_dir()`/`CARGO_MANIFEST_DIR` locale-loading defect** — reserved as RFC-073,
  scoped separately; adjacent to D3's own filesystem-path work but distinct (the locale catalog's
  loading path, not config/state directory resolution).
- **Any dependency swap for the terminal runtime.** `alacritty_terminal` 0.26 (already a workspace
  dependency) ships a complete cross-platform PTY abstraction (`src/tty/unix.rs` and a full
  `src/tty/windows/` module, each pulled in via the crate's own per-platform dependency split) that
  Tekstide's own code never imports — confirmed by grepping every `alacritty_terminal::` call site
  in this tree, which reaches only the VTE/terminal-emulator pieces (`Term`, `vte::ansi::Processor`,
  `grid::Dimensions`), never `alacritty_terminal::tty`. RFC-043's own containment model (session-
  leader enumeration, a concrete runtime type renamed by D3 but not restructured) is specific,
  deliberate and already proven; swapping to a library's own PTY layer is a separate, much larger
  decision with its own review, not a side-effect of scoping macOS. Recorded here only because it
  bears on future cost —
  Windows support, whenever its own RFC is written, would not need to build PTY handling from
  nothing if that swap were ever made — which is one more reason D1's split is the right shape,
  not a reason to make the swap now.
- **`/proc/meminfo`-based memory-pressure measurement on macOS** — D5; not required for product
  correctness, a `future-work.md` note if this RFC does not pick it up.
- **Reading the user's login-shell `PATH`** — D8; deferred to `future-work.md` as a known partial
  mitigation beyond this RFC's own scope.

## Acceptance — required correction, review 567: the first draft had no answer for this at all

**What the macOS job runs, by slice.** PR-028-A: `cargo build --workspace` only
(`continue-on-error`) — the measurement slice has nothing to test yet. From PR-028-D onward (once
the six D5 mechanisms are replaced and the tree plausibly compiles end to end), the job adds
`cargo test --workspace --no-fail-fast`, still non-blocking. It flips to blocking, both build and
test, in PR-028-F once genuinely green. **Tests expected to need per-platform gating, named rather
than discovered at the first macOS test run**: `reader.rs`'s own unit tests for
`create_eventfd`/`block_on_eventfd` and the thread-id helper `SYS_gettid` backs; `launch.rs`'s and
`termination.rs`'s process-group-enumeration tests (anything asserting against `/proc/<pid>/stat`
output shape); `channel.rs`'s `SO_PEERCRED`/`peer_uid` tests. **Added since the first draft**:
`tests.rs`'s own `no_concurrently_forked_sibling_inherits_a_pty_master_fd` (review 579/580's PTY
`CLOEXEC` regression test) — its holder scan reads `/proc/<pid>/fd`, which does not exist on macOS
at all, so it is `/proc`-dependent by construction, not merely untested there yet; it did not exist
when this list was first written two days before this correction. Each gets a `#[cfg(target_os =
"linux")]`/`#[cfg(target_os = "macos")]` split testing the same behavioural contract against each
platform's own mechanism, not a shared test disabled on one side.

**The flake register's own answer, decided before it is needed rather than after.**
`test-process-leak.md`'s rows currently carry no platform column because every row has only ever
meant Linux. Once two platforms run the same suite, a row needs to say which platform(s) it was
observed failing on — added as a column (or a per-platform disclosure line within the existing row
format, whichever reads better when the first cross-platform recurrence actually happens) in
PR-028-D, the slice where macOS first starts running real-process tests at all. A failure on one
platform and not the other is not evidence either way about the other platform's own rate; the
register must be able to say so rather than silently conflating two populations the way review
556 corrected for load earlier this release.

**The Run It gate — decided.** Run It is a human, display-attached gate; a CI runner cannot
perform it, and nobody on this project currently has macOS hardware to run it on. **No macOS
artifact ships without either a real-hardware Run It or an explicit label saying it never had
one** — those are the only two shapes, matching `what-the-installer-must-not-do.md` §3's own
discipline of never claiming more than what was actually verified:

1. The owner runs Run It on real Apple hardware before the first macOS artifact tags, the same
   gate every Linux release already requires. **Preferred**: a GUI product's first release on a
   platform is exactly the release most likely to carry a defect only a real screen would show
   (window chrome, font rendering, the D9 keybinding sweep's own correctness), the same reasoning
   that makes Run It a required gate at all on Linux.
2. The artifact ships, explicitly labelled in its own release notes as *not yet run on real
   hardware* — true, in the same register the checksum wording already uses so a reader can see it
   without knowing what to ask, rather than silently implying parity with the Linux release's own
   Run It history. **The stated fallback, not deleted once (1) is confirmed** — a fallback
   considered and rejected is more useful to a future reader than one never named.

Whether the owner has Apple hardware to run (1) on before the first tag is a factual question
reserved to the owner, needed by PR-028-F and not before — it blocks nothing in PR-028-A through E.

## Decided on acceptance (2026-10-15)

**The Run It gate: option (1).** The owner accepted this RFC *with the reviewer's recommendation*,
which was that **Run It is performed on real Apple hardware before the first macOS artifact is
tagged** — the same gate every Linux release already requires. PR-028-F does not close without it.

Option (2) stays written in *Acceptance* above rather than being deleted: if the hardware is not
available when PR-028-F is reached, the artifact ships **labelled in its own release notes as not
yet run on real hardware**, and that is the only other shape this release may take. A fallback that
was considered and ranked is more useful to a later reader than one that was never named — and
leaving it in place means nobody has to re-decide this under schedule pressure.

**Unchanged by the acceptance, recorded so it is not re-opened as an open question:**
`aarch64-apple-darwin` only for the first macOS artifact (D7), Windows reserved as its own future
RFC (D1), and the keybinding slice landing before any artifact ships (D9).

## PR-028-A measured, 2026-10-15 — this replaces the evidence table's point-in-time sweep

`ci.yml`'s `macos-measurement` job ran on a real push (commit `c6b8a87`) and failed, as its own
slice description says it is expected to. **11 compile errors, in order, `cargo build --workspace`
on `macos-latest`:**

1. `approval/channel.rs:1179,1187,1188,1193` (4 sites) — `libc::ucred`/`libc::SO_PEERCRED` don't
   exist; rustc's own suggestion names `libc::xucred` directly, matching D4's own scoping.
2. `runtime/terminal/launch.rs:1152` — `libc::SYS_close_range` doesn't exist; D5's own
   `closefrom` replacement.
3. `runtime/terminal/pty.rs:252` — **`libc::ptsname_r` doesn't exist on macOS.** This is D5's own
   flagged concern (added when review 579/580's `CLOEXEC` fix was still in flight) confirmed as a
   real compile failure, not a hypothetical one. `posix_openpt`/`grantpt`/`unlockpt` themselves
   compiled without error — the gap is `ptsname_r` specifically, not the whole replacement.
   rustc's own suggestion (`ptsname`, the non-reentrant form) is exactly the one review 577 ruled
   out on Linux for thread-safety — a macOS port needs its own reentrant equivalent, not a silent
   fallback to the unsafe one.
4. `runtime/terminal/reader.rs:484` — `libc::SYS_gettid` doesn't exist; D5's own
   `pthread_threadid_np` replacement.
5. `runtime/terminal/reader.rs:674` (2 errors) — `libc::eventfd`/`libc::EFD_CLOEXEC` don't exist;
   D5's own self-pipe/kqueue replacement.
6. **`runtime/terminal/launch.rs:1133` — `libc::ioctl(ctty_fd, libc::TIOCSCTTY, 0)`: a type
   mismatch, not a missing symbol.** `TIOCSCTTY` itself exists on macOS, but `ioctl`'s own `request`
   parameter is `u64` there against `u32` on Linux — a new finding, not named in D5's original six.
   Added to D5's own table below. rustc's own suggested fix (`.into()`) is almost certainly right
   but unverified against actual ioctl behavior on real hardware, since the semantics of the
   conversion are mechanical, not functional.

**D5's own mechanism table gains a seventh row** for the `TIOCSCTTY` type mismatch, found only by
this real measurement, not by the original `libc::`/`/proc`/`SYS_*` sweep (a symbol that exists on
both platforms with a different parameter type was never going to show up in a presence/absence
grep). PR-028-B/D's own scope grows by exactly this one line; nothing else in D3/D4 changes.

Measured, not assumed: `nproc`/`pid_max` on this runner type were not captured this slice (not
needed for a build-only measurement); if PR-028-D's own test-splitting work needs them, capture
then, not now.

- **PR-028-A — the measurement, before the fix.** Add a `macos-latest` job to `ci.yml`,
  **non-blocking** (`continue-on-error`), running `cargo build --workspace` at minimum. Add the
  `macos_kqueue` feature to the `notify` dependency (D2), with the `cfg`-chain reasoning carried as
  a manifest comment. This slice is not expected to turn the job green — it exists to measure, from
  here on, exactly how far macOS is from compiling, rather than relying on another point-in-time
  audit.
- **PR-028-B — the platform-dispatch convention** (D3). `ConfigPathProvider`'s and
  `AppStatePathProvider`'s host dispatch, `AppStatePathProvider::macos_default()` written and
  tested from scratch, the convention documented once as a review-gate rule for future providers.
  `LinuxTerminalRuntime` → `TerminalRuntime` (D3's own rename, performed here — before D5 starts
  editing the type's internals, not inside D5's own slice).

  **Done, 2026-10-15.** `default_for_host()` added to both providers as a whole-function
  `#[cfg(target_os = "linux")]`/`#[cfg(target_os = "macos")]` split, with a `compile_error!` for
  anything neither names — the convention is load-bearing immediately, not decorative: every
  production call site that used to name `linux_default()`/`AppStatePathProvider::linux_default()`
  directly (`main.rs`, `shell.rs`, 7 real call sites total) now goes through the dispatcher instead.
  `AppStatePathProvider::macos_default()`/`macos_from_env()` written from scratch, resolving to the
  *same* `~/Library/Application Support/tekstide` directory `ConfigPathProvider`'s own macOS
  resolution already uses — a deliberate choice, documented at the definition: unlike XDG's
  config/state split, Apple's own convention draws no such distinction, so `config.toml`,
  `recent-projects.json`, and the audit store's subdirectory share one folder on macOS where Linux
  keeps two. `TerminalRuntime` rename: 112 occurrences across 17 files, a pure identifier rename
  (confirmed no existing `TerminalRuntime` to collide with before starting), workspace builds and
  the full test suite passes unchanged. One pre-existing source-scanning test
  (`the_configuration_comes_from_the_resolved_user_path_and_no_other`, `shell/tests.rs`) had to be
  updated — it asserted the literal text `"ConfigPathProvider::linux_default("` appears exactly once
  in production code outside `config/path.rs`; corrected to assert `"ConfigPathProvider::
  default_for_host("` instead, since that is now genuinely the only production call.
- **PR-028-C — the IPC replacements** (D4). `LOCAL_PEERCRED`/`xucred` with its version check; the
  `/dev/fd` bind-path question resolved on PR-028-A's own `macos-latest` runner — the only macOS
  machine this project has — via a test asserting `fstat`'s `st_dev`/`st_ino` identity (or a
  write-then-read round trip) through the bind path, not assumed from BSD heritage.
- **PR-028-D — the process/PTY replacements** (D5), and the macOS job starts running tests. `/proc`
  enumeration → `sysctl`/`libproc`; `SYS_close_range` → `closefrom`; `eventfd` → self-pipe or
  kqueue `EVFILT_USER`; `SYS_gettid` → `pthread_threadid_np`; `posix_openpt`/`grantpt`/`unlockpt`'s
  macOS `O_CLOEXEC` atomicity verified via the Linux regression test's own shape on the real
  `macos-latest` runner, not inferred from the call succeeding (review 579 — see D5). The macOS CI
  job gains `cargo test --workspace --no-fail-fast`
  (still non-blocking); the per-platform test splits named in Acceptance land with the mechanisms
  they test; `test-process-leak.md` gains its platform column in this same slice, since this is the
  first point a row could ever need one.
- **PR-028-E — the keybindings** (D9). The modifier abstraction, the 7 call sites, the 15 catalog
  strings, the documentation sweep, the macOS-system-shortcut collision check. Lands before the
  first artifact, after the product actually runs on macOS (PR-028-D) so the sweep is checked
  against real behaviour rather than planned behaviour.
- **PR-028-F — green, and the first artifact.** Flip the macOS CI job to fully blocking once
  PR-028-A through E are all landed and it is actually green, build and test both. `install.sh`'s
  refusal message updated (it currently names this RFC by number as the reason macOS is refused).
  The Run It gate's own answer (Acceptance) resolved before this slice closes, not assumed. First
  macOS release artifact, `aarch64-apple-darwin` only (D7). `config/profile.rs`'s AI-CLI lookup
  list gains `/opt/homebrew/bin` (D8), and `agent/profile.rs`'s own `claude_code_from_env` gains a
  macOS dispatch arm alongside it (D3's own third-kind note, above). **Required, found at review
  585: `docs/src/users/local-data-and-privacy.md` and `docs/src/users/configuration.md` both name
  only Linux paths** (`$XDG_STATE_HOME`/`$XDG_CONFIG_HOME` and their `~/.local/state`/`~/.config`
  fallbacks) — every one of them wrong for a macOS user once this RFC's own PR-028-B gave
  `AppStatePathProvider`/`ConfigPathProvider` a real macOS resolution. Not owed by any earlier
  slice's own docs sweep (D9's covers the keybinding strings only); update both files to state the
  macOS path alongside the Linux one before the first macOS artifact ships, not discovered at the
  release gate.

PR-028-C and PR-028-D may land in either order; PR-028-A and PR-028-B must land first; PR-028-E
needs PR-028-D's own macOS test run to check its collision sweep against, not just its own code.
