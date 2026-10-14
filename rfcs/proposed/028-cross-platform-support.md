# RFC-028: Cross-Platform Support — macOS

Status: **Proposed 2026-10-14.** Scoped against what the current code actually assumes, not a
hypothetical platform matrix — found by a four-part investigation (process/PTY, IPC/sockets,
filesystem/paths/watcher, GUI/clipboard/packaging) whose load-bearing claims were independently
re-derived against vendored dependency sources, not taken on report alone. D5 was returned once
(review 566) for an incomplete inventory and re-scoped against a full `libc::`/`/proc`/`SYS_`
sweep before this draft. `0.37.0` was first of M14; this is the second. Reserved 2026-09-24 from
the requirements/roadmap/GUI audit of 2026-09-23.

## Summary

**Tekstide is Linux-only today, and not by a thin margin.** Every subsystem that touches a
process, a PTY, or an IPC channel is unconditionally Unix-coded with no `cfg` gate at all — code
that will not *compile* on a non-Linux target, not merely behave differently on one. GUI,
clipboard, fonts, and locale resolution are the opposite: already fully cross-platform, entirely
through dependencies (`iced`, `cosmic-text`, `sys-locale`) Tekstide never had to write
platform-specific code for. This RFC closes the gap for **macOS only**. Windows is reserved as its
own future RFC (D1) — not because the individual primitives are harder, but because Windows has
no design to port *to*: no process groups, no POSIX signals, no `AF_UNIX`. macOS has all three, so
RFC-043's containment model (session leader → enumerate members → escalate) ports as the same
model with different system calls underneath, which is a materially different, and materially
smaller, kind of work.

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
| `kill`, `SIGHUP`/`SIGKILL`/`SIGTERM`, `fcntl`, `poll`/`pollfd`/`POLLIN`, `ioctl`/`TIOCSCTTY`/`TIOCSWINSZ`/`winsize`, `openpty`, `setsid`, `waitpid`, `openat`/`O_NOFOLLOW`/`O_DIRECTORY`/`O_CLOEXEC`, `geteuid`, `recv` w/ `MSG_PEEK`\|`MSG_DONTWAIT`, `sockaddr_un` | throughout `runtime/terminal/`, `approval/channel.rs`, `project_lock/` | **Yes, unchanged** | — (one check owed, not assumed: `openpty` lives in `libutil` on glibc and `libSystem` on macOS — confirm the link succeeds, don't infer it from the matching signature) |

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
- **D4. The approval channel's two Linux-specific mechanisms get macOS-specific replacements, not a
  shared abstraction.** `SO_PEERCRED` → `LOCAL_PEERCRED`/`xucred` (option level `SOL_LOCAL`, not
  `SOL_SOCKET`; `peer_uid` only reads `cred.uid`, so `xucred`'s `cr_uid` suffices and macOS's
  separate `LOCAL_PEERPID` is not needed; `xucred`'s own `cr_version` must be checked against
  `XUCRED_VERSION` before trusting the rest of the struct — an unchecked version field on a
  security boundary is exactly the shape this codebase already catches in review, so it is built in
  from the start, not added after). The `/proc/self/fd` TOCTOU bind path's macOS replacement is an
  **open question with a named check**, not a decision: `/dev/fd/<fd>` is the plausible analogue,
  verified on a real macOS machine before this RFC commits to it, specifically checking that
  binding through it is the same open file rather than a re-open by name (the property the Linux
  mechanism actually depends on, not merely that the path exists).
- **D5. Process/PTY: six Linux-only mechanisms, re-scoped against a full sweep rather than the
  three files expected to hold them (review 566's own correction).** Four replaced in-kind
  (`/proc` enumeration → `sysctl`/`libproc`; `SYS_close_range` → `closefrom`; `eventfd` → self-pipe
  or kqueue `EVFILT_USER`; `SYS_gettid` → `pthread_threadid_np`), one already portable and only
  needing its link target confirmed (`openpty`: `libutil` on glibc, `libSystem` on macOS — check,
  don't infer), one out of scope (`/proc/meminfo`, already self-labelled). Everything else in this
  subsystem — `kill`, the three termination signals, `fcntl`, `poll`, the `ioctl` calls, `setsid`,
  `waitpid`, the `openat` TOCTOU chain, `geteuid`, `recv` with `MSG_PEEK`\|`MSG_DONTWAIT`,
  `sockaddr_un` sizing — is POSIX and lands unchanged.
- **D6. No GUI-layer work is in scope.** Clipboard, fonts and locale resolution are already fully
  cross-platform through dependencies; stated explicitly so this is not rediscovered as a gap.
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
  leader enumeration, a named `LinuxTerminalRuntime` type) is specific, deliberate and already
  proven; swapping to a library's own PTY layer is a separate, much larger decision with its own
  review, not a side-effect of scoping macOS. Recorded here only because it bears on future cost —
  Windows support, whenever its own RFC is written, would not need to build PTY handling from
  nothing if that swap were ever made — which is one more reason D1's split is the right shape,
  not a reason to make the swap now.
- **`/proc/meminfo`-based memory-pressure measurement on macOS** — D5; not required for product
  correctness, a `future-work.md` note if this RFC does not pick it up.
- **Reading the user's login-shell `PATH`** — D8; deferred to `future-work.md` as a known partial
  mitigation beyond this RFC's own scope.

## Slices

- **PR-028-A — the measurement, before the fix.** Add a `macos-latest` job to `ci.yml`,
  **non-blocking** (`continue-on-error`), running `cargo build --workspace` at minimum. Add the
  `macos_kqueue` feature to the `notify` dependency (D2), with the `cfg`-chain reasoning carried as
  a manifest comment. This slice is not expected to turn the job green — it exists to measure, from
  here on, exactly how far macOS is from compiling, rather than relying on another point-in-time
  audit.
- **PR-028-B — the platform-dispatch convention** (D3). `ConfigPathProvider`'s and
  `AppStatePathProvider`'s host dispatch, `AppStatePathProvider::macos_default()` written and
  tested from scratch, the convention documented once as a review-gate rule for future providers.
- **PR-028-C — the IPC replacements** (D4). `LOCAL_PEERCRED`/`xucred` with its version check; the
  `/dev/fd` bind-path question resolved on a real macOS machine before being written as a decision,
  not assumed from BSD heritage.
- **PR-028-D — the process/PTY replacements** (D5). `/proc` enumeration → `sysctl`/`libproc`;
  `SYS_close_range` → `closefrom`; `eventfd` → self-pipe or kqueue `EVFILT_USER`; `SYS_gettid` →
  `pthread_threadid_np`; `openpty`'s macOS link target confirmed, not inferred.
- **PR-028-E — green, and the first artifact.** Flip the macOS CI job to blocking once PR-028-A
  through D are all landed and it is actually green. `install.sh`'s refusal message updated (it
  currently names this RFC by number as the reason macOS is refused). First macOS release artifact,
  `aarch64-apple-darwin` only (D7). `config/profile.rs`'s AI-CLI lookup list gains
  `/opt/homebrew/bin` (D8).

PR-028-C and PR-028-D may land in either order; PR-028-A and PR-028-B must land first.
