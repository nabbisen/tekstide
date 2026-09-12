---
title: "Dependency advisories: what we carry, why, and what retires it"
status: "Register. Opened 2026-09-12, before `0.18.0`'s cut. Reviewed every release."
rfc_file: "none — maintenance register, like `test-process-leak.md`"
target_milestone: "M12"
created: "2026-09-12"
---

# Dependency advisories

**Why this exists.** Until 2026-09-12 this project had **no dependency advisory scan and no step in
the release checklist for one**, through seventeen releases. The gap was found when the snora team
told five other teams — explicitly not us — that their own first scan had found two real
denial-of-service CVEs in `quick-xml`, reached through `iced` → `winit` → `wayland-scanner`. That is
**iced's graph, not snora's**, so it applied to us and we were not on the letter.

**We were already clean, by luck rather than by process.** The dependency-currency slice's
`cargo update` on 2026-09-10 had lifted `wayland-scanner` to `0.31.11` and `quick-xml` to `0.41.0`
— the fixed versions — two days before the advisory reached us. A routine maintenance slice closed a
vulnerability nobody here knew about. That is a good outcome and a bad process: nothing would have
told us if it had gone the other way.

## The gate

`cargo audit` (installed; no config file, defaults are correct). It **exits non-zero on a
vulnerability** and **warns** on `unmaintained` and `unsound` advisories.

**The pass condition is: zero vulnerabilities, and the warning list matches this register exactly.**
A warning that is not here is a finding — either a new advisory, or a dependency change that pulled
one in. A row here that no longer appears has been retired upstream and must be deleted, not left
to rot: a register that keeps rows past their own justification is the state-asserting-text failure
`ARCHITECTURE.md` records.

## What we carry, as of 2026-09-12 (`0.18.0`, 418 crates)

**Zero vulnerabilities.** Three warnings, all from `iced`'s graph, none of ours, none with a
published fix:

| Advisory | Crate | Class | Reached by | Reachability here |
| --- | --- | --- | --- | --- |
| `RUSTSEC-2024-0436` | `paste 1.0.15` | unmaintained | `metal` | **None on Linux.** `metal` is macOS-only; the crate is in `Cargo.lock` (which records every platform) and in no Linux build. Same shape as the 29 `windows*` crates. |
| `RUSTSEC-2026-0192` | `ttf-parser 0.25.1` | unmaintained | `fontdb`, `owned_ttf_parser` | Run time, font parsing on the default text path. |
| `RUSTSEC-2026-0253` | `lru 0.16.4` | **unsound** | `cryoglyph` → `iced_wgpu` → `iced_renderer` → `iced` | Run time, GPU text rendering. **Verified as a normal (non-dev, non-build) dependency of `tekstide` on Linux** with `cargo tree -e normal -i`. |

**`lru` cannot be cleared, and that is now confirmed upstream rather than inferred.** snora's
2026-09-12 correction states that `cryoglyph` holds it below the patched `0.18.2` and has no release
that lifts it, so neither they nor we can move it. **Verified here, not taken on their word:**
`cargo update -p lru --dry-run` locks **0 packages**. The retirement condition for this row is
therefore **a `cryoglyph` release that lifts its `lru` bound**, not merely "a published fix" — the
fix exists and is unreachable.

Reachability is also sharper than this register first recorded: the use-after-free needs a stored
key whose `Drop` **panics**, with unwinding enabled. That is not a realistic path for this
application, and it is memory corruption on a run-time path, which is why it stays named here and
is not folded in with the two unmaintained rows.

**`lru` is the one to watch, and it was not on snora's original list of three.** Its class is *unsound*, not
*unmaintained*: a potential use-after-free if a `Drop` implementation panics inside
`LruCache::pop()`. We do not call it — nothing in either crate depends on `lru` directly — so
reaching it requires `cryoglyph` to pop an entry whose drop panics during text rendering. That is
remote, and it is not nothing.

**Accepted, all three**, because none has a published upgrade and none is reachable by a path this
product controls. **Each retires when upstream fixes it**, which the gate detects by the row
disappearing.

## Our gate was checked against someone else's failure, and the reason it holds is worth keeping

snora's 2026-09-12 correction: their `cargo-deny` gate had reported *"three advisories, all
unmaintained"* while silently omitting three **unsound** ones, including `lru`. The cause was that
`cargo-deny`'s `unsound` setting is a **scope** (`all`/`workspace`/`transitive`/`none`), not a lint
level, and **its default excludes transitive dependencies** — and every crate in a library's graph
is transitive. An absent key behaving as a permissive default, which checking *values* would never
have caught.

**Checked here, and it does not apply — for a reason, not by luck.** This gate uses `cargo audit`,
where the classes are always reported and only the **exit code** varies: `--deny` controls what
*fails*, `--no-yanked` exists to *disable* a check that is otherwise on. Our own scan found `lru`
when theirs did not. The release checklist already says **read the warnings, do not trust the exit
code** — which is the same lesson, written before their letter arrived, and it is what makes this
gate sound rather than lucky.

**So the thing to protect is the absence of configuration.** Adding a `deny.toml` or an
`.cargo/audit.toml` to this repository would introduce exactly the class of mistake described above:
a file full of keys, one of which is missing, defaulting permissively and silently. **If a config
file is ever wanted here, it must come with an assertion that every advisory class is set
explicitly** — snora's own fix, which now refuses to run when a class is unset, because the failure
was never a wrong value but an absent key.

## What this register must not become

- **A place to silence a vulnerability.** Vulnerabilities are not accepted here; they are fixed, or
  the release does not go. This register is for `unmaintained` and `unsound` warnings only.
- **A list nobody reads.** The reachability column is the point. "It is transitive" is not a reason;
  *which build, which path, and can a user reach it* is.
