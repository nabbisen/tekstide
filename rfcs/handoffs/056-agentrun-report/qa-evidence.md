# RFC-056 — QA evidence

Written as each slice lands. Numbers are from this machine, 2026-09-25.

## PR-056-A — the pin, and two gate steps

### The defect, reproduced (against the registry, not a description)

`cargo install tekstide --version 0.25.0 --root <temporary> --target-dir <temporary>` — a full build:

```
  Installing tekstide v0.25.0
   Compiling tekstide-core v0.26.0
   Compiling tekstide v0.25.0
error[E0004]: non-exhaustive patterns: `FallbackReason::NotABoolean` not covered
error[E0004]: non-exhaustive patterns: `Some(FileGitStatus::Ignored)` not covered
error[E0004]: non-exhaustive patterns: `ExplorerTreeRowKind::IgnoredHidden { .. }` not covered
error: could not compile `tekstide` (bin "tekstide") due to 3 previous errors
```

**`0.25.0` resolved core `0.26.0`.** The same command with `--locked`:

```
  Installing tekstide v0.25.0
   Compiling tekstide-core v0.25.0
   Compiling tekstide v0.25.0
  Installed package `tekstide v0.25.0` (executable `tekstide`)
```

### A correction to the finding this slice was written from

The post-publish finding said the app archives' lockfiles name the **previous** core, so `--locked` does not rescue an old release. **Measured
against the registry, that is not so**: read with `curl` from `static.crates.io` and `tar`, the published `tekstide 0.24.0`, `0.25.0` and `0.26.0`
lockfiles name core `0.24.0`, `0.25.0` and `0.26.0` — the matching one — and `--locked` builds `0.25.0` (above). What named the previous core was the
**local** `target/package` archive, packaged before its core exists on the registry — the very difference `release-checklist.md` already explains
("`Cargo.lock` differs in the `tekstide` package, and the published one is the correct one"). So the defect is cause A alone (`version = "0"`), and
**`--locked` is a working remedy for an older release**; the `0.27.0` changelog says so, and `delivery-plan.md`'s record carries the correction.

### The pin

`[workspace.dependencies] tekstide-core = { version = "0.26.0", path = … }`, with a comment saying why and what fails if it drifts.
**Not `"0.27.0"` as the plan writes it, and that is deliberate:** a path dependency's version must be satisfied by the crate it points at, and
`tekstide-core` is `0.26.0` until the `0.27.0` candidate bumps it — `"0.27.0"` would not resolve. The property the plan wants (Cargo's `0.x` rule protects
every future release without anyone remembering) is held by a test instead:

`the_workspace_pins_tekstide_core_to_its_own_version` (`rfc_docs_invariants`) fails if the pin is `"0"` **or** differs from `[workspace.package] version`,
so the `0.27.0` candidate's version bump is a red test until the pin moves with it. The packaged manifest is checked directly:
`tar xzOf target/package/tekstide-0.26.0.crate tekstide-0.26.0/Cargo.toml` now reads `[dependencies.tekstide-core] version = "0.26.0"` (and the same for the
dev-dependency). `Cargo.lock` is unchanged.

### The post-publish check

`rfcs/handoffs/post-publish-check.sh <version> [--no-install]` — three checks against the registry: the app's `tekstide-core` requirement is the
release's own version (not `"0"`); the app archive's `Cargo.lock` names the matching core; and `cargo install tekstide --version <v>` into a temporary root
builds. `release-checklist.md` gains it as a post-publish step **for the release just published and the previous one**, with the reason (only an old app
shows a new core's break), and a scope line for the pin. Run against the published releases, structural checks only:

| Release | Requirement | Lockfile core | Result |
| --- | --- | --- | --- |
| `0.26.0` | `"0"` — **fails** | `0.26.0` — ok | FAIL, as it must: frozen metadata |
| `0.25.0` | `"0"` — **fails** | `0.25.0` — ok | FAIL, as it must |

The install step reproduces the `0.25.0` failure above.
