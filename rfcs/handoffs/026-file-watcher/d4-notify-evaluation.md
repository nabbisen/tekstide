# RFC-026 D4 — the `notify` evaluation, recorded before adoption

Status: **evaluation only. Nothing is added to `Cargo.toml` or `Cargo.lock`.** The decision to adopt
is the architect's, on this record. Evidence: `qa-evidence.md` § Slice B.

Candidate: `notify` **8.2.0**, read from its published source (`~/.cargo/registry`, fetched by
`cargo fetch` into a scratch project; the crate was read, not run).

## The three facts D12 asks for

| Fact | Value | Source | Verified here? |
| --- | --- | --- | --- |
| MSRV | **1.77** (`rust-version`) | `notify-8.2.0/Cargo.toml` | yes — under our 1.90 floor |
| Licence | **CC0-1.0** (`license`), `LICENSE-CC0` shipped in the crate | `Cargo.toml`, the crate's own files | yes — unlike every other direct dependency |
| Last stable release | **2025-08-03** (carried from RFC-026 D12) | RFC-026 D12 | **no** — the crates.io API is unreachable from this environment (HTTP 403), and the crate has no changelog. Carried, not re-checked. |

## What it adds to our dependency graph

On Linux, with default features, `cargo tree` lists **five new crates** and reuses five we already
have. The optional `crossbeam-channel` and `flume` are not enabled by default and are not in the tree.

| Crate | Version | Licence | Status in our lock |
| --- | --- | --- | --- |
| `notify` | 8.2.0 | CC0-1.0 | **new** |
| `notify-types` | 2.1.0 | MIT OR Apache-2.0 | **new** |
| `inotify` | 0.11.5 | ISC | **new** |
| `inotify-sys` | 0.1.8 | ISC | **new** |
| `mio` | 1.2.4 | MIT | **new** |
| `libc`, `log`, `walkdir`, `same-file`, `bitflags` | — | — | already present |

Each new crate's licence is in the same family as the existing set, except `notify` itself. The
advisories register takes all five rows at adoption, as RFC-026 D12 asks.

## What it does when the kernel refuses another watch

**Read from its source, not from its documentation** (`notify-8.2.0/src/inotify.rs`).

The kernel's refusal is `ENOSPC` from `inotify_add_watch`. `add_single_watch` maps it to
`ErrorKind::MaxFilesWatch` with the path attached, and the message is "OS file watch limit reached."

1. **A non-recursive `watch(path)` returns that error to the caller, synchronously.** The call goes
   over a reply channel (`EventLoopMsg::AddWatch`) and the error comes back as its result. The watch is
   **not recorded** on failure (`self.watches.insert` runs only on `Ok`), so there is no partial state
   to clean up. **This is the path we will use.**
2. **A recursive `watch(root, Recursive)` is not safe to refuse halfway.** It walks the tree and adds
   each directory with `?`, so the first `ENOSPC` returns an error with every earlier directory
   already watched. That is a scope we cannot account for. We do not use recursive mode.
3. **Event-driven adds** (a directory created under a recursive watch) report the limit to the event
   handler and then stop adding. Not our mode.
4. **Creating the inotify instance itself** can fail (the per-user instance limit is 1,024 here), and
   that fails at watcher construction, before any path is involved.

So, for our design, a refused watch is an ordinary `Err` returned from a call we make. It can be turned
into D2's sentence and the existing fallback without any special recovery.

## Two hazards the source shows, which the design must answer

**H1 — a panic on notify's thread is not contained.** The event loop has no `catch_unwind`. A panic in
the event handler kills the loop thread, and after that every `watch` and `unwatch` call panics in
*our* thread, at `channel.send(..).unwrap()` and `rx.recv().unwrap()`. The loop also panics outright on
a `poll(2)` failure (`panic!("poll failed")`). Consequences for B:

- The handler must not panic: it only forwards into an unbounded channel and ignores a send error.
- Our wrapper must catch a panic from a watch call and turn it into D2's "watching stopped" state,
  not let it reach the user as a crash. This is a requirement of the wrapper, not a hope about notify.

**H2 — the real refusal cannot be forced here.** The limit on this machine is `max_user_watches =
524288` (RFC-026 measurement 5), settable only by root and not something a test can lower. R3 says
the exhaustion must be forced in a test, so the design is:

- A `WatchBackend` trait with two implementations: the real notify one, and a fake that refuses on
  demand. The budget-exhaustion test runs against the fake.
- The real `ENOSPC` path is therefore **evidenced by reading the code above, not by a test**. Stated
  as such in the qa-evidence. A real-kernel check needs a machine with a lower limit, which is an
  environment we do not have.

## Other properties checked

- **Threads:** notify runs one event-loop thread per watcher. Events arrive through a handler we
  supply; with the default std channel or a closure, delivery does not block the loop on our side if
  the handler's send is unbounded. Matches D10: no render-thread work, no second threading model.
- **Unwatching an unknown path** returns `watch_not_found`, an `Err`, not a panic. Good for R6.
- **Removing a watch** does not walk the tree when non-recursive, so closing a project drops its
  watches in a bounded number of calls.

## Recommendation (the architect's decision, not taken here)

Adopt `notify` 8.2.0 with **default features off** (its default `macos_fsevent` is macOS-only and
irrelevant on Linux, and M14 owns other platforms), using **only non-recursive per-directory watches**,
behind a `WatchBackend` trait that holds both H1 and H2 answers. Record the five new crates in
`dependency-advisories.md` at adoption, and the unverified release date as a row to re-check.

The licence (CC0) is the one fact that differs from our set, and it is permissive, so it does not
change the decision. What *would* change it: a reviewer finding that the release date above is
materially older than stated, or a real-kernel refusal that does not match the source.
