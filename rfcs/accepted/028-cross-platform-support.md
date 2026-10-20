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

  **PR-028-C, 2026-10-15: the `LOCAL_PEERCRED`/`xucred` half is done; the `/dev/fd` half is written,
  not yet verified, and a real sequencing dependency on D5 was found doing it.** `peer_credentials`
  split into Linux (`SO_PEERCRED`/`ucred`, unchanged) and macOS (`LOCAL_PEERCRED`/`xucred`, with the
  `cr_version` check this decision specified) arms behind this crate's own cross-platform
  `PeerCredentials` struct (`pid`/`gid` as `Option`, since macOS's mechanism reports neither the
  same way Linux's does); `peer_uid` and the `BindTargetAlreadyListening` diagnostic both updated to
  the new shape. The `/dev/fd` test (`dev_fd_magic_path_follows_the_fd_not_a_renamed_pathname`,
  `approval/tests/channel.rs`) is written — a directory-rename race, not merely a path-resolves
  check — and its logic was sanity-checked by temporarily building it under `#[cfg(target_os =
  "linux")]` (where `/dev/fd` is itself a `/proc/self/fd` symlink, so the test is expected to pass
  there and did). **It cannot run on macOS yet, only compile, and that is a real dependency this
  RFC's own "C and D may land in either order" did not account for**: `cargo build --workspace`
  fails on macOS today (PR-028-A's own 11 errors, all D5's), so no test — this one included — can
  execute on that platform until D5's own sweep makes the crate compile there at all. The hypothesis
  stays unverified, stated as exactly that, until a PR-028-D (or later) CI run can actually execute
  it.

  **Required, review 587: the test must say which of two failure shapes it hit, and the two
  fallbacks are named now rather than discovered if the hypothesis fails.** Updated to report "NOT
  TRAVERSABLE" (the magic path never resolved at all) distinctly from "TRAVERSABLE BUT UNSAFE" (it
  resolved but followed the renamed pathname, not the fd) — the first is the likelier outcome:
  `/dev/fd` on BSD-derived systems is `fdescfs`, which re-opens the file a descriptor names, and is
  not obliged to support `/proc`-style path traversal *through* a directory descriptor
  (`/dev/fd/<n>/<name>`) the way `/proc/self/fd` does on Linux. Two named fallbacks, either usable
  if the real answer is "not traversable" or "unsafe":

  1. **`pthread_fchdir_np` + a relative `bind()`.** Sets the *per-thread* current directory (not the
     process-global one `fchdir` would mutate — unacceptable in a multi-threaded GUI process, where
     it would move every thread's relative paths, not just the caller's), then binds a short
     relative name instead of an absolute magic path — the same TOCTOU-closing property
     `/proc/self/fd` gives on Linux, reached a different way. **Not in the `libc` crate** (checked
     against the vendored Apple module directly): needs this crate's own `extern "C"` declaration,
     and being an `_np` ("non-portable," Apple's own naming convention for such APIs) interface, its
     availability should be confirmed on the real runner rather than assumed from the name matching
     a known Darwin symbol.
  2. **Accept the already-verified directory fd and document the residual.** The `approval`
     directory this binds into is already opened `O_NOFOLLOW|O_DIRECTORY` through a pinned
     `state_root_fd` and `fchmod`ed `0700` (response 113's own existing discipline); what would
     remain exposed is a same-user rename of a directory this process already holds an open fd to —
     narrower than the general TOCTOU class this mechanism exists to close, but a real, stated
     residual risk if chosen, not a mitigation. Must be written as exactly that in whichever slice
     decides between these two, not left implicit.

  **Decided, 2026-10-18-19, reviews 597/600/601/602/603.** `/dev/fd` is dead — confirmed on the
  real runner, "NOT TRAVERSABLE," the likelier outcome named above. **Option 1,
  `pthread_fchdir_np`**, chosen over option 2 for the reason D4 already gave it preference for:
  it preserves the TOCTOU property the Linux path actually has rather than accepting a residual on
  this security boundary specifically. Declared directly (absent from the vendored `libc` crate,
  as expected); `ThreadDirScope`'s own `enter` carries a postcondition (`fstat` on the directory fd
  against `stat(".")` by `st_dev`/`st_ino`) rather than trusting a bare zero return, and its `Drop`
  checks its own revert (`debug_assert_eq!`) rather than discarding it silently.

  **A second, real problem surfaced while wiring it in, independent of the mechanism: `sockaddr_
  un`'s usable `sun_path` is 103 bytes on macOS (104 is the formula's own number, not the true
  limit — `size_of::<sockaddr_un>() - size_of::<sa_family_t>()` never accounted for macOS/BSD's
  extra leading `sun_len` byte; fixed by reading `sun_path`'s own array length directly instead),
  and a real `AgentRunId`-based socket path under a state root as long as `~/Library/Application
  Support/tekstide/approval` does not fit — no bind-time mechanism shortens the path an adapter
  must later `connect()` to, since that path is never resolved relative to anything.** Fixed by
  moving the socket's own directory off `state_root` entirely on macOS (`std::env::temp_dir().
  join("tekstide")`, pinned as its own anchor fd with the same ancestor-swap protection
  `state_root_fd` already gives Linux, and checked for ownership/writability before being trusted
  — `std::env::temp_dir()` falls back to the world-writable `/tmp` when `TMPDIR` is unset) and
  dropping `AgentRunId`'s own `"agent-run-"` prefix from socket filenames on both platforms. D3's
  own `Application Support` decision for *durable* state is untouched; only the socket's own
  directory moves.

  **General rule, so a third socket does not rediscover the same ceiling: every `AF_UNIX` path
  this product binds derives from a short per-user anchor, never from `state_root`, with a
  headroom test asserting comfortable (not merely sufficient) margin on each platform that binds
  it.** `project_lock::attention`'s own socket — a deliberate, documented second copy of this
  module's socket-path-length check (RFC-058 D10's own trade) — had the exact same off-by-one for
  the exact same reason, found and fixed in the same pass, and now inherits this rule too
  (`$TMPDIR/tekstide-attention`, a sibling anchor, not nested inside the approval channel's own).

  **Found in the same pass, and corrected separately rather than absorbed here: the attention
  socket's own knock was never wired into any real open path** (three GUI call sites, one CLI
  path) — only a standalone diagnostic binary ever called `request_attention`. The board's own
  "it has been asked for your attention" sentence was therefore never true on either platform, a
  pre-existing RFC-058 defect that this RFC's own socket-length work happened to surface, not
  create. The false claim is removed here (a thing the product says that is not true is fixed
  before a thing the product does not yet do); wiring the knock itself is reserved as **RFC-075**,
  with RFC-058's own closeout gaining a dated correction recording what its own verification
  actually checked.
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

  **PR-028-D, 2026-10-15: two of six mechanisms done, measured 11 → 6 → 4 on the real runner; two
  more have a real open question each, found doing this, not assumed going in.**

  **Done, confirmed on `macos-latest` itself, not inferred from a header:**
  - `TIOCSCTTY`'s own `ioctl` request-type mismatch (found by PR-028-A's measurement, not in D5's
    original six by name): `ioctl`'s `request` parameter is `c_ulong` on macOS against
    `TIOCSCTTY`'s own `c_uint` there, the same `Ioctl` alias as Linux everywhere else. Widening
    only on the platform that needs it (a bare `.into()` compiles on both but fails Linux's own
    `clippy::useless_conversion` gate, caught by running the gate, not assumed clean).
  - `SYS_gettid` → `pthread_threadid_np(pthread_self(), &mut tid)`, not a `0`/`NULL` sentinel for
    "current thread" (documented for Apple's own API, but `pthread_self()` is unambiguous and
    free). The *only* consumer of this value turned out to be `reader/tests.rs`'s own
    `thread_cpu_ticks`, which reads `/proc/self/task/<tid>/stat` directly — genuinely Linux-only,
    not merely untested on macOS, so `reader_thread_does_not_busy_wait_while_idle` (the Acceptance
    section's own named test) is now `#[cfg(target_os = "linux")]`; a macOS equivalent of the
    *property* (a real `poll(2)` park, not a busy-wait) is not written and is named here rather
    than silently dropped.

  **Done, 2026-10-17, review 593: `eventfd`/`EFD_CLOEXEC` → `kqueue`+`EVFILT_USER`, not a
  self-pipe.** The self-pipe design sketched above (two genuinely distinct fds, Linux `dup`'d from
  one `eventfd`, macOS from a genuinely distinct pipe) was superseded before any code was written
  against it, for a reason specific to this process, not a style preference: neither `pipe2`
  (atomic `O_CLOEXEC`) nor `socketpair`'s `SOCK_CLOEXEC` exists for Apple targets in the vendored
  `libc` crate, and `rustix` — already a transitive dependency — omits its own flagged `pipe_with`
  entirely for Apple rather than ship a racy fallback, confirming this is a genuine platform gap,
  not a crate-coverage one. A self-pipe's specific failure mode under that gap — a concurrently
  `exec`'d child inheriting the write end during the unavoidable `pipe()`-then-`fcntl` window, so
  the read side never sees EOF and shutdown hangs — does not exist at all for a single-fd
  `kqueue` with one `EVFILT_USER` registration, triggered via `kevent(..., NOTE_TRIGGER)`: there is
  no write end for anything to inherit. `polling` (already a transitive dependency) uses the same
  filter for the same reason, by its own account, "to avoid allocating a pipe." Implemented as
  `WakeSignal` (`reader.rs`), a small type exposing only `signal`/`wait` — never a raw fd a caller
  could structure around — backing both the shutdown signal and `WakeNotifier` on both platforms.
  **The contract is stated once, precisely, because the two backends do not agree on more than
  this**: after `signal`, at least one subsequent `wait` observes a wake; no wake count is
  promised, and no one-wake-per-signal guarantee either (`eventfd` counts; `EVFILT_USER` with
  `EV_CLEAR` coalesces). Every existing caller already tolerated spurious/coalesced wakes before
  this change, so none needed updating. The kqueue fd still has no atomic-`CLOEXEC` constructor
  (`fcntl` after `kqueue()`, same two-step `polling` itself uses) — a postcondition check refuses
  construction if that somehow did not take, same shape as `pty.rs`'s own `assert_cloexec`, and the
  residual (a window in which an fd could leak) is an fd leak, not a hang, handled generally by the
  close-loop fix immediately below rather than specifically for this one fd. Not independently
  verifiable without macOS hardware: that a `kqueue` fd is `poll(2)`-able (documented BSD/Darwin
  behaviour this module relies on to keep one `poll_for_readiness` implementation for both
  platforms) and that `kevent(2)` behaves as described are both first proven by the real runner's
  first executable macOS suite, not by anything checked here.

  **Found and fixed the same day, review 593/594: the bounded close loop above (`close(2)`)
  silently broke `Command::spawn()`'s own exec-failure reporting, on both the git-subprocess path
  and this one.** `Command::spawn()`'s `pre_exec` path creates its own pipe before `fork` to report
  a genuine `exec()` failure back to the parent with the real `errno`; that pipe's write-end fd is
  ordinary, not reserved, and sits inside whatever range a loop like this one closes. Closing it
  early makes the parent see `EOF` — std's own "exec succeeded" signal — before `exec` was ever
  attempted, so a target that cannot run is reported as a successful launch, with the real failure
  only surfacing later as a dead child with no diagnostic attached. Found via a real test
  regression (`git_not_found_reports_unavailable_not_a_panic`, when `runtime/git.rs`'s own spawn
  path first gained this loop) and confirmed to hold for `spawn_pty_child`'s pre-existing, identical
  loop too — review 594's own reproduction, built after an initial probe here was spoiled by
  glibc's `ENOEXEC` → `/bin/sh -c` retry (a shebang-less file still starts *something*, masking
  the defect regardless of which loop body runs): a file marked executable with a `#!` shebang
  naming an interpreter that does not exist fails `exec` with `ENOENT`, which is not retried.
  Against that case, `close(2)` reports `spawn() -> Ok` followed by the child dying on `SIGABRT`
  with nothing attached (std's own child-side code aborts on the errno pipe it needed being gone);
  `fcntl` reports `spawn() -> Err(ENOENT)` correctly, immediately. **Fixed in both places by
  marking fds close-on-exec (`fcntl(fd, F_SETFD, FD_CLOEXEC)`) instead of closing them outright**:
  `fcntl` is on POSIX's async-signal-safe list the same as `close` is, gives the identical security
  property for every *other* descriptor (nothing survives a *successful* `exec`), and is a no-op
  against std's own pipe either way (already `CLOEXEC`), so a *failed* `exec` can still report the
  real error through it. Regression tests at both sites now: `git_not_found_reports_unavailable_
  not_a_panic` (pre-existing, caught this first) and `launching_a_shell_with_a_nonexistent_
  interpreter_reports_failure_not_a_dead_child` (new, `terminal/tests.rs`), the latter ablated by
  temporarily restoring `close(2)` and confirming it fails before restoring the real fix.

  (The self-pipe sketch originally here was superseded before any code was written against it --
  see "Done, 2026-10-17, review 593" above for what was actually built and why.)

  **Both open questions answered, review 589 — decisions, not suggestions:**
  1. **`SYS_close_range`/`closefrom`: neither exists on macOS (confirmed: `closefrom` is a BSD-ism
     Apple never adopted, absent from the vendored `libc` crate for Apple targets). Answer: a
     bounded `close(2)` loop instead, with the bound computed in the parent.** (**Superseded
     2026-10-17** — see "Found and fixed the same day, review 593 follow-up" above: the mechanism
     is now `fcntl(F_SETFD, FD_CLOEXEC)` in the same loop, not `close(2)`, for a real correctness
     reason, not a style change.) `close(2)` is on
     POSIX's async-signal-safe list, so a loop of it is legal inside `pre_exec` where
     `proc_pidinfo`-based emulation (the alternative considered and rejected) is not. The bound
     (`sysconf(_SC_OPEN_MAX)`, clamped both ends against an error return and a pathological limit)
     is computed before the `fork`, since `sysconf` itself is not async-signal-safe, and moved into
     the closure. **Implemented and done**: this replaces the Linux-only raw `syscall(SYS_close_range,
     …)` entirely with one mechanism for both platforms, removing that path's own glibc-version
     caveat as a side effect. Framing kept exactly as before: a bonus layer over `OpenPty::new`'s
     own `O_CLOEXEC`, not something a spawn may fail over.
  2. **`ptsname_r`: review 589's first answer (`ptsname()` under a crate-owned lock) was superseded
     by review 590 once a later check refined the premise it rested on — `ptsname_r` is not absent
     from Apple's own libSystem, only from the Rust `libc` crate's Apple module.** Confirmed in two
     independent places: Apple's own open-source `Libc`, and `rustix` (already a transitive
     dependency of this workspace via `alacritty_terminal`, both vendored versions present in this
     tree's own `Cargo.lock`), whose own backend (`src/backend/libc/pty/syscalls.rs`) calls the real
     symbol on Apple targets via `weak!` linkage specifically to tolerate macOS versions older than
     10.13.4, falling back to the `TIOCPTYGNAME` ioctl only then. **Decided: a direct, non-weak
     `extern "C"` declaration of the real POSIX signature**, not a dependency on `rustix` for one
     function (a D-level decision, not a slice detail, per review 590) and not a guessed ioctl
     value. **Safe only because of D7's own `aarch64-apple-darwin`-only restriction on the first
     artifact** — Apple Silicon cannot run anything older than macOS 11.0, four majors past the
     10.13.4 floor `rustix`'s own `weak!` linkage exists to tolerate. **If an Intel
     (`x86_64-apple-darwin`) artifact is ever added** (D7 defers this, does not rule it out, and
     Intel Macs can run 10.13.0-10.13.3, where this symbol is genuinely absent), this floor no
     longer holds on its own — whoever adds that artifact must either re-establish a floor above
     10.13.4 for it or switch this declaration to `weak!` linkage the way `rustix` already does,
     stated at the declaration site itself (`pty.rs`), not only here. **Review 589's own lock-based
     answer is kept, not deleted, as the rejected alternative**: a real residual the chosen answer
     does not have (the lock only ever protected this crate's own calls, not a hypothetical
     concurrent caller elsewhere in the dependency tree), recorded so the reasoning already exists
     if the declaration above ever needs to fall back to it.

  **Both implemented and confirmed on `macos-latest` itself, not assumed from local Linux
  testing**: 11 (PR-028-A) → 6 (PR-028-C) → 4 (`TIOCSCTTY`/`SYS_gettid`) → 3 (close-range) → done,
  this slice, for `ptsname_r`. **Gate**: `cargo fmt --check`, `clippy -D warnings`, full workspace
  test suite (three consecutive runs), all clean on `ubuntu-latest`. `eventfd`/`EFD_CLOEXEC` — its
  own slice, by design, agreed in review 589 — is the only mechanism left; once it lands, this is
  the first point PR-028-C's own `/dev/fd` TOCTOU question (held, unverified, since that slice) can
  finally execute at all.
  **Found while making the macOS job's own test suite runnable, review 596/609 — not a D5
  mechanism itself, but surfaced by D5's work and recorded here since this is where the macOS
  coverage story lives.** `HostileFixture::build` (RFC-052's own hostile-filename fixture,
  `project/root/explorer/hostile_fixture.rs`) and four standalone tests each built a filename
  whose bytes are not valid UTF-8 and unconditionally `.unwrap()`ed creating it. APFS validates
  UTF-8 in filenames where ext4 accepts arbitrary bytes; every one of those writes panicked with
  `EILSEQ` on the real runner (28 panics, five locations) — a filesystem guarantee, not a product
  regression. Two different fixes for two different shapes, not one: the shared fixture (24 of the
  28, every test where the hostile name is incidental to what it checks) now tolerates exactly
  this refusal and records which name was refused on a new `escaping_refused` field, rather than
  silently building a fixture with fewer hostile rows — a new test
  (`the_non_utf8_escaping_row_is_recorded_as_refused_on_a_filesystem_that_cannot_hold_it`) names
  the gap permanently. The four standalone tests where the non-UTF-8 name *is* the property under
  test (`scanner_preserves_non_utf8_filename_identity_in_relative_path`,
  `a_non_utf8_name_is_asked_as_its_bytes_and_the_answer_lands_on_its_row`,
  `a_non_utf8_path_is_skipped_from_the_map_but_still_counted`,
  `a_non_utf8_filename_survives_the_round_trip`) are instead `#[cfg_attr(target_os = "macos",
  ignore = "...")]`: tolerating the write there would leave a test that passes while checking
  nothing. Checked by raw errno (`92`, observed on the runner's own log, not inferred from Darwin's
  ABI docs) rather than `libc` or `std::io::ErrorKind` (`io::Error::kind()` reports
  `Uncategorized` for this failure on the real toolchain, confirmed, not assumed) — the fixture is
  standard-library only on purpose, shared with the standalone
  `rfcs/handoffs/052-file-explorer/measurement` crate.

  **596's own item 6, same place for the same reason: two more Linux-only test assumptions, both
  fixed rather than gated away.** `setsid(1)` is a `util-linux` command, absent from macOS
  entirely (only the `setsid(2)` syscall is POSIX); `a_job_that_leaves_the_session_via_setsid_
  survives_a_real_close` now detaches via `perl`'s `POSIX::setsid()` instead, forking first the
  same way `setsid --fork` does (avoiding `EPERM` on a process already leading its own group) —
  portable, since `perl` ships by default on both platforms. `/dev/full` likewise does not exist on
  macOS; the two tests exercising a genuinely-unwritable transcript now go through one
  platform-split helper, `make_path_always_fail_writes` (`runtime/terminal/reader/tests.rs`): the
  Linux symlink unchanged, macOS a FIFO whose read end is opened non-blocking and dropped right
  after the real writer's own blocking `open()` succeeds, turning every subsequent write into
  `EPIPE`. Verified by a standalone probe reproducing the exact `BrokenPipe` error, and by
  temporarily exercising the macOS branch itself on this Linux machine (FIFO/`EPIPE` semantics are
  POSIX-portable, unlike `getsid`/`proc_listallpids` above) before it was proposed for review —
  the one item in this list that could be fully validated without a real macOS runner.

  **596's own item 5, and a correction to review 602's own hypothesis for it.** 602 proposed that
  `project::watch`'s real `PermissionDenied` failures on macOS might trace to D2's own `kqueue`
  choice: `notify`'s `kqueue` backend watches file descriptors (it must `open` every directory
  watched) where Linux's `inotify` watches paths directly, never opening them — a plausible reason
  a directory traversable-but-not-openable could surface differently across the two. Reading the
  actual test file first, not the kernel: all 14 of this file's `/dev/shm` literals (12 fixed; the
  other two are an operator's own documented manual setup for an `#[ignore]`d measurement) were a
  Linux-only tmpfs mount absent from macOS (`devfs` there holds device nodes only, no such mount).
  The real runner's own log names the exact mechanism, confirming the explanation rather than
  merely permitting it: `mkdir("/dev/shm")` there fails `EACCES`, not `ENOENT` — `/dev` exists and
  simply refuses a new directory, rather than a missing parent. 11 distinct tests failed this way,
  every one of them trying to **create a directory** before any kernel watch, `kqueue` or
  otherwise, was ever placed. The simpler cause was hiding behind the more interesting one. Fixed
  with one shared helper (`watch_test_root()`, `watch/tests.rs`): `/dev/shm` unchanged on Linux,
  `std::env::temp_dir()` elsewhere.

  **A second, real macOS-shaped failure the fix's own first draft would have introduced**, found
  by a Linux-runnable probe before it ever reached a real runner: `std::env::temp_dir()` on macOS
  is reached through a symlink (`$TMPDIR` under `/var` -> `/private/var`), and `watch_test_root()`
  returned it uncanonicalized. Forcing the same shape on Linux (the helper's own root hidden behind
  a symlink) made `ProjectRootValidator::validate(..., SymlinkPolicy::FailClosed)` refuse every
  project root four of this file's tests build under it, with `SymlinkAmbiguous` — its own selected
  and canonical paths disagree, exactly the ambiguity that policy exists to catch, not a bug in it.
  This is 596 item 3's own class (a `/tmp`-vs-`/private/tmp` trap, found three times in this RFC
  already), rediscovered here independently rather than inherited knowingly. Fixed by
  canonicalizing the helper's own output once, so every caller's root is already in the form the
  validator accepts.

  **The `kqueue`/`inotify` fd-semantics question itself stays open, not closed by this** — none of
  this file's fixtures happen to exercise a traversable-but-unopenable directory, so neither finding
  rules 602's own hypothesis in or out; together they only establish it was not needed to explain
  what the CI log actually showed. One piece of evidence against it, found for free: all 27 tests
  pass identically through the real `inotify` backend when `watch_test_root()` is forced to return
  an ordinary, non-tmpfs directory instead of `/dev/shm` (unsymlinked) — the watch mechanism itself
  does not depend on which kind of filesystem backs the watched directory, at least not on Linux.
  D2's `macos_kqueue` choice is not revisited by either finding; it would need a macOS-specific
  fixture deliberately exercising the fd-vs-path distinction to actually test the hypothesis, which
  nothing here does.
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

PR-028-C and PR-028-D may land in either order **at the code/decision level — found, PR-028-C,
2026-10-15: not at the test-execution level**. `cargo build --workspace` fails on macOS until every
one of D5's six mechanisms is ported (PR-028-A's own 11 errors are all D5's), so no macOS test —
including C's own `/dev/fd` TOCTOU check — can actually *run* until D5's sweep is done, even though
C's code can be written and reviewed first. PR-028-A and PR-028-B must land first; PR-028-E needs
PR-028-D's own macOS test run to check its collision sweep against, not just its own code.
